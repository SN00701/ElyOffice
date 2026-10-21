# MPelyoffice 架构决策记录（ADR）

版本：0.1
日期：2026-10-20
方法：每条决策按**第一性原理**记录推导链：
`原理（为什么存在）→ 约束（已核实的事实）→ 决策 → 后果（含代价）→ 验证方式 → 回退路径`。
凡"约束"一栏的每条事实，均为 2026-10-20 一手取证（源码、仓库树、厂商页面、Microsoft Learn），
出处见 `02-ElyOffice剖析与复用清单.md` 与 `01-竞品对标.md`。

---

## ADR-001 交付形态：单安装包 + 单启动器 + 单窗口多标签

**原理**
用户的真实诉求不是"有 8 个软件"，而是"装一次、点一次、什么都能干"。
WPS 的护城河正在于此（微软至今无原生多标签）。

**约束**
- WPS 整合模式：四个组件塌缩为一个桌面图标，文档以**混合类型标签页**聚合于同一框架，标签可拖出成独立窗口，可存入命名工作区跨重启恢复（已核实）。
- ONLYOFFICE Desktop Editors 同样以标签式界面提供五个编辑器（已核实）。
- LibreOffice / OpenOffice 均为 Start Center **派发**模型：多开多窗，无混合标签（已核实）。
- 上游 ElyOffice 是 8 个 app、每个独立 NSIS 包（已核实）。

**决策**
1. MPelyoffice = **一个** exe，**一个**主窗口，标签页混排 6 类文档。
2. 同一 exe 允许另起第二个顶层窗口（对齐 WPS"多开"）。
3. 工作区（命名标签组）跨重启持久化。
4. **废弃"每 app 一个包"**，改为单一离线安装器（ADR-007）。

**后果**
- 获得 WPS 式核心体验与差异化。
- 代价：`Host::Engine` 静态关联类型必须动态化（ADR-003），工程量从"写 6 个 app"变成"改造外壳内核"。
- 标签数无上限会带来内存与 worker 进程膨胀 → M2 必须实现 worker 回收与按需启动。

**验证**：双击 `.docx` 与 `.xlsx` 落在同一窗口不同标签；Ctrl+Tab 循环；标签拖出成独立窗口且状态不丢；重启后恢复工作区。

**回退**：若动态化受阻，可退化为"同 exe 多窗口 + 每窗口单一引擎族"的弱整合（弱于 WPS，但优于现状）。

---

## ADR-002 PDF 内核：MuPDF(AGPL) → PDFium(BSD-3)，自写 FFI

**原理**
MPelyoffice 面向**对外分发的免费软件**。分发即再分发，许可证义务随二进制走。

**约束**
- MuPDF 为 AGPL-3.0；Artifex 官网明示"嵌入产品请联系商业授权"；NuGet 包 `MuPDF.NET` 3.28.0 适用 **Artifex Community License（非商用）**（已核实）。
- PDFium 为 BSD-3-Clause，仅要求保留版权声明（已核实）。
- Rust 生态常用封装 `pdfium-rs` 是 **GPL-2.0-or-later**，会污染二进制（已核实）。
- `documentcloud/pdfium` 分支带"不得以 PDFium 名义推广"的命名条款（已核实）。
- 仓库既有 `lopdf 0.45`（MIT）承担 PDF 写入层，且该层与渲染内核解耦（已核实模块边界）。

**决策**
1. PDF 渲染与页面/文本层改为 **PDFium**，**自写最小 FFI 绑定**，不使用 `pdfium-rs`。
2. 保留 `lopdf` 写层、`Tesseract` OCR 层、`proto.rs` IPC 层不变。
3. 改动面限定为 `crates/ely-pdf-engine/src/client.rs`、`sample.rs` + 新增 `pdfium.rs` 绑定模块。

