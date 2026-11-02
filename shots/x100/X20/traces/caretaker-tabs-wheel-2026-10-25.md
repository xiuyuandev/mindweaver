# X20 门后维护 · 标签条滚轮映射去重（2026-10-25）

> 范围：Stage B 修复，不改变 X20 门结论，不越过人工门，不开始 Stage C。
> 触发：看护代理接手时心跳超时（05:20 → 05:35），按 `docs/24`
> 「X20 门后维护 · 标签条滚轮映射去重验收口径」执行。

## 1. 缺陷

2.5 的滚轮横向映射写在了 `for (i, (title, dirty)) in labels` 循环体内：

```rust
let wheel = ui.input(|inp| inp.smooth_scroll_delta.y);
if wheel.abs() > 0.5 {
    ui.scroll_with_delta(egui::vec2(wheel, 0.0));
}
```

`Ui::scroll_with_delta` 只是把增量累进本帧 pass state（egui 0.37
`ui.rs:1616-1626`），`ScrollArea` 在本帧 `end` 时一次性取走
（`scroll_area.rs:1201-1209`）。因此同一帧的滚轮增量被**每个标签各应用一次**：

- 9 个标签 → 一次滚轮 = 9 倍位移，直接夹到最右端；
- 6/8 个标签 → 6/8 倍，滚动速度随标签数变化；
- 命中判定用内容 `ui.max_rect()`（随内容变宽），不是 600px 滚动视口。

## 2. 修复

`crates/mw-shell/src/tab_strip.rs`

- 新增纯函数 `wheel_scroll_x(events)`：把一帧内的
  `(dx, dy, usable)` 事件合成一个横向位移；`usable=false`（Ctrl/Alt/Win
  组合）丢弃；`|sum| <= 0.5` 视为抖动。新增单测 1 条（模块共 11 条）。

`crates/mw-shell/src/app_methods_2.rs`

- 映射移到标签循环外，每帧最多一次；直接读本帧 `Event::MouseWheel` 的
  `dx + dy`，不再经过 `smooth_scroll_delta`（它按真实帧间隔摊到后续帧，
  离屏回放依赖机器速度）。
- 命中区改为 `scroll_rect`（滚动视口，右端 40px 加号区除外）。
- `scroll_with_delta_animation(..., ScrollAnimation::none())`：与 X19 揭示
  同口径，滚轮没有惯性尾巴，截图可复现。
- 保留 `mouse_wheel: false`（避免 egui 先吃掉事件）与 `interactive` 门
  （菜单/重排拖拽期间不响应）。
- `MW_TRACE` 新增 `tab_scroll x= content= view=`：横向偏移每次变化记一笔。

`crates/mw-shell/src/main.rs` / `app_methods_1.rs`：新增 `tab_scroll_x: f32`
（仅 trace 用，初始 0）。

## 3. 修复前证据

脚本 `shots/golden/inputs/tabs-wheel.txt`（`wait 4` / `hover 300 65` /
`scroll 0 -100` / `wait 12`），9 标签、640×460、ppp 1.5、warmup 16。

- trace `caretaker-tabs-wheel-before-2026-10-25.trace`：`tab_wheel` **108 行**
  = 9 标签 × 12 帧，同一帧 9 次；
- 实拍 `caretaker-tabs-wheel-before-2026-10-25.png`：标签条已冲到最右端
  （首标签只剩尾部残缺），左渐隐满强度。

## 4. 修复后证据

- trace `caretaker-tabs-wheel-9-2026-10-25.trace`：`tab_wheel` **1 行**
  （`dx=-100.00`），下一帧 `tab_scroll x=100.0 content=1006.0 view=600.0`。
- trace `caretaker-tabs-wheel-8-2026-10-25.trace`：同一脚本、8 标签，
  `tab_scroll x=100.0 content=896.0 view=600.0` —— 位移与标签数无关。
- 实拍 `caretaker-tabs-wheel-after-2026-10-25.png`：滚动 100px，左侧露出
  上一标签的 20px 渐隐，右侧仍有隐藏内容（右渐隐），加号位置不动。
- 修复前后 960×690 差异 **14,328 px = 2.163%**，包围盒
  `x[1..900] y[77..119]`（物理像素），全部落在标签条，画布/标题栏/状态栏 0 差异。

## 5. 回归

- 新增第 27 组 golden `tabs-wheel`：960×690 / 111,542 B /
  SHA256 `d5ef9b4a…0261b910`，双跑 `--tol 0` 0 差异。
- `tabs`（`F11988FD…AD2553F`）、`tabs-overflow`（`96D62007…48300A90`）
  逐字节未变；完整机器门 **27 组 × 2 = 54/54** 全 0 差异。
- `tabs-overflow` 复跑 trace 仍只有第 5 次切页
  `tab_reveal active=5 tab=558.0..666.0 view=0.0..600.0`，随后
  `tab_scroll x=312.0`（即时居中，无动画跳动）。
- 门槛：fmt 干净；mw-shell clippy 仅 `export_pdf.rs` 3 条历史、新增 0；
  **756 tests / 0 failed**；release `23,458,816 B` /
  SHA256 `6B23D098…79901116`；结论
  `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`
  （finished 2026-10-25 05:46:09 +08:00）。
- 日志：`caretaker-stage-b-gate-tabs-wheel-2026-10-25.log`。

## 6. 未做

- 滚轮只拍了 ppp 1.5；ppp 2.0 的溢出 + 滚轮组合未实拍。
- 触控板横滑用 `scroll dx` 事件脚本覆盖了映射逻辑，但没有真实触控板采样。
- 触摸板惯性/系统精确滚动被刻意放弃（标签条用即时滚动）；若后续要惯性，
  需另设可复现的帧时钟。
- 滑块/菜单/开始页的滚动行为未改。
