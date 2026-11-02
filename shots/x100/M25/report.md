# M25 · 大纲行「更多」与行右键菜单（打磨轮 48）

- 日期：2026-10-31
- 依据：`docs/06` §6 上下文菜单表——「大纲行 | 与节点菜单一致 + 设为根主题（P2） |
  未做（大纲行的「…」仍是占位）」；`docs/28` §5.2 行规格——「更多 | 16px `⋯`，距列右 8 | 悬停/选中」；
  §5.3 状态表（悬停 4% / 选中 7%，动作按需出现）。
- 状态：**已实现**（机器门绿；人工门未开）

## 1. 现状（代码复核，先于改动）

全屏大纲已经是思维导图的主编辑面（`docs/28` §5），行内键盘路径齐全：
`Tab/Enter` 建子/同级、`F2` 改名、`/` 折叠、`Alt+Enter` 聚焦、`Ctrl+X/C/V` 剪贴板、
`Delete` 删除、`Shift+Tab` 缩进……但**指针用户没有行级动作入口**：

- 行尾只有 5px 角标圆点（备注/卡片），没有 `⋯`；`docs/28` §5.2 要求它在悬停/选中时出现。
- 行上右键没有菜单；`resp` 的次键点击被整行 `click_and_drag` 吞掉，什么都不发生。
- 唯一能用的行菜单是页头那个 `⋯`（折叠层级/密度/帮助/文档信息），它不作用于某一行。
- `docs/06` §6 从外壳 v2 起就把它记成「未做（大纲行的「…」仍是占位）」。

结果：在大纲里工作的用户想「剪切这一行」「折叠这块」「聚焦这个分支」，只能退回画布右键，
或者先背快捷键。这正是 `docs/28` §5.2「动作按需出现」要补的最后一格。

## 2. 范围

1. **行尾 24px 动作道（固定）**：非编辑行的右缘留 `[右-32, 右-8]` 给 `⋯`（三颗 1.4px 点，
   总宽约 16px，距列右 8）。动作道**始终占位**，不是悬停才挤进来——否则指针一进行，
   长标题会当场重排，是比「按钮晚出现」更差的体验。换行测量与绘制同用一把尺
   （`text_avail` / `text_right` 各减 `MORE_LANE`），量几行就画几行。
2. **角标让位**：备注/卡片圆点左移到动作道左边（右-34.5），热区随中心移动；
   折叠计数仍在文字尾部。文字、角标、`⋯` 三者互不重叠。
3. **出现时机**（`docs/28` §5.2）：行悬停或选中时显示 `⋯`；编辑中、前台浮层开着的帧隐藏，
   但动作道不收回（布局不跳）。悬停底色/选中底色沿用 §5.3 现有档位。
4. **两个入口，同一张菜单**：
   - 点 `⋯`：在按钮下方展开菜单；
   - 行上右键：在指针处展开同一张菜单（右键顺带把这一行变成操作目标）。
5. **动作集（只收大纲这一面看得见结果的动作）**：
   - 单行：新建子主题（大纲里无直接键，快捷键列留空）· 新建同级主题 `Enter` · —— ·
     剪切 `Ctrl+X` · 复制 `Ctrl+C` · 粘贴 `Ctrl+V` · 删除 `Delete` · —— · 编辑文字 `F2` · —— ·
     折叠/展开 `Alt+.` · 聚焦 `Alt+Enter`。
   - 多行（右键的那一行在多选里且选中数 > 1）：折叠 · 移入同一父级 · —— · 复制 · 删除。
   - 画布专属项（插入关系线 / 新建卡片笔记 / 添加概要 / 添加边界 / 分离为自由主题 / 样式）
     **不放进行菜单**：大纲上看不到它们的结果，点了像没反应。
   - 快捷键列按**大纲**的真实键位写（`docs/28` §5.5）：`Tab` 是缩进、加子级没有直接键；
     折叠是 `Alt+.` 不是 `/`。不做「照抄画布菜单，再把用户骗一次」。
6. **选择语义**：右键一行 → 若它已在多选里则保留整组（批量项生效），否则单选它；
   菜单动作全部走既有 `Command` 与 `apply_outline_action` 的同一套落地函数，一次撤销。
7. **键盘/浮层纪律**：菜单开着时 `↑↓/Enter/Esc` 先于画布与大纲行消费（复用 `menu` 引擎），
   并进 `overlay_state().context` → 画布与行键都不收输入；`Esc` 只关菜单，不退出大纲。
8. **轨迹**：`outline_row_menu id=… multi=… source=more|context`、
   `outline_row_action kind=… target=… multi=…`；动作落地沿用既有 `outline_*` 轨迹。
