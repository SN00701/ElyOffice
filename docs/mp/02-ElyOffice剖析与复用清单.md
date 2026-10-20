# ElyOffice 架构剖析与 MPelyoffice 复用清单

版本：0.1
日期：2026-10-20
对象：`https://github.com/ELY-Open-Source/ElyOffice`（MIT，436 star，453 commit，取样于 2026-10-20 的 `main`）
方法：对仓库目录树、关键源文件与打包脚本做一手取证（GitHub API + raw 文件），非二手转述。

---

## 0. 结论

ElyOffice 的真实形态不是"8 个独立软件"，而是：

> **1 套通用文档外壳（`crates/ely-office`）+ 8 个按格式特化的薄 app + 各自独立的引擎绑定。**

`crates/ely-office/src/lib.rs` 的首行注释即写着：
`//! The shell document apps share (§6), over any engine.`

这意味着 MPelyoffice 的本质工作**不是写 8 个软件，而是把"一个 app 一种引擎"的静态结构，
改造成"一个进程多引擎"的动态结构**，再补 Windows 集成层与统一打包层。
外壳、会话、保存、恢复、撤销、查找、缩放、对话框等横切能力**可以整体复用**。

---

## 1. 工程结构（一手取证）

```
ely-office/
  apps/           ely-agent ely-doc ely-mail ely-mark ely-pdf ely-show ely-table ely-tree
  crates/         ely-files ely-ipc ely-lok ely-mail-engine ely-office ely-pdf-engine ely-platform ely-ui
  assets/icons/   图标资源
  docs/           help.md compatibility.md accessibility.md perf/ licenses.md
  packaging/      apps.tsv types.tsv new/ templates/ libreoffice/ macos/ linux/ windows/
  patches/        libreoffice/ mupdf/
  scripts/        doctor.sh fetch-references.sh build-libreoffice.sh build-mupdf.sh
                  build-git.sh fetch-git-lfs.sh fetch-tessdata.sh apply-lo-patches.sh inert-lo.sh
  site/           官网
  tasks/          任务清单
  tests/          scripts/(macOS, Accessibility) perf/bench.py
  vendor/         gpui-pre/  gpui-pre-windows/
  reference/      外部上游源码检出目录（gitignore）
  engines/        lo-src 等引擎源码目录（gitignore）
```

**workspace 元数据**（`Cargo.toml` 一手取证）：
- `version = 0.1.9`，`edition = 2024`，`rust-version = 1.95`
- `rust-toolchain.toml` 固定 `channel = "1.96.0"`
- UI：`gpui` = `gpui-pre 0.3.8`（`[patch.crates-io]` 指向 `vendor/gpui-pre`）；
  平台层 `gpui-pre-platform 0.3.8`（features: font-kit / wayland / x11）
- 组件库：`ely-gpui-component` = git `ELY-Open-Source/Ely-GPUI-Components` rev `105bf948f1571bbd0765d3bf86bcfb0bab9eccb2`
- 关键第三方：`rfd 0.17`（文件对话框）、`notify 8`（文件监听）、`rusqlite 0.40`（bundled，本地库）、
  `windows 0.62` / `windows-sys 0.60`（含 `Win32_Graphics_Printing`、`Win32_Storage_Xps`、`Win32_System_WinRT`）、
  `lopdf 0.45`（PDF 写入）、`html5ever 0.39`、`tokio`、`rustls`、`quick-xml`、`image 0.25`

> `windows` crate 已启用 `Win32_Graphics_Printing` 与 `Win32_Storage_Xps`：
> **打印与 XPS 相关系统能力在依赖层已经就位**，MPelyoffice 不需要重新选型打印栈。

---

## 2. `crates/ely-office` —— 通用文档外壳（核心复用对象）

文件（`src/`，一手取证）：
`autosave.rs` `chart.rs` `chrome.rs` `close.rs` `dialogs.rs` `engine.rs` `files.rs` `find.rs`
`lib.rs` `lok.rs` `menu.rs` `open.rs` `panel.rs` `recover.rs` `save.rs` `stopped.rs` `zoom.rs`
+ 子目录 `recover/` `save/`

### 2.1 关键抽象（摘自 `lib.rs` 一手源码）

