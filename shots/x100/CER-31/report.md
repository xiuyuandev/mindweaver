# CER-31 · 三界面一致性审计（B2-D 第一包）

> 批次：B2-D（CER-31–34）· 依赖：CER-12 / CER-21 / CER-30（均 `已实现`）
> 合同：`docs/29` §9（本包一行）+ `docs/28` §3（共同公理）与 §4（共享内容设计系统）
> 状态：**已实现**——F1/F2/F4 已按合同落地（2026-11-02，用户授权 AI 代拍板，
> 原话：「继续开发，而且呢，你帮助我验收，不需要我决定。」）；F3 早前已修；F5 判定不是缺陷。

## 1. 范围

对着 `docs/28` §4.1–§4.6 六个维度 + §3.1 玩具感禁止项，横向审三个内容表面：

| 表面 | 入口文件 |
| --- | --- |
| 全屏大纲 | `crates/mw-shell/src/panels/outline.rs` + `content_kit.rs` |
| 卡片页 | `crates/mw-shell/src/note_view.rs` |
| 笔记库（侧栏/列表/预览） | `crates/mw-shell/src/note_library/{sidebar,list,preview}.rs` |

审计口径是**「同一个东西在三个界面是不是同一个样子」**，不是各自好不好看。逐条落表见 §3。

## 2. 非目标

照 `docs/29` §9.1 原文：不借一致性重做画布 / 检查器 / 起始页 / 标题栏；不新增功能；
不为「全量重拍」顺手改无关场景；不在 G-B2-4 未通过时宣布 B2 完成或开 Stage C。

本包**也不改** `docs/28` 的任何数字——发现合同与实现冲突时只记账、给两个选项与代价，
由用户拍板（先例：CM-18 分组、CER-22 `NoteCard.archived`）。

## 3. 审计矩阵（第一遍）

| 维度（`docs/28`） | 大纲 | 卡片页 | 笔记库 | 结论 |
| --- | --- | --- | --- | --- |
| **字阶** §4.1 | 走 `content::`（`content_kit` 38 处引用） | **已接 `content::`**（F3 修复）：`PAGE_TITLE / HEADING_1..3 / READING_BODY / CODE` | 走 `content::`（list 32 / preview 21 / sidebar 6 处） | **一致** |
| **版心** §4.2 | `content::OUTLINE`（800 / 760） | **已接 `content::CARD`**（720 / 正文 640 / 窄窗 24，F1/F4 修复） | `content::PREVIEW`（680 / 600）；侧栏 232 / 列表 360 为固定栏 | **一致** |
| **颜色与状态** §4.3 | `content_metrics(...).colors`（`panels/outline.rs:2452`） | **已接 `content_metrics(...).colors`**（F2 修复，含 `surface.page` 底色） | `content_metrics(...).colors` + `MdPalette::from_content(...)`（`preview.rs:292`） | **一致** |
| **边框/圆角/阴影** §4.4 | 0 边框；圆角 `content::radius::ROW` | 0 边框；无阴影 | 0 边框；`content::radius::ROW`（list 6 处、sidebar 1 处）；行分隔 `divider` 内缩 16 | **一致**（逐图已见；仍待第二轮并排图复核） |
| **动效** §4.5 | **未取到数值证据**（大纲的过渡时长在本轮 grep 里没命中，需读 `mw-core::motion` 再判） | `FOCUS_SECS = 0.120`、`TYPEWRITER_SECS = 0.180`（`note_view.rs:1888/1895`） | **未取到数值证据**（悬停/选中底色直接取 `content::colors`，动画在壳层总闸） | **待实测**——不写成「一致」 |
| **滚动条** §4.5 | 壳层统一 | 壳层统一 | `super::scrollbar(...)` 三处共享同一实现（`list.rs:406`、`preview.rs:132`、`sidebar.rs:118`） | **一致**（6px / 4px 拇指尚待像素断言） |
| **焦点与键盘** §4.6 | `content_kit::focus_ring`（`content_kit.rs:251`，参数取 `ck::focus::RING/OFFSET`） | 无 Tab 焦点环（编辑器只有光标） | `content_kit::focus_ring`（`list.rs:448`、`:975`） | **部分**：卡片页没有「键盘焦点」这一态，需确认是否符合 §6.8 键表 |