9. **概念图大纲不弹这张菜单**：概念图大纲没有树级动作（`Enter` 是「新建概念」），
   本轮只把 `⋯` 与行右键留在思维导图大纲；概念图大纲右键写
   `outline_row_menu_skipped reason=concept_map` 轨迹后什么都不弹。它的行动作
   （新建概念/断边/改短语）另开轮次，不在这一包里塞半套。
10. **文档**：`docs/06` §6 大纲行一格从「未做」改成「已实现（画布专属项除外；设为根主题 P2 未做）」；
   `docs/28` 无需改（它本来就是合同）。

落地位置：`crates/mw-shell/src/panels/outline.rs`（动作道、`⋯`、右键、`Action::RowMenu`、
`RowAction`/`RowMenu`）、`crates/mw-shell/src/app_methods_2.rs`（建菜单/落地/选择）、
`crates/mw-shell/src/main.rs`（键盘与绘制顺序）、`crates/mw-shell/src/app_methods_1.rs`
（浮层状态）、`scripts/x100-stage-b-gate.ps1` + 输入脚本（三场新基线）。

## 3. 非目标

- 不把画布节点菜单原样搬过来：关系线、卡片、概要、边界、分离、样式在大纲面没有落点。
- 不做「设为根主题」（`docs/06` 标 P2，且会改文档根语义，属模型变更）。
- 不做概念图大纲的行菜单：它的动作集（新建概念/断边/改短语）与树语义不同，另开轮次；
  本轮在概念图大纲里只保留轨迹 `outline_row_menu_skipped`，不出 `⋯`、不弹菜单。
- 不做长按/触摸手势（桌面鼠标右键已经覆盖；触摸另开轮次）。
- 不改 `Row`/`Node`/`.zmind` 字段，不加依赖，不改撤销语义。
- 不改页头 `⋯` 菜单与大纲行内既有快捷键；行菜单只是它们的指针补充。

## 4. 验收条款

### 4.1 单元（`mw-shell` 纯函数）

| # | 用例 | 期望 |
| --- | --- | --- |
| 1 | `text_avail` 普通行 @800 列 | 716 − 24 = **692** |
| 2 | 有备注/卡片 | 692 − 15 = **677** |
| 3 | 折叠且有子项 | 692 − 22 = **670** |
| 4 | 任务行 | 692 − 18 = **674** |
| 5 | `row_more_rect` @右侧 800、行顶 100、高 32 | `(768, 102)–(792, 130)`（24×28，距右 8，垂直居中） |
| 6 | 多选作用域：目标在 n>1 选中里 | `multi = true`；否则 `multi = false` |
| 7 | 单行菜单条目数/禁用态 | 根主题无「新建同级」「剪切」「删除」；无子项时「折叠」禁用；已是聚焦根时「聚焦」禁用 |

### 4.2 端到端（轨迹 + 金样）

- 悬停一行 → `⋯` 出现（三颗点 + 悬停底色不变）；`outline-more` 基线。
- 行上右键 → 菜单在指针处展开：`outline_row_menu id=… multi=false source=context`；
  `outline-row-menu` 基线。
- 菜单点「新建子主题」→ 新行进入编辑态（与 `Tab` 同一条路径），轨迹
  `outline_row_action kind=add_child` + `outline_add_child`；`outline-row-menu-add` 基线。
- 右键多选中的一行 → 菜单出现「折叠 / 移入同一父级 / 复制 / 删除」，`multi=true`；
  执行「复制」后可 `Ctrl+V` 粘出整组（13 → 20 个节点）。
- 菜单开着按 `↑↓/Enter` 执行、`Esc` 关闭；`Esc` 后仍在**大纲视图**，不退出到画布。
- 菜单开着时行键与大写字母不进编辑：焦点不在行上、`action` 不被行消费
  （`text x` 后 `outline_edit_start*` 计数 0）。
- 概念图大纲右键：轨迹 `outline_row_menu_skipped reason=concept_map`，无菜单、无命令。

### 4.3 既有基线

动作道改变所有非编辑行的文字可用宽，长标题可能提前一行换行；角标圆点整体左移 24px。
`outline` / `outline-wrap` / `outline-hover-select` / `outline-search*` / `outline-20k*` /
`outline-drag*` / `outline-focus` / `outline-compact` / `cm-outline` 均按实际漂移**有意重拍**，
逐场记 bbox 与像素数；没有角标、没有长标题、没有悬停/选中的场景应零漂移。

## 5. 证据

