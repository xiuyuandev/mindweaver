# SE-05 · 检查器骨架 v2（P 轨 E2 第一包）

> 设计合同：`docs/30` §4.1（结构）、§4.2（控件规格，只取「行高/分段/分组」部分）、§4.3（无选中状态）、§4.5（禁止项）。
> 执行合同：`docs/31` §5 第 SE-05 行。依赖 SE-02（共用图标与令牌）。

## 1. 开工前审计（对着 `docs/30` §4.1 逐条查代码）

审「合同写了什么」对上「代码做了没有」。

| 合同条目 | 现状 | 结论 |
| --- | --- | --- |
| 面板宽默认 **288** | `panels::inspector::PANEL_W = 260.0` | ❌ |
| 可拖 **240–360**，记忆 | 可拖已有（`egui::Panel::resizable` + `window_state.inspector_width()` 落盘），但夹取区间是 **220–520** | ❌ 区间不符 |
| 与画布之间 1px `hairline` | `ui.painter().vline(panel.left() + 0.5, …, Stroke::new(1.0, ch.sep))` | ✅ |
| 头行高 **40**：「检查器」12/16 secondary + 右侧关闭图标按钮 **24** | `add_space(8)` + 一行 `Type::BODY` strong 文本 + 20×20 自绘叉；没有固定行高 | ❌ |
| 对象行高 **44**：标题 13.5/20（省略）+ 副行 11.5/16 secondary | 标题行与副行是两个 `horizontal`，各自被 `add_space` 撑开，高度随内容变；字号用 `HEADING`(16)/`SMALL`(12) | ❌ |
| 分段控件高 **30**：选中 8% 强调底 + 底部 1.5px 强调条，文字 12.5/18，**无描边胶囊** | `tabs` 用 `ui_kit::chip`：高 **28**、选中画 `accent_soft` 底 **+ 1px hairline 描边**、圆角 `BUTTON_RADIUS` —— 正是合同禁止的「描边胶囊」 | ❌ |
| 分组标题行高 **28**：chevron 12 + 名称 12/16 500 + 右侧摘要 11.5/16 secondary | `section_label_hint`：3px 强调竖条 + `LABEL`(15) strong + `CAPTION`(11) 说明；**没有 chevron、不能折叠** | ❌ |
| 组间 8px `hairline` 分隔 | 无分隔线（只有 `add_space`） | ❌ |
| 组内上下 8、左右 **12**；控件行高 28；行间距 6 | 左右用 `panels::PAD = space::LG = 16`；行距 `add_space(6)` 有的地方有、有的地方是 4 | 部分 ✅（左右宽 4px、行距不齐） |
| 折叠状态**记忆**到设置 | 没有折叠，也没有记忆 | ❌ |
| 默认展开：样式组（颜色、文本、边框与形状）；内容组（文本内容、任务、标签、链接、卡片）；标记组折叠 | 样式页有「颜色」「形状与文字」两组；内容页与标记页紧跟控件、没有分组 | ❌ |
| 无选中节点：**只显示一句话 + 文字动作「选中第一个主题」** | `doc_body` 直接铺文档属性 / 主题 / 强调色 / 对比度 / 配色微调，没有空状态 | ❌ |
| §4.5 禁止项（420 宽大面板、描边胶囊 tab、三处「自定义…」整行、应用/重置/确定、彩色 chip 墙、字形图标按钮、画布级操作） | 逐条对照：只有「描边胶囊 tab」与「宽 260 但可拖到 520」两项擦边，其余均无；「自定义…」已是色板第 9 格（SE-06 前身） | ❌ 两条（由上表修） |

**结论**：SE-05 是**骨架级几何重排**，不是加控件——面板宽、头行、对象行、分段控件、分组标题
与折叠记忆、空状态六件事；控件本身（色板 8 列、stepper、三段对齐…）属于 SE-06/07。

## 2. 非目标

- 不加新属性控件（色板列数、字号 stepper、字重、对齐、边框/圆角/形状都是 SE-06；备注/任务/标签/链接/卡片是 SE-07）。
- 不做作用域语义（SE-08）、不做键盘全路径（SE-09）。
- 不动 `Node` / `StyleOverride` / `.zmind` 字段；分组折叠状态只进窗口状态文件（`window_state`），不进文档。
- 不改大纲面板的 `PAD`（它有自己的行内距）。

## 3. 验收场景

（拍摄尺寸用 1280×720 @ ppp 1.5——与起始页各场景同一张窗户，轨迹数字好换算。）

1. **`inspector`**（1280×720 @1.5，`MW_INSPECTOR=1 MW_SELECT_DEMO=1`）：面板宽落在 288；
   头行 40、对象行 44、分段控件 30、分组标题 28 都能在轨迹里量到
   （`inspector w=288.00 head=40 obj=44 seg=30 bar=1.5 group=28 gutter=12`）。
2. **`inspector-narrow`**（同上，`MW_INSPECTOR_W=240`）：面板 240 时不溢出、不裁字，
   「形状」四段按版心均分收进一行。
