# G-B3-2 · 检查器 v2 人工门清单（P 轨 E2 出口）

> 依据：`docs/31` §1.3 的五条判词 + `docs/30` §4 全节；机器证据见
> [report.md](report.md) §2（十个基线）/ §3（逐条核对）/ §4（能力清单）。
> **本清单只列判词与证据入口，不代写结论。** 五条判词全部通过前，SE-05–SE-10 只记 `已实现`，
> 不记 `已验证`。
>
> **2026-11-02 晚更新**：第 4 条已复判通过，G-B3-2 现为 **5 / 5**。
> 下面「判词」表与「判词记录」是**当天傍晚的原判（4 / 5）**，原样保留不改；
> 复判依据、能力逐项落点与**三条限制**见下方「复判记录（2026-11-02 晚 · 授权代判）」一节。

## 判词

| # | 判词（`docs/31` §1.3 原文） | 看什么 | 证据入口 | 结论 |
| --- | --- | --- | --- | --- |
| 1 | **窄面板** | 默认 288、可拖 240–360、能记住；拖窄后控件仍放得下（色板 7 列、形状四段不塌） | `shots/golden/inspector/`、`inspector-narrow/` | ☑ 通过（授权代判） |
| 2 | **分组折叠** | 样式组与内容组默认展开、标记组默认折叠；关掉面板再打开，折叠状态还在 | `inspector`、`inspector-shape`、`inspector-content`；`window_state` 的折叠记忆单测 | ☑ 通过（授权代判） |
| 3 | **色板尾格** | 第 9 格是「自定义」虚线圆（点开就地取色），另有「无颜色」斜杠格；不是三处整行「自定义…」 | `inspector`、`inspector-shape`；SE-06 报告 §3 | ☑ 通过（授权代判） |
| 4 | **能力清单齐全** | `docs/30` §4.2 的 14 项：本轮 **10 项已实现、1 项部分（形状缺椭圆）、3 项延后（字重 / 对齐 / 圆角，都要加 `StyleOverride` 字段）** | [report.md](report.md) §4 那张表 | ☒ **不通过**：4 项能力未做（见判词记录 4） |
| 5 | **作用域明确** | 「本节点 / 含子树」写在分组标题右侧（不是页顶那一行），选中段有 8% 强调底；只作用于样式类属性 | `inspector-multiselect`；SE-08 报告 | ☑ 通过（授权代判） |

## 判词记录（2026-11-02 · 授权代判）

授权：用户 2026-11-02 原话「继续开发，而且呢，你帮助我验收，不需要我决定。」
逐条：

1. **窄面板** — 默认 288、可拖 240–360 并有落盘记忆；`inspector-narrow` 的轨迹
   `inspector w=240.00 …` 与 7 列色板、形状四段均未塌。修过「形状写死 66×4 顶开面板」的回归。
2. **分组折叠** — 样式组/内容组默认展开、标记组默认折叠；`seed_inspector_group` 只写内存，
   关面板再开仍按 `window_state.inspector_groups` 恢复；折叠记忆有单测。
3. **色板尾格** — 第 9 格是虚线圆「自定义」（点开就地取色），另有「无颜色」斜杠格；
   三处整行「自定义…」已删。
4. **能力清单（本门唯一不通过项）** — 如实盘点 14 项：**10 已实现 / 1 部分（形状缺椭圆）/
   3 延后（字重三档、文本对齐、圆角）**。后四项都要给 `StyleOverride` 加字段（数据模型变更），
   按 `docs/30` §4.2 与 `shots/x100/SE-06/report.md` §5 的方案单独批准；本轮按授权仍未动字段，
   所以这一条不能判通过。**门因此不通过**，直到该批能力补齐。
5. **作用域明确** — 「本节点 / 含子树」在分组标题右侧、选中 8% 强调底；只作用于样式类属性，
   换主题/写备注/删除会关掉 500ms 合并窗口。

