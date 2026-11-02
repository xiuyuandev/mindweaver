# X100 Stage B gate report

> Prepared: 2026-10-25 02:34 CST · Reference: Xmind `26.5.1107.202609082009`
>
> Gate state: **machine gate passed; human X20 review FAILED on 2026-10-25.**
> X20 stays `implemented`, not `verified`. Stage C (`X21–X30`) is frozen.
> Stage B2 (CER-01–CER-34) is planned but not started; design contract
> `docs/28-内容体验重构规范.md`, execution contract
> `docs/29-内容体验重构工作包.md`.

## 1. Round status

| Round | Status | Evidence |
| --- | --- | --- |
| X11 | verified | 设计令牌 v7、壳层字阶去重、动效/阴影/控件几何统一、裸字号防回退测试 · `shots/x100/X11/` |
| X12 | verified | 30 个矢量图标统一 16 单位网格 / 1.5 线宽，字形按钮清零 · `shots/x100/X12/` |
| X13 | verified | 组件令牌 + `ui_kit` v2（按钮/图标按钮/胶囊/分段/开关/输入/下拉/菜单壳/标签页）；本地控件清零；704 tests · `shots/x100/X13/` |
| X14 | verified | 48px 标题栏、30px 菜单行、四态保存芯片、28px 工具栏；706 tests · `shots/x100/X14/` |
| X15 | verified | 画布上下文工具条、154×32 缩放条、24px 状态区；`selected` 第 14 场景；711 tests · `shots/x100/X15/` |
| X16 | verified | 统一弹出菜单引擎：四类条目、子菜单/边缘翻转、禁用项、快捷键列、键盘导航；`menu` 第 15 场景；718 tests · `shots/x100/X16/` |
| X17 | verified | 统一 `dialog` 壳 + 19 处模态迁移、焦点陷阱、Esc 路由与层级互斥；731 tests · `shots/x100/X17/` |
| X18 | verified | 开始页信息层级、`start::layout` 单一断点源、空状态与清除最近文件、Tab/Enter 焦点闭环；735 tests · `shots/x100/X18/` |
| X19 | verified | 标签条重做、六态保存与延后写盘、两段式打开与失败弹窗；746 tests · `shots/x100/X19/` |
| X20 | implemented · **人工门未通过 2026-10-25** | 25 场景外壳视觉矩阵（门时 23 组 golden）、golden 确定性修复、sand 强调色过 AA；门时为 23 组 golden 双跑 / 747 tests；门后维护 14 项后为 31 组 / 765 tests；机器门 PASS，但用户人工验收判定三个内容界面不合格 · `shots/x100/X20/` |

X20 的 manifest 状态写 `implemented` 而不是 `verified`：机器证据齐全，但
`docs/26` 的「已验证」要求阶段门通过，而人工门已于 2026-10-25 判定未通过。

### 1.1 Human gate result (2026-10-25)

用户原话（摘要，完整上下文见 `docs/28` §1）：

> “大纲界面无论是编辑还是 UI 都超级难看，根本不够简洁。”
> “写笔记的页面也很糟糕……没有实现那种克制的美感、精致的美感。”
> “卡片库……那简直就是一个玩具。”

判定：

- **X20 人工门：未通过。** 外壳方向有进展，但全屏大纲、卡片笔记页、笔记库三个
  内容界面不合格；机器门不能替代审美/可用性判词。
- **Stage C：冻结。** 不开始 X21–X30 的画布重做。
- **后续：Stage B2。** 34 个工作包、4 批、4 道人工门；旧 X61–X67、X69–X70、
  X81–X82 范围并入；X68 命令面板延后。
- **X20 证据保留。** 不删、不改写、不 `--update`；它仍是外壳稳定性的基线，
  但不是“外壳已完成”的凭据。

## 2. Machine gate

一条命令：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/x100-stage-b-gate.ps1
```

2026-10-25 02:24–02:28 的结果（`target/x100/stage-b-gate.log`）：

- X11–X19 证据包完整性：9/9 PASS（共 259 条证据路径）。
- 全部 `scripts/x100-*.ps1` 语法：PASS。
- `cargo fmt --all -- --check`：干净。
- `cargo clippy -p mw-shell --all-targets`：mw-shell 仅 3 条
  `export_pdf.rs` 历史告警；X20 改动文件新增 0。mw-core 的
  `theme.rs:22 self_named_constructors` 是历史告警，按精确位置放行；
  mw-core 10 条、mw-render 7 条历史告警与 X19 相同。
- `cargo test --workspace -- --test-threads=1`：**747 passed / 0 failed**。
- release 构建：`target/release/mw-shell.exe` 23,454,720 B，SHA256
  `0f26bbb9…3dd72b`。
- golden：23 组比较 × 2 次 `--tol 0`，46/46 全绿。
- 结论行：`MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`。

**一次有记录的重试**：首轮门脚本把所有 golden 误报为 FAIL。原因是
`Run-GoldenScene` 把 cargo 的 stdout 和 `$LASTEXITCODE` 一起返回，数组与 0
比较永远非空，同时输出被吞。改为显式收集输出并只返回 `[int]$LASTEXITCODE`
后重跑，得到 46/46。失败与修复都保留在 X20 报告里，没有删记录。

### 2.1 门后维护（2026-10-25 02:57，不影响门结论）

本地 `scripts/ci.ps1` 曾因 `MaxClippyWarnings=37` 过期而必失败（workspace
实计 78 条）。根因是 `mw-core/src/design.rs` 测试模块的 44 条常量不变量断言
被 `clippy::assertions_on_constants` 误报（f32 令牌无法 const-eval）；局部
`#![allow]` 并注明原因后回落到 33/34 条，未抬高预算。测试步骤同步改为
`--test-threads=1`，与阶段门一致。复跑 `scripts/ci.ps1 -SkipGolden` →
**CI OK / exit 0**（clippy 33、**747 tests / 0 failed**、fmt 干净）；
mw-shell 自身仍仅 `export_pdf.rs` 3 条历史告警；
`x100-evidence.ps1 -Round X20 -Check` PASS（80 路径）。未改外观、
未重拍 golden、未开始 X21。日志：
`shots/x100/X20/traces/caretaker-ci-2026-10-25.log`。

### 2.2 门后维护（2026-10-25 03:30，不影响门结论）

