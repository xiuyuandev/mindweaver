# M26 · 选中互斥（节点 / 关系线 / 注解）（打磨轮 49）

- 日期：2026-10-31
- 依据：`HANDOFF.md` §7.5 第 5 条「**未做**：……节点与关系线选中互斥」；
  `docs/30` §5.4（关系线选中）；`docs/06` §5 画布交互表；
  `canvas.rs` 里 `annotations` 字段的既有注释「与节点选择互斥：选中注解时节点选择会清空」。
- 状态：**已实现**（机器门绿；人工门未开）

## 1. 现状（代码复核 + 复现，先于改动）

画布有**三条选择道**：

| 道 | 字段 | 选中时画什么 |
| --- | --- | --- |
| 节点 | `sel: Selection` | 节点 8% 强调底 + 1.5px 环 |
| 注解 | `annotations: Vec<AnnotationSel>` | 概要/边界 2px 强调环 + 跨度手柄 |
| 关系线 | `relationship: Option<RelationshipSel>` | 3px 强调线 + 两端手柄 |

设计意图是三选一（注释与 `esc_plan` 都按这个前提写）。**大部分入口确实做到了**：
点节点 / 点关系线 / 点注解 / 右键 / `select_node` / `select_many` / 全选 / 搜索跳转
都会清掉另外两条道。

但有四个入口只改了自己那条道：

1. `navigate()`（方向键遍历，`canvas_methods_1.rs:612`）只 `self.sel = …`，
   不清关系线与注解；
2. 框选 / 套索松手（`canvas_methods_2.rs:1677`、`:1694`）只 `sel.replace(…)`，
   不清关系线与注解；
3. `select_ids()`（切回标签页时恢复选中，`canvas_methods_1.rs:846`）只 `sel.replace(…)`；
4. `select_annotation()`（`canvas_methods_2.rs:1878`）本身不清关系线，靠调用方；
   `select_relationship_by_label()`（验收夹具）也不清节点选择。

### 复现（改前，release 二进制，默认示例文档）

脚本：选中「中文排版」→ `Ctrl+L` → 点「交互与性能」建线并选中 →
按两次 `ArrowRight`（关系线仍选中时，`navigate` 从根走到子节点）→ `Delete`。
轨迹（`trace-before.txt`，节选）：

```text
event canvas_click at=(1167,503) … hit=01a19020-…-c86a701120ad
event canvas_click at=(626,634)  … hit=01a19020-…-c86e77489bda
event select relationship 01a19020-…-c86a701120ad 01a19020-…-e1cd-71d2-a89c-3bb483d51c5b
event reveal idx=0 cam=(-78,0) z=1.33 …      # 第一次 ArrowRight：节点选择落根
event reveal idx=4 cam=(-78,0) z=1.33 …      # 第二次 ArrowRight：节点选择落子节点
event delete relationship 01a19020-…-c86a701120ad 01a19020-…-e1cd-71d2-a89c-3bb483d51c5b
event delete-confirm pending=4 selected=1
```

**一次 `Delete` 跨了两条道**：关系线当场删除（不入确认），同时把刚选中的节点
（带 4 个节点的子树）立起删除确认框。改前像素证据：
`before-arrow.png`（关系线强调与节点选中环并存，状态栏「已选: 中文排版」）、
`before-delete.png`（关系线已消失 + 4 节点删除确认框）。

同样的洞也存在于注解道：选中概要/边界后按方向键或框选，注解环会与节点选中环并存；
下一次 `Delete` 会先删注解、再立节点删除确认。

### 改后（同一脚本，release 二进制）

```text
event select relationship …（建线）
event selection_clear lane=node reason=select-relationship
event selection_clear lane=relationship reason=navigate   # ← 方向键接手，关系线让位
event reveal idx=0 …
event reveal idx=4 …
event delete_dispatch lane=nodes                          # ← 一次只走一道
event delete-confirm pending=4 selected=1                 # ← 没有 delete relationship
```