**门结论**：G-B3-2 **未通过（4 / 5 条通过；第 4 条能力清单缺 4 项）**。
SE-05–SE-10 仍记 `已实现`；下一步是「`StyleOverride` 加字重/对齐/圆角/椭圆」的一批，
补齐后复判第 4 条即可。**其余四条不需要重做。**

## 复判记录（2026-11-02 晚 · 授权代判）

> 上一节「判词」与「判词记录」是**当天傍晚的原判（4 / 5）**，本节**不改写它们**，只追加复判。
> 授权仍是同一条用户原话（2026-11-02）：「继续开发，而且呢，你帮助我验收，不需要我决定。」
> 复判时逐条回代码核对过，**核不到的已如实标注**（见 §1 末尾那处口述与代码的出入）。

| # | 判词 | 原判（傍晚） | 复判（晚） | 复判依据 |
| --- | --- | --- | --- | --- |
| 1 | 窄面板 | ☑ 通过 | ☑ 通过（不重判） | 见上一节第 1 条 |
| 2 | 分组折叠 | ☑ 通过 | ☑ 通过（不重判） | 见上一节第 2 条 |
| 3 | 色板尾格 | ☑ 通过 | ☑ 通过（不重判） | 见上一节第 3 条 |
| 4 | 能力清单齐全（`docs/30` §4.2 的 14 项） | ☒ 不通过（缺字重 / 对齐 / 圆角 / 椭圆） | ☑ **通过** | 14 / 14 `已实现`：§1 模型 + §2 控件 + §3 导出同源 + §4 盘点 |
| 5 | 作用域明确 | ☑ 通过 | ☑ 通过（不重判） | 见上一节第 5 条 |

**门结论（复判）**：G-B3-2 **通过 5 / 5**（2026-11-02 授权代判）。
第 4 条缺的四项由 SE-13 补齐，原判的两条「已知缺口」同时关闭（见文末「已知缺口」）。
**门通过 ≠ `已验证`**：下面 §5 的三条限制必须与结论一起读。

### 1. 数据模型：三个可选键（`docs/32` §15 记的例外）

- `crates/mw-core/src/model.rs`：`pub struct StyleOverride {` 那行之后多出三个键，都带
  `#[serde(default, skip_serializing_if = "Option::is_none")]`：
  `pub radius: Option<f32>`（0–24）、`pub align: Option<String>`（`left` / `center` / `right`）、
  `pub weight: Option<u16>`（400 / 500 / 600）。**没设就不写键** ⇒ 旧 `.zmind` 逐字节不变，
  `mw-format` 的 `the_new_style_keys_are_optional_and_default_to_the_old_look` 钉住。
- `crates/mw-core/src/theme.rs`：`pub struct ResolvedStyle {` 那行之后有 `pub radius`、`pub align`、
  `pub weight: u16`（注释写的是「`weight` 覆盖优先，否则 `bold ? 600 : 400`」）；
  字重档位是 `pub const WEIGHT_REGULAR: u16 = 400;` / `WEIGHT_MEDIUM = 500` /
  `WEIGHT_SEMIBOLD = 600`，以及 `pub const WEIGHT_BOLD_ALIAS: u16 = WEIGHT_SEMIBOLD;`
  ——**旧 `bold` 是 600 的别名，不是 700**（理由见 [`../SE-13/report.md`](../SE-13/report.md) §3.1：
  TYPO-01 定下的粗体字面本来就是 Semibold 600，取 700 会把所有既有基线漂一格）。
- `fn apply_concept_defaults(` 那行（概念图在级联之后补默认值）改主概念时，
  `style.bold = primary;` 之后紧跟 `if override_.and_then(|s| s.weight).is_none() {` 分支，
  把 `style.weight` 同步成 `WEIGHT_BOLD_ALIAS`；单测
  `concept_themes_move_the_primary_concept_to_the_semibold_weight` 钉住。
  没有这一步，换 `cm-*` 主题时中心概念会掉回 400（[`../SE-13/report.md`](../SE-13/report.md) §6 第 1 条）。

