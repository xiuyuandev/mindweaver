# X21b · 画布底色 + 四主题点阵纹理 + 导出同源

> 阶段：Stage C 第 1 轮（`docs/24` §3 阶段 C 的 X21 行）
> 切片：**X21b** = X21 的第二刀。X21a 的合同与边界见 [`shots/x100/X21/report.md`](../X21/report.md)；
> 该文件 §7.2 第 6 条把「画布底色 / 点阵纹理 / SVG·PDF 点阵同源」三项划给本轮。
> 状态：**已实现**（不是 `已验证`：人工门未走、实机 DPI/观感未验；包状态以 `docs/STATUS.md`
> 包表为准）；代码与单测已落地，两道正式门的 golden 均零差异（`stage-b` 356/356 PASS、
> `stage-cm` 146/146 但整门红在并行会话的 `fmt`/`clippy`，见 §5.2/§5.3），漂移量见 `drift.md`
> 决策来源：团队任务 `task-5` 的**第一条**口径 + Lead 2026-11-02 的**驳回与裁决**
> （本文件 §1.2 逐条记录）。裁决依据是 `docs/06` §2.1 的实拍结论表，不是口味。
> 写作范围：本文件与 `shots/x100/X21b/`。代码只动
> `crates/mw-core/src/theme.rs`、`crates/mw-render/src/{compositor.rs,instances.rs,shaders/lattice.wgsl}`、
> `crates/mw-shell/src/{export_svg.rs,export_pdf.rs}`。

## 0. 一页结论

**本轮验收口径（Lead 2026-11-02 裁定）：「零外观改动 + 导出同源 + 修一处导出缺口」，
`cargo golden` 不重拍任何场景；`stage-b` / `stage-cm` 里任何漂移都算 bug。**

| # | 合同 | 改前（2026-11-02 代码实测） | 本轮 |
| --- | --- | --- | --- |
| A | 点阵令牌落进 `Theme`：间距 / 半径 / 颜色（含透明度）/ 开关 | 只有颜色一条令牌 `ui.dot`；间距与半径是 `mw-render::instances` 里的两个**全局常量**（`GRID_SPACING = 28.0`、`GRID_DOT = 1.1`），主题换不了 | 新增 `ui.dot_spacing` / `ui.dot_radius` + `mw_core::theme::{DOT_SPACING, DOT_RADIUS}`；删掉 `mw-render` 那份 |
| B | 底色/点阵有**唯一出口**，画布 / SVG / PDF 都读它 | 三条路径各自读 `theme.ui.dot` + 两个全局常量；PDF 干脆不画（代码里写着「点阵：矢量导出不铺几万个圆点…只保留画布底色」） | `Theme::canvas_dots() -> Option<CanvasDots>`：三条派生路径全部改读这一句 |
| C | SVG 与 PDF 对「有没有点阵」的判断必须一致 | **不一致**：同一份 `ink` 主题，SVG 有 `<pattern id="zdots">`，PDF 没有 | 两家都按 `canvas_dots()` 判断；PDF 补一个平铺图案对象 |
| D | 相位（点的位置）在画布与导出之间必须同源 | 画布的点在**格子中心** `(k+0.5)×spacing`（`lattice.wgsl` 的 `off = cell - floor(cell) - 0.5`）；SVG 的 `<pattern>` 从 (0,0) 铺、圆心在 `(spacing/2, spacing/2)` —— **本来就同相**。但这条一致性没有任何断言钉着 | 保持原相位，并加断言把「画布公式 → 两个导出器」钉死（见 §5.2 的说明：任务里说的「错半格 bug」经核实**不存在**） |
| E | **不做**：不改画布底色、不给亮色主题加点阵 | `snow` 画布 `#FCFBF8`、`mist` `#F3F8F6`、`sand` `#F8F2E9`、`ink` `#17191C`；只有 `ink` 有 `dot = #3C424A` | **原样保留**（§1.2）：`docs/06` §2.1 的实拍结论是「画布纯色近白、没有点阵」，且把「点阵 24px、处处可见」标成被否掉的旧实现 |
| F | 零漂移 | — | `stage-b` / `stage-cm` / `mw-render` 的 fixture 基线**一张都不重拍**；实测见 `drift.md` |

