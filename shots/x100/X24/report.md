# X24 · 分支线 / 锥形宽度 / 曲线控制点 / 箭头 / 间距 / 颜色继承（现状审计与合同）

> **本包只做文档与审计，不改 `crates/`**：仓库里有并行会话在改 `crates/`，本包写作用域严格限制在
> `shots/x100/X24/`。审计方法 = 只读 `grep`/`read` + 逐条引用**代码里的字面**（行号是快照，
> 行号漂了按引号里那句话找）。
> 状态：**进行中**（§2 现状测绘与 §3 差距清单已落；实现与基线全部未开始）。
> 路线出处：[`docs/24`](../../../docs/24-X100升级路线.md) §阶段 C 的 X24 行——
> 「X24 重做分支线、锥形宽度、曲线控制点、箭头、间距和颜色继承。」
> 时间：2026-11-02 20:2x。

## 0. 一页结论

连接线这一块**不是从零开始**：锥形宽度、颜色继承、关系线箭头/线型/线宽在**画布**上已经落地并有单测，
真正的问题集中在**三处**：

1. **导出不同源（最严重）**：思维导图分支线在 SVG / PDF 里是**重新算一遍**的，而且算的时候丢了三样东西——
   线型（`elbow`/`straight` 一律画成曲线）、锥形（只写一个 `width`，端点细不了）、箭头（SVG 一条都不画；
   PDF 画的是另一套 9×6.5 世界单位的三角形，不是画布那枚 7 css px 的屏幕恒定箭头）。
2. **合同里的线宽档位比代码粗**：`docs/06` §2.2 写「根 4 / 一级 3 / 二级 2 / 更深 1.5」，
   代码实际是 3.5 / 2.625 / 1.75 / 1.3125（snow）。差 ~12.5%，四个主题各差各的。
3. **几何没有进 `Scene`**：`SceneEdge` 只有 `start/end/orientation/shape`，没有 `path`。画布用
   `edge_path_with(…, e.shape)` 现算，导出用 `edge_path(…)` 现算（且丢 `shape`）——
   「`Scene` 是几何唯一真相源」这条不变量在**树边**上是破的（关系线没破，它存 `r.path`）。

另有四条**合同缺口**（不是代码 bug）：起点/终点内缩没有合同值、兄弟/层间距没有合同值、
四主题色板与 `docs/06` §2.2 那张八色表逐值不同、逐分支的线型/箭头/锥形覆盖属 `docs/27` 的 **X45**（要新字段，**需拍板**）。

**触发「需拍板字段」：是，1 项**（G-11 逐分支覆盖；提案格式按 `docs/32` §12）。

## 1. 范围与非目标

### 1.1 范围（X24 的六个词，逐词对应）

| X24 的词 | 本包要回答的问题 |
| --- | --- |
| 分支线 | 线型有几种、谁决定、画布/导出是否同一套；端点内缩多少 |
| 锥形宽度 | 逐层宽度多少、怎么插值、导出有没有 |
| 曲线控制点 | S 形三次曲线的控制点公式、算在哪一层、画布与导出是否同一份 |
| 箭头 | 存在哪几种、尺寸量法（屏幕恒定还是世界固定）、四档端点语义、三处导出是否一致 |
| 间距 | 兄弟间距、层间距的现值、可调范围、有没有合同值 |
| 颜色继承 | 分支色怎么来、继承边界在哪、显式覆盖优先级、四主题取值、选中/悬停改不改色 |

### 1.2 非目标

| 不做 | 原因 |
| --- | --- |
| 改任何 `crates/` 代码 | 本包硬约束；实现另开一轮 |
| 改 `docs/`（含 `docs/STATUS.md`） | 写作用域只有 `shots/x100/X24/`；合同缺口只在本文件登记 |
| 重拍任何 golden 基线 | 本包不产生外观改动 |
| 逐分支覆盖（线型/箭头/锥形 per node） | 属 `docs/27` §3 的 **X45**，且要新字段 → 本文件只登记（G-11） |
| 层级样式默认值重校准（字号/填充/卡片） | 属 **X25**；X24 只管线的宽度与颜色 |
| 概念图关系线的新能力（框选命中、Shift 范围选择） | `docs/30` §5.4.4 已写明「未做」，不属本包 |

## 2. 现状测绘（能力 × 现状 × 出处）

出处写法：`文件` + 那一行的字面。**行号是快照**，引用一并给字面，漂了也能找。

### 2.0 先定口径：这几篇文档各自管什么

