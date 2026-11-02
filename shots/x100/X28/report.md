# X28 · 折叠圆点 / 快速加子按钮 / 关系线手柄 / 注解手柄

> 合同：`docs/24` §阶段 C「X28 重做折叠圆点、快速加子按钮、关系线手柄和注解手柄」。
> 设计来源：`docs/30` §5.3（概念节点与卡片圆点）、§5.4（连线与链接短语）、§5.10（验收场景表里
> 端点手柄 / 跨度手柄两张）、§5.12（切换型交互要拍「撤下」）、§5.13（拖拽要拍「松手之后」）；
> `docs/28` §4.3（动作按需出现）、§5.6（拖拽阈值 4px / 禁止落点）；`docs/02` §3（双击下钻、折叠语义）、
> §10（快捷键）。
> 相邻已收口结论：`shots/x100/M9b/`（整图折叠与聚焦视图下的圆点）、`shots/x100/M17/`（悬停 1.3 倍 120ms）、
> `shots/x100/M32/`（双击下钻）、`shots/x100/M26/`（选中互斥）、`shots/x100/X27/report.md`（8 状态 × 3 表面）。
> 状态：**进行中**（本轮只做文档与只读审计：代码零改动、金样未拍、门未跑）。

---

## 0. 一页结论

| 对象 | 结论 | 主证据 |
| --- | --- | --- |
| 折叠圆点 | 几何/命中/显隐/悬停/双击下钻**都已实现**，屏幕空间恒定；根节点与概念图不出（合规） | `shots/x100/M9b/`、`shots/x100/M32/`、`shots/x100/M17/` |
| 快速加子按钮 | 有（悬停工具条 `+`，24 css px 屏幕恒定，点击进编辑态）；**只在悬停出现，选中态不出**；提交撤销粒度是两条 | `canvas/canvas_methods_3.rs:3492`、`canvas.rs:168`、`canvas/canvas_methods_2.rs:585` |
| 关系线手柄 | 有，但**只有终点可拖**：起点画 2.6 css px 实心点（装饰态）、终点画 4.5 css px 白底圆（手柄态），命中**只判终点 9 css px**；C3 的 12 css px 是**目标吸附**半径，与手柄命中不是一回事 | `canvas/canvas_methods_2.rs:2717-2740`、`:2137-2151`、`canvas/consts.rs:191-203` |
| 注解手柄 | 有（上下两端，命中 9 / 视觉 4.5）；拖动改跨度发一条 `SetSummaryTargets` / `SetBoundaryTargets`；键盘只有「新建」没有「调整」 | `canvas/consts.rs:202-203`、`canvas/canvas_methods_2.rs:2671` |

- **差距 9 条**：高 1（D5 Esc 取消可能进不去）、中 5（D1/D2/D3/D6/D9）、低 3（D4/D7/D8）。
- **不需要新增数据模型字段**：4 个对象全是交互层/派生几何，涉及的命令（`SetCollapsed` /
  `SetSummaryTargets` / `SetBoundaryTargets` / `RetargetLink` / `AddNode`）都已存在。
- **需要 Lead 拍板的行为口径 3 条**：D1（选中态是否也出加子按钮）、D2（新建 + 提交是否合并成一条撤销）、
  D3（关系线是否要起点手柄）。
- 与 X27 的关系：X27 的 I1–I4（悬停环 1.5 vs 1px、选中环 2.0 vs 1.5、过渡 180 vs 140ms、
  焦点环不透明 vs 80%）本包**不重复判定**，见 `shots/x100/X27/report.md` §3。

---

## 1. 范围与非目标

### 1.1 范围

1. 四个对象在**画布**上的现状测绘：几何、命中、状态（显隐/悬停/选中/编辑）、键盘路径、导出。
2. 差距清单：每条带严重度、是否要动数据模型、是否要拍板。
3. 量化验收草案（像素判据 / 轨迹判据）。
4. 预计 golden 场景名单（只列名字）。

### 1.2 非目标

- **不改 `crates/`**（本轮只读审计，并行会话在改 `crates/`）；不拍图、不跑 `cargo`、不登记门。
- 不重复 X27 的 I1–I4 判定，也不在这里裁决。
- 不覆盖大纲侧 chevron（`docs/28` §5.2 的行内折叠）与检查器圆点（`docs/30` §4.2 色板）。
- 不改「全展开 / 全折叠」「聚焦视图」「框选/套索」的既有口径。

---

## 2. 现状表

> 行号是快照，同时写出**那一行的字面**；路径相对仓库根。

