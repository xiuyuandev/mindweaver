# CER-32 · 联动与命令 / 撤销审计

- **日期**：2026-11-02
- **批次**：B2-D（`docs/29` §9）
- **状态**：**已实现** —— 审计第一遍完成（联动矩阵 + Command 审计 + 往返），
  发现 5 条；**F8 已修复**（2026-11-02 09:37），**F6 已按案 ② 修复**（2026-11-02，
  用户授权 AI 代拍板，原话：「继续开发，而且呢，你帮助我验收，不需要我决定。」）。

---

## 1. 范围

`docs/29` §9 CER-32 的六项交付，逐项落点：

| 合同项 | 本包审计的路径 |
| --- | --- |
| 大纲节点 → 卡片（右点 / 入口） | 大纲行菜单「新建卡片笔记 / 打开卡片」、画布节点菜单、检查器「关联已有」 |
| 卡片来源 → 画布选中 | 卡片页顶栏来源、库预览「跳到来源节点」 |
| 标签 → 库筛选 | 侧栏标签树 → `CardQuery` → 列表 / 预览 |
| 反链 | 卡片页页尾「反向链接」、`[[标题]]` 断链 |
| 创建 / 归档 / 删除卡片 | `Command::AttachNewCard` / `NoteStore::set_archived` / `remove_many` |
| 文档内写入全走 `Command`；库内卡片走 `NoteStore` | 壳层全部 `Document` 写入点 + 阅读页保存两路 |

## 2. 非目标

- 不新增功能、不改数据模型（`NoteCard.archived` 是 CER-22 已批准的例外）。
- 不重做画布 / 检查器 / 库页界面；只审联动与写入路径。
- 不修 CM-18 的两张基线（`stage-cm` 仍按 70 / 72 红引用）。
- 不在树未静默时改 `mw-shell`（本包只读）。

## 3. 变更

- `crates/mw-shell/src/canvas/canvas_methods_1.rs`：把 `select_node_keep_focus` 的揭示逻辑抽成
  `reveal_and_select(id, why)`（`select_node_keep_focus` 以 `"search"` 委托，行为不变），
  轨迹前缀随 `why` 走（`card_focus_exit` / `card_reveal`）。
- `crates/mw-shell/src/app_methods_3.rs`：`jump_to_source` 改调 `canvas.reveal_and_select(id, "card")`，
  且**先揭示、成功后才关阅读页 / 库页**——失败时调用方的提示还有地方显示。
- 新建本目录 `report.md` + `manifest.json`；`docs/STATUS.md` §3.3 记 CER-32 `进行中`。
- 验证：`cargo check -p mw-shell --release` 绿；`cargo test -p mw-shell --bin mw-shell -- canvas`
  147 passed、`-- note_view` 80 passed（TEMP/TMP 指向 `target/tmp`）。

## 4. 验证（审计证据）

### 4.1 联动矩阵

