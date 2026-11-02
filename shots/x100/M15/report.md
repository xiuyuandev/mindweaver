# M15 · 画布交互十项修复（打磨轮 31）

状态：**已实现**（十条全部落地，机器门槛绿：`--bins` 588 passed / `mw-ui` 42 passed；
人工门未开，暂不记 `已验证`）。

写权范围：`crates/mw-shell/src/canvas.rs`、`crates/mw-shell/src/canvas/**`、`crates/mw-ui/src/**`。
其余文件（`app_methods_*`、`main.rs`、`session.rs`、面板、`mw-core`、`mw-render`、`mw-format`）本轮**只读**。

---

## 1. 这一轮要解决什么

审计出的十条画布交互缺陷，全部落在「编辑态按键 / 手势入口 / 破坏性操作 / 缩放区间」四类。
其中 1–3 是 P0（数据会丢或不可逆地丢），4–10 是 P1/P2（手感与语义错位）。

| # | 级别 | 现象 |
| - | - | - |
| 1 | P0 | 编辑态按 `Shift+Enter` 变成「提交 + 新建同级」 |
| 2 | P0 | 选中节点后直接敲字什么都不发生（必须先双击） |
| 3 | P0 | 选中带子节点的分支按 `Delete` 立即连子树删掉，无确认 |
| 4 | P1 | 拖进**折叠**的父节点松手，屏幕毫无变化 |
| 5 | P1 | 思维导图 `Ctrl+A` 没反应（只有概念图接了这个键） |
| 6 | P1 | 新建节点后按 `Esc` 再压一条 `DeleteNodes`，撤销栈多一步 |
| 7 | P1 | 缩放能到 0.5%，与「10%–500%」的合同不符 |
| 8 | P1 | `Ctrl+=` / `Ctrl+-` 不保锚点，画面会整体漂 |
| 9 | P2 | 概念图里 `Tab`/`Enter` 建出带 `parent_id` 的树子节点 |
| 10 | P2 | `Esc` 先清多选、最后才轮到关系线，层级反了 |

---

## 2. 非目标

- **不动外壳**：`app_methods_*.rs` / `main.rs` / `session.rs` / 面板 / `overlay.rs` 一行不改。
  需要壳层配合的只有一处（§7 风险 1：`Ctrl+A` 的快捷键文案），本轮以「行为落地 + 记账」收尾。
- **不动数据模型**：不加 `Node`/`NoteCard`/`LayoutSettings` 字段，不升 `FORMAT_VERSION`，不做迁移。
- **不动外观**：确认框复用既有 `dialog` 组件（令牌与其它弹窗同源），不新增视觉规格，因此**不重拍 golden**。
- 不改上一轮已落地的三块：椭圆编辑态环（`editor_ui` 里 `Shape::Ellipse` 分支）、概念图 `Ctrl+拖拽` 复制、
  `mw_ui::drag` 既有的三分区落点判定。
- 不碰 `Alt+Enter` 聚焦语义、概念图整理布局 (`arrange_batch`)、`F6` 焦点交接。

---

## 3. 逐条：根因 → 修法 → 落点

### 3.1 `Shift+Enter` 软换行（P0）

**根因**：`egui::InputState::consume_key` 的修饰键匹配是「宽松」的——只校验「要求的修饰键按下了」，
不校验「有没有多按别的」。所以 `consume_key(Modifiers::NONE, Key::Enter)` 把 `Shift+Enter` 一起吃掉。
编辑态那段先查 `Esc`、再查**纯** `Enter`，于是软换行落到了「提交 + 新建同级」上。

**修法**：编辑态的按键不再用 `consume_key`，改用「精确修饰键」取键（`take_exact_key`，与大纲
`panels/outline.rs` 同一口径），并把 `Shift+Enter` 排在纯 `Enter` **之前**：在光标处插入 `\n`、
光标停在换行之后、立刻按新文本重排（节点盒当场变高），本帧不再提交。

