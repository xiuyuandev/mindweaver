# M23 · 思维导图插入关系线（Ctrl+L，打磨轮 46）

- 日期：2026-10-31
- 依据：`docs/02` §10 快捷键表「`Ctrl+L` 插入关系线」；§3 节点编辑的多选/批量上下文；
  `docs/06` §5 画布交互表与 §6 上下文菜单。
- 状态：**已实现**（机器门绿；人工门未开，不记 `已验证`）

## 1. 现状（代码复核，先于改动）

关系线在模型与渲染里早就是完备的：`Command::AddRelationship`、`Relationship.head/color/label`、
`Scene.relationships`、wgpu / SVG / PDF 四处渲染、检查器「关系线」页、右键「编辑关系线标签 / 删除 /
箭头方向」都在；默认样例里也有一条「字体同源」的关系线。**已有但不可发现的创建入口**：
悬停节点露出的四边锚点可以拖到另一个节点上松手（M2b-4，`classify → DragKind::Link`，
`commit_link` 对思维导图同样生效），但这条手势既不在 `docs/06` §5 的交互表里，
也没有菜单或快捷键，用户不可能知道它存在。**没有可发现的入口**：

- 思维导图节点右键菜单（`app_methods_2.rs` 的 `ContextTarget::Node`）只有新建子/同级、剪切复制粘贴、
  删除、编辑文字、卡片、概要、边界、分离、样式、折叠、聚焦——没有「插入关系线」。
- 键盘路径没有 `Ctrl+L`；`docs/02` §10 的脚注从 2026-10-18 起就写着
  「⑤ `Ctrl+L/B/]`（关系线/边界/概要）尚无创建入口」。
- 概念图有「从概念拖到另一个概念/空白」的连线手势（`cm-link-*` 一系列基线），
  但那是命题语义，且只长在概念图上。

于是「思维导图能画关系线」这件事只能靠碰巧悬停到节点边缘、看见四个小圆点才发现；
`docs/02` §3 与 §10 都把关系线当作应有能力，缺的是**能被找到**的入口。

## 2. 范围

1. **插入态**（`mw-shell::canvas`）：新增 `Canvas.rel_link: Option<String>`（源节点 id）。
   - 入口一：选中**恰好一个**节点、画布握着键盘时按 `Ctrl+L`；
   - 入口二：节点右键菜单「插入关系线 `Ctrl+L`」（仅思维导图、单节点）。
   - 进入时源节点画 2px 强调环，虚线从源盒中心跟随指针，光标变十字；
     提示条说「点击另一个主题建立关系线 · Esc 取消」；轨迹 `rel_link_mode source=…`。
   - 再按一次 `Ctrl+L` = 取消（`rel_link_cancel reason=toggle`）。
2. **落点**：插入态下的下一次左键单击按目标判定（纯函数 `rel_link_step`）：
   - 落在另一个主题上 → 发一条 `Command::AddRelationship { from, to, label: None }`，
     选中新建的线（检查器可直接改颜色/箭头/标签），相机不动，轨迹
     `rel_link_created from=… to=… id=…`；一次 `Ctrl+Z` 收回这条线。
   - 落在源主题自身 / 空白 / 折叠点 / 注解 / 已有关系线 → 取消插入态
     （`rel_link_cancel reason=self|blank|other`），随后照常处理这一击（该选谁选谁）。
3. **取消**：`Esc` 优先退出插入态（在「关系线选中 → 聚焦 → 多选」之前），
   轨迹 `rel_link_cancel reason=esc`；不碰文档、不入撤销栈。
4. **重复与失败**：思维导图里同一对主题只留一条线（模型层既有护栏）。
   `AddRelationship` 返回 `Invalid("两节点之间已存在关系线")` 时不静默：
   提示条说明原因，插入态退出，轨迹 `rel_link_failed from=… to=…`。
5. **金样**：`mm-rel-link`（插入态 + 指针悬在目标上，源/目标双环 + 虚线预览可见）与
   `mm-rel-link-created`（点击后线出现、提示条换成「已插入关系线 · Ctrl+Z 撤销」、
   标题栏「未保存」、源主题仍可见），都进 `stage-b` 门。关系线的选中态没有屏上手柄，
   金样不承诺「选中环」；选中由轨迹 `select relationship` 与检查器对象行证明。

落地位置：`crates/mw-shell/src/canvas.rs`（字段 + `rel_link_step` + 预览绘制）、
`crates/mw-shell/src/canvas/canvas_methods_3.rs`（快捷键 / Esc）、
`crates/mw-shell/src/canvas/canvas_methods_2.rs`（点击落点 + 菜单动作）、
`crates/mw-shell/src/app_methods_2.rs`（菜单行 + `A::BeginRelationship`）、
`scripts/x100-stage-b-gate.ps1`（两场新场景）。

## 3. 非目标

