# M32 · 双击折叠圆点「展开到下一级」

> 打磨轮 66 · 2026-11-01（开工在 19:13 前后，第 65 轮 TYPO-01 尚未收口时；**本轮收口 = 66**）
> 账本：`docs/x100/polish-ledger.md` §3 候选第 13 条（`docs/02` §3 的 P2「展开层级」）
> 合同：`docs/02` §3「双击折叠圆点：展开到下一级（逐级深入），区别于全展开」；
> `docs/06` §5「折叠圆点」行（本轮同时把该行里与实现不符的「双击展开下一级」标清）

## 1. 为什么是这一轮

`docs/02` §3 从 10-18 就写着「双击折叠圆点：展开到下一级（逐级深入），区别于全展开」，
但代码里**没有这条路径**：`canvas_methods_2.rs` 的折叠圆点命中只做单击切换，
双击与单击同义；`docs/06` §5 把这条未实现的行为写成了既有能力。本轮把它补成真行为。

## 2. 语义（本轮定死的口径）

对**折叠圆点**（分支末端的小圆环）：

| 当前状态 | 双击后 | 与单击的差别 |
| --- | --- | --- |
| 节点折叠 | 展开该节点，并把它**直接子级全部显式折叠**——恰好露出下一层 | 子级此前若展开过，单击会把它们连同子级一起露出来；双击只露一层 |
| 节点展开 | 把它的**直接子级**展开（再深一层）；节点本身保持展开 | 单击会折叠节点整棵；双击是「往下走」而不是「收回」 |

## 3. 范围

- 纯逻辑抽成 `canvas::drill_toggle_commands(doc, id) -> Vec<Command>`：
  按上面的口径返回一组 `SetCollapsed`；壳层把它们包成**一条 `Command::Batch`** 下发（一次撤销）。
- 折叠圆点的双击识别：记录上一次圆点按下的 `(id, egui 时间)`，同一点 500ms 内第二次按下且位移 ≤ 4 css px 判双击；
  单击路径保留（双击时**不**先执行单击的切换，否则会「先折后展」闪一下）。
  **时间源必须是 egui 时间**（`ui.input(|i| i.time)`），不能用 `Instant`：金样是帧计数虚拟时钟
  （第 35 轮 M16），读墙钟会让双击判定随机器负载漂。
- 轨迹：`toggle_drill id=… depth=N`（N = 本次展开后被展开的子级数）。

## 4. 非目标

- 不改「全展开 / 全折叠」（`Ctrl+Shift+/` / `Ctrl+/`）与 `set_collapsed_all` 的口径。
- 不做动画（折叠状态的过渡沿用既有 motion）。
- 不改变折叠圆点的命中半径（仍是 10 css px / 视口缩放无关）。
- 概念图没有折叠语义，不涉及。

## 5. 验收场景

1. 折叠的 A（子级 B、C 曾经展开过）双击圆点 → 只有 B、C 作为一层出现，B/C 各自的子级不可见。
2. 展开的 A 双击圆点 → A 的直接子级展开、A 仍展开；再双击继续深入。
3. 任意一次双击 = 一条撤销记录；`Ctrl+Z` 回到双击前的折叠状态。
4. 单击行为零变化（既有 `mm-collapse` / `mm-toggle-*` 基线零漂移）。
5. 双击时应用一条 `toggle_drill` 轨迹。

## 6. 预计证据

- 单测：`drill_toggle_commands_expand_exactly_one_level`（折叠态与展开态两种）、
  `drill_toggle_is_a_single_batch`（撤销一次复原）、双击识别纯函数 `is_double_click`（时间/位移阈值）。
- 新基线 `mm-toggle-drill`（折叠子树 → 双击圆点 → 只露一层）与 `mm-toggle-drill-deep`（再双击深入一层）。
- `stage-b` 151 → **153** 场（TYPO-01 报告里的 151 场 + 本轮两张；实际登记数见门脚本）。

## 7. 实测结果

### 7.1 核心纯逻辑（2026-11-01 18:40 · 已完成）

落点从报告原方案的 `canvas::drill_toggle_commands` 调整为
**`mw-core/src/drill.rs::drill_toggle_command(doc, id) -> Option<Command>`**：

- 理由：判定只读 `Document`（折叠态 + 直接子级），放在核心层可以**独立 `cargo test -p mw-core` 验证**，
  不必等 `mw-shell` 的窗口/GPU；
- 返回值直接是**最终命令**：0 条 = `None`（不发命令、不留空撤销记录）、1 条 = 单条 `SetCollapsed`、
  多条 = `Command::Batch`（顺序固定先父后子）——「一次双击 = 一条撤销」由构造函数保证，壳层不再自己做。
