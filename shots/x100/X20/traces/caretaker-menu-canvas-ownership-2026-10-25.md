# X20 门后维护 ⑩ · 标题栏菜单下画布点击归属（2026-10-25）

> 结论：**已实现 · 31 组 golden 双跑 62/62 无差异 · 机器门 PASS**。
> 仍为 Stage B 门后维护，不改变 X20 门结论，人工 X20 验收仍待确认。

## 缺口

右键菜单/标签菜单走 `overlay_state().context`，已进 `input_blocked`；标题栏菜单
（文件/编辑/视图/插入/帮助/更多）是 egui Memory 型弹层，不进 overlay 状态，
`input_blocked` 也没看它。1080×720、ppp 1.5 实拍：

1. `click 115 27` 打开「文件」菜单（对照图 `...-menu-open-2026-10-25.png` 证明菜单已展开）；
2. `click 605 365` 点「织思」：菜单收起**的同时**节点被选中；
3. `move 100 600` 移开指针后选中仍在。

修复前 trace：`canvas_click blocked=false swallowed=false overlay=false`；
修复前状态栏「已选：织思」、节点选中环、画布上方节点工具条。

## 修复

`crates/mw-shell/src/main.rs`：

```rust
let menu_popup = egui::Popup::is_any_open(&ctx);
let input_blocked = demo
    || self.dialog_demo
    || overlay_now.blocks_canvas()
    || self.tab_drag.is_some()
    || menu_popup;
```

读的是上一帧 Memory 里的弹层状态，而画布在本帧稍后才绘制——收起菜单的那一下
正好被挡住，下一帧弹层已关，不会误伤后续点击。tooltip 不写 Memory 弹层，不受影响。
门一开 `handle_pointer` 清掉平移/框选/拖节点/连线/跨度拖拽，`shortcuts` 也提前返回。

## 验收

- 新脚本 `shots/golden/inputs/menu-canvas-click.txt`（24 帧，warmup 24）：
  `wait 3 → click 115 27 → wait 4 → click 605 365 → wait 2 → move 100 600 → wait 8`。
- 修复后 trace：`canvas_click blocked=true swallowed=false overlay=false`；
  菜单收起、节点无选中环/工具条、状态栏无「已选」。
- 前后差异：29,917 px = **1.710%**，最大通道差 **205**，
  `bbox=(267, 87, 1022, 1071)`（画布工具条 + 节点环 + 状态栏）。
- 新增第 31 组 golden `menu-canvas-click`（1620×1080 / 175,220 B /
  SHA256 `811C6D36…9CF925`），隔离 APPDATA 双跑 `--tol 0` 0 差异；
  既有 30 组逐字节不变。
- 机器门（`scripts/x100-stage-b-gate.ps1`）：fmt 干净、mw-shell clippy 仍仅
  `export_pdf.rs` 3 条历史告警（新增 0）、**761 passed / 0 failed**、
  31 组 golden × 2 轮 = 62/62 0 差异、`MACHINE GATE PASS`。
- release `23,487,488 B` / SHA256 `3EA8B70F…E72240`。
- 证据：本目录 `caretaker-menu-canvas-ownership-{before,after,menu-open}-2026-10-25.*`、
  `caretaker-stage-b-gate-menu-canvas-2026-10-25.log`。

## 已知缺口

- 键盘方向键同被该门挡住（`shortcuts` 提前返回），但没有单独的键盘回放脚本；
  本轮只锁了指针路径。
- 其他 Memory 弹层（大纲菜单、更多菜单）共用同一条门，未逐个拍前后对照。
- 真实菜单入场/收起动画仍未量化（沿用 X20 已记缺口）。
