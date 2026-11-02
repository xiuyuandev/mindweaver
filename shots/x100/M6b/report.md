# M6b · 结构移动的边界反馈（把「按了没反应」说清楚）

> 打磨轮 60 · 2026-11-01
> 账本：`docs/x100/polish-ledger.md`（M6 行的「已知粗糙」）
> 合同：`docs/02` §3/§10「`Alt+↑/↓` 同级上移 / 下移」；`docs/26` 的可发现性与反馈要求

## 1. 为什么是这一轮

账本 M6 记着两条粗糙：①「跨父级多选按 `Alt+↓` 什么都不动且不给反馈」；
② 边界上按 `Alt+↑/↓` 没有任何提示。本轮先复核 ①：

- 新增单测 `batch_move_cross_parent_moves_each_within_its_own_parent`：
  两颗不同父级、各自都有后兄弟的主题一起 `MoveDown` → **各自在自己的父级内换位**，
  证明「跨父级多选什么都不动」是**旧结论，不成立**（`batch_structure_command`
  本来就是逐颗按自己的父级规划）。
- 真正剩下的是 ② 和「多选里部分到边界」：`move_command` / `batch_structure_command`
  返回 `None` 时调用方静默返回 `false`，用户只看到「按了没反应」。

本轮只做反馈：边界上给出**具体原因**（根主题 / 已经是第一个 / 已经是最后一个 /
整组已在边界），并记一条轨迹。

## 2. 范围

- 纯函数 `move_block_reason(doc, id, delta) -> MoveBlock`：
  `Root` / `First` / `Last` / `None`，只读文档，供单测与提示文案共用。
- `Canvas::move_selected`：
  - 单选：`move_command` 为 `None` 时按原因给 toast（`notice`，非告警色）；
  - 多选：`move_step_many` 返回 `false` 时给「选中的主题已经在边界，无法再移动」；
  - 轨迹 `structure_move_blocked delta=… selected=… reason=root|first|last|batch`。
- 新基线 `mm-move-blocked`：打开 `tiny-cn` 夹具 → 点「两周内完成首版」（第一个子主题）
  → `Alt+↑`。画面应与「只选中」**逐像素相同**，另加一条提示条；轨迹给出 `reason=first`。
- 单测：跨父级批量下移（复核旧结论）+ `move_block_reason` 四种情形。

## 3. 非目标

- 不改 `move_command` / `batch_structure_command` 的移动语义（只加反馈）。
- 不把边界项置灰（键盘没有可置灰的入口），也不新增菜单项。
- 不改 `.zmind` / `Node` / 命令。

## 4. 验收场景

1. **第一个上移**：选中第一个子主题按 `Alt+↑` → toast「已经是第一个同级主题」，
   文档不变、撤销栈不变。
2. **最后一个下移** →「已经是最后一个同级主题」。
3. **根主题** →「根主题不能移动」。
4. **多选到边界** →「选中的主题已经在边界，无法再移动」。
5. **能移动时零变化**：`Alt+↑/↓` 成功路径的基线（`mm-move-keys` / `mm-move-multi`）零漂移。

## 5. 预计证据

- 新基线 `mm-move-blocked`（`--tol 0` 双跑）。
- 轨迹 `structure_move_blocked … reason=first`。
- `stage-b` 145 → 146 场 / 290 → 292 次比较；`stage-cm` 64/128 顺序复跑。

## 6. 实测结果（2026-11-01）

- **旧结论复核**：`batch_move_cross_parent_moves_each_within_its_own_parent` 通过——
  跨父级多选 `MoveDown` 时两颗各自在自己的父级内换位，账本 M6 的
  「跨父级什么都不动」**不成立**（已在账本行改写）。
- **反馈落地**：`move_block_reason` 四态；`move_selected` 在
  `move_command` 为 `None` / `move_step_many` 返回 `false` 时给 `notice`：
  「根主题不能移动 / 已经是第一个同级主题 / 已经是最后一个同级主题 /
  选中的主题已经在边界，无法再移动」，并记
  `structure_move_blocked delta=… selected=… reason=Root|First|Last|batch`。
- **金样**：点「uniform grid 命中」（第一个子主题）→ `Alt+↑`：
  轨迹 `structure_move_blocked delta=-1 selected=1 reason=First`；
  与「不发键」的对照帧差 **10 760 px**，bbox `(16,1028)–(375,1057)`——
  **只有底部提示条**，文档/相机/选中环零变化，证明这一下没有偷偷移动。
  双跑 `--tol 0` 零差异。
- **门内抓到 `mm-move-multi` 的真相**：旧脚本 (600,430)→(780,640) 实际只框到根
  「织思」（`structure move … selected=1 path=single` + `reason=Root`），
  旧基线锁的是「根主题按 Alt+↓ **静默不动**」——新反馈一上，门立刻红 10 760 px
  （恰好也是提示条区域）。这正好说明这个场景从来不是「整组下移一格」。
  已改脚本框选两颗可动子主题（`marquee picked=2` → `structure move … selected=2
  path=batch`，无 blocked），基线按现状重拍并在门脚本里记了这段考证；
  「根主题被拦 + 提示」的机器证据由 `mm-move-blocked` 的 `reason=First` 与单测承担。
- **单测**：`move_block_reason_covers_root_first_last_and_free`（四态）与
  `batch_move_cross_parent_moves_each_within_its_own_parent`（跨父级批量）；测试 1124 → **1126**。
