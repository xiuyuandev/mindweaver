# X21 · 相机、缩放进度、滚轮锚点与 Shift 换轴（X21a）

> 阶段：Stage C 第 1 轮（`docs/24` §3 阶段 C 的 X21 行：「重做相机、画布底色、纹理、缩放进度和滚轮锚点」）
> 切片：本轮只做 **X21a**（缩放读数 / 入口收口 / 锚点可观测 / Shift 换轴）；
> **X21b**（画布底色 / 四主题点阵纹理 / SVG·PDF 点阵同源）由另一棒落地，结论见
> [`shots/x100/X21b/report.md`](../X21b/report.md)。
> 状态：**进行中**（包状态以 `docs/STATUS.md` 包表为准，本文件只写细节）。
> **X21a 自己的证据已齐**：单测全绿、`mm-zoom-wheel` 首拍 `--tol 0` 零差异、轨迹自证锚点不漂。
> **门的最终事实（2026-11-02 20:15 / 20:19 两道正式门整跑）**：
> ① **`stage-b` 正式门 `MACHINE GATE PASS`**（`scripts/x100-stage-b-gate.ps1` 整跑，19:48:38–20:15:05，
> 日志 `target/x100/stage-b-gate.log`）：X11–X19 证据包 PASS、PowerShell 语法 PASS、`cargo fmt` PASS、
> `mw-shell` clippy 新增 0、**全量串行测试 1328 passed / 0 failed / 0 ignored**、
> **golden 178 场 × 2 遍 = 356 次比较零差异**；
> ② **`stage-cm` 正式门未通过**（20:15:14–20:19:32，日志 `target/x100/stage-cm-gate.log`）：门内
> `.zmind` 往返 PASS、全量测试 PASS、**golden 73 场 × 2 遍 = 146 次比较零差异**，**红的只有 `fmt` 与
> `clippy`，而且都落在并行会话正在改的文件上**——`crates/mw-core/src/geom.rs` 与
> `crates/mw-shell/src/canvas/tests.rs` 未格式化，`crates/mw-shell/src/canvas/canvas_methods_3.rs:588`
> 一条新增 clippy 警告。
> 严格区分：**`stage-b` 门绿了；`stage-cm` 门没绿**（红在别人在飞的代码上）——**不写「两道门全绿」**。
> 19:05 那次 177/178、70/73 的并发假红归因、以及 `cm-inspector` / `cm-inspector-pinned` 的真漂移重拍，
> 保留在 §7 R2 / R3。仍**不写 `已验证`**（人工门未走）。
> 口径修正（Lead 2026-11-02，本文件已按修正后的口径改写，不许再改回去）：
> ① **锚点语义保持不变**——缩放条/命令面板以视口中心为锚、键盘/滚轮以画布内指针为锚；「锚点收口是行为变更」那条已作废；
> ② 轨迹名定死 **`zoom_anchor`**，字段 `src=wheel|pinch|key|button|minimap|reset` / `at=(x,y)` / `world=(x,y)` /
> `zoom=…` / **`drift=(x,y)`**（19:0x 交付形态里 drift 回来了，作「同一世界点缩放后在屏幕上漂了多远」的自证；
> 本文 17:2x 版曾按中间口径删掉它，已按实测恢复）；
> 编辑期锁住时写 **`zoom_blocked src=… editing=1`** 且**不写** `zoom_anchor`；限位写 `zoom_anchor … clamped=1`；
> ③ `docs/06` 的 `0.95^Δ` **已由 Lead 更正**为「跟 egui `zoom_delta` 的 `exp(0.005 × Δ)`，一格 ≈ 1.28×」，
> 本文不再记「疑 docs 与实现不符」，只留「已更正，见 `docs/06` 那一行」。
> 设计来源：缩放条几何与命中见 `docs/24` 的「X15 验收口径」与 `docs/06` §3 布局表第 ⑥ 行；
> 滚轮 / 双指 / Shift 换轴 / 编辑期锁缩放见 `docs/06` §5 画布交互表（**第 148–149 行**，其中 148 行即 Lead 更正后的步进口径）
> 与 §7 编辑态；执行协议见 `docs/24` §4。
> 交付边界：本包的写作权只在 `shots/x100/X21/`（本轮第二刀另含 `shots/x100/X21b/report.md` 与
> `docs/x100/log/execution-log-2026-10.md` 的一条记录）；`docs/STATUS.md` 由 Lead 回填，本文件不代改。

## 0. 一页结论