### 2.0 总览

| 维度 | 折叠圆点 | 快速加子 `+` | 关系线手柄 | 注解手柄 |
| --- | --- | --- | --- | --- |
| 几何 | r 8 环 + 1.5 halo，位置在节点右/左中线外侧 | 24×24（`control::HIT`），节点外 `HOVER_GAP` | 起点 2.6 实心点 + 终点 r 4.5 圆（+1.5 halo / 1.5 描边） | r 4.5 圆点（+1.5 halo / 1.5 描边），压在手括号/边界缘上下两端 |
| 命中 | 10 css px（`TOGGLE_R + 2`），优先级 7 | 整个 24×24 | **只终点** 9 css px | 9 css px |
| 状态 | 悬停 1.3 倍 120ms；折叠/展开两形态；**有父级且有子级**才画 | 编辑/拖拽/移动/连线/浮层时消失；**只在悬停节点时出现** | 只在关系线被选中时出现；拖动时换预览线 | 只在注解被选中时出现；拖动时换预览框 || 键盘路径 | `Ctrl+/`、`Ctrl+Shift+/`、`/`；双击=下钻（`Ctrl+Z` 一条撤销） | `Tab`（与按钮同源 `add_child`） | 无（只能用鼠标拖） | 只有「新建」（`Ctrl+B` / `Ctrl+]`），没有调整/删除 |
| 导出 | **导出有**（SVG/PDF 都用 `toggle_pos` 画圆点） | 不导出（egui 覆盖层） | 不导出（egui 覆盖层） | 不导出（手柄是覆盖层；注解本体在 Scene 里，会导出） |

### 2.1 折叠圆点

| 维度 | 取值 | 出处 | 状态 |
| --- | --- | --- | --- |
| 几何 | 描边环（非实心点）r = **8 css px**、环宽 1.5、`+`/`−` 横竖线 3.5；环外再垫一圈**画布色**底、外扩 **1.5**（挖掉穿过来的连线，避免「环粘在线上」） | `crates/mw-render/src/instances.rs:37` `pub const TOGGLE_R: f32 = 8.0;`、`:41` `TOGGLE_HALO = 1.5`、`:42` `TOGGLE_BAR = 3.5`；`:921-925` 注释「折叠圆点：一圈**描边环**（不是实心点），外加一圈比环大的画布色底」；`:937` `let r = cam.chrome(TOGGLE_R, 4.5);`（→ 屏幕空间恒定） | 有 |
| 位置 | 节点右侧中线再外移一档（镜像到左侧的规则在 `toggle_pos`，导出与壳层共用同一出口） | `crates/mw-core/src/layout.rs` 的 `toggle_pos`；调用方 `crates/mw-shell/src/export_svg.rs:321` `let local = toggle_pos(n.geom.rect, scene.kind, Some(scene.node(p).geom.rect.x));`、`export_pdf.rs:428` 同一行；测试侧 `canvas/tests.rs:14` `fn toggle_disc(node, on_left) { let x = if on_left { node.left() - TOGGLE_OFFSET } else { node.right() + TOGGLE_OFFSET }; … }` | 有 |
| 命中 | **10 css px**（`TOGGLE_R + 2.0`），编译期断言必须盖住 `r + halo`（9.5）；命中优先级 **7**，高于节点 | `crates/mw-render/src/hit.rs:32` `pub const TOGGLE_HIT_CSS: f32 = TOGGLE_R + 2.0;`、`:34` `const _: () = assert!(TOGGLE_HIT_CSS >= TOGGLE_R + TOGGLE_HALO);`、`:60` `Self::Toggle(_) => 7,`、`:318` `radius_css: TOGGLE_HIT_CSS,` | 有 |
| 显隐 | **有父级 + 有子级**才画：根节点永远没有圆点，叶子永远没有圆点；视图外裁剪用 `rect + r + 2` 判 `sees` | `instances.rs:598-600` `if let Some(h) = n.parent.and_then(\|p\| f.scene.nodes.get(p as usize)) { if n.has_children { push_toggle(`；`:595` `if !f.sees(n.geom.rect.expanded(cam.chrome(TOGGLE_R + 2.0, 4.5)))` | 有 |
| 悬停 | **1.0 → 1.3 倍**，`s = 1 + 0.3×alpha`，120ms（`motion::FAST`）淡入淡出；画在壳层覆盖层，未改 `Frame` ABI | `shots/x100/M17/report.md` §3.2；`crates/mw-shell/src/canvas/hover.rs:49` `camera.chrome(TOGGLE_R, 4.5) * camera.zoom`、`:107-109` halo/bar 同倍率换算 | 有 |
| 单击 | 折叠 ↔ 展开整棵；点击会把该节点变唯一选中（保留多选组不允许）；轨迹 `toggle idx=… collapsed=… kids=…` | `canvas/canvas_methods_3.rs:1553-1580` `pub(crate) fn toggle(&mut self, node: u32) -> bool`，内含 `self.select_node_only(node, "fold-dot");` 与 `"toggle idx={node} collapsed={} kids={}"` | 有 |
| 双击下钻 | 同一节点 + **500ms** 内 + 位移 ≤ **4 css px** + 撤销栈深未变 → 判双击；先撤掉第一击，再按 `mw_core::drill` 规划下发**一条** `Batch`；轨迹 `toggle_drill id=… depth=… open=…` | `crates/mw-ui/src/drill_click.rs:56-66`（`window_s: 0.5, slop_css: 4.0`）、`:97-108`（`is_double` 四条判据）；`crates/mw-core/src/drill.rs:22` `pub fn drill_toggle_command(doc: &Document, id: &str) -> Option<Command>`；`canvas/canvas_methods_2.rs:310-340`（命中分支）、`canvas_methods_3.rs:1595+` `pub(crate) fn toggle_drill` | 有 |
| 整图折叠 / 聚焦视图 | 整图折叠后「可见的带下级节点全是 ＋」，根不画；聚焦视图里**聚焦根不画**（裁剪后没有父级）、边界上的节点该画就画、视图外零残点 | `shots/x100/M9b/report.md` §4.1（两张基线 `mm-collapse-all-dots` / `mm-focus-dots`，`scene_toggle` 计数分别 3 / 1） | 有 |
| 概念图 | 不出：概念图是平铺结构（概念没有子级），`has_children` 恒假；M32 已定「概念图没有折叠语义」 | `instances.rs:599` 的 `n.has_children` 判据；`shots/x100/M32/report.md` §4 非目标 | 有 |
| 导出 | **导出里也有圆点**：SVG/PDF 用同一 `toggle_pos` 算位置后自己画 | `crates/mw-shell/src/export_svg.rs:19` `use mw_core::layout::{edge_path, toggle_pos, Edge};`、`:321`；`export_pdf.rs:18`、`:428` | 有 |