X20 归档的 `narrow-menu` 脚本按原样重放，菜单打不开：`click` / `rightclick` /
`middleclick` 原语把 `PointerMoved` 与按下放进了同一帧，而 egui 的命中测试读
**上一帧**登记的控件矩形；离屏脚本没有真实光标，首帧按下被丢弃。真实鼠标总是
先移动再按下，所以产品日常不可见，但脚本回放会稳定丢点击。修复只动测试工具
`crates/mw-shell/src/golden_input.rs`：每个点击先单独推进一帧 `PointerMoved`，
下一帧按下、再下一帧松开；`move` 共用新的 `move_step`；单测钉住帧序。
修复后同一脚本（文件未改）重放与归档
`actual/narrow-menu-snow-640x460-p150.png` **逐字节一致**
（SHA256 `79C4E412…810DBB`，0 px），X19 的 `tab-menu` / `middle-close`
行为不变。产品代码、外观、golden 基线、`.zmind` 均未动。复跑：
fmt 干净、mw-shell clippy 仅 `export_pdf.rs` 3 条历史、**747 tests / 0 failed**、
阶段门 23 组 × 2 次 `--tol 0` 全 0 差异，结论仍为
`MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`；release 因源码改动
重建为 23,455,744 B / SHA256 `E67D79A6…03221F`（manifest 里的旧哈希保持历史
不动）。证据：`shots/x100/X20/traces/caretaker-input-click-determinism-2026-10-25.md`、
`caretaker-narrow-menu-replay-2026-10-25.{png,trace}`、
`caretaker-stage-b-gate-2026-10-25.log`。

### 2.3 门后维护（2026-10-25 04:07，不影响门结论）

开始页在 640×460 / 720×620 下纵向溢出，但 egui 浮动滚动条静止态透明，且
`CentralPanel::no_frame()` 让背景色透明、内置渐隐失效——用户看不到「还能往下」。
按 `docs/24` 新增的验收口径，在 `crates/mw-shell/src/start.rs` 自绘 4px 可拖拽
滑块（静止 alpha 130 / 悬停 200 / 拖拽 235、最短 28px、抓取偏移跟手）与
40px 上下渐隐；命中区左右各扩 6px，使用非 focusable 的 `Sense::DRAG`，不进入
X18 焦点闭环；内置滚动条改为 `AlwaysHidden`。未新增依赖/线程/crate，未改文档
模型，未越 X20 人工门。

`start`（1440×1000，不溢出）逐字节 0 差异。`start-narrow` 重拍：131,626 B
`5BE44212…11E8` → 196,898 B `2BDBDCA1…5523`，漂移 6.419%（最大 176），
bbox 全在滑块与渐隐带。新增第 24 组 golden `start-scroll`（640×460、ppp 1.5、
拖滑块到底，960×690 / 160,515 B / `AEC0D24D…97B7A`）；两组新基线各双跑
`--tol 0` 0 差异。复跑阶段门：**24 组 × 2 = 48/48** 全 0 差异、
**750 tests / 0 failed**、fmt 干净、mw-shell clippy 仅 `export_pdf.rs` 3 条历史，
结论仍为 `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`
（finished 2026-10-25 04:06:56 +08:00）。release 重建为 23,479,296 B /
SHA256 `ABFA27C9…BA29F`。证据：
`shots/x100/X20/traces/caretaker-start-scroll-2026-10-25.md`、
`caretaker-start-scroll-2026-10-25.png`、`caretaker-start-narrow-2026-10-25.png`、
`caretaker-start-scroll-replay-2026-10-25.trace`、
`caretaker-stage-b-gate-start-scroll-2026-10-25.log`。

### 2.4 门后维护（2026-10-25 04:47，不影响门结论）

本节对应 §6 P1「窄窗右键菜单（非应用菜单）的边缘翻转没有实拍」。实拍确认
不只是「没拍」，而是 X16「任何一级都不得出内容区」在「菜单比视口高」时
不成立：640×460 下节点右键菜单 19 项自然高 477，卡片从 y=8 起画到 y=485，
末两项「折叠」「聚焦」被窗口裁掉，鼠标与键盘都到不了。

修复只动 `crates/mw-shell/src/menu.rs`：`card_height` 把卡片高度夹到
`content.height() - EDGE*2`（根菜单与子菜单同口径），装不下时层级内容改
`ScrollArea`（原生条 `AlwaysHidden`、保留滚轮、自绘 4px 静止滑块
alpha 130 / 最短 28px）；键盘导航置 `reveal`，高亮行出视口时
`scroll_to_rect(Align::Center)`。不溢出的菜单保持原绘制路径，像素零漂移。

证据：640×460 / ppp 1.5 的几何为自然 477 → 卡片 444 → 视口 432 /
内容 465（溢出 33）；新增单测 3 条（菜单模块共 14 条）；新增第 25 组 golden
`menu-narrow`（960×690，`32b20b46…4f255c75`），双跑 `--tol 0` 0 差异；
`menu` 修复前后逐字节一致（`fbfaf8ce…3342610`），`shots/golden` base 三档
ppp 也逐字节一致；键盘 End 到达末项「聚焦」的实拍 `ac13bdb0…b1c91a440`
与 trace 一起归档。复跑阶段门：**25 组 × 2 = 50/50** 全 0 差异、
**753 tests / 0 failed**、fmt 干净、mw-shell clippy 仅 3 条历史、release
`23,505,408 B` / SHA256 `3b6394be…e8e5a47`，结论仍为
`MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`
（finished 2026-10-25 04:47:50 +08:00）。本目录 `actual/` 的 25 张门时实拍
未改。证据：`shots/x100/X20/traces/caretaker-menu-overflow-2026-10-25.md`、
`caretaker-menu-overflow-{before,after,end}-2026-10-25.png`、
`caretaker-menu-overflow-end-2026-10-25.trace`、
`caretaker-stage-b-gate-menu-overflow-2026-10-25.log`。

**本轮未做**：滑块只做静止指示 + 滚轮/键盘，不做拖拽命中（与开始页滑块交互
层级不同，统一留到后续轮次）；200% DPI 下的菜单溢出未实拍。

### 2.5 门后维护（2026-10-25 05:16，不影响门结论）

X19 规定「横向溢出时标签条内部横向滚动」，但滚动是隐形的：34px 标签条没有
滚动条/渐隐；`Ctrl+Tab`、关闭左侧标签或恢复会话后，活动标签在视野外也不会滚
回来。640×460 下 9 个最小宽标签内容 1004px、视口 600px。修复只动
`tab_strip.rs` / `app_methods_2.rs` / `main.rs` / demo 钩子：

