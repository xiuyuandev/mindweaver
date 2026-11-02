# SE-10 · 检查器证据包与 G-B3-2（P 轨 E2 收口）

> 设计合同：`docs/30` §4 全节（检查器 v2）。执行合同：`docs/31` §5 第 SE-10 行 + §1.3 的 **G-B3-2** 五条判词。
> 依赖：SE-05 … SE-09 全部 `已实现`。本包**不改功能**，只把前五包的证据收成一份能过人工门的东西。

## 1. 本包做什么

1. 把 `docs/30` §4.1–§4.6 逐条对回代码/基线，写成一张「合同 → 落地 → 证据」总表（§3）。
2. 盘一遍 `docs/30` §4.2 的**必选能力清单**（14 项），如实标出 `已实现 / 部分 / 延后`（§4）——
   这一条直接决定 G-B3-2 的第 4 条判词能不能过，不许含糊。
3. 列检查器的**全部基线**（本轮 10 张）与它们各自锁住什么（§5）。
4. 量化 v1 → v2 的漂移（SE-05 起到本轮，每一包都记了数；汇总成一段，§6）。
5. 写 `gate.md`：G-B3-2 五条判词 + 每条看什么 + 证据入口，**判词留空给人**（不代写结论）。
6. 写 `manifest.json`，把「6 场景、脚本、前后对比、逐条核对、阶段报告章节」逐项对上。

## 2. 检查器的十个基线（全部在 stage-b 门内，`--tol 0` 双跑）

| 场景 | 钉住什么 | 来自 |
| --- | --- | --- |
| `inspector` | 样式页默认态：面板 288、头行 40、对象行 44、分段 30 无描边、颜色组展开（色板 8 列 × 24） | SE-05 / SE-06 |
| `inspector-narrow` | 面板拖到 240：色板降到 7 列、形状四段仍放得下（内容最小宽不再顶开面板） | SE-05 / SE-06 |
| `inspector-empty` | 无选中：一句话 + 「选中第一个主题」，文档/主题页（不显示属性控件） | SE-05 |
| `inspector-shape` | 「形状与文字」展开、颜色收起（边框色板 / 边宽 stepper / 形状四段 / 字号 stepper） | SE-06 |
| `inspector-ink` / `-mist` / `-sand` | 同一页的三套主题 | SE-06 |
| `inspector-content` | 内容页：五组默认展开、备注 3 行起、标签 6px 色点、任务三态、链接 host、卡片标题 + 来源 | SE-07 |
| `inspector-multiselect` | 三颗一起选中：对象行「3 个节点」、样式组标题右侧「本节点 / 含子树」 | SE-08 |
| `inspector-error` | 命令失败：面板内 28px 错误条（warn 8% 底 + 警示点 + 关闭）+ 底部 toast | SE-09 |
| `cm-arrow-inspector` | 概念图关系线的检查器（CM 轨的基线，检查器每次改动都会波及它） | CM-04 |

## 3. `docs/30` §4 逐条核对（合同 → 落地 → 证据）

