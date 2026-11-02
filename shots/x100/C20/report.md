# C20 · 概念图大纲行菜单（`⋯` 与行右键）

> 打磨轮 56 · 2026-11-01
> 账本：`docs/x100/polish-ledger.md`（新增 C20 行）
> 合同：`docs/30` §5.6「概念图在大纲里按 relationships 的生成树展开……让用户能像编辑
> 思维导图一样用大纲改结构」+ §5.6.1「大纲改结构」动作表；`docs/06` §6「大纲行」

## 1. 为什么是这一轮

概念图的大纲键盘路径在 CM-09 已全部落地：`Tab` 建「概念 + 边」、`Enter` 加概念不加边、
`Shift+Delete` 断进来那条边、`Shift+F2` 改链接短语、`Delete` 删概念。但这些都是**隐藏快捷键**：

- 打磨轮 48（M25）给思维导图大纲加了行尾 `⋯` 与行右键共用菜单，概念图被**有意跳过**：
  `outline_row_menu_skipped reason=concept_map`，不出 `⋯`、右键不弹（`shots/x100/M25/report.md` §9）。
- 结果是概念图用户在大纲里没有任何**可发现**的结构入口；`docs/30` §5.6 明确要求
  「像编辑思维导图一样用大纲改结构」，只靠记快捷键不算兑现。

本轮只补「大纲这一面」的入口，不新增命令、不改画布。

## 2. 范围

- 概念图大纲行尾显示同一枚 16px `⋯`（24px 动作道已预留，`M25` 的布局尺不动）。
- `⋯` 与行右键共用一张**概念图行菜单**，条目精确映射到既有 `Action`：
  | 菜单项 | 快捷键列 | 动作 |
  | --- | --- | --- |
  | 新建概念并连线 | `Tab` | `Action::LinkConcept` |
  | 新建概念 | `Enter` | `Action::NewConcept` |
  | 编辑连接短语 | `Shift+F2` | `Action::EditVia`（没有进来边时禁用） |
  | 重命名概念 | `F2` | `Action::EditText` |
  | 断开进入的边 | `Shift+Delete` | `Action::UnlinkConcept`（没有进来边时禁用） |
  | 复制 | `Ctrl+C` | `Action::Copy` |
  | 删除概念 | `Delete` | `Action::Delete` |
- 菜单引擎沿用 `menu::Menu`（键盘 ↑↓/Enter/Esc、贴边翻转与思维导图行菜单同源）。
- 轨迹：`outline_row_menu concept=1 entries=N`；概念图**不再**落
  `outline_row_menu_skipped reason=concept_map`（那条轨迹只保留给「没选中/编辑中」等拒绝路径）。

## 3. 非目标

- 不往概念图行菜单里放任何树语义：新建子主题/同级、升级/降级、折叠、聚焦、概要、边界、
  分离自由主题——`docs/30` §5.2/§5.7 明令禁止。
- 不做画布侧菜单的改动；不改 `.zmind` / `Node` / `NoteCard`。
- 不新增 `Command`：全部复用 `LinkConcept` / `NewConcept` / `EditVia` / `UnlinkConcept` /
  `Copy` / `Delete` / `EditText` 既有意图。
- 多选行只列「复制 / 删除概念」两项：建边、改短语、断边、重命名都只对**一行**成立，
  在多选里列出来会让人以为对整组生效；删除本来就是整组（与 M25 的
  `row_menu_scope_keeps_a_multi_selection_only_for_its_members` 同一条口径）。

## 4. 验收场景

1. **菜单可发现**：思维导图大纲的 `⋯` 行为不变；概念图大纲选中/悬停一行时出现同一枚 `⋯`。
2. **动作等价**：菜单每一项与对应快捷键落到同一 `Action`（轨迹里 `outline_row_action kind=…` 一致）。
3. **禁用态**：根行 / 孤立概念行没有「进来的边」→「编辑连接短语」「断开进入的边」置灰，
   点了不落地、不崩。
4. **无树语义**：菜单项数 = 7（3 条分隔线另算），不含 `AddChild` / `AddSibling` / `ToggleCollapse` / `Focus`。
5. **零污染**：不改命令、不改画布、不改导出；`cm-outline*` 既有基线因 `⋯` 出现而有意重拍，
   漂移量按行登记。

## 5. 预计证据