### 2.2 快速加子按钮（悬停工具条 `+`）

| 维度 | 取值 | 出处 | 状态 |
| --- | --- | --- | --- |
| 出现条件 | 只在**悬停节点**时出现；`editor` / `drag` / `moving` / `link` / `input_blocked` 任一为真则不出现；**选中但未悬停不出现** | `canvas/canvas_methods_3.rs:3492-3500` `pub(crate) fn hover_bar(&self, rect, viewport) -> Option<HoverBar> { if self.editor.is_some() \|\| self.drag.is_some() \|\| self.moving.is_some() \|\| self.link.is_some() \|\| self.input_blocked { return None; } let idx = self.hovered?;` | 有（见 D1） |
| 几何 | 按钮 **24×24 css px**（`design::control::HIT`），两个按钮间距 `space::XS`；工具条在节点右侧外 `HOVER_GAP = space::SM`，右侧放不下**镜像到左侧**，上下都试；都不行就不画（不压折叠点）；有卡片角标时再抬高一个按钮高 | `canvas/consts.rs` `pub(crate) const HOVER_BTN: f32 = design::control::HIT;`、`HOVER_BTN_GAP = design::space::XS`、`HOVER_GAP = design::space::SM`（注释「必须**大于** `mw_ui::drag::DROP_PAD_X`（6）」）；`mw-core/src/design.rs:553` `pub const HIT: f32 = 24.0;`；`canvas.rs:168-205` `fn hover_bar_rects(node, canvas, has_card) -> Option<(Rect, Rect)>`（右→左→上→下→放弃） | 有 |
| 命中与点击 | 命中区就是 24×24 矩形；点击先 `select_node_only(node, "hover-add")` 再 `add_child()`；轨迹 `node-plus`、控件名 `node-plus`；悬停提示「添加子主题（Tab）」 | `canvas/canvas_methods_2.rs:585-592` `crate::trace::event("node-plus"); self.select_node_only(b.node, "hover-add"); dirty \|= self.add_child();`、`:591` `.on_hover_text("添加子主题（Tab）")` | 有 |
| 点击后的编辑态 | 新节点**立刻进编辑态**（同一帧进场景与编辑器），并标记「这枚是本次编辑建出来的」：`ed.fresh = true` + `fresh_depth`；视图会把新节点滚进可见区 | `canvas/canvas_methods_3.rs:2104-2131` `add_relative`（`self.add_to(&sel_id, kind)` → `select_node_only(i, "add-relative")` → `start_edit(i, viewport)` → `ed.fresh = true; ed.fresh_depth = Some(depth);` → `reveal_node(i)`） | 有 |
| 撤销粒度 | 建节点本身是**一条** `AddNode`；提交文字另发一条 `SetText`（= 两步）。**Esc 取消**时 `cancel_fresh_node` 若发现栈顶仍是建它那条，就连节点一起收回（= 一步干净） | `canvas/canvas_methods_3.rs:4115-4126` `/// 栈顶仍是建它那条命令时（fresh_depth 与当前栈深相同），一次 undo 就把整个动作…` + `if fresh_depth == Some(doc.undo_depth()) && doc.undo().is_some()` | 有（见 D2） |
| 键盘等价 | `Tab` = 加子主题（与按钮同源 `add_child`）；`Enter` = 加同级 | `app_methods_2.rs:453` 注释「画布默认握着键盘——`Tab` 是「加子主题」，这是思维导图的肌肉记忆」 | 有 |
| 导出 | 不导出（egui 覆盖层手势，不入 `Scene`） | 结构事实：绘制在 `paint_hover_bar` / `ui.interact` 层 | 不导出（合规） |

