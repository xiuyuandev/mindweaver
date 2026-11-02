# X23 · 节点卡片 / 主题形状 / 边框 / 圆角 / 阴影（现状审计）

> 谁做的：只读测绘（独立 agent）。**本文件只记录现状、差距与验收草案；本轮不动 `crates/`，
> 不拍任何 golden。** 状态词只用「未开始 / 进行中 / 延后」。
> 上一包 SE-13 已把**检查器侧**的形状 / 圆角 / 椭圆落地（`ResolvedStyle.radius`、`ELLIPSE_CODE`、
> `mw_render::corner_style`、形状四段含椭圆），所以 X23 的重点是**画布渲染侧的完整性**。
> 数字与门状态只以 [`docs/STATUS.md`](../../../docs/STATUS.md) §1 为准，本文不复制场次。

## 1. 范围与非目标

**范围**（`docs/24` §阶段 C 第 23 条：「重做节点卡片、主题形状、边框、圆角和阴影，覆盖常用形状」）拆成六问：

1. 形状集合齐不齐（枚举实际几个、检查器可达几个、XMind 常用还缺哪些）；
2. 圆角成不成体系（默认值、可覆盖范围、单位、上限与盒高耦合、钳制点）；
3. 边框成不成体系（宽度单位与屏幕恒定、四主题颜色来源是令牌还是派生）；
4. 阴影（画布节点有没有、合同怎么规定、其余浮层怎么给）；
5. 三处是否同一出口（画布 wgpu / SVG / PDF / 命中 / egui 编辑环）；
6. 四主题（雪 / 墨夜 / 雾青 / 暖砂，加概念图 `cm-*` 四套）是否一致。

**非目标（本轮明确不做）**：

- 不改任何 Rust 代码、不改 `docs/`、不新增 golden、不重拍基线；
- 不裁决「要不要加阴影 / 要不要加新形状」——那两条要用户拍板（见 §3 G-1 / G-2）；
- 不碰 X22（字阶/字重/行高）、X24（分支线）、X25（层级默认值）：边界只在「形状几何」这一层；
- 不评估实机 DPI/字体目检（欠账口径同 SE-01–13）。

**X23 状态：`进行中`（本文件只做了文档与现状审计）。** §3 各条的状态见该节表格。

## 2. 现状测绘（逐格标出处）

### 2.1 形状枚举：**5 个**，检查器可达 4 段 + 1 个 U 开关

`crates/mw-core/src/theme.rs:207-219`（`pub enum Shape {` 那一段）实际是 **5 个变体**：

| # | 变体 | `as_str` | 检查器入口 | 圆角规则 |
| --- | --- | --- | --- | --- |
| 1 | `Rounded` | `rounded` | 形状段第 1 格「圆角」 | `NODE_RADIUS = 10`（可被 `radius` 覆盖） |
| 2 | `Rect` | `rect` | 第 2 格「直角」 | `0.0`（可被 `radius` 覆盖） |
| 3 | `Capsule` | `capsule` | 第 3 格「胶囊」 | `r.h / 2.0`（不可覆盖） |
| 4 | `Ellipse` | `ellipse` | 第 4 格「椭圆」 | `ELLIPSE_CODE = -1_000.0` 哨兵 |
| 5 | `Underline` | `underline` | **不在形状段**，是 B / I 同排的 U 开关 | `0.0`；只画底边一条线 |

出处补充：`from_name` 未知值回退 `Rounded`（`theme.rs:222-230`）；`is_ellipse()` 供命中与导出判定；
`docs/30` §4.2 第 219 行写明「形状 | 四段分段控件 **圆角 / 直角 / 胶囊 / 椭圆**；下划线不做第五段——
240 面板下五个 12.5px 汉字格放不下」，即**第 5 个变体刻意不进分段控件**。
`docs/30` §5.3 第 306-307 行给概念盒的几何：椭圆 = 文本盒 **1.22（横）×1.16（纵）**外扩、文本盒不变；
圆角矩形 圆角 **10**（令牌 `radius::MD`）、宽 = 文本宽 + 28、最小 96、最大 240、高 36（双行 52）。