| 文档 | 关于「连线/分支线」的原文 | 对本包的作用 |
| --- | --- | --- |
| [`docs/24`](../../../docs/24-X100升级路线.md) §阶段 C | 「X24 重做分支线、锥形宽度、曲线控制点、箭头、间距和颜色继承。」 | **任务来源**（就是这六个词） |
| [`docs/06`](../../../docs/06-界面布局与交互设计.md) §2.1 实拍结论 | 「连接线 \| 按分支取色、约 2px、**S 形三次曲线**、端头干净 \| 细线 + 彩色描边盒，线被盒子抢掉」 | 画布分支线的**形态基准**（有实拍背书） |
| `docs/06` §2.2 令牌表 | 「分支线 \| **根 4 / 一级 3 / 二级 2 / 更深 1.5**，逐边由父端向子端收细 \| `Theme::edge_widths` + `SceneEdge.width_end`，按弧长插值（XMind `line-tapered: tapered`）」 | 线宽的**合同值**（G-08 就是拿它和代码对） |
| `docs/06` §2.2 令牌表 | 「分支色（线） \| 青 `#1F9E86`、珊瑚 `#E4635B`…」八色 +「一级填充」八色 | 色板合同值（G-12 与代码对不上） |
| `docs/06` §2.2「背景纹理」 | 「导出默认剔除」 | **已被 `docs/30` §5.8 取代**；导出必须同源，不剔除（X21b 已按此改） |
| [`docs/28`](../../../docs/28-内容体验重构规范.md) §5 大纲 | 「行只承载文字；层级靠缩进和留白，**不靠竖线、连接线、色块**。」「**禁止树形竖线、连接线、每层缩进引导线。**」 | **只管大纲视图**。X24 的连线是**画布**上的，`docs/28` 没有画布连线条款——别把这两句当「不许画分支线」的依据 |
| [`docs/30`](../../../docs/30-外壳体验与概念图设计规范.md) §5.4 | 「线 \| 直线段，线宽 1.5（屏幕空间恒定）」「箭头 \| 端点实心三角形，**按屏幕空间恒定画**：尖端锚在连线终点、指向不动，长度不少于 7 css px，只放大不缩小」 | **概念图**关系线合同（画布已达标，见 §2.3） |
| `docs/30` §5.4.1 | 跨分支取色、「每条线最多让一次…偏 `6px`（上限 8px）」「短语胶囊跟着曲线上 `t = 0.5` 的点走」 | 概念图取色/微弧合同（已达标） |
| `docs/30` §5.4.3 | 线型/线宽/注记三键 + 「虚线节奏 **6 实 4 空，世界单位**（`mw_core::scene::REL_DASH`）」+「PNG / SVG / PDF 三处同源」 | CM-16 合同（已达标；同源的正例） |
| `docs/30` §5.8 | 「**`Scene` 是几何唯一真相源：PNG / SVG / PDF 导出自动可用，无需新代码路径。**」 | 导出同源的**总合同**；G-01–G-07 全是违反它 |
| `docs/30` §5.9 | 「**禁止曲线/贝塞尔连接线（先做直线）**」 | 概念图线形合同（已达标） |
| [`docs/32`](../../../docs/32-概念图数据结构提案.md) §3 | 「`ArrowHead { Off, Forward, Backward, Both }`」「几何只在 `Scene` 算一次…wgpu 渲染、egui 覆盖层、SVG、PDF 四处只读这一份几何」 | 箭头字段的已批准决定；**「几何只在 Scene 算一次」在树边上不成立**（G-07） |
| `docs/32` §12 | 字段提案格式：`为什么不能复用现有字段` / `兼容` / `代价` / `回滚` / `验收` / 依据 | G-11 的提案按此格式写（§3.2） |
| [`docs/27`](../../../docs/27-X100数据与命令审计.md) §3 | 「X45 \| 分支线型/箭头/锥形覆盖 \| 节点覆盖、主题继承、导出一致」 | 逐分支覆盖**属于 X45**，不是 X24；本包只登记（G-11） |

> 冲突处理：`docs/28` / `docs/30` 优先于 `docs/06`（`AGENTS` §4「`docs/06` 只作外壳/令牌参考」）。
> 本包只发现**一处**两者冲突：`docs/06` §2.2「导出默认剔除」（已被 `docs/30` §5.8 取代，X21b 已按同源改）。

### 2.1 思维导图分支线