```rust
/// 一个 app 打开什么、写什么
pub struct Kind {
    pub app: AppId,
    pub kept: &'static [&'static str],      // 保存时保留的格式
    pub save_as: &'static [&'static str],   // 另存为可转换的格式，首项为默认
    pub macros: Option<&'static [&'static str]>,
}

/// 基于共享外壳构建的 app
pub trait Host: Sized + 'static {
    type Extra: Default;                    // 每个 app 自有的标签附加数据
    type Engine: Engine;                    // ← 静态关联类型：本次改造的核心靶点
    const KIND: Kind;
    fn core(&self) -> &Core<Self>;
    fn core_mut(&mut self) -> &mut Core<Self>;
    fn observed(&mut self, id: u64, view: &Entity<View<Self>>, cx: &mut Context<Self>);
    fn search_leaves(&mut self, _: &mut Context<Self>) {}
    fn quitting(&mut self, _: &mut Context<Self>) {}
    fn close_cancelled(&mut self, _: &mut Context<Self>) {}
    fn conflicted(&mut self, _id: u64, _: &mut Context<Self>) -> bool { false }
    fn tab_closed(&mut self, _tab: &Tab<Self::Extra, Self::Engine>, _: &mut Context<Self>) {}
    fn read_at(&self, _tab: u64, _cx: &App) -> Option<<Self::Engine as Engine>::Doc> { None }
}
```

`Core<H>` 持有的状态（**几乎就是 WPS 式外壳的全部内��**）：
`worker` / `sessions` / `tabs` / `active` / `loading` / `focus` / `root` / `work` / `next` / `untitled` /
`dialog` / `locked`（密码失败与副本队列）/ `window` / `waiting`（排队输入）/ `queued_saves` /
`finder` / `keeping`（恢复点）/ `recovery_off` / `stopped`（内核崩溃）/ `quiet`（自动保存静默计数）

`Tab<E, G>` 持有的状态：`id` `view` `doc` `session` `registry`（同文件注册表）/ `name` `modified` /
`risk`（`Macros`/`Signed`/`Repaired`）/ `mark`（撤销锚点）/ `work`（工作副本）/ `exporting` / `reopen` / `extra`

> `Tab::state_key()` 已经实现 ElyOffice PRD 的 **FILE-02**（加载中/已保存/未保存/保存中/只读/外部已修改/保存失败）
> 以及三种风险态（宏不可往返 / 签名被破坏 / 修复副本）。**这套状态机直接复用，无需重写。**

### 2.2 关键结论

| 事实 | 对 MPelyoffice 的含义 |
| --- | --- |
| `Host::Engine` 是**关联类型**（编译期定死一种引擎） | 一个进程只能承载一种引擎 → **无法在同一窗口混排 DOCX 与 PDF**。这是 M2 必须解决的唯一结构性障碍 |
| `Core<H>` 里 `tabs: Vec<Tab<H::Extra, H::Engine>>` 同理被引擎类型绑死 | 需把 `Tab` 的引擎参数换成**运行期多态**（trait object + `Extra` 类型擦除） |
| `Core` 已内建标签、会话、对话框、查找、恢复、崩溃态、自动保存、排队保存 | M1 只做**外壳皮肤**（Ribbon/标签栏/开始页），不必重写会话逻辑 |
| `Engine` trait 已抽象 `Doc`/`View`/`Worker` 三元组 | 多态化可沿此边界做，`Worker` 已按进程隔离设计（`crates/ely-lok/src/worker.rs` 17KB） |
| `Tabs` trait 提供 `toast` / `read_in` / `ask_in` / `when_settled` / `print_later` | 统一外壳的通知与后台读取可直接用 |

### 2.3 复用评级

- **直接复用**：`Core` 的会话/恢复/查找/缩放/关闭/保存状态机、`Tab`、`Kind`、`Risk`、`Dialogs`、`Chrome`、`Panel`、`Commands`
- **改造复用**：`Host` 的引擎泛型 → `EngineDyn`；`Tab` 的 `Extra` → `Box<dyn Any>`
- **不适用**：`crates/ely-office/src/lok.rs`（LOK 专用命令表，改为按引擎注册命令）

---

## 3. `crates/ely-lok` —— LibreOfficeKit 接入层（完整存在）

文件与体积（一手取证）：`client.rs` 17KB、`client/`、`commands.rs` 13KB、`commands/`、`events.rs` 15KB、
`events/`、`ffi.rs` 12KB、`ffi/`、`handshake.rs` 7KB、`history.rs`、`ipc.rs` 4KB、`lib.rs`、`menu.rs` 5KB、
`review.rs` 8KB、`sheet.rs` 16KB、`show.rs`、`table.rs` 8KB、`view.rs` 16KB、`worker.rs` 18KB、`bin/`

**读法**：LOK 接入已被拆成"进程外 worker（`worker.rs` + `ipc.rs`）+ 协议（`events.rs`/`commands.rs`）+ FFI（`ffi.rs`）+
客户端（`client.rs`）+ 领域视图（`sheet.rs`/`table.rs`/`show.rs`/`review.rs`）"五层。
这正好满足 ElyOffice PRD §6.2「LOK 与 MuPDF 放在本地工作进程中，避免原生库故障直接终止整个界面」。

> **对 MPelyoffice 的意义**：多标签 + 多引擎时，**每个引擎一个 worker 进程**的隔离模型天然成立，
> 且崩溃隔离已由上游设计好（`Core::stopped` 字段就是为此存在）。

## 4. `crates/ely-pdf-engine` —— PDF 引擎（MuPDF 接触面清单）