### 2.2 形状 × 圆角 × 边框 × 阴影 × 渲染 × 导出

| 形状 | 圆角半径（世界单位） | 边框 | 阴影 | wgpu 画法 | SVG / PDF 导出 |
| --- | --- | --- | --- | --- | --- |
| `Rounded` | `10` 或 `radius`（0–24，`theme.rs:1412` `r.clamp(0.0, 24.0)`） | `stroke_width`（默认见 §2.3） | **无** | `rect.wgsl` 的 `sd_round` 距离场（`shaders/rect.wgsl:27` 附近 `fn sd_round`） | `export_svg.rs:534` `<rect rx=ry=…>`；`export_pdf.rs:700` `let rad = radius.min(r.w*0.5).min(r.h*0.5)` |
| `Rect` | 同上（默认可为 0，也可被 `radius` 加点） | 同上 | **无** | 同一个 `sd_round`，`r = 0` | 同 `<rect>`；PDF 同上 |
| `Capsule` | `r.h / 2.0`（`instances.rs:981`） | 同上 | **无** | 同一个 `sd_round` | 同 `<rect rx=height/2>`；PDF 同上 |
| `Ellipse` | `ELLIPSE_CODE = -1_000.0`（`instances.rs:1001`） | 同上（沿椭圆边界） | **无** | `radius < -0.5` 走 `sd_ellipse`（`rect.wgsl:119` / `:123`） | `<ellipse cx cy rx ry>`（`export_svg.rs:517-533`）；PDF 四段三次贝塞尔（`export_pdf.rs:671-698`，系数 `0.552_284_8`） |
| `Underline` | 无卡片，只有底边线（`rect.bottom() - 1 CSS px`） | 线宽取 `stroke_width.max(1.0)`（CSS px） | **无** | `out[L_NODES].segs.push(seg(...))`（`instances.rs:429-436`） | `<line>`（`export_svg.rs:193-207`）；PDF 同走线路径（`export_pdf.rs:304`） |

### 2.3 四主题的默认值（形状 / 边框 / 圆角 / 填充）

| 主题 | 层级 | `shape` | 圆角 | `stroke` / `stroke_width` | 填充 |
| --- | --- | --- | --- | --- | --- |
| `snow` 纸感 | 根 | `Rounded` | 10 | `TRANSPARENT` / `0.0` | `#2C6658` 深绿实色 |
| | 1–3 级 | `Rounded` | 10 | `#EBE6DD`→`#EFEAE1` / `1.0` | 一级 = 调色板实色卡；二三级 = `level.fill.mix(branch, 0.07)` |
| `ink` 墨夜 | 根 | `Rounded` | 10 | `TRANSPARENT` / `0.0` | 深色实色 |
| | 1–3 级 | `Rounded` | 10 | `#394049`→`#30363E`→`#2C3239` / `1.0` | 同上规则 |
| `mist` 雾青 | 根 | `Rounded` | 10 | `TRANSPARENT` / `0.0` | — |
| | 1–3 级 | `Rounded` | 10 | `#DCE6E2`→`#E2EBE7`→`#E5EDE9` / `1.0` | — |
| `sand` 暖砂 | 根 | `Rounded` | 10 | `TRANSPARENT` / `0.0` | — |
| | 1–3 级 | `Rounded` | 10 | `#E6D8C7`→`#ECE0D0` / `1.0` | — |
| `cm-paper` / `cm-mint` | 单档 | **`Ellipse`** | 哨兵 | `ui.border` / `1.0` | `panel` |
| `cm-slate` / `cm-clay` | 单档 | **`Rounded`** | 10 | `ui.border` / `1.0` | `panel` |