| 能力 | 现状 | 出处 |
| --- | --- | --- |
| 线型（曲线/直角/直线） | **有**，文档级 `edge_shape`：`curve`（默认）/ `elbow` / `straight` | `crates/mw-core/src/model.rs`：`pub edge_shape: String,`（注释「分支线形态：curve / elbow / straight」）、`Default` 里 `edge_shape: "curve".to_string(),` |
| 线型 → 几何 | **三条形态都在 `mw-core` 的同一个函数里**：`EdgeShape::{Curve,Elbow,Straight}`，`pub fn edge_path_with(e: &Edge, shape: EdgeShape) -> Path` | `crates/mw-core/src/layout.rs`：`EdgeShape::Curve => match e.orientation {`、`EdgeShape::Elbow =>`、`EdgeShape::Straight => p.line(e.end),` |
| 曲线控制点公式 | 一次三次贝塞尔，控制点在**两端水平/垂直中点**：水平边 `dx = (e.end.x - e.start.x) / 2.0`，`p.cubic(Point::new(e.start.x + dx, e.start.y), Point::new(e.end.x - dx, e.end.y), e.end)`；垂直边同理用 `dy` | `crates/mw-core/src/layout.rs`，`pub fn edge_path_with(...)` 里 `EdgeShape::Curve` 分支 |
| 控制点算在哪一层 | 算在**消费方**：画布 `edge_path_with(…, e.shape)`；SVG/PDF `edge_path(&Edge { … })`（默认 `Curve`） | 画布 `crates/mw-render/src/instances.rs`：`let path = edge_path_with(` + `e.shape,`；SVG `crates/mw-shell/src/export_svg.rs`：`let path = edge_path(&Edge {`；PDF `crates/mw-shell/src/export_pdf.rs`：`let path = edge_path(&Edge {` |
| 几何是否进 `Scene` | **没有**：`SceneEdge` 只有端点 + 形态枚举，没有路径 | `crates/mw-core/src/scene.rs`：`pub struct SceneEdge { pub from: u32, pub to: u32, pub start: Point, pub end: Point, pub orientation: Orientation, … pub shape: EdgeShape, … }`（对比 `SceneRelationship` 有 `pub path: Path`） |
| 锥形宽度 | **画布有**：子节点深度决定 (父端宽, 子端宽)，按**弧长**线性插值 | `crates/mw-core/src/theme.rs`：`pub fn edge_widths(&self, child_depth: u32) -> (f32, f32)`；`fn edge_width_at`：`0 => self.edge_width_root, 1 => self.edge_width_root * 0.75, 2 => self.edge_width, _ => (self.edge_width * 0.75).max(1.0)`；插值 `crates/mw-render/src/instances.rs`：`let t = (0.5 * (cum[i] + cum[i + 1]) / total).clamp(0.0, 1.0);` + `w0 + (w1 - w0) * t` |
| 逐层实际宽度（世界单位/snow） | 3.5 → 2.625 → 1.75 → 1.3125 | `edge_width_root: 3.5` + `edge_width: 1.75`（`theme.rs` 的 `snow()`） |
| 逐层实际宽度（其余主题） | ink 3.2 / 1.6；mist 3.3 / 1.65；sand 3.4 / 1.7（规则同上） | `theme.rs`：`taper: true, edge_width: 1.6, edge_width_root: 3.2,`（ink）等同款三处 |
| 收细开关 | 每套主题一个 `taper: bool`；四套内置全部 `true`；概念图主题 `taper: false` | `theme.rs`：`pub taper: bool,` + 注释「XMind 的 `line-tapered: tapered`；M14 这类主题不写该属性，线是等粗的」；`ConceptSpec::theme` 里 `taper: false,` |
| 起点/终点内缩 | 起点离父卡 0.5、终点离子卡 1.5（**世界单位**）；组织结构图布局直接贴边（`p.bottom()` / `c.y`）；鱼骨/时间轴另有 `AXIS_GAP = 34.0` | `crates/mw-core/src/layout.rs`：`pub const EDGE_GAP_START: f32 = 0.5;`、`pub const EDGE_GAP_END: f32 = 1.5;`、`start: Point::new(p.right() + EDGE_GAP_START, p.y + p.h / 2.0),`、`end: Point::new(c.x - EDGE_GAP_END, c.y + c.h / 2.0),` |
| 分支线箭头 | **文档级开关** `edge_arrow: bool`，默认 `false`（= 「端头干净」，与 `docs/06` §2.1 实拍一致）；画布箭头长度 `cam.css_to_world(7.0)`、半宽 `size * 0.42`（屏幕恒定） | `model.rs`：`pub edge_arrow: bool,` + `edge_arrow: false,`；`scene.rs`：`arrow: settings.edge_arrow,`；`instances.rs`：`let size = cam.css_to_world(7.0);`、`let half = size * 0.42;` |
| 箭头入口 | 检查器「布局」组里一个开关（`edge_arrow`），没有逐分支覆盖 | `crates/mw-shell/src/panels/inspector/inspector_methods_2.rs`：`let mut arrow = self.layout_draft.edge_arrow;` |
| 兄弟/层间距 | `sibling_gap = 24.0`、`level_gap = 80.0`（世界单位），下限 clamp 4 / 20；检查器「间距」滑杆可调 | `model.rs`：`pub sibling_gap: f32,` `pub level_gap: f32,` + `Default` 里 `sibling_gap: 24.0, level_gap: 80.0,`；`crates/mw-core/src/layout.rs`：`let gap = settings.sibling_gap.max(4.0);`、`let level_gap = settings.level_gap.max(20.0);` |
| 分支线可选中/悬停 | **不可**：命中测试不产出「边」这个目标（只有节点/边界/概要/关系线），所以思维导图分支线没有悬停反馈，也没有选中态 | `crates/mw-render/src/hit.rs`：`pub fn hit(&self, cam: Camera, p: Point) -> Option<Target>`（`Target` 无 edge 变体；全文只有 `brace_and_boundary_edges_are_hit` 这一条与 edge 有关的测试名） |

### 2.2 颜色继承

| 能力 | 现状 | 出处 |
| --- | --- | --- |
| 分支色来源 | 根 = `theme.ui.accent`；一级 = `palette_at(兄弟序号)`；子孙 = **沿父链继承**；显式 `branch_color` 覆盖以上全部 | `crates/mw-core/src/theme.rs`：`if id == doc.root_id { branch = theme.ui.accent; } else if let Some(c) = own_branch { branch = c; } else if depth == 1 { if let Some(c) = theme.palette_at(sibling_idx) { branch = c; } }` |
| 连线取色 | 分支线颜色 = **子节点**的 `branch_color`（所以一条分支从根到末梢同色） | `crates/mw-core/src/scene.rs`：`color: child.style.branch_color,` |
| 覆盖键 | `StyleOverride.branch_color: Option<String>`（检查器「分支线」那行写它） | `crates/mw-core/src/model.rs`：`pub branch_color: Option<String>,` |
| 二级/三级卡片淡染 | 深层卡片是该层填充/描边按 0.07 / 0.20 混入分支色 | `theme.rs` 测试：`assert_eq!(s.fill, theme.levels[2].fill.mix(s.branch_color, 0.07));`、`assert_eq!(s.stroke, theme.levels[2].stroke.mix(s.branch_color, 0.20));` |
| 四主题色板 | snow **8 色**（`#3a7d6c` 起）、ink 6 色（`#7fb69a` 起）、mist 6 色（`#3e7f76` 起）、sand 6 色（`#a45c43` 起） | `theme.rs` 的 `palette: vec![`，四个 `_()` 函数里各一处 |
| 概念图关系线取色 | 显式 `color` > 跨分支取**来源分支**色 `palette_at(branch)`（按色板长度取模）> 默认 `ui.dim` | `scene.rs`：`fn concept_rel_color(...)` 里 `(Some(a), Some(b)) if a != b => theme.palette_at(a as usize).unwrap_or(theme.ui.dim),`；调用点 `Rgba::parse(...).unwrap_or_else(|| concept_rel_color(&branches, &ids, from, to, theme))` |
| 选中/悬停改色 | 关系线：选中 3px 强调描边（画在壳层覆盖层，`docs/30` §5.4.4）；分支线：**不改**（不能被选中） | `docs/30` §5.4.4「每条选中的线都画 3px 实色强调」；金样 `cm-parallel-select-a` / `cm-parallel-select-b` |
| 对比度硬门 | **已有**：四主题 × 所有 `palette` 色，对画布底与一级卡片底都 ≥ 3.0（WCAG 非文本） | `crates/mw-core/tests/contrast.rs`：`fn every_theme_keeps_branch_colours_visible_on_its_canvas()`，断言文案「分支线在底色上不够清楚（WCAG 非文本对比度 3:1）」 |

