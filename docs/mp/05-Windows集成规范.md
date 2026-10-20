# MPelyoffice Windows 集成规范

版本：0.1
日期：2026-10-20
对应 ADR：ADR-006（per-user + 永不夺默认）、ADR-007（分发）
上游依据：`packaging/windows/types.sh`、`packaging/windows/installer.nsi`、`packaging/windows/bundle.nsi`、
`crates/ely-platform/src/instance.rs`、`crates/ely-ui/src/...`（`ely_ui::run` 的启动参数与打开请求）

---

## 1. 目标

在不污染用户环境、不要求管理员权限的前提下，让 MPelyoffice 做到：
双击受支持文件 → 只启动一个进程 → 文件落在已有窗口的新标签；卸载后系统无残留。

## 2. 安装

### 2.1 模式

| 模式 | 路径 | 注册表 | 提权 | 说明 |
| --- | --- | --- | --- | --- |
| **按用户（默认）** | `%LOCALAPPDATA%\Programs\MPelyoffice` | `HKCU\Software\Classes` | 否 | 免管理员；现代 Windows 推荐的 per-user 路径 |
| 按机器（可选） | `%ProgramFiles%\MPelyoffice` | `HKLM\Software\Classes` | 是 | 企业部署用，配合 WiX MSI 交付 |

> 本机无管理员权限，故开发期只验证按用户模式；按机器模式靠 MSI 交付与 CI 验证。

### 2.2 安装器职责

| 步骤 | 行为 | 失败处理 |
| --- | --- | --- |
| 1 | 校验磁盘空间、已有安装 | 空间不足即中止并写明缺多少 |
| 2 | 写 `.ely-installed` 清单（每个文件与目录） | 写不进即中止 |
| 3 | 复制程序文件 | 逐文件先查"能否写"（占用/只读即拒），任一不行则**整体中止**，旧版原样保留，退出码 2 |
| 4 | 注册文件关联（§3） | 注册失败回滚程序文件 |
| 5 | 建快捷方式、开始菜单项、卸载项（ARP） | 失败不致命，记日志 |
| 6 | 写版本与安装记录 | — |

**升级语义**（继承上游 `installer.nsi`）：先把旧文件逐个挪进 `.ely-old`（编号顺序，备份路径不随原路径变长），
任一项挪不动即放回、什么也不改、退出码 2；写到一半失败即删去本次写下的、放回旧版。
**卸载语义**：只移去 `.ely-installed` 清单中的文件；目录空了才删；**用户文档永不触碰**。

### 2.3 参数

| 参数 | 行为 |
| --- | --- |
| `/silent` 或 `/S` | 静默安装（企业用） |
| `/dir=<路径>` | 安装目录 |
| `/components=doc,table,show,pdf,md,text` | 选择组件（对齐 Zoho 的组件可选安装） |
| `/noassoc` | 不注册文件关联 |
| `/uninstall` | 卸载 |

## 3. 文件关联

### 3.1 ProgID

| 格式族 | ProgID | 扩展名 |
| --- | --- | --- |
| 文字 | `MPelyoffice.Document.1` | .docx .odt .doc .rtf |
| 表格 | `MPelyoffice.Spreadsheet.1` | .xlsx .ods .xls .csv .tsv |
| 演示 | `MPelyoffice.Presentation.1` | .pptx .odp .ppt |
| PDF | `MPelyoffice.Pdf.1` | .pdf |
| Markdown | `MPelyoffice.Markdown.1` | .md .markdown |
| 文本 | `MPelyoffice.Text.1` | .txt |

写入（全部在 `HKCU\Software\Classes`）：