3. **`inspector-empty`**（`MW_INSPECTOR=1`，不选中）：一句话 + 文字动作「选中第一个主题」。
4. **折叠**：点「颜色」分组标题 → 组内容收起（轨迹 `group collapse=颜色 open=0`），再次进入面板仍收着（记忆）。
5. **几何常量**：单测钉住 `PANEL_W=288`、`PANEL_W_MIN/MAX=240/360`、`clamp_width` 的夹取、
   四段行高（40/44/30/28），以及「分段控件不描边」用的画笔路径（用绘制调用计数或常量断言）。

## 4. 预计证据

- 轨迹：`group` / `segment` / `inspector width=` 行（`MW_TRACE`）。
- 基线：`inspector`、`inspector-narrow`、`inspector-empty` 三张（`--tol 0` 双跑）。
- 单测：宽度夹取、折叠默认值、几何常量。
- 两道门绿；测试数只增不减。

## 5. 风险

- 面板几何是全窗基线的一部分：`base-ppp*`、`outline*` 等既有无检查器的场景应当**零漂移**；
  若漂了，说明改动漏到了面板之外。
- 折叠记忆写进 `window_state`：harness 用 `MW_WINDOW_STATE=off` 时只在内存里，不影响基线确定性。
- 左右内距 16 → 12 会让所有检查器控件轻微左移：这是本节有意为之（§4.1 组内左右 12），
  但**必须在报告里量化**，不能默默漂。

## 6. 结果（2026-10-30 回填）

| 项 | 结果 |
| --- | --- |
| 面板宽 | `PANEL_W = 288`、`PANEL_W_MIN/MAX = 240/360`；`clamp_width` 夹取；`window_state.inspector_width()` 落盘。默认值来自 `Store::default`，旧存档缺字段走 288 |
| 头行 / 对象行 | 固定行高 40 / 44（`geo::HEAD_H`/`OBJ_H`），标题 13.5/20 省略 + 副行 11.5/16；多选副行「N 个节点」；关闭按钮 24×24 落在右 12px |
| 分段控件 | `segment()`：高 30、**无描边**，选中 8% 强调底 + 底部 1.5px 强调条（老 chip 的描边胶囊已删） |
| 分组标题 | `group_header()`：28 高、chevron 12（两段线）、左名称 12/16、右摘要 11.5/16；组间 8px hairline（`GROUP_GAP`）；点它返回 `ToggleGroup` |
| 折叠记忆 | `window_state.inspector_groups`（`键=0/1`，只记用户动过的组）；默认展开=非标记组，标记组默认折叠（§4.1） |
| 无选中状态 | 「还没有选中主题」+ 副行 + 文字动作「选中第一个主题」（点它 `SelectNode(root)`，`main.rs` 已接） |
| 场景 | `inspector` / `inspector-narrow` / `inspector-empty` 三张基线（1920×1080 @ ppp 1.5，`--tol 0` 双跑零差异）；已进 stage-b 门（107 → 110 场） |
| 轨迹 | `inspector w=240.00 head=40 obj=44 seg=30 bar=1.5 group=28 gutter=12`（每运行一行，`app_methods_2.rs` 面板落定后报）；折叠点击报 `group key=… open=…` |
| 单测 | `the_skeleton_geometry_matches_the_contract`、`panel_width_clamps_to_the_drag_range`、`shape_segments_fit_the_narrowest_panel`、`group_state_prefers_what_the_user_toggled`（mw-shell 测试 537 → **538**） |
| 内距 | 组内左右 `panels::PAD(16)` → `GUTTER(12)`（§4.1「左右 12」），全部检查器行左移 4px——有意为之，已量化 |
| 门 | stage-b **110 场 / 220 次比较**双跑 PASS（2026-10-30 01:33）；stage-cm **48/96** 双跑 PASS（01:45）——**首跑红过一场 `cm-arrow-inspector`**（95242 px / 4.134%，检查器整列重排的预期漂移），按 `-Update` 重拍后双跑零差异 |

### 修掉的回归（开发中实测）

- 「形状」四段（圆角/直角/胶囊/下划线）写死 `66×4 + 间距`，内容最小宽 **292** > 目标 240 →
  egui 把面板顶到 292、用户拖到 240 也不生效，且旧回写逻辑把 293 记成用户的选择
  （「拖到 240，下次打开变 293」）。修法：`shape_seg_w(available)` 按版心均分（下限 44）；
  回写只在 `drag_stopped()` / `dragged()` 时发生。`inspector-narrow` 基线锁住修后的行为。

### 未做 / 风险

- 折叠只有单测（记忆、默认值、前缀不串）+ 点击轨迹，**没有**「点分组标题收起」的金样——
  点击坐标要从布局常数推，脚本脆弱；SE-10 证据包再补。
- 实机微软拼音、实机键盘（F9 关闭、Tab 进分组）未核——全员共享缺口，不记 `已验证`。
- `GroupSpec` 没抽：`group_header`/`group_with` 参数 8 个，按本仓库惯例挂
  `#[allow(clippy::too_many_arguments)]`（`app_helpers.rs` 等十处同款）。
