# X20

## Scope

Stage B 的收口轮：把 X11–X19 的设计系统放进真实外壳，用 25 个场景覆盖
snow / ink / mist / sand 四套主题、100%–200% 四档 DPI、1620×1080 / 1440×900 /
1080×720 / 800×600 / 640×460 五种窗口，以及开始页、标签、菜单、弹窗、加载、
打开失败、大纲、检查器和窄窗应用菜单。只做视觉门、确定性修复和证据，不改
`.zmind` 版本，不新增依赖、crate 或构建工具，不进入 Stage C。

本轮机器门已通过；**人工视觉门尚未确认**，因此 manifest 状态写
`implemented`，不写 `verified`。

## Changes

- **25 场景外壳矩阵**：`target/x20-matrix.sh` 在隔离 `APPDATA` /
  `LOCALAPPDATA` 下逐场景 `--update --ppp N --tol 0 --warmup N` 实拍。宿主为
  1920×1200 物理 / 1280×800 逻辑（系统缩放 1.5），用
  `MW_WINDOW_SIZE = 目标逻辑宽 × ppp / 1.5` 补偿，使离屏物理像素恰好等于
  「目标逻辑尺寸 × ppp」。25/25 尺寸逐张回读校验通过（`traces/matrix-captures.log`），
  拼成 `actual/contact-sheet.png`（1896×1480，5 列，中文场景名）。
- **golden 确定性修复**：egui 的 `Area` 淡入按墙钟时间推进，离屏帧快慢会让
  遮罩/弹窗停在中间透明度，`dialog` / `opening` / `open-error` 在 warmup 8
  下出现 ±1 色阶抖动，`opening` 甚至整幅漂移。`Harness::prepare` 现在每帧
  UI 前把 `animation_time` 归零，金样只截终态；冻结后 `opening` /
  `open-error` 重拍结果与 X19 基线逐字节一致，`dialog` 重拍后双跑稳定。
- **sand 强调色过 AA**：`#a95f45` 对 `ui.panel` 只有 4.46:1，主按钮 AA
  压不住。改为 `#a45c43`（4.69:1），同时把 OKLCH 色相留在原位（39.77°），
  避免色条预视与应用结果差 3 个色阶。新增
  `theme_ui_contrast_meets_wcag_aa`：四套主题的正文/面板、正文/背景、
  辅助文字/面板、主按钮、强调色/面板（≥3:1）全部断言；色相往返测试的失败
  消息补上主题 id。
- **检查器色点均分**：11 颗强调色点原来自然换行，默认面板宽度会留下最后一排
  单颗。现在先按可用宽度算每排容量，再让每排颗数
  `ceil(n / rows)` 均分（默认 6 + 5），面板变宽时自然回到一排；窄面板仍
  允许降级为更多排。
- **golden 扩到 23 组比较**：新增 `shell-narrow`（640×460 逻辑、ppp 1.5、
  `home.txt` 输入、warmup 16，960×690）与 `shell-ink-compact`
  （1080×720、ink、ppp 1.5、warmup 16，1620×1080）。`scripts/ci.ps1`
  同步接入并隔离用户数据目录。诊断并重拍了 4 份既有基线，但**实际只有
  `theme-sand` 的字节发生变化**（强调色）；`dialog` / `opening` /
  `open-error` 冻结动画后重拍，像素与各自既有基线逐字节一致——此前的
  `dialog` 4.016% 与 `opening` / `open-error` 大面积差异是运行态遮罩透明度，
  不是设计变更。
- **Stage B 机器门**：新增 `scripts/x100-stage-b-gate.ps1`（UTF-8 BOM，
  Windows PowerShell 5.1 可直接跑），覆盖 X11–X19 证据包完整性、全部
  `x100-*.ps1` 语法、`cargo fmt --check`、只针对 X20 改动文件的 clippy
  新增告警（`theme.rs:22` 的 `self_named_constructors` 等历史告警按精确
  位置白名单放行）、全量串行测试（≥747）、release 构建、23 组 golden 双跑。
  日志落在 `target/x100/`。
- **门脚本自身缺陷修复**：首轮门把所有 golden 误报为 FAIL——函数把 cargo 的
  stdout 与 `$LASTEXITCODE` 一起返回，数组与 0 比较永远为真，且输出被吞。
  改为显式收集输出、只返回 `[int]$LASTEXITCODE`；随后重跑，46/46 golden
  全绿。该次失败与重试记录见本文件「Verification」。

## Verification

### 四类证据

- **行为**：矩阵场景都用 `MW_GOLDEN_INPUT` 停在稳定帧，不截动画中间态；
  `home.txt`（wait 6 / Home / wait 6）让相机回中，
  `narrow-menu.txt`（wait 3 / 点应用菜单 / wait 6 / 指针移开）避免悬停残留，
  `start-scroll.txt`（滚轮两次 -240）证明 640×460 开始页第二排模板可滚动
  到达。文档仍只走 `mw_core::Command`，本轮没有新增文档写路径。
- **视觉**：25 张实拍逐张绑定「逻辑尺寸 + DPI + 主题 + 状态」，尺寸回读
  25/25 精确；参考图取自 `shots/x100/X04/`
  （`canvas.png`、`start-page.png`、`node-context-menu.png`、
  `export-dialog.png`、`outline.png`、`inspector-style.png`）与
  `docs/16-视觉重塑基准.md`。本轮没有启动 Xmind，参考图是 X04 已归档的
  26.5.1107 截图。