### 2.3 概念图关系线（`docs/30` §5 的落地情况）

| 能力 | 现状 | 出处 |
| --- | --- | --- |
| 线型/线宽/命题注释（CM-16） | **已实现**：`Relationship.dashed` / `Relationship.width` / `Relationship.note` 三个可选键；画布、SVG、PDF 三处同源 | `docs/30` §5.4.3；`scene.rs`：`dashed: r.dashed,` `width: r.width_css(),`；`model.rs`：`pub fn width_css(&self) -> f32 { self.width.unwrap_or(1.5) }`；`scene.rs`：`pub const REL_DASH: [f32; 2] = [6.0, 4.0];` |
| 箭头四档（CM-10 ⑦ / `docs/32` §3） | **已实现**，默认 `Forward`；箭头三角形存**世界坐标** `SceneRelationship.arrow/arrow_back`，渲染层按 `arrow_at_zoom` 撑到 ≥7 css | `model.rs`：`pub head: ArrowHead,`；`scene.rs`：`pub arrow: Option<[Point; 3]>,` `pub arrow_back: Option<[Point; 3]>,`；`crates/mw-core/src/geom.rs`：`pub const ARROW_MIN_CSS_LEN: f32 = 7.0;` + `let k = (ARROW_MIN_CSS_LEN / z / len).max(1.0);` |
| 箭头导出同源 | **是**：SVG/PDF 都调 `mw_core::geom::arrow_at_zoom(tri, 1.0)`，与画布同一函数 | `export_svg.rs`：`let tri = mw_core::geom::arrow_at_zoom(tri, 1.0);`；`export_pdf.rs`：同款一行；两侧都有 `exported_arrow_matches_arrow_at_zoom_one` 单测 |
| 线形 | **直线段**（`straight_between`），符合合同禁止项 `docs/30` §5.9「**禁止曲线/贝塞尔连接线（先做直线）**」；同一对概念多条命题走车道微拱（`bowed_straight`，`CONCEPT_LANE_GAP = 24.0`）；交叉时后声明者微弯 `min(6, len/12)` | `scene.rs`：`fn straight_between(p1: Point, p2: Point) -> (Path, [Point; 3], Point)`、`const CONCEPT_LANE_GAP: f32 = 24.0;`、`fn bowed_straight(...)`、`fn bow_crossings(rels: &mut [SceneRelationship])` |
| 平行命题车道（C14 / C15 / C21） | **已实现**：无向对分组、≤2 条走旧公式、≥3 条按实测短语宽自适应净空 | `crates/mw-core/src/concept_layout.rs` 测试：`three_equal_lanes_get_real_clearance_and_stay_symmetric`、`five_lanes_with_mixed_widths_never_crowd`；`scene.rs` 注释「C21：按无向对算一次短语切线偏移」 |
| 跨分支取色/交叉微弧（CM-09） | **已实现**（见 §2.2 最后两行） | 同上 |
| 命题注释（注记） | 已实现但**画布不画角标**（刻意取舍） | `docs/30` §5.4.3「注释的画布表现：**不画角标**」 |

### 2.4 导出同源总账（只看「连线」这一块）

| 消费方 | 数据源 | 同源？ |
| --- | --- | --- |
| 画布（wgpu） | `SceneEdge.start/end/orientation/shape` → `edge_path_with` → `push_curve(width, width_end)`；箭头 `cam.css_to_world(7.0)` | 基准 |
| SVG | `edge_path(&Edge{ start, end, orientation })`（**丢 `shape`**）→ 单条 `<path>` + `stroke-width = css_to_world(e.width)`（**丢 `width_end`**）；**不画树边箭头** | ✗ 三项不同源 |
| PDF | 同上 `edge_path(...)`（**丢 `shape`**）→ `stroke_path(..., e.width, ...)`（**丢 `width_end`**）；箭头 `arrow_head` 用 `const AL: f32 = 9.0; const AW: f32 = 6.5;`（**世界单位，第二套几何**） | ✗ 两项不同源 + 箭头几何分叉 |
| 概念图关系线（SVG/PDF） | `SceneRelationship.path` / `.color` / `.width` / `.dashed` / `.arrow`，箭头 `arrow_at_zoom(tri, 1.0)` | ✓ 完全同源 |
| 概要括号 / 边界 / 轴线 | `Scene.summaries[].brace/connector`、`Scene.spines[].path`（都存了 `Path`） | ✓ |

## 3. 差距清单

格式：**现状 → 合同要求 → 影响面 → 是否要动数据模型**。合同条款出处写在「合同要求」里。

