# MPelyoffice 引擎抽象规范（EngineDyn）

版本：0.1
日期：2026-10-20
对应 ADR：ADR-003（引擎动态化）、ADR-001（单窗口混排标签）
上游依据：`crates/ely-office/src/lib.rs`（`Host`/`Core`/`Tab`/`Engine`）、`crates/ely-lok`（worker 模型）

---

## 1. 问题

上游外壳把**引擎类型编译期绑定**在宿主上：

```rust
pub trait Host: Sized + 'static {
    type Extra: Default;
    type Engine: Engine;      // ← 一个 app 一种引擎
    const KIND: Kind;
    ...
}
pub struct Core<H: Host> { pub tabs: Vec<Tab<H::Extra, H::Engine>>, ... }
pub struct Tab<E, G: Engine> { pub view: Entity<G::View>, pub doc: G::Doc, ... }
```

后果：一个进程只能承载一种引擎 → 无法在同一窗口的标签栏里混排 DOCX 与 PDF，
这正是 ADR-001 要解决的核心问题。

**已核实的可用积木**（`vendor/gpui-pre/src/view.rs` 一手取证）：

```rust
/// A dynamically-typed view handle that can be downcast to a specific `Entity<V>`.
pub struct AnyView {
    entity: AnyEntity,
    render: fn(&AnyView, &mut Window, &mut App) -> AnyElement,
}
impl<V: Render> From<Entity<V>> for AnyView { ... }
impl AnyView {
    pub fn downcast<T: 'static>(self) -> Result<Entity<T>, Self>;
    pub fn entity_type(&self) -> TypeId;
}
```

`AnyView` 正是 GPUI 为"运行期多态视图"准备的类型：**它本身实现了 `View`**，
可经 `ViewElement` 进入响应式图，与具体类型视图走同一路径。
因此**不需要自造渲染桥**，只需把 `Tab` 的视图字段从 `Entity<G::View>` 换成 `AnyView`。

---

## 2. 目标模型

```
                     ┌──────────────────────────────┐
                     │  Core（外壳状态机，复用上游）   │
                     │  tabs: Vec<TabDyn>            │
                     │  sessions / finder / keeping  │
                     └───────────────┬──────────────┘
                                     │ 引擎多态
                     ┌───────────────▼──────────────┐
                     │  trait EngineDyn（对象安全）   │
                     │  ├ kind() / formats()         │
                     │  ├ open() / save() / close()   │
                     │  ├ view() -> AnyView          │
                     │  ├ commands() -> &[Command]   │
                     │  ├ snapshot() / restore()      │
                     │  └ worker 生命周期             │
                     └──────┬────────┬────────┬──────┘
                            │        │        │
                        LokEngine PdfEngine  MarkdownEngine / TextEngine
```

### 2.1 `EngineDyn`（对象安全）

```rust
pub trait EngineDyn: Send + Sync + 'static {
    /// 该引擎负责的格式（扩展名 + MIME）。
    fn kind(&self) -> EngineKind;
    /// 打开一个文档，返回视图句柄与会话所需信息。
    fn open(&self, source: Source, cx: &mut App) -> anyhow::Result<Opened>;
    /// 视图（已是运行期多态）。
    fn view(&self, id: DocId) -> Option<AnyView>;
    /// 列出该引擎在当前文档状态下可用的命令（功能区/菜单据此生成）。
    fn commands(&self, id: DocId, cx: &App) -> Vec<CommandSpec>;
    /// 执行一条命令。
    fn run(&self, id: DocId, cmd: &str, args: &CommandArgs, cx: &mut App) -> anyhow::Result<()>;
    /// 写出（保存/另存为/导出）。
    fn write(&self, id: DocId, target: &WriteTarget, cx: &App) -> anyhow::Result<WriteOutcome>;
    /// 恢复副本快照。
    fn snapshot(&self, id: DocId) -> anyhow::Result<Snapshot>;
    /// 关闭并释放引擎侧资源（视图、临时文件、worker 句柄）。
    fn close(&self, id: DocId) -> anyhow::Result<()>;
    /// 引擎进程是否已结束（崩溃检测）。
    fn alive(&self, id: DocId) -> bool;
}
```