- 显式拒绝概念图（`doc.is_concept_map()`）与叶子/未知 id。

单测（`cargo test -p mw-core --lib drill::`，**6/6 通过**）：

| 测试 | 钉住什么 |
| --- | --- |
| `drilling_into_a_collapsed_node_reveals_exactly_one_level` | 折叠 A（B/C 曾展开）→ A 展开、B/C 被折回，只露一层 |
| `drilling_into_an_expanded_node_opens_only_its_direct_children` | 展开 A → 只展开直接子级，A 仍展开 |
| `drilling_is_a_noop_when_nothing_would_change` | 子级全展开 → `None`；折叠态仅剩「展开自己」一条仍成立 |
| `the_plan_skips_children_already_in_the_target_state` | 不复发已满足的子级；顺序先父后子 |
| `drilling_is_one_undo` | `undo_depth` 只 +1，撤销一步回到双击前 |
| `leaves_unknown_ids_and_concept_maps_do_not_drill` | 叶子 / 未知 id / 概念图 → `None` |

### 7.2 双击识别纯逻辑（2026-11-01 18:46 · 已完成）

`mw-ui/src/drill_click.rs`：`ToggleClickMemory::accept(click) -> Option<ToggleClick>`。
判据全部是屏幕常量 / egui 时间：同节点、`≤500ms`、`≤4 css px`、**撤销深度没变**（两次点击之间
别的命令落地就不算双击，避免把别人的命令当成第一击撤销）。命中时把第一击的现场交回来，
并清空记忆（第三击不会连着下钻）。

单测（`cargo test -p mw-ui --lib drill_click::`，**6/6 通过**）：双击成立 / 第三击不重复 /
换节点重新计数 / 超时不算 / 位移 5px 不算而 4px 内算 / 撤销深度变化取消。

回归：`cargo test -p mw-core` **337 passed / 0 failed**；`cargo test -p mw-ui` **57 passed / 0 failed**。

> 说明：并行会话在 18:20 起占用 `canvas.rs` / `canvas_methods_*.rs` / `panels/inspector.rs`
> 做 CM-17（关系线多选），本报告这一轮先落可独立验证的核心层；协调记录见
> `docs/x100/log/parallel-session-2026-11-01.md`。

### 7.3 待做（壳层释放 canvas 后补）

> **下列四项已于 19:44 全部落地**（实现见 §7.4，实拍与轨迹见 §7.5，门见 §9）。
> 清单原样保留作过程记录。

- `Canvas` 双击识别（egui 时间 + 同一点 500ms + ≤4 css px）与接线 `drill_toggle_command`；
- 轨迹 `toggle_drill id=… depth=N`；
- 两张新基线 `mm-toggle-drill` / `mm-toggle-drill-deep`，`--tol 0` 双跑；
- `stage-b` 门复跑（场次以 `docs/STATUS.md` §1 为准）；账本 M32 行与 STATUS 回填。

**夹具**：默认样例只有两层（根 → 分支 → 叶），拍不到「再深入一层」。新增验收钩子
`MW_DRILL_DEMO=<节点文字>`（默认「纯 Rust 原生」）：在该节点的一颗直接子级下再挂一颗
（例如在「wgpu 四层合成器」下挂「GPU 合成」），只在设了环境变量时生效，**不改默认样例**，
既有 `mm-*` 基线不受影响。两个场景都走这个夹具：

| 场景 | 脚本（坐标待 `scene_toggle` 轨迹量） | 终态 |
| --- | --- | --- |
| `mm-toggle-drill` | `click <圆点>`（折叠）→ `sleep 600` → `doubleclick <圆点>` | 只露直接子级；孙级仍隐藏 |
| `mm-toggle-drill-deep` | 上面再加 `sleep 600` → `doubleclick <圆点>` | 直接子级展开，孙级可见 |

`sleep 600` 是必需的：`wait N` 只推进 N 帧（约 16ms/帧），不够跨出 500ms 双击窗口，
会让「单击折叠」和紧随的第一次双击被判成同一次双击。

### 7.5 金样与行为证据（2026-11-01 19:22–19:26 · 已完成）

夹具：`MW_DRILL_DEMO=纯 Rust 原生`，`MW_NOTES=none`；折叠圆点实测屏幕中心 **`(661,442)`**
（`scene_toggle` 轨迹；加了一颗「GPU 合成」后落定相机的值）。

轨迹（`MW_TRACE`，两场）：

