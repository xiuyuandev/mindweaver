# X22 字阶 / 字重 / 行高 / CJK 混排 —— 设计合同与现状测绘（报告先行）

- **状态**：`进行中`。第一轮只交文档与只读测绘（§1–§6）；Lead 2026-11-02 裁决后追加两件本轮就收的事：
  **G04 修码**（`mw-render/src/text.rs` + `dwrite.rs` 与它们的单测，见 §7.0 与
  [`evidence/g04-measure-parity-2026-11-02.md`](evidence/g04-measure-parity-2026-11-02.md)）与
  **§7 画布字阶表**。仍未做：任何 golden 采集/重拍、门登记、`docs/` 与 `crates/mw-core` 改动（Lead 明确禁止）。
- **路线出处**：`docs/24-X100升级路线.md` 阶段 C「- X22 重做 DirectWrite 字阶、字重、行高和 CJK 混排，
  统一画布与编辑器度量。」；执行协议见 `docs/24` §4（报告先行、四类证据、双跑哈希）。
- **设计权威**：`docs/28`（内容体验重构规范）§4.1 字阶、§4.2 版心、§5.2 行解剖、§6.2 正文排版、
  §9.3 判词；`docs/30` §7.6（可量化视觉项与「字号只有 `design::text::*` 令牌」）；
  `docs/07` §4/§5（禁则、混排间距、字体栈）；`docs/26` §6（DPI 矩阵 1.00/1.25/1.50/2.00）。
  `docs/06` 只作外壳令牌参考：它的全屏大纲旧规格已被 `docs/28` §5 取代，**不当合同**。
- **测绘方式**：`rg` / 只读 `read`，每条给出**文件:行 + 那一行的字面**（行号是快照，字面才是锚）。
  完整抓取记录见 [`evidence/read-code-2026-11-02.md`](evidence/read-code-2026-11-02.md)。

---

## 1. 范围与非目标

### 1.1 范围（本包要给合同的六个面）

| 面 | 现状实现位置（只读测绘） |
| --- | --- |
| 画布节点文字 | `mw-render::dwrite`（DirectWrite 真字宽 + 断行 + 禁则兜底）→ `mw-render::text`（字形图集 / `TextJob`） |
| 行内编辑器 | `mw-shell::canvas` 的 egui `TextEdit` 覆盖层（epaint 排版，与画布两条调用） |
| 全屏大纲行 | `mw-shell::panels::outline`（egui 直接排版，行高固定 32） |
| 卡片页 / 库预览正文 | `mw-shell::note_view` 吃 `mw-markdown` AST，套 `mw_core::design::content` 字阶 |
| 笔记库列表 | `mw-shell::content_kit::font_for` + `note_library` 的 `content::LIST_*` |
| 四条派生路径 | 画布合成器 / PNG（`Canvas::export_png`）/ SVG（`export_svg.rs` 写 `<text>`）/ PDF（`export_pdf.rs` 写字形轮廓） |

### 1.2 非目标（本轮明说不做）

- **不动数据模型**：`StyleOverride` 不加字段。斜体 / 下划线 / 删除线 / 逐节点行高覆写属 X44，
  本包只登记「模型里没有该键」。不改 `.zmind`、不加迁移。
- **不做 X23**（卡片形状/边框/阴影）、**不做 X25**（层级样式与主题默认值重校）、
  **不做 X27**（状态表）、**不做 X44**（检查器字体控件）。
- **不引入依赖**（不加变量字体、不加 `skrifa`/`cosmic-text` 类新排版库）。
- **不重拍任何基线**，不登记新门场次：§5 只列名字与「钉住什么」，实际拍图归收口方。
- **不动 `crates/`**：本包交付物只有本文件、`evidence/`、`manifest.json`。

---

## 2. 现状表（画布 / 编辑器与内容面 / 导出 三列）

### 2.1 字阶

| 项 | 画布（DirectWrite 路径） | 编辑器与内容面（egui 路径） | 导出 | 出处（字面） |
| --- | --- | --- | --- | --- |
| 令牌 | `design::text`：中心 28 / 一级 20 / 二级及更深 16 / 最深层 15 | `design::content` 14 档（30/28/22/18/16/16/16/15/13/12/14/12/12/14） | 继承画布 `TextJob.font_size` | `design.rs:39` `pub const CANVAS_SUB: f32 = 16.0;`；`design.rs:44-48` `CANVAS_LEAF 15 / CANVAS_MAIN 20 / CANVAS_CENTER 28`；`design.rs:120-235` `PAGE_TITLE…CODE` |
| 层级 → 字号 | `theme::LEVEL_TYPE` 四元组 `(size, bold)`：`(28,true) (20,false) (16,false) (15,false)` | 内容面按角色取常量，不随层级 | 同画布 | `theme.rs:265` `pub const LEVEL_TYPE: [(f32, bool); 4] = [` |
| 壳层字阶 | — | `design::ui`：12 / 13 / 14 / 15 / 16 / 26（`chrome::Type` 只是别名） | — | `design.rs:78-92` `pub const BODY: f32 = 14.0;`；`chrome.rs:105-120` `pub struct Type;` |
| 裸字号防线 | 画布里不许出现裸字号字面量（有测试拦） | 壳层同样有防回退测试 | — | `docs/30` §7.6 第 751–752 行；`chrome.rs:473` `/// 壳层源码禁止写裸字号：字号只能来自 Type / design` |

**画布字阶没有合同表**：`docs/28` §4.1 只覆盖内容表面；`docs/30` §7.6 只说「画布字号只能来
`design::text::*` 五个令牌」，四个画布令牌与层级的对应只存在于代码（`theme.rs:265`）。
→ 见 G01。

### 2.2 行高

