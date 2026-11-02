# 开始页键盘焦点滚入视野（X20 门后维护 ⑧）

- 日期：2026-10-25
- 轮次：X20 门后维护，不新建轮次目录，不影响 X20 门结论
- 结论：**已实现 · 待人工随 X20 一并验收**

## 缺口

X18 要求开始页所有卡片可 Tab 聚焦，X20 P1 要求窄窗下滚动 affordance 可见，
但两者之间漏了一段：640×460 / 720×620 下开始页内容高于视口，从主操作卡按
Tab 走到「模板」时，焦点已落到首屏之外，视口却不动——焦点环画在屏幕外，
键盘用户不知道自己在哪。

egui 的 `ScrollArea` 不会因为普通按钮获得键盘焦点而自动滚动；`ui.scroll_to_rect`
只对内容内调用有效，也无法在焦点变化的当帧可靠拿到卡片最终矩形。

## 口径

按 `docs/24-X100升级路线.md` 本轮定稿：

- 只在焦点 id 变化后检查一次；只认键盘 Tab/Shift+Tab 与「页面出现 / 从菜单
  返回时自动聚焦主操作卡」。鼠标点击卡片、标题栏与菜单触发器获得焦点都不
  触发滚动。
- 最小滚动：完整可见（含 8px 边距）不滚；上方裁切对齐视口顶 +8px，下方裁切
  对齐视口底 −8px；卡片高于视口时顶对齐。不做总是居中。
- 目标夹在 `[0, content_h − viewport_h]`，不溢出（1px 容差）或视口高为 0 时
  返回 `None`。
- 复用 X20 自绘滑块已有的 `scroll_to` 下一帧偏移通道；不改 `ScrollArea` 布局、
  Tab 顺序、卡片几何与焦点环；不新增依赖/线程/crate。

## 实现

只改 `crates/mw-shell/src/start.rs`：

- 新增纯函数 `focus_scroll_target(focus, viewport, offset_y, content_h) -> Option<f32>`，
  负责最小滚动与夹取；新增 3 组单测（完整可见、下方最小滚动与上方回滚夹 0、
  底部夹取与超高卡片顶对齐），`start::tests` 共 13 条。
- `StartPage` 新增 `focus_seen: Option<Id>` 与 `reveal_focus: bool`。`show()` 在
  自绘滑块之后比较本帧焦点 id：变化且来源是 Tab 或页面自动聚焦时，用
  `ctx.read_response(id)` 取卡片矩形，算出目标偏移写入既有 `scroll_to`。
- 焦点未变不滚动：用户用滚轮或滑块把页面拖到别处，不会被下一帧强行拉回。
- 根因修复前曾无条件响应焦点变化，标题栏「更多」获得焦点会带动开始页滚动；
  已用 Tab/自动聚焦两道门挡住，并以标题栏点击实拍复核。

## 证据

输入脚本 `shots/golden/inputs/start-tab-focus.txt`：`wait 4` + 7 次 `key Tab`
+ `wait 8`（共 26 个输入帧）。

修复前（`caretaker-start-focus-scroll-before-2026-10-25.{png,trace}`）：
trace 只有 focus 变化，最后停在「模板 思维导图」，没有任何滚动事件；实拍
960×690 / 153,877 B / SHA256 `3186D9D1…6E68AE`，焦点卡片在视口外。

修复后（`caretaker-start-focus-scroll-after-2026-10-25.{png,trace}`）：
trace 出现 3 次
`start_focus_scroll from=0.0 to=41.3`、
`from=41.3 to=93.3`、
`from=93.3 to=335.3`；实拍 960×690 / 223,988 B / SHA256
`007C6DD7…7B86F4A9`，焦点卡片与 1.4px 焦点环完整可见。

前后差异 440,664 px = 66.525%，Pillow `bbox=(1, 72, 959, 654)`，全部落在
开始页内容区（x≥1、y≥72），未动标题栏与标签条。

回归复核（`caretaker-start-focus-titlebar-2026-10-25.{png,trace}`）：在
(66, 24) 点击标题栏「更多」，15 帧 trace 无 `start_focus_scroll`，实拍
154,689 B / `0C74D466…FBD084`。

外观：`start`（ppp 1.0）`4EC10E68…B054544F`、`start-narrow`
`2BDBDCA1…CF655523`、`start-scroll` `AEC0D24D…ACA97B7A` 均逐字节未变。
新增第 29 组 golden `start-tab-focus`（640×460、ppp 1.5、`MW_HOME=1`
`MW_START_DEMO=1`、warmup 16）：960×690 / 223,988 B /
SHA256 `007C6DD76662427E52487B30823DE3E83A5E62ADEAA3EBE7279204657B86F4A9`，
双跑 `--tol 0` 0 差异；副本在
`shots/x100/X20/golden/start-tab-focus-shell-ppp1.50.png`。

## 门槛

`scripts/x100-stage-b-gate.ps1`（已在场景表与 `scripts/ci.ps1` 接入）：

- 29 组比较 × 2 轮 `--tol 0` = **58/58** 全 0 差异
- **761 tests / 0 failed**（`--test-threads=1`）
- fmt 干净；mw-shell clippy 仅 `export_pdf.rs` 3 条历史告警，新增 0
- release `23,484,928 B` / SHA256
  `33CE76583F62121E7DFF94A1E6772377BB082E26C92D62D55305A7B3133EE8EA`
- 结论仍为 `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`
  （started 2026-10-25 06:43:04 +08:00，finished 06:44:37 +08:00）
- 日志：`caretaker-stage-b-gate-start-focus-2026-10-25.log`

## 未做

- 只实拍了正向 7 次 Tab；Shift+Tab 反向回滚只有纯函数单测，没有单独 golden。
- 只在 ppp 1.5 / 640×460 实拍；ppp 2.0 与更矮窗口未覆盖。
- 没有真实键盘/读屏采样，也没有测高对比度模式下的焦点环。
- 未改文档模型与 `.zmind`，未新增依赖，未操作 Xmind 或个人文件，未提交 git，
  未开始 X21。