出处：`theme.rs:603-680`（snow 四级）、`:755-836`（ink）、`:840-916`（mist）、`:924-1000`（sand）；
概念图四套由 `ConceptSpec` 统一组装（`theme.rs:1055-1180`，字段注释写「概念盒形状（`cm-paper` / `cm-mint`
椭圆；`cm-slate` / `cm-clay` 圆角矩形）」）。**四套思维导图主题在形状/圆角/描边宽度上完全一致，
差异只在填充、文字与分支色**；概念图四套**形状随主题变**。

两处**派生**要先说清，否则会误判「主题的 stroke 字段没生效」：

1. `resolve_all` 里一级（`depth == 1 && theme.main_card`）的描边是 `level_fill.mix(branch, 0.22)`
   ——从卡片色里自己长出来；二三级是 `level.stroke.mix(branch, 0.20)`（`theme.rs` 里
   `let level_stroke = if depth == 1 && theme.main_card {` 那段）。**所以 `LevelStyle.stroke` 只对根生效。**
2. 概念图非 `cm-*` 主题（即概念图用 `snow` 等）由 `apply_concept_defaults` 强设
   `style.shape = Shape::Ellipse` 且描边改 `theme.ui.border` / 1.0（`theme.rs:1487-1492`，
   `if !own_look && override_…shape.is_none() { style.shape = Shape::Ellipse; }` 那两行）；
   `cm-*` 主题走 `own_look` 分支，形状取主题自己的。

### 2.4 三处是否同一出口

| 出口 | 判定来源 | 现状 |
| --- | --- | --- |
| wgpu 实例 | `instances::corner_style(&st, rect)`（`instances.rs:989-997`，节点在 `:440`、幽灵在 `:497`、悬停/选中环在 `:641-650` 用 `corner_style + gap`） | **同源** |
| SVG | `export_svg.rs:384-385` `fn corner(...) { mw_render::corner_style(style, r) }` | **同源** |
| PDF | `export_pdf.rs:885-886` 同上 | **同源** |
| 命中 | `hit.rs:285-286` 只有二分：`if n.style.shape.is_ellipse() { Shape::Ellipse(rect) } else { Shape::Rect(rect) }` | **部分**：圆角/胶囊仍按矩形命中（见 G-8） |
| egui 编辑环 | `canvas.rs:1964-1967` 已改走 `mw_render::corner_style`（SE-13 复审修） | **同源** |

`corner_style` 本身的规则（`instances.rs:989-997`）：

```rust
pub fn corner_style(style: &mw_core::ResolvedStyle, r: Rect) -> f32 {
    match style.shape {
        Shape::Rounded | Shape::Rect => style
            .radius
            .map(|v| v.clamp(0.0, 24.0))
            .unwrap_or_else(|| corner(style.shape, r)),
        _ => corner(style.shape, r),
    }
}
```

### 2.5 阴影：画布节点**完全没有**，浮层有四档令牌但走 egui

- `grep -rn "shadow\|Shadow" crates/mw-render/src` → **0 命中**（`mb` 无阴影层、无阴影 pass、无 SDF 外扩）；
- 画布节点只有「填充 + 描边」两层，叠在 `L_LINES`（分支线）之上、`L_OVERLAY`（选中环/角标）之下，
  层序常量在 `instances.rs:18-21`（`L_BACKDROP / L_LINES / L_NODES / L_OVERLAY`），装配入口 `build_layers`（`:272`）；
- 现有阴影全部是 **egui 浮层**：`crates/mw-shell/src/canvas_chrome.rs:17` `fn float_shadow()`（悬浮工具条）、
  `chrome.rs:247`（窗口/弹出层）、`dialog.rs:210`（对话框读 `design::elevation::DIALOG`）；
- 令牌本身有四档：`design.rs:765-800` `elevation::{PANEL, WINDOW, POPOVER, DIALOG}`；
- 合同三处口径一致，都是「**节点不投影**」：
  `docs/06` §2.2 第 64 行「阴影 | 只用于浮层（悬浮工具栏、小地图、菜单、对话框）：`0 4 16 rgba(0,0,0,.10)` |
  **节点本身不投影**」；`docs/35` §2.0 底线 2「层级靠留白 + 发丝边，阴影只留给浮层（`elevation` 四档），
  不做「每张卡都投影」」；`docs/28` §4.4 第 237-238 行同上（内容表面口径）。