| # | 合同 | 实测现状（2026-11-02 19:0x） | 结论 |
| --- | --- | --- | --- |
| A | 缩放读数单一出口 `format_zoom(zoom)`：pct ≥ 10 取 0 位小数、< 10 取 1 位、非有限印 `—` | **落地**：`canvas/consts.rs::format_zoom`，缩放条（`main.rs`）与 `MW_STATS` 浮层（`stats.rs`）共用；先四舍五入到 1 位再判档（`r = (pct×10).round()/10`），整数档印 `pct` 避免二次舍入；`NaN`/`±∞` → `—` | 单测 11 例全绿（含 `0.0996→10%`、`0.0994→9.9%`、`0.8545001→85%`、`NaN/±∞→—`） |
| B | 缩放入口收成**一条**：一个入口函数 + 一个倍率常量 + `Ctrl+0` 走 `zoom_reset_action()` | **落地**：`ZoomSrc` 六值 + `zoom_step_at(anchor, factor, src)`（五条手势入口共用）+ `zoom_by(factor, src)`（视口中心锚，缩放条/命令面板用）+ `zoom_reset_action()`（唯一 100% 实现）；`Ctrl+0` 已改调它；倍率只剩 `canvas_chrome::ZOOM_STEP` | 源码断言钉住「复位只有一个写入点」「轨迹词汇单一来源」；字面量清零 |
| C | 一条轨迹 `zoom_anchor`（`at`/`world`/`zoom`/`drift`）+ 编辑期 `zoom_blocked` + 限位 `clamped=1` | **落地**：`trace_zoom_anchor` 写 `zoom_anchor src=… at=(x,y) world=(x,y) zoom=… drift=(x,y)[ clamped=1]`；编辑期写独立一行 `zoom_blocked src=… editing=1`；小地图旧轨迹 `minimap zoom factor=…` 已撤（源码断言） | 实测：`mm-zoom-wheel` 轨迹里 `zoom_anchor src=wheel` 反复出现、`drift=(0.00,0.00)` |
| D | `docs/06` §5「滚轮 / 触控板双指：平移（横向 Shift 切换轴向）」落地为纯函数 `wheel_pan(scroll, shift)` | **落地**：`consts.rs::wheel_pan`，接线在 `canvas_methods_2.rs` §4 平移分支；语义**纵向优先**（`y ≠ 0` 时纵向搬横轴并清零 `y`；`y == 0` 保留横向零头） | 单测 4 组；`mm-pan-camera` 在 19:3x 两遍 356/356 零差异名单内（只发 `scroll`、不按 `Shift`，行为逐字节不变） |
| E | 非目标：画布底色 / 点阵纹理 / 导出同源 → **X21b** | 已由另一棒交付：纹理令牌单源 + 导出同源 + PDF 点阵缺口关闭，「SVG 错半格 bug」已证伪（详见 X21b 报告） | 本包不再跟踪 |
| F | 验收：单测 + 新金样 `mm-zoom-wheel` + 两道正式门 | **已采**：`stage-b` 登记 **178 场 / 356 次比较**（第 178 场即 `mm-zoom-wheel`），首拍即 `--tol 0` 零差异；**`stage-b` 正式门 `MACHINE GATE PASS`**（20:15）：1328/0/0 ignored、fmt PASS、clippy 新增 0、golden 356/356 零差异 | X21 自身场景零差异；**`stage-b` 门绿；`stage-cm` 门未通过**（红在并行会话的 `fmt`/`clippy`，golden 146/146 零差异）——不写「两道门全绿」（见 §7 R1–R3） |

**一句话**：X21a 不换皮、不动相机手感、**零外观改动**，只把「缩放」这件事从 9 条各说各话的路径
收成 1 条可观测的路径（一条入口 + 一个倍率常量 + 一个读数函数 + 一条轨迹 + 编辑期锁），
并把 `docs/06` 里一直没实现的 Shift 换轴补上；因为锚点语义没变，**既有基线一张都没重拍**，
画面层唯一的新证据是 `mm-zoom-wheel` 一张终态——首拍就 `--tol 0` 零差异，轨迹 `drift=(0.00,0.00)`。

## 1. 范围

### 1.1 A · 缩放读数单一出口

`format_zoom(zoom: f32) -> String`，入参是**倍率**（1.0 = 100%），不是百分比。

| 档位 | 规则 | 例 |
| --- | --- | --- |
| pct ≥ 10 | `{:.0}%`（印 `pct` 本身，不印二次舍入后的 `r`） | 1.0 → `100%`；0.74 → `74%`；5.0 → `500%` |
| pct < 10 | `{r:.1}%`（`r` = 四舍五入到 1 位后的 pct） | 0.065 → `6.5%`；0.005 → `0.5%` |
| 非有限 | `—` | `f32::NAN` / `∞` / `-∞` 都不许印出 `NaN%` / `inf%` |

- **档位判据用「四舍五入到 1 位后的 pct」**：`r >= 10.0` 走整数档。于是 9.96% 与 10.0% 都印 `10%`，
  不会出现「9.96% 印 `10.0%`、10.0% 印 `10%`」这种同一档位两种宽度的缝合。
- **整数档印 `pct` 而不是 `r`**：`r` 已在 0.1 网格上，再取 0 位就是二次舍入——fit 出来的
  `0.85450012` 会先变 `85.5` 再进成 `86%`，而真值 85.45% 该读 85%。这条有单测钉住（`0.8545001 → 85%`）。
- 共用点两处：右下缩放条的百分比块（`main.rs::zoom_control`，`format_zoom(canvas.zoom())` 那一行）与
  `MW_STATS=1` 性能浮层（`stats.rs`，`crate::canvas::format_zoom(snap.zoom)` 那一行）。
  状态栏不显示缩放（X15 已移除），**不允许**出现第三个缩放格式化点。
- 为什么值得单列一条：取景（`fit`）可以把 20k 文档放到 0.5%，交互下限 `MIN_ZOOM = 0.1`（10%）。
  旧缩放条 `{:.0}` 会把 0.5% 印成 `0%`——读数说「没有图」，画面却有一整张。

### 1.2 B · 缩放入口收成一条

现状 10 个调用点（读码 + 实测，行号是快照，同时写出那一行长什么样）：