## 1. 范围

### 1.1 做（本轮全部交付物）

1. **令牌单一出口**（`mw-core/src/theme.rs`）

   ```rust
   pub struct ThemeUi {
       // …既有字段不动…
       pub canvas_bg: Rgba,
       pub dot: Rgba,          // 既有：alpha == 0 即「这套主题不铺点阵」
       pub dot_spacing: f32,   // 新增：格距，世界单位（随缩放）
       pub dot_radius: f32,    // 新增：半径，CSS px（屏幕恒定）
   }
   pub const DOT_SPACING: f32 = 28.0;   // 唯一来源（值 = X21 之前那份，一个像素都没动）
   pub const DOT_RADIUS: f32 = 1.1;

   /// 画布 / SVG / PDF 的唯一出口：`None` = 不铺点阵。
   pub struct CanvasDots { pub spacing: f32, pub radius: f32, pub color: Rgba }
   impl Theme { pub fn canvas_dots(&self) -> Option<CanvasDots>; }
   ```

   `CanvasDots` 上另有两个**唯一实现**的小函数，三条派生路径共用，避免「同一个数在三处各算一遍」：
   `blended(bg)`（把点色按 alpha 叠成不透明色：画布预乘混、SVG 由查看器混、PDF 只能先混成实色）、
   `vector_radius()`（导出半径，按 1 CSS px = 1 世界单位，并钳在半格内）。

2. **四套内置主题的令牌表**（值 = X21 之前的行为，逐条对齐）

   | 主题 | 画布底色 | 格距 | 半径 | 点色 | 点阵 |
   | --- | --- | --- | --- | --- | --- |
   | `snow` 纸感（默认） | `#FCFBF8`（不动） | 28 | 1.1 | 透明 | **关**（纯色纸底） |
   | `ink` 墨夜 | `#17191C`（不动） | 28 | 1.1 | `#3C424A` | **开**（现观感，暗色基线钉着它） |
   | `mist` 雾青 | `#F3F8F6`（不动） | 28 | 1.1 | 透明 | 关 |
   | `sand` 暖砂 | `#F8F2E9`（不动） | 28 | 1.1 | 透明 | 关 |
   | `cm-*` 四套概念图 | 各自纸色（不动） | 28 | 1.1 | 透明 | 关（概念图是纯纸） |

3. **三条派生路径改读同一个出口**
   - 画布：`compositor::dot_layer(theme, transparent, low_render)` 纯函数拼 uniform
     `grid = [spacing, radius]`、`dot = color`；`None`/透明底/省渲染 → 全 0 且**跳过 L0 绘制**。
   - SVG：`<pattern id="zdots">` 的 `width/height`、圆心、半径、颜色全部来自 `CanvasDots`。
   - PDF：新增**平铺图案对象**（`/PatternType 1`，整份 PDF 只有一个），页内容流里以
     `/Pattern cs /P1 scn` + `re f` 铺一层；透明底/无点阵主题不建该对象、页资源也不带 `/Pattern`。

4. **单测**（§4）与**零漂移证据**（`drift.md`）。

### 1.2 不做（Lead 驳回的两项，依据写清）

Lead 2026-11-02 的裁决原文要点：「驳回底色与『四主题点阵』这两项。依据是权威文档的口径，不是我的口味。」
依据 = `docs/06` §2.1「从实拍提炼的硬结论」那张表：

- **画布背景**：XMind 2026 是「纯色近白，**没有点阵**」；同表把「点阵 24px，处处可见」明确标成
  「旧实现（我们此前）」并**否掉**；表末还有一句「旧文档里『点阵背景 8%』的示意**已作废**」。
- 因此：`#FBFBFC` **改回** `#FCFBF8`（暖白是 WP2「纸感」的刻意选择；`docs/06` §2.2 那张令牌表
  是「素白 v2」历史口径，而 `AGENTS.md` §4 写明 `docs/06` 只作**参考**）；
  `snow` / `mist` / `sand` **保持 `TRANSPARENT`**，只有 `ink` 有点阵；
  格距**保持 28**（`docs/06` 的 24 是被否掉的旧实现读数，不是目标值）。