| 项 | 画布 | 内容面（大纲 / 卡片 / 库） | 出处（字面） |
| --- | --- | --- | --- |
| 公式 | `round(字号 × 分档倍数)`，倍数按字号分四档：≤12→**1.5**、≤15→**1.46**、≤21→**1.44**、其余→**1.4** | 每个角色一个**绝对值**（`TypeStyle.line_height`），与字号无关 | `design.rs:58-69` `if size <= 12.0 { 1.5 } else if size <= 15.0 { 1.46 } else if size <= 21.0 { 1.44 } else { 1.4 }`；`text.rs:31-33` `(font_size * crate::design::text::line_height(font_size)).round()` |
| 实际落值 | 28→**39**、20→**29**、16→**23**、15→**22**（比值 1.393 / 1.450 / 1.4375 / 1.467） | H1 30/42、大纲标题 28/40、H2 22/33、H3 18/28、H4 16/26、大纲行 16/26、阅读正文 16/28、列表标题 15/22、摘要 13/19、元信息 12/18、导航 14/21、辅助 12/18、标签 12/18、代码 14/22 | `layout.rs:85` `pub line_height: f32,`（节点几何直接存 `Measured.line_height`）；`design.rs:120-235` |
| 合同值 | **无表**（见 G01） | `docs/28` §4.1 表（第 162–175 行）：30/40、28/38、22/32、18/28、16/26、16/26、16/28、14.5/22、12.5/19、11.5/16、13/20、12/18、11.5/16、13.5/22 | — |
| 基线 | `line_height × 0.72`，垂直居中：`start = (盒高 − 标签高 − 行数×行高)/2` | egui 自身行盒；卡片页显式 `line_height`；大纲单行**不锁**、换行才锁 26 | `layout.rs:829` `pub const BASELINE_RATIO: f32 = 0.72;`；`layout.rs:836-851` `line_baseline`；`note_view.rs:2300` `line_height: Some(self.line_h),`；`outline.rs:306` `/// line_height 只在真的换行时给 26` |

→ 见 G02（两套公式）、G03（合同值与代码 9/14 档不一致）、G08（大纲单行不锁 26）。

### 2.3 字重

| 项 | 画布 | 内容面 / 壳层 | 导出 | 出处（字面） |
| --- | --- | --- | --- | --- |
| 档位 | 400 / 500 / 600（700 只在钳制上界出现） | 同 | 写 `font-weight="{weight}"` | `theme.rs:1264-1266` `pub const WEIGHT_REGULAR: u16 = 400;` … `WEIGHT_SEMIBOLD: u16 = 600;`；`export_svg.rs:656` `write!(out, " font-weight=\"{}\"", job.weight)` |
| 旧别名 | `bold: true` ≡ 600 | 同 | 同 | `theme.rs:1268` `pub const WEIGHT_BOLD_ALIAS: u16 = WEIGHT_SEMIBOLD;`；`theme.rs:1415-1419` 钳制 `clamp(WEIGHT_REGULAR, 700)` |
| 落到哪个字面 | 原样交给 `CreateTextFormat(…, DWRITE_FONT_WEIGHT(weight), …, font_size, …)`，由 DWrite 取族内最近面 | 600→`FontFamily::Name("zhisi-bold")`（Segoe Semibold + 雅黑 Bold）、500→`"zhisi-semibold"`（Segoe Semibold + 雅黑**常规**）、400→`Proportional`（Segoe + 雅黑） | 与画布同 weight 数字 | `dwrite.rs:105-127` `self.factory.CreateTextFormat(…, DWRITE_FONT_WEIGHT(weight as i32), …)`；`content_kit.rs:46-53` `600 => … BOLD … 500 => … SEMIBOLD … _ => FontFamily::Proportional`；`fonts.rs:122-128` `read("msyh.ttc", 1)` / `read("msyhbd.ttc", 1)` / `read("segoeui.ttf", 0)` / `read("seguisb.ttf", 0)` |
| 装了几面 | 系统族：雅黑只有 400/700，Segoe UI 有 400/600/700 | egui 只装了 400（`msyh.ttc#1`、`segoeui.ttf`）、600（`seguisb.ttf`）、700（`msyhbd.ttc#1`） | — | 同上 `fonts.rs:122-128` |
| 500 的口径 | 代码相信「由系统按最近可用字面合成」（未实测） | 内容面 500 = 拉丁 600 + 中文 400（不对称） | 写 500 | `theme.rs:1259-1263` `/// 500 由系统按最近的可用字面合成，但**度量与绘制都走同一个 weight**` |

→ 见 G06（500 无真面且两侧落面不同）、G07（CJK 600 = 雅黑 700）。

### 2.4 字号 → 物理像素 / DPI

| 项 | 画布 | 编辑器 | 导出 | 出处（字面） |
| --- | --- | --- | --- | --- |
| 单位 | DIP（= 旧 CSS px）；世界坐标 DIP，物理像素 = `zoom × ppp` | egui points × ppp | PDF 另按 96dpi→72pt `×0.75` | `dwrite.rs:1-5` `//! 单位是 **DIP**（等于旧版的 CSS px）`；`export_pdf.rs:8-9` `内容按 96dpi→72pt（×0.75）换算` |
| 光栅倍率 | `ppd = quantize_ppd(zoom × ppp)`，档位 0.5/0.75/1.0/1.25/1.5/1.75/2.0/2.5/3.0/4.0 **向上取整** | 精确 `ppp`（epaint 自己光栅） | PNG 走同一 `TextEngine`；SVG/PDF 不涉及光栅 | `text.rs:240` `let ppd = quantize_ppd(f.camera.zoom * f.camera.ppp);`；`glyph.rs:31-43` `PPD_BUCKETS` / `find(|b| *b >= p - 1e-4)` |
| 字形 em | `em = font_size × ppd`，`IDWriteFactory2` 灰度 NATURAL 优先、基工厂 GDI ClearType 兜底 | epaint 内部（`skrifa` 借用 mmap 字体 blob） | — | `glyph.rs:283-320` `let em = font_size * ppd;` … `DWRITE_RENDERING_MODE_NATURAL` … `DWRITE_RENDERING_MODE_GDI_NATURAL`；`fonts.rs:6-10` `epaint 0.37 的 skrifa::FontRef` |
| 字体栈 | `FONT_CANDIDATES = ["Segoe UI", "Microsoft YaHei UI", "Microsoft YaHei", "Noto Sans SC"]`，locale `zh-Hans` | `Proportional` = Segoe → 雅黑（同族优先 Segoe，中文按字形回退） | SVG 写 `font-family="{DWrite 候选族}"`，查看器自行回退 | `dwrite.rs:28-33`；`dwrite.rs:38` `pub const LOCALE: &str = "zh-Hans";`；`export_svg.rs:648-660` |