像素账（1620×1080，逐像素零容差）：改前/改后 `mm-rel-select-arrow` 差 **1628 px**，
全落在关系线包围盒 `(710,515)–(1094,619)` 内；`mm-rel-select-delete` 差 **163 px**，
落在 `(1045,517)–(1095,526)`——确认框盖住了线的其余部分，只露出「中文排版」左缘这一段，
正是改前被删掉、改后留下来的那截线。

## 2. 范围

1. **把「三选一」写成可测的规则**：`SelectionLane { Node, Annotation, Relationship }`、
   `foreign_lanes(keep, relationship, annotations, nodes) -> [bool; 3]`，
   返回除 `keep` 外、当前非空、必须清掉的道。
2. **一个清场方法** `Canvas::clear_foreign_selection(keep, reason)`：按 `foreign_lanes`
   清字段，只在实际清掉时写轨迹 `selection_clear lane=… reason=…`。
3. **补上漏掉的入口**：
   - `navigate()` → `keep=Node, reason=navigate`；
   - 框选松手 → `reason=marquee`；套索松手 → `reason=lasso`；
   - `select_ids()`（阅读模式退出/标签恢复）→ `reason=select-ids`；
   - `select_annotation()` → `keep=Annotation, reason=select-annotation`，
     同时把节点选择一起清掉（返回值改为「是否有变化」，单击路径据此置脏）；
   - `select_relationship()` / `select_relationship_by_label()` →
     `keep=Relationship, reason=select-relationship[-label]`，不再各写一份清场代码。
4. **`Delete` 单道分派**：抽 `delete_lane(relationship, annotations, nodes)`，
   一次只走一条道：关系线 → 注解 → 节点 → 无。即使将来又出现两条道同时非空，
   一次 `Delete` 也**不可能**删两类东西；落地时写 `delete_dispatch lane=…`。
5. **轨迹**：`selection_clear lane=… reason=…`、`delete_dispatch lane=…`；
   既有 `select relationship` / `delete relationship` / `delete-confirm` 保持不变。
6. **两场新基线**：
   - `mm-rel-select-arrow`：关系线选中后按两次 `ArrowRight`。改前两道并存；
     改后线回到未选中态，只剩节点环与节点底。
   - `mm-rel-select-delete`：再按 `Delete`。改前线消失 + 4 节点确认框；
     改后线仍在 + 确认框，且轨迹没有 `delete relationship`。
7. **单测**：`foreign_lanes` 三条道 × 是否非空的组合；`delete_lane` 的优先级与「只有一条」。
8. **文档**：`docs/06` §5 补一行「选中互斥 / Delete 只删当前道」；
   `docs/30` §5.4 补同一句合同（关系线选中与概念/注解互斥）。

落地位置：`crates/mw-shell/src/canvas.rs`（三条道的规则与纯函数）、
`crates/mw-shell/src/canvas/canvas_methods_1.rs`（`navigate` / `select_ids`）、
`canvas_methods_2.rs`（框选/套索/注解/关系线选中）、
`canvas_methods_3.rs`（`clear_foreign_selection` / Delete 分派）、
`canvas/tests.rs`（两条单测）、`scripts/x100-stage-b-gate.ps1` + 两个输入脚本。

## 3. 非目标

- 不改选中/悬停的视觉（底色、描边、手柄、关系线强调都不动）。
- 不新增「关系线多选」；注解多选仍只在注解道内。
- 不改 `Ctrl+C/X/V`、`Ctrl+A`、`Esc` 的既有语义；`Esc` 的两级顺序保持
  「插入态 → 关系线 → 聚焦 → 节点选择」。
- 不把关系线/注解选中带进大纲或检查器的分组结构；检查器继续按当前道显示。
- 不改 `Node` / `.zmind` 字段，不加依赖，不改撤销粒度。
- 不处理「点折叠圆点时注解读数保留」这类**非选择手势**的既有状态（本轮不扩大范围）。

## 4. 验收条款

### 4.1 单元（`mw-shell` 纯函数）