**落点**：
- `canvas_methods_3.rs` · `take_exact_key(ui, mods, key) -> bool`（从事件队列里按位摘一条）
- `canvas_methods_3.rs` · `soft_newline_insert(text, sel) -> (String, usize)`（纯函数：插在选区处、替换选区）
- `canvas_methods_3.rs` · `Canvas::insert_soft_newline`（读 `TextEditState` 光标位 → 写回 → 重排）
- `canvas_methods_2.rs` · `show` 里的编辑态取键块（`esc` / `shift+enter` / `enter` / `tab` 顺序）

**单测**：`soft_newline_inserts_at_caret_and_keeps_caret_after_the_break`（含选区替换、越界夹取）。

### 3.2 选中即打字（P0）

**根因**：画布只有「双击 / `F2` / 悬停＋」三个进编辑入口，非编辑态的 `Event::Text` 没有任何人接，
所以选中节点后直接敲字是被静默丢弃的。

**修法**：非编辑态、画布握焦点时，先摘一枚 `Event::Text`（无 `Ctrl`/`Alt`/`Cmd`，且不是空格/控制符），
就地对**主选**调用 `start_edit`，并把草稿整体换成这个字符（等价「全选 + 输入」），光标落末尾。
`IME` 预编辑没有可显示文字，按大纲的口径先开一个空编辑框（`TypeStart::Compose`），让下一帧的
`TextEdit` 接管组合串。

**落点**：
- `canvas_methods_3.rs` · `enum TypeStart { Replace(String), Compose }`、`type_char(text)`、
  `take_type_start(events, mods) -> Option<TypeStart>`（纯函数，便于单测）
- `canvas_methods_3.rs` · `Canvas::type_to_edit(ui, viewport) -> bool`
- `canvas_methods_2.rs` · `show` 里在**快捷键之前**调用（`!editing && !input_blocked`）

**单测**：`type_to_edit_takes_one_char_and_leaves_the_rest_of_the_queue`（含：`Ctrl` 按下不吃、
空格不吃、队列里只剩别的文字时不吃、`IME Predit` 走 `Compose`）。

### 3.3 删除确认（P0）

**根因**：`delete_selected` 把整片子树一次性下发 `Command::DeleteNodes`，没有任何确认——
选中一个分支按 `Delete`，几十个节点一起消失。

**修法**：待删集合里**有节点带非空子节点**时，不发命令，先把「待删集合 + 总数」存进画布状态，
当帧用既有 `dialog::show` 立一个确认框：标题「删除 N 个节点？」、默认焦点在**取消**、`Esc` 也等于取消。
叶子节点（没有子节点）删除**不弹**，保持一键速删。确认后才真正下发 `DeleteNodes`（一条撤销）。
`Ctrl+X` 剪切不走确认——剪贴板里已有内容、且是明确的双向动作（记账见 §7）。

**落点**：
- `canvas.rs` · `struct DeleteConfirm { ids, count }` + `Canvas::confirm_delete` 字段
- `canvas_methods_3.rs` · `delete_confirm_plan(doc, ids) -> Option<usize>`（纯函数：要不要确认、删几个）
  、`Canvas::delete_selected`（分流）、`Canvas::delete_now`、`Canvas::confirm_delete_ui`
- `canvas_methods_2.rs` · `show` 末尾画确认框；`shortcuts` / `handle_pointer` 在确认框开着时收手

**单测**：`delete_confirm_counts_the_whole_subtree_and_skips_leaf_only_deletes`（含根节点保护、去重）。

### 3.4 拖进折叠父节点（P1）

**根因**：`mw_ui::drag::move_commands` 只下发 `MoveNode`。父节点是折叠态时，子树挂进去以后
在屏幕上依旧不可见——用户看到的是「拖了一下，什么都没发生」，再拖一次还会重复。

**修法**：`move_commands` 顺带把落点父节点的 `SetCollapsed { collapsed: false }` 拼进**同一条** `Batch`，
一次撤销既回到旧父、也回到折叠状态。去重、按场景顺序，不重复下发。

**落点**：`mw-ui/src/drag.rs` · `move_commands(scene, moves)`（签名不变）