文件（一手取证）：`client.rs` 11KB、`content.rs` 6KB、`edit.rs` 3KB、`lib.rs`、`marks.rs` 9KB、
`ocr.rs` 3KB、`output.rs` 3KB、`proto.rs` 7KB、`redact.rs` 3KB、`sample.rs` 13KB、`secure.rs` 1.5KB、
`sign.rs` 5KB、`structure.rs` 8KB、`bin/`

**按依赖关系分层**：
- **渲染 / 页面 / 文本层（与 MuPDF 强绑定）**：`client.rs`（MuPDF FFI 主体）、`sample.rs`（页面采样渲染）
- **写入层（可保留）**：`edit.rs`、`marks.rs`、`redact.rs`、`sign.rs`、`structure.rs`、`secure.rs`、`output.rs`
  —— 依赖 `lopdf 0.45`（MIT），与渲染内核无关
- **OCR**：`ocr.rs`（Tesseract，Apache-2.0，**许可干净，可保留**）
- **协议**：`proto.rs`（IPC，与内核无关，保留）

> **MuPDF → PDFium 替换面（ADR-002 的落地范围）**：
> 主要落在 `client.rs` + `sample.rs` 两个文件 + 新增 PDFium FFI 模块；
> `marks.rs`/`redact.rs`/`sign.rs`/`structure.rs`/`secure.rs`/`output.rs` 全部保留。
> 预计改动 **2 个文件 + 1 个新增模块**，而非重写整个引擎。

## 5. 其他 crates 与 8 个 app

| crate / app | 定位 | MPelyoffice 处置 |
| --- | --- | --- |
| `crates/ely-files` | `Session`/`Sessions`/`SaveState`（保存状态机、会话注册表） | **直接复用** |
| `crates/ely-ipc` | 进程间通信原语 | **直接复用** |
| `crates/ely-platform` | 文件对话框、打印（`print_pdf`）、凭据、`AppId`、系统集成 | **直接复用**，Windows 部分需核对 |
| `crates/ely-ui` | 组件库的应用级组合（`Root`、`i18n::text`） | **直接复用 + 扩展**（新增 Ribbon 组合层） |
| `crates/ely-mail-engine` | 邮件协议栈（`imap-proto`/`mail-send`/`mail-parser`） | 首版不使用 |
| `apps/ely-doc` | 文字模块薄壳 | **并入** `apps/mpelyoffice` |
| `apps/ely-table` | 表格模块薄壳 | **并入** |
| `apps/ely-show` | 演示模块薄壳 | **并入** |
| `apps/ely-pdf` | PDF 模块薄壳 | **并入**（内核换 PDFium） |
| `apps/ely-mark` | Markdown 模块薄壳 | **并入** |
| `apps/ely-tree` | Git 客户端 | 首版不做（二期） |
| `apps/ely-mail` | 邮件客户端 | 首版不做（二期） |
| `apps/ely-agent` | Agent 会话管理 | 首版不做（二期） |

## 6. `vendor/` —— GPUI 在 Windows 的落点

| 目录 | 内容（一手取证） | 许可 |
| --- | --- | --- |
| `vendor/gpui-pre` | GPUI 主体（workspace `[patch.crates-io]` 指向它） | 见其 LICENSE |
| `vendor/gpui-pre-windows` | **完整的 Windows 移植 crate**：`Cargo.toml`(6KB)、`Cargo.toml.orig`、`build.rs`(7.7KB)、`src/`、`Cargo.lock`(123KB)、`LICENSE-APACHE` | Apache-2.0 |

> `gpui-pre-windows` 是带 `build.rs` 的**独立可编译 crate**，不是补丁片段。
> 这使 **GPUI-on-Windows 成为上游一等路径**，而不是需要我们从零移植。
> 其 `LICENSE-APACHE` 满足本项目许可红线。

## 7. 组件库 `ELY-Open-Source/Ely-GPUI-Components`（rev `105bf948`）

`src/` 一级目录（30 个）：`account` `agent` `buttons` `calendar` `canvas` `charts` `chat` `collab`
`dashboard` `data_display` `debug` `devtools` `documents` `editor` `feedback` `files` `finance` `forms`
`generative` `git` `i18n` `interaction` `layout` `lists` `mail` `maps` `media` `menus` `messaging`
`misc` `motion` `navigation` `onboarding` `overlays` `primitives` `project` `rendering` `settings`
`shell` `tables` `terminal` `theme` `tooling` `typography`

### 7.1 `src/shell/` —— 外壳组件（本项目最相关）