| # | 差距 | 现状 → 合同要求 | 影响面 | 动数据模型？ |
| --- | --- | --- | --- | --- |
| **G-01** | SVG 丢掉树边线型 | SVG 用 `edge_path`（默认 `Curve`）→ 合同：`docs/30` §5.8「**`Scene` 是几何唯一真相源：PNG / SVG / PDF 导出自动可用，无需新代码路径**」+ `docs/24` X24 的「分支线」。要求：SVG 必须按 `SceneEdge.shape` 画（`elbow` 要有两段折线） | 所有设成直角/直线的文档；`--tol 0` 会与画布不一致，但**没有基线覆盖**（现有场景全是 `curve`） | 否（`SceneEdge.shape` 已在） |
| **G-02** | SVG 丢掉锥形 | 单条 `stroke-width = e.width` → 合同：`docs/06` §2.2「**逐边由父端向子端收细**」（`Theme::edge_widths` + `SceneEdge.width_end`）+ `docs/30` §5.8 同一份 `Scene`。要求：导出要么多条折线段各自宽度，要么写死「导出不做锥形」并把这个取舍写进合同 | 每条分支线；SVG 现在**处处偏粗一个档**（子端应是 1.75，实际整条 3.5） | 否（`width_end` 已在） |
| **G-03** | PDF 丢掉树边线型 | 同 G-01，PDF 也是 `edge_path` 默认曲线 | 同 G-01 | 否 |
| **G-04** | PDF 丢掉锥形 | 同 G-02，`stroke_path(..., e.width, ...)` 单值 | 同 G-02 | 否 |
| **G-05** | SVG 完全不画树边箭头 | `settings.edge_arrow = true` 时画布与 PDF 都有箭头，SVG 没有 → 合同：同源 | 打开「分支线箭头」的文档，SVG 缺箭头 | 否 |
| **G-06** | 箭头有两套几何 | 画布：长 7 css（`cam.css_to_world(7.0)`）、半宽 0.42×size；PDF：`AL = 9.0`、`AW = 6.5`（**世界单位**，200% 缩放下只有 4.5 css 长）→ 合同：`docs/30` §5.4「长度不少于 7 css px、屏幕空间恒定、只放大不缩小」；`docs/24` X24 的「箭头」 | 所有带箭头的导出；两条导出路径互不一致，且 PDF 在非 100% 缩放下不恒定 | 否（改成共用 `geom::arrow_at_zoom` 即可，与关系线同款） |
| **G-07** | 树边几何不进 `Scene` | `SceneEdge` 无 `path`；画布/导出各自现算 → 合同：`AGENTS` §6 不变量「`Scene` 是几何与视觉唯一真相源；画布、golden、PNG/SVG/PDF 导出共用同一份 `Scene`」 | 任何控制点公式调整都要改 3 处；G-01/G-03 就是这个结构病的症状 | **两个选项**，见 §3.1（推荐不新增字段：导出必须调 `edge_path_with(…, e.shape)`） |
| **G-08** | 线宽档位低于合同 | `docs/06` §2.2 写「根 4 / 一级 3 / 二级 2 / 更深 1.5」；代码 snow = 3.5 / 2.625 / 1.75 / 1.3125（差 −12.5%），ink/mist/sand 各自又不同 → 要定：按合同抬到 4/3/2/1.5，还是把合同改成现值 | 抬档 = 全仓基线重拍（254 个目录里凡有分支线的都漂）；改合同 = 只改文档（但文档不在本包写作用域） | 否（改常量即可，但属外观改动） |
| **G-09** | 起点/终点内缩没有合同值 | 现值 `EDGE_GAP_START = 0.5` / `EDGE_GAP_END = 1.5`（世界单位）；`docs/06` 没写、`docs/28` 没写 → 合同缺口 | 合同补齐后别人才能判断「线顶到卡片」是不是 bug | 否（只补文档） |
| **G-10** | 兄弟/层间距没有合同值 | 现值 `sibling_gap = 24` / `level_gap = 80`；`docs/06` §2.2 只写「间距滑杆（带数值）」，没有默认值与允许范围 → 合同缺口 | 同上；另外 `docs/06` 的间距滑杆章节还没落（`Ctrl+T` 面板未做） | 否（只补文档） |
| **G-11** | 逐分支覆盖缺字段 | `arrow` / `taper` / 线型都只有**文档级**；`docs/27` §3 的 X45 行要求「节点覆盖、主题继承、导出一致」 → **需用户拍板**才能加字段 | 一个分支局部换线型/关箭头；现在只能改整篇文档 | **是**（`StyleOverride` 或每节点加键），见 §3.2 |
| **G-12** | 四主题色板与合同表逐值不同 | `docs/06` §2.2「分支色（线）」表：青 `#1F9E86`、珊瑚 `#E4635B`… 八色；代码 snow 是 `#3a7d6c`、`#c4644e`…（八个全不同，饱和度更低）与 ink/mist/sand 的 6 色 | 判「颜色对不对」时两份文档会打架；建议以**代码现状**为准（已过 SE-11 对比度硬门、已进 254 张基线），把 `docs/06` 那张表标成旧；但 `docs/` 不在本包写作用域 | 否（只改文档） |

> 计数：**12 条**，其中「必须动数据模型」1 条（G-11，`需拍板`），其余 11 条不动模型。
> 画布侧**已达标**的项（锥形插值、颜色继承、关系线同源、箭头屏幕恒定、虚线节奏）见 §2，不重复列为差距。

### 3.1 G-07 的两个选项（本包建议 = B）

| 选项 | 做法 | 代价 |
| --- | --- | --- |
| A. `SceneEdge` 加 `path: Path` | 与 `SceneRelationship` 同款，导出直接读 | `Scene` 每条边多一份 `Vec<PathSeg>`（20k 节点 ≈ +几 MB），且 `Scene::edges` 的 `PartialEq` 指纹变化；M12/P7 的性能账要重测 |
| **B. 导出改调 `edge_path_with(…, e.shape)`（推荐）** | 不新增字段，只把两个导出器里那一句 `edge_path` 换成 `edge_path_with`，并补一条「导出与画布同函数」的单测 | 仍需调用方记得传 `shape`；靠契约 + 测试钉住（导出侧写 `scene_edge_path(e)` 这个唯一入口） |

### 3.2 G-11 的字段提案（`docs/32` §12 格式）· **需拍板**

