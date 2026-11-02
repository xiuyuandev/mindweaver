# CER-25 · 列表行与虚拟化

日期：2026-11-02　状态：**已实现**（机器证据齐；G-B2-3 人工门未开）
合同：`docs/29` §8 CER-25；设计：`docs/28` §7.4（列表）、§7.5（滚动与返回）。
本报告与 [`../CER-24/report.md`](../CER-24/report.md) 是同一轮实现的两个包，
侧栏部分（导航/标签树）的证据在 CER-24，这里只写列表。

## 1. 范围

- 行高 60、内边距 12×16：标题 `LIST_TITLE` 15/22/600 一行省略、摘要 `LIST_EXCERPT` 13/19 一行省略、
  元信息 `LIST_META` 12/18 = 至多 1 个标签（6px 圆点 + 纯文字）+ 日期，超出用 `+n`。
  （`docs/28` §7.4 写的 14.5/22/500、12.5/19、11.5/16 是 TYPO-01（第 65 轮）之前的旧档；
  落地按 `design::content` 整数档，差异记在 CER-30 核对表。）
- 无卡片底、无圆角、无阴影；行间 1px `divider`，左右内缩 16。
- 默认不显示复选框（选择模式才出现，CER-28）；hover 4% 中性底、选中 7% 强调底、
  键盘焦点 1.5px 环。
- 摘要走 `NoteCard::excerpt`：待办框 `[ ] / [x] / [X]` 的方括号不进摘要，
  Markdown 记号剥掉后仍单行。
- 虚拟化：`ScrollArea` 只排可见行 + 上下余量；行高恒定 60，所以滚动偏移即下标，
  不需要测量。10k 卡片首屏证据 = `library-long`（10000 张，侧栏计数 10000、
  标签树 甲/乙 各 5000），warmup 3 帧画完、双跑零差异。
- 光标与焦点：`cursor` 随 `rows` 收缩夹取、两端停住、空库无光标无焦点；
  重排后按 id 保住焦点（`sync_keeps_focus_by_id_across_a_reorder`），
  换行后丢掉缓存正文（`sync_forgets_the_cached_body`）。
- 滚动位置在 `NoteLibrary::list_scroll`：切筛选回顶，从阅读面返回保留列表滚动（§7.5）。

## 2. 验收场景与证据

| 合同要求 | 证据 |
| --- | --- |
| 行 bbox / 字阶 / 省略 / `+n` | `library`（ppp 1.00，三栏，全部卡片 16 张）、`library-two-pane`（同一份夹具，两栏） |
| 10k 虚拟化与首屏 | `library-long` + `note_store::tests::search_thousand_cards_is_bounded`、`one_thousand_cards_rebuild_without_losing_order_or_content` |
| 摘要不含待办框 | `note::tests::excerpt_drops_task_list_checkboxes` |
| 排序默认 = 更新时间降序、搜索时相关度优先 | `sort_default_is_updated_desc`、`searching_puts_relevance_first` |
| 光标/焦点/双击 | `cursor_is_clamped_when_rows_shrink`、`move_cursor_stops_at_both_ends`、`empty_library_has_no_cursor_and_no_focus`、`double_click_needs_two_hits_on_the_same_id`、`double_click_window_is_400ms` |
| 单击只移焦点不进选择模式 | `plain_click_moves_focus_without_selecting` |

## 3. 未做 / 风险

- **10k 滚动 ≥55fps 没有实机帧率证据**：离屏 harness 无 vsync、无帧率计时。
  已证明的是「每帧只排可见行，代价与 N 无关」+「10k 首屏在 harness 超时内双跑零差异」。
  实机帧率留 G-B2-3 人工门判。
- 摘要单行省略按字宽截断，不按词边界（中文优先，符合 §7.4；英文长词会被切）。