**补充（2026-10-20 取证后的修订）**
上游在 2026-10-19 把自有代码改为 MIT，但**保留了 MuPDF**，其处理方式是
"**ElyPDF 链接 MuPDF，其发行包整体仍须按 AGPL 履行**"（上游 `AGENTS.md` 原文）。
这给了本项目一条**备选合规路径**：保留 MuPDF + 在发行包中提供对应源码与 AGPL 全文，
使整个发行包按 AGPL-3.0 履行（对免费分发可行，但要求"完整对应源码"的交付与后续维护）。

**选择 PDFium 而非走 AGPL 路径的理由**（第一性原理：谁承担义务、义务有多重）：
1. AGPL 路径要求**本产品整体**（含我们的 Rust 外壳代码）以 AGPL 条款分发，并随每次更新提供对应源码——
   这会与我们"自有代码保持 MIT"的意图冲突，且增加持续义务（每次发版都要打包源码）。
2. PDFium 是 BSD-3，义务仅为"保留版权声明与免责声明"，可与 MIT 自有代码自由组合。
3. PDFium 是 Chromium 的 PDF 渲染器，渲染质量与文本层能力满足本项目首版的 PDF 承诺
   （阅读/搜索/批注/页面管理/OCR），**不因换内核而降低首版承诺**。
4. 代价自担：我们需自写约 200–400 行 FFI 绑定，并自行管理 PDFium 二进制的获取与登记。

**后果**
- 许可干净（仅 BSD-3 + MIT + Apache-2.0），满足对外分发红线。
- 代价：需自己维护 PDFium 二进制获取与加载；PDFium 预编译产物来源需在许可清单中逐项登记。
- PDF 的"文字对象编辑"能力弱于 MuPDF 的部分 API → 按支持矩阵明示（首版 PDF 承诺：阅读/搜索/批注/页面管理/OCR）。

**验证**：`cargo deny`/人工清单核对分发物许可证；渲染一页 PDF 出图；批注写入后用独立阅读器打开验证。

**回退**：若 PDFium 集成受阻，可仅用 `lopdf` 做结构化读写 + 借用系统/Edge 渲染（仅查看降级），但这会削弱 PDF 承诺，需同步下调支持矩阵。

---

## ADR-003 引擎抽象：从静态关联类型到运行期多态（`EngineDyn`）

**原理**
混排标签页要求"一个窗口里同时存在 LOK 文档、PDF、Markdown、文本四种会话"。
外壳的状态机（保存/恢复/查找/撤销/关闭）必须对所有引擎**通用**。

**约束**
- 上游 `Host::Engine` 与 `Core<H>::tabs: Vec<Tab<H::Extra, H::Engine>>` 均为编译期绑定（已核实源码）。
- `Engine` trait 已抽象 `Doc`/`View`/`Worker` 三元组，且 `Worker` 已是进程外模型（`crates/ely-lok/src/worker.rs`）（已核实）。
- Rust 的 trait object 要求**对象安全**：方法不能有泛型参数、不能返回 `Self`、不能有关联类型泄漏。

**决策**
1. 新增 `trait EngineDyn`（对象安全），把 `Doc`/`View`/`Worker` 三个关联类型**擦除为** `Box<dyn Any>` 与 `Entity<dyn ...>` 的等价封装。
2. `Host::Extra` 同样类型擦除为 `Box<dyn Any>`，由各引擎自行 downcast。
3. `Core` 的 `tabs` 改为存 `TabDyn`（含 `engine: Arc<dyn EngineDyn>`）。
4. 引擎注册表按扩展名派发：`ext → EngineKind`。
5. 命令按引擎注册：功能区与菜单命令由**当前活动标签的引擎**提供可用性与执行体。

**后果**
- 达成 ADR-001 的混排目标。
- 代价：一次类型擦除换来的动态分派，编译期检查力下降 → 必须以**引擎契约测试**（每个引擎跑同一套 `EngineContract` 测试套件）补偿。
- 旧的 8 个 app 可继续用原静态 `Host`（保留泛型路径），新旧并存，逐步迁移。

**验证**：同一窗口同时持有 LOK 文档与 Markdown 标签；两者的保存/撤销/恢复互不串扰；每个引擎通过 `EngineContract` 测试。

