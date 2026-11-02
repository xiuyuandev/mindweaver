# G-CM-1 · 概念图核心人工门（M 轨 M1 出口 · CM-01–05）

> 依据：`docs/31` §1.3 的 G-CM-1 判词「概念图是命题网：边有方向有短语、节点可自由摆放、
> 整理布局不重叠」；设计合同 `../../docs/30-外壳体验与概念图设计规范.md` §5.1–§5.10；
> 执行合同 `../../docs/31-外壳体验与概念图工作包.md` §6 的 CM-01–05 交付项。
> **本清单只列判词与证据，不写「基本完成」**：四态只有 `通过` / `部分` / `缺证据` / `未判`。
> 授权：用户 2026-11-02 原话（记录在 `../../docs/STATUS.md` 抬头）
> 「继续开发，而且呢，你帮助我验收，不需要我决定。」——据此代判，但每条都要给依据与复跑方式。
> 审计快照：2026-11-02 17:32（本机工作区当前未提交状态；行号是快照，落点行文字一并写出）。
> 包报告：[`report.md`](report.md)；逐条机器证据见 §3。
>
> **门通过 ≠ 记 `已验证`**：`已验证` 另欠实机微软拼音与系统 DPI 两项（同 G-B2-1/G-B2-2/G-B2-3 口径）。

---

## 0. 结论

| 项 | 结果 |
| --- | --- |
| 判词（`docs/31` §1.3 拆四句） | **通过 4 / 共 4**（2026-11-02 授权代判） |
| 逐条证据表（§3，23 条） | 通过 18 / 部分 3 / 缺证据 1 / **未判 1**（合同冲突，见 §7） |
| 机器门 | `stage-cm` **73 场 / 146 次比较两遍零差异**（2026-11-02 12:37:21→12:41:19 正式脚本）——**但只对 12:41 的代码成立**，见 §0.2 |
| 不阻塞但必须留在纸上的欠账 | ① 悬停态无基线 ② 重叠红边无单测无基线 ③ 拖拽落 `pos` 的手势层无脚本 ④ 短语断口是等效实现（非几何断开） ⑤ `docs/30` §5.9 与 `docs/33` §2 B1a 就「空短语」互相打架 |
| 状态建议 | CM-01–05 维持 `已实现`；是否升 `已验证` 由有 `docs/STATUS.md` 写权限的一棒决定（见 §0.1） |

### 0.1 为什么本门通过却不建议直接写 `已验证`

`docs/30` §7 第 5 条的「已验证」只在人工门通过后使用，但 `AGENTS.md` §3 第 3 条把
**中文四痛点**（IME 不打架 / CJK 禁则 / 字体不糊 / 拼音检索）列为验收项。
CM-03 的 `cm-label-ime` 只做到**离屏等效**（注入 preedit，不提交），
实机微软拼音选字那一下从未人工核对（`../CM-03/report.md` §5 第 2 条）。
所以本门只判「概念图核心做得对不对」，**判不了 IME 实机**——这一笔与 G-B2-1/G-B2-2/G-B2-3 完全同口径。

### 0.2 审计边界：工作区在门后继续前进（这一条必须先看）

本门判的是 **CM-01–05 的交付形态**，不是「今天这棵工作树绿不绿」。两条边界必须写清楚：

1. **机器门是 12:41 的快照，工作树比它新**：`stage-cm` 最后一次真跑是 2026-11-02 12:37–12:41；
   而 `crates/` 下有 **40 个 `.rs` 在那之后被改过**（`find crates -name '*.rs' -newermt "2026-11-02 12:42"`
   计数），其中包含概念图核心文件：`mw-core/src/scene.rs`（17:26）、`theme.rs`（17:31）、
   `layout.rs`/`model.rs`（16:26–16:28）、`mw-render/src/instances.rs`（17:32）、
   `mw-shell/src/canvas/canvas_methods_3.rs`（17:04）、`app_methods_2.rs`（17:15）、
   `mw-format/src/lib.rs`（17:26）。**因此「73 场 / 146 次比较零差异」只能证明 12:41 那一刻**；
   要在今天这棵树上复现，得先按 `docs/29` §5 重拍受影响的基线再跑门——本棒不跑 golden。
2. **合同也在门后前进**：`docs/33-概念图v2设计规范.md`（2026-11-02 11:53）已经**改写了
   概念图的一条硬规则**，与 `docs/30` §5.9 直接冲突（见 §7）。另有 `docs/34`（17:28）把
   `.cxl`/`.cmap` 互导落成了代码（`crates/mw-io/src/cmap.rs`，本轮在 `mw-io` 的 lib 测试里
   看得到 `cxl_roundtrips_fidelity_lossless` 等三条）。这两件事都**不在 G-CM-1 判词范围**，
   本文件只负责在冲突处留账，不替它们判。
