# G-CM-2 · 概念图联动与导出人工门（M 轨 M2 出口 · CM-06–09）

> 依据：`../../docs/31-外壳体验与概念图工作包.md` §1.3 的 G-CM-2 判词
> 「概念能关联笔记、卡片能跳回概念、大纲可编辑结构、导出往返无损」；
> 设计合同 `../../docs/30-外壳体验与概念图设计规范.md` §5.3.1 / §5.4.1 / §5.6 / §5.6.1 / §5.8 / §5.9；
> 执行合同 `../../docs/31` §6 的 CM-06–09 交付项。
> 授权：用户 2026-11-02 原话（记录在 `../../docs/STATUS.md` 抬头）
> 「继续开发，而且呢，你帮助我验收，不需要我决定。」
> 审计快照：2026-11-02 17:32（本机工作区当前未提交状态；行号是快照，落点行文字一并写出）。
> 包报告：[`report.md`](report.md)；旧门证据包 `../CM-08/report.md` §6。
>
> **本文件保留旧结论不删**：§1 是 2026-10-26 的原判（不通过），§2 起是 2026-11-02 的复判。
> 门通过 ≠ 记 `已验证`：`已验证` 另欠实机微软拼音与系统 DPI 两项。

---

## 0. 结论

| 项 | 结果 |
| --- | --- |
| 旧判（2026-10-26） | **不通过**：`docs/30` §5 的 48 条里「做到 37 / 部分 6 / 没做到 5」，唯一的**功能**缺口是「大纲里改结构」 |
| 复判（2026-11-02，授权代判） | 判词 **通过 4 / 共 4** |
| 逐条证据表（§4，19 条） | 通过 15 / 部分 2 / 缺证据 1 / **未判 1**（合同冲突：本文件 §4.2 第 8b 行 + G-CM-1 §7） |
| 机器门 | `stage-cm` **73 场 / 146 次比较两遍零差异**（2026-11-02 12:37:21→12:41:19）——**只对 12:41 的代码成立**，见 §0.2 与 §6 |
| 不阻塞但必须留在纸上的欠账 | ① 概念 tooltip 画面无基线 ② 「已关联卡片」提示无证据 ③ `cm-card-link` 当前字节是门后重拍的 ④ 实机微软拼音未核 ⑤ `docs/30` §5.9/§5.6.1 与 `docs/33` §2 B1a 就「空短语」互相打架 |
| 状态建议 | CM-06–09 维持 `已实现`；是否升 `已验证` 由有 `docs/STATUS.md` 写权限的一棒决定（同 G-CM-1 §0.1 的理由） |

### 0.2 审计边界：工作区在门后继续前进（这一条必须先看）

1. **机器门是 12:41 的快照**：最后一次真跑是 2026-11-02 12:37–12:41；此后 `crates/` 有
   **40 个 `.rs`** 被改过（`find crates -name '*.rs' -newermt "2026-11-02 12:42"`），含本门直接依赖的
   `mw-core/src/scene.rs`（17:26）、`mw-render/src/instances.rs`（17:32）、
   `mw-shell/src/canvas/canvas_methods_3.rs`（17:04）、`mw-shell/src/app_methods_3.rs`（13:25）、
   `mw-shell/src/export_svg.rs`（17:33）、`mw-format/src/lib.rs`（17:26）。
   所以「73/146 零差异」证明的是 12:41 那一刻；今天复现要先重拍受影响的基线（本棒不跑 golden）。
2. **合同在门后也前进**：`docs/33`（11:53）改了空短语口径（§2 B1a），`docs/34`（17:28）落了
   `.cxl` 互导（`crates/mw-io/src/cmap.rs`）。两者都**不在 G-CM-2 判词范围**，本文件只在
   冲突处留账。
3. 本轮 `cargo test -- --exact` 复跑发生在 **17:18–17:35**，期间工作树仍在被并行会话修改。

---

## 1. 旧判（2026-10-26 · 保留原文结论）

`shots/x100/CM-08/report.md` §6 把 `docs/30` §5 的 48 条可判条款逐条对到代码，
结论是**做到 37 / 部分 6 / 没做到 5**；§8 列出「G-CM-2 要过必须补的五条」：