→ 见 G09（画布量化档 vs 编辑器精确 ppp）。

### 2.5 CJK 混排：断行、禁则、0.1em 间距

| 项 | 画布 | 编辑器 / 大纲 / 卡片页 | 出处（字面） |
| --- | --- | --- | --- |
| 断行 | DWrite `CreateTextLayout(limit, …)` + 本项目禁则兜底，兜底后**按新行重量宽**，最多 3 趟收敛 | egui/epaint 自己的折行（无禁则） | `dwrite.rs:250-280` `enforce_kinsoku(&mut lines)` … `if !rest.is_empty() && … && kinsoku_legal(&rest)`；`dwrite.rs:57` `const REFINE_PASSES: usize = 3;` |
| 禁则表 | 行首 20 字 `，。、；：？！）】」』》〉…·—～%”’`、行尾 6 字 `（【「『《〈` | **无** | `text.rs:46` `pub const FORBIDDEN_LINE_START: &str = "，。、；：？！）】」』》〉…·—～%”’";`；`text.rs:49` `FORBIDDEN_LINE_END` |
| 中英混排 0.1em | **只在渲染期步进**：`CJK_LATIN_GAP_EM * font_size` 加到每个「汉字↔ASCII 字母数字」边界，累进到 `ShapedLine.width` | 无（egui 无 `text-autospace`） | `text.rs:37` `const CJK_LATIN_GAP_EM: f32 = 0.1;`；`text.rs:409-421` `gap_sum += gap_before; width = width.max(raw.x + raw.advance + gap_sum);`；`text.rs:793-811` `cjk_latin_gaps` / `crosses_script` |
| 度量是否含这 0.1em | **不含**：`width_of` 只读 DWrite `DWRITE_TEXT_METRICS.width` | — | `dwrite.rs:304-323` `let mut metrics = DWRITE_TEXT_METRICS::default(); one.GetMetrics(&mut metrics)?; width = width.max(metrics.width.max(0.0));` |
| 对齐锚点用哪份宽 | 渲染 `Center(c) => c - line.width * 0.5`（**含**间距） | 内容面各自排版 | `text.rs:250-251` / `text.rs:306-307` |

→ 见 G04（度量与渲染不同源）、G05（两套 shaper、编辑器无禁则）。

### 2.6 画布 ↔ 编辑器的一致性机制（现状）

| 机制 | 做法 | 出处（字面） |
| --- | --- | --- |
| 字号 | `n.geom.font_size × zoom` | `canvas.rs:1981` `let font_size = (n.geom.font_size * zoom).max(6.0);` |
| 字重 | `weight ≥ 600 → BOLD 族`，否则 `Proportional`（真实打算 500 也落 400 族） | `canvas.rs:1976-1985` `let weight = n.style.weight; let family = if weight >= mw_core::theme::WEIGHT_SEMIBOLD {` |
| 行高 | 锁到画布那一份：`(n.geom.line_height * zoom).max(font_size)`，写进 `LayoutJob.section.format.line_height` | `canvas.rs:2004` `let line_h = (n.geom.line_height * zoom).max(font_size);`；`canvas.rs:2115` `section.format.line_height = Some(line_h);` |
| 首行基线 | 用同一个 `edit_galley` 排一个探针字（`"中"`），实测 `row.pos.y + glyph.pos.y` 反推位移 | `canvas.rs:2103-2116` `let probe = edit_galley(ui.ctx(), BASELINE_PROBE, …)`；`canvas.rs:2140-2150` `first_baseline` |
| 内距 | `card_padding(font_size)` 按四档：≥26→(26,12)、≥19→(18,10)、≥14.5→(14,8)、其余→(12,7) | `design.rs:690-700` `pub fn card_padding(font_size: f32) -> (f32, f32)` |

---

## 3. 差距清单（13 条，按严重度排序）

每条格式：**现状 → 合同要求 → 影响面 → 要动哪里（crate / 数据模型）**。

### P0（不解决就无法「统一度量」）

**G01 画布字阶没有合同表。**
- 现状：画布字号/行高只存在于代码（`theme.rs:265` 的四元组 + `design.rs:58-69` 的四档倍数），
  `docs/28` §4.1 只写内容表面；`docs/30` §7.6 只说「画布字号只能来 `design::text::*` 五个令牌」。
- 合同要求：X22 必须给出**画布字阶表**（层级 / 字号 / 行高 / 字重 / 用途），像 `docs/28` §4.1 那样可直接断言。
- 影响面：所有画布基线（字号或行高一改，节点盒高就变）、编辑器行高（跟着 `geom.line_height` 走）。
- 要动：`mw-core`（`design.rs` 加常量/表）；**不动数据模型**。文档侧要新增合同表（归 Lead / 报告 owner）。

**G02 行高两套公式。**
- 现状：画布 = `round(字号 × 分档倍数)`（1.5/1.46/1.44/1.4）；内容面 = 每角色一个绝对值（26/28/22…）。
  同一句 16px 文字，画布行高 **23**、大纲/卡片 **26/28**。
- 合同要求：`docs/28` §4.1 给的比值（1.33…1.75）与画布倍数（1.4–1.5）不是同一套；X22 要定一条：
  要么「一张表管全部表面」，要么「画布倍率 / 内容绝对值」两条并写明各自适用面。
- 影响面：画布节点盒高、卡片页/大纲行距、`line_baseline` 的居中量；所有基线。
- 要动：`mw-core::design`（行高出口）+ `mw-render::text`（图集键随行高？不，键只有字号，但基线位置变）；
  **不动数据模型**。