```
HKCU\Software\Classes\MPelyoffice.Document.1
    (Default)          = "MPelyoffice 文档"
    FriendlyTypeName   = "MPelyoffice 文档"
    DefaultIcon        = "<安装目录>\MPelyoffice.exe,0"
    shell\open\command = "\"<安装目录>\MPelyoffice.exe\" \"%1\""
HKCU\Software\Classes\MPelyoffice.Document.1\SupportedTypes
    .docx = ""
HKCU\Software\Classes\.docx\OpenWithProgids
    MPelyoffice.Document.1 = ""
HKCU\Software\Classes\Applications\MPelyoffice.exe
    FriendlyAppName = "MPelyoffice"
    SupportedTypes  = ".docx" ".odt" ...
HKCU\Software\Microsoft\Windows\CurrentVersion\RegisteredApplications
    MPelyoffice = "Software\Classes\MPelyoffice.Document.1\Capabilities"
```

### 3.2 铁律（写入代码前先读）

| 编号 | 规则 | 依据 |
| --- | --- | --- |
| WA-01 | **任何代码路径都不得写 `UserChoice`** | 该键对所属用户 DENY 写；`Hash` 算法未公开；Win11 25H2 UCPD 驱动主动拦截 |
| WA-02 | **不得删除 `HKCR\.<ext>` 的默认值** | 微软安装器指引：卸载时保持文件类型映射不变 |
| WA-03 | 只写 `OpenWithProgids`，使 MPelyoffice 出现在"打开方式"列表 | 微软指引的推荐做法 |
| WA-04 | `ShellNew` 仅在**该扩展名当前无任何处理器**时接管默认值 | 上游 R7.9 实测：资源管理器只在扩展名有默认 ProgID 时列出新建项 |
| WA-05 | 注册后调用 `SHChangeNotify(SHCNE_ASSOCCHANGED, 0, 0, 0)` | 否则资源管理器可能不刷新 |
| WA-06 | 应用内"设为默认"按钮**只跳转** `ms-settings:defaultapps` | 由用户完成，不由程序代劳 |

### 3.3 卸载的反向规则

- 删除 `HKCU\Software\Classes\MPelyoffice.*`（我们自己建的键）。
- 删除 `HKCU\Software\Classes\.<ext>\OpenWithProgids` 中值为我们的那一项（**仅当仍指向我们**）。
- 若 `ShellNew` 的 `FileName` 仍指向我们的模板则删除该项。
- **不动** `HKCR\.<ext>` 的默认值，即使它指向我们（微软明确要求保持映射不变）。

## 4. 单实例与文件交接

### 4.1 复用上游实现（不另造）

上游 `crates/ely-platform/src/instance.rs` 与 `ely_ui::run` 已实现并在 Windows 上被真窗口核对过：

| 环节 | 上游机制 | Windows 细节 |
| --- | --- | --- |
| 互斥 | 应用数据目录的 `instance.lock` | `LockFileEx`；被拒时用 `F_GETLK` 问出持锁进程号（Unix 路径） |
| 交接通道 | `127.0.0.1` 上的随机端口（`instance::Door`） | 端口 + 进程号 + 32 字节随机令牌写在 `instance.door` |
| 交接协议 | `ely-ipc` 帧（Windows 为 UTF-16） | 第二份先发令牌，门答"开/拒"后才发路径；收到回执即以 0 退出 |
| 前台 | 第二份先 `AllowSetForegroundWindow` | 主窗口最小化的先复原 |
| 重试 | 门未开/令牌被拒时 10 秒内重试；**路径发出后不重试** | 避免打开两次 |

> **决策修订**：ADR-006 原写"命名管道 + JSON"。取证后改为**复用上游 `instance::Door`**：
> 它已在 Windows 上验证（第二份 132 毫秒退出、第一份多一个标签），行为可预期，代码零重复。
> 命名管道仅在需要"跨会话/跨用户"的场景才考虑，届时以新 ADR 记录。

### 4.2 命令行与打开请求

| 输入 | 行为 |
| --- | --- |
| `MPelyoffice.exe` | 无文件 → 打开开始页（或恢复上次工作区） |
| `MPelyoffice.exe a.docx b.xlsx` | 两个文件 → 两个标签 |
| 资源管理器拖放 | 同上 |
| 第二实例带文件启动 | 经门交给首实例，首实例开新标签，第二实例退出（退出码 0） |
| 第二实例不带文件启动 | 仅把首实例带到前台 |