### 2.3 关系线手柄

| 维度 | 取值 | 出处 | 状态 |
| --- | --- | --- | --- |
| 视觉 | **两端不对称**：起点（from 端）画 **2.6 css px 实心强调点**（无描边、无 halo，纯装饰）；终点（to 端）画 **r 4.5 css px** 圆：白底（`theme.ui.panel`）+ 1.5px 强调描边 + 1.5px 画布色 halo。被选中的关系线同时画 3px 强调色线 | `canvas/canvas_methods_2.rs:2717-2740` `pub(crate) fn paint_relationship_handles`：`painter.circle_filled(…, 2.6, accent);`（起点）、`painter.circle_filled(c, SPAN_HANDLE_R + 1.5, halo); painter.circle_filled(c, SPAN_HANDLE_R, fill); painter.circle_stroke(c, SPAN_HANDLE_R, Stroke::new(1.5, accent));`（终点）；`docs/30` §5.10「`cm-relink-select-only`：端点手柄 + 3px 强调线」 | 有（见 D3/D9） |
| 位置 | 终点手柄钉在 `path.end()`；起点只是画一个点，**不参与命中** | `canvas_methods_2.rs:2137-2151` `let end = sr.path.end(); let d = ((world.x - end.x).powi(2) + (world.y - end.y).powi(2)).sqrt(); (d <= hit).then_some(rel)` | 有（见 D3） |
| 命中 | **9 css px**，且只在**关系线已是主选**时可命中；注意它读的是 `SPAN_HANDLE_HIT`（注解手柄的常量），不是 `ANCHOR_HIT` | `canvas_methods_2.rs:2141` `let rel = self.relationship.primary().cloned()?;`、`:2147` `let hit = self.camera.css_to_world(SPAN_HANDLE_HIT);`；常量定义 `canvas/consts.rs:203` `pub(crate) const SPAN_HANDLE_HIT: f32 = 9.0;` | 有（见 D9） |
| 拖动 | 按下建 `RelinkDrag { sel, at, target: None }`；拖动时画预览线与落点提示；松手走 `commit_relink`：有目标 → 改指；**空白落点**在概念图里就地新建概念（`RetargetLinkToNewConcept`），在思维导图里直接 `return false`（静默取消） | `canvas_methods_2.rs:1471-1474`（`start_relink` → `relink_drag`）、`:1853-1854`（松手 `commit_relink`）、`:2155-2170` `start_relink`、`:2502-2532` `commit_relink`（`let Some(target) = rd.target else { if !self.concept_map() { return false; } … }`）、轨迹 `relink_new_concept from id -> new_id` | 有（见 D4） |
| 与 C3 的关系 | C3 的 **12 css px**（`LINK_SNAP_CSS`）是**目标吸附**半径：手抖的落点差 4–10px 也读成「改到这颗」；它与「手柄自身命中 7px」是两个半径，验收要分开量 | `consts.rs:191-196` `/// 关系线改端点时的**吸附**距离（CSS px，屏幕空间恒定）。/// 12 的来历：…取 12 既能把「差几个像素」的意图读成「改到这颗」…（打磨轮 12，`cm-relink-near` 金样钉住）。` + `pub(crate) const LINK_SNAP_CSS: f32 = 12.0;` | 有（见 D8） |
| 取消 | `Esc` 取消分支写在 `if self.drag.is_some() && consume_key(Escape)` 里，同时清 `span_drag` / `relink_drag`；**只拖手柄（没有节点拖拽）时 `self.drag` 为 `None`，这条分支可能进不去** | `canvas_methods_2.rs:1425-1433` `if self.drag.is_some() && ui.input_mut(\|i\| i.consume_key(Modifiers::NONE, Key::Escape)) { self.drag = None; … self.span_drag = None; self.relink_drag = None; … }` | 待核（见 D5） |
| 键盘路径 | 无：改端点只能鼠标拖 | 无快捷键登记（`docs/02` §10 无此项） | 缺失（见 D6） |
| 导出 | 不导出（手柄是 egui 覆盖层；关系线本体在 `Scene` 里，正常导出） | 结构事实 | 不导出（合规） |