- **回归**：23 组 golden × 2 次 `--tol 0` 全部「无超容差差异」（46/46，
  `traces/stage-b-gate.log`）。已知漂移：`theme-sand` 0.552% 像素、
  最大通道差 2（强调色 0xa9→0xa4）；`dialog` 4.016%、最大差 2、
  `opening` / `open-error` 接近全图漂移，都是动画未到终值导致的运行态
  差异，冻结后归零，重拍像素与既有基线一致。新增两份基线双跑 0 差异。
- **工程**：`cargo fmt --all -- --check` 干净（`traces/fmt-final.txt`
  为空）；`cargo clippy -p mw-shell --all-targets` 仅 3 条
  `export_pdf.rs` 历史告警，X20 改动文件新增 0（`traces/clippy-final.txt`，
  mw-core/mw-render 历史告警与 X19 相同）；全量串行测试 **747 passed /
  0 failed**（`traces/tests-final.txt`）；release 构建成功
  （`target/release/mw-shell.exe` 23,454,720 B，SHA256
  `0f26bbb9…3dd72b`，`traces/hashes.txt`）。

### 矩阵（25/25，物理像素）

| 场景 | 逻辑窗口 | DPI | 物理像素 | 主题 / 状态 |
| --- | --- | --- | --- | --- |
| shell-snow-1620x1080-p100 | 1620×1080 | 1.00 | 1620×1080 | snow · 画布 |
| shell-snow-1440x900-p125 | 1440×900 | 1.25 | 1800×1125 | snow · 画布 |
| shell-snow-1080x720-p100 | 1080×720 | 1.00 | 1080×720 | snow · 画布 |
| shell-snow-1080x720-p125 | 1080×720 | 1.25 | 1350×900 | snow · 画布 |
| shell-snow-1080x720-p150 | 1080×720 | 1.50 | 1620×1080 | snow · 画布 |
| shell-snow-640x460-p150 | 640×460 | 1.50 | 960×690 | snow · 窄窗 |
| shell-snow-640x460-p200 | 640×460 | 2.00 | 1280×920 | snow · 窄窗 |
| shell-snow-800x600-p200 | 800×600 | 2.00 | 1600×1200 | snow · 窄窗 |
| shell-ink-1080x720-p150 | 1080×720 | 1.50 | 1620×1080 | ink · 画布 |
| shell-ink-1440x900-p125 | 1440×900 | 1.25 | 1800×1125 | ink · 画布 |
| shell-ink-640x460-p200 | 640×460 | 2.00 | 1280×920 | ink · 窄窗 |
| theme-mist-1440x900-p125 | 1440×900 | 1.25 | 1800×1125 | mist · 画布 |
| theme-sand-1440x900-p125 | 1440×900 | 1.25 | 1800×1125 | sand · 画布 |
| tabs-ink-1080x720-p150 | 1080×720 | 1.50 | 1620×1080 | ink · 三标签 |
| menu-ink-1080x720-p150 | 1080×720 | 1.50 | 1620×1080 | ink · 右键菜单 |
| dialog-ink-1080x720-p150 | 1080×720 | 1.50 | 1620×1080 | ink · 弹窗 |
| start-ink-1440x900-p125 | 1440×900 | 1.25 | 1800×1125 | ink · 开始页 |
| start-640x460-p200 | 640×460 | 2.00 | 1280×920 | snow · 开始页 |
| opening-ink-960x680-p150 | 960×680 | 1.50 | 1440×1020 | ink · 正在打开 |
| open-error-ink-960x680-p150 | 960×680 | 1.50 | 1440×1020 | ink · 打开失败 |
| outline-ink-1080x720-p150 | 1080×720 | 1.50 | 1620×1080 | ink · 大纲 |
| inspector-snow-1080x720-p150 | 1080×720 | 1.50 | 1620×1080 | snow · 检查器 |
| inspector-ink-1080x720-p150 | 1080×720 | 1.50 | 1620×1080 | ink · 检查器 |
| narrow-menu-snow-640x460-p150 | 640×460 | 1.50 | 960×690 | snow · 应用菜单 |
| start-640x460-p200-scroll | 640×460 | 2.00 | 1280×920 | snow · 开始页滚动后 |

### 人工判词（待用户确认）

机器门只能证明尺寸、稳定性和对比度。需要人眼确认的三点：

1. `actual/contact-sheet.png` 总体密度是否接近 Xmind，而不是「控件都在但气质不对」。
2. `inspector-snow-1080x720-p150.png` / `inspector-ink-1080x720-p150.png`
   的色点 6+5 两排与上下留白是否自然。
3. `shell-snow-640x460-p150.png` 与 `narrow-menu-snow-640x460-p150.png`
   在窄窗下是否仍有可点击感，标题栏是否拥挤。
4. `golden/tabs-overflow-shell-ppp1.50.png` 的左右 20px 渐隐是否自然，
   「文档 6」居中后是否还能看出标签条可继续横向滚动（门后维护 ⑤）。

## Known gaps

- 本轮未启动 Xmind 做实时侧对侧；视觉参考来自 X04 归档图，判词只能证明
  「相对既有参考不退化」，不能证明「与当前 Xmind 版本逐像素接近」。
- 状态 × DPI 不是全交叉：画布外壳覆盖 4 档 DPI 和 5 种窗口，但标签、菜单、
  弹窗、加载/失败、大纲、检查器只在 ppp 1.5 实拍；开始页只有 1.25 / 2.0。
