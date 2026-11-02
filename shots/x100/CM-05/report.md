# CM-05 概念图交互 · 报告

> 状态：**已实现**（本包范围；G-CM-1 门未开）
> 设计合同：`docs/30-外壳体验与概念图设计规范.md` §5.3 §5.5 §5.7 §5.9
> 执行合同：`docs/31-外壳体验与概念图工作包.md` §6
> 完成：2026-10-26 · 依赖 CM-03 / CM-04

## 1. 起点（审计结果）

概念图的「交互骨架」大半借自思维导图，能点能拖，但**语义是错的**——树的手势被套在网络结构上：

| 能力 | 审计时的现状 | 问题 |
| --- | --- | --- |
| 选中 / 多选 | 框选、`Ctrl` 点选、`Alt` 减选都在（`mw_ui::Selection`） | 可用；但没有概念图自己的「全选」入口 |
| 方向键 | `Canvas::navigate` 按屏幕几何**换选**下一个节点 | §5.7 要的是**微调位置**：概念图没有「上/下一个」，只有「往右挪一点」 |
| `Shift` + 方向 | 被 `Modifiers::NONE` 的 `consume_key` 一并吃掉，行为同方向键 | 8px 步长根本不存在 |
| 右键菜单（节点） | 思维导图口径：新建子主题 / 新建同级 / 添加概要 / 添加边界 / 分离为自由主题 / 折叠 / 聚焦 | 概念图里**全部无意义**，其中「新建子主题」会把树语义直接写进命题网 |
| 右键菜单（空白） | 已加「新建概念」+「整理布局」（CM-04） | 仍带「全部展开 / 全部折叠」——概念图没有折叠 |
| 新建概念 | 复用 `AddFloatingTopic`，占位文字写死 `"自由主题"` | 概念是名词短语，「自由主题」是树的口径 |
| 删除概念 | `Command::DeleteNodes` **一律拒绝根节点**（`RootProtected`） | 概念图的主概念 = 第一个概念，不是树根，用户删它必然失败 |
| 删连线 | `Delete` 先删选中连线，再删注解，最后删节点 | 可用（CM-03 已钉） |
| 撤销 / 重做 | 全部走 `Command` + `ChangeSet` | 可用；但 `ChangeSet` 不带 `root_id`，删主概念的重挂无处可回滚 |

## 2. 范围（本包做的七件事）

| # | 缺口 | 处置 | 落点 |
| --- | --- | --- | --- |
| 1 | 方向键语义错 | `nudge_concepts(dx, dy)`：概念图里方向键 = **移动选中概念**，1px；`Shift` = 8px；多选整组移动；**一条 Batch 一次撤销**；trace `concept-nudge n=… dx=… dy=…` | `canvas_methods_3.rs`、`consts.rs::nudge_step` |
| 2 | 右键菜单口径错 | 概念图单独一套：节点（新建概念 / 编辑概念 `F2` / 整理布局 / 剪切复制粘贴 / 删除概念 / 关联卡片笔记 / 打开样式检查器 / 适应画布）、多选（整理布局 / 剪贴板 / 删除概念 / 排列）、空白（去掉展开折叠）、连线（沿用） | `app_methods_2.rs::build_context_menu` + `arrange_submenu()` |
| 3 | 主概念删不掉 | `DeleteNodes` 在 `kind == "conceptMap"` 时允许 `ids` 含主概念；删完把根重挂到剩下概念里**最靠左上**（`y`→`x`→`id`）的那个；`ChangeSet.before_root / after_root` 让撤销/重做把根一起还原；**最后一个概念不可删** | `document.rs`（`DeleteNodes`、`surviving_concept`、`ChangeSet`）、`canvas_methods_3.rs::delete_selected` |
| 4 | 新建概念占位文字 | 概念图里占位「新概念」；思维导图保持「自由主题」 | `canvas_methods_1.rs::add_floating_world` |
| 5 | 全选入口缺失 | 概念图 `Ctrl+A` 选中全部概念（主选不变、清掉连线/注解选择）；思维导图**不接**这个键 | `canvas_methods_3.rs::select_all_concepts` |
| 6 | `cm-crosslink` 场景缺失 | `MW_CONCEPT_CROSS=1` 交叉连接变体 + `MW_CONCEPT_SELECT=<文字>` 预选（不动相机）+ 新基线 `cm-crosslink` + 脚本 `shift+方向` 5 下 | `app_methods_1.rs`、`shots/golden/cm-crosslink/` |
| 7 | 「删除」按钮可用性 | `can_delete()` 概念图分支：概念数 ≥ 2 且选中里有概念才可删 | `canvas_methods_3.rs::can_delete` |

## 3. 命令契约（本包落地的三条）

| 契约 | 内容 |
| --- | --- |
| 方向键 | 1px / `Shift` 8px，整组一条 `Batch`；不动相机、不闪重叠警示（微调是刻意的，不是失手） |
| 删概念 | 允许删主概念；重挂规则 `(pos.y, pos.x, id)` 取最小；`ChangeSet` 带根快照；删到 0 个返回 `RootProtected` |
| 菜单 | 概念图菜单不含「新建子主题 / 新建同级 / 概要 / 边界 / 分离 / 折叠 / 聚焦 / 全部展开 / 全部折叠」 |

## 4. 非目标