| # | 用例 | 期望 |
| --- | --- | --- |
| 1 | `foreign_lanes(Node, true, 2, 3)` | `[true, true, false]`（清关系线 + 注解，保留节点） |
| 2 | `foreign_lanes(Annotation, true, 2, 3)` | `[true, false, true]` |
| 3 | `foreign_lanes(Relationship, true, 2, 3)` | `[false, true, true]` |
| 4 | 三条道都空 / 只有本道非空 | 全 `false` |
| 5 | `delete_lane(true, 2, 3)` | `Relationship`（绝不落到注解/节点） |
| 6 | `delete_lane(false, 2, 3)` | `Annotation` |
| 7 | `delete_lane(false, 0, 3)` | `Nodes` |
| 8 | `delete_lane(false, 0, 0)` | `Nothing` |

### 4.2 端到端（轨迹 + 金样）

- 关系线选中 → `ArrowRight` ×2：轨迹出现
  `selection_clear lane=relationship reason=navigate`；改前 `before-arrow.png` 与改后基线
  `mm-rel-select-arrow` 只在关系线强调上不同（节点选中态一致）。
- 接 `Delete`：轨迹为 `delete_dispatch lane=nodes` + `delete-confirm pending=4 selected=1`，
  **没有** `delete relationship`；基线 `mm-rel-select-delete` 里关系线仍在。
- 只选中关系线按 `Delete`：`delete_dispatch lane=relationship` + `delete relationship`，
  不出现 `delete-confirm`，节点数不变（跑一次轨迹核对）。
- 只选中注解按方向键：`selection_clear lane=annotation reason=navigate`，注解环消失。
- 关系线选中时框选：`selection_clear lane=relationship reason=marquee`。
- 既有基线零漂移：不涉及两条道并存的场景，画面不应变化。

### 4.3 证据

`shots/x100/M26/`：`before-arrow.png` / `before-delete.png`（改前）、
`after-arrow.png` / `after-delete.png`（改后）、
`trace-before.txt` / `trace-before-arrow.txt`（改前）、
`trace-after-arrow.txt` / `trace-after-delete.txt` / `trace-after-rel-delete.txt`、
`trace-after-annot-arrow.txt` / `trace-after-marquee.txt`、报告与 `manifest.json`；
金样在 `shots/golden/mm-rel-select-arrow/` 与 `shots/golden/mm-rel-select-delete/`。

### 4.4 实测（2026-10-31）

- 单测：`selection_lanes_keep_exactly_one_lane`、`delete_press_reaches_exactly_one_lane` 2 条；
  全量 `cargo test --workspace -- --test-threads=1` **1073 passed / 1 ignored / 0 failed**。
- 只选中关系线按 `Delete`：`delete_dispatch lane=relationship` + `delete relationship`，
  没有 `delete-confirm`（`trace-after-rel-delete.txt`）。
- 只选中注解按方向键：点概要主题 → `select summary` + `selection_clear lane=node
  reason=select-annotation`，`ArrowRight` 后 `selection_clear lane=annotation reason=navigate`
  （`trace-after-annot-arrow.txt`）。
- 关系线选中时框选：松手帧 `selection_clear lane=relationship reason=marquee` +
  `marquee picked=1 selected=1`（`trace-after-marquee.txt`）。

## 5. 风险 / 未做（收口实测）

- 注解道与框选道只锁轨迹 + 改前截图，没有再各加一场金样（两场已经覆盖共享的清场代码）。
- `select_ids` 只在阅读模式退出时被调用；标签切换恢复选中若将来走别的入口，需要同一规则。
- 关系线选中态的像素差异较小（3px 强调线），漂移表按逐像素统计给出，不做主观描述。
- 机器门：`stage-b` **136 场 / 272 次比较**（2026-10-31 `12:13:36–12:21:00`，含 fmt、clippy
  子集、全量测试 1073、两遍 golden）、`stage-cm` **58 场 / 116 次比较**（`12:21:23–12:24:34`）
  两遍 `--tol 0` 全过；既有 134 场**零净漂移**。新场景 sha256-12：
  `mm-rel-select-arrow` = `C7143666B0EE`、`mm-rel-select-delete` = `E9028C9C82F7`。
- 未做：关系线多选、实机键鼠节拍、150%/200% DPI 下的强调线辨识。