- 1620×1080 逻辑在 ppp 2.0 需要 3240×2160 物理像素，超出本机 1920×1200
  屏幕；同理 1080×720 @ 2.0 也无法实拍。因此 2.0 只覆盖 640×460 与
  800×600，1620×1080 只覆盖 ppp 1.0。
- 开始页在 1440×900 @ 1.25 下第二排模板被裁掉；640×460 滚动可到达，但滚动
  没有可见滚动条或渐隐提示，用户不一定知道还能往下。列为 P1。**已于 2026-10-25
  门后维护修复**（见文末「门后维护（2026-10-25 04:07）」）。
- 矩阵是静态终态，没有覆盖悬停、焦点环、拖拽中的外壳状态；这些由 X13–X19
  的 golden 与 trace 覆盖，但不在本轮 25 张图里。
- 冻结 `animation_time` 只服务 golden；真实运行仍有淡入淡出，本轮没有对动效
  时长、曲线和掉帧做测量。
- 窄窗 `narrow-menu` 截的是应用菜单（标题栏触发器），不是画布右键菜单；
  640×460 下的右键菜单边缘翻转未在本轮实拍。**已于 2026-10-25 04:47
  门后维护修复并实拍**（见文末「门后维护（2026-10-25 04:47）」）。
- 检查器色点均分只验证了默认面板宽度（1080×720 下 6+5）；更窄或更宽面板
  的排数变化没有截图。
- 标签条横向溢出（门后维护 ⑤）只在 640×460 / ppp 1.5 实拍；200% DPI、
  触控板横向手势与 Shift+滚轮没有单独证据。**滚轮映射已在 2026-10-25 05:46
  门后维护修复并实拍**（见文末「门后维护（2026-10-25 05:46）」），但 ppp 2.0
  组合与真实触控板采样仍未覆盖。
- 开始页键盘焦点在 640×460 下会落到首屏之外且不滚动（第 7 次 Tab 到「模板」
  时焦点环不可见），此前的 Tab 顺序验收只检查了「可聚焦」。**已于 2026-10-25
  06:44 门后维护修复并实拍**（见文末「门后维护（2026-10-25 06:44）」）；
  Shift+Tab 反向、ppp 2.0 与真实键盘/读屏仍未覆盖。
- 门脚本 clippy 白名单按精确位置放行 `mw-core/src/theme.rs:22` 的
  `self_named_constructors`；同一文件其他行新增告警仍会失败。
- 全部证据来自 demo / 隔离目录，不包含真实用户文档、IME 组合输入或长时间
  运行后的内存/帧率数据。

## 门后维护（2026-10-25 03:30）

归档脚本 `inputs/narrow-menu.txt` 曾重放不出菜单（3 次同哈希 `26A6182B…04DEF3`，
与归档实拍差 49.066%）。根因不在产品外观：golden 点击原语把指针移入与按下放在
同一帧，egui 读上一帧控件矩形做命中，离屏脚本首帧点击被丢弃。已在
`golden_input.rs` 拆帧（移入一帧 → 按下 → 松开），单测钉住。修复后同一脚本重放
与 `actual/narrow-menu-snow-640x460-p150.png` 逐字节一致（`79C4E412…810DBB`，
0 px）；X19 标签菜单/中键关闭回归不变；fmt / clippy（仅 3 条历史）/ 747 tests /
23 组 golden × 2 全部通过。产品代码、外观与本目录 `actual/` 均未改；
release 重建为 23,455,744 B / SHA256 `E67D79A6…03221F`。
详见 `traces/caretaker-input-click-determinism-2026-10-25.md`。

## 门后维护（2026-10-25 04:07）

开始页纵向溢出时现在静止可见：`start.rs` 自绘 4px 可拖拽滑块（静止 alpha 130 /
悬停 200 / 拖拽 235、最短 28px、抓取偏移跟手）+ 40px 上下渐隐；命中区左右各扩
6px，非 focusable，不抢 X18 Tab 焦点；内置浮动滚动条改为 `AlwaysHidden`。未新增
依赖/线程/crate，未改文档模型，未越 X20 人工门。

`start` 1440×1000 逐字节 0 差异。`start-narrow` 重拍：`5BE44212…11E8` →
`2BDBDCA1…5523`，漂移 6.419%，全在滑块与渐隐带。新增第 24 组 golden
`start-scroll`（640×460、ppp 1.5、拖到底，`AEC0D24D…97B7A`）；新基线双跑
`--tol 0` 0 差异。阶段门复跑 24 组 × 2 = 48/48、750 tests / 0 failed、fmt 干净、
clippy 新增 0，结论仍为 `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`
（finished 2026-10-25 04:06:56 +08:00）。本目录 `actual/` 的 25 张门时实拍未改；
新增证据见 `traces/caretaker-start-scroll-2026-10-25.md`、
`caretaker-start-scroll-2026-10-25.png`、`caretaker-start-narrow-2026-10-25.png`、
`caretaker-start-scroll-replay-2026-10-25.trace`、
`caretaker-stage-b-gate-start-scroll-2026-10-25.log`。

## 门后维护（2026-10-25 04:47）