**回退**：若擦除代价过高，退化为"按引擎族分窗口"（同窗口内只放同族文档），仍满足"单 exe 单安装包"的核心诉求。

---

## ADR-004 代码基线：复用 ElyOffice 源码，而非自研

**原理**
文档编辑器的**内核**（排版、公式、PDF 解析、格式往返）是 10 年量级工程。
自研不可能在可接受周期内达到可用质量；复用一个已成型的 MIT 项目是唯一理性路径。

**约束**
- ElyOffice 为 MIT 许可（已核实），其第三方组件另计。
- 上游已实现 LOK 接入全栈（`crates/ely-lok` 约 12 个模块）、PDF 引擎全栈、会话/恢复状态机、GPUI 组件库、Windows 关联注册范式（已核实）。
- 上游 Windows 打包链"从未跑通"（已核实 README 自述）。

**决策**
1. **Fork** ElyOffice 为 `mpelyoffice`，保留 upstream remote 以便长期 rebase。
2. 以"最小侵入"原则改造：尽量不动 `crates/ely-office` 内部逻辑，只在类型边界动手。
3. 上游缺陷不即时全量同步；只同步与 MPelyoffice 相关且低风险的修复，记录于 `docs/mp/09`。

**后果**
- 复用度极高（见 `02` 复用矩阵），M1 可快速出可运行外壳。
- 代价：承担上游技术债与 rebase 税；上游 8 app 结构与"单 exe"目标冲突，需新增 `apps/mpelyoffice` 作为唯一宿主。

**验证**：MPelyoffice 编译产出可执行文件；上游 `cargo test --workspace` 在改造后仍全绿（旧 app 保留）。

**回退**：若改造代价超预期，保留上游 8 app 不动，另起独立 `apps/mpelyoffice` 只复用 crates（不复用 apps），届时以"多 exe + 单一安装器"形态交付（仍满足 R1/R2 的"只装一个包、只启动一个程序"）。

---

## ADR-005 OOXML 引擎：LibreOfficeKit + 稀疏源码构建

**原理**
DOCX/XLSX/PPTX 的**往返保真**（不丢内容、不丢格式、不丢修订）是硬需求；
Rust 生态无成熟"改写既有文件"内核（`docx-rs` 是写入器、`rust_xlsxwriter` 明文不能改既有文件、`calamine` 只读且丢格式，均已核实）。
可用的成熟内核是 LibreOffice，而唯一能让它进入自有 UI 的接口是 LOK。

**约束**
- LOK 分块渲染与编辑 API 上游标注 **unstable**（需 `LOK_USE_UNSTABLE_API`），已知编辑客户端仅 2 个（已核实）。
- LOK C API **非线程安全**，全部调用被 LO 全局 SolarMutex 串行（已核实）。
- LibreOffice Windows 构建需 Cygwin + VS + JDK，官方要求 Windows 自动化编译 ≥16 vCPU（已核实）。
- 上游 `fetch-references.sh` 对 LO 采用**稀疏检出**（仅 5 个路径），`build-libreoffice.sh` 已含 `CYGWIN*` 分支（已核实）。
- LOK 许可为 MPL-2.0，随包分发需附许可与源码获取说明。

**决策**
1. OOXML 引擎 = LOK，锁死版本 + 固定 commit `8221e31b` + Ely 的 LOK 补丁集。
2. 沿用稀疏检出策略，**不构建全量 LibreOffice**。
3. 构建目录与 Cygwin 全部放**无空格路径**（`F:\cygwin64`、`F:\mpely-lo`）。
4. 保留 LOK-11：不执行宏、不自动更新外部链接，打开时明示处理状态。
5. 因 SolarMutex 串行：**每个引擎一个 worker 进程**（沿用 `ely-lok` worker 模型），不同文档并行由进程隔离实现，不在单进程内并发调 LOK。