- `tab_seen = (活动下标, 标签数, 标签条宽)` 变化时，若活动标签未被视口完整包含，
  用 `scroll_to_rect_animation(..., Align::Center, ScrollAnimation::none())`
  即时揭示；已在视口内不动，滚轮滚动不触发回拉；标签条隐藏时清空记忆。
- 横向溢出时在视口左右各画 20px `chrome.panel` 渐隐（左侧
  `clamp(offset/20)*0.9`、右侧 `clamp(剩余/20)*0.9`），只提示内容，不接管命中；
  加号固定。不用 4px 滑块：会和标签下划线/分隔线抢边。
- `MW_TABS_DEMO=N`（N>3）在内存里生成 N 个标签，供溢出场景使用。

证据：新增第 26 组 golden `tabs-overflow`（640×460、ppp 1.5、
`MW_TABS_DEMO=9`、`Ctrl+Tab` ×5、warmup 30，960×690 / 64,575 B /
SHA256 `96D62007…48300A90`），双跑 `--tol 0` 0 差异；`tabs` 基线逐字节不变
（`F11988FD…AD2553F`，与 X19 一致）；trace 只有第 5 次切换触发
`tab_reveal active=5 tab=558.0..666.0 view=0.0..600.0`，前 4 次完整可见无跳动。
复跑阶段门：**26 组 × 2 = 52/52** 全 0 差异、**755 tests / 0 failed**、
fmt 干净、mw-shell clippy 仅 3 条历史、release `23,457,792 B` /
SHA256 `BAED8EA0…DCA23965`，结论仍为
`MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`
（finished 2026-10-25 05:16:19 +08:00）。证据：
`shots/x100/X20/traces/caretaker-tabs-overflow-2026-10-25.md`、
`caretaker-tabs-overflow-2026-10-25.{png,trace}`、
`caretaker-stage-b-gate-tabs-overflow-2026-10-25.log`。

**本轮未做**：标签溢出只有 ppp 1.5 实拍；触控板横向手势/Shift+滚轮未单独实拍。

### 2.6 门后维护（2026-10-25 05:46，不影响门结论）

本节对应 2.5 的实现缺陷：滚轮横向映射写在了「每个标签」的循环体内。
`Ui::scroll_with_delta` 只把增量累进本帧 pass state，`ScrollArea` 在本帧收尾
一次取走，于是同一个滚轮增量被按标签数重复累加——9 个标签时一次滚轮＝9 倍，
标签条直接冲到头；滚动速度还随标签数变化。修复只动
`tab_strip.rs` / `app_methods_2.rs` / `main.rs` / `app_methods_1.rs`：

- 新增纯函数 `wheel_scroll_x`：一帧内的 `Event::MouseWheel` 合成一个
  `dx + dy` 位移；Ctrl/Alt/Win 组合丢弃，`<= 0.5px` 视为抖动；调用点移到
  标签循环外，每帧最多一次。
- 不再用 `smooth_scroll_delta`（按真实帧间隔摊到后续帧，离屏回放依赖机器
  速度）；改用 `scroll_with_delta_animation(..., ScrollAnimation::none())`，
  与 X19 揭示同口径，滚轮无惯性尾巴。
- 命中区从内容 `ui.max_rect()` 改为 600px 滚动视口（右端 40px 加号区除外）；
  触控板横滑与 Shift 滚轮走同一条 `dx + dy` 路径。
- `MW_TRACE` 新增 `tab_scroll x= content= view=`，横向偏移变化逐笔记账。

证据：修复前同一脚本（`hover 300 65` + `scroll 0 -100` + `wait 12`）
产生 **108 行** `tab_wheel`（9 标签 × 12 帧），实拍冲到最右；修复后 **1 行**
`dx=-100.00`，`tab_scroll x=100.0 content=1006.0 view=600.0`；同一脚本换
8 标签（content 896）仍为 `x=100.0`，证明位移与标签数无关。修复前后
960×690 差异 14,328 px = 2.163%，包围盒 `x[1..900] y[77..119]`，全部落在
标签条。新增第 27 组 golden `tabs-wheel`（960×690 / 111,542 B /
SHA256 `d5ef9b4a…0261b910`），双跑 `--tol 0` 0 差异；`tabs` /
`tabs-overflow` 逐字节未变。复跑阶段门：**27 组 × 2 = 54/54** 全 0 差异、
**756 tests / 0 failed**、fmt 干净、mw-shell clippy 仅 3 条历史、release
`23,458,816 B` / SHA256 `6B23D098…79901116`，结论仍为
`MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`
（finished 2026-10-25 05:46:09 +08:00）。证据：
`shots/x100/X20/traces/caretaker-tabs-wheel-2026-10-25.md`、
`caretaker-tabs-wheel-{before,after}-2026-10-25.png`、
`caretaker-tabs-wheel-{before,8,9}-2026-10-25.trace`、
`caretaker-stage-b-gate-tabs-wheel-2026-10-25.log`。

**本轮未做**：滚轮只拍了 ppp 1.5；ppp 2.0 的溢出 + 滚轮未实拍；触控板
横滑只有事件脚本覆盖，没有真实触控板采样；惯性/滑动摩擦被刻意去掉。

### 2.7 门后维护（2026-10-25 06:15，不影响门结论）

2.6 只解决了「一帧应用几次」，没有解决「一个增量是多少」：`wheel_scroll_x`
直接相加 `delta`，忽略了 `MouseWheelUnit`。egui-winit 对普通鼠标发 `Line`
（一格 ±1）、对触控板发 `Point`（已是点）；egui 自己的 `ScrollArea` 会把
Line 乘 `input_options.line_scroll_speed`（egui 0.37 原生端 40），但标签条
`mouse_wheel: false` 没有原生兜底——真实鼠标滚一格只走 1px。2.6 的 golden
脚本发 Point，所以基线测不出这条硬件路径。

修复只动 `crates/mw-shell`：`tab_strip::wheel_scroll_x` 接收
`(Vec2, MouseWheelUnit, bool)`，`Point` 原样、`Line × line_scroll_speed`、
`Page × 视口高`，换算后再做 0.5px 抖动过滤；调用点从
`ctx.options().input_options.line_scroll_speed` 与 `viewport_rect().height()`
取参数，不硬编码 40；`golden_input` 新增 `scroll_lines` 原语（Line 单位）与
解析单测，`scroll` 保持 Point。命中区、修饰键、每帧一次、
`ScrollAnimation::none()` 均不变。未新增依赖/线程/crate，未改文档模型。