**G03 `docs/28` §4.1 与 `design::content` 的 14 档里 **9 档不一致**。**
- 现状（`design.rs:120-235` vs `docs/28` 第 162–175 行）：

| 角色 | docs/28 §4.1 | `design::content` | 差 |
| --- | --- | --- | --- |
| 页面标题 H1 | 30 / **40** / 600 | 30 / **42** / 600 | 行高 +2 |
| 大纲页标题 | 28 / **38** / 600 | 28 / **40** / 600 | 行高 +2 |
| H2 | 22 / **32** / 600 | 22 / **33** / 600 | 行高 +1 |
| H3 | 18 / 28 / 600 | 同 | — |
| H4 | 16 / 26 / 600 | 同 | — |
| 大纲行 | 16 / 26 / 400 | 同 | — |
| 阅读正文 | 16 / 28 / 400 | 同 | — |
| 列表标题 | **14.5** / 22 / **500** | **15.0** / 22 / **600** | 字号 +0.5、字重 +100 |
| 列表摘要 | **12.5** / 19 | **13.0** / 19 | 字号 +0.5 |
| 列表元信息 | **11.5** / **16** | **12.0** / **18** | 字号 +0.5、行高 +2 |
| 行内说明/导航 | **13** / **20** | **14** / **21** | 字号 +1、行高 +1 |
| 辅助/元信息 | 12 / 18 | 同 | — |
| 小标签/角标 | **11.5** / **16** / **500** | **12.0** / **18** / **600** | 字号/行高/字重全差 |
| 代码/等宽 | **13.5** / 22 | **14.0** / 22 | 字号 +0.5 |

- 合同要求：二者只能有一个真相。**建议以 `docs/28` §4.1 为准改码**（文档是设计合同、且已被
  CER 各包引用），但这会让卡片页/库/大纲 9 个角色的排版整体变化 → 全量重拍。
- 影响面：卡片页 36+ 张、库 20+ 张、大纲若干、`inspector-*`（若用到 `content::NAV`）→ 数量以
  `docs/STATUS.md` §1 为准，本包不抄数字。
- 要动：`mw-core::design::content`（纯常量）；**不动数据模型**。**需 Lead 拍板**（改码 vs 改文档）。

**G04 中英混排 0.1em 只在渲染期加，度量不知道它。**
- 现状：`text.rs:409-421` 把 `0.1em` 累进 `ShapedLine.width`，而决定**盒宽与折行**的
  `dwrite.rs:304-323` 只读 DWrite 的 `DWRITE_TEXT_METRICS.width`。后果两条：
  (a) 一条行里 N 个「汉字↔拉丁」边界，渲染宽比度量宽多 `0.1em × N × 字号`，混排行可能**溢出**节点盒；
  (b) 居中/右对齐锚点用含间距的宽（`text.rs:250-251`），而场景居中用不含间距的宽 → 文字整体**偏左/偏右 `间距和/2`**。
- 合同要求：`docs/07` §4 第 90 行（ADR-010 对照表）自称「渲染期步进（**计入行宽**、不写回文本）」——
  要求本身正确，实现只做到了「渲染期」，没做到「计入行宽（= 参与度量与折行）」。
- 影响面：所有含中英混排的节点（`tiny-cn`、演示文档）、折行点、命中盒、SVG/PDF 文本位置（都吃 `TextJob`）。
- 要动：`mw-render::dwrite`（度量侧补间距）+ 可能 `mw-core::text::Measured` 口径；**不动数据模型**。

**G05 两套 shaper：画布 DWrite，其余表面 epaint。**
- 现状：画布走 DirectWrite 逐字形（`dwrite.rs` / `text.rs`），行内编辑器、全屏大纲、卡片页、库都走
  egui/epaint；两边只靠「行高锁定 + 基线实测 + 同一份 `Scene` 几何」对齐（`canvas.rs:2004/2115/2103`）。
  编辑器侧**没有禁则**，折行点、字距、字偶距都与画布不同。
- 合同要求：`docs/28` §5.4「编辑框与行使用同一字体、字号、行高、左边距」；§6.3「不在渲染层与编辑层
  之间切换控件」。合同没要求同一 shaper，但 X22 的标题「统一画布与编辑器度量」至少要**度量一致**
  （行宽/折行/行距），否则编辑态一按 F2 就重新折行。
- 影响面：F2 编辑态、大纲、卡片页；任何「编辑态与画布同一行」的 golden。
- 要动：`mw-shell`（编辑器侧要么接受 egui 折行并做等价断言，要么把 DWrite 分行结果喂给编辑器）；
  **不动数据模型**。

### P1（合同可断言但会连带重拍）

**G06 500 档没有真面（探针已实测：画布与内容面落面一致）。**
- 现状（2026-11-02 探针读数）：雅黑只有 400/700、Segoe UI 有 400/600/700，**没有 500**。
  `请求 500` 在 Segoe UI 落 **600 Semibold**、在雅黑落 **400 Regular**；内容面 `zhisi-semibold`
  族是 [Segoe Semibold, 雅黑 Regular]，落面**完全相同**（见 §7.2 表）。
  所以「500」实际是「拉丁半粗 + 中文常规」的**非对称档**，不是独立字重。
- 合同要求：`docs/28` §4.1「只用 400 / 500 / 600 三档字重」；`docs/30` §4 检查器字重三段。
  三档必须**看得见差别**，且同一档在画布与内容面同形。
- 影响面：检查器字重三段（SE-13）、`docs/28` 列表标题/小标签（500）。
- 要动：`mw-render::dwrite`（选面策略或改用变量字体）+ `mw-shell::content_kit::font_for`；**不动数据模型**。
  **需 Lead 拍板**：定「500 落到哪一面」的口径（现无证据）。

