# CER-29 · 键盘 / 焦点 / 模态 / IME / 空·加载·错误态

日期：2026-11-02　状态：**已实现**（机器证据齐；G-B2-3 人工门未开）
合同：`docs/29` §8 CER-29；设计：`docs/28` §7.7（空/加载/错误）、§7.8（键盘）。

## 1. 键表（§7.8）逐条落地

| 合同键 | 落地 | 证据 |
| --- | --- | --- |
| `Ctrl+F` | `focus_search`：抢搜索框焦点 + `zone = List` | 轨迹 `library_search_focus` |
| `↑/↓`、`J/K` | `move_cursor(±1)`；`Shift` 同键改走 `select_range` | `move_cursor_stops_at_both_ends`、`cursor_is_clamped_when_rows_shrink` |
| `Enter` | 打开焦点卡片页；**IME 组合态不收**（`ime_composing_now` 门） | 轨迹 `library_open … via=enter`；`ime_composing_only_for_nonempty_preedit` |
| `Space` | 选择模式 = 切换当前行勾选；非选择模式 = 预览当前（三栏本就跟随） | 键表分支 `Key::Space` |
| `Shift+↑/↓` | 从锚点范围选择并滚进视野 | `shift_click_selects_the_range_from_the_anchor`（同一条 `select_range`） |
| `Ctrl+A` | 只全选过滤后的行；文字框在焦点上时让给输入 | `select_all_only_takes_the_filtered_rows` |
| `Esc` | 六级阶梯：清搜索词 → 交还搜索框焦点 → 退选择 → 关整页预览 → 关抽屉 → 关库 | 轨迹 `library_esc step=…`（每级一条） |
| `Tab` / `Shift+Tab` | 焦点区环：三栏 侧栏→列表→预览；两栏跳过不存在的停靠预览；单栏跳过关着的抽屉 | `tab_ring_three_pane_cycles_all_three`、`tab_ring_two_pane_skips_the_docked_preview`、`tab_ring_one_pane_skips_a_closed_drawer` |
| `F6` | **沿用壳层** `Action::PaneCycle`（画布↔面板模式位）；库页打开时画布整帧不画不收键（`main.rs` 的 `if self.library.is_some() { return; }` 早退），所以 F6 在库内只翻模式位 | 见 §3 风险 |
| `Ctrl+Shift+N` | 新建卡片（走画布「为选中节点建卡」） | `LibraryAction::NewCard` |

- **单字母键只在列表握着键盘时生效**：`typing = ctx.memory(focused().is_some())` 门住
  `J/K/Space/Ctrl+A`；搜索框内输入不被页面拦截。
- **模态门**：`page_keys_owned(modal_top, dialog_demo)` 委托
  `content_keys::page_owns_bare_keys`（CER-03 起的同源门）；
  `legacy_page_gates_delegate_bit_for_bit` 钉住「旧入口与新门逐位一致」，
  `gate_matrix_covers_every_foreground_combination` 覆盖前景组合矩阵。
- **菜单先于本页收键**：`handle_keys` 早、`poll_menus_show` 晚（`menu.rs` 的调用顺序约定），
  菜单开着时 `bare = false`。
- **加标签行内输入**开着时键盘全归它：Enter 提交、Esc 取消，其余不抢。

## 2. 空 / 加载 / 错误三态（§7.7）

- **空**：左对齐两句「还没有卡片」+「在导图节点上按 Ctrl+Shift+N 新建，或从文件导入」，
  不画 56px 图标、不画居中大插画（§7.9 禁止项）；基线 `library-empty`。
- **加载**：`notes == None` 时画 **3 行 × 60px 骨架**（4% 中性底、三根长短不一的占位条），
  不转菊花；基线 `library-loading`（夹具把 `notes` 置空，骨架才拍得到）。
- **错误**：`store_errors > 0` 时页头下方一条 **32px 行内条**（`content_kit::BANNER_H`，
  有单测钉 32）+「重试」（= `LibraryAction::Reload`，与「更多」里的重读索引同一个动作），
  不弹模态；内容区高度随之重算分栏，三栏不会盖住横幅。
- 三态互斥且各有基线：`library-empty` / `library-loading` / `library-no-results`
  （后者是「有库但零命中」，与空库不是一回事）。

## 3. 未做 / 风险

- **Esc 阶梯没有单测钉顺序**：六级顺序只由代码与轨迹 `library_esc step=…` 作证；
  补一条状态机单测更稳，留作后续打磨轮。
- **F6 在库页内没有可见效果**：合同写「沿用现有」，我们没有为库页新造 F6 语义；
  库内焦点交接走 Tab 环。若人工门认为库内 F6 应有可见行为，记为缺口。
- IME 证据是**离屏 preedit 事件级**（`ime_composing` 单测 + `Enter` 门），
  实机微软拼音的组合窗行为仍未核对（与 B2-A 第 8 条同一欠账）。