### 2.4 注解手柄（边界 / 概要）

| 维度 | 取值 | 出处 | 状态 |
| --- | --- | --- | --- |
| 视觉 | 上下两端各一颗：r **4.5 css px**，白底 + 1.5px 强调色描边 + 1.5px 画布色 halo（压在手括号/边界缘上） | `consts.rs:202` `pub(crate) const SPAN_HANDLE_R: f32 = 4.5;`；`canvas_methods_2.rs:2710-2712` `painter.circle_filled(c, SPAN_HANDLE_R + 1.5, halo); painter.circle_filled(c, SPAN_HANDLE_R, fill); painter.circle_stroke(c, SPAN_HANDLE_R, Stroke::new(1.5, accent));`（`:2738-2740` 同款第二段） | 有 |
| 位置 | 由选中注解的**跨度**算出上下两端（`SpanHandles { ann, top, bottom }`），跟着花括号/边界几何走 | `canvas.rs:491` `pub(crate) struct SpanHandles`；`canvas_methods_2.rs:2568-2581` `pub(crate) fn span_handles(&self) -> Option<SpanHandles> { … Some(SpanHandles { ann, top, bottom }) }` | 有 |
| 命中 | **9 css px**（`SPAN_HANDLE_HIT`），比视觉半径大 4.5 | `consts.rs:203` `pub(crate) const SPAN_HANDLE_HIT: f32 = 9.0;`；`canvas_methods_2.rs:2585+` `pub(crate) fn span_handle_at(…)` | 有 |
| 拖动 | 抓住某一端拖动 → `SpanDrag { preview, origin, start, end_idx }`，拖动中画一排强调色预览框；松手不同则发命令：概要 `SetSummaryTargets` / 边界 `SetBoundaryTargets`（一条即一次撤销）；轨迹 `span <label> <id> <start>..<end>`；预览与原值相同则**不发命令** | `canvas_methods_2.rs:2614` `span_drag_start`、`:2671-2687` `commit_span_drag`（`if sd.preview == sd.origin { return false; }` + `Command::SetSummaryTargets` / `SetBoundaryTargets` + `"span {} {} {}..{}"`） | 有 |
| 键盘路径 | 只有**新建**：`Ctrl+B` 加边界、`Ctrl+]` 加概要（与右键菜单同源、一次撤销；无选中/选了根给提示；概念图不响应）。**没有**「调整跨度」或「删除注解」的键盘路径 | `shots/x100/M24/report.md`（两场基线 `mm-boundary-key` / `mm-summary-key`、轨迹 `annotation_add_failed`）；`crates/mw-shell/src/app_methods_2.rs` 的 `A::AddBoundary` / `A::AddSummary` 动作映射 | 部分（见 D6） |
| 导出 | 手柄不导出（覆盖层）；注解本体（概要与边界几何）在 `Scene` 里，正常导出 | 结构事实；基线 `mm-summary-select` / `mm-boundary-select`（`docs/30` §5.10） | 不导出（合规） |

---

## 3. 差距清单

严重度口径：高 = 会丢用户操作或状态错乱；中 = 合同写过或明显该有但缺；低 = 手感/一致性。
「动模型」= 是否要改 `.zmind` / `Node` / `Relationship` 字段。