| 不做 | 为什么 |
| --- | --- |
| 改画布底色 | 见上。改了 = 250 场基线全漂，而方向与实拍结论相反 |
| 给三套亮色主题加点阵 | 同上。`docs/02` §「背景样式…随主题」是 P1，且实拍结论是「没有点阵」 |
| 「纯色 / 点阵 / 网格三选」用户开关 | P1，且要文档级新字段（违反「不新增字段」） |
| 「导出是否含背景纹理」勾选框 | 导出对话框在 `main.rs`，不在本轮写作用域 |
| 主题编辑器里的点阵滑杆 | `theme_studio*.rs` 不在写作用域 |
| 改 `canvas*.rs` / `panels/` / `mw-io` / `docs/` / `scripts/` | 并发纪律 |
| 批量重拍基线 | 本轮零外观改动 ⇒ 不需要 |

## 2. 设计合同

### 2.1 不变量

1. **间距 > 0、直径（2×半径）< 间距、alpha ∈ (0,255]** 才叫「开」；脏值（0 / 负 / `NaN` / `∞`）
   一律降级成「不画」。
2. 格距是**世界单位**（随缩放），半径是 **CSS px**（屏幕恒定）——与 `lattice.wgsl`
   （`pitch = spacing·scale·ppp`、`r_world = radius / scale`）逐字一致。
3. **导出按 1 CSS px = 1 世界单位**画（= 100% 缩放的观感）。导出没有「当前缩放」，
   `docs/30` §5.8 要求的是同一份 `Scene`，而 `Scene` 与缩放无关。
4. **相位 = 格子中心 `(k + 0.5) × spacing`**。这是画布的定义（`off = cell - floor(cell) - 0.5`），
   导出必须同相：SVG `<pattern>` 从 (0,0) 铺、圆心在 `(spacing/2, spacing/2)`；
   PDF 图案 `/Matrix` 取单位阵、圆心同样画在格子中心。
5. **合成器开关优先于令牌**：透明底、省渲染都不画点阵，且**不许改主题**
   （`canvas_bg` 兼着挖空色，改令牌会把标签胶囊的字一起擦掉——`docs/13` 教训 45）。
6. **`ui.dot` 的「alpha = 0 即关」语义保留**：`crates/mw-shell/src/demo.rs` 靠它关点阵，
   而该文件不在本轮写作用域。

### 2.2 为什么 PDF 用平铺图案，而不是逐点画圆

一页 A4 约 `37 × 33 ≈ 1200` 个点；逐点写连会让内容流涨到百 KB 级、多页翻倍，而那正是旧代码
「PDF 干脆不画点阵」的理由。图案对象把代价摊回常数：**整份 PDF 只有一个**，页数再多也不重复。
`/TilingType 1` = 恒定间距（打印/查看器都按常数步进，不重排）。

### 2.3 失败与边界

| 情形 | 行为 |
| --- | --- |
| `dot` 的 alpha = 0（三套亮色主题、概念图、演示模式） | 三家都「没有点阵」：画布跳过 L0、SVG 不写 pattern、PDF 不建图案对象 |
| `dot_spacing/radius` ≤ 0 或非有限 | 同「不画」（不 panic、不铺 `NaN` 个点） |
| 透明底导出 | 画布与 SVG 都不画；PDF 没有透明底模式，页底永远是画布色 |
| 省渲染（帧预算 < 45fps） | 画布不画点阵；导出照画（省渲染是**上屏**降级，不是导出语义） |
| 半径 ≥ 间距/2 | `vector_radius()` 钳到 `spacing/2 - 0.01`，防止连成一条实线 |

## 3. 文档口径对照（不动 `docs/`，只点名）

**权威顺序**（`AGENTS.md` §4/§9）：`docs/28` / `docs/30` 是当前设计合同；`docs/06` 只作外壳/令牌**参考**，
`docs/06` 的部分旧规格已被取代。本轮的设计来源是 `docs/24` §3 X21 行 + `docs/02` 功能清单，
**但外观裁决以 `docs/06` §2.1 的实拍硬结论为准**（Lead 2026-11-02）。

