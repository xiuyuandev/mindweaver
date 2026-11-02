# CM-15 剩余三项 · 字段与互导提案（2026-11-02）

> **目的**：`docs/30` §12.2 的 CM-15 复审按 CmapTools 编辑能力对账 23 项，已有 15、缺 8；
> 其中 18 / 19 已由 CM-16 收口、17 已由 CM-18 收口、20（多选关系线）已由 CM-17 收口、
> 22（复制/粘贴样式入口）已补。**剩下三项**：⑯ 嵌套概念、㉑ 命名样式、㉓ `.cxl` 互导。
> ⑯ 与 ㉑ 卡在 `docs/30` §5.2「禁止新增 `.zmind` 字段」，按仓库纪律**必须用户拍板**才能动；
> ㉓ 不涉及字段，是导入导出层的事。本文件就是拍板材料——**不代替用户决定、不改合同**。
>
> **授权原话（2026-11-02，`docs/STATUS.md:17` 与 `AGENTS.md` §2 都记着）**：
> 「继续开发，而且呢，你帮助我验收，不需要我决定。」
> ——授权的是**验收代判**；「新增 `.zmind` 字段」明确不在代判范围（`docs/32` §12/§13/§15
> 三次都是先拍板再落码），所以本文件只出提案。
>
> **状态词口径**：本文件只用 `未开始` / `进行中` / `延后` 三词，不写「已完成」「已验证」，
> 也不写「已批准」。快照：2026-11-02 20:2x；**行号是快照，每条引用都写出那一行长什么样**。
>
> **本轮纪律**：只写文档。不改 `crates/`、不改 `.zmind`、不跑 golden、不 commit。
> 本文件里的代码引用都是**只读核对**的结果。

---

## 0. 一页结论

| 项 | 要什么能力 | 是否要新字段 | 不升 `FORMAT_VERSION` 可行？ | 动哪些 crate | 工作量 | 状态 | 我的建议 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **⑯ 嵌套概念** | 一个概念「装」住别的概念 | **要**，且要动**结构语义**（不是属性） | 技术上行（可选键 + 缺键回落），但语义代价大 | `mw-core`（模型/命令/布局/大纲/校验）、`mw-format`、`mw-shell`、`mw-io` | 大 | `未开始` | **延后**（单独一批、先写验收场景） |
| **㉑ 命名样式** | 「样式名 → 一组属性」，一键套用 | **要 1 个可选键**：`conceptMap.styles` | **行**：空数组不写、缺键 = 没有命名样式、旧版忽略 | `mw-core`（模型/命令/主题解析）、`mw-format`（块映射）、`mw-shell`（面板/菜单） | 中 | `未开始` | **批准后第二优先** |
| **㉓ `.cxl` 互导** | 与 CmapTools 交换概念图文件 | **不要**（复用保真槽 + XML 互通映射） | 不涉及 | `mw-io`（代码已在）、`mw-shell`（分发已在） | 小–中（**代码已落，缺证据与边界**） | `进行中` | **先做**（三项里唯一零字段成本） |

**一句话**：三项里真正需要新增 `.zmind` 字段的只有 ⑯ 与 ㉑；而两项都能塞进
`docs/33` §3 已经落地的**可选** `conceptMap` 块（`crates/mw-format/src/concept_map.rs`
的 `ConceptMapBlock`），不必碰 `Node`、不必碰树模型、不必升版本。⑯ 虽然「技术上可行」，
但要做的事等于在命题网里再插一层包含关系——**能力本身值与不值，得你拍**。

---

## 1. ⑯ 嵌套概念（一个概念里装着别的概念）

### 1.1 是什么能力

给一颗概念挂「内部概念」：外层概念撑大成一个容器，里层概念画在容器内部，
移动外层时里层跟着走。对 CmapTools 用户来说，这就是「把一张大图拆成几块、每块能整块搬」。

### 1.2 CmapTools 与 XMind 各怎么做