```text
mm-toggle-drill
  toggle idx=1 collapsed=true kids=2          # 单击 → 折叠
  toggle idx=1 collapsed=false kids=2         # 双击第一击（立即生效）
  undo label=折叠/展开 left=19                 # 双击第二击：先撤掉第一击
  toggle_drill id=… depth=1 open=1            # 再按「点之前」的状态下钻一层
mm-toggle-drill-deep
  … 同上 …
  toggle idx=1 collapsed=true kids=2          # 第二次双击的第一击
  undo label=折叠/展开 left=20
  toggle_drill id=… depth=1 open=0            # 展开直接子级，露出第三层
```

新基线两张（`--tol 0` 各跑两遍，**零差异**）：

| 基线 | 相对谁 | 像素 | 包围盒 | 说明 |
| --- | --- | --- | --- | --- |
| `mm-toggle-drill` | 同夹具「只单击折叠」的对照帧 | **61 774 px / 3.5307%**，max 210 | `(271,381)–(934,720)` | 直接子级露出、孙级仍藏 |
| `mm-toggle-drill-deep` | `mm-toggle-drill` | **11 975 px / 0.6844%**，max 195 | `(73,381)–(394,502)` | 只有「winit」子树展开、「GPU 合成」出现 |

**已知环境账（不是本轮改动）**：`stage-b` 既有基线多为 10-31 的旧图，而 TYPO-01
（2026-11-01 全局字体/排版重做，见 `docs/x100/typography-plan-2026-11-01.md`）让整页
纵向差 **2px**（底部状态栏上边线 `y=1056–1057` → `1054–1055`），例如 `mm-collapse`
对旧基线差 61 716 px / 3.527%。这轮的新基线是在 TYPO-01 之后的树上拍的；既有场景的
全矩阵重拍由 TYPO-01 那一轮负责，**本轮门结论必须等那次重拍完成后再跑**。
并行会话的重拍记录见 `docs/x100/log/parallel-session-2026-11-01.md` §1.1。

### 7.4 壳层接线设计（已按此实施，`canvas_methods_2.rs` / `canvas_methods_3.rs`）

第二轮已把两块纯逻辑落地并各自单测：

1. `mw-core::drill::drill_toggle_command(doc, id) -> Option<Command>`：给**点第一击之前**的文档状态，
   返回「恰好露一层」的命令（0 条 = 不变，1 条 = 单命令，多条 = `Batch`）；
2. `mw_ui::drill_click::ToggleClickMemory`：记第一击的 `(id, at, pos, collapsed_before, undo_depth)`，
   第二击只认「同节点 + ≤500ms + ≤4 css px + 撤销深度没变」，命中时把第一击现场交回来。

接线顺序（`canvas_methods_2.rs` 的点击处理，`Target::Toggle(n)` 分支）：

1. `Target::Toggle(n)` 命中时先构造 `ToggleClick`（`at` 取 `ui.input(|i| i.time)`，不是 `Instant`）；
2. `memory.accept(click)` 返回 `Some(first)` = 双击：**先 `self.undo()` 撤掉第一击**（安全阀：
   `doc.undo_depth()` 必须仍等于 `first.undo_depth`，否则按普通单击处理），此时文档回到第一击之前；
3. 再对回到的状态调 `drill_toggle_command(&doc, id)`：`None` 表示双击本来就不改变状态
   （如已全展开），撤销后**不留撤销记录**；`Some(cmd)` 则 `session.doc.apply(&cmd)` + `rebuild()`；
4. 非双击：照旧 `self.toggle(n)`，并记下这一击的现场；点击落在别处时 `memory.clear()`。
5. 轨迹 `toggle_drill id=… depth=N`，N = 本次命令实际改动的直接子级数（0 表示「双击无净变化」）。

**为什么不延迟单击**：500ms 的单击延迟会让折叠这个高频动作明显发粘；双击第二击到达时撤销+重发
只需两帧，且最终状态与「从未单击」等价（一次撤销记录）。代价是双击帧前会出现一帧第一击的中间态，
金样按最终帧截取不受影响。

**与双击节点进编辑的冲突**：圆点在节点盒外，命中分派时 `Target::Toggle` 先于节点处理
（`hit_at` 的 `Owner` 顺序不动）；节点双击编辑路径只认 `Target::Node`，两者不会互相吃掉。

## 8. 未做 / 风险