## 3. 差距清单（现状 → 合同要求 → 影响面 → 要不要动数据模型）

| # | 差距 | 现状 | 合同要求 | 影响面 | 动数据模型？ | 状态 |
| --- | --- | --- | --- | --- | --- | --- |
| **G-1** | **画布节点无阴影**，与 `docs/24` 的 X23 标题相撞 | 渲染层 0 处阴影（§2.5） | `docs/24` §阶段 C 第 98 行「X23 重做节点卡片、主题形状、边框、**圆角和阴影**」；但 `docs/06` §2.2 / `docs/35` §2.0 / `docs/28` §4.4 都写「节点不投影」 | 若做：wgpu 新层或外扩模糊（SDF 需 blit）+ SVG `<filter>` + PDF 无原生模糊要叠多层；**所有含画布的基线全部漂移**（200+ 场） | **可能要**：`ThemeUi.elevation` 或 `LevelStyle.elevation`（新增字段）→ **需用户拍板** | **延后（待拍板）** |
| **G-2** | **形状只有 5 个**，与「覆盖常用形状」有距离 | §2.1 的 5 个变体；XMind 常用还有平行四边形 / 六边形 / 云 / 大括号 / 尖角 | `docs/24` 第 98 行「覆盖常用形状」；`docs/30` §4.2 只定义了四段（240 面板放不下第 5 格，这是排版约束不是形状结论） | 每个新形状 = `Shape` 变体 + `from_name`/`as_str` + WGSL SDF 分支（或三角化）+ 命中几何 + SVG 路径 + PDF 路径 + 检查器可达性（四段放不下 → 需下拉/第二排） | **不需加字段**（`shape` 仍是字符串，未知值回退 `rounded`），但要改 `docs/30` §4.2 与检查器布局 → **需用户拍板做哪几个** | **未开始（待拍板）** |
| **G-3** | **思维导图节点的椭圆/胶囊没有盒外扩** | 概念图椭圆盒按 1.22×1.16 外扩（`theme.rs:1487` + `docs/30` §5.3 第 306 行）；思维导图节点选椭圆时**文字盒不变**、四角可能压弧线（SE-13 `report.md` §7 已如实记账） | 「形状换了，文字不能被形状吃掉」——`docs/30` §5.3 对概念图已立此口径 | `scene.rs` 节点几何 + 布局宽度（盒变宽 = 兄弟节点移位）；会改所有含椭圆节点的基线 | 不动 | **未开始** |
| **G-4** | **圆角上限与盒高耦合，stepper 会饱和** | chk：`radius` 钳 0–24（`theme.rs:1412`），WGSL 再钳到 `min(half.x, half.y)`（`rect.wgsl:120` `let r = min(max(in.radius, 0.0), min(in.half.x, in.half.y));`）；节点高 36 → 视觉上限 18，**24 与 18 同形** | 检查器「圆角」0–24 步 2（`docs/30` §4.2）没写上限耦合 | 用户拖到 24 看不到变化＝控件说谎；验收也无法用「像素差」证明 24 与 18 有区别 | 不动（改显示/文案或按盒高动态上限） | **未开始** |
| **G-5** | **边框色不是主题字段，而是派生** | 根：`TRANSPARENT`/0；一级：`level_fill.mix(branch, 0.22)`；二三级：`level.stroke.mix(branch, 0.20)`（§2.3 说明 1） | `docs/24` X23「边框」成体系；`docs/06` §2.2 只给「1px 边框 + 低频阴影」（顶栏） | 想「统一改边框色」只能改填充/分支色；四主题一致性判据必须写成「派生后同规则」，不能逐值比对 | 不动 | **未开始（口径先行）** |
| **G-6** | **令牌三套语言并存** | 圆角七档 4/6/8/10/12/14/18（`design.rs:706-718`）；描边五值 1.0/1.1/1.2/1.5/2.0；画布节点只用 `radius::MD=10` 与 `stroke_width` 0/1（以及覆盖 0–24、0–8） | `docs/35` §1.3 G1–G4 已把这类冲突列成审计项；X23 要给「节点卡片」定档 | X23 若不定义「节点卡片档」，后续 X25/X27 会继续各写各的 | 不动（只定口径） | **未开始** |
| **G-7** | **阴影/形状若有新档，导出无通路** | 现在 SVG/PDF 只有矩形/椭圆/线三种图元；`corner_style` 同源已成立 | 「导出与画布同源」（AGENTS §6 不变量） | 一旦 G-1 拍板做阴影，SVG 要 `<filter>`/渐变近似、PDF 要叠多层模拟，且三者像素不可能完全一致 → 验收要写容差 | 视 G-1 | **延后（依赖 G-1）** |
| **G-8** | **命中判定与形状不完全一致** | `hit.rs:285-286` 只有椭圆/矩形二分；`radius=24` 的圆角外区域、胶囊外区域仍命中 | 命中应与「看得见的形状」一致（`docs/06` §2.1「选中 外扩 2px 蓝色圆角框」隐含） | 圆角/胶囊边缘点击会「点到空白却选中」；改成内点判定是纯 CPU 小改 | 不动 | **未开始** |
| **G-9** | **概念图的形状由主题决定** | `cm-paper`/`cm-mint` = `Ellipse`，`cm-slate`/`cm-clay` = `Rounded`（`theme.rs:1118-1180`），节点显式 `shape` 覆盖优先 | `docs/24` X23「主题形状」——但没写「换主题=换形状」是否合法；`docs/30` §5.3 把椭圆写成「概念分布的视觉语言」 | 同一份概念图换主题，形状变了但文档没变；四主题一致性验收必须区分「主题默认 vs 文档显式」 | 不动（要写清口径） | **未开始（口径先行）** |