| 工具 | 做法 | 证据强度 |
| --- | --- | --- |
| XMind | **子主题**：父主题右侧挂子主题，是**树结构**（`children`），布局、大纲、导出全按树走 | 强：织思自己的思维导图就是这套（`Node.parent_id` / `Node.children`，`crates/mw-core/src/model.rs:230-231`），`docs/30` §12.2 第 16 项也这么记 |
| CmapTools | 没有「概念里装概念」的一等公民；相近能力是**把子图另存为一张 cmap、用资源链接打开**（下钻），以及用命题把概念连起来 | **弱**：本机没有 CmapTools，未真机复核；`docs/30` §12.2 的表述是「一个概念『装』别的概念」，来自对账表而非实机操作 |

> 这一条必须说清：⑯ 的**结构范式来自 XMind（树）**，而织思概念图从第一天起就规定
> 「**不用** `parent_id`/`children` 表达层级」（`docs/30` §5.2，
> `crates/mw-core/src/document.rs` 的 `Document::validate` 里也把它钉成了不变量（850 行那条 `pos` 规则））。所以 ⑯ 不是「加个功能」，
> 是「在命题网里开第二个结构范式」。**拍板前请先确认：你要的到底是「概念套概念」，
> 还是「一个概念能展开成另一张图」。**

### 1.3 现有实现差在哪（只读核对）

| 落点 | 现状 | 为什么挡住了嵌套 |
| --- | --- | --- |
| `crates/mw-core/src/model.rs:230` | `pub parent_id: Option<String>,` | 字段本身在，但概念图里**必须**为 `None`（见下一行） |
| `crates/mw-core/src/document.rs:850` | `if n.parent_id.is_some() \|\| n.id == self.root_id {` | 带 `pos` 的节点只允许「浮动根」；下面 853–855 行是那个例外的条件 —— `let concept_root = n.id == self.root_id` + `&& n.parent_id.is_none()` + `&& self.meta.layout.kind == "conceptMap";`，**条件里明确要求 `parent_id.is_none()`**，所以有父节点的概念拿不到 `pos`：一加嵌套就与「自由摆放」直接打架 |
| `crates/mw-core/src/concept_tree.rs:43` | `pub fn concept_rows(doc: &Document) -> Vec<ConceptRow> {` | 概念图大纲只按 `relationships` 生成树；嵌套成员在大纲里**看不见**（=「大纲里没有、导出里丢了」的半吊子状态） |
| `crates/mw-core/src/concept_layout.rs:127` / `:145` | `pub fn arrange(input: Arrange<'_>) -> Vec<(f32, f32)>` / `arrange_scoped(...)` | 分层只认命题的源/目标；容器与成员的关系要重写分层规则 |
| `crates/mw-io/src/export.rs:72` / `:119` / `:231` / `:344` | 四处 `mw_core::concept_rows(doc)` | Markdown / OPML 导出按生成树走；嵌套不认就会丢成员 |
| `crates/mw-shell/src/panels/outline.rs:2160` | `let rows = mw_core::concept_rows(doc);` | 大纲面板同上 |
| `crates/mw-format/src/concept_map.rs:38` | `pub struct ConceptEntry {`（`id` / `text` / `x` / `y` / `pinned` / `style`，**没有 parent**） | v2 专属块里也没有包含关系的位置 |

结论：**缺的不是一个键，是「包含关系」在六处消费点的定义**——大纲、布局、导出（两处）、
检索（命中后的 reveal 要不要自动展开容器）、`validate`。这就是 `docs/32` §12.3 里写「不建议用再加一个可选键的方式做」的原因。

### 1.4 需要哪些字段或结构（三个方案，含精确键名）

**方案 A（推荐，若真要拍板）—— `conceptMap.containment` 边表**

```jsonc
"conceptMap": {
  // ……concepts / propositions / focusQuestion / groups 不动……
  "containment": [ { "outer": "n3", "inner": "n7" } ]
}
```

- Rust：`concept_map::Containment { outer: String, inner: String }`，
  挂在 `ConceptMapBlock` 上：`#[serde(default, skip_serializing_if = "Vec::is_empty")] pub containment: Vec<Containment>`。