- 双击节奏与系统设置的双击间隔（Windows 默认 500ms）一致，实机可调间隔未读取。
- ~~壳层双击识别会与「双击节点进编辑」共用 `double_clicked_by`~~ **没有共用**：
  实现走自己的 `ToggleClickMemory`（按节点 id + egui 时间 + 位移 + 撤销深度），
  命中分派仍由 `hit_at` 的 `Owner` 顺序决定（圆点先于节点盒），节点双击编辑只认 `Target::Node`。
- ~~本轮未改单击语义，既有 `mm-collapse` / `mm-toggle-*` 基线应零漂移（待门验证）~~
  **已由 20:52–21:02 的正式 `stage-b` 门验证**：153 场两遍 306 次比较零差异（§9）。

## 9. 收口（打磨轮 66 · 2026-11-01 20:52–21:15）

本轮代码与两张基线在 19:09–19:44 就已落盘（壳层接线与实拍由并行会话完成，见 §7.1 末的协调说明），
缺的是**门与证据包**。收口做三件事：

### 9.1 正式机器门（补上 TYPO-01 欠的那一份）

TYPO-01 当时用 19:30 的冻结二进制逐场复刻门环境，明写「原脚本待并行会话停止后补跑」。
并行会话 19:55 后停止重建，本轮 20:52 起用正式脚本按**当前源码**重编重跑：

| 门 | 时间 | 结果 |
| --- | --- | --- |
| `scripts/x100-stage-b-gate.ps1` | 20:52–21:02 | **MACHINE GATE PASS**：证据包 X11–X19 PASS、PowerShell 语法 PASS、`cargo fmt --check` PASS、clippy 新增 0、全量测试 **1202 passed / 0 failed / 1 ignored**、golden **153 场 × 两遍 = 306 次比较零差异**（含 `mm-toggle-drill` / `mm-toggle-drill-deep`）。日志 `target/x100/stage-b-gate.log`，副本 `gate/stage-b-gate.log` |
| `scripts/x100-stage-cm-gate.ps1` | 21:03–21:07 | **MACHINE GATE FAIL（与 M32 无关）**：72 场里 70 场两遍零差异，`cm-multi-menu` 差 **67 106 px / 2.913%**（bbox `x[346..808] y[137..795]`）、`cm-submenu-bottom` 差 **243 935 px / 16.288%**（bbox `x[366..828] y[19..779]`），**两遍数字逐位相同 = 确定性漂移**。日志 `target/x100/stage-cm-gate.log`，副本 `gate/stage-cm-gate.log` |

M32 只碰思维导图的折叠圆点，两张 cm 场景是概念图**多选菜单**——逐像素比对确认差异只有一行：
菜单首行多出「创建分组」（`app_methods_2.rs` 的 `items.insert(0, Item::row("创建分组", "", A::CreateGroup, true))`），
整张菜单与子菜单因此下移一行。

### 9.2 顺带量出来的一盏红灯（不在本轮范围内解决）

「创建分组」来自并行会话实现的 **CM-18 概念分组**，而 `docs/31` 给 CM-18 的状态是 `待拍板`、
`docs/32` §13 明写「拍板前不写代码」、`docs/STATUS.md` §3 的 CM-18 行此前还写着「未写一行代码」。
实际工作树里（未提交）已经有：`ConceptGroup`（`mw-core/src/model.rs`）、`Document.groups`、
五条分组命令 + 6 条单测、`SceneGroup`（思维导图不画）、**`.zmind` 顶层 `groups` 键**
（`mw-format/src/lib.rs`，`serde(default, skip_serializing_if = "Vec::is_empty")`、`FORMAT_VERSION` 仍 1）
+ 1 条落盘测试、壳层九个文件的接线。这撞在 `AGENTS.md` §8「其它字段仍不动 / 要用户拍板」这条硬约束上。

本轮的处理边界：**不重拍**（不给未拍板的改动盖视觉章）、**不回退**（不是本会话的活，且禁止清理他人未提交工作）、
**不改拍板状态**（那是用户的决定）。已把红灯写进 `docs/STATUS.md` §1 与 §6 第 11 条，等用户二选一。

### 9.3 本轮改到的文档

- `docs/02`「展开层级」行：验收要点从 `—` 补成可判的三条（只露一层 / 一条撤销 / 单击零变化）。
- `docs/06` §5 折叠圆点行：删掉「**未实现**：双击『展开下一级』」，改成实际语义与判据。
- `docs/x100/polish-ledger.md`：§1 新增 M32 行；§3 候选第 13 条划掉并写完成口径。
- `docs/STATUS.md` §0 / §1 / §3（CM-18 行）/ §5 / §6；`AGENTS.md` 头部轮次与断点。

