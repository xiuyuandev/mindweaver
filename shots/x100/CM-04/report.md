# CM-04 自由摆放与整理布局 · 报告

> 状态：**已实现**（本包范围；G-CM-1 门未开）
> 设计合同：`docs/30-外壳体验与概念图设计规范.md` §5.5 §5.7 §5.9
> 执行合同：`docs/31-外壳体验与概念图工作包.md` §6
> 开始：2026-10-26 · 依赖 CM-02

## 1. 起点（审计结果）

| 能力 | 现状 |
| --- | --- |
| 拖拽落 `pos` | 松在**空白处**时 `detach_on_blank_command` 走 `SetNodePos`——但**主概念（`root_id`）被排除**，`SetNodePos` 本身也拒绝根，于是概念图里第一个概念拖不动 |
| 拖到别的节点上 | 走 `plan_moves` → `MoveNode`，把概念**挂成子节点**——概念图不允许树语义 |
| 整理布局 | 不存在 |
| 重叠提示 | 不存在 |

## 2. 范围（本包做的五件事）

| # | 缺陷/缺口 | 处置 |
| --- | --- | --- |
| 1 | 主概念拖不动 | 新增 `Command::MoveConcept { id, x, y }`：允许 `parent_id = None` 的任意节点（**含根**），但**只在 `kind == "conceptMap"` 的文档里成立**，思维导图的根仍不可摆 |
| 2 | 概念图里拖到节点上会改结构 | 概念图拖拽一律按「落 `pos`」处理：帧内不计算 `drop` 目标，`commit_move` 走概念图专用分支 |
| 3 | 场景不认主概念的 `pos` | `build_scene_cached` 在 `conceptMap` 下把主概念布局整体平移到它的 `pos`，与浮动概念同一套语义 |
| 4 | 没有「整理布局」 | `mw-core/src/concept_layout.rs`：最长路径分层（环按首次访问截断）+ 同层父重心排序 + 层内等距 + 24 轮松弛；`LAYER_GAP = 56`、`NODE_GAP = 24`；整体左对齐到原包围盒左上角；**一条 `Batch` 一次撤销** |
| 5 | 没有重叠提示 | 落点与任何概念相交时，给被拖概念画 1px `content::warn` 描边，0.25s 后自动消失（`overlap_flash`，随帧老化） |

## 3. 整理布局的算法契约

`mw_core::concept_layout::arrange(sizes, edges, current) -> Vec<(f32, f32)>`（纯函数，无窗口/无 egui）：

1. **分层**：DFS 首次访问定层，遇到「正在访问」的边即判为回边并丢弃（环按首次访问截断）；
   再对保留边做最长路径松弛（≤ n 轮），保证 `layer(to) ≥ layer(from)+1`。
2. **排序**：层内初始按当前 `y`/`x`/下标排列；随后 4 趟自上而下按**入边父重心**稳定重排。
3. **初摆**：列宽 = 该层最宽概念，`x` 逐列累加 +56；层内自上而下等距 +24。
4. **松弛 24 轮**：每个概念朝「邻居中心均值」靠一半，再按层内顺序压开（间隙 ≥24），
   整块回到目标重心（不低于锚点）。
5. **收口**：再压一遍保证不重叠；整体平移到原包围盒左上角（左对齐）。

## 4. 非目标

- 不做交叉处 ≤8px 微弧（CM-03 已挂账，留 CM-05）。
- 不做「整理选中概念」——本包只整理整图；选中子集排序属 CM-05 交互。
- 不做连线的拖动改目标、不做卡片联动（CM-06）、不做大纲（CM-07）。
- 不改 `.zmind` 版本/字段；`MoveConcept` 只写既有 `pos`。

## 5. 验收

| 项 | 结果 |
| --- | --- |
| 新单测（`concept_layout`） | 分层/环安全/不重叠/最小间隙/确定性/单节点/空输入 |
| 命令测试 | `MoveConcept` 在概念图里能移动根；在思维导图里拒绝根；撤销回原位 |
| `cargo fmt --all -- --check` | 干净 |
| `cargo clippy -p mw-shell --all-targets` | 新增 0（只剩 `export_pdf.rs` 3 条历史） |
| `cargo test --workspace -- --test-threads=1` | **796 passed / 0 failed**（790 + 6：`concept_layout` 5 条、`move_concept_only_works_in_concept_maps` 1 条） |
| 机器门 | `MACHINE GATE PASS`：34 组比较 ×2 遍 `--tol 0` 全 0 差异（`target/x100/stage-b-gate.log`，终局一跑完成 2026-10-26 02:13:54 +08:00；摘要见 `gate-summary.txt`） |
| 新旧基线 | `cm-default` / `cm-narrow` 逐字节未变（本包未改它们的外观）；新增 `cm-arrange` = `shots/golden/cm-arrange/shell-ppp1.00.png`（1920×1200 @1.0） |

## 6. 场景与证据

| 文件 | 内容 |
| --- | --- |
| `cm-default`（`shots/golden/cm-default/`） | 整理前的示例概念图（既有权重，本包未改其外观） |
| `cm-arrange`（`shots/golden/cm-arrange/`） | `MW_CONCEPT_DEMO=1 MW_CONCEPT_ARRANGE=1` 启动即整理，取景按新包围盒 |
| `cm-blank-menu.png` | 概念图空白处右键菜单实拍：「新建概念」+「整理布局」 |
| `cm-arrange-click.png` | **走真实菜单**（`inputs/cm-arrange-click.txt`：右键空白 → 点「整理布局」）的整理结果 |
| `cm-arrange-click.trace.txt` | 同一轮的 trace：`menu_open anchor=700,620 entries=12` → `concept-arrange n=6 moved=6` |

**最强的一条证据**：`cm-arrange-click.png` 与基线 `cm-arrange/shell-ppp1.00.png`
**逐字节相同**（`56BC40170F3DE479…`）——菜单路径与验收场景走的是同一份代码、同一个结果。
（顺便纠了一个坐标坑：`MW_GOLDEN_INPUT` 的坐标是**物理像素**，1280×800@1.5 下点菜单要点 800,720。）

## 7. 未做 / 偏差（必须留在纸面上）

1. **拖拽落 `pos` 没有脚本化场景**：菜单路径已由 `cm-arrange-click` 实拍 + trace 钉住，
   但「按住概念拖到新位置」只有单测（`MoveConcept` 的语义/撤销）与人工试用覆盖；
   拖拽脚本要按相机算世界→屏幕坐标，与 CM-03 的 `cm-link-create` 同一笔账，留 CM-08。
2. **重叠提示没有 golden**：0.25s 的瞬时描边与帧率耦合，离屏截图抓不稳；
   实现按 `input.time` 老化，人工试用验证，不拍基线。
3. **整理结果不做动画**：一次命令直接落新位置（`docs/30` §5.5 未要求过渡动画）。
4. 整理只处理整图；选中子集的整理/对齐仍走既有「排列」子菜单（自由主题口径）。
5. **入口只有空白处右键**：单概念右键菜单仍是思维导图口径（新建子主题/分离为自由主题…），
   本轮没往里插「整理布局」——那一插会顶开「样式」子菜单的位置，`menu` / `menu-narrow`
   两张基线跟着漂。概念图自己的菜单整套留给 CM-05 统一重做。
6. `MoveConcept` 的 `kind` 判定放在 `Document::apply` 里读 `meta.layout.kind`：
   若将来有人把概念图另存成 `mindmap` 但保留浮动概念，这些概念会退化成
   「只能整块拖动」的旧行为（不会崩，也不会坏数据）。