证据：新增脚本 `tabs-wheel-lines.txt`（3 格）。修复前 trace
`tab_wheel dx=-3.00`、`tab_scroll x=3.0 content=1006.0 view=600.0`，实拍
113,614 B / `F551E86D…6978BFF`；修复后 `dx=-120.00`、`x=120.0`，实拍
111,498 B / `03E71072…75091E`。前后差异 12,691 px = 1.916%，包围盒
`x[2..894] y[72..122]` 全在标签条。`tabs`（`F11988FD…AD2553F`）、
`tabs-wheel`（`d5ef9b4a…0261b910`）、`tabs-overflow`
（`96D62007…48300A90`）逐字节未变；新增第 28 组 golden `tabs-wheel-line`
（960×690 / 111,498 B / `03E71072…75091E`），双跑 `--tol 0` 0 差异。
复跑阶段门：**28 组 × 2 = 56/56** 全 0 差异、**758 tests / 0 failed**、
fmt 干净、mw-shell clippy 仅 3 条历史、release `23,439,872 B` /
SHA256 `81C2A523…FFEDAE`，结论仍为
`MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`
（finished 2026-10-25 06:15:06 +08:00）。证据：
`shots/x100/X20/traces/caretaker-tabs-wheel-unit-2026-10-25.md`、
`caretaker-tabs-wheel-unit-{before,after}-2026-10-25.{png,trace}`、
`caretaker-stage-b-gate-tabs-wheel-unit-2026-10-25.log`。

**本轮未做**：ppp 2.0 的 Line 滚轮未实拍；Page 单位只有单测，没有硬件来源；
没有真实鼠标/触控板采样。

### 2.8 门后维护（2026-10-25 06:44，不影响门结论）

X18 只规定「开始页所有卡片可 Tab 聚焦」，X20 P1 只规定「窄窗滚动可见性」，
两者之间漏了「聚焦后必须可见」：640×460 / 720×620 下开始页内容高于视口，
从主操作卡按 Tab 走到「模板」时焦点已落到首屏之外，视口不动，焦点环画在
屏幕外。egui 的 `ScrollArea` 不会因为普通按钮获得键盘焦点而自动滚动。

修复只动 `crates/mw-shell/src/start.rs`：新增纯函数
`focus_scroll_target(focus, viewport, offset_y, content_h)`，完整可见（含 8px
边距）返回 `None`，上方裁切对齐视口顶 +8px、下方裁切对齐底 −8px、卡片高于
视口时顶对齐，目标夹在 `[0, content_h − viewport_h]`；`StartPage` 用
`focus_seen` 只在焦点 id 变化后检查一次，且只认 Tab/Shift+Tab 与页面出现 /
菜单返回的自动聚焦，鼠标点击与标题栏、菜单触发器获得焦点都不触发滚动；
焦点未变不滚动，用户滚轮/拖拽的位置不会被抢回。目标偏移复用自绘滑块已有的
`scroll_to` 下一帧通道，不改 `ScrollArea` 布局、Tab 顺序、卡片几何与焦点环。
新增 3 组单测（`start::tests` 共 13 条），不新增依赖/线程/crate，未改文档模型。

证据：脚本 `start-tab-focus.txt`（`wait 4` + 7 次 Tab + `wait 8`）。修复前
trace 只有 focus 变化、最后停在「模板 思维导图」，实拍 960×690 / 153,877 B /
`3186D9D1…6E68AE`；修复后 trace
`start_focus_scroll from=0.0 to=41.3` → `41.3 to=93.3` → `93.3 to=335.3`，
实拍 223,988 B / `007C6DD7…7B86F4A9`，焦点卡片与 1.4px 焦点环完整可见。
前后差异 440,664 px = 66.525%，`bbox=(1, 72, 959, 654)` 全在开始页内容区。
标题栏 (66, 24) 点击复核 15 帧无 `start_focus_scroll`
（154,689 B / `0C74D466…FBD084`）。`start`（`4EC10E68…B054544F`）、
`start-narrow`（`2BDBDCA1…CF655523`）、`start-scroll`
（`AEC0D24D…ACA97B7A`）逐字节未变；新增第 29 组 golden `start-tab-focus`
（640×460、ppp 1.5、`MW_HOME=1 MW_START_DEMO=1`、warmup 16，960×690 /
223,988 B / `007C6DD7…7B86F4A9`），双跑 `--tol 0` 0 差异。
复跑阶段门：**29 组 × 2 = 58/58** 全 0 差异、**761 tests / 0 failed**、
fmt 干净、mw-shell clippy 仅 3 条历史、release `23,484,928 B` /
SHA256 `33CE7658…33EE8EA`，结论仍为
`MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`
（started 2026-10-25 06:43:04 +08:00，finished 06:44:37 +08:00）。证据：
`shots/x100/X20/traces/caretaker-start-focus-scroll-2026-10-25.md`、
`caretaker-start-focus-scroll-{before,after}-2026-10-25.{png,trace}`、
`caretaker-start-focus-titlebar-2026-10-25.{png,trace}`、
`caretaker-stage-b-gate-start-focus-2026-10-25.log`。

**本轮未做**：只实拍正向 7 次 Tab，Shift+Tab 反向回滚只有纯函数单测；只在
ppp 1.5 / 640×460 实拍；没有真实键盘/读屏采样。

### 2.9 门后维护（2026-10-25 07:17，不影响门结论）

X17 规定「同一时刻只有一个指针/键盘主人」，但标签条有两条**裸输入**路径
绕开 egui 层级命中：滚轮直接用 `pointer_interact_pos()` 判断落点，重排用
`press_origin()` + `interact_pos()` 比对 4px 位移自己起手。模态暗幕挡得住
点击，挡不住这两条路径：640×460 / ppp 1.5 / `MW_DIALOG_DEMO=1
MW_TABS_DEMO=9` 下，指针在暗幕后的标签条上滚动会出现 `tab_wheel dx=-100.00`
与 `tab_scroll x=100.0`；按住拖到 520px 出现
`tab_reorder from=2 slot=5 active=0`，活动文档在弹窗背后被换掉。
`confirm_close_tab` 按**下标**记关闭目标，若在「关闭未保存标签」确认框背后
重排，确认后会关掉另一份标签，可能静默丢掉未保存内容。文件菜单
（Memory popup）同样不覆盖标签条，滚轮会穿过去。