**需用户拍板的两条**（其余是内部收口，可直接做）：**G-1（节点要不要阴影）**、**G-2（加哪几个形状）**；
G-2 虽不加字段，但会改 `docs/30` §4.2 的控件布局，属合同变更，同样要拍板。
**不动数据模型的结论**：除 G-1 若要做阴影需新增 elevation 字段外，本节其余条目都可以在现有
`StyleOverride`（`radius` / `shape` / `stroke_width`）与 `Theme` 令牌里收口。

## 4. 验收场景草案（可量化，全部只写口径，本轮不执行）

| # | 判据 | 量化方法 | 现状能否直接验 |
| --- | --- | --- | --- |
| A-1 | **圆角像素**：`radius=k` 的节点，四角圆弧半径 = `min(k, min(w,h)/2) × zoom` | 100% 与 200% 各取一张；沿 `x + r` 处扫描首行填充像素；直角（`radius=0`）要求`(x+1, y+1)` 落在填充内 | 能（`inspector-radius` 已有 24 档） |
| A-2 | **椭圆长短轴**：概念盒 `rx/ry` 与 PNG 实测轴长差 ≤ 1px | 测 `scene` 的椭圆盒 bbox 与 PNG 边缘；三处取值 `wgsl sd_ellipse` / SVG `<ellipse rx ry>` / PDF 贝塞尔 bbox 必须同值 | 能（`inspector-ellipse` + `cm-*` 基线） |
| A-3 | **边框宽度屏幕恒定**：`stroke_width=1` 在 50/100/200% 下屏幕像素宽度相同（±1px） | 每档缩放截同尺寸窗口，量描边带像素宽 | 能（`zoom` 系列基线可加） |
| A-4 | **零投影（现口径）**：节点盒外 8px 环带与画布底色的像素差 ≤ 1 | 取节点 bbox 外扩 8px 减去 bbox 的环带，比较底色 | 能（任何含画布基线） |
| A-5 | **四主题一致性**：同一文档四主题下，圆角像素半径、描边像素宽、零投影环带**逐项相同**；只允许填充/文字/分支色不同 | 四张基线同文档、同机位；先比形状区域掩码，再比颜色 | 能（`-ink`/`-mist`/`-sand` 已是同机位） |
| A-6 | **往返不变**：`.zmind` 读→改 `shape`/`radius`/`strokeWidth`→存→读，三键保持；未知 `shape` 回退 `rounded` | `mw-format` 单测（无 UI） | 能（SE-13 已有同类用例） |
| A-7 | **命中一致（G-8 若做）**：`radius=24` 圆角对角线外 3px 处点击**不**选中；盒中心点击选中 | `MW_GOLDEN_INPUT` 点击脚本文本 + `MW_TRACE` 选中轨迹 | 现不满足（会选中） |
| A-8 | **钳制可见（G-4 若做）**：`radius=24` 与 `radius=18` 在高 36 的盒上**像素相同**（作为现状证据），改完后 stepper 显示上限 = `min(24, h/2)` | 两张图逐像素比 = 0 说明饱和；改完比较 stepper 文案 | 能（证饱和） |