- **不碰 `Node`**：概念仍是浮点概念，`pos` 仍由自己说了算。
- 缺键 = 没有包含关系 = 今天的行为，一个字节都不变。
- 不变量（进 `validate`）：`outer`/`inner` 必须存在；`inner` 只能有一个 `outer`；
  自己不能包含自己；禁止成环（A 含 B、B 含 A）。
- **必须同时定死的一条**：容器移动时成员怎么走——写「`outer` 移动 → 成员按 `pos` 平移同样的
  `dx,dy`」（命令层一次 `Batch[MoveConcept]`），否则容器与成员会各走各的。

**方案 B（不推荐）—— `ConceptEntry.parent`**

- 在 `concepts` 表里加 `"parent": "n3"`。看着最像 XMind，代价最贵：等于把 `parent_id`
  语义灌回概念图，`docs/30` §5.2 的根规则要改，`document.rs:850` 的 `pos` 规则要改，
  `parent_id`/`children` 的对称性也要一起维护。**不建议**。

**方案 C（替代品，成本最低）—— 概念挂「子图链接」，不做视觉嵌套**

- `"drills": [ { "concept": "n3", "target": "子图.zmind" } ]`（或存文档内嵌 id）。
- 效果是「点概念 → 下钻到另一张图」，几何零改动、布局零改动、大纲零改动。
- 代价：不是「视觉上套住」，与 ⑯ 的原始期待有偏差；还要定「目标文件丢失」的行为。

### 1.5 不升 `FORMAT_VERSION` 是否可行

**技术可行**（三个方案都是可选键 + 缺省回落 + `FileDoc` 的 `flatten` 还能把旧版不认识的键
原样留住——`crates/mw-format/src/lib.rs:100` 那行注释写的就是这件事：
「`flatten` 让它们读进来、存出去的形状都不变；**新版本写的字段被旧版本打开再存不会消失**」）。
**但这不等于便宜**：方案 A 会新增一条跨六处消费点的结构不变量，`validate` 一旦放行，
错文件就是永久性的（含环、多父、父已删）。**建议按 `docs/32` §12.3 的结论单独一批做。**

### 1.6 迁移与回滚路径

- 迁移：不需要。缺键 = 无包含关系；旧文件读进来一个字节不变。
- 回滚（撤掉这个功能）：删键 + 删命令 + 删 `validate` 三条不变量 + 删布局/大纲/导出四处分支。
  已落盘的文件里 `containment` 会变成旧版 `extra` 里的未知键**原样保留**（不丢数据），
  但旧版不认它 → 表现成「没有嵌套」。**这一点必须在报告里写清楚，不能让用户以为数据丢了。**

### 1.7 影响哪个 crate

`mw-core`（`model.rs` / `document.rs` 的 validate / `concept_tree.rs` / `concept_layout.rs` /
`command.rs` 新命令 / `scene.rs` 画容器）、`mw-format`（`concept_map.rs` 块映射 +
`round_trip` 双向测试）、`mw-shell`（大纲、命中、拖动、菜单）、`mw-io`（Markdown/OPML 四处导出）。
`mw-render` 只多一个「容器底/边」的画法，属常规。

### 1.8 工作量：**大**

单看代码量像中，但它是「第二个结构范式」：六处消费点都要定义清楚，还要 5 张以上新基线
（建容器、移容器、拖入/拖出、删外层、导出），按 `docs/32` §12.3 的口径估。

### 1.9 不做的代价

- CmapTools 那边「整块搬运」的体验没有；但**可用 ⑰ 分组 + 命题近似**（分组已落地）。
- 用户目前的真实反馈是「概念之间连线不足」，已由 CM-14 收口；**没有实证需求指向嵌套**。
- 题外话：`docs/30` §12.2 第 16 项把「嵌套」列为 CmapTools 能力，但 CmapTools 侧我未真机复核——
  可能是把 XMind 的树能力错记到了 CmapTools 名下。**这条在拍板前值得先确认**。

---

## 2. ㉑ 命名样式（Styles 面板：存一组样式、一键套用）

### 2.1 是什么能力

