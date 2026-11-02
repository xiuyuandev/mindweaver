# CER-27 · 搜索 / 筛选 / 排序

日期：2026-11-02　状态：**已实现**（机器证据齐；G-B2-3 人工门未开）
合同：`docs/29` §8 CER-27；设计：`docs/28` §7.6。

## 1. 范围

- **搜索框 32px**（`content_kit::SEARCH_H`，有单测钉 32）+ 下方 28px 排序行（`SORT_ROW_H`）；
  `Ctrl+F` 聚焦搜索框（轨迹 `library_search_focus`），框内输入不被页面键拦截。
- **120ms 去抖**（`DEBOUNCE`）：去抖期间 `request_repaint_after(DEBOUNCE)` 自己续帧，
  结算读**帧计数虚拟时钟**（M16 的教训：读墙钟会让「打了字还没到 120ms」在离屏里漂）。
- **搜索范围**：标题 / 正文 / 标签 / 来源节点标题。节点文字只有壳层喂得进来，
  所以 `poll_search_frame` 把 `card_sources()` 的 `node_text` 收成一张 id→文字表传进
  `NoteStore::library_hits`；匹配方式固定 `mw_core::MatchMode::Pinyin`（全拼 + 首字母）。
- **命中高亮 12% 强调底**（`HIT_ALPHA = 0.12`）：区间是**字符下标**半开区间
  （`match_ranges`，大小写不敏感、CJK 不切字）；拼音命中在原文里没有字面对应串，
  宁可不高亮也不在用户没打的地方画颜色（`list.rs` 注释记了这条口径）。
- **标签 AND**：侧栏点选即 AND 收窄，已选筛选在列表顶画纯文字 chip（中性 6% 底、
  逐个可删）+「清除筛选」文字动作；轨迹 `library_tag_filter` / `library_clear_filters`。
- **排序三档**：`CardSort::{UpdatedDesc, CreatedDesc, Title}` = 最近更新 / 最近创建 / 按标题，
  排序行走菜单，轨迹 `library_sort`；搜索时相关度优先压过排序（`searching_puts_relevance_first`）。
- **两个计数**：页头 = 当前范围多少张（`scope_count`），列表顶 = 过滤后剩多少张；
  两数不同正好说明「筛选收窄了多少」。
- **无结果**：左对齐一句「没有匹配的卡片」+「清除筛选」文字动作，不画插画（§7.6）；
  基线 `library-no-results`。
- **Esc 逐级**：先清搜索词（`step=clear_search`）→ 再交还搜索框焦点（`step=blur_search`）→
  退选择 → 关整页预览 → 关抽屉 → 关库（CER-29 的整张阶梯）。

## 2. 验收场景与证据

| 合同要求 | 证据 |
| --- | --- |
| 无结果 + 清除筛选 | `library-no-results`（查询串故意零命中） |
| 标签 AND + chip + 命中高亮 | `library-tag-filter`（知识 单选 + chip + 6 张 + 摘要内 12% 底） |
| 字符下标 / CJK / 大小写 | `match_ranges_is_case_insensitive_and_char_indexed`、`match_ranges_handles_cjk_without_splitting_chars`、`match_ranges_empty_cases` |
| 拼音与来源节点命中 | `note_store::tests::full_text_search_finds_body_snippet_and_pinyin`、`note_store::tests::library_search_covers_tags_and_source_node_titles` |
| 排序与相关度 | `sort_default_is_updated_desc`、`searching_puts_relevance_first`、`note::tests::sorts_are_total_and_stable` |
| 去抖与续帧 | `poll_search_frame`（`DEBOUNCE` + 虚拟时钟）；轨迹 `library_search q=… hits=… mode=pinyin` |
| 10k 上界 | `note_store::tests::search_thousand_cards_is_bounded` |

## 3. 未做 / 风险

- 搜索**不做**模糊/错别字容错，也不做正则；`MatchMode` 只有字面与拼音两档。
- 命中高亮只在列表的标题/摘要行上画，预览正文里不画命中底（§7.6 只要求列表）。
- 去抖 120ms 是固定值，没有按库大小自适应（10k 场检索本身有上界单测，暂不需要）。