**对象安全约束**（Rust 要求，实现时必须遵守）：
- 不得有泛型方法（`open` 用 `&dyn Any` 传参，不写 `fn open<T>(...)`）。
- 不得有关联类型出现在方法签名里（`Doc`/`View`/`Worker` 一律经 `Box<dyn Any>` / `AnyView` 擦除）。
- 不得返回 `Self`（返回 `Box<dyn Any>` 或具体装箱类型）。

### 2.2 类型擦除方案

| 上游类型 | 动态化后 | 说明 |
| --- | --- | --- |
| `Host::Engine` | `Arc<dyn EngineDyn>` | 每个引擎一个实例，被所有该引擎的标签共享 |
| `Engine::Doc` | `Box<dyn Any>` | downcast 回引擎自己的文档句柄 |
| `Engine::View`（`Entity<V>`） | `AnyView` | GPUI 官方类型，可 downcast 回 `Entity<V>` |
| `Engine::Worker` | `Box<dyn Any>` | 引擎私有的 worker 句柄（LokWorker / PdfWorker） |
| `Host::Extra` | `Box<dyn Any>` | 每个标签的引擎私有附加数据（如幻灯片当前页、PDF 滚动位置） |

> **不使用 `unsafe` 的替代方案**：若 `AnyView` 的 downcast 不满足需要（例如需要 `Entity<V>` 的强类型调用），
> 则在 `AnyView` 之外另存一个 `Box<dyn Any>` 持有 `Entity<V>`，两者成对构造、同一处 downcast。

### 2.3 引擎注册表

```rust
pub struct Registry {
    /// 扩展名（小写，含点）→ 引擎。
    by_ext: HashMap<String, EngineId>,
    /// 引擎 → 实例。
    engines: HashMap<EngineId, Arc<dyn EngineDyn>>,
}
```

- 派发规则：`path.extension()` 小写查表 → 引擎；查不到则由"通用文本"兜底或提示"不支持的格式"。
- **派发表来自 `packaging/types.tsv`**（上游唯一事实来源，AGENTS.md 明确要求），
  我们的 fork 在其基础上**新增 `text` 引擎与 `txt` 行**（见 §4）。
- 引擎可**多实例**（如 PDFium 与 LOK 各一个），标签引用具体实例。

### 2.4 命令模型（同源）

WPS 式体验的核心约束是"同操作同位置同名同图标"（ADR-008）。因此：

1. **命令是数据**：`CommandSpec { id, label_key, icon, enable: EnableState, kind }`，
   由**当前活动标签的引擎**提供 `commands()` 返回。
2. **执行体统一走 `EngineDyn::run`**；外壳（菜单、功能区、右键、快捷键）只负责收集 `id` 与调用。
3. 快捷键与菜单项引用同一 `id`，因此**天然同源**。
4. 引擎不提供某命令时，功能区该项**显示为禁用并说明原因**（继承上游 UX-04 的"混合/禁用"规则）。

### 2.5 worker 生命周期

- 每个引擎实例按需启动 worker（上游 PDF 的做法：打开第一个 PDF 才起 worker；LOK 随窗口启动）。
- 标签关闭 → `close(id)` → 引擎释放视图与句柄 → 引用计数归零时回收 worker。
- **崩溃隔离**：worker 退出由 `alive()` 轮询发现（上游 `Core::stopped` 已有此状态），提示"内核已停止"并提供重开。
- **绝不**：在界面线程上同步等待 worker（上游 R6 的教训：键入延迟中位 290 毫秒 → 改推送式后中位 19 毫秒）。

---

## 3. 迁移步骤（渐进，随时可停）

