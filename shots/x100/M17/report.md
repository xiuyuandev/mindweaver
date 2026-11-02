# M17 · 折叠圆点 / 概要 / 边界的悬停反馈

- 日期：2026-10-31（打磨轮 37）
- 合同：[`HANDOFF.md`](../../../HANDOFF.md) §7.5②；[`docs/06`](../../../docs/06-界面布局与交互设计.md) §5「折叠圆点：悬停放大 1.3 倍」；[`docs/30`](../../../docs/30-外壳体验与概念图设计规范.md) §7 第 6 条「看得见才存在」
- 范围：**壳层悬停反馈**，不改 `mw_render::Frame`、不改场景/文档模型、不动 `.zmind`
- 状态：进行中（代码与证据在本文 §3–§5 回填）

---

## 1. 问题

`HANDOFF.md` §7.5② 记着：「悬停反馈只做了一半：折叠圆点 / 边界 / 概要的 hover 仍是『没有反馈』」。
实测确认：

- **折叠圆点**：`docs/06` §5 明写「悬停放大 1.3 倍」，但 `instances::push_toggle` 只按固定
  `TOGGLE_R=8` 绘制，`Frame` 里没有「哪个圆点被指着」的信息；
- **概要 / 边界**：`hit.rs` 能把指针解析成 `SummaryTopic/SummaryBrace/Boundary`（优先级分别 4/1/0），
  但 `Canvas::show` 只把 `Toggle/Node/Label` 映射进 `hovered`，这两种目标拿到命中后既不亮、也不给光标之外的反馈；
- 用户能点中一个花括号或一条边界，但点之前屏幕上没有任何东西说明「这儿是一个可点的整体」。

`Frame.hovered` 只带一个 `Option<u32>` 节点下标，这是根因。HANDOFF 原判断是「要做得改 `Frame`（渲染层 ABI）」，
本轮重新核对后**不需要**：画布已有成熟的「合成纹理之上再画 egui 覆盖层」通道（跨度手柄、拖连线预览、
橡皮圈、落点指示都走这条），悬停反馈是纯交互态、本来就不该进 `Frame`（否则离屏导出会带上悬停）。

## 2. 非目标

- 不改 `Frame` 字段、不改 `instances.rs`、不改导出（PNG/SVG/PDF）；
- 不改命中优先级/容差；悬停**不等于**选中，不改变选择、不发命令；
- 不做注解 tooltip、不做悬停淡出后的残影；概念图节点 hover（`docs/30` §5.3）已由 `Frame` 覆盖，本轮不动；
- 不新增依赖、不扩数据结构。

## 3. 设计

### 3.1 壳层状态

```text
HoverDetail = Toggle(node) | Summary(idx) | Boundary(idx)   // 下标都是本帧 Scene 的密集索引
hover_detail: Option<HoverDetail>
hover_detail_alpha: f32                                     // 120ms（design::motion::FAST）
```

`hover_detail_of(Target) -> Option<HoverDetail>` 是纯函数：`Toggle→Toggle`、
`SummaryTopic|SummaryBrace→Summary`、`Boundary→Boundary`，其余（节点/标签/关系线）返回 `None`
（节点与关系线已有自己的反馈，不叠第二层）。取目标与节点悬停共用同一组门：
`hover_enabled && editor.is_none() && !input_blocked && drag.is_none() && !response.dragged()
&& (response.hovered() || pointer_over_bar)`，另加 `summary_edit/rel_edit` 不亮。

`hover_detail` 存下标，所以**重建场景时必须清空**（与 `hovered` 同批），否则会指到另一条注解；
`advance_alpha` 驱动 120ms 淡入淡出，harness 的 `reduce_motion` 下直接落位。

### 3.2 画法（全部在 egui 覆盖层，合成纹理之上）

| 目标 | 画法 |
| --- | --- |
| 折叠圆点 | **按 1.3 倍重绘整颗圆点**：先以画布色填一颗半径 `(TOGGLE_R+TOGGLE_HALO)×s` 的圆（盖住原圆点与穿过来的连线），再画半径 `TOGGLE_R×s`、线宽 `TOGGLE_W×s` 的分支色环与 `TOGGLE_BAR×s` 的 +/− 横竖线；`s = 1 + 0.3×alpha`，alpha=0 时与渲染层原样重合，所以淡入就是从原尺寸「长」到 1.3× |
| 概要 | 主题盒按 h/2 圆角、外扩 2px 描 1.5px 强调色（55%）；花括号与连接线用 `flatten` 折线后描 2px 强调色（55%） |
| 边界 | 世界矩形换到屏幕，圆角 `radius::PANEL × zoom`、外扩 2px 描 1.5px 强调色（55%） |

不画底色填充：边界覆盖面积可以很大，铺 4% 强调底会把整块区域染色，反而看不出边界线在哪。
正在选中的注解不重复画（选中环已有 2px 实色），正在编辑的概要/关系短语不亮。

### 3.3 与 `docs/06` 的 1.3 倍对齐