```rust
pub struct StyleOverride {
    // …现有字段…
    /// 分支线线型覆盖：curve / elbow / straight；缺键 = 跟随文档 `layout.edge_shape`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_shape: Option<String>,
    /// 分支线箭头覆盖：true 才画；缺键 = 跟随文档 `layout.edge_arrow`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_arrow: Option<bool>,
    /// 分支线是否收细；缺键 = 跟随主题 `taper`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_taper: Option<bool>,
}
```

- **为什么不能复用现有字段**：`layout.edge_shape` / `layout.edge_arrow` 是**文档级**（一张图一套），
  「这个分支用直角、那个分支用曲线」只能落在节点上；`taper` 在**主题**里（第 387 行 `pub taper: bool,`），
  主题是四套内置 + 覆盖，不能装「第 3 个一级分支不收细」。
- **兼容**：三个键都 `Option` + `skip_serializing_if`，缺键 = 老行为，不升 `FORMAT_VERSION`、无迁移。
- **代价**：`SceneEdge.shape/arrow` 的取值从 `settings` 改为「节点解析结果」（`theme::resolve_styles` 一次解析、
  多处消费，与 `branch_color` 同一条路）；**改一处会动全部有分支线的基线**（但默认值不变 ⇒ 零漂移）。
- **回滚**：删三个字段 + 删 `resolve_styles` 三行 + 删 `scene.rs` 三行取值。
- **验收**：单测（缺键默认、覆盖优先、子树继承边界）、新场景 `mm-edge-shape-override` / `mm-edge-arrow-override`。
- **依据**：`docs/27` §3 X45 行「分支线型/箭头/锥形覆盖 | 节点覆盖、主题继承、导出一致」；
  `docs/06` §2.2 分支线行把宽度写成 `Theme::edge_widths` + `SceneEdge.width_end`，覆盖语义未定。
- **不做也可以**：X24 只要求「箭头」这一件在文档级正确；逐分支覆盖可推到 X45。**这就是为什么它需要拍板。**

## 4. 验收场景草案（全部可量化）

> 现状：**未开始**。写成「输入 → 机器判据」，实现轮直接照此写测试。

| ID | 场景 | 判据（数值） |
| --- | --- | --- |
| AC-01 | 控制点公式 | 任取一条水平边 `(100,100)→(300,180)`：`edge_path_with(…, Curve)` 的第 1 个 `Cubic` 段 `c1 = (200,100)`、`c2 = (200,180)`，误差 ≤ `0.01`；垂直边对称 |
| AC-02 | 三处几何同一份 | 对同一 `Scene`：画布（离屏 `Frame` 采样）、SVG `d=`、PDF 内容流里的路径，还原成 12 个等弧长采样点，三点集两两 ≤ `0.01` 世界单位 |
| AC-03 | 线型传导 | `edge_shape = elbow` 时：SVG 的 `d` 含 **2 个 `L`**（直角两段）、PDF 含同名两段、画布像素包围盒与曲线不同（`--tol 0` 会红）；`straight` 时三处都是 1 段 |
| AC-04 | 锥形公式 | depth 1..4 的边：弧长 `t` 处宽度 = `w0 + (w1-w0)t`，误差 ≤ `1e-3` css；四主题逐档 == 合同档位（抬档后 4/3/2/1.5） |
| AC-05 | 锥形进导出 | SVG/PDF 在 `t = 0.05 / 0.5 / 0.95` 三处实测线宽与 AC-04 同值（SVG 读 `stroke-width` 或分段宽度，PDF 读 `w`），误差 ≤ `0.02` css |
| AC-06 | 箭头 ≥7 css 且恒定 | `zoom ∈ {0.25, 0.5, 1, 1.5, 2, 4}`：箭头屏幕长度 ∈ `[7, 7.001]`；四档 `Off/Forward/Backward/Both` 在每个档位三角形存在性正确、面积 > `8 css px²`；`Off` 时三处都没有三角形 |
| AC-07 | 箭头导出一致 | 同一 `Scene` 的箭头三角形三点：画布（`arrow_at_zoom(tri, zoom)` 反算）、SVG `polygon points`、PDF 路径，误差 ≤ `0.01` 世界单位；**PDF 必须不再用 `AL = 9.0 / AW = 6.5`** |
| AC-08 | 颜色继承比例 | 一级分支线颜色 == `palette[i]`（RGB 全等，四主题各 6–8 个分支）；孙代 == 祖先分支色；显式 `branch_color` 覆盖优先；深层卡片填充 == `levels[d].fill.mix(branch, 0.07)`（误差 0） |
| AC-09 | 概念图取色 | 同分支连线 == `ui.dim`；跨分支 == `palette_at(from_branch % palette.len())`（四主题长度 8/6/6/6 各验一次）；显式 `relationship.color` 优先 |
| AC-10 | 分支线对比度（**已有硬门，照抄别手填**） | `crates/mw-core/tests/contrast.rs` 的 `every_theme_keeps_branch_colours_visible_on_its_canvas`：四主题 × 所有 `palette` 色，对画布底与一级卡片底都 ≥ **3.0**（WCAG 非文本）。X24 改色板时必须继续绿 |
| AC-11 | 虚线节奏 | `dashed = true`：SVG `stroke-dasharray="6 4"`、PDF `[6 4] 0 d`、画布实段长 6/4 世界单位（误差 ≤ `0.02`） |
| AC-12 | 导出逐像素 | 同一 `Scene` 导 `png@2x` 与 `svg→栅格化@2x`：连线区域（按 `Scene.edges` 包围盒外扩 4px）逐像素差 ≤ `2/255`，超容差像素数 ≤ 总数 `0.05%`，并打印包围盒 |
| AC-13 | 零漂移（若只改导出） | 9 场代表场景 `--tol 0` 全零差异（画布不动）；`stage-b` / `stage-cm` 登记场次不增不重拍 |
| AC-14 | 四主题一致性 | 同一棵树切四主题：分支线宽度档位、色板长度、箭头尺寸三处逐主题断言；画布与 SVG 对每主题的连线 bbox 相同 |