| # | 入口 | 处理链 | 存储 | 撤销 | 结论 |
| --- | --- | --- | --- | --- | --- |
| L1 | 大纲行菜单「新建卡片笔记」/ 检查器「新建卡片」 | `A::NewCard` → `capture_card_for_selected` → `canvas.capture_card_for_primary` → `Command::AttachNewCard` | `Document.cards`，随后 `sync_notes()` 落库 | 一条命令一次撤销 | ✅ |
| L2 | 大纲行菜单「打开卡片」 | `OA::OpenCard` → `doc.card_for_node` → `card_reader_for`，轨迹 `outline_open_card node=… card=…` | 只读打开 | 不涉及 | ✅ |
| L3 | 检查器「关联已有」 | `card_pick_list` 只列 `doc.linkable_cards`（当前文档内）→ `Action::LinkCard` → `Command::SetNodeCard` + `sync_notes()` | 文档 + 库同步 | 一条命令一次撤销 | ✅ |
| L4 | 检查器「解除关联」 | `Action::DetachCard` → `Command::SetNodeCard { card: None }`；卡片保留为孤儿 | 文档 + 库同步 | 一次撤销可恢复链接 | ✅ |
| L5 | 卡片页来源 / 库预览「跳到来源节点」 | `ReaderAction::JumpToNode` / `LibraryAction::JumpToNode` → `jump_to_source` → `canvas.reveal_and_select(id, "card")`（退聚焦 → 一条 `Batch` 展开 → 选中 + glow）→ 轨迹 `card-jump node=… concept=…` | 只动选择与相机；展开走 `Command::Batch` | 展开可一次撤销 | ✅（F8 已修） |
| L6 | 库「挂到当前主题」 | `LibraryAction::Attach` → `NoteStore::attach_to_node`（只写 `source_doc/source_node`） | 只写库 | 库操作，无文档撤销 | ⚠️ **F7（语义边界）** |
| L7 | 标签树 → 列表 | 侧栏 tag → `filter.tag` → `recompute` → `CardQuery::filter` | 只读 | 不涉及 | ✅（基线 `library-tag-filter`） |
| L8 | 反链 | `Document::backlinks_for_card`（节点 `card_id` + 卡片正文 `[[标题]]`）→ 页尾 → `ReaderAction::OpenCard` | 只读 | 不涉及 | ✅ |
| L9 | 创建 / 归档 / 删除卡片 | 创建走 `Command::AttachNewCard`；归档走 `NoteStore::set_archived`；移出走 `NoteStore::remove_many`（只删库文件，不删 `.zmind` 内嵌副本） | 见左 | 创建可撤销；归档/移出是库操作 | ⚠️ **F6** |
| L10 | 每日笔记 | `NoteStore::daily_note` → `upsert`，不伪造文档命令 | 库 | 页内文本撤销 | ✅ |

### 4.2 Command 审计

壳层全部 `Document` 写入点（`grep` 过 `.doc.*` 赋值 / `get_mut` / `push`）：

| 位置 | 内容 | 判定 |
| --- | --- | --- |
| `crates/mw-shell/src/app_methods_1.rs:746` | 概念图卡片演示夹具直接从零构造 `cards.push` + `node.card_id` | 允许：等价 `Template::session()` 的初始文档，无撤销历史（注释已说明） |
| `crates/mw-shell/src/canvas/canvas_methods_1.rs:148` | `add_asset` 把图片字节插进 `Document.assets` | 允许：内容哈希去重、非结构语义、不进撤销栈（`docs/27` 资产口径） |
| 其余卡片写入 | `save_card_from_reader`：文档内卡片 → `SetCardTitle` / `SetCardBody` / `SetCardTags`（多条时一条 `Batch`）；库内独立卡片 → `NoteStore::upsert` | ✅ 与 `docs/28` §8 一致 |

- 撤销/重做：`AttachNewCard` / `SetNodeCard` / `SetCard*` 全部走 `ChangeSet::with_cards(before, after)`，
  卡片集合随命令一起入栈（`crates/mw-core/src/document.rs` 各分支）。
- 库内卡片不直接写文件：`note_library*` / `note_view.rs` 内没有卡片存储写盘；
  `note_view.rs` 两处 `fs::OpenOptions` 是打字机探针日志（`MW_TRACE` 口径），不是卡片正文。

### 4.3 `.zmind` 往返

| 点 | 证据 |
| --- | --- |
| `cards` 缺键 = 老行为 | `crates/mw-format/src/lib.rs:80-82`：`#[serde(default, skip_serializing_if = "Vec::is_empty")]` |
| `archived` 缺键 = `false`、只在 `true` 时写键 | `docs/32` §14.2；`mw-format` 的 `embedded_card_json_treats_missing_archived_as_false` |
| 卡片逐字段往返 | `mw-format` 的 `cards_round_trip_and_become_readable_markdown_entries`（`:958`） |
| 未知顶层键不丢 | `mw-format` 的 `unknown_keys_survive_a_read_save_read_roundtrip`（`:1558`） |

