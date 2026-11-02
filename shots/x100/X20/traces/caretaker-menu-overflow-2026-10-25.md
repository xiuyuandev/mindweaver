# 门后维护：窄窗右键菜单溢出（2026-10-25）

X20 人工门仍未确认；本轮只修 Stage B 自身缺陷，不进入 Stage C。

## 现象

`docs/24-X100升级路线.md` X16 的验收口径是「任何一级都不得出内容区」。
X20 报告「Known gaps」也记了这条尾巴：640×460 下只实拍了应用菜单，
画布右键菜单的边缘翻转没有实拍。

实拍后确认是真缺陷：节点右键菜单 19 项自然高 477（14 个动作 30 +
5 条分隔 9 + 内边距 12），640×460 窗口内容区只有 460，卡片按旧逻辑
只做 x 轴与 y 轴翻转，不做高度约束：

- 卡片从 y=8 开始，477 高 → 底部到 y=485，超出窗口 25px；
- 末两项「折叠」「聚焦」被窗口下沿裁掉，鼠标与键盘都到不了；
- 换到窗口上沿（翻转）也只是把溢出挪到顶部，问题不消失。

修复前实拍：`caretaker-menu-overflow-before-2026-10-25.png`
（SHA256 `5fa9f667565b0c6b8d6114f535d9f596575051b2c2ac0c48d908155b34d5ef2f`），
可见「折叠」只剩半行、「聚焦」完全在窗口外。

## 修复

`crates/mw-shell/src/menu.rs`：

- `card_height(natural_h, content)`：卡片高度夹到
  `content.height() - EDGE * 2`（EDGE=8），根菜单与子菜单同一口径；
- 卡片装不下时，层级内容改为 `ScrollArea`：
  - 原生滚动条 `AlwaysHidden`，`scroll_bar: false`，保留滚轮；
  - 自绘 4px 静止滑块（alpha 130、最短 28px、右侧留 4px），
    与开始页 2026-10-25 门后维护的滑块同一视觉档；
  - 不溢出的菜单不走 `ScrollArea`，保持原绘制路径，像素零漂移；
- 键盘导航（↑/↓/Home/End/进入子菜单）置 `reveal` 标记，
  当前高亮行不在视口内时 `scroll_to_rect(Align::Center)`；
  demo 打开菜单不算键盘导航，不会自动滚动。

修复后几何（640×460，ppp 1.5）：

| 量 | 值 |
| --- | --- |
| 自然内容高 | 477 |
| 卡片高（夹后） | 444（= 460 − 8×2） |
| 滚动视口 | 432（= 444 − 6×2） |
| 可滚动内容 | 465（= 477 − 6×2） |
| 溢出 | 33 |
| 滑块长度 | 432 / 465 × 432 ≈ 401 |

## 验证

- 单测（`menu` 模块 14 条）新增 3 条：
  `card_height_clamps_to_the_content_rect`、
  `overflow_menu_stays_inside_the_content_rect`、
  `scroll_thumb_only_appears_when_overflowing`；`cargo test -p mw-shell menu`
  14 passed。
- 新基线 `shots/golden/menu-narrow/shell-ppp1.50.png`（960×690，
  SHA256 `32b20b468775107eb7bb11c3d3aa9c3b6898c677a157d3ae3f467dee4f255c75`），
  `--tol 0 --warmup 8` 连跑两次 0 差异。
- 非溢出路径回归：`shots/golden/menu`（1620×1080）修复前后逐字节一致
  （SHA256 `fbfaf8ceea11f074234f0cc8c4f26fa12fc9a63f6568ff7876c5e9ccc3342610`），
  `shots/golden` base 三档 ppp 也逐字节一致。
- 键盘到达末项：`inputs/menu-narrow-end.txt`（wait 4 / Left / wait 2 /
  End / wait 6）重放得到
  `caretaker-menu-overflow-end-2026-10-25.png`
  （SHA256 `ac13bdb0c65b3c8ce7b3b4377599989722570a9958e57019b09fea4b1c91a440`），
  末项「聚焦」带高亮且完整落在卡片内，滑块贴底。
  trace `caretaker-menu-overflow-end-2026-10-25.trace` 含
  `event menu_submenu_close key=left level=1` 与
  `event menu_nav key=end label=聚焦`。
- 阶段门（`scripts/x100-stage-b-gate.ps1`，2026-10-25 04:46:38–04:47:50
  +08:00）：**25 组 × 2 = 50/50 golden 全 0 差异**、
  **753 tests / 0 failed**、fmt 干净、mw-shell clippy 仅 `export_pdf.rs`
  3 条历史、release `23,505,408 B` / SHA256
  `3b6394be…e8e5a47`；结论行仍为
  `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`。日志：
  `caretaker-stage-b-gate-menu-overflow-2026-10-25.log`。

## 未做 / 边界

- 滑块本轮是**静止指示 + 键盘/滚轮滚动**，不做拖拽命中；
  开始页滑块可拖拽，两者交互层级不同，菜单拖拽留到后续轮次统一
  （不在 X20 门内扩大改动）。
- 未实拍 200% DPI 下的菜单溢出；几何全部用逻辑像素，滑块宽度/最短长度
  不随 ppp 变化，属于有意选择，待 Stage C 视觉轮复核。
- 未改 `.zmind`、未新增依赖/crate/线程、未改文档写路径。