## 5. 预计 golden 场景名单（只列名字与钉住什么，**本包不拍图**）

| 场景 | 钉住什么 | 状态 |
| --- | --- | --- |
| `theme-ink` | 暗色主题的分支色 + 锥形宽度 + 点阵（X21b 已收口） | 已有 |
| `theme-sand` / `theme-mist` | 亮色主题分支色与逐层宽度 | 已有（`theme-mist` 需确认已进登记表） |
| `mm-collapse` | 曲线分支线 + 三层收细 + 折叠后端点 | 已有 |
| `mm-rel-select-arrow` | 关系线选中态箭头 | 已有 |
| `cm-default` | 概念图直线关系线 + 6×4 箭头 | 已有 |
| `cm-arrow-off` / `cm-arrow-backward` / `cm-arrow-both` | 四档端点语义 | 已有 |
| `cm-rel-curve` | 交叉微弧 / 车道曲线 | 已有 |
| `cm-parallel-3` | 三车道避让 | 已有 |
| `mm-edge-elbow` | **新增**：`edge_shape = elbow` 的直角分支线（画布 + 导出） | 未开始 |
| `mm-edge-straight` | **新增**：`edge_shape = straight` | 未开始 |
| `mm-edge-arrow` | **新增**：`edge_arrow = true` 的箭头（屏幕恒定，ppp 1.0 / 1.5 两档） | 未开始 |
| `mm-edge-taper-ink` | **新增**：暗色主题锥形线在 200% 缩放下仍然收细 | 未开始 |
| `mm-export-parity` | **新增（证据场）**：同一文档同时导出 PNG/SVG/PDF 的三份产物，供 AC-12 逐像素比 | 未开始 |

> 不拍图的原因：本包零外观改动；实现轮落定后再按 `docs/29` §5 重拍并连跑两次 `--tol 0`。

## 6. 未做 / 风险

### 6.1 未做（如实记账）

1. 全部实现（G-01–G-12）**未开始**：本包只交合同与差距清单。
2. 文档侧的合同补齐（G-08 的取值、G-09 内缩、G-10 间距）**未做**，因为 `docs/` 不在写作用域。
3. G-11 的字段提案**未提交**给用户拍板（只在本文件 §3.2 备好）。
4. 实机 150% / 200% DPI 下的锥形与箭头观感**未验**（与全仓同欠）。
5. `docs/06` §2.2 的「间距滑杆」章节与 `Ctrl+T` 主题/布局面板本身**未实现**（属别的包）。

### 6.2 风险

| # | 风险 | 处理 |
| --- | --- | --- |
| R1 | 并行会话正在改 `crates/`，本包引用的行号/字面可能在你读到时已漂 | 每条引用都带**那一行的字面**；实现轮开工前重跑一遍 §2 的 grep |
| R2 | 抬线宽档位（G-08）会漂掉 254 个目录里所有带分支线的基线 | 先定「改代码还是改合同」，再决定是否重拍；别两件事一起做 |
| R3 | 导出改锥形（G-02/G-04）没有现成基线，容易「改了但看不出来」 | 按 §5 先建 `mm-export-parity` 证据场，再动代码 |
| R4 | `SceneEdge.path`（G-07 选项 A）会碰 20k 性能账（M12/P7 已达标的读数） | 默认走选项 B；A 只有在「导出必须与画布逐像素一致」被证伪时才考虑 |
| R5 | G-12 若改成「以合同八色为准」会连锁漂掉全部一级卡片填充（`palette_fill` 与 `palette` 手工成对） | 建议以代码为准、把合同表标旧；本包不执行，只登记 |
| R6 | 箭头恒定量法在 25% 缩放下会让箭头看起来「相对很大」 | 合同 `docs/30` §5.4 已经明确「只放大不缩小」，按合同；不要按观感改 |

## 7. 本轮实现：导出同源（G-01–G-07）

> **授权**：2026-11-02 20:4x Lead 原话「你写的 X24 报告质量够，**接着把其中最要紧的一类真 bug 修掉**——
> 导出不同源（G-01…G-07）。本轮**允许改代码**」。写作用域 = `crates/mw-shell/src/export_svg.rs`、
> `crates/mw-shell/src/export_pdf.rs` 及其单测；**禁止**碰 `mw-core`（`document/scene/search/geom`）、
> `mw-render/src/hit.rs`、`mw-shell/src/canvas/**`、`docs/`、`scripts/`、`docs/STATUS.md`、其它 `shots/x100/*`；
> 不 commit、不 `cargo build --release`、不跑 golden（与并行会话抢锁）。**G-08 / G-11 / G-12 不动**（拍板项）。

### 7.1 改法（每条差距一句话）