## 5. 发现

### F6（高）· 归档状态会被「文档 → 库」同步冲掉

**复现**（全部是现有代码路径）：

1. 打开一份带节点的文档，节点上挂卡片 A（`Command::AttachNewCard`），`sync_notes()` 已把 A 写进库。
2. 在笔记库把 A **归档**：`NoteStore::set_archived` 只改库文件与 `index.json`，
   **按 `docs/32` §14.3 不碰 `updated_at`**；`Document.cards` 里的 A 仍是 `archived: false`。
3. 从画布 / 大纲重新打开 A 并改一个字 → `save_card_from_reader` 走 `Command::SetCardBody`，
   `document.rs` 把文档副本的 `updated_at` 刷成 `now_ms()`；保存成功后
   `app_methods_3.rs:388` 立即 `sync_notes()`。
4. `NoteStore::sync_document` 判据是 `meta.updated_at < card.updated_at`（`note_store.rs:638-644`）——
   文档那份更新 → 用文档副本覆盖库文件，`archived` 被写成 `false`，**卡片悄悄回到「全部卡片 / 收件箱」**。

**影响**：归档不持久（只要卡片还挂在节点上并被再次编辑）。`.zmind` 内嵌副本也从来没有
`archived: true`（文档侧没有写这个字段的命令），所以把文档带到另一台机器同样丢归档状态。

**两案**：

| 案 | 改动 | 代价 |
| --- | --- | --- |
| ① 文档侧也走命令 | 新增 `Command::SetCardArchived`，库归档时同步发给打开的文档；撤销会分叉（画布撤销不动库） | 动 `Command` 面，且要再解一次撤销↔库的一致性 |
| ② 库是 `archived` 的唯一真相源 | `sync_document` 用文档内容覆盖时**保留库现有的 `archived`**（`CardMeta.archived` 已有）；加一条单测钉住 | 只动 `note_store.rs` 一处 + 测试；`.zmind` 副本仍不携带归档态 |

**决定（2026-11-02，用户授权代拍板）：案 ②，库是归档状态的唯一真相源。**

理由：① 归档是「库如何组织卡片」的状态，不是文档内容，`.zmind` 副本本来就不携带它；
② 案 ① 会把库操作塞进画布撤销栈，产生「撤销文档命令要不要把库也退回」的分叉；
③ 案 ② 只动 `sync_document` 一处，行为可被一条单测钉死，代价最小。

**已修**：`NoteStore::sync_document` 在文档份更新、准备覆盖库之前，先取库里已有的
`archived` 并写回这次落盘的对象（库里没有该卡时才用文档份的值）；索引与
`CardMeta` 都从合并后的对象重建。新增单测
`syncing_a_newer_document_card_keeps_the_library_archived_flag`
（归档 → 文档份更新正文 → 覆盖后仍归档 + 落盘重开仍归档）。
**边界**：`.zmind` 内嵌副本仍不携带 `archived: true`（文档侧没有写这个字段的命令），
把文档带到另一台机器不会带走归档状态——这是案 ② 明确接受的代价，不是回归。

### F7（低 · 语义边界，不是缺陷）· 库「挂到当前主题」只写来源，不写 `node.card_id`

`LibraryAction::Attach` → `NoteStore::attach_to_node` 只写 `source_doc/source_node`；
节点圆点（`docs/30` §5.3.1）只看 `Node.card_id`（`scene.rs:1311` 一类指纹也只认它），
所以挂完之后节点不会亮卡片圆点，大纲行也没有卡片指示。

这是「来源（provenance）」与「节点卡片（1:1 绑定）」两套关系：
- 来源：卡片属于哪张图 / 哪个节点，用于「跳到来源节点」与检索；
- 节点卡片：`card_id`，决定圆点、检查器「打开卡片」、卡片保存走哪条路。

**结论**：保持现状，但写进本报告避免下一任把它当 bug 修成双重绑定（`SetNodeCard` 是 1:1，
一张卡只能挂一个节点，两者混用会互斥）。