3. 本轮所有 `cargo test -- --exact` 复跑发生在 **17:18–17:35**，期间工作树仍在被并行会话修改；
   结果对应的是「执行那条命令那一刻的代码」，不是某个固定提交。

---

## 1. 判词

| # | 判词（`docs/31` §1.3 原文拆句） | 看什么 | 结论 | 一句依据 | 复跑方式 |
| --- | --- | --- | --- | --- | --- |
| 1 | **概念图是命题网**（不是思维导图的辐射树） | 椭圆/浅色概念、直线有向边、菜单无树语义 | ☑ 通过（授权代判） | `cm-default`/`cm-narrow` 基线锁住「不是辐射树」；概念图节点菜单已整块换成命题网口径（无「新建子主题/概要/边界/折叠/聚焦」） | `powershell -File scripts/x100-stage-cm-gate.ps1`（门内含 63 场 `cm-*`）；`cargo test -p mw-shell --bins -- --exact canvas::tests::concept_relative_never_produces_a_tree_child` |
| 2 | **边有方向有短语** | 四档箭头、短语必填（空白落「未命名」占位）、短语进往返 | ☑ 通过（授权代判） | `cm-arrow-off/backward/both` 三张锁四档箭头；`cm-phrase-clear` 锁「既有短语清空=拒绝」；短语必填由模型层 `Invalid("概念图的连接短语必填")` 兜底；往返断言逐字段比 `label`/`color`/`head` | 门同 1；`cargo test -p mw-shell --bins -- --exact canvas::tests::blank_phrase_commit_follows_the_concept_map_contract` |
| 3 | **节点可自由摆放** | `pos` 落盘、拖拽/方向键落 `pos`、主概念也能挪 | ☑ 通过（授权代判，含证据边界） | 命令层 `MoveConcept` 允许浮动概念**与根**、思维导图根仍拒；`cm-crosslink` 的 5 条 `concept-nudge` 轨迹证明键盘路径真的走到命令层 | `cargo test -p mw-core --lib -- --exact document::tests::move_concept_only_works_in_concept_maps document::tests::nudging_a_group_of_concepts_is_one_undo_step`；`cargo test -p mw-shell --bins -- --exact canvas::tests::concept_nudge_steps_are_one_and_eight`；轨迹 `cm-crosslink.trace.txt` |
| 4 | **整理布局不重叠** | 分层/等距/不重叠、层距 ≥56 层内 ≥24、一次撤销 | ☑ 通过（授权代判） | 三条 `concept_layout` 单测钉住「层内最小间隙、环安全、确定性」；`cm-arrange` 与 `cm-default` 构成前后对比；`cm-arrange-undo` 锁「撤销连取景一起还回来」 | `cargo test -p mw-core --lib -- --exact concept_layout::tests::arrange_keeps_min_gap_inside_a_layer concept_layout::tests::arrange_survives_cycles concept_layout::tests::arrange_is_deterministic_and_keeps_single_node_put`；`cargo test -p mw-shell --bins -- --exact canvas::tests::arranging_three_concepts_is_one_undo` |

## 2. 判词记录（2026-11-02 · 授权代判）

1. **概念图是命题网** — 节点走 `theme.rs` 的 `apply_concept_defaults()`（级联之后统一覆盖、
   仍尊重显式样式），概念图默认形状是椭圆（`theme::tests::concept_maps_default_to_ellipses_and_mindmaps_do_not`
   断言「没覆盖的概念都是椭圆、思维导图不能跟着变椭圆」）；边是直线段
   （`scene.rs` 的 `straight_between`，注释写着「凭什么不是曲线」）；右键菜单在
   `app_methods_2.rs` 里按 `canvas.is_concept_map()` 整块分叉，节点菜单第一行是
   「新建概念并连接」，全套树语义条目消失。基线 `cm-default`/`cm-narrow`/`cm-link-menu`/
   `cm-blank-menu` 都在门内且 12:30–12:32 一次拍成、12:37–12:41 两遍复核零差异。
2. **边有方向有短语** — 方向是 `Relationship.head` 四档（`Off/Forward/Backward/Both`，默认
   `Forward` 即原行为），`cm-arrow-off`/`-backward`/`-both` 三张锁住；
   短语是硬约束：概念图里 `SetRelationshipLabel{label:None}` 返回
   `Invalid("概念图的连接短语必填")`，既有命题被清空时壳层保留原短语并提示
   （`cm-phrase-clear` 锁 `rel-label-reject reason=empty`）。
   **新建线留空不再是「整条撤掉」**：`phrase_commit` 现在返回 `Set(Some("未命名"))`
   （`canvas_methods_3.rs:41`，`CONCEPT_UNNAMED_LINK`），只有显式 `Esc` 才撤线——
   这是 `docs/33` §2 B1a 改的口径，`docs/30` §5.9 的「未填不给连线落盘」已被它取代，
   两处合同没对齐，本门按「未判」记账（§7）。
   `mw-format` 的往返断言逐字段比 `label = Some("解决")` / `color = None`（不偷偷补默认色）。
