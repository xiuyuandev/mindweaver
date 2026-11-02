# C7 · 整理布局的提示与撤销动线（打磨轮 13）

> 依据：`docs/x100/polish-ledger.md` §1 C7 行「提示与撤销动线只有单测」；
> 设计合同：`docs/30` §5.5「整理布局 …… **一次命令，可撤销**」。
> 状态：`已实现`（提示补上了、撤销动线有了端到端图与单测；**不记 `已验证`**：实机鼠标/键盘手感未核，见 §7）。

## 1. 这一轮解决什么

「整理布局」会**丢掉所有手工拖出来的位置**（分层重排 + 松弛），是这张图上破坏性最大的一个动作，
而它现在：

| 应有的东西 | 现状 |
| --- | --- |
| 撤销动线：一次 `Ctrl+Z` 完整复原 | 代码上是 `Batch[MoveConcept…]`，但**一条测试都没有**（账本 C7 行写的「有单测」指的是
 `concept_layout` 的纯函数算法，不是这条命令） |
| 提示：告诉用户「刚动了 N 颗、可以撤销」 | **没有**。整理后相机直接 `fit` 到新布局，用户只看到画面一跳 |
| 端到端图 | 只有 `cm-arrange`（整理后），**没有「整理 → 撤销」的图** |

所以这一轮做三件事：把「整理」发出的命令抽成可测的纯函数、给它一条真单测、
在右键菜单落地点补一条 toast，并用「整理 → `Ctrl+Z`」的图把撤销动线钉住。

## 2. 非目标

- 不改 `concept_layout::arrange` 的算法与参数（`LAYER_GAP` / `NODE_GAP` / 24 轮松弛都不动）。
- 不加撤销确认弹窗：整理是可撤销的，弹窗反而多一步。
- 不改「整理」的入口（仍是空白处右键）。
- 不做「整理前预览」，也不做重叠红边提示的改动。

## 3. 设计

### 3.1 命令组装抽成纯函数

```rust
pub(crate) fn arrange_batch(
    ids: &[String],
    current: &[(f32, f32)],
    placed: &[(f32, f32)],
) -> Option<Command>
```

- 位置没变（`< 0.01`）的概念不出命令；
- 一条 = 直接那条 `MoveConcept`；多条 = `Command::Batch`（撤销栈里仍是**一条**）；
- 一条都没有 = `None`（调用方据此决定要不要提示）。

`arrange_concepts` 改成 `-> Option<usize>`（返回动过几颗），落地点在右键菜单那里补 toast。

### 3.2 提示

落地点（`app_methods_2.rs` 的 `A::ArrangeConceptMap`）在请求焦点之后：

```text
已整理 6 颗概念 · Ctrl+Z 撤销
```

只挑「真的动了」的时候弹（`None` 不弹）。toast 是既有的 2.4s 提示条，
同时给 `set_toast` 加一行 `toast text=…` 轨迹——提示条本身是**时间敏感**的（2.4 秒后自己消失），
不拍进 golden（拍了会在慢机器上时有时无），证据用轨迹 + 下面那张撤销图。

### 3.3 撤销动线怎么证明

新场景 `cm-arrange-undo`：`MW_CONCEPT_ARRANGE=1` 启动即整理，输入脚本按一次 `Ctrl+Z`。
图上应该回到**整理前的手工散布**。与 `cm-arrange`（整理后）对照就是这条路的两端。
另外用一行 `undo label=` 轨迹（M8 已有）证明这次撤销是**一条**，不是 N 条。

## 4. 量法

- 单测：`arranging_three_concepts_is_one_undo`——三颗概念各挪一截，`apply` 之后 `undo_depth` 只 +1，
  一次 `undo()` 三颗全部回到原位；`arrange_batch` 对「没动」返回 `None`。
- 轨迹：`concept-arrange n= moved=`（已有）、`undo label=`（M8 已有）、`toast text=`（本轮新增）。
- 图：`cm-arrange-undo`（整理后撤销）、与既有 `cm-default` / `cm-arrange` 对照。

## 5. 实测

### 5.1 提示（真菜单路径，轨迹证据）

脚本走的是**真的右键菜单**：`MW_CONTEXT_DEMO=blank` 打开概念图空白菜单，键盘两次 `↓` 到「整理布局」，`Enter` 激活。
轨迹（`shots/x100/C7/toast-trace.txt`，脚本 `shots/x100/C7/toast-trace-script.txt`）：

```text
event menu_activate key=enter label=整理布局
event concept-arrange n=6 moved=6
event toast bad=false text=已整理 6 颗概念 · Ctrl+Z 撤销
```

`set_toast` 本轮新增了 `toast bad= text=` 一行轨迹——提示条 **2.4 秒后自己消失**，拍进 golden 会在慢机器上时有时无，
所以所有 toast 的证据从此都走轨迹（这是本轮顺带补的公共设施）。

### 5.2 撤销动线（新基线 `cm-arrange-undo`）

`MW_CONCEPT_ARRANGE=1` 启动即整理 → 脚本按**一次** `Ctrl+Z`：

| 项 | 值 |
| --- | --- |
| 轨迹 | `concept-arrange n=6 moved=6` → `undo label=摆放概念 left=12`（撤掉一条，栈里还剩 12 条） |
| 基线 | `shots/golden/cm-arrange-undo/shell-ppp1.00.png` |
| 与 `cm-arrange`（整理后）的差 | **40022 px**，包围盒 (395,21)–(1718,730) |
| 与 `cm-default`（整理前）的差 | **40161 px**，包围盒 (13,19)–(1805,1195) |