| # | 入口 | 位置 / 那一行长什么样 | 倍率 | 锚点 | `src` |
| --- | --- | --- | --- | --- | --- |
| 1 | 缩放条 `−` | `main.rs::zoom_control`：`canvas.zoom_by(1.0 / canvas_chrome::ZOOM_STEP, ZoomSrc::Button);` | `ZOOM_STEP` | 视口中心 | `button` |
| 2 | 缩放条 `+` | 同上：`canvas.zoom_by(canvas_chrome::ZOOM_STEP, ZoomSrc::Button);` | `ZOOM_STEP` | 视口中心 | `button` |
| 3 | 缩放条 百分比 | 同上：`canvas.zoom_reset_action();` | — | 中心（不改 `center`） | `reset` |
| 4 | 缩放条 适应 | 同上：`canvas.fit_view();` | — | 重取景 | 不写 `zoom_anchor`（走既有 `event fit …`） |
| 5 | 键盘 `Ctrl+=` / `Ctrl+-` | `canvas_methods_2.rs`：`let factor = if zoom_in { ZOOM_STEP } else { 1.0 / ZOOM_STEP };` → `self.zoom_step_at(at, factor, ZoomSrc::Key)` | `ZOOM_STEP` | 画布内指针（**已排除浮层**）否则中心 | `key` |
| 6 | 键盘 `Ctrl+0` | `canvas_methods_3.rs`：`i.consume_key(cmd, Key::Num0)` 分支里的 `dirty \|= self.zoom_reset_action();` | — | 中心 | `reset` |
| 7 | 命令面板 放大/缩小 | `app_methods_2.rs`：`A::ZoomIn => canvas.zoom_by(ZOOM_STEP, ZoomSrc::Button)` / `ZoomOut` 下一行 | `ZOOM_STEP` | 视口中心 | `button` |
| 8 | 右键菜单 `100%` / `适应画布` | `A::ResetZoom => { canvas.zoom_reset_action(); }`；适应走 `A::FitView => canvas.fit_view()` | — | 中心 | `reset` |
| 9 | 小地图滚轮 | `main.rs`：`canvas.zoom_step_at(screen, factor, ZoomSrc::Minimap)` | `ZOOM_STEP`（`minimap_wheel_factor`） | 世界点投影回屏幕点 | `minimap` |
| 10 | 画布 `Ctrl+滚轮` / 双指 | `canvas_methods_2.rs` §4：`self.zoom_step_at(p, zoom_delta, ZoomSrc::Wheel \| ZoomSrc::Pinch)` | egui `zoom_delta`（唯一来源） | 画布内指针 | `wheel` / `pinch` |

合同：

1. **只有一个乘数来源**：`ZOOM_STEP`（`canvas_chrome.rs`，现值 1.2）。10 个调用点里**不出现**
   `1.2` / `1.0 / 1.2` 字面量；`minimap_wheel_factor` 也走它。
2. **只有一个手势入口**：`Canvas::zoom_step_at(anchor, factor, src)`（`canvas_methods_1.rs`）——
   锚点换算、钳位、编辑期锁、轨迹只写一遍；纯核是 `consts.rs::zoom_step_camera`
   （病态因子 `NaN/0/负` 直接返回 `false`；下界用 `MIN_ZOOM.min(当前值)`，从 0.5% 再缩一档不许跳回 10%）。
   按钮/命令面板走 `zoom_by(factor, src)`，它只把锚点钉在视口中心再转调 `zoom_step_at`。
3. **锚点语义本轮不变**（Lead 口径①）：缩放条 `−/+`、命令面板、右键菜单 = **视口中心**；
   `Ctrl+=`/`Ctrl+-` 与滚轮/pinch = **画布内指针**（指针不在画布内、或落在缩放条/小地图/面包屑等
   浮层上时退回中心——`canvas_methods_2.rs` 里 `local` 已加 `!self.pointer_over_overlay(ui)` 过滤）。
   所以「点按钮/点面板导致取景漂」这件事**不存在**，也不需要重拍基线。
4. **`Ctrl+0` 只有一个实现**：`zoom_reset_action()`。`crates/mw-shell/src` 全文除
   `consts.rs::zoom_reset` 自身外**没有** `camera.zoom = 1.0`；单测 `ctrl_zero_shares_the_single_reset_path`
   用源码文本钉住这一条。
5. **编辑期锁缩放收在 `zoom_step_at` 一处早退**（`docs/06` §5「编辑期间画布锁定缩放，平移仍可用」）：
   五条入口一致；`fit` 不是缩放入口（它连 `center` 一起换，天然不满足锚点不漂），继续走 `fit_view`。
6. `zoom_step_at` 只改相机（视图态），不碰文档、不入撤销栈。

### 1.3 C · 锚点可观测

一条轨迹（`crate::trace::event`，即 `MW_TRACE` 下的 `event ` 前缀行）：

```text
event zoom_anchor src=wheel at=(812.00,401.00) world=(120.44,-33.18) zoom=1.0361 drift=(0.00,0.00)
```

| 字段 | 语义 |
| --- | --- |
| `src` | 闭集 `wheel` / `pinch` / `key` / `button` / `minimap` / `reset`（不许长第 7 个；吸入映射见 §3.2） |
| `at` | 锚点屏幕位置（画布局部 css px） |
| `world` | 被锚定的**世界点**（缩放前该屏幕点对应的世界坐标） |
| `zoom` | 缩放**后**倍率，4 位小数 |
| `drift` | 同一世界点缩放后在屏幕上的位移绝对值（css px，两分量）；正确实现只该有浮点误差 |
| `clamped=1` | 后缀，仅当「请求合法但被上下限整口吃掉、倍率纹丝没动」时追加 |
| `zoom_blocked` | **另一条独立句法**：编辑期锁住 → `zoom_blocked src=… editing=1`，不写 `zoom_anchor` |

- 判据一句话：**同一世界点缩放前后屏幕位置一致**——`drag` 的两分量就是这句话的量化；
  实测 `mm-zoom-wheel` 的每一条都是 `drift=(0.00,0.00)`。
- **为什么 `drift` 留着**：17:2x 曾按中间口径删掉它（认为 `at`+`world`+`zoom` 已够、drift 是冗余推导）。
  19:0x 交付形态把它写回来了：日志自己就能证完「锚住了」，探针不必再拿三个字段重算一遍；
  且 `drift` 是**同一实现内**的反投影残差，能抓住「`at`/`world` 抄错一帧」这类抄写错误。
- 写之前判 `crate::trace::is_enabled()`：不设 `MW_TRACE` 时零格式化开销。
- 小地图旧的 `minimap zoom factor=…` **已撤**（同一条轨迹不留第二套写法）；源码断言
  `zoom_trace_vocabulary_is_single_sourced` 钉住：`zoom_blocked` 句法存在、`locked=1` 不许再出现、
  `clamped=1` 必须存在、`reason=limit` 不许留、`main.rs` 不许再打第二条小地图轨迹。