窄窗画布右键菜单在 640×460 下超出窗口：19 项自然高 477，卡片从 y=8 画到
y=485，末两项「折叠」「聚焦」鼠标与键盘都不可达，违反 X16「任何一级都不得
出内容区」。修复只动 `crates/mw-shell/src/menu.rs`：卡片高度夹到内容区
（`card_height`，根/子菜单同口径），装不下时层级内容改 `ScrollArea`
（原生条隐藏、保留滚轮、自绘 4px 静止滑块 alpha 130 / 最短 28px）；
键盘导航把出视口的高亮行滚到中央；不溢出的菜单保持原路径，像素零漂移。
未新增依赖/线程/crate，未改文档模型，未越 X20 人工门。

`menu`（1620×1080）修复前后逐字节一致（`fbfaf8ce…3342610`），base 三档
ppp 亦一致。新增第 25 组 golden `menu-narrow`（640×460、ppp 1.5、warmup 8，
960×690，`32b20b46…4f255c75`），双跑 `--tol 0` 0 差异；键盘 End 到达末项
「聚焦」的实拍 `ac13bdb0…b1c91a440` 与 trace 归档。复跑阶段门
**25 组 × 2 = 50/50** 全 0 差异、**753 tests / 0 failed**、fmt 干净、
mw-shell clippy 仅 `export_pdf.rs` 3 条历史，结论仍为
`MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`
（finished 2026-10-25 04:47:50 +08:00）；release `23,505,408 B` /
SHA256 `3b6394be…e8e5a47`。本目录 `actual/` 的 25 张门时实拍未改；证据见
`traces/caretaker-menu-overflow-2026-10-25.md`、
`caretaker-menu-overflow-{before,after,end}-2026-10-25.png`、
`caretaker-menu-overflow-end-2026-10-25.trace`、
`caretaker-stage-b-gate-menu-overflow-2026-10-25.log`。

## 门后维护（2026-10-25 05:16）

X19 只规定标签条「溢出时能横向滚」，没有规定「看得出还能滚、切页要滚回来」。
34px 标签条没有滚动条或渐隐；`Ctrl+Tab`、关闭左侧标签或恢复会话后，活动标签
若在视野外不会回滚，用户只看到一排非活动标签。640×460 下 6 个最小宽标签已有
674px 内容、600px 视口，9 个标签时内容 1004px，最后一个直接藏在加号区后。

修复只动 `crates/mw-shell`：`tab_strip::edge_fade_strengths` 纯几何（内容 >
视口 +0.5 才画，左右各 20px，强度按 `offset/20` 与剩余量夹取 ×0.9）+ 2 组
单测；`Shell.tab_seen` 记 `(活动下标, 标签数, 标签条宽)`，键变化且活动标签
不完全可见时 `scroll_to_rect_animation(.., Align::Center, ScrollAnimation::none())`
即时居中，滚轮自由滚动不回拉，标签条隐藏时清空记忆；渐隐画在标签之上、加号
之下，不接管命中。不用 4px 滑块，是因为它会和 2px 选中下划线、1px 分隔线抢
同一条边。`MW_TABS_DEMO=N` 仅在内存追加标签。未新增依赖/线程/crate，未改
文档模型，未越 X20 人工门。

`tabs`（1080×720 三标签）逐字节不变（`F11988FD…AD2553F`），其余 24 组哈希
不变。新增第 26 组 golden `tabs-overflow`（640×460、ppp 1.5、
`MW_TABS_DEMO=9`、`tabs-overflow.txt` 26 步 `Ctrl+Tab` ×5、warmup 30，960×690，
64,575 B，`96D62007…48300A90`），画面「文档 6」居中且左右渐隐同时可见，
双跑 `--tol 0` 0 差异；trace 只有第 5 次切页触发
`tab_reveal active=5 tab=558.0..666.0 view=0.0..600.0`，前 4 次目标完整可见、
无横向跳动。复跑阶段门 **26 组 × 2 = 52/52** 全 0 差异、**755 tests / 0 failed**、
fmt 干净、mw-shell clippy 仅 `export_pdf.rs` 3 条历史，结论仍为
`MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`
（finished 2026-10-25 05:16:19 +08:00）；release `23,457,792 B` /
SHA256 `BAED8EA0…DCA23965`。本目录 `actual/` 的 25 张门时实拍未改；新基线
副本在 `golden/tabs-overflow-shell-ppp1.50.png`，证据见
`traces/caretaker-tabs-overflow-2026-10-25.md`、
`caretaker-tabs-overflow-2026-10-25.{png,trace}`、
`caretaker-stage-b-gate-tabs-overflow-2026-10-25.log`。

## 门后维护（2026-10-25 05:46）

上一节的滚轮横向映射存在实现缺陷：它写在 `for (i, (title, dirty)) in labels`
循环体内。`Ui::scroll_with_delta` 只把增量累进本帧 pass state，`ScrollArea`
在本帧收尾一次取走，因此同一帧滚轮增量被每个标签各应用一次——9 个标签就是
9 倍，标签条直接冲到头；滚动速度随标签数变化。命中判定还用了内容
`ui.max_rect()`（随内容变宽），而不是 600px 滚动视口。

修复只动 `crates/mw-shell`：新增 `tab_strip::wheel_scroll_x` 纯函数，把一帧内
`Event::MouseWheel` 的 `dx + dy` 合成一个位移（Ctrl/Alt/Win 组合丢弃、
≤0.5px 视为抖动），调用点移到标签循环外、每帧最多一次；不再用
`smooth_scroll_delta`（按真实帧间隔摊到后续帧，离屏回放依赖机器速度），改
`scroll_with_delta_animation(.., ScrollAnimation::none())`，与 X19 揭示同口径、
无惯性；命中区改为 `scroll_rect`（右端 40px 加号区除外），触控板横滑与
Shift 滚轮走同一条 `dx + dy`；`MW_TRACE` 新增 `tab_scroll x= content= view=`。
未新增依赖/线程/crate，未改文档模型，未越 X20 人工门。

