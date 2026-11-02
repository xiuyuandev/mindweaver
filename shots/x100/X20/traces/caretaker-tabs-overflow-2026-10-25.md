# X20 门后维护 · 标签条横向溢出（2026-10-25）

结论：**已实现 · 待人工随 X20 一并验收**。这是 X19「横向溢出时标签条内部横向
滚动」留下的可用性缺口，属于 Stage B 修复，不是新一轮；不改变 X20 门结论，也
不越过人工门。未开始 X21，未改文档模型 / `.zmind`，未操作 Xmind 或个人文件，
未提交 git。

## 缺口

X19 只规定「能横向滚」，没有规定「看得出还能滚、切页要滚回来」：

- 34px 标签条没有滚动条或渐隐，内容被裁时没有任何提示。
- `Ctrl+Tab`、关闭左侧标签或恢复会话后，活动标签若在视野外，标签条不会滚回来；
  用户看到的是一排非活动标签，不知道当前文档在哪。
- 640×460 下 6 个最小宽标签已有 674px 内容、600px 视口，最后一个直接藏在加号区
  后面（9 个标签时内容 1004px）。

## 修复

- **活动标签揭示**：`Shell.tab_seen` 记录上一帧的
  `(活动下标, 标签数, 标签条宽)`；键变化且活动标签没有被滚动视口完整包含时，
  用 `scroll_to_rect_animation(..., Align::Center, ScrollAnimation::none())`
  即时滚进视口。已在视口内不动，避免每次切页都横向跳；滚轮滚动不改这个键，
  用户想看别的标签时不会被回拉；标签条不画时（单标签/开始页）清空记忆，
  下次出现重新揭示。
- **边缘渐隐**：内容宽 > 视口宽 + 0.5 时，在滚动视口左右边缘各画 20px 渐隐，
  颜色从 `chrome.panel` 0.9 透明度收敛到全透明；左侧强度 =
  `clamp(offset/20, 0, 1) * 0.9`，右侧 =
  `clamp((content − viewport − offset)/20, 0, 1) * 0.9`。画在标签之上、加号之下，
  只提示还有内容，不接管命中；加号始终固定在右端。不用 4px 滑块，是因为标签条
  只有 34px，滑块会和选中标签的 2px 下划线、1px 分隔线抢同一条边。
- **证据钩子**：`MW_TABS_DEMO=<数量>` 在内存里按数量生成标签；`=1`/缺省仍为
  X19 的三个固定标签，`>3` 时追加「文档 4…N」，全部不落盘。

改动文件：`crates/mw-shell/src/tab_strip.rs`（`edge_fade_strengths` 纯几何 +
`EDGE_FADE_W` + 2 组单测）、`crates/mw-shell/src/app_methods_2.rs`（揭示与渐隐
绘制）、`crates/mw-shell/src/main.rs`（`tab_seen` 字段、标签条隐藏时清空）、
`crates/mw-shell/src/app_methods_1.rs`（demo 数量）、`shots/golden/inputs/tabs-overflow.txt`、
`scripts/x100-stage-b-gate.ps1` 与 `scripts/ci.ps1`（第 26 组场景）。

## 证据

- **新 golden**：`shots/golden/tabs-overflow/shell-ppp1.50.png`，960×690，
  64,575 B，SHA256 `96D62007…48300A90`；场景 640×460 / ppp 1.5 /
  `MW_TABS_DEMO=9` / `tabs-overflow.txt`（wait 4 + Ctrl+Tab ×5 + wait 8）/
  warmup 30。画面：活动标签「文档 6」居中，左右渐隐同时可见，加号未移动。
- **行为 trace**：`caretaker-tabs-overflow-2026-10-25.trace` 记录 5 次
  `key ctrl+Tab` 与 `golden_input_done steps=26 applied=26`；只有第 5 次切到
  视野外的标签触发揭示：`tab_reveal active=5 tab=558.0..666.0 view=0.0..600.0`，
  前 4 次切换目标完整可见、没有横向跳动。
- **回归**：`tabs`（1080×720，三标签不溢出）逐字节不变，SHA256
  `F11988FD…AD2553F` 与 X19 基线一致；阶段门 26 组 × 2 次 `--tol 0`
  **52/52 全 0 差异**，其余 24 组哈希不变。
- **单测**：`tab_strip` 新增 2 条（不溢出 / 1px 容差 / 视口宽 0；顶部 / 中段 /
  末尾强度、offset 越界夹取），模块共 10 条。
- **门槛**：`cargo fmt --all -- --check` 干净；`cargo clippy -p mw-shell --all-targets`
  仅 `export_pdf.rs` 3 条历史，新增 0；`cargo test --workspace -- --test-threads=1`
  **755 passed / 0 failed**；阶段门结论仍为
  `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`
  （finished 2026-10-25 05:16:19 +08:00）；release
  `23,457,792 B` / SHA256 `BAED8EA0…DCA23965`。

复跑命令：

```powershell
$env:MW_TABS_DEMO='9'; $env:MW_WINDOW_SIZE='640x460'
$env:MW_GOLDEN_INPUT='shots/golden/inputs/tabs-overflow.txt'
cargo golden --ppp 1.5 --tol 0 --warmup 30 --dir shots/golden/tabs-overflow
```

## 未做

- 触控板横向手势与 Shift+滚轮没有单独实拍（滚轮映射沿用 X19 既有路径）。
- 200% DPI 下的标签溢出未实拍；本场景只有 ppp 1.5。
- 没有做 Xmind 实时侧对侧；标签溢出策略是产品自定义，不在 Xmind 参考范围。