读法：撤回来的**几何**是整理前的散布（图上五颗概念回到手工位置、连线跟着回去），
与 `cm-default` 的差主要来自**镜头**——整理会 `request_fit` 到新布局，撤销不动相机，
所以同一批位置现在按整理后的取景画。这条差异是当时的行为，**打磨轮 18 已改**（见 §9）。

## 6. 单测与门

新增 `canvas::tests::arranging_three_concepts_is_one_undo`：

- 三颗概念各挪一截 → `arrange_batch` 返回 `Command::Batch`、颗数 3（提示里的 N 就是它）；
- `apply` 之后 `undo_depth` 只 **+1**（整理在撤销栈里是一条），位置确实到了新坐标；
- 一次 `undo()`：栈深回到底，**三颗全部回到原位**；
- 位置没变时返回 `None`（不发命令、落地点也不弹提示）。

门与全量测试见 §8 的表。

## 7. 未做 / 风险

1. ~~**提示条不进 golden**~~（打磨轮 18 已改：harness 里提示条不按墙钟过期，见 §9）。
2. ~~**撤销不回镜头**~~（打磨轮 18 收口：撤销踩到整理那一条时重新 `fit`，重做再 `fit` 一次，见 §9）。
3. **toast 文案写死中文**：和其他 toast 一致，暂无本地化层。
4. **没有量「整理后用户多久按撤销」**：真机行为数据要埋点，本次不做。
5. 实机键位手感（右键菜单里键盘选「整理布局」两步 `↓`）没在真机上试过。

## 8. 门

| 门 | 结果 |
| --- | --- |
| stage-b（`scripts/x100-stage-b-gate.ps1`） | **绿**：100 场 / 200 次比较，`--tol 0` 双跑全过（`target/x100/gate-b-c7.log`） |
| stage-cm（`scripts/x100-stage-cm-gate.ps1`） | **绿**：47 → **48 场 / 96 次比较**（新增 `cm-arrange-undo`）；既有 151 张零漂移（`target/x100/gate-cm-c7-retry.log`） |
| 全量测试 | **932 passed / 0 failed / 1 ignored**（上一轮 931 + 本轮 1） |
| fmt / clippy | 干净；clippy 指纹新增 **0**（第一版 `let...else` 写法被 clippy 抓成 `?` 可简化，已改） |

**一次假红（如实记）**：第一遍 stage-cm 门的第 2 轮在 `cm-arrow-menu` 上有 **2 个像素**超容差（最大差 7，
包围盒 x[592..592] y[522..523]），第 1 轮零差异；整门重跑一次 48/48 双跑全过。这是门槛脚本注释里
记过的「紧跟重负载之后第一遍会多抓半帧」的同类现象（2026-10-28 先是 0.496%、后是 2.751% 两次假红），
与本轮改动无关（这张图不碰整理布局、也不弹 toast）。

## 9. 打磨轮 18 · 撤销把取景一起还回来（2026-10-29）

§7 第 2 条的原话是「撤销只还原位置、不还原相机，用户看到『原来的散布 + 整理后的取景』」。
本轮把它收口。

**做法**：`Canvas` 记两枚字段——`arrange_undo_depth: Option<usize>`（整理落地时的栈深）与
`arrange_redo_fit: bool`。撤销前先取一次 `undo_depth()`，踩到的正好是整理那一条，就在 `rebuild()`
之后 `request_fit()`（下一帧按当时的视口重新取景）；重做时同样 `request_fit()`，并把栈深重新挂上，
来回按都对。

**为什么不是「存一份相机、撤销时塞回去」**：整理可能在**首帧 `fit` 之前**就跑（演示/启动即整理），
那一刻的相机还不是用户看到的取景；存下来等于把错的那份还回去。位置回到哪、取景就该按哪份位置算，
`fit` 是唯一说得清的定义。

**证据**（都在 `shots/x100/C7/`）：

| 文件 | 说的是什么 |
| --- | --- |
| `undo-fit-trace.txt` | `concept-arrange n=6 moved=6` → `undo label=摆放概念 left=12` → `undo fit=1` → `fit zoom=1.5000 center=(23.00,3.00) bounds=(-346.0,-246.0,738.0,498.0)` |
| `fit-default-trace.txt` | 同一份文档的默认首帧 `fit`：`zoom=1.2369`、同一个 `bounds`（见下面的「发现」） |
| `undo-refit.png` / `fit-ref.png` | 整理 + 一次 `Ctrl+Z` ／ 同一文档手动 `Ctrl+1` |
| `pixel-diff.txt` | **画布行 100–1099：最大差 0**；全图 1828 px（>8）只在顶栏（缩放读数 124% vs 150%、撤销可用态）与左下提示条 |

新基线 `cm-arrange-undo` 已重拍，双跑 `--tol 0` 零差异。

**顺带修掉一处确定性问题**：提示条原来按 2.4 秒墙钟过期——离屏跑得快它就进基线、跑得慢就不在，
`cm-arrange-undo` 第一版基线就是这么带着提示条拍下来的。现在 harness 里提示条不按墙钟过期
（`app_methods_3::toast_bar`），脚本触发过就一定拍得到；这张图也第一次给了提示条像素覆盖。

**这一轮没有新单测**（如实记）：`arrange_concepts` 与 `undo` 都要真实 `Canvas`（要 wgpu 渲染态），
单元测试里没有无头构造路径；证据靠轨迹 + 像素 + 新基线。

**发现但本轮没做**：`cm-default` 的首帧 `fit` 用的是**还没落定的视口**——同一份包围盒
`(-346,-246,738,498)`，首帧算出 `zoom=1.2369`，稳定视口上是 `1.5`（`FIT_MAX_ZOOM` 钳位）。
也就是说「打开即适应」一直比它该有的小一档。改它会连带动所有取景基线，已经记进账本 §3 候选。