- **不做交叉处 ≤8px 微弧**（CM-03 挂账）：微弧要改 `Scene` 的边路径，会让 `cm-default` 全部边跟着漂；
  本包只保证「交叉可以存在且渲染稳定」（`cm-crosslink` 基线锁住）。
- 不做概念图专用工具条（§5.9 禁止第二套工具条）。
- 不做「按连线方向键跳转」——概念图的键盘是**摆位**，不是遍历。
- 不改 `.zmind` 版本与字段；`root_id` 重挂只换指针。
- 不做卡片联动（CM-06）、大纲（CM-07）、导出（CM-07）。

## 5. 验证

| 项 | 结果 |
| --- | --- |
| 新单测 | `mw-core`：删主概念换根 + 撤销还原 + 重做再删（`deleting_the_first_concept_rehangs_the_root_and_undo_restores_it`）、删最后一个概念被拒（`deleting_the_last_concept_is_rejected`，含「思维导图根仍受保护」）、整组微调一条撤销（`nudging_a_group_of_concepts_is_one_undo_step`）、换根后场景坐标不移位（`scene_keeps_each_concept_at_its_pos_after_the_root_swap`）；`mw-shell`：`concept_nudge_steps_are_one_and_eight` |
| 反例 | 把 `cs.with_root(...)` 临时换成不做快照 → `deleting_the_first_concept…` 在「撤销后根要还原」处断红；恢复后转绿。这条断言是真的会失效的。 |
| `cargo fmt --all -- --check` | 干净 |
| `cargo clippy -p mw-shell --all-targets` | 新增 0（mw-shell 4 条全部是历史：`export_pdf.rs` 3 + `panels/outline.rs` 1，本包未碰这两个文件） |
| `cargo test --workspace -- --test-threads=1` | **801 passed / 0 failed**（796 + 5：4 条 core + 1 条 shell） |
| 机器门 | `MACHINE GATE PASS`：**35 组比较 ×2 遍 `--tol 0` 全 0 差异**（`gate-summary.txt`，终局 **02:38:26**，加完换根场景测试后重跑的一遍；脚本 `scripts/x100-stage-b-gate.ps1`） |
| 旧基线 | `cm-default` `5b0d9dc4…`、`cm-narrow` `fc7f31f1…`、`cm-arrange` `56bc4017…`、`menu` `fbfaf8ce…` **逐字节未变** |
| 新基线 | `cm-crosslink` = `shots/golden/cm-crosslink/shell-ppp1.00.png`（1920×1200 @1.0，`f2e948d1b9ac0eca…`），双跑 `--tol 0` 0 差异 |

## 6. 场景与证据

| 文件 | 内容 |
| --- | --- |
| `shots/golden/cm-crosslink/` | `MW_CONCEPT_DEMO=1 MW_CONCEPT_CROSS=1 MW_CONCEPT_SELECT=用户痛点` + `inputs/cm-crosslink.txt`：3×右 + 2×下 = **24/16px**，交叉连接完整可见 |
| `cm-crosslink.trace.txt` | 5 条 `concept-nudge n=1 dx=8.0 dy=0.0 / dx=0.0 dy=8.0`——键盘路径真的走到了命令层 |
| `cm-node-menu.png` | 概念图节点右键菜单实拍：新建概念 / 编辑概念 F2 / 整理布局 / 剪切 复制 粘贴 / 删除概念（无选中时置灰）/ 关联卡片笔记 / 打开样式检查器 / 适应画布；**树语义条目全部消失** |
| `cm-blank-menu.png` | 概念图空白右键菜单实拍（`inputs/cm-blank-menu.txt` 真实右键 700,620）：新建概念 / 粘贴 / 整理布局 / 适应画布 / 100% / 打开文件夹；**没有「全部展开 / 全部折叠」** |
| `gate-summary.txt` | 机器门各步 PASS 与完成时刻 |

## 7. 未做 / 偏差（必须留在纸面上）

1. **微调不做 500ms 合并撤销**：每按一次方向键一条撤销（连按 5 下 = 5 步）。
   `docs/30` §4.3 的合并规则写的是检查器滑杆/stepper，键盘微调按「一次按键一次动作」处理；
   若人工门认为长按连按太碎，下一包再加合并。
2. **1px 步长只有单测**，没有单独基线：1px 画面上不可辨，`cm-crosslink` 只锁 8px 那 5 步。
3. **拖出连线的脚本场景仍缺**（CM-03 挂账）：拖拽脚本要按相机算世界→屏幕坐标，与 `cm-link-create` 同一笔账，留 CM-08。
4. **交叉处 ≤8px 微弧未做**（CM-03 挂账，见 §4）：`docs/30` §5.4 最后一条仍未落地。
5. **删掉主概念后选择被清空**：思维导图删节点是「落到父级」，概念图没有父级，退化为清空；
   若要「落到新主概念」，得在 `delete_selected` 里记一次换根前后映射，本包按「宁空不错」处理。
6. **菜单里的「新建概念」用指针当前位置**（与思维导图「新建自由主题」同源，`latest_pos`）；
   若指针已移出画布，落点会退到最后一次位置。
7. **`Ctrl+A` 只在概念图生效**：思维导图的选择语义受折叠与聚焦影响，一把全会把隐藏节点也选进去，不在本包范围。
8. **`MW_CONCEPT_SELECT` 是演示入口**，只在 `MW_CONCEPT_DEMO` 之后生效；它不是产品功能。
