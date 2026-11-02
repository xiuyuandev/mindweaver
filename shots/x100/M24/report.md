# M24 · 边界/概要的键盘路径（Ctrl+B / Ctrl+] ，打磨轮 47）

- 日期：2026-10-31
- 依据：`docs/02` §10 脚注 ⑤——`Ctrl+B/]`（边界/概要）仍无快捷键，创建入口只在节点右键菜单；
  `docs/06` §5 画布交互表与 §6 上下文菜单；M23 报告 §3 把这两键列为「留给后续轮次」。
- 状态：**已实现**（机器门绿；人工门未开）

## 1. 现状（代码复核，先于改动）

边界与概要的**模型、命令、渲染、菜单都在**：

- `Command::AddBoundary { parent, targets, color }` / `Command::AddSummary { parent, targets, text }`
  在 `mw-core` 里各有撤销语义（一次 `Ctrl+Z` 收回一条注解）。
- 画布 `add_boundary_action()` / `add_summary_action()`（`canvas_methods_1.rs`）已经实现，
  目标是「主选所在父级下、同时被选中的那些子级」，判定在 `annotation_targets()`
  （`canvas.rs`：主选必须有父级；只圈同一父级下的选中项；根主题因此不可注解）。
- 右键菜单有「添加概要 / 添加边界」，启用条件 `can_annotate_selection()`；
  概念图菜单明确不出现这两项（`app_methods_2.rs` 的概念图分支：没有子主题、没有概要/边界）。

缺的只有**键盘路径**：菜单行第二列是空的，`docs/02` §10 的脚注从 2026-10-18 起就写着
「`Ctrl+B/]` 仍无快捷键」。一个画布上纯键盘工作的用户，加完一批子主题后想圈边界，
只能伸手去右键。菜单里没有「快捷键」这一列字，也让人以为它本就没有键。

## 2. 范围

1. **快捷键**（`mw-shell::canvas`，思维导图专有）：
   - `Ctrl+B` = 添加边界，`Ctrl+]` = 添加概要；只在画布握键盘、无行内编辑
     （`editor` / `summary_edit` / `rel_edit`）、且不在插入关系线态时生效；
   - 与菜单共用 `add_boundary_action()` / `add_summary_action()`，一次 `Ctrl+Z` 收回；
   - 概念图不响应：概念图没有概要/边界语义（菜单也不给），不抢键。
2. **失败不静默**：条件不成立时提示条说明原因，不发命令、不动文档：
   - 没有选中：`先选中主题，再添加边界` / `先选中主题，再添加概要`；
   - 选中了根主题（或主选没有父级）：`边界只能圈同一父级下的主题` / `概要只能覆盖同一父级下的主题`。
3. **轨迹**：成功时 `annotation_add kind=boundary targets=N parent=…` /
   `annotation_add kind=summary targets=N parent=…`；失败时 `annotation_add_failed kind=… reason=no-selection|no-parent`。
4. **可发现**：思维导图单节点菜单与多选菜单的「添加概要 / 添加边界」右侧补 `Ctrl+]` / `Ctrl+B`；
   `docs/06` §5 画布交互表补两行，`docs/02` §10 脚注 ⑤ 改成「已补齐」。
5. **金样**：`mm-boundary-key`（选中「禁则换行」→ `Ctrl+B`，边界出现）与
   `mm-summary-key`（多选「禁则换行」「字形图集」→ `Ctrl+]`，概要出现）进 `stage-b` 门。

落地位置：`crates/mw-shell/src/canvas/canvas_methods_3.rs`（键位分发）、
`crates/mw-shell/src/canvas/canvas_methods_1.rs`（动作里补轨迹）、
`crates/mw-shell/src/app_methods_2.rs`（菜单快捷键列）、
`crates/mw-shell/src/canvas/tests.rs`（映射单测）、
`scripts/x100-stage-b-gate.ps1`（两场新场景）。

## 3. 非目标

- 不把概要/边界加进概念图（概念图的结构只由「连线 + 链接词」表达，`docs/30` §5）。
- 不自动进入概要文字编辑：创建后仍按 `F2` / 双击概要编辑（与菜单同一行为）。
- 不改 `Annotation` / `Command` / `.zmind` 字段，不改撤销语义，不新增依赖。
- 不做快捷键自定义/冲突禁用（`docs/02` §10 脚注 ⑥，属设置面，另开轮次）。

## 4. 验收条款

### 4.1 单元（`mw-shell` canvas 纯函数）

