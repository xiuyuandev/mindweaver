# M6 · 键盘移动（打磨轮 7）

> 依据：`docs/x100/polish-ledger.md` §1 M6 行（「连续按多次的节奏没人看过」）。
> 状态：**两条真实缺陷已改，三张基线与四条轨迹在**；实机键盘连按的节拍仍 `未验证`（离屏脚本按帧推进）。

## 1. 这一轮要解决什么

账本只写了一句「连续按多次的节奏没人看过」。按脚本连按 `Alt+↓` 三下再看每帧节点盒，毛病不是节奏，是两件事：

| 动作 | 改之前 | 应有的样子 | 轨迹证据 |
| --- | --- | --- | --- |
| 镜头被拖走以后按 `Alt+↓` | 节点在 `(-716,-428)` 屏幕外，按完还在原地 | 跟着主选走，回到 `(48,96)` 的 48px 安全边内 | `trace-reveal-before/after.txt` |
| 框选两颗按 `Alt+↓` | 只动**主选**那一颗（`Ctrl+Shift+←/→` 同样只动一颗） | 整组移动一格，组内顺序不翻，一次撤销 | `trace-move-multi.txt` |

第一条的不一致最关键：折叠、方向键遍历、`Tab` 新建、右键菜单都跟镜头，偏偏这三条层级命令
（`Alt+↑↓`、`Ctrl+Shift+←/→`）不跟——连按十几下，节点会一路走出屏幕，用户只能自己再平移回来。
第二条的不一致在**同一件事的两条路**上：大纲与右键菜单走 `BatchOp` 批量命令，画布快捷键走单条命令，
选了五颗按 `Alt+↓` 只动一颗，看着像「多选无效」。

## 2. 非目标

- 不动 `BatchOp` / `batch_structure_command` 的语义（组内顺序、边界不动），只换调用点。
- 不改撤销粒度：整组移动本来就由 `batch_structure_command` 合成**一条** `Batch`，本轮只补验证。
- 不碰拖拽重排（M5 已收）、不碰 `Ctrl` 拖拽复制、不碰大纲页的快捷键（它早就走批量 API）。
- 不量实机键盘连按的节拍（系统键重复率、抢焦点那一帧），与 M1/M2 一样留作共用缺口。

## 3. 改了什么（代码）

| 位置 | 改动 |
| --- | --- |
| `canvas_methods_1.rs::reveal_id` | 新私有入口：id → `scene.find` → `reveal_node`（`reveal_center` 的纯计算早就有了，本轮只是把这三条命令接上去） |
| `canvas_methods_1.rs::move_selected` | 先取 `selected_ids()`；`len > 1` 走 `move_step_many`（批量），否则保持老的单条路径 + `reveal_id` |
| `canvas_methods_1.rs::{promote_selected, demote_selected}` | 同上：多选走 `outdent_many` / `indent_many`，单选保持老路径 + `reveal_id` |
| `canvas_methods_1.rs::move_selected` | 新轨迹 `structure move=±1 selected=N path=single\|batch`（只在 `MW_TRACE` 下写），用来分辨走的是哪条路 |
| `canvas_methods_3.rs::apply_batch_structure` | 整组操作成功后同样 `reveal_id(主选)`（主选不在了就退第一个），失败时写 `structure_batch op=… n=… command=none` |
| `canvas_methods_2.rs` 框选提交 | 新轨迹 `marquee picked=N selected=M`——「框到几颗」和「选上几颗」不是一回事（被覆盖的后代会被丢掉） |
| `canvas/tests.rs` | 新单测 `single_selection_moves_the_same_way_through_both_paths`：`move_command`（单条）与 `batch_structure_command`（批量，只给一颗）必须挪出同一棵兄弟顺序，到边界两条都不动 |

**为什么单选不直接走批量**：批量命令会按 `selected_roots` 归一化并重建 `Batch`，单条路径是既有行为
（含 `PlanTree` 的补偿逻辑），本轮不换——但两条路的**结果**必须一致，所以补了那条等价单测钉住它。

## 4. 证据

### 4.1 单次移动（`mm-move-keys`）

输入：点中「纯 Rust 原生」→ `Alt+↓` ×3 → `Alt+↑` ×1。轨迹 `trace-move-keys.txt` 四次移动各一行：

```
event structure move=1 selected=1 path=single
event scene_build nodes=13 edges=12 ms=0.08
event reveal idx=5 cam=(-78,0) z=0.85 node=(134,-118,150,49) vp=(1620,1008)
```