### 1.4 D · Shift 切换轴向

`docs/06` §5 两行原文（**第 148–149 行**）：
`Ctrl+滚轮 | 以光标为锚缩放，无动画（跟手优先）`——步进口径 2026-11-02 已更正为
「跟 egui 的 `zoom_delta` 走，即 `exp(0.005 × Δ)`，一格滚轮（Δ≈50）约 **1.28×**」；
`滚轮 / 触控板双指 | 平移（**横向 Shift 切换轴向**：按住 Shift 时纵向滚动改走横轴；X21 落地时按此实现）`。
第二行后半句在本轮之前一直没实现。

纯函数（`crates/mw-shell/src/canvas/consts.rs`，与 `zoom_at_anchor` / `zoom_step_camera` 同处）：

```rust
/// `Shift+滚轮` = 横向平移。不按 Shift 时原样返回（与改动前逐像素一致）。
pub(crate) fn wheel_pan(scroll: Vec2, shift: bool) -> Vec2 {
    if !shift { return scroll; }
    if scroll.y != 0.0 { Vec2::new(scroll.y, 0.0) } else { Vec2::new(scroll.x, 0.0) }
}
```

- 语义（**以落地代码为准**）：`shift=false` 原样透传；`shift=true` 且纵向有量 → 纵向搬到横轴、纵向清零；
  `shift=true` 且纵向恰好为 0 → 保留原横向分量（真·横向滚轮不被 `Shift` 吃掉）。
  旧草稿写的是「两轴相加 `scroll.x + scroll.y`」，落地选了「**纵向优先**」——鼠标滚轮报的就是纵向，
  横向只是触控板的零头，相加会让一次横向甩动在按住 Shift 时方向反转。两者都消除死输入，取后者。
- 接线点唯一：`canvas_methods_2.rs` §4 的 `else if scroll != Vec2::ZERO` 分支，读 `i.modifiers.shift`
  后把 `center.x/y -= d / zoom`。**编辑期仍然可以平移**，所以这里不加 `!editing`。
- 画面影响：无。既有含 `scroll` 的金样脚本没有一个按过 `Shift`（逐条读码核对），
  所以 `wheel_pan` 的 `shift=false` 分支就是它们看到的全部行为；`mm-pan-camera` 实测在零差异名单里。

## 2. 非目标（明确不做，且写明为什么）

| 不做 | 为什么排 X21b | 状态 |
| --- | --- | --- |
| 画布底色改 `#FBFBFC` | 实拍结论是「纯色近白、没有点阵」，改底色方向相反，且会让 250+ 场基线全漂 | **X21b 已驳回并回退**（暖白 `#FCFBF8` 不动） |
| 四主题点阵纹理表 | 要新令牌表（4 主题 × 间距/半径/颜色），并与网格、标尺、聚焦视图的底纹互斥 | **X21b 已交付令牌单源**，但只有 `ink` 铺点阵、零外观变化 |
| 点阵在 SVG/PDF 同源 | 画布合成、SVG、PDF 三条派生路径都要读同一份 `Theme::canvas_dots()` | **X21b 已交付**（PDF 平铺图案对象，缺口关闭） |

> 三项已在 `shots/x100/X21b/` 交付（详见其报告 §4.1 与 §5），本包只记「非目标」。

本轮另外不做：

- **不改相机手感**：`Ctrl+滚轮` 仍以 egui 0.37 的 `zoom_delta()` 为唯一来源，不自己算指数、不做缩放动画。
- **不改缩放区间**：`MIN_ZOOM = 0.1` / `MAX_ZOOM = 5.0`（交互档）与 `FIT_MIN_ZOOM = 0.005` /
  `FIT_MAX_ZOOM = 1.5`（取景档）两级不动；`floor = MIN_ZOOM.min(camera.zoom)` 那条「下界不越过当前值」
  的规矩不动。
- **不动数据**：不碰 `.zmind`、`Node`、`StyleOverride`，不新增字段、不升 `FORMAT_VERSION`、不引依赖。
- **不做**：状态栏缩放读数（X15 已移除）、缩放档位下拉、百分比可键入。
- **不重拍既有基线**：本轮**零外观改动**，矩阵里 Inspector 那 14 张重拍是 SE-13 的行距改动（§6.4 另记），
  与 X21 无关。

## 3. 设计合同

### 3.1 `format_zoom`（最终形态，契约测试即验收）

```rust
pub(crate) fn format_zoom(zoom: f32) -> String {
    let pct = zoom * 100.0;
    if !pct.is_finite() { return "—".to_string(); }
    let r = (pct * 10.0).round() / 10.0;      // 先四舍五入到 1 位
    if r >= 10.0 { format!("{pct:.0}%") }      // 判据用 r、墨迹用 pct（避免二次舍入）
    else { format!("{r:.1}%") }
}
```

必测边界（表即用例，已全绿）：`1.0→"100%"`、`0.74→"74%"`、`0.12→"12%"`、
`0.0996→"10%"`（进位到整数档）、`0.0994→"9.9%"`、`0.1→"10%"`、`0.05→"5.0%"`、`0.005→"0.5%"`、
`0.8545001→"85%"`（二次舍入的活边界）、`0.996→"100%"`、`12.345→"1234%"`（f32 半值取偶）、
`NaN→"—"`、`+∞→"—"`、`-∞→"—"`。
单测：`format_zoom_switches_precision_at_ten_percent`。

### 3.2 入口 → `src` token 映射（闭集，不许长第 7 个）

