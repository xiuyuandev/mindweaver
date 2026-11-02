# 门后维护 ⑬ · 开始页标题栏菜单下的裸键归属（2026-10-25）

> 结果：**已实现 · 待人工随 X20 一并验收**。这是 X17「同一时刻只有一个
> 指针/键盘主人」在开始页上留的缺口，属于 Stage B 修复，不是新一轮，
> 不改变 X20 门结论，也不越过人工门。

## 缺口

开始页（`MW_HOME=1`）盖着文档时，标题栏菜单仍是 egui Memory 弹层，
不进 `overlay` 状态；`window_shortcuts` 里开始页那段裸键（Esc / Enter /
1–5）没有看它。1440×1000 目标窗口、ppp 1.0、`MW_START_DEMO=1` 实拍：

- 打开「文件」菜单后按 `1`：开始页在菜单背后被换成了「思维导图」模板
  文档（状态栏「已新建『思维导图』」），菜单仍开着。
- 同样的状态按 `Esc`：`window_shortcuts` 先把 Esc 消费掉（`home_return`
  为假时不动作），菜单收不到 Esc，屏幕与「菜单展开」对照图逐字节一致。

trace 证据：修复前输入 `key 1` 后焦点从「开始页 · 新建空白图」变成
`canvas`；`Esc` 没有 `popup_menu key=escape`。这与 2.9/2.10/2.11 定下的
「浮层是第一键盘主人」冲突。

## 规则

- 开始页的裸键只在没有前台弹层时归开始页：`home_bare_keys_owned(menu_open,
  help_open)`——标题栏/下拉 Memory 弹层或快捷键表开着时整段跳过。
- F1 等应用级快捷键仍在前面处理；菜单自己的 Esc 由 `PopupNav` 收，
  其它键按既有规则收起菜单。
- 不改菜单布局/文案/动作；无弹层时开始页行为与 X20 基线逐像素一致。

## 验收

- 纯函数单测 `start_page_bare_keys_yield_to_foreground_layers`：菜单/速查
  表任一在前台都不归开始页。
- 新脚本 `shots/golden/inputs/start-menu-open.txt`（开菜单对照）、
  `start-menu-input.txt`（菜单 + `1`）、`start-menu-esc.txt`（菜单 + Esc），
  均为 `wait 8 → click 114 27 → wait 6/8`。
- 修复前：`key 1` 后焦点 `canvas`、开始页被模板文档替换（122,992 B）；
  Esc 后菜单仍在（202,498 B，与对照图同尺寸）。
- 修复后：`key 1` 后仍是开始页、菜单收起、无文档事件（169,377 B）；
  Esc 后出现 `popup_menu key=escape`、菜单收起（169,377 B）。对照图
  202,498 B 证明点击确实打开了菜单。
- 阶段门：31 组 × 2 = 62/62 全 0 差异、765 tests / 0 failed、fmt 干净、
  mw-shell clippy 仅 3 条历史；基线未被改写。

## 证据文件

- `caretaker-start-menu-context-2026-10-25.png`（菜单展开对照）
- `caretaker-start-menu-before-2026-10-25.png/.trace`（修复前按 `1`）
- `caretaker-start-menu-after-2026-10-25.png/.trace`（修复后按 `1`）
- `caretaker-start-menu-esc-before/after-2026-10-25.png`、
  `caretaker-start-menu-esc-after-2026-10-25.trace`
- `caretaker-stage-b-gate-start-menu-library-2026-10-25.log`
- 脚本：`shots/golden/inputs/start-menu-{open,input,esc}.txt`