**G07 CJK 600 = 雅黑 700 面（探针实测确认）。**
- 现状：探针 `请求 600 → 实际面 700`（雅黑族）；egui 侧 `zhisi-bold` = `msyhbd.ttc#1`（700）。
  即「半粗中文」实际是**粗体**，且是字体里唯一比常规重的真面（不是合成粗体）。
- 合同要求：`docs/28` §4.1 第 179 行「只用 400 / 500 / 600 三档字重；**不用 700，不用合成粗体**」。
- 影响面：所有粗体中文（根主题、H1–H4、大纲标题）；字重视觉分级失真。
- 要动：字体策略（捆绑/变量字体 或 改用 400 + 描边模拟），**可能新增资源文件**（需 Lead 拍板，
  且与「不引入依赖」的边界相邻）；**不动数据模型**。

**G08 大纲行高：单行不锁 26，换行才锁。**
- 现状：`outline.rs:306` `/// line_height 只在真的换行时给 26——单行仍用字体行高，既有基线零漂移；`
  `outline.rs:321` `job.sections[0].format.line_height = Some(26.0);` 只在多行分支。
- 合同要求：`docs/28` §5.2 行解剖表「文本 16/26/400，始终」+ 行高 32「始终」。
- 影响面：全屏大纲所有单行行（基线随字体内置行高走，雅黑 ≈1.17em ≈ 18.7px vs 26px）。
- 要动：`mw-shell::panels::outline`（一处）；**不动数据模型**。

**G09 无排版专项 golden，DPI 档不全。**
- 现状：门里只有 `base-ppp1.0/1.5/2.0` 三场（`scripts/x100-stage-b-gate.ps1:81-83`），
  `docs/26` §6 的矩阵要求 1.00 / **1.25** / 1.50 / 2.00；没有 CJK 混排、字重三档、多行行高专项场景。
- 合同要求：`docs/26` §6 DPI 矩阵；`docs/28` §9.3 判词第 8 条「中文：CJK 禁则、中英混排、200% DPI 下是否仍成立」。
- 影响面：验收覆盖（不是像素）。
- 要动：`scripts/` + `shots/golden/`（Lead 收口职责，本包只给名字，见 §5）。

### P2（记账 / 文档级）

**G10 画布光栅量化档 vs 编辑器精确 ppp。**
- 现状：`glyph.rs:36-43` 把 `zoom×ppp` **向上**取到 0.5/0.75/1.0/1.25/1.5/1.75/2.0/2.5/3.0/4.0
  再等比缩小采样；编辑器 epaint 按精确 ppp 光栅。125% / 175% DPI 下两边笔画浓淡不同源。
- 合同要求：`docs/07` §5「四档 ppp（1.0/1.25/1.5/2.0）物理高度线性与文字 golden 容差 0」（现状已满足
  线性与容差 0），但「编辑器与画布同浓淡」没有合同 → X22 可加一条记账（不要求改）。
- 要动（若改）：`mw-render::glyph`（档位表）；**不动数据模型**。

**G11 三处陈旧口径（文档/注释与代码不符）。**
- 现状：`crates/mw-shell/src/canvas.rs:2002-2003` 注释「行高锁到画布那一份（`字号 × 1.45`）」、
  `crates/mw-core/src/text.rs:27-30` 注释「现在小字 1.42（更松）、正文 1.36、画布大字 1.22」、
  `docs/06` 第 175 行「行高显式写成 `字号 × 1.45`」——**现码是 1.5/1.46/1.44/1.4 四档**。
- 合同要求：`AGENTS.md` §9「行号是快照…发现漂了就更新」。
- 影响面：误导下一位实现者（本轮测绘先撞上）。
- 要动：注释（`crates/`，本轮禁止）与 `docs/06`（本轮禁止）→ 记 `延后`，由 Lead 收口统一改。

**G12 caret 高度占满行盒。**
- 现状：`docs/06` 第 175 行末段自认「caret 高度占满 1.45 em 行盒，比 §7 想要的『贴着字高』明显偏长…
  排在 v5」；编辑器行高由 `geom.line_height` 决定（`canvas.rs:2004`），caret 自然继承整行盒。
- 合同要求：`docs/28` §6.3 只要求光标可见、不跳位；无 caret 高度条款。
- 要动（若做）：`mw-shell`（覆盖 `TextEdit` caret 几何）→ 记 `延后`。

**G13 逐像素对齐的边界要说清（导出）。**
- 现状：SVG 写 `<text>`（`export_svg.rs:648-660`），位置/基线/`font-weight` 与画布同源，但形状由**查看器
  自己排版**（头注第 11 行已声明「查看器缺字体时会自行回退；这是矢量导出相对位图导出唯一无法保证逐像素」）；
  PDF 是 DWrite 轮廓路径（`export_pdf.rs:1-16`），与画布同源但按 `×0.75` 换算。
- 合同要求：X22 的验收口径必须区分「**基线/行数同源**（可断言）」与「**逐像素一致**（只有 PNG 能保证）」。
- 要动：文档口径（§4 已按此写）；**不动代码**。

---

## 4. 验收场景草案（可量化，全部给数字与判据）

> 状态一律 `未开始`。每条标明「断言什么 / 用什么测 / 失败怎么算」。