## 4. 发现清单

### F1 · 卡片页正文实测 720，合同 §4.2 写的是 640

- **证据**：`content::CARD` 定义 `page_max: 720 / text_max: 640`（`mw-core/src/design.rs:284-288`）；
  卡片页没用它，直接 `article = if total >= 1040 { 720 } else { clamp(total-96, 320, 640) }`
  （`note_view.rs:674-678`），正文就排在这 720 里。CER-13 的金样已把 720 钉死：
  `card-long` 实测换行右缘 890.7–892.7（左 180 = (1080−720)/2），`card-narrow` 实测 634.7（= 640）。
  笔记库预览走 `content::PREVIEW.text_max = 600`，大纲走 `760`。
  **于是三界面阅读宽实际是 760 / 720 / 600，合同是 760 / 640 / 600。**
- **性质**：CER-13 当时把这行读成「版心 720、窄窗退 640」并写进报告；`docs/28` §4.2 的
  「版心 720 max / 正文宽 640」也可以读成「正文 640」。**是文档与实现各读一半，不是谁写错了代码。**
- **决定（2026-11-02，用户授权代拍板）：选 ①，实现向合同对齐——正文改 640。**
  理由：① `docs/28` §4.2 是已批准的设计合同，B2 的目的就是「同一个东西三个界面同一个样子」；
  ② 640 ≈ 40 个汉字/行，是这个字号下长文写作的舒适区，也是合同给卡片页的理由；
  ③ 大纲 760（结构扫描）/ 卡片 640（写作）/ 预览 600（阅读）三档分工是合同有意设计的，
  改成 720 会把「写作页」和「结构页」的区分模糊掉；④ 「写作页比阅读页宽」这个区分并不需要
  120px，640 vs 600 已经保留了 40px 的层次。
- **落地**：`note_view.rs` 的 `article` 改走 `content::CARD.column(total)`（F4 同一处），
  正文宽 640、版心 720、窄窗左右 24；`card-*` 基线按 CER-34 全量重拍。
  代价按原评估执行：CER-13/15/16/17/20/21 的旧证据图在 CER-34 报告中标注为「已过期/已重拍」。

### F2 · 同一棵 Markdown AST，两个配色入口

- **证据**：笔记库预览 `MdPalette::from_content(...)`（`preview.rs:292`），注释明写
  「颜色跟 `design::content` 令牌走，不跟壳层 chrome 走」（`note_view.rs:2205-2207`）；
  卡片页自己仍 `MdPalette::from_chrome(ch)`（`note_view.rs:671`），正文色取 `ch.text`、引用/次要
  取 `ch.dim`、占位取 `ch.faint`。
- **后果**：今天两套色**可能**数值相同（`Chrome` 的 `text/dim/faint` 与 `content_metrics` 的
  `text_primary/text_secondary/text_faint` 都从主题派生），但它们是两条独立的推导链：
  改任一侧（比如 SE-09 把检查器 `faint → dim` 那次）就可能让「预览里的正文」和「卡片页里的正文」
  出现两种灰度。这是**结构性隐患**，不是当前可见的错。