3. **节点可自由摆放** — `Command::MoveConcept` 只在 `kind == "conceptMap"` 成立，
   允许 `parent_id = None` 的任意节点**含根**，思维导图的根仍不可摆
   （`document::tests::move_concept_only_works_in_concept_maps` 双向断言）。
   浮概念拖到别的概念上**不改结构**（`commit_concept_move` 只发 `MoveConcept`，
   不计算 drop 目标）。主概念的 `pos` 由 `build_scene_cached` 整体平移实现，
   `scene_keeps_each_concept_at_its_pos_after_the_root_swap` 钉住换根后画面不移位。
   **证据边界**：鼠标拖拽这一层没有脚本场景、也没有单测（`commit_concept_move` 无
   `#[test]` 覆盖），只有代码 + CM-04 §7.1 的人工试用记录——这条记在 §4 第 3 项。
4. **整理布局不重叠** — `mw_core::concept_layout` 是纯函数：最长路径分层（DFS 丢回边）、
   同层父重心排序、层内等距、24 轮松弛、整体左对齐到原包围盒左上角；
   `LAYER_GAP = 56` / `NODE_GAP = 24`。单测 `arrange_keeps_min_gap_inside_a_layer`
   直接量层内间隙，`arrange_survives_cycles` 造 A→B→C→A 仍收敛，
   `arrange_is_deterministic_and_keeps_single_node_put` 连算两次逐值相同。
   落地是一条 `Batch` 一次撤销（`arranging_three_concepts_is_one_undo` 比撤销深度），
   C7 那一轮还补了「撤销踩到整理那条时重新 `fit`」。

---

## 3. 逐条证据表（`docs/30` §5 → 代码落点 → 证据）

口径：**通过** = 有门内基线，或有 `cargo test -- --exact` 找得到的单测；
**部分** = 主体做到、有一条子项只有代码或等效实现；**缺证据** = 只有代码，没有基线也没有会红的断言。

### 3.1 CM-01 数据契约

| # | 合同条款 | 代码落点（行号是快照） | 证据 | 判定 |
| --- | --- | --- | --- | --- |
| 1 | `LayoutSettings.kind = "conceptMap"`，不新增字段、不升 `FORMAT_VERSION` | `crates/mw-core/src/layout.rs:44` `ConceptMap,`；`:54` `"conceptMap" => Self::ConceptMap,`；`:72` `self == Self::ConceptMap` | `mw-format` `tests::concept_map_roundtrips_positions_and_relationships`（**已复跑：1 passed**）；门内 `concept map .zmind roundtrip` 步 | 通过 |
| 2 | 概念 = 浮动 `Node`（`parent_id=None` + `pos`）；**主概念也带 `pos`**（CM-04 例外） | `crates/mw-core/src/document.rs:703` `fn surviving_concept(&self, removing: &HashSet<String>) -> Option<String>`；`:1078` `self.surviving_concept(&to_delete)` | `scene::tests::scene_keeps_each_concept_at_its_pos_after_the_root_swap`（**已复跑**）+ 门内 `cm-default` | 通过 |
| 3 | 关系 = `Node.relationships`（有向 + `label` + `color`），**不用 `children` 表达层级** | `crates/mw-core/src/command.rs` 的 `AddRelationship` / `SetRelationshipLabel`；大纲与导出都走 `concept_tree::concept_rows` | `mw-format` 往返断言（`label`/`color` 逐字段）+ `canvas::tests::concept_relative_never_produces_a_tree_child`（**已复跑**：Tab/Enter 只产生 `AddLinkedConcept`，新概念无 `parent_id`） | 通过 |

### 3.2 CM-02 渲染