| # | 缺口 | 归属 |
| --- | --- | --- |
| 1 | **大纲里建/删连线**（带短语编辑态） | 唯一的功能缺口，需先补设计 |
| 2 | 概念省略时的 tooltip | §5.3 |
| 3 | 卡片关联右下 5px 圆点 + hover 提示 | §5.3 |
| 4 | 跨分支连线取来源分支色 | §5.4 |
| 5 | 交叉处 ≤8px 微弧 | §5.4 |

当时另有 6 条「部分」（圆角 8/10、箭头 9×8.4/6×4、线宽 1.6/1.5、标签断口等效实现、
悬停底纹、选中底纹）。按 `docs/31` §3 的完成定义，那一次**人工判词不能算通过**；
CM-09 就是为这五条开的一包，补齐后重跑门再判。

---

## 2. 复判判词（2026-11-02 · 授权代判）

| # | 判词（`docs/31` §1.3 原文拆句） | 看什么 | 结论 | 一句依据 | 复跑方式 |
| --- | --- | --- | --- | --- | --- |
| 1 | **概念能关联笔记** | 卡片绑定三态、候选只列未被占的、来源措辞 | ☑ 通过（授权代判） | 检查器三态（新建/关联已有/解绑）落地为 `SetNodeCard`/`AttachNewCard`；`linkable_cards` 排除别人占的卡、换绑仍可选回原卡（单测）；来源措辞统一为 `{文档名} · {节点名}`（单测） | `cargo test -p mw-core --lib -- --exact document::tests::linkable_cards_exclude_cards_taken_by_other_nodes`；`cargo test -p mw-shell --bins -- --exact note_library::tests::source_phrase_is_doc_dot_node note_library::tests::source_phrase_gives_up_on_an_unknown_doc` |
| 2 | **卡片能跳回概念** | 回跳后画布选中该概念、焦点交还 | ☑ 通过（授权代判） | `Shell::jump_to_source` 是唯一落地点（卡片页与笔记库共用），门内 `cm-card-link-back` 锁「跳回来得选中谁」；CM-06 记了一条反例：拿掉 `select_node` → 该基线报 13729 px 超差 | 门内 `cm-card-link-back`（`powershell -File scripts/x100-stage-cm-gate.ps1`）；轨迹 `../CM-06/cm-card-link.trace.txt` 的 `card-jump node=… concept=1` |
| 3 | **大纲可编辑结构** | `Tab` 建「概念+边」/ `Shift+Delete` 断边 / `Shift+F2` 改短语 | ☑ 通过（授权代判） | 三张门内基线各锁一条键位（`cm-outline-link` / `cm-outline-unlink` / `cm-outline-via`）；键位映射有单测；概念图里树语义键一律不接（`outline_tree_key_ignored`） | 门内三场；`cargo test -p mw-shell --bins -- --exact panels::outline::tests::row_key_maps_the_docs28_5_5_table panels::outline::tests::concept_rows_follow_the_relationship_tree` |
| 4 | **导出往返无损** | `.zmind` 逐字段往返、四种文本导出走同一张生成树、PNG/SVG/PDF 零新增路径 | ☑ 通过（授权代判） | 往返断言逐字段比 `kind`/`pos`/`relationships.label`/`color`；Markdown/OPML/FreeMind/纯文本共用 `concept_rows` 并有单测；SVG 有「概念文字与短语都走同一 `Scene`」的守卫单测 | `cargo test -p mw-format --lib -- --exact tests::concept_map_roundtrips_positions_and_relationships`；`cargo test -p mw-io --lib -- --exact export::tests::concept_map_exports_follow_the_generated_tree`；`cargo test -p mw-shell --bins -- --exact export_svg::tests::concept_map_exports_through_the_same_scene` |

## 3. 判词记录（2026-11-02）