- `shots/golden/cm-outline-menu/` 新基线（概念图大纲、选中一行、`⋯` 菜单展开）。
- `cm-outline-menu-cancel`：`Esc` 关闭后回到行、无命令落地（新增基线）。
- `MW_TRACE`：`outline_row_context` → `outline_row_menu concept=1 entries=7` →
  `outline_row_action kind=LinkConcept` 等。
- `stage-cm` 62 → 64 场 / 124 → 128 次比较；`stage-b` 142/284 顺序复跑。

## 6. 实测结果（2026-11-01）

- **落地改动**：
  - `crates/mw-shell/src/panels/outline.rs`：`more_on` 去掉 `!self.concept_map` 一条
    （24px 动作道、16px `⋯`、悬停/选中出现与思维导图同一把尺）；行右键不再落
    `outline_row_menu_skipped reason=concept_map`（该轨迹退役）；`RowAction` 新增
    `LinkConcept` / `NewConcept` / `EditVia` / `Unlink` 四个变体；新增
    `Outline::row_edge(id)`（返回 `(from, rel)`，菜单禁用态与断边共用）与
    `concept_row_items(has_edge, can_delete)` 纯函数。
  - `crates/mw-shell/src/app_methods_2.rs`：`build_outline_row_menu` 按文档类型切菜单
    （轨迹 `outline_row_menu … concept=1 entries=9` = 7 条动作 + 2 条分隔线）；
    `run_outline_row_action` 落四个新动作——全部 `apply_outline_action` 走既有
    `OA::LinkConcept` / `OA::NewConcept` / `OA::EditVia` / `OA::UnlinkConcept`，
    与快捷键同一个入口；`Unlink` 在无进来边时兜一条
    `outline_concept_unlink_skipped reason=no_incoming_edge`。
  - `scripts/x100-stage-cm-gate.ps1`：门清单 +2 场（62 → 64 场 / 124 → 128 次比较）。
- **像素账**（1920×1200 @ ppp 1.0，对照帧 = 同夹具不开菜单）：
  - `cm-outline-menu` ↔ 对照：**25 848 px**，bbox `(561,153)–(1358,437)`（菜单区）。
  - `cm-outline-menu-cancel` ↔ 对照：**3 284 px**，bbox `(561,153)–(1358,182)`——
    只是选中行区域：右键那一下走 `canvas.select_node` 且 Esc 后焦点回行，与
    `MW_OUTLINE_SELECT` 钉住的静态选中态在焦点环/行底色上有差；**无任何命令落地**
    （轨迹里没有 `outline_row_action`，`menu_close reason=escape`）。
  - `cm-outline-menu` ↔ `-cancel`：**23 062 px**，bbox `(686,166)–(1351,437)`（菜单本体）。
- **既有基线有意重拍 ×3**：`cm-outline-via` / `cm-outline-via-clear` / `cm-outline-unlink`
  各差 **36 px**，bbox 一模一样 `x[1336..1351] y[166..169]`——选中行右侧冒出的那颗 `⋯`
  （15×4 px）。概念图大纲**没有**树语义变化，三场都是同一处装饰。
- **动作等价**（轨迹，`target/trace-menu-act.txt`）：右键 → `outline_row_menu … concept=1
  entries=9` → `menu_nav key=down` ×3（`新建概念并连线` → `新建概念` → `编辑连接短语`，
  分隔线自动跳过）→ `menu_activate key=enter` → `outline_row_action kind=EditVia`，
  与 `Shift+F2` 同一条 `OA::EditVia`。
- **单测**：`panels::outline::tests::concept_row_menu_is_proposition_only`——
  七条动作的顺序与标签、无树语义断言、无进来边时「编辑连接短语」「断开进入的边」
  「删除概念」置灰。测试 1120 → **1121**。

## 7. 未做 / 风险

- 概念图多选只给「复制 / 删除概念」两项，没有「批量改短语 / 批量断边」——本轮按最小集收口，
  要扩先补设计。
- 「复制」在概念图里是 CM-11 的独立浮动概念复制（经 `copy_action`），与树复制不同；
  菜单只给入口，语义由画布既有路径保证。
- 菜单从右键锚点展开，页头搜索行开着时同一层互斥逻辑沿用 M25 的实现，未单独拍图。
- 实机 IME / 键盘手感未核。
