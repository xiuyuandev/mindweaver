# CER-28 · 多选与批量操作

日期：2026-11-02　状态：**已实现**（机器证据齐；G-B2-3 人工门未开）
合同：`docs/29` §8 CER-28；设计：`docs/28` §7.4（多选与批量条）、§7.11（选择与批量语义）。

## 1. 范围

- **进入选择模式**：`Ctrl+click` 行（勾选语义）即进入选择模式并切换该行；
  选择模式下普通单击 = 切换勾选；纯点击（无修饰键）只移焦点、不勾选、不进选择模式。
- **范围选择**：`Shift+click` 从锚点（`anchor`）选到点击行；键盘 `Shift+↑/↓` 同样走
  `select_range`，并把 `cursor_by_key` 置上以便列表把光标滚进视野。
- **全选当前筛选**：`Ctrl+A` 只取**过滤后**的行（`select_all_rows`），不是全库；
  文字框在焦点上时让给输入（`typing` 门）。
- **退出**：`Esc`（阶梯里 `step=exit_select`）或批量条「取消」；退出同时清勾选与锚点。
- **复选框只在选择模式出现**：默认行不画勾选框（§7.4），选择模式下焦点行也算勾上——
  「当前行」必定在批量作用范围内，否则会出现「看着选中了却不在批量里」。
- **44px 批量条在列表栏内**（`BATCH_BAR_H`），不横跨整窗：横跨会让「作用在谁身上」
  失去边界，三栏时还会压住预览。按钮五枚：归档（归档作用域下变「取消归档」）/
  导出 / 加标签 / 移出 / 取消；「取消」永远可点——选择模式必须有一条
  不需要先取消勾选就能退出的路。
- **危险动作二次确认**：「移出」只置 `confirm_remove`，确认弹窗走 `crate::dialog`
  （「移出本地库？」420 宽、默认动作 = 取消、副行说明不删 `.zmind` 内嵌副本），
  轨迹 `library_remove_prompt` → 确认后 `library_remove`。
- **一次批量 = 一次确认 + 一条结果提示**：`after_library_batch` 把 `BatchReport`
  收成一句 toast：`{动词} N 张`，有失败追加 `，M 张失败（第一条原因）`，
  索引没落盘追加 `；索引没写盘，下次启动自愈`；不允许静默部分成功。
- **批量 API 逐 id 报成败**：`BatchReport{changed, skipped, failed, index_lagged}`，
  `batch_reports_every_id_including_failures` 钉住「每个 id 都要有下落」。
- **选择按 id 不漂移**（§7.11）：排序/筛选/批量之后 `sync` 按 id 对齐选中集，
  卡片没了就丢掉那一项（`sync_drops_selection_for_cards_that_are_gone`）；
  导出没选时回落到当前筛选的全部（`export_falls_back_to_the_whole_filter`）。
- **加标签是行内输入**（不开模态）：`tag_input` 抢焦点，Enter 提交 / Esc 取消，
  空串报「标签不能为空」；`overlay::Modal` 是全壳层共用栈，为一个非危险动作
  加变体不值当（§7.4 只要求危险动作二次确认）。

## 2. 验收场景与证据

| 合同要求 | 证据 |
| --- | --- |
| 选择模式视觉（勾选框 + 批量条） | `library-select`（`MW_LIBRARY_DEMO=select`：焦点第 1 行 + 勾选第 4 行） |
| Ctrl+click / Shift 范围 / Ctrl+A | `ctrl_click_enters_select_mode_and_toggles`、`shift_click_selects_the_range_from_the_anchor`、`select_all_only_takes_the_filtered_rows` |
| 退出清两种状态 | `exit_select_clears_both_kinds_of_state` |
| 导出回落 | `export_falls_back_to_the_whole_filter` |
| 选中不漂移 | `sync_drops_selection_for_cards_that_are_gone` |
| 部分失败逐 id 报 | `note_store::tests::batch_reports_every_id_including_failures` + `after_library_batch` 的 toast 文案 |
| 危险确认 | `remove_dialog`（默认取消）+ 轨迹 `library_remove_prompt` / `library_remove` |

## 3. 未做 / 风险

- **卡片批量不进撤销栈**：`NoteStore` 没有撤销语义（卡片是独立文件，不是 `Document`），
  所以「一次批量一条撤销」在库内不成立；合同 §7.4 要的是「一次确认 + 一条结果提示」，
  已满足。文档侧的批量（边界/概要等）走 `Command::Batch`，与本包无关。
- 移出不可撤销：确认弹窗里已写明后果；没有回收站（§8.1 不做）。
- 批量条五枚按钮在极窄列表（320）里会挤：按钮宽度按文字量算，放不下时
  「取消」仍保证可点（它排在最后且恒启用，实测 320 宽能放下五枚）。
