# C3 · 关系端点拖拽（RelinkDrag）的落点余量（打磨轮 12）

> 依据：`docs/x100/polish-ledger.md` §1 C3 行「拖拽中强调抑制有图，手感（阈值/吸附）没量」；
> 设计合同：`docs/30` §12 第 1c 条（本轮新增）。
> 状态：`已实现`（**未记 `已验证`**：缺一次实机鼠标手感核对，见 §7）。

## 1. 这一轮解决什么

改一条关系线的终点（拖端点、松手落在另一颗概念上）走的是
`RelinkDrag { at, target }` → `commit_relink(rd)`。`target` 来自
`mw_ui::link::target_node(scene, from, at)`——**严格矩形包含**：

```text
压中某颗概念（且不是起点自己）→ 改指到它
没压中任何一颗            → 算「落空」
```

而落空在 CM-10 是**一条正经命令**：`RetargetLinkToNewConcept`（在落点新建一颗概念并把线接过去）。
两条行为各自都对，凑在一起就成了毛病：**指针离目标框差 1–10 个像素**（拖拽手抖、框是圆角、
指针压在描边上），用户明明是想「改指到这颗」，松手却凭空多出来一颗概念。

同一个函数也在思维导图的连线拖拽（`DragKind::Link`）里用，所以修在 `mw_ui` 这一层，两处一起好。

## 2. 非目标

- **不改**「落空 = 新建概念」这条语义（`docs/30` §12 第 1、1b 条是合同，不是 bug）。
- **不做**磁吸动画 / 磁性曲线 / 落点涟漪：这一轮只要「差几个像素也算压中」。
- 不改 `Scene`、不改 `.zmind`、不加依赖。
- 不动树的连线锚点命中（`anchor_at`），那是抓取端，不是落点。

## 3. 设计

### 3.1 判定

```rust
pub fn target_node(scene, from, at) -> Option<u32>          // 余量 0，原行为
pub fn target_node_snapped(scene, from, at, snap) -> Option<u32>
```

`target_node_snapped` 两轮扫：

1. **压中轮**：`rect.contains(at)`，取面积最小的（父盒套子盒时取子盒）——与旧判定逐位一致；
2. **近邻轮**（只有压中为空且 `snap > 0`）：到矩形的欧氏距离（`dx`/`dy` 各自夹到 0 再取模），
   `d <= snap` 里取最近；距离相同取面积小的，再相同取 `draw_order` 靠前的。

排序键写死成「压中 > 近邻」，所以**吸附永远不会抢走一次真正的压中**。
起点自己（`from`）两轮都跳过。

### 3.2 余量的单位是**屏幕像素**

```rust
pub(crate) const LINK_SNAP_CSS: f32 = 12.0;   // crates/mw-shell/src/canvas/consts.rs
// 调用点
target_node_snapped(&scene, from, at, self.camera.css_to_world(LINK_SNAP_CSS))
```

12 css px 是 UI 常用手感余量（Win11 的按钮外扩也是这个量级）；它必须吃掉镜头缩放——
否则缩小后 12 世界单位会变成 4 个屏幕像素，放大后又变成 30 个，手感随缩放漂。实测示例图的
镜头是 0.808×，所以世界余量是 9.7，屏幕上始终 12。

### 3.3 拖拽途中要看得见

`RelinkDrag.target` 本来就有、也本来就会画目标高亮框，只是**没人给它赋值过非空值**之外的正确判定；
这次把两条鼠标路径（`DragKind::Link` / `DragKind::Relink`）都换成带余量的版本，
松手前画出来的框就是「松手会连到谁」。**这条必须有图**，否则「吸没吸上」只能靠嘴说。

## 4. 量法（证据）

### 4.1 离屏夹具要真的走一遍判定

`begin_relink_drag_label()`（`MW_CONCEPT_RELINK_DRAG`）原来写死 `target: None`——
夹具拍出来的永远是「落空」预览，等于拍了个寂寞。这一轮改成和鼠标走同一个判定，
于是同一份场景能拍出「吸上了」的图。而且**现在打 `target=` 进轨迹**：

```text
event relink-preview drew from=(292.1125, 76.0) at=(-198.1125, 128.0) target=None
event relink-preview drew from=(292.1125, 76.0) at=(-198.1125, 128.0) target=Some("01a1861e-...")
                         ↑ 余量 0（旧行为）                    ↑ 余量 12 css px
```

### 4.2 新基线 `cm-relink-near`

指针停在一颗概念的框外 6 个世界单位（吸附半径 9.7 之内），落点高亮框必须画出来：

| 场次 | 环境 | 期望 |
| --- | --- | --- |
| `cm-relink-near` | `MW_CONCEPT_DEMO=1`、`MW_CONCEPT_RELINK_DRAG='带来|-243|-72'`、1280×800、ppp 1.0、warmup 10 | 「增长策略」框外 6px，**画出**目标框 |

A/B 图（同一份场景、同一帧，只差 `LINK_SNAP_CSS` 是 0 还是 12）：

| | 图 | 差异 |
| --- | --- | --- |
| 修复前（余量 0） | `shots/x100/C3/before-snap0.png` | 无目标框 |
| 修复后（余量 12） | `shots/x100/C3/after-snap12.png` | 「增长策略」出现高亮框 |
| 逐像素差 | `before-crop.png` / `after-crop.png` | **6991 px**，包围盒 (557,741)–(806,839)（209×98） |

## 5. 单测（新增 3 条，`mw-ui::link` 共 8 条）

1. `a_near_miss_snaps_inside_the_margin_only` — 框外 6 单位：余量 0 落空、余量 12 吸附、余量 5 不吸。
2. `an_inside_hit_beats_a_nearer_outside_one` — 压中的结论不许被吸附改掉（哪怕旁边有更近的框外节点）。
3. `the_nearest_outside_node_wins` — 没压中时取最近；起点自己永远不被吸成目标。

## 6. 门与测试

- `cargo fmt --all -- --check` 干净。
- clippy 指纹：**新增 0**（顺手把第 11 轮留下的 9 条行号漂移重生成进基线，
  基线仍是 37 条、种类 0 变化——只改行号）。
- `cargo test --workspace -- --test-threads=1`（TEMP 重定向）：**931 passed / 0 failed / 1 ignored**
  （上一轮 928 + 本轮 3）。
- stage-b 门、stage-cm 门见 §8 的表（本轮改了拖拽判定、又加了一张 cm 基线）。

## 7. 未做 / 风险

1. **实机鼠标手感没量**：12px 是 UI 惯例，不是我在这台机器上用真鼠标试出来的；
   离屏 harness 只有几何，没有指针节拍。要闭合得跑一次真实拖拽（`m5-perf.ps1` 那类脚本）。
   所以本包只记 `已实现`，**不记 `已验证`**。
2. **余量对嵌套盒没有特殊处理**：父子盒同在图里时，近邻轮取「距离最近的矩形」，
   距离相同才按面积。圆角盒的四个角是矩形外接，理论上角上会多出一点点余量（4 个角各 ~1px 量级）。
3. **吸附没有中间态**：进入余量就整块高亮，没有「越近越明显」的渐变。这轮刻意不做（动效克制），
   但如果实机觉得「框突然跳出来」刺眼，下一轮再谈。
4. 只覆盖概念图与思维导图**同一个** `target_node` 入口；卡片页/大纲没有拖连线，不涉及。
5. 轨迹里 `target` 打的是 id，不是文字——轨迹只用于对照「有没有吸上」，不用于读内容。