**路径规范化**：启动参数先按当前目录转绝对路径（上游 2026-10-08 的修复：相对路径会变成 `file://相对` 打不开）。

## 5. Windows 平台适配清单

| 项 | 要求 | 来源 |
| --- | --- | --- |
| 发行构建子系统 | `windows_subsystem = "windows"`（开发构建保留控制台看日志） | 上游 2026-10-11 |
| 子进程无窗口 | worker、Git 等一律 `CREATE_NO_WINDOW`（`ely_ipc::windowless`） | 上游 2026-10-11 |
| 图标资源 | 构建脚本把 `.ico` 编为资源 1（gpui 的窗口图标取资源 1） | 上游 `packaging/windows/icon.rs` |
| 静态 CRT | `.cargo/config.toml` 已为 MSVC 配 `+crt-static` | 上游现成 |
| 真实路径 | 用 `dunce::canonicalize`（Windows 上给 `\\?\C:\…` 会被用户看见且不可用） | 上游 2026-10-12 |
| 中文排版 | `gpui-pre-windows` 的 `text_locale` 用 Windows 语言列表第一个中日韩语言；界面语言为中文时经 `gpui_windows::set_text_language` | 上游 R4.13 |
| IME 热键 | 避开 `Ctrl+Shift+0`（中文输入法占用），另绑 `Ctrl+Shift+9` | 上游 R4.11 |
| 标题栏按钮 | `WM_NCHITTEST` 取最上层未被遮挡的窗口控制区（vendor 已修） | 上游 2026-10-11 |
| 打印 | `PrintDlgEx`；设纸型前先按打印机纸型匹配；页超出纸张半点以上即报错不印 | 上游 R7.6 |
| 凭据 | 系统凭据库（Windows 凭据）；不可用时只在内存 | 上游 C18.2 |
| 崩溃报告 | Windows 上的崩溃报告位置（上游标注"未验证"） | 需补验证 |

## 6. 验收脚本

| 脚本 | 断言 |
| --- | --- |
| `mp-association.ps1` | 安装后 `HKCU\...\FileExts\<ext>\UserChoice` **未变**；`OpenWithProgids` 含我们的 ProgID；`RegisteredApplications` 含我们 |
| `mp-uninstall.ps1` | 卸载后：我们建的键已删、`HKCR\.<ext>` 默认值未动、程序目录清空、用户文档完好 |
| `mp-single-instance.ps1` | 启动两次（含带文件），进程数恒为 1；第二个进程退出码 0；首进程标签数 +1 |
| `mp-openwith.ps1` | "打开方式"菜单出现 MPelyoffice |
| `mp-shellnew.ps1` | 对**无处理器**的扩展名出现"新建"，且默认值指向我们；对已有处理器的扩展名**不出现**、默认值不变 |
| `mp-install-clean.ps1` | 干净虚拟机上装单个包后 6 类文件均可打开可编辑 |

## 7. 已知风险

| 风险 | 说明 | 缓解 |
| --- | --- | --- |
| SmartScreen 警告 | 未签名 + 新哈希 → 信誉归零 | 文档写明预期；预留 Azure Artifact Signing / OV 证书接入位；发布包附"如何安装"说明 |
| 企业策略禁用 | 组策略可能禁止新程序 | 逐用户安装仍可用；提供 MSI 供企业选择 |
| `rc.exe` 找不到 | `embed-resource` 需要 Windows SDK 的资源编译器 | 构建机装 SDK 组件（`VC.CMake.Project` 同批）；`doctor` 脚本检查 |
| Defender 干扰 | 构建目录被扫描会导致并行 configure 失败 | 把 Cygwin 与 LO 构建目录加入 Defender 排除项（上游明确要求） |