1. **概念能关联笔记** — 检查器卡片区三态：未绑定时给「新建卡片笔记 / 关联已有」，
   点「关联已有」内联候选列表（限高、最多 8 条、只列当前文档里**没被别的节点占用**的卡片，
   自己挂着的那张仍可选 = 换绑），落地走 `SetNodeCard`；解绑走 `SetNodeCard{card:None}`。
   来源措辞按 `Document::is_concept_map()` 在「主题/概念」之间切，卡片页写
   `来源 [概念图] 概念图 · 用户痛点`（文档名恰为「概念图」时不重复类型字）。
   **证据边界**：「关联已有」的候选列表只有一张静态实拍 `../CM-06/cm-card-pick.png`，
   **不在门内基线里**；面板内的展开态没有像素回归保护。
2. **卡片能跳回概念** — `app_methods_3.rs:621` 的 `pub(crate) fn jump_to_source(&mut self, id: &str) -> bool`
   收口「关浮层 + 选中 + 交还焦点 + `trace card-jump`」，卡片页与笔记库共用同一个落地点。
   门内基线 `cm-card-link-back`（`MW_CONCEPT_CARD_BACK=用户痛点`）拍的就是跳完之后画布的选中态；
   CM-06 §6 记了反例「拿掉 `select_node` → 13729/2304000 px（0.596%）超差」，说明这张图真的在守。
   **证据边界**：跨文档回跳仍只 toast「来源概念不在当前文档里」（CM-06 §9.3 挂账）。
3. **大纲可编辑结构** — 这是旧判唯一的**功能**缺口，CM-09 落地为：`Tab` = `Batch[AddFloatingTopic,
   AddRelationship]`（先写概念名、再写链接短语；短语没填就整条撤掉），`Shift+Delete` 只断
   「进来那条边」、概念留成孤立根，`Shift+F2` 就地改短语（空串不提交、保留原短语并提示），
   `Enter` 只加概念不加边；`Shift+Tab` 与树语义移动键在概念图里**不接**（留
   `outline_tree_key_ignored` 轨迹）。键位映射在 `outline.rs:950` `(false, true, Key::Delete) => Some(RowKey::Unlink)`
   与 `:952` `(false, true, Key::F2) => Some(RowKey::RenameVia)`，概念图分支在
   `:4869` `RowKey::Indent if self.concept_map =>` 处把 `Tab` 改成建边。
   三张门内基线：`cm-outline-link`（新行 + 短语前缀）、`cm-outline-unlink`（深度回 0、概念还在）、
   `cm-outline-via`（行首短语被改、行高不变）。
   **一条子规则已过期**：`docs/30` §5.6.1 第 4 条写「`Tab` 新建的边在未填就提交/取消时整条撤掉」，
   而现行代码只撤 `Esc`、留空则给「未命名」占位（`docs/33` §2 B1a，见 §4.2 第 8b 行）。
   本门对这条**不判**，理由同 G-CM-1 §7：两份合同没对齐，判哪边都不对。
   **证据边界**：这三场的轨迹只写在报告正文（[`report.md`](report.md) §5.4 的表里引了
   `outline_concept_link_done` / `outline_concept_unlink … ok=true` / `outline_concept_via_edit`），
   目录里**没有留 `*.trace.txt`**；唯一落盘的旁证是 `../CER-12/trace-via-keys.txt`
   （`outline_row … via=Some("解决")` → `shift+F2` → `outline_concept_via_edit`）。所以键位这一层靠
   **基线像素 + 单测**，轨迹只作旁证。
4. **导出往返无损** — 三条一起看：① `.zmind` 往返逐字段相等、不升 `FORMAT_VERSION`
   （`mw-format` 单测，本轮已复跑 1 passed）；② 四种文本导出一律走
   `mw_core::concept_tree::concept_rows` 的**同一张生成树**，Markdown 文首 `> 概念图 · 按链接层级展开`、
   OPML 标题后缀、FreeMind XML 注释，链接短语随行导出成 `{短语} → {概念}`
   （`crates/mw-io/src/export.rs:30` `fn concept_label`、`:42` `const CONCEPT_NOTE`、`:135` `fn concept_children`）；
   ③ PNG/SVG/PDF 一行代码都没动，仍吃 `Scene`（`export_svg::tests::concept_map_exports_through_the_same_scene`
   守「零新增导出路径」）。
   **口径**：文本格式没有「边」这种结构，属**有损导出**（`docs/05` §7 口径）；
   「无损」指的是 `.zmind` 往返，这一点在判词里必须写清楚，不能拿文本导出当无损证据。