| ID | 差距 | 证据 | 严重度 | 动模型 | 要拍板 |
| --- | --- | --- | --- | --- | --- |
| **D1** | 快速加子按钮**只在悬停**节点时出现，选中态不出现。键盘焦点在画布时也不出现（只有 `Tab` 这条键路） | `canvas_methods_3.rs:3492-3500`（`let idx = self.hovered?;`）；`docs/28` §4.3「动作按需出现…在悬停/**选中**/选择模式出现」 | 中 | 否 | 是（选中态也出？） |
| **D2** | 加子后的撤销粒度：建节点一条 + 提交文字一条 = **两步**；只有 `Esc` 取消才合成一步 | `canvas_methods_3.rs:2104-2131`（`add_to` + `start_edit`）、`:4115-4126`（`cancel_fresh_node` 的合并只在取消时） | 中 | 否 | 是（要不要合并） |
| **D3** | 关系线**只有终点可拖**，起点不可拖；且起点画成 2.6 实心点、终点画成 4.5 白底手柄——两个端点在视觉上像同类，实际只有一端能拖，容易白拖一次 | `canvas_methods_2.rs:2717-2740`（两端绘制不对称）、`:2137-2151`（命中只判 `path.end()`） | 中 | 否（`RetargetLink` 够用，方向相反即可） | 是 |
| **D4** | 关系线拖到空白：概念图里新建概念（有提示），思维导图里**静默取消**（`return false`，无 toast） | `canvas_methods_2.rs:2502-2508` `if !self.concept_map() { return false; }` | 低 | 否 | 否（可顺手补 toast） |
| **D5** | `Esc` 取消拖拽的分支被 `self.drag.is_some()` 门住；只拖注解/关系线手柄时 `self.drag` 为 `None`，Esc 可能**取消不掉**（松手才生效） | `canvas_methods_2.rs:1425-1433`；`span_drag` 的另两处清空只在 `input_blocked`（`:1412`）与松手（`:1847`） | **高** | 否 | 否（先实机复核再改） |
| **D6** | 注解键盘路径只有「新建」，没有「调整跨度 / 删除注解」；关系线手柄完全没有键盘路径 | `shots/x100/M24/report.md` §2（只做 AddBoundary/AddSummary）；`docs/02` §10 无对应项 | 中 | 否 | 否（可排 X36） |
| **D7** | 圆点命中 **10 css px** 比视觉（r8 + halo1.5 = 9.5）大，且优先级 7 高于节点：大目标贴边时容易误折子树 | `mw-render/src/hit.rs:32-34`、`:60`；`canvas/tests.rs:14` 的 `toggle_disc` 用它断言「工具条没盖住圆点」 | 低 | 否 | 否 |
| **D8** | 三个半径容易被当成一个：手柄视觉 4.5 / 手柄命中 9 / 目标吸附 12（C3）；现有金样只钉了 12（`cm-relink-near`），**没有**钉「跨缩放档位下视觉半径与命中半径都不变」 | `consts.rs:191-203`；场景表 `docs/30` §5.10 | 低 | 否 | 否 |
| **D9** | 常量串用：关系线端点手柄的视觉与命中都借注解手柄的 `SPAN_HANDLE_R` / `SPAN_HANDLE_HIT`；而名字更贴切的 `ANCHOR_R` / `ANCHOR_HIT` 实际服务的是**连线锚点**（从节点边拖出建线，`mw_ui::link::nearest_anchor`）。改注解手柄尺寸会顺带改关系线手柄，反之亦然 | `canvas_methods_2.rs:2732-2740`（用 `SPAN_HANDLE_R`）、`:2147`（用 `SPAN_HANDLE_HIT`）、`:1466`（`ANCHOR_HIT` 用于 `nearest_anchor`）、`canvas.rs:1434-1435`（`ANCHOR_R` 用于连线锚点绘制） | 中 | 否 | 否（重建时拆成两组常量） |

> 说明：D7/D8/D9 是「命名与证据」缺口多于「功能错误」——实现已有编译期断言（`TOGGLE_HIT_CSS >= r + halo`）
> 与屏幕空间换算，欠的是**跨缩放档位的像素证据**与**两组常量的边界**（D9 的串用会让改一边伤另一边）。

---

## 4. 量化验收草案

统一纪律：hover / 点击 / 拖拽必须走 `MW_GOLDEN_INPUT` 脚本（`AGENTS.md` §7 第 6 条）；
拖拽要分「拖拽中」与「松手之后」两半（`docs/30` §5.13）；切换型交互要拍「加上」与「撤下」（§5.12）；
时长类判据用**帧数**不用墙钟（M16 的帧计数虚拟时钟）。

