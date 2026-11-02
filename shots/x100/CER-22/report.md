# CER-22 · 信息架构与查询 API（笔记库 B2-C 第一包）

日期：2026-11-02　状态：**已实现**（机器证据齐；G-B2-3 人工门未开）
合同：`docs/29` §8 CER-22；设计：`docs/28` §7.1–§7.3、§7.11；字段决定：`docs/32` §14。

## 1. 范围

- 四个 scope 的边界写死在领域层：`CardScope::{All, Inbox, Untagged, Archived}`
  （`crates/mw-core/src/note.rs` 的 `accepts`）：
  - `All` = 未归档；
  - `Inbox` = 未归档 **且** 无标签 **且** 未挂导图节点（修正旧版「收件箱 = 全部」的错误语义）；
  - `Untagged` = 未归档且无标签（挂了节点但没标签的卡仍在这里）；
  - `Archived` = 已归档。
- `CardQuery { scope, tags(AND), sort }` 成为**唯一**查询入口：侧栏计数、列表行、预览取数
  都走它（`accepts` / `filter` / `sort_in_place` / `sort_owned` / `cmp_cards`）。
  搜索要读正文，归壳层 `NoteStore`，领域层只认「哪些 id 命中」。
- 标签筛选按路径 AND，且父路径命中子标签（`知识` 命中 `知识/方法`），大小写不敏感。
- 侧栏标签树 `TagNode`：最多两级（`TAG_TREE_DEPTH = 2`），计数只数**未归档**卡且聚合子标签。
- `NoteCard.archived: bool`（用户 2026-11-01 拍板，`docs/32` §14）：不升 `FORMAT_VERSION`、
  缺键即老行为、无迁移；`set_archived` 不碰 `updated_at`（归档不改内容）；
  `CardMeta.archived` 进本地索引，重开库不扫全文。
- 批量 API：`NoteStore::set_archived / add_tags / remove_tags / delete` 一律返回
  `BatchReport`（逐 id 报成败，整批失败不静默）。

## 2. 非目标

- 不改 `.zmind`、不改 `Node`；归档只存在于卡片库。
- 不做「按归档时间排序」（故不加 `archived_at`，见 `docs/32` §14 的否决理由）。
- 不做标签重命名/合并的管理 UI（`rename_tag` API 在，入口留到页头「更多」，CER-24）。

## 3. 验收场景与证据

| 合同要求 | 证据 |
| --- | --- |
| 每个 scope 的边界单测 | `note.rs`：`inbox_is_a_strict_subset_of_all`、`archived_cards_leave_every_scope_except_archive`、`untagged_includes_attached_cards`、`new_cards_start_unarchived` |
| 计数一致性（侧栏 = 列表 = 查询） | `note_library/tests.rs`：`sidebar_counts_are_one_source_of_truth`、`list_rows_match_the_sidebar_count_when_unfiltered`、`archived_scope_shows_only_archived`、`scope_switch_keeps_filters_visible` |
| AND 查询 + 子标签命中 | `note.rs::tag_filter_is_and_and_reaches_descendants`；`note_library/tests.rs::tag_click_is_single_select_and_ctrl_click_is_and` |
| 排序全序且稳定 | `note.rs::sorts_are_total_and_stable` |
| 标签树两级 + 只数未归档 | `note.rs::tag_tree_caps_at_two_levels_and_counts_unarchived_only` |
| 旧库 roundtrip / 无迁移 | `note_store.rs`：`archiving_keeps_updated_at_and_survives_reopen`、`reopening_uses_the_index_and_rebuild_survives_bad_files`、`a_newer_library_card_is_not_overwritten_by_an_older_document_copy` |
| 批量整批报告 | `note_store.rs::batch_reports_every_id_including_failures` |
| 归档视图长什么样 | 基线 `library-archive`（`shots/golden/library-archive/shell-ppp1.50.png`）：归档行选中、4 张、预览同源 |

## 4. 未做 / 风险

- 「收件箱」语义变严（挂了节点的卡不再进收件箱）会改变老用户的计数预期；
  已在 `docs/32` §14 与 `docs/28` §7.1 写明，无数据迁移。
- 归档入口目前在选择模式批量条与行菜单；页头「更多」里的库级管理留到 CER-24 的菜单收口。