| 入口 | `src` |
| --- | --- |
| `Ctrl+滚轮` / 触控板 pinch（`zoom_delta != 1.0`） | `wheel` / `pinch`（`multi_touch().is_some()` 区分） |
| 键盘 `Ctrl+=` / `Ctrl+-` | `key` |
| 缩放条 `−` / `+`、命令面板「放大 / 缩小」、右键菜单同名项 | `button` |
| 小地图滚轮 | `minimap` |
| `Ctrl+0`、缩放条百分比、菜单/命令面板「100%」 | `reset` |
| 缩放条「适应」、菜单「适应画布」、`Ctrl+1` / `Home` | 不写 `zoom_anchor`（写既有 `event fit …`） |

单测 `zoom_src_names_are_the_trace_contract` 把六值锁成
`["wheel", "pinch", "key", "button", "minimap", "reset"]`。

### 3.3 失败与空动作口径（已落地）

| 情形 | 行为 | 轨迹 |
| --- | --- | --- |
| 已在下限 / 上限 | 相机不动，返回 `false`（不白重绘一帧） | `zoom_anchor … clamped=1`（倍率没变也留一行） |
| 编辑期 | 相机不动，返回 `false` | **`zoom_blocked src=… editing=1`**，不写 `zoom_anchor` |
| 已在 100% 按 `Ctrl+0` | 不动，返回 `false`（菜单据此置灰） | 不写 |
| 指针不在画布内 / 在浮层上 | 锚点退化为视口中心，**不算失败** | 正常写 `zoom_anchor`，`at=视口中心` |
| 因子是 `NaN` / `0` / 负数 | 不动，返回 `false`（`zoom_step_camera` 第一道门） | 不写 |

### 3.4 不变量（改完必须仍然成立）

1. 所有文档变更经 `Command`；本条不涉及文档，帧内只改相机（视图态）。
2. 铬件屏幕空间恒定：缩放条几何 `ZOOM_W = 154` / `ZOOM_BTN = 24` / `ZOOM_PCT_W = 48` 不随缩放变。
3. 浮层上的滚轮归浮层（小地图/面包屑），不得穿透成画布缩放或平移。
4. 编辑期锁缩放、不锁平移（`docs/06` §7）。
5. `MW_TRACE` 未设时零行为差异、零像素差异（所有轨迹都在 `is_enabled()` 门后）。
6. **零外观改动**：本轮改完，除新增的 `mm-zoom-wheel`，其余场景帧与帧之间逐字节不变——
   两道正式门 20:15 / 20:19 的 golden 各自 178×2 = **356/356** 与 73×2 = **146/146** 零差异
   即是这条的实测证据（19:05 那批假红的归因见 §7 R2）。

## 4. 落地对账（计划 → 交付形态）

| 文件 | 计划 | 交付形态（实测） |
| --- | --- | --- |
| `canvas/consts.rs` | `wheel_pan` + 单测 | `wheel_pan`（纵向优先）+ `wheel_pan_switches_axis_only_with_shift` |
| `canvas/consts.rs` | `format_zoom` + 单测 | 位置从 `canvas_chrome.rs` 挪到 `consts.rs`（与 `zoom_at_anchor` 同处），经 `canvas.rs` 的 `pub(crate) use consts::*;` 导出 |
| `canvas/consts.rs` | `zoom_factor(dir)` 纯函数 | **未抽**：倍率由调用点 `ZOOM_STEP` / `1.0 / ZOOM_STEP` 表达，语义已收口，少一层间接 |
| `canvas/consts.rs` | —（计划外新增） | `zoom_step_camera`（钳位 + 锚点补偿纯核，「是否真的动了」）与 `ZoomSrc` 六值 |
| `canvas/canvas_methods_1.rs` | `zoom_step` / `zoom_reset` / `zoom_world` | `zoom_step_at` / `zoom_by(factor, src)` / `zoom_reset_action` / `trace_zoom_anchor` / `trace_zoom_blocked` |
| `canvas.rs` | 新增 `last_pointer: Option<Point>` 字段 | **未加字段**：每帧在 `canvas_methods_2.rs` 算 `local = hover_pos.filter(rect.contains && !pointer_over_overlay)` 往手势里传 |
| `canvas/canvas_methods_2.rs` | 键盘/滚轮改调新入口；平移分支调 `wheel_pan` | 已落地；键盘锚点已排除浮层（`!self.pointer_over_overlay(ui)` 过滤） |
| `canvas/canvas_methods_3.rs` | `Num0` 分支删掉 `camera.zoom = 1.0` | 已落地：`dirty \|= self.zoom_reset_action();` |
| `main.rs` | 缩放条三处改调新函数；小地图改调新入口 | 已落地；小地图**只**写 `zoom_anchor src=minimap`，旧 `minimap zoom factor=` 已删 |
| `app_methods_2.rs` | `ZoomIn/ZoomOut` → `ZOOOM_STEP` 入口；`ResetZoom` → `zoom_reset` | 已落地：`zoom_by(±ZOOM_STEP, ZoomSrc::Button)` / `zoom_reset_action()` |
| `stats.rs` | 浮层读数改调 `format_zoom` | 已落地：`crate::canvas::format_zoom(snap.zoom)` |
| `golden_input.rs` | 让 `scroll`/`scroll_lines` 带上当前立起的修饰键 | 已落地：`Event::MouseWheel { modifiers: held_mods }` + 单测 `scroll_carries_the_held_modifiers_for_ctrl_wheel_zoom` |
| `shots/golden/inputs/mm-zoom-wheel.txt` | 新输入脚本 | 已落地（17 行；`hover 700 500` → `mods ctrl` → 两格 `scroll 0 50` → 移开指针） |
| `shots/golden/mm-zoom-wheel/shell-ppp1.00.png` | 新基线 | 已落地；**首拍即 `--tol 0` 零差异** |
| `scripts/x100-stage-b-gate.ps1` | 登记 `mm-zoom-wheel` | 已登记为第 178 场（`@{ Name = 'mm-zoom-wheel'; Dir = 'shots/golden/mm-zoom-wheel'; Ppp = '1.0'; Warmup = 30; Env = @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/mm-zoom-wheel.txt' } }`） |
| `scripts/golden-matrix.ps1` | —（计划外新增） | 只跑已构建 release 二进制、场景表从正式门脚本抽，避开并行会话触发的 4 分钟 LTO 重建 |