**单测**：`dropping_into_a_collapsed_parent_expands_it_in_the_same_undo_step`
（断言命令里既有 `MoveNode` 也有 `SetCollapsed`；`apply` 后父展开；`undo` 一次全复原）。

### 3.5 思维导图 `Ctrl+A`（P1）

**根因**：`shortcuts` 里 `Ctrl+A` 只接了 `select_all_concepts`，非概念图直接返回 `false`。

**修法**：分流——概念图仍走 `select_all_concepts`（只选顶层概念）；思维导图走新增的
`select_all_visible`：选当前场景里所有**可见**节点（折叠中的本来就不在 `Scene` 里，天然排除），
主选保持原主选、清掉关系线与注解选择。

**落点**：`canvas_methods_3.rs` · `Canvas::select_all_visible`；`shortcuts` 的 `cmd+A` 分支

**单测**：`select_all_visible_skips_what_folding_hid`（用真实 `build_scene`：折叠分支的后代不在场景里，
所以选择集就是屏幕上的那些节点）。

### 3.6 新建节点后 `Esc` 的撤销栈（P1）

**根因**：`cancel_edit` 对 `fresh` 节点再压一条 `DeleteNodes`。`Tab` 建节点 → 写两个字 → `Esc`，
撤销栈里会多一条「删除节点」，按 `Ctrl+Z` 先撤删除、再撤新建，要按两下。

**修法**：进编辑时记下当前撤销栈深；取消时若栈深**没变**（栈顶仍是「建这枚节点」那条），
用一次 `doc.undo()` 把它整个回滚——一条撤销回到建之前。栈深变了（用户中途做过别的动作）
就退回原来的 `DeleteNodes` 口径，不猜。

**落点**：`canvas.rs` · `Editor::fresh_depth` 字段；`canvas_methods_3.rs` · `add_relative` 记录栈深、
`cancel_edit` 分流

**单测**：`cancelling_a_fresh_node_edit_leaves_no_extra_undo_step`（建 → 取消 → 栈深与建之前相同）。

### 3.7 缩放区间（P1）

**根因**：`MIN_ZOOM = 0.005` / `MAX_ZOOM = 4.0`，与合同「10%–500%」不符。

**修法**：`MIN_ZOOM = 0.1`、`MAX_ZOOM = 5.0`。`FIT_MIN_ZOOM` 保持 `0.005`——20k 文档的「适应视图」
本来就要缩到 2% 上下，那是**取景**不是**交互**，两件事不能合成一个常量。

**落点**：`canvas/consts.rs` · `MIN_ZOOM` / `MAX_ZOOM`（doc 注释写清两者分工）

**单测**：`zoom_range_is_ten_percent_to_five_hundred`（同时钉住 `FIT_MIN_ZOOM < MIN_ZOOM`）。

### 3.8 `Ctrl+=` / `Ctrl+-` 的锚点（P1）

**根因**：那两个键直接 `camera.zoom = zoom * factor`，改的是缩放值、不是「以某点为中心缩放」，
于是画面绕世界原点漂移；而滚轮缩放走的是 `zoom_at_anchor`，同一份相机两条口径。

**修法**：统一走 `zoom_at_anchor`，锚点取指针所在世界点（指针不在画布内时取视口中心）。

**落点**：`canvas_methods_2.rs` · `show` 里的缩放键块

另外给 `zoom_at_anchor` 补一条「下界不越过当前值」的夹取：`fit` 可以把相机放到 0.5%（20k 文档），
此时按一下缩小键不该**跳大**到 10%——下界取 `MIN_ZOOM.min(当前缩放)`，交互区间仍是 10%–500%。

**单测**：`zoom_anchor_keeps_the_world_point_under_the_cursor`（已有，扩一条：从 `FIT_MIN_ZOOM` 起缩不跳变）。

### 3.9 概念图 `Tab`/`Enter`（P2）

**根因**：`shortcuts` 无条件调 `add_relative`（`Command::AddNode` + `parent_id`），概念图因此长出树结构，
把命题网变成了一棵隐藏的树。