| 文件 | 大小 | 用途推断 |
| --- | --- | --- |
| `tabs.rs` | 8.4KB | **文档标签栏**（WPS 式混排标签的基础） |
| `bars.rs` | 7.2KB | 工具栏 / 菜单栏 |
| `status.rs` | 10.5KB | **状态栏** |
| `chrome.rs` | 15KB | 外框（标题栏/边框） |
| `frame.rs` | 5.9KB | 容器框架 |
| `columns.rs` | 14KB | 分栏布局 |
| `activity.rs` | 5.6KB | 活动指示 |
| `switcher.rs` | 6KB | 切换器 |
| `dialogs.rs` | 8.1KB | 对话框 |
| `windows.rs` | 9.8KB | **Windows 专项窗口处理** |
| `native.rs` | 7.7KB | 原生窗口集成 |
| `vibrancy.rs` | 1KB | 窗口透明/材质（macOS 为主） |
| `update.rs` | 8.8KB | 更新流程 |
| `crash.rs` | 5.7KB | 崩溃处理 |
| `tests.rs` | 17.4KB + `tests/` | 已有测试资产 |

> **关键缺口**：组件库**没有 ribbon 模块**。WPS 式功能区（Ribbon）需在
> `crates/ely-ui` 内**自建组合层**，基于 `bars.rs` + `buttons` + `icons` + `theme` 令牌。
> 这是 M1 的主要新增工作量，必须单列估算。

## 8. `packaging/` —— 现有打包与关联注册

### 8.1 `packaging/types.tsv`（一手全文，扩展名 → app + MIME）

```
ely-doc    docx   application/vnd.openxmlformats-officedocument.wordprocessingml.document
ely-doc    odt    application/vnd.oasis.opendocument.text
ely-doc    doc    application/msword
ely-doc    rtf    application/rtf
ely-table  xlsx   application/vnd.openxmlformats-officedocument.spreadsheetml.sheet
ely-table  ods    application/vnd.oasis.opendocument.spreadsheet
ely-table  xls    application/vnd.ms-excel
ely-table  csv    text/csv
ely-table  tsv    text/tab-separated-values
ely-show   pptx   application/vnd.openxmlformats-officedocument.presentationml.presentation
ely-show   odp    application/vnd.oasis.opendocument.presentation
ely-show   ppt    application/vnd.ms-powerpoint
ely-pdf    pdf    application/pdf
ely-mark   md     text/markdown
ely-mark   markdown text/markdown
ely-tree   /      inode/directory
```

> **缺口**：**无 `.txt`**。MPelyoffice 需新增 `text` 引擎与 `txt` 映射（另可考虑 `.log`/`.ini` 归入文本）。
> `/`（目录）映射到 ely-tree 是 Git 客户端的"打开文件夹"，首版**移除**（不实现 Git）。

### 8.2 `packaging/windows/types.sh`（一手全文要点）—— 关联注册范式

已做的正确动作：
- ProgID 命名 `ElyOffice.$name`
- 只写 `HKCU\Software\Classes`（**按用户，免管理员**）
- 只写 `<ext>\OpenWithProgids`（**进"打开方式"列表，不夺默认**）
- 写 `Applications\<exe>\SupportedTypes` 与 `FriendlyAppName`
- `ShellNew` 只在"该扩展名当前没有处理器"时才接管默认（读 `HKCR\.<ext>` 为空才写）
- 卸载时**只删自己写的值**，并判断 `HKCR\.<ext>` 当前值是否仍指向自己才删
- 注册后调用 `shell32::SHChangeNotify(0x08000000, ...)` 通知资源管理器

> **结论**：ElyOffice 的关联范式**与本项目 §R4 完全一致**，可直接继承并扩展为
> `MPelyoffice.Document.1` / `.Spreadsheet.1` / `.Presentation.1` / `.Pdf.1` / `.Markdown.1` / `.Text.1`。

### 8.3 现有打包形态

`packaging/windows/`：`installer.nsi`(10.7KB)、`bundle.nsi`(1.4KB)、`files.sh`、`icon.rs`、
`templates.sh`、`types.sh`、`mupdf-parts.py`、`new/`（ShellNew 模板文件）。
→ **NSIS 按 app 分别打包**，与用户诉求（单一安装包）正相反，**需重写为单一安装器**。

## 9. `scripts/` —— 上游构建链

| 脚本 | 作用 | MPelyoffice 处置 |
| --- | --- | --- |
| `fetch-references.sh` | **稀疏检出**固定 commit 的上游源码 | **直接复用**，新增 PDFium 条目 |
| `build-libreoffice.sh` | 构建带 LOK 补丁的 LO | **移植为 Windows 版**（脚本已有 `CYGWIN*` 分支） |
| `build-mupdf.sh` | 构建 MuPDF | **废弃**（AGPL） |
| `build-git.sh` / `fetch-git-lfs.sh` | 构建 Git | 首版不需要（二期 Git 客户端） |
| `fetch-tessdata.sh` | 拉取 OCR 语言包 | **复用** |
| `doctor.sh` | 检查工具链版本 | **移植为 Windows 版** |
| `apply-lo-patches.sh` | 应用 LOK 补丁 | **复用** |