---

## 4. 逐条证据表（`docs/30` §5 → 代码落点 → 证据）

判定口径同 G-CM-1：**通过** = 有门内基线或 `--exact` 单测；**部分** = 主体做到、有子项缺证据；
**缺证据** = 只有代码。

### 4.1 CM-06 概念 ↔ 笔记联动

| # | 合同条款 | 代码落点 | 证据 | 判定 |
| --- | --- | --- | --- | --- |
| 1 | 卡片绑定三态（新建 / 关联已有 / 解绑），`card_id` 同语义 | `crates/mw-core/src/document.rs:384` `pub fn linkable_cards(&self, node_id: &str, limit: usize) -> Vec<&NoteCard>`；检查器 `card_section` 三态 | `document::tests::linkable_cards_exclude_cards_taken_by_other_nodes`（**已复跑**）；静态实拍 `../CM-06/cm-card-pick.png`（**非门内基线**） | 通过 |
| 2 | 来源 = `{文档名} · {节点名}`；卡片页/笔记库/导出包三处措辞一致 | `crates/mw-core/src/document.rs` 的 `display_title()`/`is_concept_map()`；`crates/mw-shell/src/app_methods_3.rs:579` `pub(crate) fn card_sources(`；`crates/mw-shell/src/collection.rs:28–30` 概念图写 `"概念"` | `document::tests::display_title_falls_back_and_marks_concept_maps`（**已复跑**）；`note_library::tests::source_phrase_is_doc_dot_node` + `source_phrase_gives_up_on_an_unknown_doc`（**均已复跑**）；导出包那一行**无单测** | 通过 |
| 3 | 回跳（卡片 → 概念）只有一个落地点，选中 + 居中 + 交还焦点 | `crates/mw-shell/src/app_methods_3.rs:621` `pub(crate) fn jump_to_source(&mut self, id: &str) -> bool` | 门内 `cm-card-link-back`；轨迹 `../CM-06/cm-card-link.trace.txt` 的 `card-jump node=… concept=1` + `focus canvas`；反例（拿掉 `select_node` → 13729 px） | 通过 |
| 4 | 反查用例：空白标题回落「未命名」、别人占的卡不进候选、概念图下反查取名 | `document.rs` 三处 | `display_title_falls_back_and_marks_concept_maps` / `linkable_cards_exclude_cards_taken_by_other_nodes` / `concept_card_backlink_names_the_concept`（**三条均已复跑**） | 通过 |

### 4.2 CM-07 大纲视图与导出