机械自查（一条命令就能判的，别靠眼睛）：

- `grep -rn "zoom_by(1\.2\|1\.0 / 1\.2" crates/mw-shell/src` → 0 命中（`tests.rs` 的 `1.0 / 1.2` 是测试夹具）
- `grep -rn "camera.zoom = 1\.0" crates/mw-shell/src` → 只允许 `consts.rs::zoom_reset` 内那一处
- `grep -rn "{:.0}%\|{:.1}%" crates/mw-shell/src` → 只允许 `consts.rs::format_zoom` 内；
  `stats.rs` 的 `{:.0}%` 是「可见节点占比」，不是缩放读数，别误判

## 5. 验收场景

### 5.1 机器判据（全部可自动化）

| ID | 场景 | 判据 | 落点 / 实测 |
| --- | --- | --- | --- |
| AC-1 | `format_zoom` 边界表 | 逐例相等（11 例 + 非有限 3 例） | `format_zoom_switches_precision_at_ten_percent` ✅ |
| AC-2 | 倍率来源唯一 | 无 `1.2` / `1.0 / 1.2` 字面量，全走 `ZOOM_STEP` | 机械自查 ✅、未成单测 |
| AC-3 | `wheel_pan` 表 | 不按 Shift 原样；按 Shift 纵向优先；`y==0` 保留横向 | `wheel_pan_switches_axis_only_with_shift` ✅ |
| AC-4 | 锚点选择 | 无画布指针/在浮层上 → 视口中心；画布内 → 指针 | **无独立纯函数单测**：逻辑在 `zoom_step_at` 内 + `local` 的浮层过滤，读码 + 轨迹覆盖 |
| AC-5 | 复位唯一性 | `Ctrl+0` 与缩放条百分比同走 `zoom_reset_action()`，全文无 `camera.zoom = 1.0` | `ctrl_zero_shares_the_single_reset_path` ✅（源码断言） |
| AC-6 | 锚点不漂 | 任意屏幕点缩放前后反投影 ≤ 0.01 css px | `zoom_step_pins_the_screen_anchor_on_every_entry`（4 档 × 4 落点）✅；轨迹 `drift=(0.00,0.00)` ✅ |
| AC-7 | `Ctrl+0` 恰好回 100% | 轨迹 `zoom_anchor src=reset … zoom=1.0000`，`world=(相机中心)` | 轨迹实测 ✅（矩阵里该场景零差异） |
| AC-8 | `mm-zoom-wheel` | 与基线 `--tol 0` 零差异 + 轨迹读数自洽 | 首拍零差异 ✅；登记第 178 场 ✅ |
| AC-9 | Shift 换轴 | AC-3 单测 + `mm-pan-camera` 复跑哈希不变 | 单测 ✅；`mm-pan-camera` 在 19:3x 两遍 356/356 零差异名单内 ✅ |
| AC-10 | 门 | `stage-b` / `stage-cm` 两道正式门 | **`stage-b` 达成**（20:15 `MACHINE GATE PASS`：1328/0/0 ignored、fmt PASS、clippy 新增 0、golden 356/356 零差异）；**`stage-cm` 未达成**（20:19 `MACHINE GATE FAIL`：golden 146/146 零差异、测试 PASS，红在并行的 `fmt` 与 `clippy`） |

### 5.2 人能判的（留给人工门 / Lead）

1. 按 `+` 五下、再按 `-` 五下，缩放条读数回 `100%`，画面回到原来的取景——**绕视口中心**转，与改动前同一手感。
2. 指针停在某个节点上按 `Ctrl+=`：画面围着**指针**转，指针下的那个节点不动。
3. 指针停在右下缩放条上按 `Ctrl+=`：锚点是**视口中心**（浮层不算画布指针）。
4. 编辑一个节点时按「+」「-」、滚轮、拖小地图滚轮：读数一律不动（编辑期锁缩放，五条入口一致），
   但按住空格/中键仍能平移。
5. 按住 `Shift` 滚滚轮：画面只横向走，纵向纹丝不动；松开 `Shift` 恢复纵向。

## 6. 证据（已采集 · 2026-11-02 19:0x–20:1x）

### 6.1 四类证据

| 类 | 内容 | 落点 |
| --- | --- | --- |
| 行为 | X21 相关单测 9 条全绿（名单见 §6.2）；轨迹 `zoom_anchor` / `zoom_blocked` / `clamped` 三种句法各由源码断言钉住 | `stage-b` 门内全量 **1328 passed / 0 failed / 0 ignored**（20:15）；`zoom_trace_vocabulary_is_single_sourced` 等 |
| 视觉 | 新基线 `mm-zoom-wheel`（`shell-ppp1.00.png`，warmup 30），**首拍即 `--tol 0` 零差异**；本轮零外观改动 ⇒ 既有基线不重拍 | `shots/golden/mm-zoom-wheel/`、`shots/golden/inputs/mm-zoom-wheel.txt` |
| 回归 | **两道正式门整跑**：`stage-b` 20:15 `MACHINE GATE PASS`（178 场 × 2 遍 = **356 次比较零差异**）；`stage-cm` 20:19 门**未通过**，但门内 golden 73 场 × 2 遍 = **146 次比较零差异**（红只在 `fmt`/`clippy`） | `target/x100/stage-b-gate.log`、`target/x100/stage-cm-gate.log`；19:05 那次 177/178、70/73 的假红归因见 §7 R2 |
| 工程 | `cargo fmt` PASS、`mw-shell` clippy 新增 0、全量串行测试 1328/0/0 ignored（均为 `stage-b` 门内 20:15 实测）；`stage-cm` 门的 `fmt`/`clippy` 红落在并行会话的 `geom.rs` / `canvas/tests.rs` / `canvas_methods_3.rs:588` | `docs/STATUS.md` §1（场次与门结论唯一真相源） |