**一处口述与代码的出入（如实记）**：复判时听到的口径是「字重三段 常规 400 / 中等 600 / 粗 700 别名」，
代码与合同（`docs/30` §4.2「字重 400/500/600 三段」）都是 **400 / 500 / 600**；
700 只作为**解析上限**出现在 `w.clamp(WEIGHT_REGULAR, 700)` 那行。本节按代码记，不按口述记。

### 2. 检查器控件（`crates/mw-shell/src/panels/inspector/inspector_methods_1.rs` · 行号是快照）

| 能力 | 落点（括号里是那一行的字面，行号漂了也能找到） |
| --- | --- |
| 形状（含椭圆） | `for (s, label) in [` 之后的四段是 `(Shape::Rounded, "圆角"),`、`(Shape::Rect, "直角"),`、`(Shape::Capsule, "胶囊"),`、`(Shape::Ellipse, "椭圆"),`——**椭圆是第 4 段**，不是「四段 + 第 5 段」；下划线搬去 B / I 同排的 U 开关（`shape = underline` ↔ 清空） |
| 圆角 stepper | 「圆角」行 + `stepper(`（0–24、步 2）+ ↺「恢复默认圆角」；只在 `let roundable = matches!(shape, None | Some(Shape::Rounded) | Some(Shape::Rect));` 那行的分支里出现——胶囊/椭圆/下划线不显示，免得给出改了看不出效果的控件 |
| 字重三段 | `mw_core::theme::WEIGHT_REGULAR,` / `WEIGHT_MEDIUM,` / `WEIGHT_SEMIBOLD,` 三段；行首「字重」+ 已覆盖圆点 + ↺；B 开关写 `s.weight = Some(want);`（开 600 / 关 400），**不再写 `bold`**（旧键只读不写） |
| 文本对齐 | 三段「左 / 中 / 右」；整行被 `if !doc.is_concept_map() {` 那行包住——**概念图不出该行**（`docs/30` §5.3 合同是概念居中，渲染层却是「显式 `align` 优先于 `center_text`」，两个口径只能选一个，理由见 [`../SE-13/report.md`](../SE-13/report.md) §7 末条） |
| 椭圆哨兵（一处判定） | `crates/mw-render/src/instances.rs` 的 `pub const ELLIPSE_CODE: f32 = -1_000.0;` + `pub fn corner_style(` 单一出口；画布编辑描边也走它（`crates/mw-shell/src/canvas.rs` 的 `let base = mw_render::corner_style(&n.style, r);` 那行），不再自己写一份 `match shape`（SE-13 复审修的第 3 条） |
| 窄面板不塌 | 240 面板下字重/对齐三段不顶宽：单测 `the_weight_and_align_segments_fit_the_narrowest_panel` |
| 组内行间距 6 | `crates/mw-shell/src/panels/inspector.rs` 的 `pub const ROW_GAP: f32 = 6.0;` + `fn group_list<R>(` 里的 `ui.spacing_mut().item_spacing.y = geo::ROW_GAP;` 那行，只作用于分组体（样式/内容/标记/概念四页）；`GROUP_GAP`（组间 8px hairline 的**带宽**，不是行距）与 `GROUP_H` 未动；单测 `the_group_body_compresses_the_row_gap_to_six` 与 `the_skeleton_geometry_matches_the_contract`（后者含 `assert_ne!(geo::GROUP_GAP, geo::ROW_GAP, …)`，防有人把两个 8/6 化简成一个常量）。基线待重拍，见 §5 第 2 条 |

### 3. 导出同源（`crates/mw-shell`）

- SVG：`export_svg.rs` 里 `write!(out, " font-weight=\"{}\"", job.weight);` 那行按真实字重写属性、
  `text-anchor` 跟 `TextAlign` 走；椭圆走 `"<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"",` 那条分支
  （判据 `radius <= mw_render::ELLIPSE_CODE + 0.5`）。单测
  `export_svg::tests::the_new_style_keys_reach_the_svg` 同时断言 `font-weight="600"` /
  `text-anchor="end"` / `rx="14"` / `<ellipse `。
