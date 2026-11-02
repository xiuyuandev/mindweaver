# M7 · 框选 / Ctrl+点击 / 批量操作（打磨轮 9）

> 依据：`docs/x100/polish-ledger.md` §1 M7 行（「框选阈值 4px 的手感未量」）。
> 状态：**阈值量到了、橡皮筋第一次有图、框选过程第一次看得见**；实机手感仍 `未验证`。
> **本轮改了一处产品行为**：框选拉动过程中，命中节点现在实时挂上选中环（以前只有松手才结算）。

## 1. 这一轮要解决什么

账本 M7 行的证据列写的是「`mm-boundary-select` 等」——那是**边界**的图，橡皮筋本身一张图都没有；
「框选阈值 4px 的手感未量」。照着把交互跑了一遍，发现比阈值更要紧的一件事：

**橡皮筋拉了半个屏幕，用户不知道框住了什么**。`marquee_pick` 只在松手那一刻调用
（`canvas_methods_2.rs` 的 release 分支），拉动过程中命中的节点没有任何视觉反馈，
只能松手看结果再拉一次。XMind 的同类操作在拖动中就实时高亮命中节点。

另外两件事属于「本来就对、缺证据」：

- 阈值 `DRAG_THRESHOLD = 4.0` 是在 `egui` 的 points（CSS px）里比较的（`length() > DRAG_THRESHOLD`），
  与缩放无关——「画的就是屏幕常量」这条以前没有证据；
- `marquee_pick` 的屏幕→世界换算（`camera.screen_to_world`）没有任何单测，
  倒过来说，把换算写坏一半、只在某一档缩放下出错，现有测试全绿。

## 2. 非目标

- 不改阈值（4px 是 docs/06 §4 的规格，本轮只把它钉住）。
- 不改 `apply_marquee` 的合并语义（替换 / Ctrl 加选 / Alt 减选），只验证。
- 不做套索以外的选择形状（`DragKind::Lasso` 已存在，另有轨迹）。
- 不碰拖拽重排（M5 已收）与批量操作命令（`BatchOp` 语义在 M6 收过一轮）。

## 3. 改了什么（代码）

| 位置 | 改动 |
| --- | --- |
| `canvas_methods_2.rs`（渲染入口） | 框选拖动中每帧算一次 `marquee_pick`，把「松手会选中谁」临时并入 `Frame.selected`——命中节点实时挂选中环。预览集合只在**变化**时写一行 `marquee preview picked=N selected=M`，并随松手/取消清空 |
| `canvas_methods_2.rs`（`select_target`） | 选中环透明度的目标值把预览也算进去。第一版漏了这里：预览算出来了、透明度停在 0，**A/B 对比逐字节零差异**——预览画了个寂寞。修掉后 A/B 才出现 3975 px 的差 |
| `golden_input.rs` | 新动词 `mods ctrl\|shift\|alt\|command\|none`：拖拽的 `additive` / `subtract` 在**按下那一刻**就读走，`sclick` 只够管单击，Ctrl+框选的脚本必须先把修饰键摆好。拼错报错不静默 |
| `canvas/tests.rs` | 新单测 `marquee_pick_maps_the_screen_box_through_the_camera`：0.5× / 1× / 2× 三档缩放，用 `world_to_screen` 把一个世界盒的两个角换到屏幕、再框回去，必须选中同一个节点 |
| `canvas.rs::Canvas` | 新字段 `marquee_preview`（预览集合，给「只在变化时写轨迹」与 `select_target` 用） |

## 4. 证据

### 4.1 阈值：2px 不算框选，5px 算

同一段脚本只差拖动的距离（按下 `(700,700)` 后松手）：

| 拖动距离 | 轨迹里的 marquee 事件 | 读法 |
| --- | --- | --- |
| 2px（`trace-2px.txt`） | **0 条** | 还在「点击」的档位，按单击空白处理 |
| 5px（`trace-5px.txt`） | `marquee picked=0 selected=0` | 越过阈值、真的在框选；框在空白上，把选择清掉 |

阈值是**屏幕常量**：比较发生在 egui 的 points 空间，缩放 50% / 200% 时 4px 还是那 4px（`DRAG_THRESHOLD` 的注释写明了 docs/06 §4 的出处）。

### 4.2 橡皮筋与「松手会选中谁」第一次有图

`mm-marquee`：从空白 `(480,330)` 拖到 `(720,560)`，**不松手**定格——虚线框 + 框内四个节点的选中环。
轨迹里预览随拖动变化：

```
event marquee preview picked=0 selected=0
event marquee preview picked=2 selected=2
event marquee preview picked=4 selected=4
```

**A/B 对比**（同一脚本，把预览代码临时关掉再拍）：

| | 与关掉预览的基线比 | 包围盒 |
| --- | --- | --- |
| 预览开 | **3975 px（0.227%）**，最大差 205 | `x[422..765] y[426..571]` |

包围盒正是框内节点的位置（含环的外扩），而不是橡皮筋自身的矩形——差的那 0.2% 就是选中环。
（第一版 A/B 是**零差异**，由此才揪出 `select_alpha` 停在 0 的漏改。）

### 4.3 Ctrl+框选 = 加选

先点「导出」（在框外），再按住 Ctrl 框左侧四个节点：

```
event marquee preview picked=0 selected=1
event marquee preview picked=2 selected=3
event marquee preview picked=4 selected=5
event marquee picked=4 selected=5
```

框外那颗全程都在组里：`1 → 3 → 5`，松手时预览与最终结算一致（`selected=5`）。
`mods` 动词保证了修饰键在**按下那一刻**就位——这正是拖拽语义读它的时机。

### 4.4 基线

| 场景 | 拍的是什么 |
| --- | --- |
| `mm-marquee` | 拉到 `(720,560)` 未松手：虚线框 + 框内四颗的选中环（§4.2 的终帧） |
| `mm-marquee-add` | 框外先选一颗 + Ctrl+框选：松手后五颗全有环（§4.3） |

## 5. 门槛

| 项 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --workspace --all-targets` | 37 条 / 新增 0 |
| `cargo test --workspace`（TEMP 已重定向） | 见 `docs/STATUS.md` §1 的测试数（+2：`mods` 解析、`marquee_pick` 换算） |
| stage-b 门 `--tol 0` 双跑 | **100 场 / 200 次比较**，`failures=0`；日志 `target/x100/gate-b-m7-solo.log` |
| stage-cm 门 `--tol 0` 双跑 | 46 场 / 92 次比较，`failures=0`；日志 `target/x100/gate-cm-m7-solo.log` |

## 6. 未做 / 风险

- **实机手感没量**：阈值 4px 是规格值，但「5px 的抖动就清掉整个多选」好不好是手感问题，没有实机数据之前不动。
- **框选拖动中每帧一次 `intersecting`**：13 节点的示例是 O(13)；20k 节点下是 O(20k) 的矩形相交测试
  （纳秒级每项），但**没有**在 20k 文档里实测过拖动中的帧率，见 M12 的账。
- Alt 减选（`mods alt`）有解析与 `apply_marquee` 单测，没有专门的金样场景。
- 套索（`DragKind::Lasso`）的视觉/手感仍在 M7 行之外的盲区，本轮没动。

## 7. 下一轮

账本 §1 剩下的未验证行：M4（实机微软拼音，阻塞）、C3（relink 拖拽手感）、C7（整理布局的提示与撤销动线）；
最硬的未做项还是 §3 第 7 条「20k 增量布局」。