把一组外观（填色 / 文字色 / 描边 / 字重 / 对齐 / 圆角 / 形状）存成**有名字的一档**；
选中若干概念 → 套用它；以后改这一档的定义，所有套用过的概念跟着变（详见 2.4 的 v1/v2 取舍）。

### 2.2 CmapTools 与 XMind 各怎么做

| 工具 | 做法 | 证据强度 |
| --- | --- | --- |
| CmapTools | 「Styles」面板里存预定义样式，可改名、可更新，应用到选中概念 | 中：来自 `docs/30` §12.2 第 21 项的对账表述，未真机复核 |
| XMind | 画布级「主题」+ 节点级「样式」面板；织思已对标：四套主题 + `StyleOverride` 单节点覆盖 | 强：`crates/mw-core/src/theme.rs:1294` `pub fn resolve_all(` 就是「主题 → 每节点解析」那一层 |

### 2.3 现有实现差在哪（只读核对）

| 落点 | 现状 | 缺什么 |
| --- | --- | --- |
| `crates/mw-core/src/model.rs:21` | `pub struct StyleOverride {`（`fill`/`text_color`/`stroke`/`stroke_width`/`bold`/`italic`/`font_size`/`shape`/`radius`/`align`/`weight`/`branch_color`/`apply_text_to_subtree`） | 属性载体**齐了**（SE-13 今天刚补 `weight`/`align`/`radius`，`docs/32` §15）——缺的是「**名字 + 一组属性的存储**」 |
| `crates/mw-core/src/command.rs:59` | `SetStyle {` | 一次只能改一个/一批节点的样式；没有「按名字套用」 |
| `crates/mw-format/src/concept_map.rs:38` | `pub struct ConceptEntry {` 有 `style: Option<StyleOverride>` | 概念能带样式值，但没有「样式定义表」 |
| `crates/mw-core/src/model.rs:281` | `pub struct LayoutSettings {`（含 `focus_question: Option<String>` 这个先例） | 放的是布局参数，不是样式表 |
| CM-15 第 22 项 | 概念节点菜单已有「复制样式 / 粘贴样式」（2026-11-01 补的入口） | 只能**当场**复制一次，不能存下来、不能跨会话复用 |

### 2.4 需要哪些字段或结构（v1 / v2 两种粒度）

**v1（推荐拍这个）—— 只加 1 个顶层块内的键，套用 = 复制式**

```rust
// crates/mw-core/src/model.rs（新结构，仅供概念图用）
pub struct NamedStyle {
    pub id: String,
    pub name: String,
    /// 复用现有 StyleOverride，不新增任何渲染键。
    pub style: StyleOverride,
}
```

```jsonc
"conceptMap": {
  // ……concepts / propositions / focusQuestion / groups 不动……
  "styles": [
    { "id": "st-1", "name": "主概念", "style": { "fill": "#FFF7E0", "shape": "ellipse" } }
  ]
}
```

- Rust 挂点：`crates/mw-format/src/concept_map.rs` 的 `ConceptMapBlock` 加
  `#[serde(default, skip_serializing_if = "Vec::is_empty")] pub styles: Vec<NamedStyle>`。
- **缺键 = 没有命名样式**（菜单里那块不出现），老文件与既有基线一个字节不变。
- **套用 = 复制**：`Command::ApplyNamedStyle { nodes, style_id }` 展开成
  `Batch[SetStyle × N]`（`SetStyle` 已存在，`command.rs:59`），一次撤销。
  不写 `Node` 新字段 → **不动 `Node`、不动 `concepts` 表、不进 `validate` 新不变量**。
- 增删改：`SaveNamedStyle` / `RenameNamedStyle` / `DeleteNamedStyle` 三条命令，各一条撤销。
- v1 明说**不联动**：改了 `st-1` 的定义，已经套用过的概念**不自动变**（要再点一次「套用」）。

**v2（现在不做，写清楚为什么）—— 加引用 `ConceptEntry.styleName: Option<String>`**