- PDF：伪粗判据已从 `t.bold` 改成 `let pseudo_bold = t.weight >= mw_core::theme::WEIGHT_SEMIBOLD;`；
  圆角/椭圆与画布同走 `mw_render::corner_style`（[`../SE-13/report.md`](../SE-13/report.md) §6 第 2 条）。
- **口径**：导出证据是**单测级**，不是「导出到桌面文件」的端到端核对（那要开文件对话框）。
  与 SE-01–12 的「离屏等效」同一口径，记在 §5 第 1 条。

### 4. 能力清单：14 / 14

- [`report.md`](report.md) §4 那张表已补记：第 3 / 4 / 6 / 7 项标成 `已实现（SE-13，2026-11-02）`，
  14 项现在全部 `已实现`，表下另有 2026-11-02 的追补段。
- 像素层不是只看「面板里多了一行」：四张新场景（`inspector-weight` / `-align` / `-radius` / `-ellipse`）
  相对 `inspector-shape` 的差异包围盒**从 x = 19 起**（画布区真的变了），而对照场景 `inspector`
  的差异只在面板区（x ≥ 1506）——[`../SE-13/verify.md`](../SE-13/verify.md) §1。
- 基线：SE-13 重拍 11 场 + 新增 4 场，**15 场双跑 `--tol 0` 零差异**
  （[`../SE-13/report.md`](../SE-13/report.md) §5.2）。

### 5. 三条限制（门通过，但不许写成「已验证」）

1. **门通过 ≠ 已验证**：**实机微软拼音**（标签框 / 备注框 / 链接框）与**系统 DPI** 两项欠账仍在，
   检查器只做到离屏等效。与 G-B2-1 / G-B2-3 同一口径：本条判词过了，包状态仍是 `已实现`。
2. **机器门：矩阵层面已全绿，正式门整跑待补**（2026-11-02 19:35–19:47，`scripts/golden-matrix.ps1`，
   同一二进制 mtime **19:32:30**，场景表与环境同正式门）：`stage-b` **178 场 × 2 遍 = 356 次比较零差异**、
   `stage-cm` **73 场 × 2 遍 = 146 次比较零差异**。
   **此前 19:05 的 177/178 与 70/73 是并发假红**：`mm-rel-link-created` / `mm-summary-hover` 等全部证实为
   并行会话当时在 `rustc` 构建 + 跑 `mw-shell`、抢 CPU/GPU 导致 harness 超时，单场景复跑逐个通过；
   只有 `cm-inspector` / `cm-inspector-pinned` 是**真漂移**——组内行距 8→6 的合同改动（只在面板区），
   **已重拍并复核零差异**（与 §2 最后一行同一笔；文末「已知缺口」第 4 条的「基线待重拍」以本条为准）。
   门的场次与结论以 [`docs/STATUS.md`](../../../docs/STATUS.md) §1 为准，本节如与那里不一致以那里为准。
   **三条限制并列**：① 这两遍跑的是**已构建二进制**，正式门脚本 `x100-stage-b-gate.ps1` 会被并行会话的
   源码改动触发约 4 分钟的 LTO 重建，**「正式门整跑」待对方落笔后补**，本文件不替它签字；
   ② 全量测试 1313 / 0 / 1 ignored、`fmt`、门内 clippy 是**分别实测**的，不是这次矩阵带出来的；
   ③ `已验证` 仍欠实机微软拼音与系统 DPI 两项（同本条 ①）。
3. **低优先项只记账、没做**：500 家族字重回退（egui 侧只有常规/600 两个族，行内编辑器 500 落回常规族）、
   500ms 合并是「同批 id」粒度（先点字重再点对齐会并成一条撤销）等，只在
   [`../SE-13/report.md`](../SE-13/report.md) §6 末两条与 §7 记账，未做。

## 机器判据已完成的部分（无需人工重做）