| # | 合同条款 | 代码落点 | 证据 | 判定 |
| --- | --- | --- | --- | --- |
| 4 | 节点形状（现行合同：**椭圆**；圆角矩形仍是可选形状） | `crates/mw-core/src/theme.rs:1392` `style.shape = Shape::Ellipse;`（在 `apply_concept_defaults` 内，显式覆盖不盖） | `theme::tests::concept_maps_default_to_ellipses_and_mindmaps_do_not`（**已复跑**）；门内 `cm-default` | 通过 |
| 5 | 盒宽 = 文本 + 28 夹 **96–240**；高 36（双行 52）；最多 2 行超出省略 | `crates/mw-core/src/scene.rs:1105` `fn measure_concept(...)`；`:1120` `(m.width + 2.0 * CONCEPT_PAD_X).clamp(CONCEPT_MIN_W, CONCEPT_MAX_W)`；`:1122` `CONCEPT_H2`；常量 `:51/:53/:57` | `scene::tests::concept_boxes_follow_the_node_contract`（**已复跑**）；门内 `cm-default`/`cm-narrow` | 通过 |
| 6 | 文本 14/20 居中；主概念 15/22 600 + 8% 强调底；层级不靠字号递减 | `scene.rs:1122` 起的 `BoxSpec` 分支 + `center_text`；`theme.rs` 的 `apply_concept_defaults` 只补字号/加粗 | 同上单测（字号 14 / 主概念 15、居中、≤2 行）+ 门内 `cm-default` 目测 | 通过 |
| 7 | 悬停 1px 环 + **4% 底**；选中 1.5px 环 + **8% 底** | `crates/mw-render/src/instances.rs:34` `pub const SELECT_FILL: f32 = 0.08;`；`:35` `pub const HOVER_FILL: f32 = 0.04;` | 选中态由门内 `cm-crosslink`（`MW_CONCEPT_SELECT=用户痛点`）锁住；**悬停态没有任何 `cm-*` 场景悬停概念节点**（只有 `cm-focus-question-hover` 悬停问题带） | 部分 |
| 8 | 相机 `fit` 用所有节点包围盒（不参与树的 `fit`） | 概念图分支的 `request_fit` + 主概念整体平移；打磨轮 19 改「按帧重 `fit` 12 帧」 | 门内 `cm-default`/`cm-narrow`（取景是画面的一部分）；**无单测** | 通过 |
| 9 | 连线**直线段** + 6×4 实心箭头（屏幕恒定 ≥7 css px 在渲染层做） | `scene.rs:699` `fn straight_between(p1: Point, p2: Point)`；`:707` `p2.x - ux * 6.0`；`:709–713` 三角 6×4；`instances.rs` 的 `screen_arrow` | `scene::tests::concept_relationships_are_straight_while_mindmap_keeps_curves`（**已复跑**：概念图 `PathSeg::Line`、思维导图 `PathSeg::Quad`）；门内 `cm-default` | 通过 |

### 3.3 CM-03 连线与链接短语

| # | 合同条款 | 代码落点 | 证据 | 判定 |
| --- | --- | --- | --- | --- |
| 10 | 线宽 **1.5**，屏幕空间恒定 | `crates/mw-render/src/instances.rs:56` `pub const REL_W: f32 = 1.5;`；绘制走 `cam.css_to_world(REL_W)` | 常量 + 门内 `cm-rel-style`（「带来」那条是 2.5 粗虚线，与默认 1.5 并存不打架） | 通过 |
| 11 | 短语胶囊 20px、`surface.page` 底、无描边、11.5/16；**线在标签处断开 8px** | 胶囊绘制在 `instances.rs`；断口是「标签底色盖住线段」的**等效实现**（`label_w = 文本 + 12`），不是几何切段 | 门内 `cm-default`（胶囊形态）+ `cm-label-ime`；断口无几何断言 | 部分 |
| 12 | 空短语只出现在「新建还没填」的编辑态：12px「＋ 链接词」占位 | `canvas_methods_3.rs` 的 `commit_link` → `start_rel_edit(blank_new = true)`；占位文案见 `rel_editor_ui` | 门内 `cm-link-create`（新线已落盘、短语框为空）+ `cm-link-two` | 通过 |
| 12b | §5.9 禁止项「新建连线未填**不给落盘**（没有无短语的线团）」 | 同上；`phrase_commit(blank_new=true, 空)` → `Set(Some("未命名"))`（`canvas_methods_3.rs:41`、`:8` `pub(crate) const CONCEPT_UNNAMED_LINK: &str = "未命名";`） | **无门内场景锁新口径**：`docs/33` §2 B1a 要求的金样 `cm-link-create-nophrase` 在 `scripts/` 与 `shots/golden/inputs/` 里都搜不到；门内 `cm-phrase-blank` 的注释写「整条线撤掉」，而 `canvas_methods_3.rs` 的 mtime 是 **17:04**（门后），所以这张 12:32 的图拍的到底是哪一版口径**没有核实**，不能拿它当任一边的证据 | **未判**（`docs/30` §5.9 与 `docs/33` §2 B1a 相反，先定合同） |
| 13 | 双击就地编辑：本地草稿 / `Esc` 还原 / `Enter` 提交 / **IME 组合不提交** | `canvas_methods_3.rs` 的 `rel_editor_ui`（`RelEdit.ime` 时 `enter = false`） | 门内 `cm-label-ime`（注入 preedit、原短语「带来」未被写坏）+ `cm-phrase-clear` + `canvas::tests::blank_phrase_commit_follows_the_concept_map_contract`（**已复跑**） | 通过 |
| 14 | 交叉处轻微弧度（曲率 ≤8px）；允许有向环且布局/遍历能处理 | `scene.rs:775` `fn bow_crossings(...)`；`:848` `fn bowed_between(...)`；`:814` `fn segments_cross(...)`；`concept_layout` DFS 丢回边 | `scene::tests::only_true_crossings_count` / `bow_offset_is_capped_and_deterministic` / `crossing_bow_leaves_a_lane_curve_alone`（**三条均已复跑**）；门内 `cm-crosslink` | 通过 |