| # | 用例 | 期望 |
| --- | --- | --- |
| 1 | `Ctrl+B` | `Some(Boundary)` |
| 2 | `Ctrl+]` | `Some(Summary)` |
| 3 | 其它键（含无修饰的 `B` / `]`） | `None`（不抢普通输入） |

既有 `annotation_targets` 的「根不可注解 / 兄弟可注解」断言继续覆盖目标判定。

### 4.2 端到端（轨迹 + 金样）

- 选中「禁则换行」→ `Ctrl+B`：轨迹 `annotation_add kind=boundary targets=1 parent=…`；
  边界出现、文档 `revision` +1；`mm-boundary-key`。
- 多选「禁则换行」「字形图集」→ `Ctrl+]`：轨迹 `annotation_add kind=summary targets=2 …`；
  概要出现、覆盖两颗主题；`mm-summary-key`；一次 `Ctrl+Z` 收回。
- 只选根 → `Ctrl+B`：`annotation_add_failed reason=no-parent` + 提示条，`revision` 不变。
- 无选中 → `Ctrl+]`：`annotation_add_failed reason=no-selection` + 提示条。
- 概念图 → `Ctrl+B`：无命令、无轨迹、画布不变。

### 4.3 既有基线

- 菜单「添加概要/添加边界」两行右侧多了快捷键文字：`menu` / `menu-narrow` **有意重拍**，
  漂移量与 bbox 记入 §5；其余场景零漂移。
- 不按键时不进入任何新分支，画布渲染逐像素不变。

## 5. 证据

| 类型 | 内容 |
| --- | --- |
| 单测 | `annotation_shortcuts_map_only_exact_ctrl_b_and_bracket`（B/]、无修饰、Ctrl+Shift+B、Ctrl+A 四组）、`annotation_shortcut_errors_distinguish_no_selection_from_root`；既有 `the_root_cannot_be_annotated` 继续钉目标判定。测试 1067 → **1069**（`mw-shell --bins` 605 → **607**） |
| 轨迹 | 成功：`canvas_click hit=…` → `annotation_add kind=boundary targets=1 parent=…` / `annotation_add kind=summary targets=2 parent=…`；失败：`annotation_add_failed kind=boundary reason=no-parent`、`… kind=summary reason=no-selection`；概念图两次按键 **0 条** `annotation_add`（守卫先返回）。全文 `shots/x100/M24/trace.txt` |
| 金样 | `mm-boundary-key`（Warmup 22，sha256-12 `8AD212179A5B`）与 `mm-summary-key`（Warmup 26，`B5EA2970F477`）。对照：边界帧 vs 「只选中禁则换行」差 **10 579 px / 0.6047%**（bbox `(13,19)–(1487,1074)`：标题栏未保存点、边界环 1286–1487、状态栏）；概要帧 vs 「两颗多选」差 **4 954 px / 0.2832%**（标签 1384–1568 一带的花括号 + 概要主题盒）；两张基线互差 **24 576 px / 1.4047%**（同一区域两种注解） |
| 菜单重拍 | 「添加概要/添加边界」右侧多了 `Ctrl+]` / `Ctrl+B`：`menu` 差 **581 px / 0.0332%**（bbox `(819,651)–(873,709)` 恰为两行快捷键列）、`menu-narrow` 差 **579 px / 0.0874%**（`(820,484)–(874,542)`）；其余区域 0 px，无滚动条变化 |
| 门 | stage-b **PASS**：`10:32:15–10:39:19`，**131 场 / 262 次比较**两遍 `--tol 0`，含 fmt / clippy 子集 / 全量测试 1069；stage-cm **PASS**：`10:41:01–10:44:09`，**58 场 / 116 次比较**两遍 `--tol 0`，概念图场景零漂移 |

补充：两处失败路径的提示条文案分别是「边界只能圈同一父级下的主题」与「先选中主题，再添加概要」；
概念图用 `MW_CONCEPT_DEMO=1` 点「产品定位」后连按两键，画布与文档都没有变化（轨迹 0 条）。
`Ctrl+Shift+B` 不建边界（`take_exact_key` 精确匹配），普通 `B` / `]` 仍走落字即编辑。

## 6. 未做 / 风险

- `Ctrl+]` 在部分中文输入法里可能是「切换全半角」，实机归属未核（与 M4 同缺口）。
- 只选一个子主题也能建概要（覆盖一颗），与菜单一致；是否要提示「概要通常覆盖多颗」留给后续。
- 概要在折叠子树里的可见性沿用既有渲染，不改。