渲染层画的圆点几何是固定的；真放大必须由上面那层「重绘」完成。选择重绘而不是加 `Frame` 字段，
是因为：① 缩放是交互态，导出不需要；② `Frame` 是渲染层 ABI，每加一个交互字段都要在导出路径考虑默认值；
③ 壳层已经拥有相机、主题与场景，重绘所需信息全在手上。代价是圆点悬停时圆点的 ± 线由 egui 画，
与合成器的抗锯齿实现不同——两者都是 1.5 CSS px 实色线，肉眼与基线像素差都能量。

## 4. 验收

1. **纯函数**：`hover_detail_of` 七个 `Target` 变体全覆盖；`Toggle` 取节点、`SummaryTopic/Brace` 取同一条概要。
2. **几何**：1.3 倍缩放公式在 alpha=0/1 时分别等于原尺寸/1.3 倍；屏幕半径按 `camera.chrome` 的
   4.5 世界单位下限（缩到很小时圆点不会退化成 0）。
3. **金样**（新增三张，`--tol 0` 双跑）：
   - `mm-toggle-hover`：默认夹具，指针停在折叠圆点上（落定相机后 **(522,429)**）；
   - `mm-summary-hover`：`tiny-cn.zmind` + 指针停在概要主题盒中心 **(176,481)**；
   - `mm-boundary-hover`：`tiny-cn.zmind` + 指针停在边界左边线 **(1123,482)**。
   三张都必须与对照图有**有限、只落在目标周围**的差异（实测 377 / 1 484 / 2 748 px，
   bbox 分别只覆盖圆点、概要、边界本身），见 `diff.txt`；其余既有基线零漂移。
   **例外并已修正**：`mm-collapse` / `mm-collapse-mid` / `mm-toggle-edge` / `mm-toggle-miss`
   四张的点击坐标自第 19 轮起就失效（0 帧落空），旧「折叠」基线锁的是没折的画面。
   本轮按落定相机重标为 (522,429) / +9px / +12px 并重拍，旧图登记见 `diff.txt` §2。
4. **导出无污染**：本轮所有绘制都在 `Canvas::show` 的覆盖层，`Frame` 未变，离屏导出代码路径不经过该函数。
5. **trace**：`hover_detail toggle <id>` / `summary <id>` / `boundary <id>` / `none`，
   截图只能证明「亮了」，trace 证明「亮的是这条」。

## 5. 证据

- 单测（3 条，`canvas/hover.rs`）：`every_hit_target_maps_to_its_own_hover_feedback`、
  `toggle_grows_from_exact_size_to_one_point_three`、`toggle_radius_keeps_the_world_floor`
  （zoom 1 → 8px、0.1 → 8px、2 → 9px）。全量数见 `docs/STATUS.md`。
- 轨迹：`shots/x100/M17/trace.txt` —— 三张悬停图的 `scene_*` 中心与 `hover` 落点同值，
  `hover_detail` 的 id 与场景 id 相同；折叠三点击的 `canvas_click hit` / `toggle idx` 原样摘录。
- 像素差：`shots/x100/M17/diff.txt` —— 悬停对照 377 / 1 484 / 2 748 px；
  旧折叠基线错误 108 025 px；中点帧与终态 62 619 px；边缘点击与圆心 0 px。
- 基线：`shots/golden/inputs/mm-toggle-hover.txt`、`mm-summary-hover.txt`、`mm-boundary-hover.txt`；
  场次登记：`scripts/x100-stage-b-gate.ps1`（toggle）与 `scripts/x100-stage-cm-gate.ps1`
  （summary / boundary，`tiny-cn.zmind`）。
- 坐标标定：`MW_TRACE` 的 `scene` 导出新增 `scene_summary` / `scene_summary_brace` /
  `scene_boundary` / `scene_toggle` 四类行，用真实布局算坐标，不靠点击网格反推。
  这三张基线里所有落点都是照这些行量的（旧坐标 (623,473)/(125,428)/(1073,675) 全部作废）。
- 机器门（2026-10-31 实测）：`stage-b` **119 场 / 238 次比较**（`01:11:26–01:17:44`，pass 1 / pass 2
  全过，`MACHINE GATE PASS`）；`stage-cm` **51 场 / 102 次比较**（`01:18:22–01:20:55`，两遍全过，
  `MACHINE GATE PASS`）。`cargo fmt --all -- --check` 干净；`cargo clippy --workspace --all-targets`
  exit 0，指纹 **36/36、新增 0**（当前集合与 `scripts/clippy-baseline.txt` 逐条相同）。

## 6. 未做 / 风险

- 圆点重绘与合成器在两套 AA 实现之间，像素差会在基线里体现；若 1.5 CSS px 线在 150% DPI 下肉眼可见，
  下一轮考虑改由 `Frame` 承担放大。
- 悬停概要不区分「指着花括号」还是「指着主题盒」——两者都亮整条概要，这是刻意的整体语义；
  如果后续要分别做 tooltip，需要拆开。
- 实机鼠标（60Hz/高刷）的淡入手感未量，只做了 harness 定格；与节点悬停同一过渡时长。