### 3.4 CM-04 布局

| # | 合同条款 | 代码落点 | 证据 | 判定 |
| --- | --- | --- | --- | --- |
| 15 | 拖动即定位写 `pos`；拖到别的概念上**不改结构**；主概念同样可拖 | `crates/mw-shell/src/canvas/canvas_methods_3.rs:512` `pub(crate) fn commit_concept_move(&mut self, m: &Moving) -> bool`；`:2443` `return self.commit_concept_move(m);` | 命令层：`document::tests::move_concept_only_works_in_concept_maps`（**已复跑**）。手势层：`commit_concept_move` **无单测、无脚本场景**（CM-04 §7.1 挂账） | 部分 |
| 16 | 重叠时给 1px 强调红边提示（0.25s） | `canvas_methods_3.rs:3666` `pub(crate) fn paint_overlap_flash(...)`；`:590` `self.overlap_flash = (!overlaps.is_empty()).then(...)` | **无单测、无基线**（0.25s 瞬时态与帧率耦合）；只有代码与人工试用 | 缺证据 |
| 17 | 整理布局：分层 + 父重心 + 等距 + 24 轮松弛 + 左对齐，**一次命令可撤销**；层距 ≥56、层内 ≥24、不重叠 | `crates/mw-core/src/concept_layout.rs:23` `pub const LAYER_GAP: f32 = 56.0;`；`:25` `NODE_GAP = 24.0`；`:27` `RELAX_ROUNDS: usize = 24`；`:126` `pub fn arrange` | `concept_layout::tests::arrange_keeps_min_gap_inside_a_layer` / `arrange_survives_cycles` / `arrange_is_deterministic_and_keeps_single_node_put`（**三条均已复跑**）+ `canvas::tests::arranging_three_concepts_is_one_undo`（**已复跑**）+ 门内 `cm-arrange` / `cm-arrange-undo` | 通过 |

### 3.5 CM-05 交互

| # | 合同条款 | 代码落点 | 证据 | 判定 |
| --- | --- | --- | --- | --- |
| 18 | 方向键 = 摆位 **1px**、`Shift` = **8px**、多选整组、**一条 `Batch` 一次撤销** | `canvas_methods_3.rs:73` `pub fn nudge_concepts(&mut self, dx: f32, dy: f32) -> bool`；`crates/mw-shell/src/canvas/consts.rs:214` `pub(crate) fn nudge_step(shift: bool) -> f32` | `canvas::tests::concept_nudge_steps_are_one_and_eight`（**已复跑**）+ `document::tests::nudging_a_group_of_concepts_is_one_undo_step`（**已复跑**）+ `cm-crosslink.trace.txt` 5 条 `concept-nudge n=1 dx=8.0 dy=0.0`（24/16px 落地） | 通过 |
| 19 | 主概念可删；根重挂到剩下里最靠左上（`(y,x,id)`）；`ChangeSet` 带 `before_root/after_root`；**最后一颗不可删** | `crates/mw-core/src/document.rs:73–74` `pub before_root: Option<String>` / `after_root`；`:703` `surviving_concept`；`:1078` 调用点 | `document::tests::deleting_the_first_concept_rehangs_the_root_and_undo_restores_it`、`deleting_the_last_concept_is_rejected`（**均已复跑**；报告里还有「拿掉 root 快照就断红」的反例记录） | 通过 |
| 20 | 右键菜单不许带树语义（无新建子主题/同级/概要/边界/分离/折叠/聚焦/全部展开折叠） | `crates/mw-shell/src/app_methods_2.rs:715` `ContextTarget::Node if canvas.is_concept_map() =>`；`:735` `Item::row("新建独立概念", "", A::AddFloating, true)`；`:737` `Item::row("连接到…", "Ctrl+L", A::BeginRelationship, true)` | 门内 `cm-link-menu`（节点菜单）、`cm-blank-menu`（空白菜单「没有全部展开/全部折叠」）、`cm-multi-menu`（多选菜单 9 项）；**无单测** | 通过 |
| 21 | 新建概念占位「新概念」（不是「自由主题」）；概念图 `Ctrl+A` 全选；删除按钮可用性 | `canvas_methods_3.rs:126` `pub fn select_all_concepts(&mut self) -> bool`；`:3092` `pub fn can_delete(&self) -> bool`；`canvas_methods_1.rs` 的 `add_floating_world` 占位分支 | 代码核对 + 门内 `cm-blank-menu`（新建概念入口）；**无单测** | 通过 |
| 22 | **所有概念图变更必须走 `Command` 与撤销栈**，不许直接改 `Document` | `crates/mw-core/src/command.rs`：`MoveConcept`(`:288`)、`InsertConcepts`(`:276`)、`ReverseRelationship`(`:205`)、`AddLinkedConcept`(`:367`) … | 抽样审计：画布侧 `commit_concept_move` 只 push 命令；各命令都有撤销单测（本轮复跑了 5 条） | 通过 |