| 合同条目 | 落地 | 证据 |
| --- | --- | --- |
| §4.1 面板宽 288、可拖 240–360、记忆、1px hairline | `PANEL_W_MIN/MAX`、`clamp_width`、落盘 `window_state.inspector_width`、右侧 vline | SE-05 报告；`inspector` / `inspector-narrow` |
| §4.1 头行 40（12/16 + 24 关闭） | `geo::HEAD_H`、关闭按钮 24、`F9 / Ctrl+Shift+I` | 同上 + SE-09 |
| §4.1 对象行 44（13.5/20 + 11.5/16；多选「N 个节点」） | `geo::OBJ_H`、`ellipsize`、`object_meta` | `inspector` / `inspector-multiselect` |
| §4.1 分段 30、8% 强调底 + 1.5px 条、无描边胶囊 | `geo::SEG_H` / `SEG_BAR`、`segment()` | `inspector` |
| §4.1 分组标题 28（chevron + 名称 + 右摘要）、组间 8px hairline | `geo::GROUP_H` / `GROUP_GAP`、`group_header` | `inspector` |
| §4.1 组内 上下 8 / 左右 12、控件行高 28、行间距 6 | 左右 12、行高 28 已做（`content_row` 钉死）；**行间距仍是全局 8**，记缺口 | SE-07 报告 §7 |
| §4.1 默认展开：样式三组 + 内容五组，标记组折叠 | `group_open` 默认值 + `window_state.inspector_groups` 记忆 | `inspector` / `inspector-content` |
| §4.2 色板 8 列 × 24、列距 8、选中 2px 环 + 1px 内白、第 9 格自定义、无颜色斜杠格 | `swatch_grid` / `Cell::{Color,None,Custom}` / `dashed_circle` | SE-06 报告；`inspector` / `inspector-shape` |
| §4.2 stepper 24×72、字号 12–96、边宽 0–8、圆角 0–24 | 字号与边宽已做；**圆角没有字段**，记 `延后` | SE-06 报告 §5 |
| §4.2 文本对齐三段（图标） | **`StyleOverride` 无 align 字段**，记 `延后` | 同上 |
| §4.2 开关 28 高行 + 32×18 轨道 | `ui_kit::switch`（标记页图标、文字样式应用到子树） | `inspector` |
| §4.2 文本域：3 行起、超过 8 行可展开、**无白底无框 + hairline 底线** | `note_rows` + `Frame::NONE` + `hline` | SE-07 报告；`inspector-content` |
| §4.2 标签：纯文字 12.5/18 + 6px 色点、回车提交、无 chip 墙 | `labels_section` | 同上 |
| §4.2 卡片绑定三态 + 标题 + 来源 | `card_section` + `card_source` | 同上 |
| §4.2 作用域：分组标题右侧两段「本节点 / 含子树」11.5/16 | `group_scoped` + `group_header(scope)` | SE-08 报告；`inspector-multiselect` |
| §4.3 全部走 `Command`、一次操作一条撤销、500ms 合并 | `style_cmd` → `Batch`；`style_gesture_key` + `apply_command_coalesced` | SE-08 报告 |
| §4.3 改动即时反映、无「应用」按钮 | `apply_command` 当帧重建 `Scene` | 全部基线 |
| §4.3 无选中只显示空状态 | `doc_body` + `inspector-empty` | SE-05 |
| §4.3 命令失败：面板内 28px 错误条 + toast | `Inspector.error` + `error_bar` | SE-09 报告；`inspector-error` |
| §4.4 `Ctrl+Shift+I`、Tab 进分组、↑↓、Enter、数字键 1–9、Esc 两级 | SE-09 | SE-09 报告；快捷键单测 |
| §4.5 禁止项（420 宽、描边胶囊 tab、三处「自定义…」、应用/重置/确定、彩色 chip 墙、图标字形按钮、画布级操作） | 均不出现 | 十张基线人工复核 |
| §4.6 六个场景 | 实际十张（见 §2） | stage-b 门 |

## 4. `docs/30` §4.2 必选能力清单（14 项）

| # | 能力 | 状态 | 说明 |
| --- | --- | --- | --- |
| 1 | 颜色（填充 / 分支线 / 文字） | 已实现 | 三块色板 + 自定义 + 无颜色 |
| 2 | 字号 | 已实现 | stepper 12–96，步 2 |
| 3 | 字重 400/500/600 | 已实现（SE-13，2026-11-02） | `StyleOverride.weight` + 三段控件；`bold` 降为 600 的别名 |
| 4 | 文本对齐 | 已实现（SE-13，2026-11-02） | `StyleOverride.align` + 「左/中/右」三段；SVG `text-anchor` 同源 |
| 5 | 边框宽度 | 已实现 | 0–8 |
| 6 | 圆角 | 已实现（SE-13，2026-11-02） | `StyleOverride.radius` 0–24 + stepper；只对有角形状生效 |
| 7 | 形状（矩形 / 圆角矩形 / 胶囊 / 下划线 / 椭圆） | 已实现（SE-13，2026-11-02） | 形状行四段「圆角/直角/胶囊/椭圆」；下划线改 B/I 同排的 U 开关 |
| 8 | 节点图标 | 已实现 | 标记页图标格 |
| 9 | 节点备注 | 已实现 | 3 行起 / 8 行封顶 / 展开 |
| 10 | 节点任务三态 | 已实现 | 未开始 / 进行中 / 完成 + 清除 |
| 11 | 节点链接 URL | 已实现 | 行内编辑 + host 预览 + 保存/打开/清除 |
| 12 | 节点标签 | 已实现 | 6px 色点 + 纯文字，上限 5 |
| 13 | 节点 ↔ 卡片绑定 | 已实现 | 三态 + 标题 + 来源 |
| 14 | 子树批量作用域 | 已实现 | 分组标题右侧两段 + 500ms 合并 |

**结论（2026-10-30 当时）**：14 项里 **10 项已实现、1 项部分（形状缺椭圆）、3 项延后（字重 / 对齐 / 圆角）**。
三项延后都要在 `StyleOverride`（`Node.style`）里加字段——按 `docs/27`/`AGENTS.md` 属于数据模型变更，
需要单独批准。**G-B3-2 第 4 条判词「能力清单齐全」因此必须由人判**：是接受「缺三项、且有方案」，
还是先补字段再开下一批。