| # | `docs/06` 的行（快照） | 本轮口径 |
| --- | --- | --- |
| 1 | §2.1 表「画布背景」行：「纯色近白，**没有点阵**；暗色主题是纯深灰」 | **照做**：亮色主题不铺点阵，只有 `ink` 有 |
| 2 | §2.1 表同一行右列「点阵 24px，处处可见」+ 表末「旧文档里『点阵背景 8%』的示意已作废」 | **明确不采用**：那是被否掉的旧实现；格距保持 28（M1b 起画布的唯一一份几何） |
| 3 | §2.2 令牌表「背景纹理 \| 默认无；点阵作为可选背景（P1）\| 导出默认剔除」 | 前半句照做（默认无）；**后半句本轮反过来**：导出不再「剔除」，与画布同源——但只有 `ink` 有，等于「默认导出也没有」 |
| 4 | §2.2 令牌表「画布底色 `#FBFBFC`」 | **不采用**：那是「素白 v2」历史口径；暖白 `#FCFBF8` 是 WP2「纸感」的刻意选择 |
| 5 | §2.1 暗色画布 `#17191F` 与代码 `#17191C` 差 3 个色阶 | 既有历史差异，本轮**不动**（改了只会白漂一串暗色基线） |

## 4. 验收场景（机器判据）

| ID | 场景 | 判据 | 落点 |
| --- | --- | --- | --- |
| AC-1 | 四套主题点阵令牌合法 | 逐主题：`spacing`/`radius` 有限且 > 0；`2·radius < spacing`；两者 = `DOT_SPACING`/`DOT_RADIUS`；alpha 只能是 0 或 255 | `mw-core` `all_builtin_themes_ship_legal_dot_tokens` |
| AC-2 | 只有墨夜铺点阵，且保持现值 | `snow`/`mist`/`sand`：`dot == TRANSPARENT` 且 `canvas_dots() == None`；`ink`：`dot == #3C424A`、`is_on()`、等效对比度落在 `[1.40, 2.10)` | `mw-core` `only_the_dark_theme_paints_dots_and_it_keeps_its_current_colour` |
| AC-3 | 默认主题底色不回退到 `#FBFBFC` | `snow.ui.canvas_bg == #FCFBF8`、≠ 纯白、`canvas_dots() == None` | `mw-core` `the_default_canvas_stays_warm_paper_and_paints_no_dots` |
| AC-4 | 关点阵的两条来源 | `ui.dot = TRANSPARENT` → `None`（演示模式语义）；四套 `cm-*` → `None` | `mw-core` `turning_the_dot_colour_off_means_no_dots` |
| AC-5 | 脏令牌降级 | `spacing ∈ {0,-1,NaN,∞}`、`radius ∈ {0,-2,∞}` → `None`；半径过大 → `vector_radius()` 钳在半格内 | `mw-core` `dirty_dot_tokens_degrade_to_no_dots` |
| AC-6 | 三条路径同一个等效色 | `ink` 不透明点混完 == 它自己；低 alpha 点混完是不透明色且分得出来 | `mw-core` `dot_blend_is_the_one_value_the_three_paths_share` |
| AC-7 | 画布 uniform 只由令牌决定 | `dot_layer(ink)` = `([28,1.1], rgba(#3C424A))`；`dot_layer(snow)` = 全 0；透明底/省渲染 = 全 0 且**主题不变**；`ui.dot` 透明 → 全 0 | `mw-render` `dot_layer_reads_the_theme_tokens_and_respects_the_compositor_switches` |
| AC-8 | **SVG 与 PDF 开关一致** | 6 套主题（4 思维导图 + 2 概念图）：`svg.contains("id=\"zdots\"") == pdf.contains("/PatternType 1") == theme.canvas_dots().is_some()` | `mw-shell` `svg_and_pdf_agree_on_whether_the_canvas_has_dots` |
| AC-9 | 相位与半径同源 | SVG `width/height = spacing`、`cx/cy = spacing/2`、`r = vector_radius()`；PDF `/BBox [0 0 s s] /XStep s /YStep s`、`/Matrix [1 0 0 1 0 0]`、圆心在格子中心且颜色 = `blended(bg)` | 同上 |
| AC-10 | 透明底不带点阵/底色 | 透明 SVG 无 `id="zdots"`、无 `fill="#fcfbf8"` | 同上 |
| AC-11 | 零漂移 | 9 场代表场景 `--tol 0` 零差异；`mw-render` 两张 fixture 基线（`canvas-ppp1.00`、`canvas-text-ppp1.00`）零漂移 | `drift.md` |
| AC-12 | 工程门 | `cargo test -p mw-core -p mw-render -- --test-threads=1` 绿；`cargo fmt --all`；clippy 新增 0；不 commit | `gate-*.log` |