| # | 场景 | 断言（可量化） | 测法 |
| --- | --- | --- | --- |
| A1 | 字阶表对齐 | 14 个内容角色 + 4 个画布令牌，逐项 `(size, line_height, weight)` 与合同表**完全相等**；失败信息带角色名 | 表驱动单测（`mw-core::design` 已有 `TYPE_SCALE` 数组可直接遍历） |
| A2 | 行高比值 | 画布每档 `line_height / font_size` ∈ 合同比值 ±0.01；内容面行高 == 合同绝对值（浮点相等） | 纯函数单测 |
| A3 | 字重面名 | 对 400/500/600 各取一次 `IDWriteFontFace::GetWeight()` + `GetFamilyNames()`，打印并断言：400→400、600→600 或 700（按 G07 拍板值）、500→（G06 拍板值）；内容面 600 只走 `zhisi-bold`、500 落哪族**写死断言** | 一次性探针 + 单测（`mw-render::dwrite` 已有 `face_by_name` 测试夹具） |
| A4 | 混排度量一致 | 样例「中文Rust混排」：`measure().width + 0.1em × n_gaps × size` == `ShapedLine.width`（±0.01）；且两者折行点一致（行数、每行首字符下标相等） | `mw-render` 单测（`cjk_latin_gaps` 已有边界用例可复用） |
| A5 | 禁则 | 26 个禁则字符逐个断言「不出现在行首/行尾」（已有 13 §4.8 覆盖，X22 复核）；**新增**：编辑器侧同一段文本折行**行数**与画布 `Measured.lines.len()` 相等 | 单测 + 编辑器 galley 行数对比 |
| A6 | 编辑态与画布基线 | 首行基线差 ≤ **0.5 css px**（现行脚本口径 0.73，X22 收严）；行距 == `geom.line_height × zoom`（±0.25 px） | `scripts/v4-edit.ps1` 口径改造（Lead 收口） |
| A7 | DPI 布局不变 | ppp 1.00 / 1.25 / 1.50 / 2.00 下，同一文档的 `Scene` 盒几何**字节相等**（DIP 不变）；只允许字形光栅差异 | 单测：同 Scene 不同 ppp 比较 + 三/四档 golden |
| A8 | 导出同源 | SVG 每个 `<text>` 的 `y` == `line_base_y`（±0.01）、`font-size` == `TextJob.font_size`、`font-weight` == `weight`；PDF 文本轮廓包围盒与 `Scene` 文本盒差 ≤ 1 DIP | `export_svg`/`export_pdf` 已有单测扩展 |
| A9 | 长文折行稳定 | 20k 文档在 100% 与 200% DPI 下，每行字符数分布**完全一致**（禁则不随 DPI 变） | 单测（`mw-core` 纯函数） |
| A10 | 字重肉眼可分 | 400/500/600 三档并排样张（中文 + 拉丁各一行），人工判词回答「三档是否都看得出差别」；若 500 落 400 面 → 记「500 无实际档位」并回填 G06 | 截图 + 判词（人工） |

---

## 5. 预计新增 / 重拍哪些 golden（**只列名字，本包不拍**）

### 5.1 新增（`未开始`）

| 场景名 | 钉住什么 |
| --- | --- |
| `type-scale` | 一屏内并排画布四级节点 + 大纲标题 + 卡片 H1–H4，钉住 18 个档位的实际字高/行高/基线 |
| `type-mix-cn-latin` | 「中文Rust混排 中文ABC 中文，标点」：钉住混排行**不溢出**节点盒 + 居中不偏 |
| `type-weights` | 400/500/600 × 中文/拉丁 六格，钉住落面（G06/G07 的视觉证据） |
| `type-outline-wrap` | 大纲同一行「单行 / 换行 / 三行」三态，钉住行高 32 与 26 行距一致（G08） |
| `type-dpi125` / `type-dpi175` | 补 `docs/26` §6 的 1.25 与 1.75/1.75 档（门里现无），钉住量化档不掉行高 |
| `type-card-body` | 卡片页正文 640 版心内的段落/引用/列表/代码，钉住 16/28 与 14/22 |

### 5.2 重拍（依赖 G02 / G03 / G04 的拍板结果，`未开始`）

- 若 A1/A2 改码：**画布全部**（节点盒高变）+ **大纲全部** + **卡片页全部** + **库全部**，并连带
  `inspector-*`（用到 `content::NAV`/`TAG` 的行）。具体张数以 `docs/STATUS.md` §1 为准。
- 若只改文档（G03 反向对齐）：零重拍，但要改 `docs/28` §4.1 的 9 行数值并记账。
- 两种走法的**旧/新哈希**按 `docs/30` §7.2 记录（重拍前先记旧哈希与漂移原因）。

---

## 6. 未做 / 风险

### 6.1 未做（本包边界）

- **G04 那一刀写完了又撤回**：`crates/mw-render/src/text.rs`、`dwrite.rs`（+ 两者自己的单测）
  曾按 Lead 批准落地并 41 条 lib 测试全绿，但引出一张集成基线的预期漂移（0.090%），
  收尾轮要求测试通过又不能重拍基线 → **整块撤回**，补丁留在
  [`evidence/g04-measure-parity-2026-11-02.patch`](evidence/g04-measure-parity-2026-11-02.patch)。
  最终工作树对 `crates/` **零净改动**；`crates/mw-core/**`、`mw-shell/**`、`hit.rs`、`docs/`、
  `scripts/`、其他 `shots/x100/*` 一律未碰，未 commit。
- **一张未拍**：没有新增/重拍任何基线，没有跑 `cargo golden`、没有登记门场次。
  已知需重拍 1 张：`shots/golden/canvas-text-ppp1.00.png`（G04 的预期漂移 484 px / 0.090%，
  见 [`evidence/g04-measure-parity-2026-11-02.md`](evidence/g04-measure-parity-2026-11-02.md) §3.2）。
- **clippy 未跑成**：并行会话在改 `mw-core`（`design.rs:891` 报 `cannot find type Rgba`），
  `cargo clippy -p mw-render --all-targets` 与最后一次 `cargo test -p mw-render` 均被挡在
  `mw-core` 编译处 → 本包 clippy 记 `未开始`，测试记「20:2x 那趟：lib 41 绿 / 集成 8 绿 1 预期红」。
- **未实测的**：DWrite 对 500 的实际选面（G06）、雅黑 600 实际落 700 面（G07 的依据是字体里只有
  400/700 两面这一事实 + DWrite 最近面规则，**没有一次 GetWeight 读数**）。两项都要等动码期探针。
- **未改的文档**：G11 的三处陈旧口径（`canvas.rs` 注释、`text.rs` 注释、`docs/06` §7 末段）本包无权动，
  记 `延后`。

### 6.2 风险