**矩阵执行方式**：`scripts/golden-matrix.ps1` 直接跑已构建的 `target/release/mw-shell.exe`
（场景表从 `scripts/x100-stage-b-gate.ps1` 抽取，只有一份环境表）。它在本轮的作用是
**在正式门之前先定性**：19:35–19:47 冻结二进制（mtime 19:32:30）连跑两遍 = 356/356 + 146/146 零差异，
证明 19:05 的红是抢占不是回归。之后 **19:48–20:19 两道正式门整跑时二进制没有被重建**
（门内每场都是 `Finished release profile [optimized] target(s) in 0.3x s`），
所以矩阵与门同源、结论互相印证。当时优先用矩阵的理由：并行会话在改 `crates/mw-core`，
任何 `cargo` 调用都可能触发 4 分钟 LTO 重建，并把验证用的二进制换成对方未完成的半成品。
矩阵脚本不落 `MW_TRACE`，所以轨迹是**实测看到**的，本目录**没有**落盘 trace 文件
（如需长期证据，可另跑一场带 `MW_TRACE` 的采一份）。

### 6.2 X21 相关单测（9 条）

1. `zoom_anchor_keeps_the_world_point_under_the_cursor` —— `zoom_at_anchor` 世界点不漂（含上界钳位）。
2. `zoom_step_pins_the_screen_anchor_on_every_entry` —— 新入口纯核：4 档 × 4 落点 + 停止在上下限 + 病态因子。
3. `format_zoom_switches_precision_at_ten_percent` —— 11 例 + `NaN/±∞`。
4. `wheel_pan_switches_axis_only_with_shift` —— 不按 Shift 原样 / 纵向优先 / `y==0` 保留横向。
5. `zoom_reset_returns_to_hundred_percent_once` —— 35% 复位、已在 100% 不动、fit 的 0.5% 也能回 100%。
6. `zoom_src_names_are_the_trace_contract` —— 六值闭集逐字。
7. `zoom_trace_vocabulary_is_single_sourced` —— `zoom_blocked` 句法在、`locked=1`/`reason=limit` 不在、
   `clamped=1` 在、`main.rs` 不打第二条小地图轨迹。
8. `ctrl_zero_shares_the_single_reset_path` —— 源码钉住「复位只有一个写入点」。
9. `scroll_carries_the_held_modifiers_for_ctrl_wheel_zoom`（`golden_input.rs`）—— `mods ctrl` 之后
   滚轮事件自带 ctrl，否则 `Ctrl+滚轮` 金样根本拍不出来。

### 6.3 `mm-zoom-wheel` 场景（已落地）

```text
# 两格滚轮（每格 50）→ exp(0.005*100) ≈ 1.6487 倍，缩放条读数落在 164% 一带
wait 3
hover 700 500
wait 2
mods ctrl
scroll 0 50
wait 2
scroll 0 50
wait 2
mods none
wait 6
move 1500 1000    # 指针移开，悬停态不进基线
wait 2
```

它证明的是 C 的锚不漂 + B 的入口收口：同一画面里指针落点处的内容缩放前后不动，
且缩放条读数等于轨迹里 `zoom` 的读数。它**不**证明 `format_zoom` 的 <10% 档（164% 走整数档）。

### 6.4 一条必须记下来的工程坑：APPDATA / LOCALAPPDATA

`mm-zoom-wheel` **首拍时误在没有重定向 `APPDATA` / `LOCALAPPDATA` 的环境里拍**，结果把
「自动保存失败」红条一起拍进了基线。正确环境与正式门完全一致：矩阵/门脚本会把这两个变量指到
`target/ci-appdata/<场景名>` 与 `target/ci-appdata/<场景名>-local` 的**每场独立目录**
（`scripts/golden-matrix.ps1` 的 `Invoke-Scene` 里用 `[Environment]::SetEnvironmentVariable(...,'Process')`
逐场设置、跑完还原）。教训：**手工拍任何 golden 之前，先把 APPDATA/LOCALAPPDATA 重定向**，
否则拍到的不是产品状态而是宿主磁盘状态。

> 附：`stage-cm` 里 `cm-inspector` / `cm-inspector-pinned` 这两张也一度红过，是 SE-13 把检查器组内
> 行距从 8 改成 6 的合同值改动——stage-b 的 14 张 `inspector-*` 与这两张都按新行距重拍过
> （漂移 1.695–3.785%，全部只在面板区），19:47 那遍 `stage-cm` 146/146 已复核零差异。

## 7. 风险与未做

### 7.1 风险（按「会不会让本轮白干」排序）