> **补记（2026-11-02，SE-13 追补）**：上表第 3、4、6、7 项已全部落地——
> `StyleOverride` 新增 `weight` / `align` / `radius` 三个可选键（`docs/32` §15 记例外，
> `docs/05` §2.3 同步），形状行补「椭圆」、下划线搬进 B/I 同排的 U 开关。
> **14 项现在全部 `已实现`**；证据、单测与 15 场基线见
> [`shots/x100/SE-13/report.md`](../SE-13/report.md)，G-B3-2 第 4 条据此可复判。

## 5. 场景怎么点起来：旋钮代替输入脚本

E2 的十个场景**不用指针/键盘脚本**，改用确定性旋钮——脚本靠坐标与帧序，检查器一改布局就得重对坐标；
旋钮只表达意图，布局怎么变都对得上：

| 旋钮 | 作用 | 用在哪 |
| --- | --- | --- |
| `MW_INSPECTOR=1` | 启动即打开检查器 | 全部十个场景 |
| `MW_INSPECTOR_W=240` | 面板宽 | `inspector-narrow` |
| `MW_INSPECTOR_GROUPS=style/color=0,style/shape=1` | 预置分组开合（**只写内存不落盘**） | `inspector-shape` |
| `MW_INSPECTOR_TAB=note` | 直接开在内容页 | `inspector-content` |
| `MW_CONTENT_DEMO=1` | 五项内容走 `Command` 灌进演示节点 | `inspector-content` |
| `MW_SELECT_DEMO=multi` | 选中「纯 Rust 原生」+ 父级 + 第一个子级 | `inspector-multiselect` |
| `MW_INSPECTOR_FAIL=1` | 走一次**真的失败路径**（指向不存在节点的 `SetStyle`） | `inspector-error` |
| `MW_THEME=ink/mist/sand` | 三套主题 | `inspector-ink/mist/sand` |

轨迹（`MW_TRACE`）：`inspector w=288.00 head=40 obj=44 seg=30 bar=1.5 group=28 gutter=12`（几何真值）、
`group key=… open=…`（折叠）、`inspector_error shown=1 text=…`（错误条）。

## 6. v1 → v2 的漂移总账（每一包都记了数）

| 包 | 动了什么 | 漂移 |
| --- | --- | --- |
| SE-05 骨架 | 整列重排（260 → 288、分段/分组/对象行全部按 §4.1） | 三张新基线；`cm-arrow-inspector` **95242 px / 4.134%** |
| SE-06 样式组 | 色板 8 列 24、自定义虚线圆、无颜色斜杠格、stepper | `inspector` 29434 / 1.419%、`-narrow` 28147 / 1.357%；`-empty` 与 `cm-arrow-inspector` 0 px；新增 `-shape`/`-ink`/`-mist`/`-sand` |
| SE-07 内容组 | 五组默认展开、备注 3/8 行、标签色点、卡片来源、文本域去框 | 新增 `-content`；七张老基线 **0 px** |
| SE-08 作用域 | 作用域搬进分组标题、500ms 合并 | 六张重拍 43983–63195 px（2.1–3.0%），包围盒全在面板内；`-empty`/`-content` 0 px |
| SE-09 键盘/对比度 | 出厂键改 `Ctrl+Shift+I`；文字 `faint → dim` | 十张重拍，最大 **5465 px / 0.264%**（`-empty`），其余 752–2103 px（0.036–0.101%）；`-ink` **0 px**（暗主题的 faint 与 dim 本来就是同一档） |

读法：SE-05/06 是**重排**（像素大面积变动，有意为之）；SE-07 之后只剩内容与文案的局部变动，
到 SE-09 已经收敛到 0.03%–0.26% 的量级——检查器的几何稳定下来了。

## 7. 未做与风险

| 项 | 说明 |
| --- | --- |
| 能力清单 3 项延后 + 形状缺椭圆 | 都要动 `StyleOverride`（`Node.style`），属数据模型变更，需单独批准；方案在 SE-06 报告 §5 |
| 组内行间距 6 | 合同 §4.1 写 6，现在是全局 8；改它会让十张基线整体下移，记下一轮候选 |
| `dim`@panel2 = 4.14:1 | 差 AA 0.36；修要动主题令牌或换文字色 |
| 实机微软拼音 | 检查器三个文本框（备注 / 链接 / 标签）都只做了离屏等效 |
| 门怎么判 | 五条判词在 [gate.md](gate.md) 里留 ☐；第 4 条（能力清单齐全）需要人决定「接受 3 项延后」还是「先补字段」 |