**修法**：`concept_map()` 为真时改走 `link_concept_below` / `Command::AddLinkedConcept` 那条路径
（与大纲 `Tab` 的概念图分支同一语义：一个 Batch 里「加概念 + 加边」），随后选中新概念并立刻立起
**链接短语**编辑态；短语留空则连边一起撤掉、概念留着（`docs/30` §5.9）。全程不产生 `parent_id`。

**落点**：`canvas_methods_3.rs` · `link_concept_command(from, kind, pos)`（纯函数：位置与命令）、
`Canvas::add_concept_relative`；`shortcuts` 的 `Tab` / `Enter` 两支

**单测**：`concept_relative_never_produces_a_tree_child`（应用命令后：节点数 +1、边数 +1、
所有概念 `parent_id` 均为 `None`；`Tab` 落点在下方、`Enter` 在右侧）。

### 3.10 `Esc` 的层级（P2）

**根因**：`Esc` 分支只看 `focus_pop()`，关系线选择根本没进这条链——选中一条线按 `Esc`，
被清掉的是节点多选，线还亮着。

**修法**：抽 `esc_plan(has_relationship, focus_depth, has_sel) -> CanvasEsc` 纯函数，按
「先清关系线 → 再退聚焦栈 → 最后清多选」逐层退，与 `inspector::esc_step` / `outline::esc_step`
的层级口径一致。

**落点**：`canvas_methods_3.rs` · `enum CanvasEsc`、`esc_plan(...)`、`shortcuts` 的 `Esc` 分支

**单测**：`escape_walks_relationship_then_focus_then_selection`。

---

## 4. 验收场景（人工可复现）

1. **软换行**：`F2` 改一个节点 → 打「第一行」→ `Shift+Enter` → 打「第二行」→ `Enter`。
   一个节点两行；撤销栈只多一条（文字那条）。
2. **打字进编辑**：鼠标点中一个节点（不双击）→ 直接打「新」字。编辑器立刻打开，内容只剩「新」，
   光标在末尾，`Enter` 提交后节点文字变成「新」。
3. **删除确认**：选中一个带子节点的分支 → `Delete` → 出现「删除 N 个节点？」，`N` 含全部后代，
   默认焦点在「取消」；`Esc` 取消后树与选择都不变；`Enter` 走默认（取消）；点「删除」才真删，
   一次 `Ctrl+Z` 全回来。选一个叶子节点按 `Delete`：**不弹框**，直接删。
4. **拖进折叠父节点**：把 `A` 折叠 → 把 `B` 拖到 `A` 上（中间区）松手 → `A` 展开、`B` 出现在 `A` 下；
   `Ctrl+Z` 一次同时回到「`B` 在旧父下 + `A` 折叠」。
5. **`Ctrl+A`**：思维导图里选中任意节点按 `Ctrl+A` → 当前可见节点全选（折叠里的不选）；
   选项圈在屏幕上能数清。
6. **`Esc` 撤销节奏**：`Tab` 建子节点（已进编辑）→ 打两个字 → `Esc` → 节点消失，按一次 `Ctrl+Z`
   回到建之前（不再多出一条「删除节点」）。
7. **缩放区间**：连按 `Ctrl+-` 到底，状态不再低于 10%；连按 `Ctrl+=` 到顶不超过 500%；
   打开一份 20k 文档按 `Ctrl+1` 适应视图仍能看到全图（`FIT_MIN_ZOOM` 没被牺牲）。
8. **缩放锚点**：把指针停在某个节点上按 `Ctrl+=`，该节点在屏幕上不动；指针移出画布后按
   `Ctrl+=`，画面绕视口中心缩放。
9. **概念图 `Tab`/`Enter`**：概念图里选中一个概念按 `Tab` → 下方出现新概念并连一条线，
   链接短语编辑框立起来；保存后 `.zmind` 里新概念的 `parent_id` 为空；短语留空回删则线消失、
   概念留着。
10. **`Esc` 层级**：画一条关系线并选中它 → 按 `Esc`：线先不再高亮；再按一次才清多选；
    编辑态按 `Esc` 仍是「取消这次编辑」（优先级不变，由编辑态分支先接走）。

---

## 5. 证据

### 5.1 命令与输出