| 类型 | 内容 |
| --- | --- |
| 单测 | `text_avail_matches_the_row_inset`（普通 692 / 备注 677 / 折叠 670 / 任务 674）、`row_more_rect_sits_in_the_action_lane`（24×28，距右 8，垂直居中）、`row_menu_scope_keeps_a_multi_selection_only_for_its_members`。全量测试 **1071**（`mw-shell --bins` **609**）；fmt / clippy 基线（36 条指纹，无新增）干净 |
| 轨迹 | `trace-single.txt`：`outline_row_context` → `outline_row_menu … multi=false entries=12` → `menu_nav/activate` → `outline_row_action kind=AddChild` → `outline_add_child new=Some(…)` → `outline_edit_start source=pending`。`trace-multi.txt`：Shift 扩选后右键 → `multi=true entries=5`，↓×2 + Enter → `kind=MoveSameParent`，`scene_build` 重建。`trace-copy.txt`：多选菜单「复制」→ `Ctrl+V`，节点 13 → **20**。`trace-esc.txt`：`menu_close reason=escape` 后焦点回行。`trace-overlay.txt`：菜单开着 `text x`，`outline_edit_start*` 计数 **0**。`outline_row_menu_skipped … reason=concept_map` 落概念图大纲 |
| 金样 | 三场新基线：`outline-more`（sha256-12 `2D5B63C450D1`，Warmup 3）、`outline-row-menu`（`85DCFE6ACF75`）、`outline-row-menu-add`（`674953F39EB4`，`MW_OUTLINE_EDIT=1`）；均两遍 `--tol 0` 零差异 |
| 有意重拍 | `outline` / `outline-compact`：**128 px / 1,749,600（0.007%）**，bbox `x[1354..1398] y[427..436]`，max 208——备注圆点左移 24px（物理 36px 含抗锯齿）；`outline-search` 128 px（`y[328..337]`）、`outline-search-labels` 128 px（`y[508..517]`）；`outline-drag` 128 px（max 196）、`outline-drag-illegal` 128 px；`outline-hover-select` **272 px / 1,944,000（0.014%）**，bbox `x[1444..1488] y[249..436]`（悬停行 + 选中行两颗角标）；`outline-wrap` **37,166 px / 2,304,000（1.613%）**，bbox `x[438..1548] y[242..870]`（文字可用宽窄 24px 后长标题提前换行）、`outline-wrap-2x` **47,089 px（2.044%）**，bbox `x[264..1744] y[321..1139]`。重拍后逐场二次 `--tol 0` 零差异 |
| 零漂移 | `outline-focus`、`outline-20k`、`outline-20k-deep`、`cm-outline` 全 **0 px**；`cm-outline-unlink` / `cm-outline-via` 在概念图大纲关掉 `⋯` 后与 HEAD 基线**逐字节相同**，`cm-outline-via-clear` 按现状重拍后零差异（36 px 的中途差就是选中行那颗 `⋯`） |
| 门 | stage-b **PASS**：`11:46:59–11:54:07`，**134 场 / 268 次比较**两遍 `--tol 0`，含 fmt / clippy 基线 / 全量测试 1071；stage-cm **PASS**：`11:54:12–11:57:16`，**58 场 / 116 次比较**两遍 `--tol 0` |

补充：证据阶段发现并修正了两处**快捷键标签与大纲键位不符**（`新建子主题` 原写 `Tab`，
但大纲的 `Tab` 是缩进、加子级无直接键；`折叠` 原写 `/`，大纲实为 `Alt+.`）——
`outline-row-menu` 基线因此重拍（sha 为修正后的 `85DCFE6ACF75`）。
中间一次门跑在 `cm-focus-question` 出现 **1/2,304,000 px（最大差 3）** 的单像素抖动；
该场景单跑三次、最终门两遍均零差异，且本轮未触碰概念画布，按既有渲染抖动记账。

## 6. 未做 / 风险

- 菜单未收画布专属项（见 §3），`docs/06` 的「与节点菜单一致」按可见面解释，不是逐条照搬。
- 「设为根主题」仍是 P2，未做。
- 动作道固定占 24px：宽屏无感，窄列（240px 面板）长标题会更早换行；这是稳定的代价，
  比悬停重排好。`outline-wrap` 的 1.61% / 2.04% 就是这笔账的量化。
- 概念图大纲本轮**没有**行菜单（右键只落 `outline_row_menu_skipped`）；它的动作集
  （新建概念 / 断边 / 改短语）要另开一包。
- 动作道在概念图大纲里仍占 24px（标记随同一把尺左移），本轮没有为它做第二套布局，
  避免出现两套测量尺；三个概念图大纲基线因此与 HEAD 对齐或按现状重拍。
- `Ctrl+]` 等键在部分输入法下的归属仍未实机核对（与 M4 同缺口，不影响本轮的指针入口）。