| # | 合同条款 | 代码落点 | 证据 | 判定 |
| --- | --- | --- | --- | --- |
| 5 | 命题网 → 生成树：环取首次访问边、孤立概念各自成根（按 `(y,x,id)`）、树文档返回空表、行带 `via_id`/`branch` | `crates/mw-core/src/concept_tree.rs:43` `pub fn concept_rows(doc: &Document) -> Vec<ConceptRow>`；`:121` `fn root_order(doc: &Document)` | `concept_tree::tests::rows_follow_edges_in_declaration_order_with_depth_and_phrase` / `cycles_are_cut_at_the_first_visit_and_never_loop` / `first_visit_edge_wins_when_two_edges_point_at_one_concept` / `unreachable_concepts_become_roots_ordered_by_position` / `tree_documents_have_no_concept_rows` / `rows_carry_edge_id_and_branch_number`（**六条均已复跑**） | 通过 |
| 6 | 大纲按生成树展开；页头「概念图 · 按链接层级展开」，页脚「N 个概念」，无树线 | `crates/mw-shell/src/panels/outline.rs:223` `via_id: Option<String>,`；`Outline::rebuild_concept` | 门内 `cm-outline`；`panels::outline::tests::concept_rows_follow_the_relationship_tree`（**已复跑**） | 通过 |
| 7 | 大纲键位改结构：`Tab` 建「概念+边」、`Shift+Delete` 只断边、`Shift+F2` 改短语、`Enter` 只加概念；树语义键不接 | `outline.rs:950` `(false, true, Key::Delete) => Some(RowKey::Unlink),`；`:952` `(false, true, Key::F2) => Some(RowKey::RenameVia),`；`:4869` `RowKey::Indent if self.concept_map =>`；`:4874` `RowKey::Unlink if self.concept_map =>` | 门内 `cm-outline-link` / `cm-outline-unlink` / `cm-outline-via`；`panels::outline::tests::row_key_maps_the_docs28_5_5_table`（**已复跑**，含 `Shift+Delete→Unlink`、`Shift+F2→RenameVia`） | 通过 |
| 8 | 短语就地编辑契约：单一表面、本地草稿、`Esc` 还原、`Enter` 提交、空串不提交、一次撤销 | `outline.rs` 的 `via_edit` / `PendingVia` / `outline_concept_via_reject` | 门内 `cm-outline-via`（行首短语被改、行高不变）+ `cm-outline-via-clear`（`outline_concept_via_reject reason=empty`，短语框开着时 `Ctrl+A` 不漏选全表）+ `cm-phrase-clear` | 通过 |
| 8b | §5.6.1 第 4 条：`Tab` 新建的边「未填就提交/取消时**整条撤掉**」 | `canvas_methods_3.rs:41` `PhraseCommit::Set(Some(CONCEPT_UNNAMED_LINK.to_string()))`；`:8` `pub(crate) const CONCEPT_UNNAMED_LINK: &str = "未命名";` | 代码与 `docs/33` §2 B1a 一致（留线 + 占位、只有 `Esc` 撤），与 `docs/30` §5.6.1/§5.9 相反；`cm-link-create-nophrase` 金样在 `scripts/` 与 `shots/golden/inputs/` 里都不存在，门内无场景锁新口径 | **未判**（合同冲突，先定合同） |
| 9 | 四种文本导出走同一张生成树，文首/标题/注释标注「概念图」，短语随行 | `crates/mw-io/src/export.rs:30` `fn concept_label(doc: &Document, row: &mw_core::ConceptRow) -> String`；`:42` `const CONCEPT_NOTE`；`:135` `fn concept_children` | `export::tests::concept_map_exports_follow_the_generated_tree`（**已复跑：1 passed**，逐字节比 Markdown/纯文本、OPML 嵌套、FreeMind 注释） | 通过 |

### 4.3 CM-08 证据包与门

| # | 合同条款 | 代码落点 | 证据 | 判定 |
| --- | --- | --- | --- | --- |
| 10 | `.zmind` 往返逐字段相等（`kind`/`pos`/`relationships` 含 `label`+`color`），版本未升 | `crates/mw-format/src/lib.rs:1202` `fn concept_map_roundtrips_positions_and_relationships()` | **已复跑：1 passed**；门内 `concept map .zmind roundtrip` 步（注意：该步日志只打了最后一个测试二进制的尾巴，断言本体要看 `mw-format` 的 lib 测试） | 通过 |
| 11 | `Scene` 是几何唯一真相源：PNG/SVG/PDF **零新增导出路径** | `crates/mw-shell/src/export_svg.rs:768` `fn concept_map_exports_through_the_same_scene()` | **已复跑：1 passed**；CM-07 §1 用 `grep -rn "children" export_*.rs` 做过审计 | 通过 |

### 4.4 CM-09 收口：旧判的五条缺口

