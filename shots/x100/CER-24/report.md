# CER-24 + CER-25 · 侧栏导航与标签树 / 列表行与虚拟化

日期：2026-11-02　状态：**已实现**（机器证据齐；G-B2-3 人工门未开）
合同：`docs/29` §8 CER-24、CER-25；设计：`docs/28` §7.3（侧栏）、§7.4（列表）。

## 1. 范围（CER-24 侧栏）

- 导航四行：全部卡片 / 收件箱 / 未打标签 / 归档，30px 行、16 图标 + `NAV` 14/21 文本 +
  `TAG` 12/18/600 计数右对齐；选中 = `nav_selected` 4% 底 + 左侧 2px 强调条；hover 4%。
  （`docs/28` §7.3 写的「文本 13 / 计数 11.5」是 TYPO-01（第 65 轮）之前的旧档；
  落地按 `design::content` 整数档，差异记在 CER-30 核对表。）
- 标签树：6px 圆点（颜色按标签路径在六色环上稳定取色，`tag_dot_index_is_stable_within_six`）、
  最多两级、chevron 展开记忆（默认全展：`tag_tree_starts_fully_expanded`）；
  更深的路径只在列表标签行与搜索里出现（§7.3）。
- 计数单一真相：侧栏与列表都从 `ScopeCounts` / `CardQuery` 取，
  `sidebar_counts_are_one_source_of_truth` 钉住「筛选时侧栏计数不跟着变」。
- 点标签 = 单选，`Ctrl+click` = AND 多选；再点已选中的唯一标签 = 清除
  （`clicking_the_only_tag_again_clears_it`）；已选筛选在列表顶显示纯文字 chip + 清除筛选。
- 管理标签不进侧栏底部：入口在页头「更多」（轨迹 `library_tag_filter` / `library_scope`）。

## 2. 范围（CER-25 列表）

- 行高 60、内边距 12×16：标题 14.5/22/500 一行省略、摘要 12.5/19 一行省略、
  元信息 11.5/16 = 至多 1 个标签（6px 圆点 + 纯文字）+ 日期，超出用 `+n`；
  无卡片底、无圆角、无阴影；行间 1px `divider` 左右内缩 16。
- 默认不显示复选框（选择模式才出现，CER-28）；hover 4% 中性底、选中 7% 强调底、
  键盘焦点 1.5px 环。
- 摘要走 `NoteCard::excerpt`：待办框 `[ ] / [x] / [X]` 的方括号不进摘要
  （`excerpt_drops_task_list_checkboxes`）。
- 虚拟化：`ScrollArea` 只排可见行 + 上下余量，行高恒定 60 所以偏移即下标；
  10k 卡片的首屏证据是 `library-long`（10000 张，侧栏计数 10000、标签树 乙/甲 各 5000）
  在 warmup 3 帧内画完且双跑零差异；`search_thousand_cards_is_bounded` 钉检索上界。
- 滚动位置在 `NoteLibrary::list_scroll`：切筛选回顶（`list.rs` 的 `lib.list_scroll = 0.0`），
  从阅读面返回保留列表滚动（§7.5）。

## 3. 验收场景与证据

| 合同要求 | 证据 |
| --- | --- |
| 导航/标签树视觉 | `library-tag-filter`（知识 选中 + chip + 6 张）、`library-archive`（归档 4 张） |
| 10k 虚拟化 | `library-long`（10000 张）+ `note_store::tests::search_thousand_cards_is_bounded`、`one_thousand_cards_rebuild_without_losing_order_or_content` |
| 行 bbox / 省略 | 行高常量 60 由 `content::LIST_*` 字阶钉住；省略在基线里可见（摘要行尾 `…`） |
| 计数/层级测试 | 见上 §1/§2 单测清单 |
| 无刷新 | 侧栏/列表同帧重画，状态在 `NoteLibrary`；轨迹 `library_scope`、`library_tag_filter` |

## 4. 未做 / 风险

- 10k **滚动 ≥55fps** 没有实机帧率证据（离屏无 vsync）：只证明每帧只排可见行
  （虚拟化窗口与 N 无关）与 10k 首屏在 harness 超时内。实机帧率留人工门判。
- 标签圆点六色环在 ink 主题下饱和度偏低（基线可见），属主题令牌问题，记在 CER-30 核对表。