修复只动 `crates/mw-shell/src/app_methods_2.rs`：`tab_bar` 入口新增
`foreground = overlay_state().top_modal().is_some() || dialog_demo ||
egui::Popup::is_any_open(ctx)`，前台存在时取消在途拖拽、停止一切标签输入；
`interactive = !foreground && tab_menu.is_none() && context_menu.is_none()`
统一门控滚轮、重排起手与加号。用 `Popup::is_any_open` 而不是「任意 Area」：
提示气泡是 `OpenKind::Open + hover`，悬停提示盖住标签条时不应把标签条冻住。
不改标签几何、顺序、渐隐、活动标签揭示，不新增依赖/线程/crate，不改文档
模型与 `.zmind`。

证据：脚本 `tabs-modal-input.txt`（`hover` + `scroll 0 -100` +
`press/move/release` 拖 220px，21 帧、warmup 24）。修复前滚轮 trace
`tab_wheel dx=-100.00` / `tab_scroll x=100.0 content=1006.0 view=600.0`
（960×690 / 107,839 B / `9AE4ADD3…A8688B`）；拖动 trace
`tab_reorder from=2 slot=5 active=0` + `focus canvas`
（106,795 B / `901B2361…70B3FBE`）。修复后两条 trace 均 0 条标签事件，
实拍同为 108,398 B / `01ACE0A2…C896648`（与滚轮后图逐字节相同）。
前后差异：滚轮 12,106 px = 1.828%（最大 154，
`bbox x[1..899] y[72..122]` 全在标签条）；拖动 2,767 px = 0.418%
（最大 208，`bbox x[354..791] y[88..495]`，含暗幕后的画布——活动文档确实
被换掉）。文件菜单复核：1080×720 打开「文件」后悬停标签条滚轮，0 条标签
事件，菜单保持打开（1620×1080 / 221,502 B / `87DEB4DE…C18B1A`）。
新增第 30 组 golden `tabs-modal-input`（960×690 / 108,398 B /
`01ACE0A2…C896648`），双跑 `--tol 0` 0 差异。复跑阶段门：
**30 组 × 2 = 60/60** 全 0 差异、**761 tests / 0 failed**、fmt 干净、
mw-shell clippy 仅 3 条历史、release `23,485,952 B` /
SHA256 `38EBF196…41F27DA`，结论仍为
`MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`
（started 2026-10-25 07:15:26 +08:00，finished 07:17:07 +08:00）。
既有基线未重写：本组比较均不带 `--update`，`shots/golden/` 下 07:00 后只有
新基线一个有 mtime 变化。证据：
`shots/x100/X20/traces/caretaker-tabs-modal-isolation-2026-10-25.md`、
`caretaker-tabs-modal-isolation-{before,after}-{wheel,drag}-2026-10-25.{png,trace}`、
`caretaker-tabs-menu-passthrough-2026-10-25.{png,trace}`、
`caretaker-tabs-modal-input-golden-2026-10-25.trace`、
`caretaker-stage-b-gate-tabs-modal-2026-10-25.log`。

**本轮未做**：文件菜单只留修复后 trace，没有修复前 A/B；只实拍新建文档
对话框一种 `top_modal()`，其余模态共享同一谓词但未逐个实拍；提示气泡覆盖
标签条时仍允许滚动（有意），未做 golden 固定；没有真实鼠标/触控板采样。

### 2.10 门后维护（2026-10-25 07:52，不影响门结论）

X17 的「同一时刻只有一个指针/键盘主人」在画布上还差一处：右键菜单/标签菜单
走 `overlay_state().context`，已进 `input_blocked`；标题栏菜单（文件/编辑/
视图/插入/帮助/更多）是 egui Memory 弹层，不进 overlay 状态，`input_blocked`
也没看它。1080×720 / ppp 1.5 实拍：打开「文件」菜单后点画布上的「织思」，
菜单收起的同时节点被选中（状态栏「已选：织思」、选中环、节点工具条），
trace `canvas_click blocked=false`——点浮层外面第一下只负责收起浮层，这与
2.9 定下的规则冲突。

修复只动 `crates/mw-shell/src/main.rs`：`input_blocked` 加入
`egui::Popup::is_any_open(&ctx)`（与 2.9 的标签条 `foreground` 同一条谓词），
菜单弹层开着时画布不收指针手势与画布快捷键；读的是上一帧 Memory 状态、而画布
本帧稍后才绘制，收起菜单的那一下正好被挡住，tooltip 不写 Memory 不受影响；
门开时 `handle_pointer` 清掉在途手势、`shortcuts` 提前返回。不改菜单布局/
文案/动作，无弹层时画布行为与 X20 基线逐像素一致，不新增依赖/线程/crate。

证据：脚本 `menu-canvas-click.txt`（开菜单 → 点节点 → 指针移开，24 帧、
warmup 24）。修复前 trace `canvas_click blocked=false`、实拍 184,050 B
状态栏「已选：织思」；菜单展开对照实拍 207,370 B 证明点击前菜单确实开着；
修复后 trace `canvas_click blocked=true`、实拍 175,220 B 无选中/工具条。
前后差异 29,917 px = 1.710%（最大 205，`bbox=(267,87,1022,1071)`：
画布工具条 + 节点环 + 状态栏）。新增第 31 组 golden `menu-canvas-click`
（1620×1080 / 175,220 B / `811C6D36…9CF925`），隔离 APPDATA 双跑
`--tol 0` 0 差异。复跑阶段门：**31 组 × 2 = 62/62** 全 0 差异、
**761 tests / 0 failed**、fmt 干净、mw-shell clippy 仅 3 条历史、
release `23,487,488 B` / SHA256 `3EA8B70F…E72240`，结论仍为
`MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`
（started 2026-10-25 07:51:08 +08:00，finished 07:52:52 +08:00）。
既有 30 组比较均不带 `--update`，逐字节未变。证据：
`shots/x100/X20/traces/caretaker-menu-canvas-ownership-2026-10-25.md`、
`caretaker-menu-canvas-ownership-{before,after,menu-open}-2026-10-25.png`、
`caretaker-menu-canvas-ownership-{before,after,golden}-2026-10-25.trace`、
`caretaker-stage-b-gate-menu-canvas-2026-10-25.log`。

**本轮未做**：方向键同被该门挡住但没有独立键盘回放脚本；其他 Memory 弹层
（大纲/更多菜单）共用同一条门，未逐个拍前后对照；真实菜单入场/收起动画未量化。

### 2.11 门后维护（2026-10-25 08:30，不影响门结论）

