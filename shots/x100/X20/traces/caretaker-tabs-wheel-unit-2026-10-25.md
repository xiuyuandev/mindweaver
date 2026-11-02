# X20 门后维护 · 标签条滚轮单位归一（2026-10-25）

> 范围：Stage B 修复，不改变 X20 门结论，不越过人工门，不开始 Stage C。
> 触发：看护代理接手时心跳停在 05:48（>15 分钟），按 `docs/24`
> 「X20 门后维护 · 标签条滚轮单位归一验收口径」执行。

## 1. 缺陷

2.6 把 `Event::MouseWheel` 的 `delta` 直接相加，忽略了 `unit`：

```rust
egui::Event::MouseWheel { delta, modifiers, .. } => Some((delta.x, delta.y, usable))
```

egui-winit 对普通鼠标上报 `MouseWheelUnit::Line`（一格 ±1），对触控板上报
`Point`（已换算成点）。egui 自己的 `ScrollArea` 会把 `Line` 乘以
`input_options.line_scroll_speed`（egui 0.37 原生端 40，见
`egui/src/input_state/wheel_state.rs:145`），但标签条设置了
`mouse_wheel: false`，没有原生路径兜底——于是真实鼠标滚一格只移动 **1px**，
手感等同失灵。2.6 的 golden 脚本用 `scroll`（Point），所以基线完全看不出
这个问题；只有真实硬件或 Line 单位脚本能暴露。

## 2. 修复

`crates/mw-shell/src/tab_strip.rs`

- `wheel_scroll_x` 改为接收 `(Vec2, MouseWheelUnit, bool)`：
  `Point` 原样、`Line × line_scroll_speed`、`Page × 视口高`，与 egui
  `ScrollArea` 同口径；换算后再做 0.5px 抖动过滤。
- 新增 `wheel_scroll_normalizes_line_and_page_units_before_the_jitter_filter`
  单测（模块共 12 条）。

`crates/mw-shell/src/app_methods_2.rs`

- 调用点读 `ctx.options().input_options.line_scroll_speed`（跟随 egui/用户
  配置，不硬编码 40）与 `viewport_rect().height()`（Page 口径）。
- 命中区、修饰键过滤、每帧一次、`ScrollAnimation::none()`、`dx + dy` 合并
  全部保持不变。

`crates/mw-shell/src/golden_input.rs`

- 新增 `scroll_lines dx dy` 原语，注入 `MouseWheelUnit::Line`；`scroll` 仍发
  `Point`（触控板语义）。带解析单测，保证两条硬件路径都有可复现脚本。
- 新脚本 `shots/golden/inputs/tabs-wheel-lines.txt`。

## 3. 修复前证据

脚本 `tabs-wheel-lines.txt`（`wait 4` / `hover 300 65` /
`scroll_lines 0 -3` / `wait 12`），9 标签、640×460、ppp 1.5、warmup 16。

- trace `caretaker-tabs-wheel-unit-before-2026-10-25.trace`：
  `tab_wheel dx=-3.00`，下一帧 `tab_scroll x=3.0 content=1006.0 view=600.0`
  ——3 格只走了 3px。
- 实拍 `caretaker-tabs-wheel-unit-before-2026-10-25.png`：
  960×690 / 113,614 B / SHA256 `F551E86D…6978BFF`，标签条几乎没动。

## 4. 修复后证据

- trace `caretaker-tabs-wheel-unit-after-2026-10-25.trace`：
  `tab_wheel dx=-120.00`，下一帧
  `tab_scroll x=120.0 content=1006.0 view=600.0` —— 3 格 = 3 × 40px。
- 实拍 `caretaker-tabs-wheel-unit-after-2026-10-25.png`：
  960×690 / 111,498 B / SHA256 `03E71072…75091E`。
- 修复前后 960×690 差异 **12,691 px = 1.916%**，最大差 225，包围盒
  `x[2..894] y[72..122]`，全部落在标签条；画布、标题栏、状态栏 0 差异。

## 5. 回归

- 新增第 28 组 golden `tabs-wheel-line`：960×690 / 111,498 B /
  SHA256 `03E71072…75091E`，双跑 `--tol 0` 0 差异；脚本同时挂进
  `scripts/ci.ps1` 与 `scripts/x100-stage-b-gate.ps1`。
- `tabs-wheel`（Point，100px）与 `tabs-overflow` 复跑 `--tol 0` 0 差异，
  逐字节未变；`tabs` / 其余基线不受影响。
- 单测：`tab_strip` 12 条（含 Point/Line/Page/换算后抖动）、
  `golden_input` 6 条（含 Line 原语）；全量 **758 tests / 0 failed**。
- 门槛复跑记录见第 6 节。

## 6. 机器门复跑

- 2026-10-25 06:13–06:15 完整阶段门：X11–X19 证据包 PASS、全部
  `x100-*.ps1` 语法 PASS、fmt 干净、mw-shell clippy 仅 `export_pdf.rs` 3 条
  历史、**758 tests / 0 failed**、28 组 × 2 = **56/56** 全 0 差异；结论行
  `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`
  （finished 2026-10-25 06:15:06 +08:00）。
- release 重建：`23,439,872 B` / SHA256 `81C2A523…FFEDAE`。
- `scripts/ci.ps1` 语法校验通过，新场景已接入；X20 manifest 补
  `tabs-wheel-unit-normalization` 场景。
- 日志：`caretaker-stage-b-gate-tabs-wheel-unit-2026-10-25.log`。

## 7. 未做

- ppp 2.0 下的 Line 滚轮未实拍；Page 单位只有单测，没有硬件来源。
- 没有真实鼠标/触控板采样，`scroll_lines` 是协议级注入。
- 未改菜单/开始页的滚动行为（它们走 egui 原生 `ScrollArea`，单位换算本来
  就正确）。