### 4.1 一处需要说清的事：所谓「SVG 错半格 bug」经核实**不存在**

任务书要求「修掉 SVG 点阵与画布错半格的老 bug（修前/修后各导一份 SVG 对比 `<pattern>` 的 x/y）」。
我按这个名字去查，结论是**相位本来就是对齐的**，没有半格偏差：

- 画布：`lattice.wgsl` 里 `off = cell - floor(cell) - 0.5`，`d = |off|·spacing - r`；
  `d` 最小处是 `cell` 的小数部分 = **0.5**，即点落在 `(k + 0.5) × spacing`（**格子中心**）。
- 旧 SVG：`<pattern width=28 height=28 patternUnits="userSpaceOnUse">`（原点默认 `0,0`）+ `<circle cx="14" cy="14">`
  ⇒ 点落在 `28k + 14` = `(k + 0.5) × 28` —— **与画布同相**。

所以本轮**没有**改相位（我一度按任务书的说法把圆心挪到 `k×spacing`，随即撤回）：改了反而会错半格。
真正的缺口是 **PDF 一个点都不画**，以及**没有任何测试钉住这条相位**——本轮补的就是这两件，
并把相位写成断言（AC-9）。这一点在 `drift.md` §5 也重复了一遍，避免下一棒照着错误结论再动一次。

## 5. 证据

| 类 | 内容 | 落点 |
| --- | --- | --- |
| 行为 | AC-1–AC-10 共 8 条新单测（`mw-core` 6 / `mw-render` 1 / `mw-shell` 1） | 各自 `#[cfg(test)]`，名字见 §4 |
| 视觉 | 改前/改后 9 场代表场景 `--tol 0` 实测；画布层 fixture 双绿（**画布逐像素不变**） | [`drift.md`](drift.md)、`out/before.log`、`out/after.log`、`baseline-before/` |
| 导出 | `ink` 主题真实导出的 SVG / PDF 各一份，`<pattern>` 与 `/PatternType 1` 逐字段对比 | `evidence/export-ink.svg`、`evidence/export-ink.pdf`、[`evidence/export-ink-pattern.md`](evidence/export-ink-pattern.md) |
| 工程 | `mw-core`+`mw-render` 444 条测试全绿；`mw-shell` 导出 46 条全绿；fmt；clippy 新增 0 | 本节 §5.3 |
| 屏空间漂移 | 5 场的漂移逐条归因到并发会话（缩放读数芯片 / 检查器布局），无一可归因到本轮 | [`drift.md`](drift.md) §2–§3 |

### 5.1 导出同源的实际产物（`ink`，同一份 `Scene` / 同一份 `Theme`）

由 `MW_X21B_EVIDENCE=1 cargo test -p mw-shell svg_and_pdf_agree` 落盘（只在显式设了环境变量时才写文件，
正常跑测试零副作用）：

```
SVG:  <pattern id="zdots" width="28" height="28" patternUnits="userSpaceOnUse">
        <circle cx="14" cy="14" r="1.1" fill="#3c424a"/></pattern>

PDF:  << /Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1
        /BBox [0 0 28 28] /XStep 28 /YStep 28 /Matrix [1 0 0 1 0 0] /Resources << >> >>
      stream: q 0.24 0.26 0.29 rg  15.1 14 m … 14 12.9 c … f Q
      页对象: /Resources << /Pattern << /P1 N 0 R >> >>    内容流: /Pattern cs /P1 scn
```