| # | 风险 | 现在的事实 | 处理 |
| --- | --- | --- | --- |
| R1 | **门状态的正确读法（最容易写错的地方）**：`stage-b` 正式门 20:15 **`MACHINE GATE PASS`**；`stage-cm` 正式门 20:19 **`MACHINE GATE FAIL`**，但红的两项 `cargo fmt --check` 与 `mw-shell clippy 新增 0` **全部落在并行会话正在改的文件上**——`crates/mw-core/src/geom.rs:168`、`crates/mw-shell/src/canvas/tests.rs:3453` 未格式化，`crates/mw-shell/src/canvas/canvas_methods_3.rs:588:30` 一条新增警告 | `stage-cm` 门内 golden 73×2 = **146/146 零差异**、`.zmind` 往返 PASS、全量测试 PASS（1335/0）⇒ 与 X21/X21b 的产物无关 | **不许写「两道门全绿」**。等并行会话把这三个文件收尾后**重跑 `x100-stage-cm-gate.ps1`**；门结论以 `docs/STATUS.md` §1 为准 |
| R2 | 19:05 那批「红灯」是并发干扰下的假红：`stage-b` 177/178（`mm-rel-link-created`）、`stage-cm` 70/73 | 并行会话正在 `rustc` 构建并跑 `mw-shell`（单场 16.5s）抢 CPU/GPU、harness 超时；`mm-rel-link-created` / `mm-summary-hover` / `card-autosave` / `search-text` / `mm-zoom-wheel` 单场景复跑逐个通过，19:20–19:30 那次两遍各 10 场失败还**互不相同**；19:35–19:47 冻结二进制两遍零差异 | 已结清，不再作为未决项；这条归因只留在报告里当口径，不许反过来当成「门没绿」的依据 |
| R3 | `stage-cm` 里**真实的**漂移只有 `cm-inspector` / `cm-inspector-pinned`：检查器组内行距 8→6 是合同值改动 | 只漂面板区，已重拍；19:47 矩阵与 20:19 门内均零差异 | 记账：合同值改动 ⇒ 基线必须重拍，这类清单要写进报告 |
| R4 | 并行会话改的是**画布底层**（`mw-core` 的 `document.rs`/`scene.rs`/`search.rs`/`geom.rs`、`mw-render/src/hit.rs`、`mw-shell/src/canvas/tests.rs`），任何一场金样都可能被带偏 | 已核：X21/X21b 自己的场景在门内零差异；`stage-cm` 门的 `fmt`/`clippy` 红正是他们那三个文件 | 只信「冻结二进制 + 场景表抽自正式门脚本」的结果；`stage-cm` 门等对方落笔后重跑 |
| R5 | `mm-zoom-wheel` 的读数依赖 egui 平滑在虚拟时钟下的排空帧数 | 首拍即零差异；M16 已把金样时钟改成帧计数（可复现） | 若将来抖动，改剧本 `wait` 帧数并如实记录，不许改代码凑数 |
| R6 | 轨迹写进滚轮热路径 | 已有 `is_enabled()` 门 | 保持门控 |
| R7 | 实机触控板（pinch / 双指）与 150%/200% DPI 未验 | 与全仓其它包同口径欠账 | 记 `未验证`，不写 `已验证` |

### 7.2 未做（本轮如实记账）

1. **`stage-cm` 门没通过**：20:19 `MACHINE GATE FAIL`，但红的是 `fmt` 与 `clippy`，而且全在并行会话
   正在改的文件（`crates/mw-core/src/geom.rs`、`crates/mw-shell/src/canvas/tests.rs`、
   `crates/mw-shell/src/canvas/canvas_methods_3.rs:588`）；门内 golden 146/146、`.zmind` 往返、
   全量测试都 PASS。等对方收尾后重跑，**不许写「两道门全绿」**。
2. **AC-4 没有单测**：锚点选择逻辑在 `zoom_step_at` 内 + `local` 的浮层过滤，只由读码与轨迹覆盖。
3. **轨迹没落盘**：`mm-zoom-wheel` 跑过、`zoom_anchor src=wheel drift=(0.00,0.00)` 看到过，
   但矩阵脚本不落 `MW_TRACE`，本目录没有 `trace-zoom.log`。
4. **编辑期锁的按钮/面板路径没有自动化**（`Canvas::new` 要 wgpu，无 GPU 夹具），只做代码收口。
5. **P2 三张建议加拍没加**：`mm-zoom-reset` / `mm-zoom-fit-20k`（`format_zoom` <10% 档唯一的画面证据）/
   `mm-pan-shift`。
6. **实机触控板与系统 DPI 未验**（同 R7）。
7. **`format_zoom` 的 <10% 档只有单测**，没有画面证据（交互下限就是 10%，只有取景能到 0.5%）。

## 8. 接口与分工

| 角色 | 写什么 | 状态 |
| --- | --- | --- |
| 本包（x21-report） | `shots/x100/X21/report.md`、`shots/x100/X21b/report.md`、`docs/x100/log/execution-log-2026-10.md` 一条记录 | 本轮完成 |
| 代码侧 teammate | `crates/mw-shell/src/canvas/{consts,canvas_methods_1,canvas_methods_2,canvas_methods_3}.rs`、`main.rs`、`app_methods_2.rs`、`stats.rs`、`golden_input.rs` + 单测 | 已落地 |
| 门脚本 owner | `scripts/x100-stage-b-gate.ps1`（登记 `mm-zoom-wheel`）、`shots/golden/inputs/mm-zoom-wheel.txt`、`shots/golden/mm-zoom-wheel/`、`scripts/golden-matrix.ps1` | 已落地 |
| Lead | `docs/STATUS.md` §1 数字与包表状态、门结论裁决、`stage-cm` 门重跑时机 | 进行中 |

**下一步（按顺序）**：① `stage-b` 正式门已于 20:15 `MACHINE GATE PASS`，本包机器证据齐 →
② 等并行会话收尾 `crates/mw-core/src/geom.rs` / `crates/mw-shell/src/canvas/tests.rs` /
`canvas_methods_3.rs:588` 三处 → ③ 重跑 `x100-stage-cm-gate.ps1` 让第二道门也绿 →
④ 两道门都绿后，包状态才允许从 `进行中` 提升为 `已实现`（`已验证` 等人工门，且仍欠实机触控板/DPI）。

## § 本轮不做什么（X21b 已交付）

画布底色 `#FBFBFC`、四主题点阵纹理、以及 PNG/SVG/PDF 点阵同源这三件事已由另一棒交付：底色与三套亮色
主题的点阵**按实拍结论驳回**（暖白 `#FCFBF8` 不动），保留的是「令牌单一出口 + 导出同源 + PDF 补点阵 +
8 条单测 + 零漂移」，结论与证据见 [`shots/x100/X21b/report.md`](../X21b/report.md) 与
`shots/x100/X21b/drift.md`。
