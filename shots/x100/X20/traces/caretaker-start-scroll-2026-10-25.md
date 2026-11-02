# X20 门后维护 · 开始页滚动 affordance（2026-10-25）

结论：**开始页在 640×460 / 720×620 下纵向溢出时，现在静止就有可见滚动条与
上下渐隐；滑块可拖且跟手。** 这是 `docs/x100/stage-b-report.md` §6 P1 清单
里的 Stage B 收尾，不是新一轮，不改变 X20 人工门结论，未开始 X21。

## 1. 缺口与根因

X18 的响应式只保证不横向裁剪：640×460 下内容高于视口，可以滚，但

- egui 浮动滚动条默认 `VisibleWhenNeeded` 且静止态 `dormant_handle_opacity = 0.0`，
  不交互时把手完全透明；
- 内置渐隐用 `ui.stack().bg_color()` 取背景色，而开始页的 `CentralPanel::no_frame()`
  背景透明，渐变画不出来。

用户看到的是一排被硬切的模板卡，没有任何「还能往下」的信号。

## 2. 实现

只改 `crates/mw-shell/src/start.rs`，不新增依赖/线程/crate，不改文档模型与
`.zmind`，开始页仍只发 `start::Action`。

- **纯几何**：`scroll_metrics` / `scroll_thumb` / `offset_for_thumb_top`。
  内容不高于视口（含 1px 容差）返回 `None`；滑块长度
  `viewport / content`，最短 28px；位置按 `offset / (content - viewport)`；
  反算与正算往返一致，offset 夹取到 `[0, max]`。
- **绘制**：视口右缘内缩 6px 画 4px 宽、圆角 2px 滑块，颜色 `tone.ch.dim`，
  静止 alpha 130 / 悬停 200 / 拖拽 235；轨道上下各留 4px，不压品牌头与脚注。
  渐隐 40px 向 `tone.bg` 收敛，强度 `剩余距离 / 40 × 0.9`，滚到底消失；
  `offset > 0` 时顶部对称出现。
- **命中与 Tab 顺序**：命中区在滑块基础上左右各扩 6px、上下各扩 4px；用
  `Sense::DRAG`（非 focusable），不进入 X18 的卡片/按钮焦点闭环。按下记住
  抓取点相对滑块顶的偏移，拖动按指针反算 offset，抓取点不跳；松手立即交还
  滚轮。内置滚动条改为 `AlwaysHidden` 且 `scroll_bar: false`，不出现双条。
- **确定性**：拖动结果通过 `vertical_scroll_offset` 在下一帧直接落到绝对
  offset，不使用时间动画，golden 不依赖帧时序。

## 3. 验收证据

| 项 | 结果 |
| --- | --- |
| `start`（1440×1000，不溢出） | **逐字节不变**（0 px） |
| `start-narrow` 重拍 | 131,626 B `5BE44212…11E8` → 196,898 B `2BDBDCA1…5523`；漂移 64,476 / 1,004,400 px = **6.419%**，最大通道差 176，bbox x[1..1078] y[78..893]，全部落在滑块与 40px 渐隐带 |
| 新增第 24 组 `start-scroll` | 640×460、ppp 1.5、拖滑块到底；960×690、160,515 B、`AEC0D24D…97B7A`；顶部渐隐 + 滑块贴底、底部渐隐消失 |
| 两组新基线各连跑两次 `--tol 0` | 0 差异 |
| golden 全矩阵 | **24 组 × 2 次 = 48/48** 全 0 差异 |
| 单测 | `scroll_thumb_hides_when_content_fits`、`scroll_thumb_tracks_both_ends_and_min_length`、`offset_for_thumb_top_round_trips`，开始页共 10 条通过 |
| workspace 测试 | **750 passed / 0 failed**（`--test-threads=1`） |
| clippy | `mw-shell` 仅 `export_pdf.rs` 3 条历史告警，新增 0 |
| fmt | `cargo fmt --all -- --check` 干净 |
| release | `target/release/mw-shell.exe` 23,479,296 B，SHA256 `ABFA27C9…BA29F` |
| 机器门 | `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`，finished 2026-10-25 04:06:56 +08:00 |

重放与归档逐字节一致：同一脚本、同一 warmup 渲染出的 960×690 PNG
SHA256 与基线相同（`AEC0D24D…97B7A`）。键盘焦点轨迹
`caretaker-start-scroll-replay-2026-10-25.trace` 显示 20 步全部应用，
指针落点依次 `(632,64) → (632,420) → (8,430)`，焦点始终是
「开始页 · 新建空白图」，滑块没有抢走 Tab 焦点。

## 4. 复跑命令

```powershell
$env:APPDATA = "$PWD/target/ci-appdata/start-scroll"
$env:MW_HOME = '1'; $env:MW_START_DEMO = '1'
$env:MW_WINDOW_SIZE = '640x460'
$env:MW_GOLDEN_INPUT = 'shots/golden/inputs/start-scroll.txt'
cargo golden --ppp 1.5 --tol 0 --warmup 16 --dir shots/golden/start-scroll
```

完整阶段门：`powershell -File scripts/x100-stage-b-gate.ps1`。
CI 同步：`scripts/ci.ps1` 已加入同一场景（隔离 APPDATA、结束清理）。

## 5. 证据文件

- `caretaker-start-scroll-2026-10-25.png` — 滚到底实拍（= 新基线）
- `caretaker-start-narrow-2026-10-25.png` — 重拍后的窄窗基线
- `caretaker-start-scroll-replay-2026-10-25.trace` — 输入脚本执行 + 焦点轨迹
- `caretaker-stage-b-gate-start-scroll-2026-10-25.log` — 完整机器门日志
- 基线：`shots/golden/start-narrow/`、`shots/golden/start-scroll/`
- 脚本：`shots/golden/inputs/start-scroll.txt`

## 6. 边界

- 未开始 X21，未改 Stage C 任何文件。
- 未提交 git；未操作 Xmind 窗口或个人文件。
- 仍是 **X20 人工视觉验收待确认**：验收时应同时看 `start-narrow` 的静止
  滑块/底部渐隐与 `start-scroll` 的拖到底状态。