证据：修复前同一脚本（`hover 300 65` + `scroll 0 -100` + `wait 12`）trace 有
**108 行** `tab_wheel`（9 标签 × 12 帧），实拍冲到最右；修复后 **1 行**
`dx=-100.00`、`tab_scroll x=100.0 content=1006.0 view=600.0`，8 标签
（content 896）同脚本仍为 `x=100.0`。修复前后 960×690 差异 14,328 px =
2.163%，包围盒 `x[1..900] y[77..119]`，全部落在标签条。`tabs` /
`tabs-overflow` 逐字节未变；新增第 27 组 golden `tabs-wheel`（640×460、
ppp 1.5、`MW_TABS_DEMO=9`、warmup 16，960×690 / 111,542 B /
`d5ef9b4a…0261b910`），双跑 `--tol 0` 0 差异。复跑阶段门
**27 组 × 2 = 54/54** 全 0 差异、**756 tests / 0 failed**、fmt 干净、
mw-shell clippy 仅 `export_pdf.rs` 3 条历史，结论仍为
`MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`
（finished 2026-10-25 05:46:09 +08:00）；release `23,458,816 B` /
SHA256 `6B23D098…79901116`。本目录 `actual/` 的 25 张门时实拍未改；
新基线副本在 `golden/tabs-wheel-shell-ppp1.50.png`，证据见
`traces/caretaker-tabs-wheel-2026-10-25.md`、
`caretaker-tabs-wheel-{before,after}-2026-10-25.png`、
`caretaker-tabs-wheel-{before,8,9}-2026-10-25.trace`、
`caretaker-stage-b-gate-tabs-wheel-2026-10-25.log`。

## 门后维护（2026-10-25 06:15）

上一节只解决了「一帧应用几次」，没有解决「一个增量是多少」：代码把
`Event::MouseWheel.delta` 直接相加，忽略了 `unit`。egui-winit 对普通鼠标发
`Line`（一格 ±1）、对触控板发 `Point`（已换成点）；egui 自己的 `ScrollArea`
会把 Line 乘 `input_options.line_scroll_speed`（egui 0.37 原生端 40），但标签条
`mouse_wheel: false` 没有原生兜底——真实鼠标滚一格只走 1px，手感等同失灵。
上一节的 golden 脚本用 `scroll`（Point），所以基线完全看不出这个问题。

修复只动 `crates/mw-shell`：`tab_strip::wheel_scroll_x` 改为接收
`(Vec2, MouseWheelUnit, bool)`，`Point` 原样、`Line × line_scroll_speed`、
`Page × 视口高`，换算后再做 0.5px 抖动过滤；调用点从
`ctx.options().input_options.line_scroll_speed` 与 `viewport_rect().height()`
取参数，不硬编码 40；`golden_input` 新增 `scroll_lines` 原语（Line 单位）与
解析单测，`scroll` 保持 Point（触控板语义）。命中区、修饰键过滤、每帧一次、
`ScrollAnimation::none()` 全部不变。未新增依赖/线程/crate，未改文档模型，
未越 X20 人工门。

证据：新增脚本 `tabs-wheel-lines.txt`（`wait 4` / `hover 300 65` /
`scroll_lines 0 -3` / `wait 12`）。修复前 trace `tab_wheel dx=-3.00`、
`tab_scroll x=3.0 content=1006.0 view=600.0`，实拍 113,614 B
`F551E86D…6978BFF`；修复后 `dx=-120.00`、`x=120.0`，实拍 111,498 B
`03E71072…75091E`。前后差异 12,691 px = 1.916%，包围盒
`x[2..894] y[72..122]`（物理像素），全部落在标签条。`tabs`
（`F11988FD…AD2553F`）、`tabs-wheel`（`d5ef9b4a…0261b910`）、
`tabs-overflow`（`96D62007…48300A90`）逐字节未变；新增第 28 组 golden
`tabs-wheel-line`（960×690 / 111,498 B / `03E71072…75091E`），双跑
`--tol 0` 0 差异。复跑阶段门 **28 组 × 2 = 56/56** 全 0 差异、
**758 tests / 0 failed**、fmt 干净、mw-shell clippy 仅 `export_pdf.rs` 3 条
历史，结论仍为 `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`
（finished 2026-10-25 06:15:06 +08:00）；release `23,439,872 B` /
SHA256 `81C2A523…FFEDAE`。本目录 `actual/` 的 25 张门时实拍未改；
新基线副本在 `golden/tabs-wheel-line-shell-ppp1.50.png`，证据见
`traces/caretaker-tabs-wheel-unit-2026-10-25.md`、
`caretaker-tabs-wheel-unit-{before,after}-2026-10-25.png`、
`caretaker-tabs-wheel-unit-{before,after}-2026-10-25.trace`、
`caretaker-stage-b-gate-tabs-wheel-unit-2026-10-25.log`。

## 门后维护（2026-10-25 06:44）

X18 只验收了「开始页所有卡片可 Tab 聚焦」，X20 P1 只验收了「窄窗滚动有
可见反馈」，两者之间漏了「聚焦后必须可见」。640×460 / 720×620 下开始页
内容高于视口，从主操作卡按 7 次 Tab 到「模板 思维导图」时焦点已在首屏之外，
视口纹丝不动——焦点环画在屏幕外，键盘用户看不到自己在哪。egui 的
`ScrollArea` 不会因为普通按钮获得键盘焦点而自动滚动。