### 9.1 `fetch-references.sh` 的固定版本（一手取证）

| 名称 | 仓库 | 固定 commit | 稀疏路径 |
| --- | --- | --- | --- |
| `Ely-GPUI-Components` | ELY-Open-Source/Ely-GPUI-Components | `105bf948f1571bbd0765d3bf86bcfb0bab9eccb2` | 全量 |
| `zed` | zed-industries/zed | `1a28cff4b409169bac058bca40dfbfeb7621d19b` | 全量 |
| `libreoffice-core` | LibreOffice/core | `8221e31b3ac356a1623c672912a3d2b492f7e3d1` | `include/LibreOfficeKit` `libreofficekit` `desktop/source/lib` `desktop/inc` `officecfg/registry/data/org/openoffice/Office/UI` |
| `mupdf` | ArtifexSoftware/mupdf | `8ad45e92f0935d3d87f1db3f873086472a5e1b24` | 全量 |
| `git` | git/git | `a018953688f1b10bddf91bff8747068f5f4746a4` | 全量 |
| `tesseract` | tesseract-ocr/tesseract | `db0ec62f81b0737fbbe184d8fea40af5738f8eef` | 全量 |
| `marktext` / `colamd` / `markion` / `magpie` / `omarchy` | 对标与参考 | 各自固定 commit | 稀疏 |
| `markdown-rs` | wooorm/markdown-rs | `1506572f9b406431402928f3a8b3df0b4ae2d8f5` | 全量 |
| `agent-client-protocol` | agentclientprotocol/agent-client-protocol | `9e032156545412be9bba5e092f12d0080c499b6d` | 全量 |

> **重大发现**：LibreOffice 采用**稀疏检出**（仅 5 个路径），不是全量源码。
> 这把"Windows 上从源码构建 LOK"的成本从"数天"压缩到"数小时"量级，
> 是 M0 尖峰 B 得以成立的前提。

### 9.2 `build-libreoffice.sh` 的 Windows 分支（一手取证）

脚本已含 `CYGWIN*` 分支，其配置差异：
```
--with-visual-studio=2022 --disable-ccache --disable-cli --without-dotnet
# 并剔除 Unix-only 开关（--disable-cups/--disable-evolution2/--disable-python 等）
# unset MSYSTEM MSYSTEM_PREFIX MINGW_PREFIX MSYSTEM_CHOST MSYSTEM_CARCH
# PATH=/opt/lo/bin:$PATH
```
公共开关包含：`--enable-mergelibs --enable-release-build --without-java --without-help --disable-odk
--disable-online-update --disable-scripting-beanshell --disable-scripting-javascript --disable-python
--disable-firebird-sdbc --disable-postgresql-sdbc --with-lang= --with-theme=colibre --with-fonts
--without-doxygen --with-myspell-dicts` 等。
构建后处理：只保留 `dict-en` 词典、剥离 Lightproof（Python，Windows 上会挂起）、注入 `ely.xcd` 注册表默认值。

> **MPelyoffice 需要做的**：① `--with-visual-studio` 改 2026（本机 VS 18.6，MSVC 14.44/14.51）；
> ② Cygwin 安装到**无空格**目录（建议 `F:\cygwin64`），构建目录 `F:\mpely-lo`；
> ③ 保留 Lightproof 剥离步骤（上游已踩过 Windows 挂起坑）。

## 10. 上游 PRD 的可继承条款

ElyOffice `PRD.md`（v1.0，2026-10-01，一手取证）含 16 章，编号化验收条目。本项目继承其中：

| 组 | 编号 | 继承情况 |
| --- | --- | --- |
| 通用 UX | UX-01..UX-12 | **全盘继承**（命名统一、快捷键语义、Tab 顺序、禁用原因、混合选择显示"混合"、长任务进度、失败保留输入、危险操作说明具体对象、中英双语、IME、缩放、不仅靠颜色、辅助技术） |
| 文件/保存 | FILE-01..FILE-10 | **全盘继承**（含原子替换、外部修改、修订号、导出不改格式、路径健壮） |
| LOK 接入 | LOK-01..LOK-12 | **继承**，并按 Windows 场景细化（LOK-01 隔离用户配置、LOK-07 IME、LOK-08 上游对话框不得泄露、LOK-11 不执行宏） |
| 各模块 | DOC-01..16 / TABLE-01..16 / SHOW-01..14 / PDF-01..14 / MARK-01..16 | **继承为支持矩阵的行**（首版按能力标注，不足则明示） |
| 边界 | §3.2 / §4.2 / §16 | **继承**：不建云、不自研排版/公式/PDF 解析、用户文件通用格式、应用可独立启动更新卸载 → **本项目改为单一安装包，故该条按本项目 R1 覆盖** |

## 11. 复用矩阵（总表）