- 好处：改定义 → 所有引用方跟着变（真「样式」）。
- 代价：引用会让「改一次样式定义 → 文档里 N 个概念同时变」，撤销粒度、检查器里的「本节点
  覆盖 vs 样式」优先级、导出都要重定义；还会与「复制/粘贴样式」并存出两条路。
- **建议**：v1 跑一轮、确认真的需要联动再拍 v2。

### 2.5 不升 `FORMAT_VERSION` 是否可行

**可行**，且是本轮三项里代价最低的一次字段变更：

1. 新键在 `conceptMap` **块内部**（`docs/33` §3 那个块本来就是「可选、缺省回落」的设计），
   不是 `FileDoc` 顶层，思维导图的 `content.json` 完全不受影响；
2. 空数组不写 → 没存过样式的文档逐字节不变；
3. 旧版读新文件：`ConceptMapBlock` 的 serde 忽略未知键，`FileDoc` 的 `extra` flatten
   还会把它**原样带回去**（`crates/mw-format/src/lib.rs:100` 那行注释），打开再存不丢；
4. 反向兼容要判一条：**旧版读到 `styles` 会忽略** → 表现成「没有命名样式、概念本身外观不变」
   （因为 `style` 已经复制进各概念），这是可接受降级。

### 2.6 迁移与回滚路径

- 迁移：无。缺键 = 空表。
- 回滚：删 `NamedStyle` + 删 4 条命令 + 删面板分组；已落盘文件的 `styles` 变成旧版未知键保留。
- 与 ⑯ 不同，这条**没有跨消费点的结构不变量**，回滚干净。

### 2.7 影响哪个 crate

`mw-core`（`model.rs` 新结构、`command.rs` 4 条命令、`theme.rs` 在 `resolve_all` 之前
把「复制式套用」落到 `Node.style`——v1 其实不需要改 `resolve_all`）、
`mw-format`（`concept_map.rs` + 一条 `round_trip` 用例）、
`mw-shell`（检查器「样式」分组里加「命名样式」小段 + 概念菜单「套用样式 ›」子菜单）。
`mw-io` / `mw-render`：**不改**。

### 2.8 工作量：**中**

字段 + 命令 + 面板 = 一天量级；麻烦的是面板与菜单的几何（要走 `docs/30` §5.11 的浮层夹取规则）
和一轮基线（`cm-inspector` 系至少 1–2 张要重拍）。

### 2.9 不做的代价

每个概念换一次外观要点 4–6 个控件；跨文档复用只能靠「复制样式 / 粘贴样式」当场搬。
对「同一张图里主概念 / 中间概念 / 叶子概念三档风格」这种典型用法，成本是线性的。

---

## 3. ㉓ `.cxl` 互导（与 CmapTools 交换概念图）

### 3.1 是什么能力

把织思概念图导出成 CmapTools 能打开的 `.cxl`，也能打开别人的 `.cxl`。

### 3.2 CmapTools 与 XMind 各怎么做

| 工具 | 格式 | 说明 |
| --- | --- | --- |
| CmapTools | `.cxl`（XML，`cmap.xsd`）：`concept-list` / `linking-phrase-list` / `connection-list` / 三张 `*-appearance-list` | 结构见 `docs/34` §4.1 |
| XMind | `.xmind`（ZIP + `content.json`） | 已由 `mw-io::xmind` 覆盖（`docs/34` §3） |

### 3.3 现有实现差在哪（**重要：代码已经在了，缺的是验收**）

| 落点 | 现状 |
| --- | --- |
| `crates/mw-io/src/cmap.rs:30` | `pub fn save_cxl(doc: &Document) -> Result<String, IoError> {` |
| `crates/mw-io/src/cmap.rs:274` | `pub fn load_cxl(xml: &str) -> Result<Document, IoError> {` |
| `crates/mw-io/src/cmap.rs:305` | `fn parse_cxl_interop(xml: &str) -> Result<Document, IoError> {`（无保真槽时的降级解析） |
| `crates/mw-io/src/cmap.rs:507` / `:522` / `:537` | `cxl_roundtrips_fidelity_lossless` / `cxl_interop_has_concepts_phrases_connections_and_focus` / `cxl_degraded_parse_without_fidelity` 三条单测 |
| `crates/mw-io/src/dispatch.rs:40` | `"cxl" \| "cmap" => DocFormat::Cxl,` |
| `docs/34` §11 | 已把「概念图 `.cxl`」列进「已实现（非视觉，已测）」 |

