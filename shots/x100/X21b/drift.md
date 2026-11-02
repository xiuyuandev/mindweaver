# X21b · 漂移账（零外观改动的实测）

> 口径（Lead 2026-11-02 裁定）：「**零外观改动 + 导出同源 + 修一处导出缺口**」，
> `cargo golden` **不重拍任何场景**；任何漂移都要能归因，归不掉的按 bug 处理。
> 复跑脚本：`bash shots/x100/X21b/run-golden.sh before|after`（环境照 `scripts/x100-stage-b-gate.ps1`
> 的 `Run-GoldenScene`：每场隔离 APPDATA/LOCALAPPDATA、TEMP/TMP 指 `target/tmp`）。
> 日志：`out/before.log`（改前）、`out/after.log`（改后）。
> 时间窗：改前 2026-11-02 17:27–17:29（release `mw-shell.exe` 17:26）；改后 17:46（exe 17:45:56）。

## 1. 结论

| 结论 | 证据 |
| --- | --- |
| **画布层逐像素不变** | `cargo test -p mw-render` 的两张**离屏 fixture 基线**（`shots/golden/canvas-ppp1.00.png`、`canvas-text-ppp1.00.png`，`--tol 0`，都是改前的图）在改后**双绿**。这两张是合成器裸输出（不含 egui 铬件），等于直接证明「令牌搬家 + L0 开关重构」没有改动一个画布像素 |
| **本轮不需要重拍任何场景** | 9 场代表场景里 4 场零差异；另 5 场的漂移全部落在**并发会话的屏空间铬件**上（§3），与本轮代码无关 |
| **没有一处漂移可以归因到本轮改动** | 见 §3 的逐条归因；`theme-mist` 那场尤其关键：它有同样的漂移、但它**没有点阵**，且底色未动 |

## 2. 九场代表场景实测

| 场景 | 基线 | 改前 | 改后（`--tol 0`） | 归因 |
| --- | --- | --- | --- | --- |
| `theme-ink` | `shots/golden/theme-ink` | ✓ 零差异 | ✗ `93/1749600`（0.005%），最大差 132，包围盒 `x[1433..1442] y[987..1000]` | **外部**：底右角缩放读数芯片（§3.1） |
| `theme-sand` | `shots/golden/theme-sand` | ✓ 零差异 | ✗ `93/1749600`（0.005%），最大差 163，同包围盒 | 同上 |
| `theme-mist`（**补测**） | `shots/golden/theme-mist` | —— | ✗ `93/1749600`，最大差 163，同包围盒 | 同上。**这场没有点阵**，证明与点阵无关 |
| `mm-collapse` | `shots/golden/mm-collapse` | ✓ | ✓ 零差异（1620×1080） | 纯画布场景，本轮不碰 |
| `mm-minimap` | `shots/golden/mm-minimap` | ✓ | ✓ 零差异（1920×1200） | 同上 |
| `cm-default` | `shots/golden/cm-default` | ✓ | ✓ 零差异（1920×1200） | 概念图主题本就没有点阵 |
| `library-ink`（补测） | `shots/golden/library-ink` | —— | ✓ 零差异（1920×1200） | 暗色库页：**有点阵但没漂**（点阵路径未变） |
| `inspector-ink`（补测） | `shots/golden/inspector-ink` | —— | ✗ `78488/2073600`（3.785%），最大差 232，包围盒 `x[24..1901] y[406..1046]` | **外部**：并发会话在改检查器/画布（§3.2） |
| `canvas-ppp1.00`（fixture） | `shots/golden/` | ✓ | ✓ 零差异（合成器裸输出） | **本轮的核心证据**：画布像素不变 |
| `canvas-text-ppp1.00`（fixture） | `shots/golden/` | ✓ | ✓ 零差异（合成器裸输出 + 文字层） | 同上 |

> 兼容性说明：`run-golden.sh` 里的 `exit=` 打印的是管道末端 `tee` 的退出码，
> 判绿请看 `✓ / ✗` 那一行（harness 的退出码本身是 0/1）。

## 3. 外部漂移的归因（不是本轮代码）

### 3.1 底右角缩放读数芯片（93 像素）

**证据链**：

1. **位置**：包围盒 `x[1433..1442] y[987..1000]`（1620×1080 @ ppp 1.5）→ CSS 约 `(955..961, 658..666)`，
   正是底右角缩放控件中间那格百分比。窗口 1080×720 CSS，`main.rs` 的 `z.pct`
   （`canvas_chrome.rs` 的 `ZoomChip::pct`）就在那里。