| 步 | 动作 | 可验证点 |
| --- | --- | --- |
| S1 | 新增 `crates/ely-engine`：`EngineDyn`、`EngineKind`、`Registry`、`CommandSpec` | 单元测试：注册表派发表正确 |
| S2 | `crates/ely-office` 新增 `TabDyn` 与 `CoreDyn`（**与旧 `Core<H>` 并存**） | 旧 8 个 app 仍全部编译通过、测试全绿 |
| S3 | 实现 `LokEngine`（包 `ely-lok`）、`PdfEngine`（包 `ely-pdf-engine`）、`MarkdownEngine`、`TextEngine` | 每个引擎跑通 `EngineContract` |
| S4 | 新建 `apps/mpelyoffice`：外壳用 `CoreDyn`，标签栏用 `navigation::Tabs` + `shell::TabBar` | 单窗口混排两类文档 |
| S5 | 功能区自建（ADR-013），命令取自活动标签的 `commands()` | 菜单与功能区同源同语义 |
| S6 | 旧 app 逐步改为薄壳（可选）；完成后删除旧的静态路径 | `cargo test --workspace` 全绿 |

---

## 4. 引擎契约测试套件（`EngineContract`）

任何新引擎必须通过同一套契约测试，否则不予合入：

| 编号 | 契约 | 断言 |
| --- | --- | --- |
| C-01 | 打开 | 打开后视图存在、状态为"已保存"、名称正确 |
| C-02 | 脏标记 | 一次编辑后 `modified` 为真；撤销后回假 |
| C-03 | 保存往返 | 编辑 → 保存 → 关闭 → 重开 → 内容一致 |
| C-04 | 外部修改 | 磁盘被改后保存**不覆盖**，并进入冲突态 |
| C-05 | 另存为 | 写到新路径成功；原文件不变；目标存在时按上游规则拒绝或覆盖（明确定义） |
| C-06 | 关闭释放 | 关闭后 worker 句柄释放、临时文件清理、无句柄泄漏 |
| C-07 | 崩溃隔离 | 杀掉 worker 后 `alive()` 为假、外壳显示"内核已停止"、其它标签不受影响 |
| C-08 | 命令一致性 | `commands()` 列出的每条命令执行后有确定结果（成功或明确错误） |
| C-09 | 快照 | `snapshot()` 产生可恢复副本，恢复后内容一致 |
| C-10 | 路径健壮 | 空格/中文/超长路径可打开可保存 |

---

## 5. 失败模式与对策

| 失败模式 | 现象 | 对策 |
| --- | --- | --- |
| downcast 失败 | 取出错误的实体类型 | 引擎句柄与视图**成对构造**，同一处 downcast；契约测试 C-06/C-07 覆盖 |
| 引擎崩溃 | worker 进程退出 | `alive()` 轮询 + `Core::stopped` 提示条 + "重新打开文档"（上游已有） |
| 命令错配 | 功能区命令发给了别的引擎 | 命令 `id` 带引擎前缀（`lok.*` / `pdf.*` / `md.*`），外壳校验前缀与活动标签一致 |
| worker 泄漏 | 长时间使用后进程数增长 | 引擎实例引用计数归零即回收；契约测试 C-06 检查进程数 |
| 界面线程阻塞 | 打开大文件卡死 | 所有引擎调用经 `Tabs::read_in` 走后台执行器（上游已有） |
| 混排时状态串扰 | 一个引擎的撤销影响到另一个 | 每标签独立 `Extra` + 引擎实例按引擎划分，不共享可变状态 |

---

## 6. 与上游外壳的复用边界

| 上游模块 | 复用程度 | 说明 |
| --- | --- | --- |
| `ely_office::Core` 的会话/查找/缩放/关闭/保存状态机 | **原样** | 与引擎类型无关的部分全部保留 |
| `ely_office::Tab` | **改造** | 引擎字段换为多态；`state_key()`、`dirty()` 逻辑保留 |
| `ely_office::Host` | **扩展** | 新增 `HostDyn` 变体；旧的 `Host` 保留给旧 app |
| `ely_office::lok`（LOK 专用命令表） | **下沉** | 移入 `LokEngine`，对外只暴露 `commands()` |
| `ely_office::find`（查找替换） | **复用 + 参数化** | 查找的引擎相关部分下沉到引擎；外壳保留 UI |
| `ely_office::chrome` / `panel` / `menu` / `zoom` | **原样** | 已是外壳级 UI 组件 |