修复只动 `crates/mw-shell/src/start.rs`：新增纯函数
`focus_scroll_target(focus, viewport, offset_y, content_h)`，完整可见（含 8px
边距）返回 `None`，上方裁切对齐视口顶 +8px、下方对齐底 −8px、卡片高于视口
时顶对齐，目标夹在 `[0, content_h − viewport_h]`，不溢出或视口高为 0 不滚；
`StartPage` 用 `focus_seen` 只在焦点 id 变化后检查一次，且只认 Tab/Shift+Tab
与页面出现/菜单返回的自动聚焦，鼠标点击与标题栏、菜单触发器的焦点都不触发，
焦点不变不滚（滚轮/拖拽位置不会被抢回）。目标偏移复用自绘滑块已有的
`scroll_to` 下一帧通道，不改 `ScrollArea` 布局、Tab 顺序、卡片几何与焦点环；
新增 3 组单测（`start::tests` 共 13 条），未新增依赖/线程/crate。

证据：脚本 `start-tab-focus.txt`（`wait 4` + 7 次 Tab + `wait 8`）。修复前
trace 只有 focus 变化、最后停在「模板 思维导图」，实拍 960×690 / 153,877 B
`3186D9D1…6E68AE`；修复后 trace
`start_focus_scroll from=0.0 to=41.3` → `41.3 to=93.3` → `93.3 to=335.3`，
实拍 223,988 B / `007C6DD7…7B86F4A9`。前后差异 440,664 px = 66.525%，
`bbox=(1, 72, 959, 654)` 全在开始页内容区。标题栏 (66, 24) 点击复核 15 帧
无滚动事件（154,689 B / `0C74D466…FBD084`）。`start`
（`4EC10E68…B054544F`）、`start-narrow`（`2BDBDCA1…CF655523`）、
`start-scroll`（`AEC0D24D…ACA97B7A`）逐字节未变；新增第 29 组 golden
`start-tab-focus`（640×460、ppp 1.5、`MW_HOME=1 MW_START_DEMO=1`、
warmup 16，960×690 / 223,988 B / `007C6DD7…7B86F4A9`），双跑 `--tol 0`
0 差异。复跑阶段门 **29 组 × 2 = 58/58** 全 0 差异、
**761 tests / 0 failed**、fmt 干净、mw-shell clippy 仅 `export_pdf.rs` 3 条
历史，结论仍为 `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`
（started 2026-10-25 06:43:04 +08:00，finished 06:44:37 +08:00）；release
`23,484,928 B` / SHA256 `33CE7658…33EE8EA`。本目录 `actual/` 的 25 张门时
实拍未改；新基线副本在 `golden/start-tab-focus-shell-ppp1.50.png`，证据见
`traces/caretaker-start-focus-scroll-2026-10-25.md`、
`caretaker-start-focus-scroll-{before,after}-2026-10-25.{png,trace}`、
`caretaker-start-focus-titlebar-2026-10-25.{png,trace}`、
`caretaker-stage-b-gate-start-focus-2026-10-25.log`。

## 门后维护（2026-10-25 07:17）

X17 规定了「同一时刻只有一个指针/键盘主人」，但标签条有两条裸输入路径
绕开 egui 层级命中：滚轮直接用 `pointer_interact_pos()` 判断落点，重排用
`press_origin()` + `interact_pos()` 比对 4px 自己起手。暗幕挡得住点击，
挡不住这两条路径——640×460 / ppp 1.5 / 新建文档对话框打开时，指针在暗幕后
的标签条上滚动出现 `tab_wheel dx=-100.00` 与 `tab_scroll x=100.0`；按住拖
200px 出现 `tab_reorder from=2 slot=5 active=0`，活动文档在弹窗背后被换掉。
`confirm_close_tab` 按**下标**记关闭目标，若在「关闭未保存标签」确认框背后
重排，确认后会关掉另一份标签，可能静默丢掉未保存内容。文件菜单
（Memory popup）不覆盖标签条，滚轮同样会穿过去。

修复只动 `crates/mw-shell/src/app_methods_2.rs`：`tab_bar` 入口新增
`foreground = overlay_state().top_modal().is_some() || dialog_demo ||
egui::Popup::is_any_open(ctx)`，前台存在时取消在途拖拽并停止一切标签输入；
`interactive = !foreground && tab_menu.is_none() && context_menu.is_none()`
统一门控滚轮、重排起手与加号。用 `Popup::is_any_open` 而不是「任意 Area」：
提示气泡是 `OpenKind::Open + hover`，不应把标签条冻住。不改标签几何、顺序、
渐隐、活动标签揭示，不新增依赖/线程/crate，不改文档模型与 `.zmind`。