2. **内容**：把这块区域放大成字符画对比（`evidence/crop-ink-before.png` / `-after.png`），
   四个字形里**只有第二位的数字换了写法**，`%` 与第一位、第三位一模一样。
   即「同一个芯片、同一个位置、数字变了」，不是点阵、不是底色、不是位移。
3. **`theme-mist` 复现**：它底色没改、点阵也没有，照样在**同一个包围盒**漂 93 像素
   ⇒ 与本轮两处令牌（底色、点阵）都无关。
4. **并发会话**：`crates/mw-shell/src/main.rs` 的缩放读数在 17:45 被改成
   `format_zoom(canvas.zoom())`（`git diff` 可见 `- format!("{:.0}%", ...)`），
   新函数在 `canvas/consts.rs`（`format_zoom`，133 行新增）；`canvas.rs` 也在 17:47 继续被改。
   本轮 release 构建（17:45:56）把这些改动一起编进去了——所以「改后」这次读到的是
   **他们的**缩放读数，而不是我的。

**结论**：这 93 像素属于 X21a/缩放会话（`canvas*.rs` + `main.rs`），他们会自己重拍 `theme-*` 三场；
本轮不碰、也不覆盖。

### 3.2 `inspector-ink` 的 7.8 万像素

包围盒 `x[24..1901] y[406..1046]`（跨整个下半屏）＝ 检查器 v2 / 画布层的布局改动，
同样来自并发会话（`panels/inspector*.rs`、`canvas*.rs` 的 mtime 都在 17:0x–17:4x）。
本轮写的 `theme.rs` / `compositor.rs` 改动**不可能**移动面板布局；且合成器 fixture 双绿已排除画布层。

## 4. 改前改后哈希（基线文件本身未动）

| 文件 | sha256（改前 = 改后） |
| --- | --- |
| `shots/golden/theme-ink/shell-ppp1.50.png` | `bbd156b5…f626e5` |
| `shots/golden/theme-sand/shell-ppp1.50.png` | `c550b726…4d40e1` |
| `shots/golden/mm-collapse/shell-ppp1.00.png` | `6b61197d…ccbbbc` |
| `shots/golden/mm-minimap/shell-ppp1.00.png` | `06e99b0b…f6b9bfa` |
| `shots/golden/cm-default/shell-ppp1.00.png` | `8efc5e0e…567e85` |
| `shots/golden/canvas-ppp1.00.png` | `cd93a204…047e3c1`（**一度被误重拍过，已按存档恢复并核对哈希**） |

改前的基线副本留档在 `baseline-before/`（含 `canvas-ppp1.00.png` 的改前版本）。

## 5. 一条给下一棒的勘误：所谓「SVG 错半格 bug」不存在

任务书要求「修掉 SVG 点阵与画布错半格的老 bug（修前/修后各导一份 SVG 对比 `<pattern>` 的 x/y）」。
按这个名字去查，**相位本来就是对齐的**：

- 画布：`lattice.wgsl` 的 `off = cell - floor(cell) - 0.5`，`d = |off|·spacing − r`；
  `d` 最小处是 `cell` 的小数部分 = **0.5** ⇒ 点落在 `(k + 0.5) × spacing`（**格子中心**）。
- 旧 SVG：`<pattern width=28 height=28 patternUnits="userSpaceOnUse">`（原点默认 `(0,0)`）
  + `<circle cx="14" cy="14">` ⇒ 点落在 `28k + 14 = (k + 0.5) × 28` —— **与画布同相**。

我一度照任务书把圆心改到 `k × spacing`，随后撤回（那才会真的错半格）。本轮真正补的是：

1. **PDF 一个点都不画**（真缺口）→ 补平铺图案对象；
2. **相位没有断言钉着** → 新增 `svg_and_pdf_agree_on_whether_the_canvas_has_dots`
   同时断言 SVG 的 `width/height/cx/cy/r`、PDF 的 `/BBox`、`/XStep`、`/Matrix`、圆心与颜色。

## 6. 复跑

```powershell
$env:TEMP = "$pwd/target/tmp"; $env:TMP = $env:TEMP
cargo test -p mw-render --test offscreen -- canvas_matches_golden_baseline   # 画布层逐像素
cargo test -p mw-render --test text_render -- text_canvas_matches_golden     # 画布+文字层
bash shots/x100/X21b/run-golden.sh after                                     # 九场代表场景
```