X17 的「同一时刻只有一个指针/键盘主人」在全屏大纲上还缺一处：大纲自绘整列表，
键盘直接读 `ui.input` / `consume_key`，不经过 egui 焦点系统。`egui::Modal`
只挡指针与 Tab 巡游，标题栏菜单又是 Memory 弹层，两者都挡不住这些裸键。
1080×720 / ppp 1.5、`MW_OUTLINE=1 MW_OUTLINE_FOCUS=1 MW_DIALOG_DEMO=1`
实拍：弹窗开着按 `Delete` → `outline_delete`，按 `Enter` →
`outline_add_sibling` 并进入行内编辑，按 `Tab` → `outline_indent`，三下都在
暗幕背后改了文档；删除/缩进落到不可见的选择上。

修复只动 `crates/mw-shell/src/panels/outline.rs`、`app_methods_2.rs`、`main.rs`：
新增纯函数 `row_keys_owned(blocked, editing, filter_owned)`；`show` 新增
`blocked` 参数，由 `main.rs` 传入既有的 `input_blocked`（与 2.9 标签条、
2.10 画布同一条谓词），键盘处理链头部整段跳过，只发
`outline_key_blocked key=<K>` trace；搜索框 Esc 回焦也加 `!blocked`。门开时
不抢/不移焦点，门关后原行焦点保持；不改行高、缩进、焦点环、滚动与外观。

证据：脚本 `outline-modal-input.txt`（`wait 8` + `Delete` + `Tab` + `Home`，
24 帧）与 `outline-menu-input.txt`（点「文件」→ `Delete`，23 帧）。修复前
26 帧脚本 trace 出现 `outline_delete` / `outline_add_sibling` /
`outline_indent`；修复后三键全部 `outline_key_blocked`，弹窗保持打开，
大纲 3/13 行与缩进层级不变（1620×1080 / 156,211 B / `2E51E8E1…D912D61D1`）。
菜单场景先拍展开对照图（146,684 B / `ADA89176…DAD2F0C`），再按 `Delete`：
只有 `outline_key_blocked`，无 `outline_delete`，菜单由 `PopupNav` 收起
（119,369 B / `96487082…504E26A9`）。修复后 `Enter` 会落到弹窗默认「确定」
并关闭弹窗，归档脚本因此改用 `Delete` / `Tab` / `Home`。复跑阶段门：
**31 组 × 2 = 62/62** 全 0 差异、**763 tests / 0 failed**、fmt 干净、
mw-shell clippy 仅 3 条历史、release `23,469,056 B` / SHA256
`A4662B1F…BBB862`，结论仍为
`MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`
（started 2026-10-25 08:32:49 +08:00，finished 08:34:09 +08:00）。
`outline` / `outline-focus` / `outline-compact` 及其余基线逐字节未变。
证据：`shots/x100/X20/traces/caretaker-outline-keys-2026-10-25.md` 及
同前缀 trace/PNG、`caretaker-stage-b-gate-outline-keys-2026-10-25.log`。

**本轮未做**：大纲行内编辑框在模态打开时的提交路径仍由 egui 焦点系统管理，
未单独实拍；卡片库/阅读页等页面型浮层未扩面。

### 2.12 门后维护（2026-10-25 08:34，不影响门结论）

2.11 的首次阶段门在 `card-slash` 上暴露了一个既有确定性缺陷（与大纲改动
无关）：`MW_CARD_DEMO` 的示例卡片用 `now_ms()` 当时间戳，阅读页渲染
「创建于/更新于」，`format_date` 按 UTC 取日；本地 08:00（UTC+8）跨日后
字符串从 `2026-10-24` 变 `2026-10-25`，而基线生成于 2026-10-24 22:30。
08:27 的门两遍都在 `x[1380..1390] y[370..415]` 出现 198 px 超差
（0.011%，最大 92）；07:52 的同一条门仍 62/62，因为当时 UTC 还在 10-24。

修复只动 `app_methods_1.rs` 与 `app_tests.rs`：示例卡片时间戳固定为
`DEMO_CARD_TS_MS = 1_792_800_000_000`（2026-10-24 00:00 UTC，与基线一致），
新增 `card_demo_timestamp_is_frozen_for_golden` 断言
`format_date(DEMO_CARD_TS_MS) == "2026-10-24"`；真实笔记仍用 `now_ms()`，
不改 `NoteCard` 行为，不重拍基线。`card-slash` 单跑 `--tol 0` 0 差异；
阶段门 62/62、763 tests（见 2.11 末段同一份日志）。证据：
`shots/x100/X20/traces/caretaker-card-demo-date-2026-10-25.md`、
`caretaker-stage-b-gate-card-date-fail-2026-10-25.log`（修复前失败）、
`caretaker-card-demo-date-{context-baseline,baseline-crop,drift-crop}-2026-10-25.png`。

**本轮未做**：其他演示日期（如笔记库 `BASE_TS`）已固定但未逐场景核对；
真实用户笔记的日期显示不受影响，也未实拍。

### 2.13 门后维护（2026-10-25 09:09，不影响门结论）

2.11 / 2.12 的「浮层是第一键盘主人」还留了两个页面型缺口，均属 Stage B
修复，不动外观、不新建 golden：

- **开始页 × 标题栏菜单**（`app_methods_1.rs`）：开始页裸键（Esc / Enter /
  1–5）在 Memory 菜单开着时仍归 `window_shortcuts`。1440×1000 / ppp 1.0 /
  `MW_START_DEMO=1` 实拍：开「文件」菜单后按 `1`，菜单背后的开始页被换成
  「思维导图」模板（122,992 B），trace 焦点变 `canvas`；按 `Esc` 被
  `window_shortcuts` 先消费，菜单收不到（202,498 B，与对照图一致）。修复：
  新增纯函数 `home_bare_keys_owned(menu_open, help_open)`，菜单或速查表在
  前台时整段跳过。修复后 `1` 保留开始页并收起菜单（169,377 B），`Esc`
  产生 `popup_menu key=escape` 并收起菜单（169,377 B）。单测
  `start_page_bare_keys_yield_to_foreground_layers`；脚本
  `start-menu-{open,input,esc}.txt`。
- **笔记库 × 模态**（`note_library.rs` / `app_methods_3.rs`）：库页自绘整页，
  Esc / ↑↓ / Enter 直接读 `ctx.input`，绕过 egui 焦点。1080×720 / ppp 1.5 /
  `MW_LIBRARY_DEMO=1 MW_DIALOG_DEMO=1` 实拍：弹窗开着按 `↑`，暗幕背后预览
  从「键盘色板验证」切到「视图跟随」；再按 `Enter` 直接打开阅读页
  （102,116 B）。修复：新增纯函数 `page_keys_owned(top_modal, dialog_demo)`，
  `NoteLibrary::show` 增加 `keys_owned` 门。修复后 `↑` 不再移动预览，
  `Enter` 落到弹窗默认「确定」并关窗（trace `dialog demo closed` /
  `dialog demo action 1`），库页保留（264,292 B）。单测
  `page_keys_yield_to_foreground_modals`；脚本 `library-modal-input.txt`。

