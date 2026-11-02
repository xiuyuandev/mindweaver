# M30b · 缩略卡双击定位节点

> 打磨轮 58 · 2026-11-01
> 账本：`docs/x100/polish-ledger.md`（M30 行的「未做」项）
> 合同：`docs/13` §4.19 的旧实现口径（「点击/拖动把指针映射回世界坐标」）与
> `crates/mw-ui/src/overview.rs` 头部注释：「双击缩略卡定位**某个节点**时，
> 壳层应走 `reveal_center` 而不是 `center_from_thumb`——那是『露出目标』，不是『拖视口框』」。

## 1. 为什么是这一轮

第 55 轮（M30）把缩略卡接回画布，但只做了「拖视口框」：卡内点击/拖动都是
`center_from_thumb`（把视口整框搬过去）。大文档里更常见的诉求是「看到缩略图上那颗点，
直接跳到它」——这也是纯几何层注释里已经写明、却一直没接上的路径。

本轮只补这一条交互：**双击卡内的一颗节点代理** → 选中该节点 + 把相机移到它
（走 `Canvas::reveal_id`，与搜索跳转同一条「露出目标」纪律）。

## 2. 范围

- `mw_ui::overview::node_at(scene, world, pad_world) -> Option<&str>` 纯函数：
  世界点落在节点盒（可外扩容差）内时返回最近中心的那颗；卡上像素只有 1–2px，
  按 `4 css px / map.scale` 折成世界容差。
- `minimap_panel` 里 `resp.double_clicked()`：命中节点 → `canvas.select_node(id)` +
  `canvas.reveal_id(id)`，轨迹 `minimap locate id=… hit=1`；未命中记 `hit=0`，相机不动
  （双击空白仍按普通点击处理，视口框照旧搬过去）。
- 新基线 `mm-minimap-locate`：`MW_MINIMAP=1` + 双击卡片中心（根节点附近）；
  选中环 + 相机居中。
- 单测：`node_at` 命中盒内 / 容差内 / 容差外 / 按钮空白；`minimap_panel` 的交互本身
  仍靠金样与轨迹。

## 3. 非目标

- 不改拖动/滚轮/折叠行为；不改 `Scene` / `Frame` ABI。
- 不做「悬停缩略卡显示节点名」、不做多选、不做双击空白新建。
- 不改 `.zmind` / `Node` / `NoteCard`。

## 4. 验收场景

1. **定位**：双击缩略卡中心附近 → 选中根节点、相机中心 = 该节点盒中心（轨迹
   `minimap locate id=… hit=1`）；与「只拖动视口框」的基线不同。
2. **空白**：双击卡内空白（远离任何块）→ `hit=0`，不选中、相机按拖视口框走。
3. **回归**：`mm-minimap` / `mm-minimap-collapsed` 零漂移；收起态角标仍只负责展开。
4. **纯函数**：容差内最近中心胜出，容差外 `None`。

## 5. 预计证据

- `shots/golden/mm-minimap-locate/` 新基线（`--tol 0` 双跑）。
- 轨迹 `minimap locate id=… hit=…`。
- `stage-b` 143 → 144 场 / 286 → 288 次比较；`stage-cm` 64/128 顺序复跑。

## 6. 实测结果（2026-11-01）

- **命中**：双击卡片内容区中心 `(1810,1057)` → `minimap locate id=… hit=1`，
  命中根节点「织思」；相机随后 `reveal_id` 居中。轨迹里同一次双击先落一条
  `minimap pan`（双击的第一下仍是普通点击），最终取景由 `reveal_id` 覆盖。
- **空白**：双击卡内左上空白 `(1724,1002)` → `minimap locate hit=0`，不选中，
  相机按拖视口框走。
- **像素**：`mm-minimap-locate` ↔ `mm-minimap`（不双击）差 **274 314 px**，
  bbox `(14,58)–(1788,1194)`——选中环 + 相机居中（整张画布移动），符合预期；
  新基线双跑 `--tol 0` 零差异。
- **既有基线**：`mm-minimap` / `mm-minimap-collapsed` 不含双击输入，代码路径不变，
  门内验证零漂移。
- **单测**：`mw-ui::overview::tests::node_at_hits_the_box_and_honours_the_pad`
  （盒内必中、4 容差外不中、12 容差内中、远处不中）；测试 1122 → **1123**。

## 7. 门内插曲

第一遍 stage-b 门 `cm-default`（无 `MW_MINIMAP`、不走本轮任何代码路径）在 pass 1 差
**1 px**（max 82，bbox `(1219,392)`），pass 2 全过；该场景单跑三连 `--tol 0` 全过。
判定为 GPU 抗锯齿覆盖度的既有偶发（与本轮改动无关），重跑整门确认后才记绿。

## 8. 未做 / 风险

- 双击空白新建概念未做（概念图惯例，但本轮不引入新命令）。
- 悬停缩略卡显示节点名未做。
- 实机鼠标双击节奏 / 150% DPI 未核。