1. **G03 是分叉点**：按 `docs/28` 改码 → 9 个内容角色变（字号/行高/字重），卡片页、库、大纲全量重拍；
   按代码改文档 → 零重拍但 `docs/28` 是设计合同，等于把设计降级成「实现现状」。**需 Lead 拍板**。
2. **G04 修完会改盒宽**：度量补上 0.1em 后，所有混排节点盒变宽 → 画布基线再次全量漂；建议与 G02
   合并成一次「度量收口」，只重拍一次。
3. **G05 的深水区**：若要求编辑器与画布**折行完全一致**，等于把 DWrite 的分行结果喂给 epaint
   （需要 `LayoutJob` 手工插入硬换行），会牵动光标映射/IME `set_ime_cursor_area`（B2 与 M2a 的核心地带）。
   建议 X22 只合同化「行高 + 基线 + 行数一致」，把「折行点一致」列到 X33（IME / 编辑）再动。
4. **G07 涉及资源**：真 600 中文需要变量字体或捆绑字体，与「不引入依赖 / 不捆绑字体」的既有决定冲突，
   需要用户级拍板；本包只登记。
5. **不要只写绿**：§4 的 A1–A3 一旦改码落地，必须给出旧/新哈希、双跑 `--tol 0`、以及 `docs/STATUS.md`
   的状态（`已实现`／`延后`），本报告不预写任何结论。
6. **G04 的收尾不在本包**：`mw-render` 那 1 张集成 golden 要重拍（旧/新哈希按 `docs/30` §7.2 记）；
   `mw-core` 编译恢复后要复跑 `cargo test -p mw-render` 与 `cargo clippy -p mw-render --all-targets`。
   交接清单见 evidence 文件 §6。
7. **并行会话的坑**：`crates/mw-core/src/design.rs:891` 的 `Rgba` 编译错误不是本包引入的
   （本包禁改 `mw-core`），但它会让所有下游 crate 的测试/门暂时红——收口方先确认那一处已修再判绿。

---

## 7. 画布字阶表（待进合同）

### 7.0 本轮改动计划与最终去向（先写后改；Lead 2026-11-02 裁决 §1：G04 是唯一批准改码的一条）

> **最终去向（2026-11-02 20:40 收尾轮）**：下面 1–4 已按计划写完并跑过（`mw-render` lib
> **41 条全绿**，含 5 条新单测），但修完后 `shots/golden/canvas-text-ppp1.00.png` 出现**预期漂移
> 484/540000 px = 0.090%**（只在那颗混排节点上，见
> [`evidence/g04-measure-parity-2026-11-02.md`](evidence/g04-measure-parity-2026-11-02.md) §3.1）。
> 收尾轮硬要求是「`cargo test -p mw-render` 通过」且禁止跑 golden 重拍 → **按「宁可少做一件、
> 不留半截」把代码整块撤回**，工作树对 `crates/` 零净改动；完整补丁留在
> [`evidence/g04-measure-parity-2026-11-02.patch`](evidence/g04-measure-parity-2026-11-02.patch)
> （`git apply --check` 已过）。重放 3 步见 evidence §6。所以本轮 **G04 状态：`进行中`（撤回）**。

1. `mw-render::text` 新增**唯一出宽函数** `line_advance(dwrite_advance, text, font_size)`
   （= DWrite 行进 + `cjk_latin_gap_width`），渲染侧 `shape_uncached` 的 `width` 只由它给出；
   逐字形 `gap_before` 仍负责落点，不再自己累加宽度。
2. `mw-render::dwrite::width_of` 改成「每行 DWrite 行宽 → `text::line_advance`」，
   于是 `Measured.width`（盒宽/折行依据）与 `ShapedLine.width`（画面对齐依据）**同源**。
3. 单测钉等式：纯函数等价式 + 一条 DWrite 实测（`measure()` 与 `shape()` 的宽差 ≤ 0.5 css px）；
   再用纯函数复算「纯中文 / 纯英文零漂移、只有含中英混排的节点变宽」，作为像素影响面论证。
4. 不动 `mw-core`、不动字形图集、不动折行策略：断行点仍由 DWrite 定；改后盒宽只会加宽到
   「装得下间距」，不会再出现混排文字顶出节点盒。

### 7.1 画布字阶表（四级 × 几何 × 四档 DPI）

本表**从代码复算**，不凭印象；复算口径与逐项出处见
[`evidence/g04-measure-parity-2026-11-02.md`](evidence/g04-measure-parity-2026-11-02.md) §3。

- **行高** = `mw_core::text::line_height(字号)` = `round(字号 × 倍数)`（`design.rs:58-69` 四档倍数 +
  `text.rs:31-33` 的 `.round()`）。
- **行内基线偏移** = `行高 × 0.72`（`layout.rs:829` `pub const BASELINE_RATIO: f32 = 0.72;`），
  即行盒顶到该行基线的距离。
- **单行盒高** = `max(1 × 行高 + 2 × pad_y, 32)`（`scene.rs:661-663` `fn text_height` 的
  `.max(32.0)` + `design.rs:690-700` `card_padding`），取「单行、无图、无标签胶囊」。
- **盒顶→基线** = `(单行盒高 − 行高) / 2 + 行内基线偏移`（`layout.rs:849-850`
  `let start = … / 2.0;` 与 `+ line_height × BASELINE_RATIO` 两行）；多行第 i 行再加 `i × 行高`。
- **物理 px 三连** = `字号 / 行盒 / 盒顶→基线` 各乘 ppp。100 / 125 / 150 / 200 四档 ppp 恰好都是
  字形图集的精确档（`glyph.rs:31` `PPD_BUCKETS` 含 1.0 / 1.25 / 1.5 / 2.0），zoom = 1 时零量化；
  zoom ≠ 1 时 `ppd = quantize_ppd(zoom × ppp)` **向上**取档（`text.rs:240`），字形 em = `字号 × ppd`
  （`glyph.rs` `let em = font_size * ppd;`）。

