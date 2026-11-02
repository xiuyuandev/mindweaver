# CER-26 · 预览阅读面（复用卡片页阅读组件）

日期：2026-11-02　状态：**已实现**（机器证据齐；G-B2-3 人工门未开）
合同：`docs/29` §8 CER-26；设计：`docs/28` §7.5。

## 1. 范围

- **版心 600**：`content::PREVIEW` = `page_max 680 / text_max 600 / pad_top 40 / pad_bottom 64 /
  narrow_side 32`；`column()` 按可用宽算 `side/inset`，窄栏时两侧余量收缩而不是把字挤小。
- **不另立字阶**：标题 `content::PAGE_TITLE`、元信息 `content::META`（12/18）、
  标签行 `content::LIST_EXCERPT`、正文 `content::READING_BODY`（16/28）。
  `docs/28` §7.5 写的「标题 30/40、标签 12.5/19」是 TYPO-01（第 65 轮）之前的旧档，
  落地是 `PAGE_TITLE` 30/42/600 与 `LIST_EXCERPT` 13/19（整数档），差异记在 CER-30 核对表。
- **正文 = 卡片页同一棵 AST**：`reading_job(text, palette, width)`
  （`note_view.rs` 的共享入口，`MarkerMode::OnlyLine(None)` 把标记全藏），
  所以待办完成态变淡、引用/代码/强调的画法与卡片页逐像素同源；
  调色板走 `MdPalette::from_content`，颜色只从内容令牌取。
- **不可编辑**：正文与标题都是 `painter.galley` 手摆，不塞只读 `TextEdit`——
  `TextEdit` 会注册焦点，`typing = memory.focus().is_some()` 一成立，
  `↑↓ / J/K / Enter` 全被当打字吞掉（`preview.rs` 头注释记了这个坑）。
- **入口只有两个**（§7.9 点名不要「打开编辑」大按钮）：悬停/键盘焦点时右上角 28px 铅笔
  （`content_kit::PAGE_ICON_BUTTON`，有单测钉 28），以及双击标题或正文；键盘是 `Enter`。
- **两栏/单栏 = 整页阅读面**：`preview_wanted`（请求位）与分档每帧合成 `preview_page`，
  拖窗跨 1200 不用重点卡片；页头返回钮在整页态改叫「返回列表」，`Esc` 阶梯同一步。
- **滚动独立**：`preview_scroll` 与 `list_scroll` 分开存；切卡片回顶（`set_focus` 里清零），
  从整页返回列表时列表滚动位置不变。
- **空预览**：左对齐一句 `EMPTY_PREVIEW`（「选择一张卡片查看内容」），`LIST_EXCERPT` +
  `text.faint`，不画大图标（§7.5）。
- **来源行**：能跳就是文字动作（`library_jump_source`）；跨文档查不到名字时只报
  「来源：另一份文档」，不编假文档名。

## 2. 验收场景与证据

| 合同要求 | 证据 |
| --- | --- |
| 三栏停靠预览（阅读排版） | `library-tag-filter`（snow）、`library-preview-ink/mist/sand` 四主题同环境 |
| 两栏整页阅读面 + 返回 | `library-preview`（`MW_LIBRARY_DEMO=preview`，css 1080 → 两栏整页） |
| 空预览左对齐一句话 | `library-three-pane` / `library`（default 场无焦点卡片） |
| 双击/Enter 打开卡片页、单击不打开 | `a_plain_click_opens_the_reading_page_only_outside_three_pane`、`double_click_needs_two_hits_on_the_same_id`、`double_click_window_is_400ms`、键表 `Enter → LibraryAction::Open`（轨迹 `library_open via=enter`） |
| 勾选不打开阅读面 | `ticking_a_checkbox_does_not_open_the_reading_page` |
| 不可编辑 / 不抢键盘 | `preview.rs` 无 `TextEdit`；`tab_ring_two_pane_skips_the_docked_preview`（两栏 Tab 环跳过不存在的停靠预览） |
| 滚动独立 | `preview_scroll` 与 `list_scroll` 两个字段；切卡回顶在 `set_focus` |
| 铅笔 28px | `content_kit` 单测 `assert_eq!(PAGE_ICON_BUTTON, 28.0)` |

## 3. 未做 / 风险

- 预览内**不做**文内链接跳转（`[[…]]` 只按普通文字画）：§7.5 没要求，卡片页才有。
- 长文预览的滚动条是自绘细条（`scrollbar()`），与列表同款式；没有「回到顶部」钮。
- 四主题阅读排版证据是离屏 1.5ppp 一档；1.0/2.0 的阅读面没有单独基线
  （字阶与主题令牌在 `design` 单测里与 ppp 无关，风险低）。