| 对象 | 场景要点 | 判据 | 已有基线 |
| --- | --- | --- | --- |
| 折叠圆点 · 几何恒定 | 同一文档在 `ppp 1.0` 与 `ppp 1.5` 各拍一张，指针都移开 | 圆点直径像素数 / ppp **相等**（±1px 抗锯齿）；圆点圆心到节点右边距离 / ppp 相等 | 新建 `mm-toggle-scale` |
| 折叠圆点 · 命中边界 | 指针停在圆心外 **9.4 / 10.1 css px** 各一张（脚本 `hover`） | 9.4 那张 `hover_detail` 轨迹是 `Toggle`、10.1 那张不是；像素上悬停倍率 1.3 只在命中那张出现 | 复用 `mm-toggle-miss` / `mm-toggle-edge`；新 `mm-toggle-hit-10` |
| 折叠圆点 · 悬停时序 | 同一目标 `wait 3` 与 `wait 8`（120ms ≈ 7.2 帧） | 3 帧那张圆点半径明显小于 8 帧（1.0 → 1.3 的中间态） | `mm-toggle-hover` |
| 折叠圆点 · 下钻 | 折叠 A → 双击圆点；再双击一次 | 轨迹 `toggle_drill id=… depth=1 open=…`；画面只露一层；`Ctrl+Z` **一次**回到双击前 | `mm-toggle-drill` / `mm-toggle-drill-deep` |
| 折叠圆点 · 整图/聚焦 | `Ctrl+/`；`MW_FOCUS_DEMO` | 可见带下级节点全为 ＋、根无点；聚焦根无点、边界点仍在、视图外零残点；`scene_toggle` 条数与表一致 | `mm-collapse-all-dots` / `mm-focus-dots` |
| 快速加子 · 出现条件 | 悬停节点（出）；选中不悬停（不出）；编辑中（不出）；拖拽中（不出） | 四种状态各一张，`node-plus` 轨迹的 `name` 只在前一种出现 | 新建 `mm-node-plus-states` |
| 快速加子 · 点击链路 | `hover` → `click` 到 `+` | 轨迹 `node-plus` + 新节点进编辑态（编辑器框出现、文本为空、光标可见）；一次 `Enter` 提交后文本落地 | 新建 `mm-node-plus-commit` |
| 快速加子 · 撤销粒度 | 同上，但提交后按 `Ctrl+Z` | 现状：第一次撤销回空文本、第二次才删节点（若 D2 拍板合并，则一次到底）；Esc 路径应一次干净 | 新建 `mm-node-plus-undo` |
| 快速加子 · 屏幕恒定 | 两个 ppp 各一张 | 按钮边长像素 / ppp = 24 ±1 | 新建 `mm-node-plus-scale` |
| 关系线手柄 · 选中态 | `MW_CONCEPT_SELECT_REL` | 终点是 4.5 css px 白底圆（+1.5 halo / 1.5 强调描边）、起点是 2.6 实心点、线与 3px 强调；**只有终点可拖** | `cm-relink-select-only` |
| 关系线手柄 · 拖动中 | `MW_CONCEPT_RELINK_DRAG` | 强调消失、换预览线 + 落空提示；与上张求交只剩手柄像素 | `cm-relink-drag` |
| 关系线手柄 · 吸附 12 | 落点距目标 11 / 13 css px 各一张 | 11 那张改到该概念、13 那张按落空处理 | `cm-relink-near`（已有 12 这一侧） |
| 关系线手柄 · 松手提交 | 拖到另一概念松手 | 轨迹 `relink_new_concept`（空白）/ 改指命令；`Ctrl+Z` 一次复原；`cm-relink-blank` 钉「落空 = 新建概念」 | `cm-relink-blank` |
| 注解手柄 · 选中态 | `MW_SUMMARY_SELECT` / `MW_BOUNDARY_SELECT` | 2px 选中环 + 两端手柄（4.5 css px，白底 + 1.5 强调描边） | `mm-summary-select` / `mm-boundary-select` |
| 注解手柄 · 拖动中 | `MW_SPAN_DRAG` | 一排强调色预览框；与选中态只差状态位 | `mm-span-drag` |
| 注解手柄 · 松手提交 | 拖一端松手 | 轨迹 `span 概要 … 0..2`；`SetSummaryTargets` 一条；`Ctrl+Z` 一次复原（`mm-span-commit` 的口径） | `mm-span-commit` |
| 注解手柄 · 取消 | 拖到一半按 `Esc` | 文档不变、无 `span` 轨迹、预览消失（**D5 未修前这条可能红**） | 新建 `mm-span-esc` |
| 注解手柄 · 多选 | 概要 + 边界同时选 | 两圈环的并集 + 两对手柄 | `mm-annot-multi` |