复跑阶段门：**31 组 × 2 = 62/62** 全 0 差异、**765 tests / 0 failed**、
fmt 干净、mw-shell clippy 仅 3 条历史、release `23,470,592 B` / SHA256
`78A01487…5F49DE`，结论仍为
`MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`
（started 2026-10-25 09:05:59 +08:00，finished 09:08:57 +08:00）。
证据：`shots/x100/X20/traces/caretaker-start-menu-2026-10-25.md`、
`caretaker-library-keys-2026-10-25.md` 及同前缀 trace/PNG、
`caretaker-stage-b-gate-start-menu-library-2026-10-25.log`。

**本轮未做**：开始页速查表（F1）只有纯函数门 + 单测，未单独实拍；
笔记库的其它页面型浮层（阅读页）未扩面。

## 3. Four evidence classes

- **行为**：X11–X19 的行为 trace 全部保留在原轮证据包；X20 的 25 个场景都用
  `MW_GOLDEN_INPUT` 停在稳定帧（`home.txt` / `narrow-menu.txt` /
  `start-scroll.txt`），不截动画中间态。阶段内测试从 681 增至 747，
  新增 66 条，覆盖图标、令牌、菜单状态机、弹窗焦点、开始页布局、标签条、
  保存六态、打开失败动作映射与 WCAG AA。
- **视觉**：X20 的 25 张实拍逐张记录逻辑尺寸、DPI、主题和状态，物理像素
  25/25 精确，并拼成 `shots/x100/X20/actual/contact-sheet.png`。参考来自
  `shots/x100/X04/` 归档的 Xmind 26.5.1107 截图与 `docs/16`。
- **回归**：31 组 golden 双跑 0 差异；新增 `shell-narrow`、
  `shell-ink-compact`、`menu-narrow`、`start-scroll`、`start-tab-focus`、
  `tabs-overflow`、`tabs-wheel`、`tabs-wheel-line`、`tabs-modal-input`、
  `menu-canvas-click` 十份新基线。2.11 / 2.12 不改外观，30 余份既有基线
  逐字节未变、未被 `--update` 改写。诊断并重拍
  4 份既有基线，但只有
  `theme-sand` 的字节真正变化（强调色漂移 0.552%/最大 2）；
  `dialog`（运行态 4.016%/最大 2）以及 `opening` / `open-error`
  在冻结 egui 时间动画后重拍，像素与各自既有基线逐字节一致。
- **工程**：fmt、clippy、765 tests、release、31×2 golden 与
  `scripts/x100-evidence.ps1 -Check` 全部并入一键机器门；日志在
  `target/x100/`，公开副本在 `shots/x100/X20/traces/`。

## 4. Three biggest improvements

1. **外壳从「各画各的」变成一套系统**：设计令牌 → 30 个矢量图标 →
   `ui_kit` v2 → 标题栏/工具栏/缩放条/状态区/菜单/弹窗/标签条/开始页，
   几何、字阶、描边、焦点环和命中区共用同一来源；本地控件清零，裸字号有
   防回退测试。四档 DPI 下控件尺寸保持屏幕恒定，不再随缩放变小。
2. **可验证的设计系统**：13 → 31 组 golden（门时 23，门后维护后 31），
   覆盖四主题、四档 DPI、五种窗口
   和开始页/菜单/弹窗/标签/加载/失败状态；离屏动画冻结消除了遮罩/弹窗的
   ±1 色阶抖动；色点排布、sand 强调色和 WCAG AA 都有测试兜底。
3. **状态与遮罩的语义收口**：统一弹窗壳 + 焦点陷阱 + Esc 路由 + 层级互斥；
   六态保存有「保存中」中间帧；打开两段式并带失败重试；开始页键盘焦点闭环；
   同一时刻只有一个指针/键盘主人。这些不是「多画了几个按钮」，而是把
   状态机钉在了界面上。

## 5. Three biggest residual gaps

1. **状态 × DPI 未全交叉**：画布外壳覆盖 4 档 DPI 和 5 种窗口，但标签、
   菜单、弹窗、加载/失败、大纲、检查器只在 ppp 1.5 实拍；1620×1080 @ 2.0
   需要 3240×2160 物理像素，超出本机屏幕。2.0 实际只覆盖 640×460 与
   800×600。
2. **开始页滚动没有可见提示**：1440×900 @ 1.25 下第二排模板被裁掉；
   640×460 可以滚到，但没有滚动条或渐隐，用户不一定知道还能往下。P1。
   **已于 2026-10-25 04:07 门后维护修复**（见 2.3：4px 滑块 + 上下渐隐）。
3. **画布内容本身还没进入 Stage B 范围**：节点卡片、分支线、字阶、角标和
   画布状态表是 X21–X30 的任务。外壳已经统一，但画布内容的层级与密度仍是
   Stage C 的主要风险；本轮也没有做动效时长/帧率测量。

## 6. P0 / P1

**P0：无未解释项。** 机器门全部通过，没有阻塞 Stage C 的已知缺陷。

**P1（不阻塞阶段门，但应在 Stage C 同期处理）**：

- ~~开始页窄窗/低高度下的滚动 affordance~~ **已修（2026-10-25 04:07，见 2.3）**。
- 2.0 DPI 大窗口的实拍能力：需要更高分辨率宿主，或在 CI 用虚拟显示；
  在此之前不能声称「全 DPI 全覆盖」。
- 本轮没有启动 Xmind 做实时侧对侧；视觉判词只基于 X04 归档参考。
- ~~窄窗右键菜单（非应用菜单）的边缘翻转没有实拍~~ **已修并实拍
  （2026-10-25 04:47，见 2.4；`menu-narrow` 第 25 组 golden）**；遗留
  滑块不可拖拽与 200% DPI 未实拍，见 2.4 末段。
- ~~标签条横向溢出时没有可见提示、切页也不把活动标签滚回视野~~ **已修并实拍
  （2026-10-25 05:16，见 2.5；`tabs-overflow` 第 26 组 golden）**；遗留
  ppp 2.0 未实拍，见 2.5 末段。
- ~~标签条滚轮位移随标签数放大（映射写在标签循环里，9 标签＝9 倍）~~
  **已修（2026-10-25 05:46，见 2.6；`tabs-wheel` 第 27 组 golden）**；遗留
  ppp 2.0 滚轮与真实触控板未采样，见 2.6 末段。