---

## 4. 缺证据清单（这些一律不算通过）

1. **悬停态（1px 环 + 4% 底）无基线**：离屏 harness 无脚本时每帧清空指针，`cm-*` 里没有
   「用脚本把指针停在概念上」的场景。要补：一条 `MW_GOLDEN_INPUT` 悬停脚本 + 新基线，
   并把 `HOVER_FILL` 的 4% 用像素差量出来（可参照 §5.12 的「加上/撤下」两态做法）。
2. **重叠提示（1px 强调红边、0.25s）无单测无基线**：CM-04 §7.2 已挂账「0.25s 瞬时态与帧率耦合，
   离屏抓不稳」。要补：把老化判据抽成纯函数（输入 `now - started` 与阈值）先写单测，
   再用 `MW_MOTION_T` 式的固定相位拍一张。
3. **拖拽落 `pos` 的手势层无脚本**：`commit_concept_move` 没有 `#[test]`，
   `../CM-04/cm-arrange-click.png` 只锁了菜单路径。要补：按相机把世界坐标算成屏幕坐标的拖拽脚本
   （与 `cm-link-create` 同一笔账，CM-03 §5.1 早已挂账）。
4. **短语断口 8px 是等效实现**（标签底色盖住线段），与合同「线在标签处断开」字面不同；
   CM-08 §6.3 判为「部分」。改几何会牵动全部 `cm-*` 基线，属单独一轮的事。
5. **「新建边留空」这条规则判不了**（§3 第 12b 行）：两份合同相反，见 §7。

## 5. 机器门现状核查（`stage-cm` 到底绿不绿）

**结论：绿，且 CM-14/16/17/18 那笔「基线一张都没重拍」的历史账已经在 2026-11-02 结清——
但有一张图是门后重拍的，当前字节没有被任何一次门复核过。**

| 核什么 | 实测 |
| --- | --- |
| 场景表 | `scripts/x100-stage-cm-gate.ps1` 登记 **73** 场 = 63 场 `cm-*` + 8 场 `mm-*` + 2 场 `search-rel*`（`grep -c "Name = '"` = 73） |
| 日志 | `target/x100/stage-cm-gate.log` 与 `shots/x100/CER-34/stage-cm-gate-2026-11-02.log` **逐字节相同**（sha256 `a434c8c622a59998…`），`started=2026-11-02 12:37:21`、`PASS concept golden pass 1 (73 comparisons)`、`PASS concept golden pass 2 (73 comparisons)`、`tests passed=1279 failed=0`、`MACHINE GATE PASS; HUMAN G-CM-2 REVIEW IS STILL REQUIRED`、`finished=12:41:19` |
| 基线新鲜度 | 除下文标注的 `cm-card-link` 外，63 场 `cm-*` 的 `shell-ppp*.png` mtime 全部落在 **2026-11-02 12:30–12:32**（即门内 `-Update` 那一遍），与门的时间窗对得上 → CM-14/16/17/18（`cm-link-menu`/`cm-link-mode`/`cm-link-two`/`cm-rel-reverse`/`cm-phrase-suggest`/`cm-rel-style`/`cm-rel-multi`/`cm-group`）**确实都重拍并被门复核过** |
| ⚠ 门后重拍 | `shots/golden/cm-card-link/shell-ppp1.00.png` mtime **13:11**、sha256 `2b1ef3c6469ebe03…`；`shots/x100/CER-34/hashes-final-2026-11-02.tsv` 第 112 行记着它是 **13:1x 重拍**（旧 `6c79b019…` → 新 `2b1ef3c6…`，漂移 **99.965%**，原因是 B2-D 的卡片页改版）。**此后没有跑过 `stage-cm` 门**，所以这一场的当前字节是「未过门」状态 |
| 孤儿基线目录 | `cm-outline-label-many4` / `cm-outline-label-many40` / `cm-relink-drag-control` / `cm-summary-select` 四个目录**不在门表内**（历史实验图），不得当作任何判词的证据 |
| ⚠ 代码比门新 | 12:42 之后 `crates/` 有 **40 个 `.rs`** 被改过（含 `scene.rs` 17:26、`theme.rs` 17:31、`instances.rs` 17:32、`canvas_methods_3.rs` 17:04、`app_methods_2.rs` 17:15、`mw-format/src/lib.rs` 17:26），见 §0.2。**「73/146 绿」只能证明 12:41 那一刻**；今天要复现得先重拍受影响的基线 |