---

## 5. 预计 golden 场景名单（只列名字，本包不拍）

**新增（8）**：`mm-toggle-scale`、`mm-toggle-hit-10`、`mm-node-plus-states`、`mm-node-plus-commit`、
`mm-node-plus-undo`、`mm-node-plus-scale`、`mm-span-esc`、`mm-relink-anchor-scale`。

**复用已有（16）**：`mm-toggle-hover`、`mm-toggle-miss`、`mm-toggle-edge`、`mm-toggle-drill`、
`mm-toggle-drill-deep`、`mm-collapse-all-dots`、`mm-focus-dots`、`mm-summary-select`、`mm-summary-hover`、
`mm-boundary-select`、`mm-boundary-hover`、`mm-span-drag`、`mm-span-commit`、`mm-annot-multi`、
`cm-relink-select-only`、`cm-relink-drag`、`cm-relink-near`、`cm-relink-blank`、`cm-link-mode`、`mm-rel-link`。

> 登记门、拍基线、双跑都**不在本包**；本轮只交表与判据。

---

## 6. 未做 / 风险

### 6.1 未做（如实记账）

1. **代码零改动**：D1–D8 只记录，未落码（本轮不许改 `crates/`）。
2. **金样零拍摄**：§5 的 8 个新场景都没拍、没登记；§4 所有判据都还没被实拍验证。
3. **无 cargo / 无门**：未跑 `fmt` / `clippy` / `test` / `golden`。
4. **D5 未实机确认**：只读代码得出「Esc 取消可能进不去」，必须用 `mm-span-esc` / 关系线同款场景实测后才能定性。
5. **跨缩放手感未量化**：圆点/按钮/手柄都是屏幕空间恒定，但**没有**一张基线是「同一场景两个 ppp 对照」。
6. **注解手柄的成员增减**（把某个子节点加进/移出跨度）只能靠拖两端间接实现，没有独立手柄；`docs/30` 未定，属未做。
7. **关系线起点手柄**未做（D3），若要做需先拍板。
8. **X27 的 I1–I4 未在此处理**（悬停环宽 1.5 vs 1px、选中环 2.0 vs 1.5、过渡 180 vs 140ms、焦点环不透明 vs 80%），
   本包引用其结论，不重复判定。

### 6.2 风险

| ID | 等级 | 内容 | 缓解 |
| --- | --- | --- | --- |
| R1 | 高 | 并行会话正在改 `crates/`（`mw-render` / `mw-io` / 画布底色），本包引用的常量与行号随时漂 | 每格带「那一行的字面」；动码前重 grep 一次 `TOGGLE_R` / `SPAN_HANDLE_*` / `ANCHOR_*` |
| R2 | 中 | D1/D2/D3 任一拍板成「要做」，都要新增状态或命令路径 → 新轨迹 + 新基线，推高 stage-b 场次 | 优先零像素方案（轨迹 + toast），像素证据按 §4 的最小集补 |
| R3 | 中 | D5 若确认是 bug，属鼠标手势取消路径，改它要重跑所有拖拽类基线（`mm-span-*` / `cm-relink-*` / 拖拽重排） | 先补 `mm-span-esc` 一张红灯基线当证据，再改 |
| R4 | 低 | 双击下钻的 500ms/4px 判据依赖 egui 时间；金样是帧计数虚拟时钟，若脚本 `wait` 帧数不够会把双击拍成两次单击 | 脚本里两次点击之间 `wait ≤ 2` 帧，并在轨迹里断言只有一条 `toggle_drill` |
| R5 | 低 | 圆点命中 10 css px 与「工具条不得压圆点」的几何耦合（`HOVER_GAP_V` 要避开 r8+1.5） | 已有单测 `hover_bar_sits_outside_the_node_and_clears_the_fold_toggle` 守着；改任一边都要跑它 |

---

## 7. 写作边界

- 本包只写 `shots/x100/X28/`；未改 `crates/`、`docs/`、`scripts/`、`docs/STATUS.md`、其他 `shots/x100/*`；未 commit。
- 只读命令：`grep` / `sed -n` / `ls`；未执行 `cargo` 或 golden。
- 状态词只用「未开始 / 进行中 / 延后」；本包不写「已完成 / 已验证」。