## 5. 预计 golden 场景名单（只列名字 + 钉住什么，**本轮不拍图**）

| 场景 | 钉住什么 |
| --- | --- |
| `x23-shape-catalog` | 五种形状同屏各一颗：形状 → 半径/路径的唯一映射（含 U 开关的下划线） |
| `x23-radius-range` | 同一节点 `radius=0/6/10/18/24` 五档并排：钳制、四角像素、stepper 显示 |
| `x23-stroke-width` | `stroke_width=0/1/2/4`：屏幕恒定、描边压在填充上（与 WGSL `sc + fc*(1-sc)` 同序） |
| `x23-no-shadow`（或拍板后的 `x23-elevation`） | 现口径：节点外环带零投影；拍板做阴影后改为钉 `elevation` 档的 offset/blur/alpha |
| `x23-themes` | `snow`/`ink`/`mist`/`sand` 同文档同机位：形状与描边几何一致，只颜色变 |
| `x23-ellipse-mindmap` | 思维导图节点椭圆 + 双行文字：钉 G-3 的现状（文字盒不变、可能压弧）与修好后的外扩 |
| `x23-export-parity` | 同场景 PNG/SVG/PDF 三份导出：钉 `corner_style` 三处同源（SVG 读 `<rect rx>`/`<ellipse>`，PDF 读路径 bbox） |
| `cm-shape-themes` | 概念图四套 `cm-*`：钉「形状随主题」与「显式 `shape` 覆盖优先」 |
| `inspector-radius-clamp` | 检查器圆角行在 h=36 盒上的上限显示（G-4 口径落地后） |

> 落盘时每场都要双跑 `--tol 0`；含阴影（若做）的场次需要额外容差口径，届时在
> `docs/29` §5 的「碰外观怎么重拍」里补一段，不要临时放宽 `--tol`。

## 6. 未做 / 风险

- **本轮零代码、零基线**：所有结论来自 `grep`/`read`（`crates/` 正被并行会话改动，故只读测绘）。
- **G-1 / G-2 未拍板**：X23 若按 `docs/24` 字面做「阴影」，会与 `docs/06` §2.2 / `docs/35` §2.0 的
  「节点不投影」直接冲突；不拍板就动手＝自己改合同。建议先由用户在这两条上给一句口径。
- **G-3 / G-9 会改几何**：任何「换形状改盒宽」的改法都会让大量含画布的基线漂移，
  必须单独立包并写清重拍清单，不要混在 X23 的其余条目里。
- **G-8 命中改动廉价但容易漏**：`hit.rs` 的二分还有 `mw-ui` 侧的手势与框选路径，
  改内点判定时要一起看（否则框选能框到圆角外的空白）。
- **欠账照旧**：`已验证` 仍欠实机微软拼音 + 系统 DPI；与本轮无关，但任何「形状像素过门」的判词
  都不能替代实机目检。