| 命令 | 结果 |
| --- | --- |
| `cargo build -p mw-shell` | 绿（0 error / 0 warning） |
| `TEMP/TMP=target/tmp cargo test -p mw-shell --bins canvas` | `121 passed; 0 failed`（本轮新增 10 条，全绿） |
| `TEMP/TMP=target/tmp cargo test -p mw-shell --bins` | `588 passed; 0 failed; 1 ignored`（全量，无回归） |
| `TEMP/TMP=target/tmp cargo test -p mw-ui` | `42 passed; 0 failed`（本轮新增 1 条） |
| `cargo clippy -p mw-shell -p mw-ui --all-targets` | 本轮触及的文件零新增告警（既有告警都在 `mw-core`，与基线一致） |
| `cargo fmt --all -- --check` | 本轮文件已 rustfmt；仅剩 `canvas.rs` 椭圆环那一段（上一轮的未提交改动，不在本轮范围） |

注意：`crates/mw-shell` 是**纯 bin crate、没有 lib target**，`--lib` 会直接
`error: no library targets found`；画布单测的入口是 `--bins`，且用 `canvas` 过滤。

### 5.2 新增单测（名称 → 钉住什么）

| 测试 | 钉住什么 |
| --- | --- |
| `exact_key_taking_leaves_shift_combos_for_their_own_handler`（canvas） | `take_exact_key_from` 只摘「修饰键完全一致」的那一条；纯 `Enter` 的匹配不命中 `Shift+Enter`——这就是缺陷 #1 的机制本身 |
| `soft_newline_inserts_at_caret_and_keeps_caret_after_the_break`（canvas） | 软换行插在光标处、光标停在换行后；有选区整段替换；越界夹到末尾且按 `char` 换算 |
| `type_to_edit_takes_one_char_and_leaves_the_rest_of_the_queue`（canvas） | 落字只摘一条，剩下的留在队列里给 `TextEdit`（吞多了丢字，吞少了字掉进虚空） |
| `type_to_edit_ignores_space_and_control_chars`（canvas） | 空格/换行/制表/控制字符不触发；带 `Ctrl` 的 `Text` 不触发 |
| `delete_confirm_counts_the_whole_subtree_and_skips_leaf_only_deletes`（canvas） | 叶子不弹框、带子节点弹框、计数含整片子树、父子同选不重复计数 |
| `select_all_visible_skips_what_folding_hid`（canvas） | 折叠藏起来的节点不进全选；主选排在最后且不重复 |
| `cancelling_a_fresh_node_edit_leaves_no_extra_undo_step`（canvas） | 栈顶仍是「建它」时一次 `undo` 收回、栈深回到建之前；深度对不上则退回「删掉空盒子」 |
| `zoom_range_is_ten_percent_to_five_hundred`（canvas） | 上下限夹住；**低于下限的取景继续缩小不会跳上来**（20k 文档 0.5% 取景不被抬到 10%） |
| `concept_relative_never_produces_a_tree_child`（canvas） | 概念图 `Tab`/`Enter` 落点是下/右；新概念 `parent_id` 为空且同时长出一条命题线 |
| `escape_walks_relationship_then_focus_then_selection`（canvas） | `Esc` 四级顺序：关系线 → 聚焦栈 → 多选 → 什么都不做 |
| `dropping_into_a_collapsed_parent_expands_it_in_the_same_undo_step`（mw-ui） | 拖进折叠父节点时 `SetCollapsed{false}` 与移动合成一条命令，一次撤销全部复原 |

### 5.3 十项「合同 → 落地 → 证据」