| 资产 | 判定 | 动作 |
| --- | --- | --- |
| `crates/ely-office` 会话/保存/恢复/查找/缩放/关闭 | ★★★★★ | 原样复用，仅动 `Host`/`Tab` 的引擎泛型 |
| `crates/ely-files` | ★★★★★ | 原样复用 |
| `crates/ely-platform` | ★★★★☆ | 原样复用，核对 Windows 打印/对话框 |
| `crates/ely-ipc` | ★★★★★ | 原样复用（多引擎 worker 的底座） |
| `crates/ely-lok` | ★★★★★ | 原样复用（DOCX/XLSX/PPTX 引擎） |
| `crates/ely-pdf-engine` 的写层/OCR/协议 | ★★★★☆ | 原样复用 |
| `crates/ely-pdf-engine` 的 `client.rs`/`sample.rs` | ★★☆☆☆ | **替换为 PDFium** |
| `crates/ely-ui` | ★★★★☆ | 复用 + 新增 Ribbon 组合层 |
| `vendor/gpui-pre-windows` | ★★★★★ | 原样复用 |
| 组件库 `src/shell/tabs.rs` 等 | ★★★★☆ | 复用；Ribbon 需自建 |
| `packaging/windows/types.sh` 关联范式 | ★★★★★ | 继承范式，改 ProgID 前缀与清单 |
| `scripts/fetch-references.sh` | ★★★★★ | 复用，新增 PDFium |
| `scripts/build-libreoffice.sh` | ★★★★☆ | 移植 Windows 版（已有 Cygwin 分支） |
| `packaging/windows/installer.nsi` | ★★☆☆☆ | **重写**为单一安装器 |
| `crates/ely-mail-engine` / `apps/ely-mail` / `ely-tree` / `ely-agent` | ★☆☆☆☆ | 首版不实现 |
| MuPDF 依赖与 `scripts/build-mupdf.sh` / `patches/mupdf` | ☆☆☆☆☆ | **移除**（AGPL） |

## 12. 上游的 Windows 实战记录（来自仓库 `AGENTS.md`，一手取证）

`AGENTS.md` 是上游的工程日志（"单一事实来源"）。它记录了 **2026-10-10 至 2026-10-20 期间上游已在 Windows 上完成构建与修复**，
这使 MPelyoffice 的"Windows 落地"从探索变为**复用**。关键条目摘录：