四次「结构 → 一次重建 → 跟镜头」，节点在左右两侧之间来回：`(634,452) → (990,452) → (634,610) → (990,610)`，
最后一次 `Alt+↑` 回到 `(634,610)`；`scene_build` 一共 4 次（没有多余重建）。
这四步里 `reveal` 的 `cam` 一直停在 `(-78,0)`——这个小文档在 `z=0.85`、视口 `1620×1008` 下整棵树都装得下，
`reveal_center` 判定「已经在视口里」就不动相机。真正的跟镜头要看 §4.3。

### 4.2 整组移动（`mm-move-multi`）

框选左侧两颗 → `Alt+↓`：

```
event marquee picked=2 selected=2
event structure move=1 selected=2 path=batch
event scene_build nodes=13 edges=12 ms=0.08
```

「纯 Rust 原生」`(634,452) → (990,500)`、「交互与性能」`(634,584) → (646,610)`——两颗一起挪，
只重建一次。同一段脚本撤一步即回到框选前（`trace-move-multi-undo.txt`）：
`undo label=移动节点 left=17` + 一次重建，**整组移动是一条撤销**。

### 4.3 镜头跟随（`mm-move-keys-reveal`）

先把镜头拖走，再 `Alt+↓`。两张轨迹是同一段脚本的关键一步之差：

| | 「纯 Rust 原生」的屏幕盒 | 说明 |
| --- | --- | --- |
| `trace-reveal-before.txt`（只拖不按键） | `(-716,-428,128,42)` | 整个盒子在视口外 |
| `trace-reveal-after.txt`（拖完按 `Alt+↓`） | `(48,96,128,42)` | 回到左上 48px 安全边内 |

`reveal` 轨迹：`reveal idx=5 cam=(1028,418) z=0.85`——只平移，`z` 没变（跟镜头不偷偷改缩放）。

### 4.4 基线

| 场景 | 拍的是什么 |
| --- | --- |
| `mm-move-keys` | 四次单条移动后的终态 |
| `mm-move-multi` | 整组下移一格后的终态（两颗同时换位） |
| `mm-move-keys-reveal` | 镜头被拖到文档外之后，`Alt+↓` 把主选拉回视口 |

三张都是本轮新增，`--update` 生成后 stage-b 门 `--tol 0` 双跑零差异。
**`mm-move-keys-reveal` 第一版不成立**：拖的距离不够，节点停在视口顶端 `(134,-118)` 仍然可见，
`reveal_center` 算出的相机与原相机相差不到 1 单位——那张基线改与不改都是一样的图。
改成从 `(1500,1000)` 拖到 `(150,120)`（节点整颗推出屏幕）后，改前的 `(-716,-428)` 与改后的 `(48,96)`
差 700 多像素，基线才真的锁住了这条行为。

## 5. 门槛

| 项 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --workspace --all-targets` | 新增 0（`non_shorthand_field_patterns` 一处当场改掉） |
| `cargo test --workspace`（TEMP 已重定向） | 见 `docs/STATUS.md` §1 的测试数 |
| stage-b 门 `--tol 0` 双跑 | 94 场 / 188 次比较，`failures=0`，既有 91 张零漂移 |
| stage-cm 门 `--tol 0` 双跑 | 46 场 / 92 次比较，`failures=0` |
| 两道门顺序跑 | 本轮再确认：并行跑会互相踩 `shots/golden/<scene>/actual`（见 M12 报告 §6） |

## 6. 未做 / 风险

- **实机键盘连按**没量：脚本是逐帧推进的，系统键重复率、按住 `Alt+↓` 不放的节拍都没测。
- **多选跨父级**：整组移动要求同一父节点（`batch_structure_command` 的既有语义，已由 `selected_roots` 归一化）；
  跨父级多选按 `Alt+↓` 现在**什么都不动**（`structure_batch … command=none`）。这是既有行为，本轮没改，也没在 UI 上给反馈。
- **跟镜头是「最小平移」不是「居中」**：只在节点要出界时挪，已经看得见就不动——与折叠/方向键同一套。
- **`Ctrl` 拖拽复制**、150%/200% 缩放下这几条命令的落点，都没拍。

## 7. 下一轮

账本 §3 第 7 条「20k 增量布局」仍是最硬的未做项（20k 打字 47ms）；往下还有 M4（实机 IME，阻塞）、
M7（框选阈值手感）、M9（折叠动画与点击热区）。