三处对得上：**格距 28**（`DOT_SPACING`）、**半径 1.1**（`DOT_RADIUS` / `vector_radius()`）、
**圆心在格子中心 (14,14)**（= `(k+0.5)·28`，与 `lattice.wgsl` 同相）；
颜色 `#3c424a` = `blended(canvas_bg)`（PDF 无透明度，故是混好的实色，`0.24 0.26 0.29 ≈ 60,66,74`）。

### 5.2 零漂移

- **画布层逐像素不变**：`cargo test -p mw-render -- canvas_matches_golden_baseline` 与
  `-- text_canvas_matches_golden` 用的是**改前的** fixture 基线（`--tol 0`），改后**双绿**。
  这两张是合成器裸输出（不含 egui 铬件），等于直接证明令牌搬家 + L0 开关重构没动一个画布像素。
- **代表场景**：`mm-collapse`、`mm-minimap`、`cm-default`、`library-ink` 四场零差异；
  `theme-ink` / `theme-sand` / `theme-mist` 各漂 93 像素（0.005%），全部落在**底右角缩放读数芯片**
  （包围盒 `x[1433..1442] y[987..1000]`），是并发会话改 `main.rs` 的 `format_zoom` 读数造成的；
  `theme-mist` **没有点阵**却漂在同一个包围盒，这一条就排除了点阵嫌疑。
- **没有重拍任何场景基线**；`shots/golden/canvas-ppp1.00.png` 一度在探索阶段被误重拍，
  已按 `baseline-before/` 的存档恢复，sha256 `cd93a204…047e3c1` 与原始一致。
- 详细归因（含 `inspector-ink` 的 7.8 万像素）见 [`drift.md`](drift.md) §2–§3。
- **19:0x–20:1x 的门事实（最终）**：19:05 那次的 `stage-b` 177/178、`stage-cm` 70/73 已判为
  **并发干扰下的假红**（并行会话 `rustc` 构建 + 跑 `mw-shell` 抢 CPU/GPU，harness 超时；
  `mm-rel-link-created` / `mm-summary-hover` / `card-autosave` / `search-text` / `mm-zoom-wheel`
  单场景复跑逐个通过，19:20–19:30 那次两遍各 10 场失败还互不相同）；19:35–19:47 冻结二进制
  （mtime 19:32:30）两遍 `stage-b` 356/356、`stage-cm` 146/146 零差异，先把红定性为抢占。
  随后**两道正式门整跑**：
  - **`stage-b` 20:15 `MACHINE GATE PASS`**（19:48:38–20:15:05，`target/x100/stage-b-gate.log`）：
    X11–X19 证据包 PASS、PowerShell 语法 PASS、`cargo fmt` PASS、`mw-shell` clippy 新增 0、
    全量串行测试 **1328 passed / 0 failed / 0 ignored**、golden 178×2 = **356 次比较零差异**。
  - **`stage-cm` 20:19 `MACHINE GATE FAIL`**（20:15:14–20:19:32，`target/x100/stage-cm-gate.log`）：
    门内 `.zmind` 往返 PASS、全量测试 PASS（1335/0）、golden 73×2 = **146 次比较零差异**；
    **红只有 `fmt` 与 `clippy`，且全落在并行会话正在改的文件上**——`crates/mw-core/src/geom.rs`、
    `crates/mw-shell/src/canvas/tests.rs` 未格式化，`crates/mw-shell/src/canvas/canvas_methods_3.rs:588`
    一条新增 clippy 警告。
  **严格区分：`stage-b` 门绿了；`stage-cm` 门没绿（红在别人在飞的代码上）——不许写「两道门全绿」。**
  真实漂移只剩 `cm-inspector` / `cm-inspector-pinned`（检查器组内行距 8→6，只漂面板区），
  已重拍，矩阵与 `stage-cm` 门内均零差异。本轮的场景（`theme-ink` / `theme-sand` / `theme-mist` /
  `cm-default` / `mm-collapse` / `mm-minimap` / `library-ink`）**全在零差异名单里**。
  注：本节上面记的 `theme-*` 那 93 像素是当时并行会话**中间态**的 `format_zoom` 读数；X21 收口后
  读数与基线逐字一致，所以这 3 场不必重拍，两道门的 golden 里也确实零差异。

### 5.3 工程门