所以 ㉓ 的状态是 `进行中`，**不是**「没做」，也**不是**「做完了」——缺的是下面这张清单。
这也是它与 ⑯/㉑ 最不一样的地方：**不需要任何字段批准**。

### 3.4 需要哪些字段或结构

**不需要。** 保真槽复用 `mw-format::doc_to_fidelity_json` / `doc_from_fidelity_json`
（`docs/34` §2/§4.3，整份 `FileDoc` JSON 塞进 `<extra-properties-list>`），
互通映射全在 XML 侧。这也意味着：**`groups` / `pinned` / 线型线宽只在保真槽里活**——
CmapTools 打开时看不见，再存回来（没走保真槽）就丢。

### 3.5 还差什么（要补的验收清单）

| # | 事项 | 判据 |
| --- | --- | --- |
| 1 | **证据**：一个新 `.cxl` 输入脚本 + 导出快照 + 往返测试落盘到 `shots/x100/` | 像 `cm-*` 那样有基线或字节快照；不能只有单测 |
| 2 | **门表登记**：把 `.cxl` 往返纳入门（或单独一包） | 要么进 `stage-cm`，要么在 `docs/31` 开一个互导小包 |
| 3 | **降级路径**：第三方 `.cxl`（无保真槽）读进来之后再存，信息损失要**写明并测出来** | 至少一条用例：无保真槽 → 解析 → 回写，说明丢了什么 |
| 4 | **边界文档**：`groups` / `pinned` / 线型在 CmapTools 侧不可见 | 写进用户手册（`docs/15`）或 `docs/34` |
| 5 | **真样本**：CmapTools 实际导出的 `.cxl` 文件（本机没有） | 可能要你从 CmapTools 导一份给我；没有真样本就只能验「自产自销」 |

### 3.6 不升 `FORMAT_VERSION` 是否可行

**不涉及**（`.cxl` 是导出格式，`.zmind` 一个字节都不变）。

### 3.7 迁移与回滚路径

无迁移。回滚 = 删 `cmap.rs` + 删 `dispatch.rs` 那行路由（注册表退回 `.zmind`/`.xmind`/`.md`）。

### 3.8 影响哪个 crate

`mw-io`（唯一实现处）、`mw-shell`（打开/另存为的过滤器与默认扩展名，已在 `docs/34` §11 记过）。
`mw-core` / `mw-format` / `mw-render` 不动。

### 3.9 不做的代价

与 CmapTools 用户交换文件不可能；`docs/34` 已把 `.cxl` 写成「新文档默认扩展名」
（`dispatch.rs:49` 那行 `".cxl"`），不做就等于默认格式打不开。

### 3.10 工作量：**小–中**

代码已在，主要是：补一份证据包 + 决定是否进门表 + 写清损失边界。**不需要任何数据决策。**

---

## 4. 如果只准挑一项：挑 **㉓ `.cxl` 互导**

三条理由，按权重排：

1. **零字段成本、零数据风险**：不需要动 `docs/30` §5.2 的例外、不需要迁移与回滚设计；
   已经写好的代码今天就能被证据包收口。
2. **它已经在影响产品行为**：`docs/34` 把概念图新文档的默认扩展名定成了 `.cxl`
   （`crates/mw-io/src/dispatch.rs:49` 的 `".cxl"`），没有验收的默认格式是负债。
3. **它能反向验证 `conceptMap` 块**（`docs/33` §3）：跨工具走一遍，能顺带量出
   「命题升到图层」这套新形状在真实数据上的完整度。

第二优先是 **㉑ 命名样式**（一次干净的小字段变更，`conceptMap.styles`，不碰 `Node`）；
**⑯ 嵌套概念建议延后**，理由见 §5。

---