### F8（中 · 已修）· 卡片「跳到来源」不展开折叠、不退聚焦，仍报「已定位」

`jump_to_source`（`app_methods_3.rs:621-645`）只判 `doc.node(id).is_some()`，
然后 `canvas.select_node(id)`；而 `select_node`（`canvas_methods_1.rs:520-532`）在
`scene.find(id)` 为空时**直接 return**——来源主题被折叠、或在聚焦视图之外时，
相机不动、选择不落，但 `jump_to_source` 仍返回 `true`，调用方 toast「已定位到来源主题」。

对照：第 43 轮 M20 / 第 45 轮 M22 已经为搜索跳转做了 `select_node_keep_focus`
（`canvas_methods_1.rs:582-613`）：先退聚焦、再用 `reveal_commands` 包一条 `Command::Batch`
展开挡路祖先、选中并加 glow。**卡片来源跳转没有复用这条路径**，是同一产品动作的两套行为。

**已修（2026-11-02 09:37）**：

- `canvas_methods_1.rs` 抽出 `reveal_and_select(id, why)`；`select_node_keep_focus` 以
  `"search"` 委托，搜索跳转的轨迹与行为逐字不变。
- `jump_to_source` 改调 `reveal_and_select(id, "card")`：来源在折叠子树里先包一条
  `Command::Batch` 展开，在聚焦视图外先退聚焦（视图态、不入撤销栈）。
- 顺序也改了：**先揭示，成功后才关阅读页 / 库页**。以前先关界面再跳，失败时调用方
  `lib.flash` 的提示已经没有可见表面；现在失败会把阅读页 / 库页留在原地。
- 轨迹新增 `card_focus_exit` / `card_reveal`（与搜索的 `search_*` 同构）。

**验证边界**：`Canvas` 没有无头构造器（`Canvas::new` 要 `egui_wgpu::RenderState`），
仓库里也没有 App 级夹具，所以 F8 的「先揭示后关界面」这一层**没有单测**；揭示逻辑本身由
M20/M22 既有的 `reveal_commands_*` Document 级测试兜底。卡片来源命中折叠子树的金样
（`card-jump-collapsed`）留到 CER-34 全量回归时随新夹具一起拍。

## 6. 未做 / 已知偏差

- **F6 已修（案 ②）**，**F8 已修**（见 §5）；F7 保持现状（来源 ≠ 节点卡片绑定）。
- F8 没有 Canvas / App 级无头夹具，揭示路径由既有 `reveal_commands_*` 测试覆盖，
  「先揭示后关界面」的金样留到 CER-34；F6 的单测已随案 ② 一起加。
- F2–F5（CER-31）不属本包；F3 已随本遍一起修（见 CER-31 报告）。
- 未实机核对微软拼音（沿用 B2-C 欠账）。
- 未审概念图的卡片联动细节（`cm-card-link` 由 CM-06 覆盖；本遍只确认它走同一套 `Command`）。
- 未审「大纲焦点行」「多标签切换文档」与卡片来源的组合；CER-33 的性能矩阵会顺带过一遍。

## 7. 风险

| 风险 | 说明 |
| --- | --- |
| 并行会话曾重构 `panels/inspector/` | 09:29 起树编译不过；09:37 已恢复，F8/F3 是在恢复后落地的（`cargo check` 06–07s 绿）。若再开工，先看文件 mtime 与 `cargo check` 再动 |
| F6 的案 ② 若被否，案 ① 会碰 `Command` + 撤销↔库一致性 | 下一任不要先把 `SetCardArchived` 写进去再讨论 |
| F8 若直接调 `select_node_keep_focus` | 该函数会发 `Command::Batch` 展开祖先，属文档变更；提示文案与撤销语义要一起看（搜索跳转已接受了这条口径） |

## 8. 人工判词

CER-32 不是门包（门包是 CER-34 / G-B2-4），本报告不代写判词。