| 命令 | 结果 |
| --- | --- |
| 全量 `cargo test --workspace -- --test-threads=1`（TEMP→`target/tmp`） | **1328 passed / 0 failed / 0 ignored**（`stage-b` 正式门内 20:15 实测，含本轮新增 8 条单测；19:0x 那次是 1313/0/1 ignored，已被门内读数取代） |
| `cargo test -p mw-core -p mw-render -- --test-threads=1` | **444 passed / 0 failed**（19:0x 分域快照：`mw-core` 368、`mw-render` 33+11+4+12+9+7） |
| `cargo test -p mw-shell export -- --test-threads=1` | **46 passed / 0 failed**（含本轮跨导出器那条） |
| `cargo fmt --all -- --check` | `stage-b` 门内 **PASS**（20:15）；`stage-cm` 门内 **FAIL**——红在并行会话的 `crates/mw-core/src/geom.rs` 与 `crates/mw-shell/src/canvas/tests.rs` |
| `cargo clippy -p mw-core -p mw-render --all-targets` | 新增 **0**（余下全是历史项：`theme.rs` 的 `rgba` 构造函数、`compositor.rs:121` 的 `manual_slice_size_calculation` 等） |
| `cargo clippy -p mw-shell --all-targets` | `stage-b` 门内新增 **0**（PASS，20:15）；`stage-cm` 门内 FAIL——`crates/mw-shell/src/canvas/canvas_methods_3.rs:588:30` 一条新增警告（并行会话所改） |
| `stage-b` / `stage-cm` 正式门 | 登记 **178/356** 与 **73/146**；**`stage-b` 20:15 `MACHINE GATE PASS`**（golden 356/356 零差异）、**`stage-cm` 20:19 `MACHINE GATE FAIL`**（门内 golden 146/146 零差异，红只在 `fmt`/`clippy` 且都在并行会话的文件上）。**不可写「两道门全绿」** |


## 6. 风险与未做

### 6.1 风险

| # | 风险 | 处理 |
| --- | --- | --- |
| R1 | **PDF 多了一个对象**，可能撞上既有断言「内容流可读 / 不嵌位图」 | 现测：`output_is_a_real_vector_pdf_without_images` 绿（图案对象是独立对象，`/Subtype /Image` 仍然没有） |
| R2 | 别的会话正在改 `canvas*.rs` / `mw-io` | 本轮实证：改前 5 场全绿、改后仍全绿，说明并行改动对这几场无影响；异常时按外部原因隔离报告 |
| R3 | 令牌搬家后有人又在 `mw-render` 写一份常量 | `instances.rs` 里留了注释说明「唯一来源在 `mw_core::theme`」 |
| R4 | 下一棒把「相位」改错方向（误信任务书里那条不存在的 bug） | §4.1 + `drift.md` §5 双写，并用 AC-9 断言钉住 |
| R5 | 概念图主题的令牌从 `0.0/0.0` 改成 `28.0/1.1` | 只影响「令牌合法性」的读法：`canvas_dots()` 仍返回 `None`（alpha=0），画布/导出一个像素不变 |

### 6.2 未做（如实记账）

1. 「纯色 / 点阵 / 网格」三选开关（要文档级新字段，违反硬约束）。
2. 「导出是否含背景纹理」勾选框（`main.rs`，不在写作用域）。
3. 主题编辑器里的点阵滑杆（`theme_studio*.rs`，不在写作用域）。
4. 实机 150%/200% DPI 下的点阵观感（与全仓其它包同样欠实机）。
5. 亮色主题是否**将来**要加点阵：留给用户/设计拍板；令牌与单一出口已就位，届时只改一行 alpha。

## 7. 回滚

纯加法，一行回滚：

```rust
// mw-core/src/theme.rs — 任何主题把点色写成 TRANSPARENT 即退回本轮之前的行为
dot: Rgba::TRANSPARENT,
```

PDF 的图案对象/`/Resources` 分支只在 `canvas_dots().is_some()` 时生效；`None` 时 PDF 字节与
本轮之前**逐字节可控**（页对象少 `/Pattern`、末尾少一个对象）。

## 8. 变更记录