要恢复「完全复现」，只需重跑 `powershell -File scripts/x100-stage-cm-gate.ps1`（不复核完整日志时
至少跑 `--tol 0` 单张 `cm-card-link`），或者把它记成「B2-D 之后待复核」一笔。
**在 §0.2 第 1 条工作树漂移被处理之前，这个脚本今天很可能先在 `cm-*` 上大面积报漂**——
那不是「概念图坏了」，是代码与基线不是同一代（彩蛋：这正是金样该起的警报作用）。

---

## 6. M 轨剩余项分类（供下一棒排期）

来源：`docs/30` §12.2 的 23 项对账（已有 15 / 缺 8，⑱⑲⑳㉒ 已由 CM-16/17/18 收口）
+ 各包 `report.md` 的「未做」段 + 本门 §4 的证据缺口。
口径三种（6.1–6.3）；另加 6.2b 记录**门后已被并行会话做掉**的项，避免下一棒重复排期：

### 6.1 能立刻做（不动数据、不缺拍板）

| 项 | 出处 | 说明 |
| --- | --- | --- |
| 分组重命名入口 | CM-18 §5「重命名没做」 | 命令 `Command::SetGroupLabel` **已经存在**（`command.rs:164`），缺的只是行内改名的手势与一个编辑态 |
| 拖成员进出组 / 组框手动缩放 / 嵌套组 | CM-18 §5 | 组框是派生量，前三项都不用改字段；`docs/32` §13 已列为下一版 |
| 框选命中关系线 / `Shift` 范围选择 | CM-17 §5 | 纯选择模型 + 命中几何，不动数据 |
| 命题注释进检索 | CM-16 §5 | 产品决定，但实现成本低（`SearchHit` 多一路字段即可） |
| 悬停/重叠/拖拽三笔证据（§4 第 1–3 项） | 本门 | 只补夹具与基线，不改功能 |
| `cm-card-dot` 纯画布基线 | CM-09 §6.2 | 现在圆点只靠 `cm-card-link-back` 作证，且与选中态耦合 |

### 6.2 要拍板（能不动字段，但口径得先定）

| 项 | 出处 | 要定什么 |
| --- | --- | --- |
| 分组颜色进检查器 / 分组选中道 | CM-18 §5 | 现在只有菜单里 5 档；要不要给分组一条独立选中道 |
| 整理布局的目标形态 | C7 / CM-04 | 「钉住 + 只整理选中」已落地；是否要动画、是否要按语义分层 |

### 6.2b 门后已被并行会话做掉的（记录用：不在本门判词范围）

| 项 | 现状（2026-11-02 17:3x 实测） |
| --- | --- |
| ㉓ `.cxl` / `.cmap` 互导 | **已落码**：`crates/mw-io/src/cmap.rs`（`save_cxl` / `load_cxl`，互通视图 + `<extra-properties-list>` 保真槽，`docs/34` §2），`dispatch.rs` 认 `"cxl" \| "cmap" => DocFormat::Cxl`，三条单测（`cxl_roundtrips_fidelity_lossless` / `cxl_interop_has_concepts_phrases_connections_and_focus` …）。**没有基线、没进 `stage-cm` 门表**，验收归 `docs/34` 那一棒 |
| docs/33 概念图 v2 的 B1/B2/B3 | B1a（空短语留「未命名」占位）已落码 + 单测；B1b/B2/B3（锚点 12px 热区、短语编辑漂移 R1–R3、落点避让）见 `docs/33` §2；本轮不判 |
| 「未命名」占位 | 与 `docs/30` §5.9 冲突，见 §7 |

### 6.3 要新字段（撞 `docs/30` §5.2，必须先批准）

| 项 | 出处 | 卡在哪 |
| --- | --- | --- |
| ⑯ 嵌套概念 | `docs/30` §12.2 / `docs/32` §12.3 | 包含关系没有任何字段；落地要连带改 `concept_rows`（大纲）、`concept_layout`、导出、检索、`validate` |
| ㉑ 命名样式（Styles 面板） | `docs/30` §12.2 | 没有「样式名 → 一组属性」的存储；要么进 `meta`，要么走窗口级偏好 |
| 组框 `rect` 覆盖（若要做手动缩放） | `docs/32` §13 | 现在矩形是派生的；要缩放就得落一个 `rect` 键 |

**已批准的字段例外**（本轮核实仍在约束内）：`LayoutSettings.focus_question`、
`Relationship.head`、`Relationship` 的 `dashed`/`width`/`note`、`Document.groups`/顶层 `groups`、
`NoteCard.archived`。除此之外的字段仍然禁止（`docs/30` §5.2）。

---

## 7. 合同冲突（判词前必须先处理的一条）

**`docs/30` §5.9 / §5.6.1 第 4 条 与 `docs/33` §2 B1a 对「新建连线留空」给出相反的规定，
代码跟的是 `docs/33`。**