- 十个检查器基线 `--tol 0` 双跑零差异（`inspector` / `-narrow` / `-empty` / `-shape` / `-ink` /
  `-mist` / `-sand` / `-content` / `-multiselect` / `-error`），`cm-arrow-inspector` 同门复核。
  （复判时这组已扩到 **15 场**：SE-13 重拍 11 场 + 新增 `inspector-weight` / `-align` / `-radius` /
  `-ellipse` 四场，见复判记录 §4。）
- `docs/30` §4.1–§4.6 逐条核对（[report.md](report.md) §3）：几何、控件、命令、反馈、键盘全部对上，
  只剩两条已知缺口（组内行间距仍是 8 而不是合同的 6；`dim` 在 `panel2` 上是 4.14:1）。
  ——**其中「组内行间距 8」一条已于 2026-11-02 晚关闭**（`geo::ROW_GAP = 6.0` + `group_list()`，
  见复判记录 §2 末行与文末「已知缺口」第 4 条）；`dim` 4.14:1 仍在。
- 行为证据：作用域两段与 500ms 合并（SE-08）、数字键/Enter/Esc 两级/错误条（SE-09）、
  备注 3/8 行与展开（SE-07）、标签 6px 色点形态（SE-07）。
- 单测：面板几何常量、色板列数与跨行、stepper 夹取、内容组默认展开、标签行宽与行高、作用域手势键、Esc 判据、四主题对比度。

## 已知缺口（门判词不得把这些算作通过）

1. ~~**能力清单缺三项**（字重 / 文本对齐 / 圆角）与**形状缺椭圆**：都要动 `StyleOverride`，
   属数据模型变更，需单独批准；方案在 `shots/x100/SE-06/report.md` §5。~~
   **已关闭（2026-11-02 晚）**：SE-13 按 `docs/32` §15 记的例外补齐 `StyleOverride.radius` /
   `align` / `weight` 三个可选键（`crates/mw-core/src/model.rs` 的 `pub struct StyleOverride {`
   那行之后，三者都 `skip_serializing_if` 缺键不写），形状行补「椭圆」段、下划线搬去 B/I 同排的 U 开关；
   能力清单 14 / 14 全部 `已实现`。证据：[`../SE-13/report.md`](../SE-13/report.md) §4.1–§4.4、
   [`../SE-13/verify.md`](../SE-13/verify.md) §1–§2、[`report.md`](report.md) §4 的 2026-11-02 追补段。
2. **实机微软拼音**在检查器里没核过（标签框、备注框、链接框）：只做了离屏等效。
3. **Esc 两级**的跨帧接线无头拍不出（egui 焦点不跨 `run_ui` 保留），只测了判据本身。
4. ~~**组内行间距 6** 还没落（现在是全局 8），会连同所有检查器基线一起变；记作下一轮候选。~~
   **已关闭（2026-11-02 晚）**：`crates/mw-shell/src/panels/inspector.rs` 新增
   `pub const ROW_GAP: f32 = 6.0;` 与 `fn group_list<R>(`（里面 `ui.spacing_mut().item_spacing.y = geo::ROW_GAP;`
   那行），只对分组体生效；`GROUP_GAP`（8px hairline 带宽）与分组标题 28 未动；
   单测 `the_group_body_compresses_the_row_gap_to_six` 把「外面 8、里面 6」两头钉住。
   漂移（`--tol 0`、**未 `--update`**、连跑两次同数）：`inspector` 场景
   `43820/2073600 像素超容差（2.113%），最大差 208，包围盒 x[1506..1901] y[406..1040]`，全部落在面板内
   （面板左边界 = (1280−288)×1.5 = 1488）；另 13 个检查器场景 1.695%–2.406% 属同一笔。
   **基线待重拍**（`stage-cm` 的 `cm-inspector` / `cm-inspector-pinned` 也在这笔里），见复判记录 §5 第 2 条。
5. `dim` 在 `panel2`（卡片候选列表底）上是 4.14:1，差 AA 0.36。