**后果**
- 获得真实 OOXML 保真度与 PDF 导出。
- 代价：Windows 构建税（数小时级）、上游 rebase 税、MPL-2.0 合规义务、LOK API 不稳定需回归测试兜底。
- 若构建失败 → 见回退。

**验证**：`instdir/program/libreofficekit.dll` 存在；`lok_init` 成功；DOCX 改字保存后重开内容一致；
与 Microsoft Word 打开对比表格/编号/修订（沿用上游 DOC-03 流程）。

**回退（分层）**
1. **官方 LO MSI + UNO/headless**：官方 Windows MSI 完整支持 UNO，可用于转换、预览与部分编辑；
   仅取转换能力时，DOCX 降级为"查看 + 转换后编辑"（对照 ONLYOFFICE 支持矩阵第二轴）。
2. **Collabora 商业 SDK**：需报价，作为商业化路径。
3. **Markdown/PDF/Text 三引擎不受阻**，可独立交付一个"可用但不含 OOXML 编辑"的版本。

---

## ADR-006 Windows 集成：per-user + 永不夺默认 + 管道式单实例

**原理**
"双击文件就能打开"是产品价值；但**以伤害用户既有设置为代价**换取默认关联是不道德且在技术上不可行的。

**约束**
- `UserChoice` 键对所属用户 **DENY 写**；`Hash` 算法未公开，只写 `ProgId` 会被 Windows 丢弃重置；Win11 25H2 UCPD 驱动主动拦截程序化变更（已核实）。
- 微软安装器指引要求：**卸载时保持文件类型映射不变**（已核实原文）。
- 命名管道默认跨会话可达；`Local\` 互斥体在 RDP 多会话下不互斥（已核实）。
- 上游 `packaging/windows/types.sh` 已采用正确范式：只写 `OpenWithProgids`、按用户 `HKCU`、卸载只删自己写的值、注册后 `SHChangeNotify`（已核实源码）。
- 本机**无管理员权限**。

**决策**
1. 安装默认**按用户**：`%LOCALAPPDATA%\Programs\MPelyoffice`，注册表写 `HKCU\Software\Classes`。
2. **任何代码路径不得写 `UserChoice`**；应用内"设为默认"仅跳 `ms-settings:defaultapps`。
3. ProgID：`MPelyoffice.Document.1` / `.Spreadsheet.1` / `.Presentation.1` / `.Pdf.1` / `.Markdown.1` / `.Text.1`；
   写 `OpenWithProgids` + `RegisteredApplications` + `SupportedTypes` + `ShellNew`（仅当该扩展名当前无处理器）。
4. 卸载**不删** `HKCR\.<ext>` 默认值，只删自己写入的键值。
5. 单实例与文件交接：**复用上游 `crates/ely-platform/src/instance.rs` + `ely_ui::run`**
   （数据目录的 `instance.lock` 文件锁 + `127.0.0.1` 随机端口的 `instance::Door` +
   `instance.door` 里的端口/进程号/32 字节随机令牌 + `ely-ipc` 帧）。
   **修订理由（2026-10-20 取证）**：上游已在 Windows 上实现并真窗口核对过
   （第二份 132 毫秒退出、第一份多一个标签；Windows 侧用 `LockFileEx`，
   第二份先 `AllowSetForegroundWindow`），行为可预期且有回归测试。
   本项目**不自造命名管道**；命名管道仅在需要跨会话/跨用户交接时才引入，届时另立 ADR。
   MPelyoffice 只需保证自己的 `AppId` 与数据目录独立即可复用该机制。

**后果**
- 零管理员门槛，符合"只装一个软件"的诉求；不污染用户环境。
- 代价：per-user 安装无法被机器策略统一管理 → 后续补 WiX MSI 的 per-machine 变体（ADR-007）。

**验证**：干净虚拟机上安装后，默认程序仍是原来的；"打开方式"出现 MPelyoffice；
连开两次 exe 只留一个进程且第二个把文件交给第一个。

---

## ADR-007 分发：自研 per-user 引导器为主，MSI 为辅，不做 MSIX

**原理**
用户要"一个包装全部"，且安装门槛越低越好。

**约束**
- MSIX：禁止驱动与按用户服务；注册表/FS 写入被虚拟化；包目录只读；原生 C++ 需 VCLibs 依赖；**不会接管已有默认程序**；`WindowsApps` ACL 锁定导致用户无法放插件（已核实）。
- Inno Setup：单 EXE、LZMA2、`PrivilegesRequired=lowest` 支持免管理员、`[Registry]` 原生支持 `HKCU`/`OpenWithProgids`、静默参数标准化（已核实）。
- WiX/MSI：企业 GPO/SCCM 友好、有回滚、关联用原生表（已核实）。
- ONLYOFFICE 同时提供 EXE + MSI + 1MB 在线包，且 `appcast.json` 证实其 EXE 为 Inno Setup（已核实）。
- 本机无管理员，但 `winget` 可获取 Inno Setup 6.7.3（已核实）。

**决策**
1. **主安装器 = 单一离线安装器**，在两条已验证的路径上取其一：
   (a) **自研 Rust 引导器**（`MPelyoffice-Setup.exe`）：单文件、per-user 默认、可编程
   （读 `config.xml` 选组件/路径/是否建关联）、静默参数、无外部工具链依赖，
   可直接复用本仓库 `crates/ely-platform` 的系统能力；
   (b) **复用上游 `packaging/windows/bundle.nsi` 的合集安装程序**（组件页勾选 + 逐个静默 `/S`），
   但把被调用的安装程序换成**我们自己的单一安装器**（而不是 6 个分装）。
   **修订理由（2026-10-20 取证）**：上游 R7.1 已有"一个包装全部"的 Windows 合集安装程序，
   且 `installer.nsi` 已实现"先查能否写 → 逐个挪进 `.ely-old` → 任一失败即回滚、退出码 2"
   的事务语义。这套语义直接继承，避免重写最易出错的升级/卸载路径。
2. **辅安装器 = WiX MSI**（per-machine 变体，面向企业）。
3. **不做 MSIX**。
4. 同时提供**免安装绿色版**（同一 payload，`MPelyoffice.exe` 直接运行），对齐 LibreOffice Portable 的存在意义（已核实）。

**后果**
- 分发形态完全可控，许可清单可由构建脚本自动生成。
- 代价：自研安装器要自己做 ARP 条目、快捷方式、卸载、UAC 提示、静默参数——工作量可控（1 个 crate）。
- 数字签名：EV 自 2024 起不再即时豁免 SmartScreen（已核实），首版不内置签名，但保证所有二进制可独立签名且发布者身份一致。

**验证**：干净虚拟机装单个包 → 六类文件可开可编；卸载后无残留注册项、用户文档完好；
`silent` 参数静默安装成功；绿色版不写任何注册表。

---

## ADR-008 功能区（Ribbon）：在 `crates/ely-ui` 自建组合层

**原理**
WPS 式体验的关键在功能区；但组件库没有 Ribbon，重复造轮子会破坏设计一致性。

**约束**
- 组件库 `src/shell/` 有 `tabs.rs`(8.4KB)、`bars.rs`(7.2KB)、`status.rs`(10.5KB)、`chrome.rs`(15KB)、`frame.rs`、`columns.rs`、`windows.rs`(9.8KB)，**无 ribbon 模块**（一手取证）。
- 上游 PRD §5.1 要求"通用交互回到组件库实现，不在 8 个应用里复制"。

**决策**
1. 不修改外部组件库（保持独立仓库依赖），而是在**本仓库** `crates/ely-ui` 内新增 `ribbon` 组合模块，
   基于组件库的 `buttons`/`icons`/`theme` 令牌/`bars` 组合出 Ribbon。
2. Ribbon 的**命令模型**与 `crates/ely-office` 的 `Commands` 打通：菜单、功能区、右键菜单、快捷键**同源**。

**后果**
- 满足"同操作同位置同名同图标"的一致性要求。
- 代价：新增约一个模块的 UI 工作量，且需与上游组件库升级保持兼容（只依赖公开 API）。

**验证**：同一操作在菜单与功能区行为一致；禁用原因可见；混选显示"混合"。

---

## ADR-009 首版范围：6 个模块；邮件 / Git / Agent 延后

**原理**
范围膨胀是项目失败的头号原因；且这三者的外部依赖最重（OAuth 应用注册审核、Git 二进制、Agent 后端）。

**约束**
- 上游 PRD 自己把 ElyMail/ElyAgent 标注为"第二发布"（一手取证）。
- Gmail/Microsoft OAuth 内置登录需要完成应用注册与审核，不能以"用户自备开发者凭据"作为默认路径（已核实上游要求）。
- 竞品趋势：Markdown 已成一等公民格式，AI 是趋势但不是本项目首版承诺（已核实）。

**决策**
1. 首版 = 文字 / 表格 / 演示 / PDF / Markdown / 文本（新增 `.txt`）。
2. 邮件 / Git / Agent 在架构上**预留引擎插槽**（ADR-003 的注册表天然支持插入新引擎），但首版不实现。
3. 首版**不做** AI 面板、云、账号体系（竞品显示 AI 是趋势，但它是外壳之上的增量能力，不应阻塞核心交付）。

**后果**
- 范围可控，可交付可用产品。
- 代价：相比 WPS 首版功能更少（无云、无 AI、无邮件）。

**验证**：首版 DoD 见 `00-需求优化与验收基线.md` §7。

---

## ADR-010 字体不再分发，使用系统字体

**原理**
字体再分发是最常见的隐性许可地雷；而本机已有 641 个字体文件（含中文），用户环境同样具备。

**约束**
- 上游 `build-libreoffice.sh` 带 `--with-fonts`（LO 自带字体）——这会把 LO 的字体集打进发行包（已核实脚本）。
- 上游还有 `packaging/fonts` 类目录惯例与 `templates.sh`（已核实文件名）。

**决策**
1. MPelyoffice **不再分发任何字体**，LO 构建去掉 `--with-fonts`。
2. 文档缺字体时按"缺字回退 + 提示"处理，不静默替换。

**后果**
- 消除一整类许可风险与体积。
- 代价：跨平台排版一致性下降（用户换机器排版略有差异）——本项目为 Windows 单平台，可接受。

**验证**：发行包内无 `.ttf`/`.otf`/`.ttc`（除 LO 运行时必需且许可允许者，逐项登记）。

---

## ADR-011 构建环境布局：LO 与 Cygwin 落 `F:\`，仓库留在原路径

**原理**
Windows 上的 C/C++ 构建对路径空格与长度敏感；仓库路径由需求方指定，不应因此更改。

**约束**
- 仓库位于 `E:\trae pg\office_modpack`（**含空格**）（本机环境）。
- `F:` 剩余 122.7GB；`E:` 44.8GB；`C:` 36GB（本机环境）。
- LO 构建产物规模在数十 GB 量级（本项目稀疏构建的保守估计）。

**决策**
1. 仓库与 Rust `target/` 保留在 `E:\trae pg\office_modpack\mpelyoffice`。
2. **Cygwin 安装到 `F:\cygwin64`**，LO 源码与构建目录 `F:\mpely-lo`（无空格）。
3. 若 Rust 构建因空格路径出问题，用 `.cargo/config.toml` 把 `target-dir` 指到 `F:\mpely-target`。

**后果**
- 避开 Windows 构建最常见的坑。
- 代价：产物分散两盘，脚本需显式处理。

**验证**：LO 构建成功；`cargo build` 在含空格路径下成功。

---

## ADR-012 过程方法：文档先行 + 对抗式审查收口

**原理**
用户明确要求"先生成开发文档，再开发，完成后按对抗式审查检查"；
同时"第一性原理"要求每个决策可追溯到约束与验证，而非模仿。

**约束**
- 竞品普遍存在"截图相近但修订丢失"式验收（上游 PRD §8.3 明列为失败）。
- ONLYOFFICE 10.0 的 CVE 清单（DOCX 超链接 XSS、任意文件写、路径穿越、本地文件读、SSRF）说明"解析不可信文档"是主要攻击面（已核实）。

**决策**
1. 文档先行：需求基线 → 竞品对标 → 剖析与复用 → ADR → 引擎抽象规范 → Windows 集成规范 → 支持矩阵与许可 → 计划 → 审查报告 → 构建手册。
2. 文档中每条验收条目**可执行**（有明确通过判据），不接受"效果良好"式描述。
3. 开发完成后执行**红队式对抗式审查**：独立 reviewer 角色、六攻击面、要求可复现证据、分级 P0/P1/P2、发现→修复→**新增回归测试**→复验闭环。

**后果**
- 过程可审计，返工成本前置到设计期。
- 代价：前期文档投入约 10 个工作文档的量级。

**验证**：每条验收条目在审查报告中都有对应结论；无"已修复"缺证据项。

---

## ADR-013 分发物的形态：单一目录 + 许可清单 + 散列清单

**原理**
需求方要的是"一个安装包、一个启动器"，并且许可红线（只允许 MIT/BSD/Apache/MPL）必须在分发物里可查。
"可查"不能靠人记得，得由构建产出。

**约束**
- 本机无管理员权限 ⇒ 安装器只能是 per-user（ADR-006）。
- 链进二进制的 crate 有数百个，许可各不相同；漏一份即不合规。
- 内核（LOK）尚未构建完成，分发包必须能在"只有界面与 PDF 内核"的状态下成立。

**决策**
1. 分发目录由 `packaging/windows/stage.ps1` 组装，固定包含：程序、安装器、`pdfium.dll`、
   `MPelyoffice.json`（逐文件 sha256）、`Licenses/`（本项目许可、第三方清单、PDFium 许可、逐 crate 许可）。
2. 逐 crate 许可由 `packaging/crates.py` 从构建产物的依赖图生成（`PYTHONUTF8=1`）；缺任何一份即打包失败。
3. 许可清单**不做**"构建期阻断"以外的妥协：不把 GPL/AGPL 的组件放进分发物（ADR-002 的 PDFium 替换即由此而来）。
4. 安装器只装清单里的文件，卸载只删清单里的文件；数据目录另算（见 ADR-014）。
5. 引擎未构建时，包如实写明"文档引擎尚未构建"，不假装齐全。

**后果**
- 任何一次分发都可自证合规：散列对得上、许可对得上。
- 代价：包比"只有一个 exe"大（当前 36.7MB，其中 7.2MB 是 PDFium，3.8MB 是逐 crate 许可）。

**验证**：`tests/scripts/mp-install.ps1` 装 → 跑 → 卸全绿；`stage.ps1` 每次都写出 `MPelyoffice.json` 与 `Licenses/`。

---

## ADR-014 数据根：`%LOCALAPPDATA%\MPelyoffice`

**原理**
旧的数据根是 `%LOCALAPPDATA%\ElyOffice`，那是上游八个应用共用的名字；本 fork 的产品是 MPelyoffice，
数据、设置、日志与恢复副本都应在一个说得出主人的文件夹里。

**约束**
- 卸载只动本应用的数据目录，不碰用户文档（规则 4）。
- 共享设置（`settings.json`）在数据根下，各应用各有一份 `settings.json`。

**决策**
1. `ely_platform::dirs::root()` 返回 `%LOCALAPPDATA%\MPelyoffice`；各应用在它下面一层自己的 `slug`。
2. 旧的 `ElyOffice` 目录不再读，也不再删（用户的文件不因改名而消失）。
3. 卸载器清空的是本应用的那一层与安装目录，不清整个根。

**后果**
- 数据目录与产品同名，卸载与诊断时说得出清楚。
- 代价：从上游切换过来的用户数据留在旧目录，需要时手工搬。

**验证**：`crates/ely-platform/src/dirs.rs` 的单测；`mp-install.ps1` 断言卸载后程序与数据都干净。