证据：脚本 `tabs-modal-input.txt`（滚轮 + 220px 拖拽，21 帧、warmup 24）。
修复前滚轮 trace `tab_wheel dx=-100.00` / `tab_scroll x=100.0
content=1006.0 view=600.0`（960×690 / 107,839 B / `9AE4ADD3…A8688B`）；
拖动 trace `tab_reorder from=2 slot=5 active=0` + `focus canvas`
（106,795 B / `901B2361…70B3FBE`）。修复后两条 trace 均 0 条标签事件，
实拍同为 108,398 B / `01ACE0A2…C896648`（与滚轮后图逐字节相同）。前后差异：
滚轮 12,106 px = 1.828%（最大 154，`bbox x[1..899] y[72..122]` 全在标签条）；
拖动 2,767 px = 0.418%（最大 208，`bbox x[354..791] y[88..495]`，含暗幕后
的画布——活动文档确实被换掉）。文件菜单复核：1080×720 打开「文件」后悬停
标签条滚轮，0 条标签事件，菜单保持打开（1620×1080 / 221,502 B /
`87DEB4DE…C18B1A`）。既有 `tabs` / `tabs-overflow` / `tabs-wheel` /
`tabs-wheel-line` 及其余基线 0 差异且未被 `--update` 改写；新增第 30 组
golden `tabs-modal-input`（960×690 / 108,398 B / `01ACE0A2…C896648`），
双跑 `--tol 0` 0 差异。复跑阶段门 **30 组 × 2 = 60/60** 全 0 差异、
**761 tests / 0 failed**、fmt 干净、mw-shell clippy 仅 `export_pdf.rs` 3 条
历史，结论仍为 `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`
（started 2026-10-25 07:15:26 +08:00，finished 07:17:07 +08:00）；release
`23,485,952 B` / SHA256 `38EBF196…41F27DA`。本目录 `actual/` 的 25 张门时
实拍未改；新基线副本在 `golden/tabs-modal-input-shell-ppp1.50.png`，证据见
`traces/caretaker-tabs-modal-isolation-2026-10-25.md`、
`caretaker-tabs-modal-isolation-{before,after}-{wheel,drag}-2026-10-25.{png,trace}`、
`caretaker-tabs-menu-passthrough-2026-10-25.{png,trace}`、
`caretaker-tabs-modal-input-golden-2026-10-25.trace`、
`caretaker-stage-b-gate-tabs-modal-2026-10-25.log`。

## 门后维护（2026-10-25 07:52）

X17 的「同一时刻只有一个指针/键盘主人」在画布上还差一处：右键菜单/标签菜单
走 `overlay_state().context`，已进 `input_blocked`；标题栏菜单（文件/编辑/
视图/插入/帮助/更多）是 egui Memory 弹层，不进 overlay 状态，`input_blocked`
也没看它。1080×720 / ppp 1.5 实拍：打开「文件」菜单后点画布上的「织思」，
菜单收起的同时节点被选中（状态栏「已选：织思」、选中环、节点工具条），
trace `canvas_click blocked=false`——点浮层外面第一下只负责收起浮层，这与
07:17 定下的规则冲突。

修复只动 `crates/mw-shell/src/main.rs`：`input_blocked` 加入
`egui::Popup::is_any_open(&ctx)`（与标签条 `foreground` 同一条谓词），菜单
弹层开着时画布不收指针手势与画布快捷键；读的是上一帧 Memory 状态、而画布
本帧稍后才绘制，收起菜单的那一下正好被挡住，tooltip 不写 Memory 不受影响；
门开时 `handle_pointer` 清掉在途手势、`shortcuts` 提前返回。不改菜单布局/
文案/动作，无弹层时画布行为与 X20 基线逐像素一致，不新增依赖/线程/crate。

证据：脚本 `menu-canvas-click.txt`（开菜单 → 点节点 → 指针移开，24 帧、
warmup 24）。修复前 trace `canvas_click blocked=false`、实拍 184,050 B
状态栏「已选：织思」；菜单展开对照实拍 207,370 B 证明点击前菜单确实开着；
修复后 trace `canvas_click blocked=true`、实拍 175,220 B 无选中/工具条。
前后差异 29,917 px = 1.710%（最大 205，`bbox=(267,87,1022,1071)`：
画布工具条 + 节点环 + 状态栏）。既有 30 组基线逐字节未变；新增第 31 组
golden `menu-canvas-click`（1620×1080 / 175,220 B / `811C6D36…9CF925`），
隔离 APPDATA 双跑 `--tol 0` 0 差异。复跑阶段门 **31 组 × 2 = 62/62**
全 0 差异、**761 tests / 0 failed**、fmt 干净、mw-shell clippy 仅 3 条历史，
结论仍为 `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`
（started 2026-10-25 07:51:08 +08:00，finished 07:52:52 +08:00）；release
`23,487,488 B` / SHA256 `3EA8B70F…E72240`。新基线副本在
`golden/menu-canvas-click-shell-ppp1.50.png`，证据见
`traces/caretaker-menu-canvas-ownership-2026-10-25.md`、
`caretaker-menu-canvas-ownership-{before,after,menu-open}-2026-10-25.png`、
`caretaker-menu-canvas-ownership-{before,after,golden}-2026-10-25.trace`、
`caretaker-stage-b-gate-menu-canvas-2026-10-25.log`。

## 门后维护（2026-10-25 08:30）

X17 的「同一时刻只有一个指针/键盘主人」在全屏大纲上还缺一处：大纲自绘整
列表，键盘直接读 `ui.input` / `consume_key`，不经过 egui 焦点系统。
`egui::Modal` 只挡指针与 Tab 巡游，标题栏菜单是 Memory 弹层，两者都挡不住
裸键。1080×720 / ppp 1.5、`MW_OUTLINE=1 MW_OUTLINE_FOCUS=1
MW_DIALOG_DEMO=1` 实拍：弹窗开着按 `Delete` → `outline_delete`，按 `Enter`
→ `outline_add_sibling` 并进入行内编辑，按 `Tab` → `outline_indent`，三下
都在暗幕背后改了文档；删除/缩进落到不可见的选择上。