| # | 差距 | 修法 |
| --- | --- | --- |
| G-01/G-03 | SVG/PDF 把 elbow/straight 画成曲线 | 两个导出器改调 `edge_path_with(…, e.shape)`，并收进**同一个导出侧入口** `tree_edge_path(e)`（`export_svg.rs` 定义，`export_pdf.rs` 导入）。控制点公式仍只有 `mw_core::layout` 一份，**不新增 `SceneEdge.path` 字段** |
| G-02/G-04 | 导出没有锥形（单条 `stroke-width`/`stroke`） | 改为**按两侧轮廓生成填充多边形**：弧长 → 宽度（与画布 `push_curve` 同一套 `cum/total` 口径）→ 法线偏移半宽 → 左轮廓 + 末端圆头 + 右轮廓反向 + 起端圆头闭合。SVG 写 `<path … Z" fill>`，PDF 写 `m/l/h f`。首末宽差 <1e-4 时退化成等宽带（仍是填充） |
| G-05 | SVG 一条树边箭头都不画 | 补三角形：`tree_edge_arrow(e, world_per_css)`，与画布 `instances.rs` 里那段**逐字同款**（`size = world_per_css*7.0`、`half = size*0.42`、尖端锚 `end`） |
| G-06 | 箭头两套几何（PDF `AL=9.0/AW=6.5` 世界单位） | 删掉 `arrow_head()` 与那两个常量，PDF 改调同一个 `tree_edge_arrow`。**一套公式，两个导出器** |
| G-07 | 树边几何不进 `Scene` | 不加字段；导出侧两条路径都只经过 `tree_edge_path` / `tree_edge_outline` / `tree_edge_arrow` 三个纯函数，单测直接断言它们与 `SceneEdge` 的对应关系 |
| （顺带） | PDF 是否二次缩放 | 实测 CTM 只有一处 `S 0 0 -S tx ty cm`（`S=0.75`），之后**全部写世界坐标**；新增断言钉住「几何点以世界坐标出现、`S` 只在 CTM 出现」 |

### 7.2 新增单测（纯函数优先；断言关键几何数值，不做字节比对）

| 测试 | 钉住什么 |
| --- | --- |
| `tree_edge_path_keeps_the_shape_the_canvas_draws` | 三个 `EdgeShape` 逐一：分段种类（`Curve`→1 个 `Cubic`；`Elbow`→2 个 `Line`；`Straight`→1 个 `Line`），且与 `edge_path_with(…, shape)` 逐点相等（≤1e-4） |
| `tapered_outline_has_the_documented_widths_at_both_ends` | `width/width_end = 3.5/1.75`（snow 一级边）：起点两侧距离 = 3.5、终点 = 1.75、中点 = 2.625（≤1e-3）；中轴线与 `tree_edge_path` 的等弧长采样点重合（≤1e-3） |
| `equal_widths_give_a_straight_band_not_a_cone` | 概念图主题（`taper:false`）或 `w0==w1`：轮廓全程等宽（±1e-4） |
| `elbow_outline_turns_at_the_corner` | `elbow` 边的中轴线确实经过 `(mid_x, start_y)` / `(mid_x, end_y)`（±0.01），`curve` 同一条边不经过 |
| `tree_edge_arrow_matches_the_canvas_triangle` | 三角形 = `(end, base±perp·half)`，`len=7`、`half=2.94`（`world_per_css=1`）、尖端 = `e.end`（≤1e-4）；`arrow=false` 时 `None` |
| `svg_fills_a_tapered_band_and_draws_the_arrow` | 渲染出的 SVG：树边不再以 `stroke-width` 线段出现，出现带 `fill` 的闭合 `<path … Z>`；`arrow=true` 时多一个三角形 |
| `pdf_fills_the_same_band_and_the_same_arrow` | PDF 内容流里轮廓用 `f` 填充、顶点数与导出侧同源；箭头尖端坐标 = `e.end`（世界坐标） |
| `pdf_applies_the_world_to_page_scale_exactly_once` | 页首 CTM 里缩放出现 2 次（x/y），此后几何是全坐标：已知点以世界坐标出现、不以 `(x*0.75, y*0.75)` 出现 |

### 7.3 「画布像素零变化」的论证

导出改动只落在 `export_svg.rs` / `export_pdf.rs`：

1. 画布像素由 `mw-render` 的 `instances.rs` + `compositor` + wgsl 决定；本轮**没有碰**这三者，
   也没有碰 `mw-core` 的 `scene.rs`（`SceneEdge` 字段一个没动，`width_end`/`shape` 早就在）。
2. `Scene` 的构造路径（`build_scene`）零改动 ⇒ 同一文档生成的 `Scene` 逐字段不变 ⇒ golden 截图不变。
   因此**不重拍任何基线**（本轮按要求不跑 golden，留待 Lead 落笔后整门复跑）。

### 7.4 未做（本轮）

- G-08 线宽档位、G-11 逐分支覆盖字段、G-12 色板对账：**拍板项，不动**。
- golden 重拍与门脚本：按要求不跑（不抢锁）；新场景 `mm-edge-*` 仍未开始。
- PNG 导出本身：走的是离屏 wgpu 渲染（与画布同一条路径），不需要改。

## 8. 出处与引用纪律

- 行号是**快照**；本文件所有引用都附了那一行的**字面**（`AGENTS` §9）。
- 审计只读：本包没有改 `docs/`、`scripts/`、`docs/STATUS.md`，也没有重拍任何基线。
- 体检：`powershell -NoProfile -ExecutionPolicy Bypass -File scripts/doc-hygiene.ps1`（结果见 §9）。

## 9. 变更记录

- 2026-11-02 20:2x（X24，**进行中**）：建立本文件。§2 测绘、§3 十二条差距、§4 十四条验收草案、
  §5 十三个场景（九个已有 / 四个新增）。**无代码改动**；触发「需拍板字段」1 项（G-11）。
- 2026-11-02 20:3x（X24，**进行中**）：补 `manifest.json`（同形状：status 进行中、12 条 gaps、
  14 条 AC、5 个新场景、三条待拍板）。文档体检
  `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/doc-hygiene.ps1`
  → **通过（57 篇 / 274 条链接 / 9 篇归档文件）**，本文件无断链。
- 2026-11-02 21:0x（X24，**进行中**）：补 §7「本轮实现：导出同源」（先写报告再动码，`AGENTS` §5 第 2 条）。
  实现**进行中**：G-01–G-07 六条，写作用域只有两个导出器；证据见
  `shots/x100/X24/evidence/export-parity-2026-11-02.md`。
- **未做**：G-08 / G-11 / G-12（拍板项）、golden 重拍与门、`docs/STATUS.md` 回填（都不在本包写作用域）。