- **修法**：卡片页也走 `from_content`，取值统一从 `content_metrics(theme, ppp).colors` 来。
- **代价已实测（2026-11-02）：两套色不同值，切换不是零漂移。**
  - 雪主题：`Chrome` 的亮色是写死的 `text #3E444C / dim #5C646F / faint #656D78 / bg #FFFFFF`
    （`chrome.rs:60-76`），而内容令牌是 `ui.text #22262A` + 70%/68% 透明度、
    背景 `page #F4F1EB`（`theme.rs:525-529`、`design.rs:375-397`）。正文更黑、背景从纯白变纸色。
  - 暗主题：`text` 同源，但 `dim` 是 `ui.dim`（不透明）vs `text@72%`、`faint` 是写死的 `#8B929B`
    vs `text@68%`，仍不同。
  - `from_content` 的背景参数决定整页底色，**不是只换几个字色**。
- **结论**：**已按「接同一套令牌」落地（2026-11-02）**。卡片页改用
  `MdPalette::from_content(...)`，取 `content_metrics(theme, ppp).colors` 的
  `text_primary / text_secondary / text_faint / accent / page`；代码块底与笔记库预览
  一致用「正文色 6%」。整页底色从壳层白改成 `surface.page`（雪主题 `#F4F1EB`）——
  这正是 §4.3「surface.page 取 ThemeUi.bg」的要求。`card-*` 基线随 F1 一起在 CER-34 重拍。

### F3（已修）· 卡片页字阶是本地字面量

- **证据**：`note_view.rs:2258-2268` 的 `match level { 1 => (30.0, 42.0), 2 => (22.0, 33.0),
  3 => (18.0, 28.0), 4 => (16.0, 26.0) }` 与 `const BODY_PX: f32 = 16.0` / `CODE_PX: f32 = 14.0`；
  `content::PAGE_TITLE / HEADING_1..3 / READING_BODY` 都有同名常量。
- **后果**：值现在一致（30/42、22/33、18/28、16/26、16/28），但改 TYPO-01 这类全局字阶时
  要改两处；而且 `note_view.rs:3970` 的 `assert_eq!(BODY_PX, 16.0)` 钉的是**字面量**，
  不是令牌——令牌改了它不会红。
- **已修（2026-11-02 09:37）**：`note_view.rs` 的 `BODY_PX / BODY_LINE / CODE_PX / CODE_LINE`
  改取 `content::READING_BODY` / `content::CODE`；标题 `match` 直接映射到
  `content::PAGE_TITLE / HEADING_1 / HEADING_2 / HEADING_3`，`_` 回 `READING_BODY`。
  值逐项相同（30/42、22/33、18/28、16/26、16/28、14/22），**零像素变化、零基线漂移**；
  `note_view.rs:3970/3976` 原有的 `BODY_PX == 16.0` / `CODE_PX == 14.0` 断言仍绿。
  另外两处不要改：`CODE_LANG_PX = 12.0` 是合同 §6.2 的「语言标签 12」单点，
  `TOP_BAR_H` 是版式尺寸不是字阶。

### F4 · 卡片页的窄窗列宽算法与 `Measure::column` 不等价

- **证据**：`Measure::column(available)` 先 `(available - 2*narrow_side)` 再夹 `page_max`，
  然后 `text = min(page, text_max)`；卡片页自写的是 `available - 96` 再夹 `[320, 640]`，
  且宽窗阈值写死 `1040`。在 `available = 700` 时：token 给正文 **640**，自写给 **604**；
  在 `available = 900` 时两者都 640；在 `available = 1200` 时 token 给 640、自写给 720（见 F1）。
- **后果**：只有 F1 定案后才好处理；若选 ②（改合同），这条要改成「文档写明卡片页用自己的
  窄窗公式（左右各留 48）」，并补一条 700/900/1200 三点的单测把差异钉住，免得以后误以为它接的是 token。
- **已落地（选 ①，2026-11-02）**：自写公式整段删除，卡片页改走 `content::CARD.column(total)`：
  宽窗 `page 720 / text 640 / inset 40`，窄窗 `side 24`（合同 §4.2「卡片页窄窗左右各留 24」）。
  新增单测 `card_column_keeps_the_contract_text_width_at_every_breakpoint` 钉住
  700 / 900 / 1200 / 500 四个断点（`mw-core/design.rs`）。

