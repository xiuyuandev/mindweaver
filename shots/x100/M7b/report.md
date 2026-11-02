# M7b · 框选的 Alt 减选与自由套索（证据闭环）

> 打磨轮 62 · 2026-11-01
> 账本：`docs/x100/polish-ledger.md` M7 行「已知粗糙：**Alt 减选与套索没拍**」
> 合同：`docs/02` §2「多选：画布空白处拖拽框选；`Ctrl+点击` 加选」；`docs/06` §5

## 1. 为什么是这一轮（先把语义读清楚）

M7 只拍了矩形框选与 `Ctrl` 加选；「Alt 减选」与「自由套索」两件事在代码里存在、
却没有任何轨迹或基线。本轮先读实现、再用两张脚本钉住真实语义——**读下来的结果与
直觉不同**，所以先把结论写在前面：

| 手势 | 真实行为 |
| --- | --- |
| 空白处普通拖 | 矩形框选（`DragKind::Marquee`） |
| 空白处 **Alt** + 拖 | **自由套索**（`DragKind::Lasso`，采样点距上一点 > 2px 才收） |
| 套索松手 | 圈中的节点**从当前选择里去掉**（减选）；圈空**不清空**选择 |
| 矩形拖拽 + Alt | **指针上不可达**：按下带 Alt 已被改判成套索 |

也就是说：`subtract` 与 `Lasso` 都读按下时的 `alt`，套索在真实手势里**恒为减选**，
**不能建立新选择**；矩形框选的 `subtract` 分支只由「套索」与「Alt+单击节点」走到；
拖动中再按 Alt 不生效（修饰键只在按下时读一次）；`subtract` 优先于 `additive`
（`Ctrl+Alt` 同按只减）。

## 2. 范围

- 抽纯函数 `canvas::apply_lasso(scene, camera, viewport, rect, points)`：
  把「多边形命中 → 减选 → 圈空返回 `None`」写成可单测的形状；壳层松手分支改用它，
  **行为零变化**，只补轨迹 `marquee picked=N selected=M path=lasso`（矩形那条原样）。
- 两张输入脚本 + 两条金样：`mm-lasso`（套索圈两颗）、`mm-marquee-alt`
  （先框 3 颗，再用套索减掉 1 颗）。
- 对照：既有 `mm-marquee-add`（Ctrl 加选）与矩形轨迹零漂移。

## 3. 非目标

- **不改**「套索不能建选」「无实时选中预览」这两个行为——它们是产品语义问题，
  要改先动 `docs/02`/`docs/06` 并单开一轮（见 §7）。
- 不改 `docs/06` §5「Alt+拖拽 = 记录自动布局偏移」与实现的矛盾（同一笔记账）。
- 不改矩形框选、`Ctrl` 加选、悬停、拖拽的既有几何。

## 4. 验收场景

1. Alt 拖多边形圈住两颗 → `marquee picked=2 selected=0 path=lasso`，节点选中数 0。
2. 先矩形框选 3 颗，再 Alt 套索只圈 1 颗 → `marquee picked=1 selected=2 path=lasso`。
3. 套索圈空 → 选择不变（`apply_lasso` 返回 `None`，调用方不清空）。
4. 对照 `mm-marquee-add` 与矩形轨迹逐字不变。

## 5. 实测结果（2026-11-01）

- **实现**：`canvas.rs` 新增 `apply_lasso`（减选 + 圈空 = `None`，行为不变）；
  `canvas/canvas_methods_2.rs` 松手分支改用它并加轨迹；`canvas/tests.rs` +2：
  `lasso_pick_maps_the_polygon_through_the_camera`、
  `lasso_subtract_only_removes_looped_nodes`。
- **脚本**（ppp 1.0 · warmup 30 · 与 `mm-marquee` 同夹具；落定相机 1620×1008、zoom≈1.327）：
  - `shots/golden/inputs/mm-lasso.txt`：Alt 起笔 (450,340) → (170,340)/(170,500)/(450,500)，
    圈住 `winit`(320,387) 与 `wgpu`(309,469) → `marquee picked=2 selected=0 path=lasso`。
  - `shots/golden/inputs/mm-marquee-alt.txt`：矩形 (450,340)→(180,590) 先选 3 颗
    `picked=3 selected=3` → Alt 套索 (450,430)/(180,430)/(180,500)/(450,500) 只圈 `wgpu`
    → `picked=1 selected=2 path=lasso`。
  - 两次复跑除节点 UUID 外逐字一致；对照组 `mm-marquee-add` 零差异、矩形轨迹未变。
- **单测**：`cargo test -p mw-shell` 631 → **637**（本包 +2，另 +4 来自并行代理）。
- **金样**：`mm-lasso`（停在松手前，锁套索多边形正在画的样子，sha256-12 `870979712E6C`）与
  `mm-marquee-alt`（先框 3 颗再套索减 1 颗，末帧剩 2 个选中环，`209ADC44E298`）
  两张基线 `--tol 0` 双跑零差异，已加进 stage-b 门（Warmup 30、`MW_NOTES=none`）。
  「空组套索圈中两颗」的结算轨迹另存 `shots/x100/M7b/trace-lasso-commit.txt`
  （与 `mm-lasso` 同坐标、末尾补 `release`，不入金样门）。
- **起笔坐标踩坑**：概要「渲染底座」主题盒 `(77,408,96,41)` 属于「纯 Rust 原生」，
  点在它上面会变成拖节点；脚本已避开。

## 6. 未做 / 风险

- **套索不能建立新选择**（空组里圈中也是 `selected=0`）、**无实时选中预览**、
  无修饰套索的「替换」分支不可达——这些是产品语义问题，建议单开一轮并先改合同。
- `docs/06` §5 写「Alt+拖拽 = 记录自动布局偏移」，与实现（Alt+拖拽 = 套索减选）相反；
  `docs/02` §2 没写 Alt/套索，且「框选以包围盒相交为准」不覆盖套索（中心在多边形内）。
- 套索松手即使圈空也会 `clear_foreign_selection`（清关系线/注解选中，不动节点选中）。
- M7 报告/门脚本注释里「`mm-marquee` 框住 4 颗、`mm-marquee-add` 加到 5 颗」是
  ppp 1.5 时代的数字；当前 ppp 1.0 下两者各只 `picked=1`（基线 0 漂移，说明数字写错但图没变）。
- 实机 Alt 键与 IME 抢占未核。