| # | 缺口 | 代码落点 | 证据 | 判定 |
| --- | --- | --- | --- | --- |
| 12 | ① 大纲里建/删/改连线 | 同 §4.2 第 7/8 行 | 同 §4.2 第 7/8 行 | 通过 |
| 13 | ② 概念**真截断才出** tooltip（停 400ms、节点下方 8px、最大宽 320、卡片优先级更高） | `crates/mw-core/src/scene.rs:113` `pub truncated: bool,`；`crates/mw-shell/src/canvas/canvas_methods_3.rs:3537` `pub(crate) fn paint_concept_tip(`；`:3603` `if now - self.tip_at < 0.40 { return; }` | `scene::tests::concept_nodes_report_truncation_and_card_dot`（**已复跑**，只断言 `truncated`/`has_card` 两个**标记**）；**提示框的绘制与 400ms 计时没有任何基线也没有单测**——离屏 harness 无脚本时 `hovered` 恒为 `None`，`cm-*` 里也没有「脚本悬停概念节点」的场景 | **缺证据** |
| 14 | ③ 卡片关联右下 5px 圆点 + hover 提示「已关联卡片：{标题}」 | `crates/mw-render/src/instances.rs:710–735`（`CONCEPT_CARD_DOT_R = 2.5` 即 5px 直径、`CONCEPT_CARD_DOT_INSET = 9.0`，走 `cam.chrome` 屏幕恒定）；提示在 `canvas_methods_3.rs:3586` `.map(\|c\| format!("已关联卡片：{}", c.title))` | 圆点：门内 `cm-card-link-back`（CM-09 §5.2 记了它的最大差 205 正是圆点/提示层）+ `concept_nodes_report_truncation_and_card_dot`；CM-09 §6.2 建议补一张纯画布 `cm-card-dot`（**未做**）。提示：**无证据**（同第 13 行） | 部分 |
| 15 | ④ 跨分支连线取**来源**分支色（`palette_at(branch % len)`、显式色优先、**不写回文档**） | `crates/mw-core/src/scene.rs:755` `fn concept_rel_color(...)`；`:767` `(Some(a), Some(b)) if a != b => theme.palette_at(a as usize)...` | `scene::tests::cross_branch_links_borrow_the_source_branch_colour`（**已复跑**）；门内 `cm-default`（CM-09 §5.2 记了它 2631 px 的漂移正是 `ui.dim` → 分支色） | 通过 |
| 16 | ⑤ 交叉处 ≤8px 微弧（后声明那条中点偏 6px，箭头按终点切线重算，同参两次逐字段相同） | `scene.rs:775` `fn bow_crossings(rels: &mut [SceneRelationship])`；`:814` `fn segments_cross`（端点相接不算、夹角 <12° 不算、0.5 单位死区）；`:848` `fn bowed_between` | `scene::tests::only_true_crossings_count` / `bow_offset_is_capped_and_deterministic` / `crossing_bow_leaves_a_lane_curve_alone`（**三条均已复跑**）；门内 `cm-crosslink` | 通过 |

### 4.5 禁止项与门完整性

| # | 条款 | 代码落点 / 证据 | 判定 |
| --- | --- | --- | --- |
| 17 | §5.9 禁止项：不渲染成辐射树、无自动互链/自动生成概念、不改 `.zmind` 版本或字段、不引入第二套工具条、不用 `children` 表达层级 | 门内 `cm-default`/`cm-narrow`（不是辐射树）；`concept_map_roundtrips_positions_and_relationships`（版本未升）；`canvas::tests::concept_relative_never_produces_a_tree_child`（不写 `children`）；审计未发现自动互链代码 | 通过 |
| 18 | 门内场景与基线的**可复现性** | 73 场 / 146 次比较两遍零差异（12:41），但 `cm-card-link` 的基线在 **13:11** 被 B2-D 重拍过一次、之后没再跑门（详见 §6） | 部分 |

---

## 5. 缺证据清单（这些一律不算通过）

1. **概念 tooltip 的绘制与 400ms 计时**（第 13 行）：`truncated` 标记有单测，但「停 400ms 才出、
   下方 8px、最大宽 320、卡片提示优先」这四条**一条都没有像素或单测证据**。
   要补：把「该不该出、出什么文案」抽成纯函数先写单测；再补一条
   `MW_GOLDEN_INPUT` 悬停脚本（指针停在长概念上 >400ms）+ 新基线。
2. **「已关联卡片：{标题}」提示**（第 14 行末）：卡片标题取不到时的退化文案
   （`已关联卡片`）也没有证据。