- 2026-11-02 17:2x（X21b，进行中）：报告先行，按 `task-5` 第一条口径（四主题点阵 + 底色 `#FBFBFC`）起草。
- 2026-11-02 17:4x（X21b，进行中）：**Lead 驳回外观两项**，按 §1.2 回退 `snow` 底色与三套亮色点阵；
  范围收敛为「单一出口 + 导出同源 + PDF 补点阵 + 单测 + 零漂移」。本节以上内容已按新口径重写。
- 2026-11-02 17:5x（X21b，**已实现**）：本轮收口。回退清单 = `snow` 底色回 `#FCFBF8`、
  三套亮色主题回 `TRANSPARENT`、`shots/golden/canvas-ppp1.00.png` 按存档恢复、
  `crates/mw-render/tests/text_render.rs` **未动**（回退后它自己绿了）。
  保留清单 = 令牌单一出口（`Theme::canvas_dots` + `DOT_SPACING`/`DOT_RADIUS`）、
  `mw-render::instances` 那两个常量的删除、SVG/PDF 同源、PDF 平铺图案对象、8 条单测、零漂移证据。
  唯一「代码里多出来的一小块」：`export_pdf.rs` 的跨导出器测试里有一段
  `MW_X21B_EVIDENCE` 守卫的取证落盘（不设环境变量时不写任何文件）。
  另：Lead 在 17:5x 顺手把 `theme.rs` 里 `snow` 的文档注释改回暖白表述，与本文件口径一致。
- 2026-11-02 19:0x–20:1x（X21b，**已实现**，机器状态复核）：把本包放进整机口径再核一遍——
  `stage-b` 登记 **178 / 356**（第 178 场 `mm-zoom-wheel` 由 X21 收口补）、`stage-cm` 登记 **73 / 146**、
  golden 基线目录 **254**、输入脚本 **152**。门事实以两次**正式门整跑**为准：
  **`stage-b` 20:15 `MACHINE GATE PASS`**（19:48:38–20:15:05，`target/x100/stage-b-gate.log`：
  X11–X19 证据包 PASS、PowerShell 语法 PASS、`cargo fmt` PASS、`mw-shell` clippy 新增 0、
  全量串行测试 **1328 / 0 / 0 ignored**、golden 178×2 = **356 次比较零差异**）；
  **`stage-cm` 20:19 `MACHINE GATE FAIL`**（`target/x100/stage-cm-gate.log`：门内 `.zmind` 往返 PASS、
  全量测试 PASS、golden 73×2 = **146 次比较零差异**；**红只有 `fmt` 与 `clippy`，且全在并行会话
  正在改的文件上**——`crates/mw-core/src/geom.rs`、`crates/mw-shell/src/canvas/tests.rs` 未格式化，
  `crates/mw-shell/src/canvas/canvas_methods_3.rs:588` 一条新增警告）。
  19:05 那次的 177/178、70/73 是**并发假红**（并行会话构建 + 跑 `mw-shell` 抢 CPU/GPU、harness 超时；
  单场景复跑逐个通过，19:20–19:30 两遍各 10 场失败互不相同），19:35–19:47 冻结二进制两遍零差异先定性。
  真实漂移只剩 `cm-inspector` / `cm-inspector-pinned`（行距 8→6，只漂面板区），已重拍，门内零差异。
  本轮 9 场代表场景全在零差异名单里。
  **严格区分：`stage-b` 门绿了；`stage-cm` 门没绿（红在别人在飞的代码上）——不许写「两道门全绿」**，
  `stage-cm` 待对方收尾后重跑；门结论以 `docs/STATUS.md` §1 为准。
- **状态口径**：`已实现`（不是 `已验证`——人工门/Lead 验收还没走，且实机 DPI/像素观感未验）。
  包状态以 `docs/STATUS.md` 包表为准；本文件只写细节。三条结论不变：
  ① 四主题点阵令牌与底色**零外观改动**（`snow` 仍 `#FCFBF8`、三套亮色主题仍不铺点阵、格距仍 28）；
  ② 导出与画布**同源**（SVG/PDF 都读 `Theme::canvas_dots()`，PDF 点阵缺口关闭）；
  ③ 所谓「SVG 点阵错半格 bug」**经核实不存在**（§4.1），本轮反而把它写成断言钉住相位。