| 出处 | 原文（行号是快照） | 现状 |
| --- | --- | --- |
| `docs/30` §5.9（第 571 行） | 「禁止无链接短语的『线团』：新建连线即进入短语编辑态，**未填不给连线落盘**」 | **未改**，仍写着旧口径 |
| `docs/30` §5.6.1 第 4 条（第 524 行） | 「`Tab` 新建的边在**未填就提交/取消**时**整条撤掉**」 | **未改** |
| `docs/33` §2 B1a（2026-11-02 11:53） | 「建线后**不写短语就留着**：线落地、短语显示为『未命名』占位，事后可补；只有 `Esc` 显式取消才撤」 | 这是**现行代码** |
| 代码 | `crates/mw-shell/src/canvas/canvas_methods_3.rs:41` `PhraseCommit::Set(Some(CONCEPT_UNNAMED_LINK.to_string()))`；`:8` `pub(crate) const CONCEPT_UNNAMED_LINK: &str = "未命名";`；`:1173` 注释「显式 `Esc` = 放弃这条命题 …这是**唯一**的撤线路径」 | 与 `docs/33` 一致 |
| 测试 | `canvas::tests::blank_phrase_commit_follows_the_concept_map_contract`（`tests.rs:3186` 起）明写「**改前是 DropEdge 整条撤掉**」 | 断言站 `docs/33` 这一边 |

**本门怎么处理**：这条**不判**（§3 第 12b 行记 `未判`）。理由：判「通过」等于认可一条被新规范
推翻的旧条款；判「不通过」等于把 `docs/33` 的主动改口径当成缺陷。正确动作是先让两份合同对齐
（改 `docs/30` §5.9 与 §5.6.1，或改 `docs/33`），再补 `docs/33` §2 B1a 要求的金样
`cm-link-create-nophrase`（**目前 `scripts/` 与 `shots/golden/inputs/` 里都没有**）。
顺带：门内 `cm-phrase-blank` 的注释还写着「整条线撤掉」，而 `canvas_methods_3.rs` 在门后
（17:04）已被改过，所以这张 12:32 的图拍的是哪一版口径**没有核实**；重拍时要先量出实际画面，
再把注释改成实测口径，别让注释继续当证据用。

> 这一条不是本门独有的问题：`docs/33` 的 B2（短语编辑漂移）、B3（落点避让）都动了
> `scene.rs` / `concept_layout.rs`，而 `docs/30` §5.4 / §5.5 还写着旧口径。
> 两套合同并存期间，任何「按 docs/30 逐条判」的门都要先做一次对齐。

---

## 8. 复跑命令清单（本文件所有「通过」都能重跑）

```bash
# 机器门（73 场 / 146 次比较；含 fmt / clippy / 全量测试 / release）
powershell -File scripts/x100-stage-cm-gate.ps1

# 判词 1–4 的单测（WORKSPACE 内均可 --exact 命中，本轮逐条跑过）
cargo test -p mw-core   --lib  -- --exact scene::tests::concept_boxes_follow_the_node_contract
cargo test -p mw-core   --lib  -- --exact scene::tests::concept_relationships_are_straight_while_mindmap_keeps_curves
cargo test -p mw-core   --lib  -- --exact scene::tests::scene_keeps_each_concept_at_its_pos_after_the_root_swap
cargo test -p mw-core   --lib  -- --exact theme::tests::concept_maps_default_to_ellipses_and_mindmaps_do_not
cargo test -p mw-core   --lib  -- --exact concept_layout::tests::arrange_keeps_min_gap_inside_a_layer
cargo test -p mw-core   --lib  -- --exact concept_layout::tests::arrange_survives_cycles
cargo test -p mw-core   --lib  -- --exact concept_layout::tests::arrange_is_deterministic_and_keeps_single_node_put
cargo test -p mw-core   --lib  -- --exact document::tests::move_concept_only_works_in_concept_maps
cargo test -p mw-core   --lib  -- --exact document::tests::nudging_a_group_of_concepts_is_one_undo_step
cargo test -p mw-core   --lib  -- --exact document::tests::deleting_the_first_concept_rehangs_the_root_and_undo_restores_it
cargo test -p mw-core   --lib  -- --exact document::tests::deleting_the_last_concept_is_rejected
cargo test -p mw-shell  --bins -- --exact canvas::tests::concept_nudge_steps_are_one_and_eight
cargo test -p mw-shell  --bins -- --exact canvas::tests::arranging_three_concepts_is_one_undo
cargo test -p mw-shell  --bins -- --exact canvas::tests::concept_relative_never_produces_a_tree_child
cargo test -p mw-shell  --bins -- --exact canvas::tests::blank_phrase_commit_follows_the_concept_map_contract
cargo test -p mw-format --lib  -- --exact tests::concept_map_roundtrips_positions_and_relationships

# 裸跑 cargo test 前必须重定向 TEMP（见 HANDOFF §3.1），否则 56+ 条写盘用例假红
```

> 注：`mw-shell` 是 bin crate，`--lib` 会报 `no library targets found`，要写 `--bins`。