3. **「关联已有」候选列表**（第 1 行）：只有静态实拍 `../CM-06/cm-card-pick.png`，不在门内。
4. **实机微软拼音**：`cm-label-ime` 只做离屏等效（注入 preedit），`docs/30` §5.4 的
   「IME 组合不提交」在真输入法里没核过——不阻塞本门，但阻塞 `已验证`。
5. **导出文件的字节没有任何基线**：PNG/SVG/PDF 的「同源」靠 `Scene` 唯一路径的代码约定 +
   一条 SVG 守卫单测，没有导出产物快照。
6. **「新建边留空」这条规则判不了**（§4.2 第 8b 行）：`docs/30` §5.6.1 第 4 条 / §5.9 说「整条撤掉」，
   `docs/33` §2 B1a 说「留着、给『未命名』占位」，现行代码跟后者。两份合同先对齐，再谈判词；
   `docs/33` 要的金样 `cm-link-create-nophrase` 目前不存在。

## 6. 机器门现状核查（`stage-cm` 到底绿不绿）

**结论同 G-CM-1 §5：绿，但有一张图是门后重拍的。** 关键数字：

| 核什么 | 实测 |
| --- | --- |
| 场景表 | `scripts/x100-stage-cm-gate.ps1` 共 **73** 场（63 `cm-*` + 8 `mm-*` + 2 `search-rel*`）；G-CM-2 自己用 10 场（`cm-default`/`cm-narrow`/`cm-arrange`/`cm-crosslink`/`cm-card-link`/`cm-card-link-back`/`cm-outline`/`cm-node-edit`/`cm-link-create`/`cm-label-ime`）+ CM-09 的三场 `cm-outline-*` |
| 日志 | `target/x100/stage-cm-gate.log` ≡ `shots/x100/CER-34/stage-cm-gate-2026-11-02.log`（同一 sha256 `a434c8c622a59998…`）：`12:37:21` 起、`PASS concept golden pass 1/2 (73 comparisons)`、`tests passed=1279 failed=0`、`MACHINE GATE PASS`、`12:41:19` 止 |
| 基线新鲜度 | 除下文标注的 `cm-card-link` 外，63 场 `cm-*` 基线 mtime 全在 `12:30–12:32`（门内 `-Update` 那一遍）→ **CM-14/16/17/18 的「一张都没重拍」历史账已结清** |
| ⚠ 门后重拍 | `shots/golden/cm-card-link/shell-ppp1.00.png` mtime **13:11**、sha256 `2b1ef3c6…`（旧 `6c79b019…`，漂移 99.965%，B2-D 卡片页改版所致；记录在 `../CER-34/hashes-final-2026-11-02.tsv` 第 112 行）。**之后没有再跑门**，所以本门第 1 条判词引用的 `cm-card-link` 当前字节是「未过门」的；同一条判词还有 `cm-card-pick.png` 与三条单测支撑，`cm-card-link-back` 仍是 12:31 的已过门字节 |
| 孤儿基线 | `cm-outline-label-many4`/`-many40`/`cm-relink-drag-control`/`cm-summary-select` 不在门表内，不得作为证据 |
| ⚠ 代码比门新 | 12:42 之后 `crates/` 有 **40 个 `.rs`** 被改（含 `scene.rs` 17:26、`instances.rs` 17:32、`canvas_methods_3.rs` 17:04、`export_svg.rs` 17:33、`mw-format/src/lib.rs` 17:26）。**「73/146 绿」只覆盖 12:41 那一刻的代码**；今天复现要先按 `docs/29` §5 重拍受影响的基线，见 §0.2 |

复现方式：`powershell -File scripts/x100-stage-cm-gate.ps1`（若只补核这一张：
单跑 `target/release/mw-shell.exe --golden --ppp 1.0 --tol 0 --warmup 6 --dir shots/golden/cm-card-link`
并带上门表里的 `MW_CONCEPT_DEMO=1 MW_CONCEPT_CARD=1` 环境）。

**顺带发现的一处报告自相矛盾（不改它，只记账）**：[`report.md`](report.md) §5.4 的表格写
`cm-outline-via`「**已重开**（2026-10-26 夜）… 基线 `shots/golden/cm-outline-via/`」，
而同一节最后一段又写「`cm-outline-via` 的目录、输入脚本、门表条目本轮已一并撤除」。
以文件与门表为准：该场**在**门表内（第 197 行）、基线在（mtime `12:32`），
所以末段那句是过期的话，§6.1 的「仍未拍」同样过期。下一棒值得顺手把那两处删掉或改期。