修复只动 `panels/outline.rs`、`app_methods_2.rs`、`main.rs`：新增纯函数
`row_keys_owned(blocked, editing, filter_owned)`；`show` 收 `blocked`
参数，由 `main.rs` 传入既有的 `input_blocked`（与 ⑨ 标签条、⑩ 画布同一条
谓词），键盘处理链头部整段跳过，只发 `outline_key_blocked key=<K>`；搜索框
Esc 回焦也加 `!blocked`。门开时不抢/不移焦点，门关后原行焦点保持；不改
行高、缩进、焦点环、滚动与外观，不新增依赖/线程/crate。

证据：脚本 `outline-modal-input.txt`（`wait 8` + `Delete` + `Tab` + `Home`，
24 帧）与 `outline-menu-input.txt`（点「文件」→ `Delete`，23 帧）。修复前
26 帧脚本 trace 出现三条改文档事件；修复后三键全部 `outline_key_blocked`，
弹窗保持打开，大纲 3/13 行与缩进层级不变（1620×1080 / 156,211 B /
`2E51E8E1…D912D61D1`）。菜单场景先拍展开对照图（146,684 B /
`ADA89176…DAD2F0C`），再按 `Delete`：只有 `outline_key_blocked`，无
`outline_delete`，菜单由 `PopupNav` 收起（119,369 B /
`96487082…504E26A9`）。修复后 `Enter` 会落到弹窗默认「确定」并关闭弹窗，
归档脚本因此改用 `Delete` / `Tab` / `Home`。复跑阶段门 **31 组 × 2 = 62/62**
全 0 差异、**763 tests / 0 failed**、fmt 干净、mw-shell clippy 仅 3 条历史，
结论仍 `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`
（started 08:32:49，finished 2026-10-25 08:34:09）；release
`23,469,056 B` / SHA256 `A4662B1F…BBB862`。`outline` / `outline-focus` /
`outline-compact` 及其余基线逐字节未变。证据见
`traces/caretaker-outline-keys-2026-10-25.md` 及同前缀 trace/PNG、
`caretaker-stage-b-gate-outline-keys-2026-10-25.log`。

## 门后维护（2026-10-25 08:34）

上一项的首次阶段门在 `card-slash` 上暴露了一个既有确定性缺陷（与大纲改动
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
阶段门 62/62、763 tests（复跑日志同上）。证据见
`traces/caretaker-card-demo-date-2026-10-25.md`、
`caretaker-stage-b-gate-card-date-fail-2026-10-25.log`（修复前失败）、
`caretaker-card-demo-date-{context-baseline,baseline-crop,drift-crop}-2026-10-25.png`。

## 门后维护（2026-10-25 09:09）

同样属于「浮层是第一键盘主人」的页面型缺口，两处都只动输入归属、不动外观：

- **开始页 × 标题栏菜单**（`app_methods_1.rs`）：开始页裸键（Esc / Enter /
  1–5）在 Memory 菜单开着时仍归 `window_shortcuts`。1440×1000 / ppp 1.0 /
  `MW_START_DEMO=1` 实拍：开「文件」菜单后按 `1`，菜单背后的开始页被换成
  「思维导图」模板（122,992 B），trace 焦点变 `canvas`；`Esc` 被
  `window_shortcuts` 先消费，菜单收不到（202,498 B）。新增纯函数
  `home_bare_keys_owned(menu_open, help_open)`，菜单或速查表在前台时整段
  跳过。修复后 `1` 保留开始页并收起菜单（169,377 B），`Esc` 产生
  `popup_menu key=escape` 并收起菜单（169,377 B）。单测
  `start_page_bare_keys_yield_to_foreground_layers`；脚本
  `start-menu-{open,input,esc}.txt`。
- **笔记库 × 模态**（`note_library.rs` / `app_methods_3.rs`）：库页自绘整页，
  Esc / ↑↓ / Enter 绕过 egui 焦点。1080×720 / ppp 1.5 /
  `MW_LIBRARY_DEMO=1 MW_DIALOG_DEMO=1` 实拍：弹窗开着按 `↑`，暗幕背后预览
  从「键盘色板验证」切到「视图跟随」；再按 `Enter` 直接打开阅读页
  （102,116 B）。新增纯函数 `page_keys_owned(top_modal, dialog_demo)`，
  `NoteLibrary::show` 增加 `keys_owned` 门。修复后 `↑` 不再移动预览，
  `Enter` 落到弹窗默认「确定」并关窗（trace `dialog demo closed` /
  `dialog demo action 1`），库页保留（264,292 B）。单测
  `page_keys_yield_to_foreground_modals`；脚本 `library-modal-input.txt`。

复跑阶段门 **31 组 × 2 = 62/62** 全 0 差异、**765 tests / 0 failed**、fmt
干净、mw-shell clippy 仅 3 条历史，结论仍
`MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`
（started 09:05:59，finished 2026-10-25 09:08:57）；release
`23,470,592 B` / SHA256 `78A01487…5F49DE`，基线未被改写。证据见
`traces/caretaker-start-menu-2026-10-25.md`、
`traces/caretaker-library-keys-2026-10-25.md` 及同前缀 trace/PNG、
`traces/caretaker-stage-b-gate-start-menu-library-2026-10-25.log`。