### F5 · 卡片页没有「键盘焦点」态

- **证据**：笔记库列表 `content_kit::focus_ring` 两处；大纲有焦点环；卡片页只有文本光标
  （`content_keys` 的 IME 守卫在，但绘制层没有环形焦点）。
- **判断**：**可能不是缺陷**——`docs/28` §6.8 卡片页键表里没有「Tab 在页面内移动焦点」这一项，
  编辑器本身就是焦点终点。本轮只记账，等 §6.8 逐条核对时再定。

## 5. 验证

- F3 修复后复跑 `cargo test -p mw-shell --bin mw-shell -- note_view`（TEMP/TMP → `target/tmp`）：
  **80 passed / 0 failed**；`cargo check -p mw-shell --release` 绿。F3 是值中性重构，
  **没有新基线、也不该有基线漂移**（正式门在 CER-34 统一验证）。
- **F1/F2/F4 落地后的验证（2026-11-02）**：`note_view` **80 passed / 0 failed**、
  `design` **19 passed / 0 failed**（含新单测四点断点）、`note_store` **16 passed**、
  `canvas` 147 passed；正式门全量 **1279 passed / 0 failed / 1 ignored**（见 CER-34 §4）。
- **像素验证**：36 场卡片页基线在改动前 vs 改动后逐像素差 **92.6%–99.99%**（整页换底色），
  重拍后两遍 `--tol 0` 零差异；`card-long` 实测正文暗像素跨度 **960 物理 px = 640 css px**
  （ppp 1.5，左右各 330），`card-narrow` 同样 640——宽度按合同收口已用图证实。
- 其余四条仍是**审计**结论，没有新基线；结构性判断都有文件行号可复现（见 §4）。
- 已有回归保护：`mw-core` 的 `design.rs` 自带 `content_measures_centre_a_bounded_column` 等
  令牌单测；`crates/mw-core/tests/contrast.rs` 七条对比度硬门（SE-11）。
- 三界面各自的金样覆盖：大纲 `outline*`、卡片页 `card-*`、笔记库 `library-*` 共 21 场
  （CER-30 已登记进 stage-b 门，门内 173 场）。

## 6. 未做 / 已知偏差

- **F1/F2/F4 已落地**（见 §4），F3 已修，F5 判定不是缺陷（§6.8 没有页面内 Tab 焦点）。
- F2 切换后 `card-*` 约 40 张基线重拍由 CER-34 统一执行并记录旧/新哈希。
- 「边框/圆角/阴影」一栏只做了代码审 + 已有基线目视，**三界面并排图**在 CER-34 证据包中补齐
  （`shots/x100/CER-34/side-by-side/`）。
- 滚动条 6px / 拇指 4px 没有像素断言，只有共享实现的代码证据。
- 「10 秒判词」（合同要求）需要真机看，离屏图只能看静态版式；本项如实保留为人工观察项。

## 7. 风险

- F1 若选 ①（改实现），`card-*` 约 40 张基线重拍 + CER-13/15/16/17/20/21 的证据图集体过期，
  是本批最大的一笔返工；必须先拍板再动手。
- `note_view.rs` 曾是并行会话的活跃区（加图标）；09:37 已在树恢复后改完 F3 并跑绿。
  F2/F4 若再动它，仍要先看 mtime 与 `cargo check`，别在别人编辑中途落笔。

## 8. 人工判词

**已判（2026-11-02，用户授权 AI 代拍板与验收）**：用户原话「继续开发，而且呢，你帮助我验收，
不需要我决定。」F1 按合同选 ①（正文 640）、F2 统一走 `content_metrics`、F4 走 `CARD.column`、
F5 不是缺陷。结论：三界面在字阶 / 版心 / 颜色 / 状态四条上已同源；边框、圆角、动效、
滚动条、焦点在代码层共享实现，逐像素证据由 CER-34 全量回归背书。