| 层级 | 字号(DIP) | 默认字重 | 行高(DIP) | 行内基线偏移 | 单行盒高 | 盒顶→基线 | @100% 字号/行盒/基线 | @125% | @150% | @200% | 出处 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 中心主题 | 28 | 600 | 39（`round(28×1.4)` = 39.2） | 28.08 | 63 | 40.08 | 28 / 39 / 40.08 | 35 / 48.75 / 50.10 | 42 / 58.5 / 60.12 | 56 / 78 / 80.16 | `theme.rs:265` 第 1 项 `(CANVAS_CENTER, true)`；`design.rs:48` `pub const CANVAS_CENTER: f32 = 28.0;` |
| 一级主题 | 20 | 400 | 29（`round(20×1.44)` = 28.8） | 20.88 | 49 | 30.88 | 20 / 29 / 30.88 | 25 / 36.25 / 38.60 | 30 / 43.5 / 46.32 | 40 / 58 / 61.76 | `theme.rs:265` 第 2 项 `(CANVAS_MAIN, false)`；`design.rs:46` `pub const CANVAS_MAIN: f32 = 20.0;` |
| 二级及更深 | 16 | 400 | 23（`round(16×1.44)` = 23.04） | 16.56 | 39 | 24.56 | 16 / 23 / 24.56 | 20 / 28.75 / 30.70 | 24 / 34.5 / 36.84 | 32 / 46 / 49.12 | `theme.rs:265` 第 3 项 `(CANVAS_SUB, false)`；`design.rs:39` `pub const CANVAS_SUB: f32 = 16.0;` |
| 最深层 | 15 | 400 | 22（`round(15×1.46)` = 21.9） | 15.84 | 36 | 22.84 | 15 / 22 / 22.84 | 18.75 / 27.5 / 28.55 | 22.5 / 33 / 34.26 | 30 / 44 / 45.68 | `theme.rs:265` 第 4 项 `(CANVAS_LEAF, false)`；`design.rs:44` `pub const CANVAS_LEAF: f32 = 15.0;` |

**覆写情形（检查器 SE-13 可改字号 12–96 与字重 400/500/600）**：同一套公式照套，但倍数按覆盖后的
字号重新取档，所以跨档处行高会跳 1–2 px——例：21px → `round(21×1.44)=30`，22px → `round(22×1.4)=31`。
这条写进合同以免实现方以为「行高连续」。

### 7.2 字重档 → 实际落面（`GetFirstMatchingFont` + `GetWeight()` 实测读数）

探针：`crates/mw-render/src/dwrite.rs` 测试 `weight_faces_are_read_back_from_dwrite`
（一次性，不改映射），原始输出见
[`evidence/g04-measure-parity-2026-11-02.md`](evidence/g04-measure-parity-2026-11-02.md) §4。
读数（本机 2026-11-02，`windows` crate 的 `IDWriteFont::GetWeight`）：

| 请求字重 | Segoe UI（拉丁） | Microsoft YaHei UI / YaHei（中文） | 画布实际视觉 | 内容面（egui 族）实际视觉 | 出处 |
| --- | --- | --- | --- | --- | --- |
| 400 | **400** Regular | **400** Regular | 常规 / 常规 | `Proportional` = [Segoe 400, 雅黑 400] → 常规 / 常规 | `dwrite.rs` 探针；`fonts.rs:52-60` |
| 500 | **600** Semibold | **400** Regular | 拉丁半粗 + 中文常规（**非对称**） | `zhisi-semibold` = [Segoe Semibold 600, 雅黑 400] → 与画布同 | `dwrite.rs` 探针；`fonts.rs:24-25`、`fonts.rs:88-97` |
| 600 | **600** Semibold | **700** Bold | 拉丁半粗 + 中文粗体 | `zhisi-bold` = [Segoe Semibold 600, 雅黑 Bold 700, 雅黑 400] → 与画布同 | `dwrite.rs` 探针；`fonts.rs:74-86` |
| 700（仅钳制上界，检查器不提供） | **700** Bold | **700** Bold | 全粗 | 内容面不走 700（`content_kit.rs:47-51` 只映射 400/500/600） | `dwrite.rs` 探针；`theme.rs:1415-1419` `.clamp(WEIGHT_REGULAR, 700)` |

**本节完成度**：表内 4 行 × 6 列**全部填实**，无「需探针」格（探针只覆盖汉字拉丁两族；
`docs/28` 需要的话可再加 `SimSun`/`Noto Sans SC` 两列，属下一棒）。原计划标「需探针」的三格——
500 档落面、CJK 600 落面、700 是否被用到——现由实测值替代。

**两条由读数改判的结论**（原 `report.md` §3 的推断）：
1. **画布与内容面在 400/500/600 上落面完全一致**——因为两边都不含真 500，最终都靠
   「最近面」落到同一对字面（拉丁 Semibold 600 / 中文 Regular 400，或拉丁 Semibold 600 / 中文 Bold 700）。
   因此 G06 中「两侧落面不同」这半条**不成立**，删去；剩下的真问题只有「500 不是独立档」。
2. **CJK 600 = 雅黑 700 面**：实测 `请求 600 → 实际 700`，G07 由「间接依据」升级为**实测确认**
   （`docs/28` §4.1 第 179 行「不用 700」与实现冲突，但 700 是雅黑族里唯一的加粗面，不是合成粗体）。

### 7.3 与内容面不是同一把尺（写进合同时必须连带声明）

- 画布行高是**倍数取整**（39 / 29 / 23 / 22，比值 1.393 / 1.450 / 1.4375 / 1.467）；
  内容面行高是**每角色绝对值**（26 / 28 / 22 …，比值 1.63 / 1.75 / 1.52 …）。
  同一句 16px 文字：画布行盒 23，大纲行 26，卡片正文 28。
- 因此「统一画布与编辑器度量」在 X22 的可达口径是：**同一表面内**同源（画布度量 = 画布字形、
  编辑器行高 = 画布行高 × zoom、卡片页吃 `content` 表），**跨表面**只保证字号令牌与基线公式一致，
  不保证行盒高相等。