| # | 缺陷 | 落点（函数） | 证据 |
| --- | --- | --- | --- |
| 1 | `Shift+Enter` 变提交 | `take_exact_key` / `soft_newline_insert` / `insert_soft_newline`（`canvas_methods_3.rs`），接线在 `canvas_methods_2.rs` 编辑键块 | `exact_key_taking_leaves_shift_combos_for_their_own_handler`、`soft_newline_inserts_at_caret_and_keeps_caret_after_the_break` |
| 2 | 选中即打字无反应 | `type_char` / `take_type_start` / `type_to_edit`，接线在 `show()` 的 `shortcuts` 之前 | `type_to_edit_takes_one_char_and_leaves_the_rest_of_the_queue`、`type_to_edit_ignores_space_and_control_chars` |
| 3 | 带子树 `Delete` 无确认 | `delete_confirm_plan` / `delete_selected` / `delete_ids` / `delete_now` / `confirm_delete_ui`（+ `Canvas.confirm_delete`） | `delete_confirm_counts_the_whole_subtree_and_skips_leaf_only_deletes` |
| 4 | 拖进折叠父节点无变化 | `mw_ui::drag::move_commands` 追加 `SetCollapsed{false}` 并入 `Batch` | `dropping_into_a_collapsed_parent_expands_it_in_the_same_undo_step` |
| 5 | 思维导图 `Ctrl+A` 无反应 | `select_all_indices` / `select_all_visible`，`shortcuts` 里按 `concept_map()` 分流 | `select_all_visible_skips_what_folding_hid` |
| 6 | 新建后 `Esc` 多一条撤销 | `Editor.fresh_depth`（`add_relative` 记栈深）+ `cancel_fresh_node`（`cancel_edit` 调用） | `cancelling_a_fresh_node_edit_leaves_no_extra_undo_step` |
| 7 | 缩放到 0.5% | `consts.rs` 的 `MIN_ZOOM = 0.1` / `MAX_ZOOM = 5.0`，`zoom_at_anchor` 用 `MIN_ZOOM.min(camera.zoom)` 当下限锚 | `zoom_range_is_ten_percent_to_five_hundred` |
| 8 | `Ctrl±` 不保锚点 | `zoom_by`（工具条/菜单）与 `show()` 的 `Ctrl±` 两处都改走 `zoom_at_anchor`（指针优先，否则视口中心） | `zoom_anchor_keeps_the_world_point_under_the_cursor`（既有）+ 同上 zoom 上下限测试 |
| 9 | 概念图 `Tab`/`Enter` 长出 `parent_id` | `concept_relative_command` / `add_concept_relative`，`shortcuts` 里 `concept_map()` 分支 | `concept_relative_never_produces_a_tree_child` |
| 10 | `Esc` 层级反了 | `CanvasEsc` / `esc_plan`，重写 `shortcuts` 的 `Esc` 分支 | `escape_walks_relationship_then_focus_then_selection` |

## 6. 复跑

```powershell
cargo build -p mw-shell
$env:TEMP="$(Resolve-Path target/tmp)"; $env:TMP=$env:TEMP
cargo test -p mw-shell --bins canvas   # 画布单测（纯 bin crate，没有 --lib）
cargo test -p mw-shell --bins          # 全量，确认没有回归
cargo test -p mw-ui
```

## 7. 未做 / 风险

1. **`Ctrl+A` 的文案**：快捷键帮助面板在 `shortcuts.rs`（本轮写权之外），「全选可见节点」这条
   说明需要壳层补一行文案；本轮只落行为，已在报告记账。
2. **剪切不确认**：`Ctrl+X` 删带子节点的分支仍是一键。理由：剪贴板里已有内容、撤销可用，
   且剪切是「搬走」不是「销毁」。若要统一，属于剪贴板语义变更，另开一轮。
3. **IME 组合中途进编辑**（`TypeStart::Compose`）：与大纲同一口径——先开空编辑框交给 `TextEdit`；
   本轮额外把草稿的 `original` 也置空，避免「组合没落地就提交」把节点文字清空。
4. **确认框的 `Esc`**：画布的确认框不在壳层 `overlay::State` 里，走不到 `dialog::route_escape`，
   所以由画布自己 `consume` 这一次 Esc 并当作取消；壳层若将来把画布弹窗也纳入 overlay，
   这段可以退化成 `dialog::show` 自带的 `closed`。
5. **概念图 `Enter`**：命题网没有「同级」，`Enter` 与 `Tab` 同义（都是「加一条命题」），
   区别只在落点（右 / 下）。这是刻意的：不制造「树的兄弟」这种概念图里不存在的结构。