## 5. 明确不建议在本阶段做的部分（及理由）

| 不做的事 | 理由 |
| --- | --- |
| **⑯ 嵌套概念整项（本轮/本阶段）** | ① 与 `docs/30` §5.2「不用 `parent_id`/`children` 表达层级」正面冲突，等于开第二个结构范式；② 六处消费点（大纲 / 布局 / 导出×2 / 检索 / `validate`）都要重定义，`document.rs:850` 的 `pos` 规则还得改；③ 没有用户实证需求（用户说的是「连线不足」，CM-14 已解决）；④ 「整块搬运」用 ⑰ 分组 + 命题已能近似 |
| **⑯ 的方案 B（`ConceptEntry.parent`）** | 把树语义灌回概念图，与根规则冲突最直接，收益不比方案 A 大 |
| **⑯ 的方案 C（子图下钻）现在做** | 想法低成本，但要新增「文件引用」语义（目标缺失、相对路径、重命名搬家），是另一类问题（属于 `docs/34` 的库/路径域），不该塞在本阶段 |
| **㉑ 的 v2（`styleName` 引用式联动）** | 撤销粒度与「本节点覆盖 vs 样式」优先级要重定义，且与「复制/粘贴样式」并存出两条路；先在 v1 上跑一轮再决定 |
| **㉓ 的「CmapTools 侧无损」** | 做不到：`groups` / `pinned` / 线型线宽只能进保真槽；诚实说法是「织思内无损，CmapTools 侧可见子集」 |
| **真 `.cmap` 老二进制格式** | `dispatch.rs` 只把 `.cmap` 当 `.cxl` 的扩展名别名；真正的旧 CmapTools `.cmap` 是另一套编码，本轮不碰、也不承诺 |
| **把 ⑯/㉑ 写进 `docs/30` / `docs/32` 合同** | 本轮只出提案。合同改动必须在你拍板之后、由落码那一棒按 `docs/32` 的格式补，避免出现「合同已批准、代码没有」的假批准 |

---

## 6. 拍板语句模板（想批哪条，抄一句给下一棒就行）

- ㉓：**「批准 ㉓：`.cxl` 互导按现有代码收口，补证据包与损失边界，不加字段。」**
- ㉑：**「批准 ㉑ v1：新增 `conceptMap.styles`（`NamedStyle { id, name, style }`），
  套用 = 复制式 `Batch[SetStyle]`，不加 `Node` 字段、不做联动。」**
- ⑯：**「批准 ⑯ 方案 A：新增 `conceptMap.containment: [{outer, inner}]`，
  单独一批、先写 5 张验收场景再落码。」**（或：「⑯ 延后，先不立项。」）
- 都不批：**「⑯/㉑/㉓ 全部延后，M 轨按 `shots/x100/CM-05/gate.md` §6 的「能立刻做」那一列排期。」**

---

## 7. 本文件的事实怎么复核（都只读）

```powershell
# 1) 字段现状：概念图 v2 块里有什么键
Get-Content crates/mw-format/src/concept_map.rs -TotalCount 50

# 2) pos 与 parent_id 的互斥规则（⑯ 的核心阻碍）
Select-String -Path crates/mw-core/src/document.rs -Pattern 'concept_root'

# 3) .cxl 已落码到什么程度
Select-String -Path crates/mw-io/src/cmap.rs -Pattern 'pub fn save_cxl|pub fn load_cxl|fn cxl_'
Select-String -Path crates/mw-io/src/dispatch.rs -Pattern 'cxl'

# 4) 未知键保真（旧版打开再存不丢新键）
Select-String -Path crates/mw-format/src/lib.rs -Pattern 'flatten'

# 5) 分组验证规则（新结构不变量该长什么样，照它写）
Select-String -Path crates/mw-core/src/document.rs -Pattern '分组不变量'
```

> 本文件**没有**改任何合同（`docs/30` / `docs/32`）、没有改代码、没有改 `docs/STATUS.md`。
> 三项状态：⑯ `未开始`、㉑ `未开始`、㉓ `进行中`（代码在、验收未完成）。