- ~~标签条滚轮忽略 `MouseWheelUnit`，真实鼠标一格（Line ±1）只走 1px~~
  **已修（2026-10-25 06:15，见 2.7；`tabs-wheel-line` 第 28 组 golden）**；
  遗留 ppp 2.0 与真实鼠标采样，见 2.7 末段。
- ~~开始页键盘焦点落到首屏之外时不滚入视野（640×460 下第 7 次 Tab 焦点环
  画在屏幕外）~~ **已修（2026-10-25 06:44，见 2.8；`start-tab-focus`
  第 29 组 golden）**；遗留 Shift+Tab 未单独实拍与无真实键盘/读屏采样，
  见 2.8 末段。
- ~~模态/菜单打开时标签条仍收裸输入：滚轮会滚动标签、拖拽会重排并换掉
  活动文档，`confirm_close_tab` 按下标可能关错标签~~ **已修
  （2026-10-25 07:17，见 2.9；`tabs-modal-input` 第 30 组 golden）**；
  遗留其余模态未逐个实拍，见 2.9 末段；文件菜单的修复前 A/B 已由 2.10 补齐。
- ~~标题栏菜单打开时第一下画布点击会穿透：菜单收起的同时选中节点~~
  **已修（2026-10-25 07:52，见 2.10；`menu-canvas-click` 第 31 组 golden）**；
  遗留方向键无独立脚本、其他 Memory 弹层未逐个对照，见 2.10 末段。
- ~~模态/标题栏菜单打开时全屏大纲仍收裸键盘：`Delete` / `Enter` / `Tab`
  会在暗幕背后删节点、加同级、缩进~~ **已修（2026-10-25 08:30，见 2.11；
  无新 golden，`outline` 三组逐字节不变）**；遗留行内编辑提交路径未实拍，
  见 2.11 末段。
- ~~`card-slash` 金样日期取墙钟，本地 08:00 UTC 跨日后每天漂一天~~
  **已修（2026-10-25 08:34，见 2.12；冻结演示时间戳，无基线重拍）**；
  遗留其他演示日期未逐场景核对，见 2.12 末段。
- ~~开始页在标题栏菜单开着时仍收裸键：`1` 在菜单背后新建模板，`Esc`
  被快捷键层吃掉、菜单关不掉~~ **已修（2026-10-25 09:09，见 2.13；
  无新 golden）**；遗留速查表只做纯函数门、未实拍，见 2.13 末段。
- ~~笔记库页在模态弹窗下仍收裸键：`↑` 移动暗幕背后的预览，`Enter`
  直接打开卡片阅读页~~ **已修（2026-10-25 09:09，见 2.13；无新 golden）**；
  遗留阅读页等其它页面型浮层未扩面，见 2.13 末段。
- 真实运行中的淡入淡出未测量；golden 冻结动画只保证截图确定性。

## 7. Human gate outcome

**结果：未通过（2026-10-25）。** 静态外壳（标题栏、菜单、弹窗、标签条、开始页）
方向有进展，但用户明确判定三个内容界面不合格：

1. 全屏大纲：编辑与 UI 都不够简洁，控件感重。
2. 卡片笔记页：缺少克制、精致的写作表面。
3. 笔记库：像玩具，不像成熟产品。

因此：

- X20 保持 `implemented`，不记为 `verified`；Stage C 不开始。
- 新证据与修复进入 Stage B2，不再往 X20 目录追加“门后维护”。
- B2 的设计合同是 `docs/28`，执行合同是 `docs/29`；每批末端重新人工验收。
- X20 的 `shots/x100/X20/` 证据只读保留，作为外壳稳定性基线，不作为内容界面基线。

Stage C 的门槛没有降低；只有 G-B2-4 通过后，才重新讨论 X21–X30 的开工。

## 8. Commands

```powershell
powershell -File scripts/x100-stage-b-gate.ps1
powershell -File scripts/x100-evidence.ps1 -Round X20 -Check
cargo fmt --all -- --check
cargo clippy -p mw-shell --all-targets
cargo test --workspace -- --test-threads=1
cargo golden --ppp 1.5 --tol 0 --warmup 16 --dir shots/golden/shell-narrow
cargo golden --ppp 1.5 --tol 0 --warmup 16 --dir shots/golden/shell-ink-compact
$env:MW_CONTEXT_DEMO='1'; $env:MW_WINDOW_SIZE='640x460'
cargo golden --ppp 1.5 --tol 0 --warmup 8 --dir shots/golden/menu-narrow
$env:MW_TABS_DEMO='9'; $env:MW_WINDOW_SIZE='640x460'
$env:MW_GOLDEN_INPUT='shots/golden/inputs/tabs-overflow.txt'
cargo golden --ppp 1.5 --tol 0 --warmup 30 --dir shots/golden/tabs-overflow
$env:MW_GOLDEN_INPUT='shots/golden/inputs/tabs-wheel.txt'
cargo golden --ppp 1.5 --tol 0 --warmup 16 --dir shots/golden/tabs-wheel
$env:MW_GOLDEN_INPUT='shots/golden/inputs/tabs-wheel-lines.txt'
cargo golden --ppp 1.5 --tol 0 --warmup 16 --dir shots/golden/tabs-wheel-line
$env:MW_DIALOG_DEMO='1'
$env:MW_GOLDEN_INPUT='shots/golden/inputs/tabs-modal-input.txt'
cargo golden --ppp 1.5 --tol 0 --warmup 24 --dir shots/golden/tabs-modal-input
$env:MW_OUTLINE='1'; $env:MW_OUTLINE_FOCUS='1'; $env:MW_WINDOW_SIZE='1080x720'
$env:MW_DIALOG_DEMO='1'
$env:MW_GOLDEN_INPUT='shots/golden/inputs/outline-modal-input.txt'
cargo golden --ppp 1.5 --tol 0 --warmup 24 --dir target/x100/probe-outline-modal/after2-modal
$env:MW_GOLDEN_INPUT='shots/golden/inputs/outline-menu-input.txt'
cargo golden --ppp 1.5 --tol 0 --warmup 24 --dir target/x100/probe-outline-modal/clean-menu
$env:MW_CARD_DEMO='1'; $env:MW_CARD_SLASH='1'
cargo golden --ppp 1.5 --tol 0 --warmup 3 --dir shots/golden/card-slash
```

本轮没有 git 提交；工作区保留全部未提交改动，供人工验收时直接复跑。