- 不改概念图的连线手势与命题语义（概念图里 `Ctrl+L` 不抢拖拽连线）。
- 不自动弹关系线标签编辑框：新建线先选中，`F2` / 双击线中点是既有的改标签入口。
- 不加关系线类型（虚线/箭头以外的样式）、不做跨标签页连线。
- 不给边界/概要补快捷键（菜单已有入口，留给后续轮次）。
- 不改 `Node` / `Relationship` / `.zmind` 字段；不新增依赖。

## 4. 验收条款

### 4.1 单元（`mw-shell` canvas 纯函数）

| # | 用例 | 期望 |
| --- | --- | --- |
| 1 | 源 → 另一个节点 | `Some(目标 id)` |
| 2 | 源 → 自身 | `None`（取消） |
| 3 | 源 → 空白 / 折叠点 / 注解 / 关系线 | `None`（取消） |
| 4 | 目标 id 不在场景里 | `None`（取消，不发命令） |
| 5 | 取消后原点击照常生效 | 选中/清空选择不被插入态吞掉 |

### 4.2 端到端（轨迹 + 金样）

- 选中「中文排版」→ `Ctrl+L` → 轨迹 `rel_link_mode source=…`，虚线预览出现。
- 指针移到「交互与性能」→ `mm-rel-link` 定格在预览帧（源环 + 目标环 + 虚线 + 指针圆点；
  十字光标是系统光标，离屏金样不画）。
- 单击目标 → `rel_link_created from=… to=… id=…`；文档 `revision` +1、撤销栈 +1；
  轨迹 `select relationship …`，检查器若开着则对象行切到该关系线；`mm-rel-link-created`。
- `Ctrl+Z` → 线消失、源/目标主题不动（`mw-core` 既有 `relationship_add_delete_cleanup_and_undo`
  钉住 `AddRelationship` 的撤销语义）。

### 4.3 既有基线

- 只新增一个 `Option<String>` 视图态与一个右键菜单行；不进入插入态时渲染逐像素不变。
- **有意漂移**：`menu`（132 584 px / 7.578%，bbox x[520..1212] y[546..980]）与
  `menu-narrow`（39 286 px / 5.931%，bbox x[557..892] y[378..669]）——节点菜单多出
  「插入关系线」一行，旧基线已按 `docs/29` §5 重拍、各双跑 `--tol 0` 零差异。
  640×460 下菜单因此进入滚动（聚焦行需滚一下），见 §6。
- 其余既有场景与 `stage-cm` 应零漂移。

## 5. 证据

| 类型 | 内容 |
| --- | --- |
| 单测 | `rel_link_step_commits_only_on_another_node`（命中另一节点 = 提交、自身/空白 = 取消、无源 = 空闲）；`escape_walks_rel_link_then_relationship_then_focus_then_selection`（Esc 四层顺序）。测试 1066 → **1067**（23 个套件；`mw-shell --bins` 605 + 1 ignored） |
| 轨迹 | `shots/x100/M23/trace.txt`：`rel_link_mode source=…` → `toast 点击另一个主题建立关系线 · Esc 取消` → `rel_link_created from=… to=… id=…` → `select relationship …` → `toast 已插入关系线 · Ctrl+Z 撤销`。取消路径 `rel_link_cancel reason=esc|toggle|self|blank|other` 与失败 `rel_link_failed` 由单测 + 代码路径覆盖，本轮未单独拍金样 |
| 金样 | `shots/golden/mm-rel-link`（sha256-12 `E7E0682C9B43`）、`shots/golden/mm-rel-link-created`（`2EC078FE25E8`），`--tol 0` 连跑两遍零差异；两帧差 **38 459 px / 2.1982%**（画布 21 616、提示条 3 270、标题栏 1 843、状态栏 1 274）。同脚本只选中不按 `Ctrl+L`（`before.png`）→ 插入态（`preview.png`）差 **43 869 px / 2.5074%**（画布 33 109、提示条 10 760）；落地帧 `created.png` |
| 门 | stage-b **PASS**：`10:04:10–10:12:12`，**129 场 / 258 次比较**两遍 `--tol 0`，`MACHINE GATE PASS`；`menu` / `menu-narrow` 有意重拍（`132 584 px / 7.578%`、`39 286 px / 5.931%`），其余场景零漂移。stage-cm **PASS**：`10:17:02–10:21:51`，**58 场 / 116 次比较**两遍 `--tol 0`，零漂移（首跑因 `canvas_methods_2.rs` 新增 `clippy::if_same_then_else` 判红，合并两个相同 `Crosshair` 分支后复跑全绿） |

## 6. 未做 / 风险

- 插入态跨文档：切标签页时若源节点属于旧文档，落点点击会因找不到节点而取消并提示；
  本轮不主动在切页时清插入态（后续可补）。
- 指针停在窗口外时预览只画到画布裁剪边：不画虚线（与框选同一取舍）。
- 640×460 这类矮窗口下，节点菜单因多一行而进入滚动（`menu-narrow` 基线可见滚动条）；
  菜单本身可滚动、键盘与滚轮可达，但「聚焦」不再一屏可见。
- 实机 IME / 键盘手感未核（与 M4 同缺口）；`Ctrl+L` 在中文输入法下的归属未实机验证。