| 主题 | 上游已验证的事实 | 对 MPelyoffice 的含义 |
| --- | --- | --- |
| 构建环境（E-01，2026-10-10） | Windows：MSVC（VS 2022，v143）；仓库与 `reference/` 一律 LF（`core.autocrlf false`、`core.eol lf`）；`%TEMP%` 下的程序被系统应用控制拦下，构建与探针都不放那里；cmake 与 ninja 须在 PATH；LibreOffice 的 configure 只认 VS 自带的 CMake | 本机 MSVC 14.44/14.51 + Windows SDK 10.0.26100 齐备；需注意 `VsDevCmd` 需带入 cmake 组件 |
| LOK on Windows（W-01） | **LibreOffice 在 Cygwin 下构建**（`build-libreoffice.sh` 的 `CYGWIN` 分支：去 Unix 专用开关、加 VS 2022、用 `/opt/lo/bin` 的 make、清掉 Git Bash 带进来的 `MSYSTEM`）；构建目录与 Cygwin 须排除在 Defender 扫描外（否则并行 configure 删不掉 `conftest.dir`）；worker 以 unipoll 模式跑，主线程是 Windows 消息循环，请求经只收消息的窗口投递（`worker/messages.rs`）——原先 LOK 自起主线程会**载入表格即死锁** | M0-B 的具体做法已由上游写明；**Defender 排除目录是硬前置** |
| LOK 图块（2026-10-11） | Win32 虚拟设备没有 `SetSizeUsingBuffer`，给客户缓冲区的设备停在 1×1 像素 → 表格只画一个像素（全黑无网格线）；补丁 `0044-lok-win32-paints-then-copies.patch` 让 Windows 先按全尺寸画进设备再拷进客户缓冲区 | 该补丁**必须包含**在 Windows 构建里 |
| GPUI Windows 缺陷（2026-10-11） | `gpui-pre-windows` 0.3.8 关窗时报三条错（销毁后仍 `RevokeDragDrop`/`DestroyWindow`、失活回报排在异步任务、退出后向已销毁窗口投递）；**标题栏最小化/最大化/关闭三个按钮按不动**（`WM_NCHITTEST` 按绘制次序取第一个含指针的窗口控制区，标题栏拖动区先于按钮画出）→ 修复在 `vendor/gpui-pre` | 上游已在 vendor 里修；**不要回退 vendor 补丁** |
| 中文排版（R4.13，2026-10-14） | Windows 上 gpui 以用户区域排版，DirectWrite 按日文为汉字选回退字体 → 同一词里字形与粗细不一；修复：`vendor/gpui-pre-windows` 的 `text_locale` 改用 Windows 语言列表里第一个中日韩语言排版；Ely 界面语言为中文时经 `gpui_windows::set_text_language` 设置 | 中文字体一致性**已有解**，需保证补丁在 |
| 中文 IME 热键冲突（R4.11，2026-10-15） | Windows 中文输入法把 `Ctrl+Shift+0` 留作热键，应用收不到 → 另绑 `Ctrl+Shift+9` | 快捷键表须避开 `Ctrl+Shift+0` |
| 控制台窗口（2026-10-11） | 发行构建须 `windows_subsystem = "windows"`；子进程（两个 worker、Git）一律经 `ely_ipc::windowless` 以 `CREATE_NO_WINDOW` 启动 | MPelyoffice 的发行构建照此 |
| 单实例（X-03，2026-10-09） | `ely_ui::run` 取应用数据目录的 `instance.lock`（Windows 为 `LockFileEx`）；持锁者在 `127.0.0.1` 开一个**门**（`instance::Door`，随机端口，端口+进程号+32 字节随机令牌写在 `instance.door`）；第二份先按当前目录把启动参数变成绝对路径，连上后先发令牌，门答"开/拒"之后才发各路径，收到回执即以 0 退出；**Windows 上第二份先 `AllowSetForegroundWindow`**；门未开时 10 秒内重试，路径发出后不重试 | **单实例与文件交接在 Windows 上已实现且被真窗口核对过**（第二份 132 毫秒退出，第一份多一个标签）。MPelyoffice 直接复用，**不必自造命名管道** |
| 合集安装程序（R7.1，2026-10-16） | Windows 出一个 `ElyOffice-<版本>-windows-x86_64-setup.exe`（`packaging/windows/bundle.nsi`）：组件页勾选应用，所选的各自的安装程序以 `/S` 运行，装法/位置/升级/卸载项与单装完全相同；某个退出码非零即停下并写明其名与退出码 | **"一个包装全部"的骨架已存在**，MPelyoffice 在其上改为单 exe |
| 单应用安装器健壮性 | `packaging/windows/installer.nsi`：装好后写下所装的每个文件与文件夹（`.ely-installed`），升级与卸载只移去其中的文件；先查每个文件能否写（运行中或被占用即不能），再逐个挪进 `.ely-old`，任一项挪不动即放回、什么也不改、退出码 2；写到一半失败即删去本次写下的、放回旧版 | 直接继承该事务语义 |
| 文件关联（R7.9，2026-10-16） | `ShellNew` 只在**该扩展名没有任何程序处理时**才设默认值（写在 HKCU，用户自己的 `UserChoice` 不动）；ProgID 名为"<应用> Document" | 与本项目 §R4 完全一致 |
| 许可证现状（2026-10-19） | **自有代码由 AGPL-3.0-or-later 改为 MIT**（`LICENSE`：Copyright (c) 2026 Yang Zhang）；第三方照旧：LibreOffice 与 `patches/libreoffice` 为 MPL-2.0 等，**Git 为 GPL-2.0，MuPDF 为 AGPL-3.0**；ElyPDF 链接 MuPDF，其发行包整体仍须按 AGPL 履行 | 上游 fork **无许可障碍**；但"带 MuPDF 的发行包整体按 AGPL 履行"这条路对我们不适用（见 ADR-002） |
| 首版发布范围（2026-10-08） | 首版发布六个应用（ElyDoc、ElyTable、ElyShow、ElyPDF、ElyTree、ElyMark），ElyMail 与 ElyAgent 第二批 | 与本项目范围基本一致；差异是我们把 ElyTree 换成 Text 引擎 |
| 性能基线 | 空白文档就绪 ~1.0 秒（窗口 0.15 秒可操作）；键入到新图块 ElyDoc 中位 19 毫秒、ElyTable 3 毫秒；100 轮编辑后应用进程 340–440 MB | MPelyoffice 的验收目标可直接对标 |

### 12.1 组件映射表（上游 AGENTS.md 的"需要 → 用"映射，原样摘录）