---

## 7. M 轨剩余项（与本门相关部分，完整分类见 `../CM-05/gate.md` §6）

- **能立刻做**：`cm-card-dot` 纯画布基线、tooltip 的纯函数单测与悬停脚本、
  候选列表进基线、框选命中关系线、`Shift` 范围选择、分组重命名入口（`SetGroupLabel` 命令已有）。
- **要拍板**：命题注释进不进检索、分组是否要独立选中道。
- **要新字段**：⑯ 嵌套概念、㉑ 命名样式、组框 `rect` 覆盖（均撞 `docs/30` §5.2，需先批准）。
- **门后已被并行会话做掉的（不在本门范围）**：㉓ `.cxl`/`.cmap` 互导已在
  `crates/mw-io/src/cmap.rs` 落码（`docs/34` §2，含保真槽与三条单测），但没有基线、没进门表，
  验收归 `docs/34` 那一棒；`docs/33` 的 B1/B2/B3 同理。原「要拍板」里的 ㉓ 一条作废。

---

## 8. 复跑命令清单

```bash
# 门（73 场 / 146 次比较；fmt + clippy + 全量测试 + release）
powershell -File scripts/x100-stage-cm-gate.ps1

# 四条判词的骨架单测（本轮逐条跑过，全部 --exact 命中）
cargo test -p mw-format --lib  -- --exact tests::concept_map_roundtrips_positions_and_relationships
cargo test -p mw-io     --lib  -- --exact export::tests::concept_map_exports_follow_the_generated_tree
cargo test -p mw-core   --lib  -- --exact concept_tree::tests::rows_follow_edges_in_declaration_order_with_depth_and_phrase
cargo test -p mw-core   --lib  -- --exact concept_tree::tests::cycles_are_cut_at_the_first_visit_and_never_loop
cargo test -p mw-core   --lib  -- --exact concept_tree::tests::first_visit_edge_wins_when_two_edges_point_at_one_concept
cargo test -p mw-core   --lib  -- --exact concept_tree::tests::unreachable_concepts_become_roots_ordered_by_position
cargo test -p mw-core   --lib  -- --exact concept_tree::tests::tree_documents_have_no_concept_rows
cargo test -p mw-core   --lib  -- --exact concept_tree::tests::rows_carry_edge_id_and_branch_number
cargo test -p mw-core   --lib  -- --exact document::tests::display_title_falls_back_and_marks_concept_maps
cargo test -p mw-core   --lib  -- --exact document::tests::linkable_cards_exclude_cards_taken_by_other_nodes
cargo test -p mw-core   --lib  -- --exact document::tests::concept_card_backlink_names_the_concept
cargo test -p mw-core   --lib  -- --exact scene::tests::cross_branch_links_borrow_the_source_branch_colour
cargo test -p mw-core   --lib  -- --exact scene::tests::only_true_crossings_count
cargo test -p mw-core   --lib  -- --exact scene::tests::bow_offset_is_capped_and_deterministic
cargo test -p mw-core   --lib  -- --exact scene::tests::concept_nodes_report_truncation_and_card_dot
cargo test -p mw-shell  --bins -- --exact panels::outline::tests::concept_rows_follow_the_relationship_tree
cargo test -p mw-shell  --bins -- --exact panels::outline::tests::row_key_maps_the_docs28_5_5_table
cargo test -p mw-shell  --bins -- --exact export_svg::tests::concept_map_exports_through_the_same_scene
cargo test -p mw-shell  --bins -- --exact note_library::tests::source_phrase_is_doc_dot_node
cargo test -p mw-shell  --bins -- --exact note_library::tests::source_phrase_gives_up_on_an_unknown_doc

# 裸跑 cargo test 前必须重定向 TEMP（HANDOFF §3.1）；mw-shell 是 bin crate，用 --bins 不用 --lib
```