| 需要 | 组件库导出 |
| --- | --- |
| 窗口外壳 | `shell::{TitleBar, TabBar, StatusBar, Toolbar, ToolbarGroup, WindowSwitcher, ZoomControl}`、`layout::{AppShell, SplitPane, Sidebar, Dock}` |
| 命令与菜单 | `menus::{MenuBar, ContextMenu, DropdownMenu}`、`navigation::{CommandPalette, QuickOpen}` |
| 导航 | `navigation::{EditorTabs, Tabs, Breadcrumb, TableOfContents}`、`lists::{Tree, Outline, FileTree, SelectableList}` |
| 属性与表单 | `data_display::{PropertyGrid, DescriptionList}`、`forms::{Select, FontPicker, ColorPicker, NumberInput, ScrubInput, Switch, SearchInput, TextInput}`、`buttons::{Button, IconButton, ToggleButton, SegmentedControl}` |
| 对话框 | `overlays::{Dialog, ConfirmDialog, AlertDialog, PromptDialog, Popover}` |
| 反馈 | `feedback::{Toaster, ToastViewport, SaveState, SavingIndicator, SyncStatus, Alert, Banner, EmptyState, ErrorView}`、`shell::{OfflineIndicator, CrashReporter, UpdateBanner, UpdateDialog}` |
| 设置 | `settings::{SettingsLayout, SettingsSection, SettingsRow, ThemeSelector, KeyboardShortcutsList, StorageSettings, PrivacySettings}` |
| 文档 | `documents::{PrintPreview, TemplatePicker, WordCount, PageThumbnailList}`、`editor::FindWidget`、`collab::{CommentSidebar, Thread, Comment, Peer}`、`lists::{List, ListItem}` |
| 表格 | `tables::{PivotTable, FilterBuilder, SortBuilder}`、`misc::{CsvImporter, ExportDialog}`、`editor::{CompletionMenu, SignatureHelp}`、`forms::CheckboxGroup` |
| Markdown | `documents::{MarkdownDocument, MarkdownRenderer, ZenMode, MarkdownReader, MarkdownBlock, alert, is_toc, BlockCommand, InlineFormat, TableEdit, ...}`、`editor::CodeEditor` |
| 动效 / 主题 | `motion::{duration, Flip, Reorder, Transition, Stagger, ProgressBar, Spinner, Skeleton}`、`theme::{Theme, Mode}` |

> **`shell::TabBar` + `navigation::Tabs` + `navigation::EditorTabs` 已存在** → WPS 式混排标签栏的组件基础齐备；
> **无 ribbon 模块** → 功能区须以 `shell::Toolbar` + `ToolbarGroup` + `navigation::Tabs` 组合自建（见 ADR-013）。

### 12.2 上游记录的项目内建缺口（MPelyoffice 需一并承接）

| 缺口 | 上游位置 |
| --- | --- |
| LOK 图块画布与输入桥 | `crates/ely-lok` |
| 富剪贴板（gpui 只有文本与图片，无 HTML） | `ely_platform::clipboard`（用 arboard） |
| gpui 无障碍无文本选区与插入点 | `LokView` 只桥接当前段落文本 |
| 选中对象的框与手柄（LOK 不画进图块） | `LokView` 自绘 8 个手柄 |
| PDF 页面虚拟化栈 | `crates/ely-pdf-engine`、`apps/ely-pdf` |
| 表格公式栏与名称框 | `apps/ely-table` |
| 幻灯片缩略图条 / PDF 页面缩略图 | `apps/ely-show/src/slides.rs`、`apps/ely-pdf/src/side.rs` |
| 演示参考线（LOK 不画） | 补丁 `0027` + `apps/ely-show/src/guides.rs` |
| 外壳左右侧栏（可拖宽、宽度持久化） | `crates/ely-ui/src/shell/panel.rs`（有单测） |
| 窄窗口版面（内容区至少 360，先藏右栏后藏左栏） | 同上 `panel.rs` 的 `room` |

---

## 13. 剖析阶段发现的风险

| 编号 | 风险 | 证据 | 应对 |
| --- | --- | --- | --- |
| RK-1 | Windows 打包链从未跑通 | README 自述 "Linux and Windows recipes are in `packaging/linux` and `packaging/windows`; **they have not been run on those systems**" | M0 尖峰 B 优先验证；失败则启用降级引擎 |
| RK-2 | LOK 编辑 API 上游标注 unstable | 官方文档需 `LOK_USE_UNSTABLE_API` 且仅 2 个已知编辑客户端 | 锁定版本 + 维护回归测试（LOK-12 要求） |
| RK-3 | GPUI Windows 分支未经验证 | 上游仅提供 vendored crate，无 Windows 构建说明 | M0 尖峰 A 优先 |
| RK-4 | `rust-toolchain.toml` 固定 1.96.0，本机只有 1.95.0/1.98.1 | 一手取证 | 首次构建自动拉取 1.96.0（`static.rust-lang.org` 已验证可达）；不可达则改钉 1.98.1 |
| RK-5 | 仓库位于含空格路径 `E:\trae pg\` | 本机环境 | Rust 构建允许；**LO 构建与 Cygwin 一律放 `F:\` 无空格路径** |
| RK-6 | 上游 `apps/*` 与 `crates/ely-office` 强耦合于 `AppId`/每 app 目录结构 | 源码结构 | 新建 `apps/mpelyoffice` 作为**唯一**宿主，保留旧 app 一段时间以便回归对照 |
| RK-7 | MuPDF 已是 workspace 依赖 | `Cargo.toml` 含 `lopdf`，`fetch-references.sh` 含 mupdf | 移除 MuPDF 后需保证 `ely-pdf-engine` 仍能编译（先做接口隔离） |
