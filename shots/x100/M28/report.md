# M28 · 撤销栈上限可配（100–2000）+ 设置中心「编辑」组（打磨轮 51）

- 日期：2026-10-31
- 依据：`docs/02` §2「撤销/重做」的 P0 行——
  「`Ctrl+Z` / `Ctrl+Y`（兼容 `Ctrl+Shift+Z`）；**栈上限默认 500 步（可配 100–2000）**；
  文本编辑按“提交”粒度入栈」。
- 代码：`crates/mw-core/src/document.rs:21` 的 `HISTORY_LIMIT: usize = 500`，
  以及两处写死的裁剪 `if self.undo_stack.len() > HISTORY_LIMIT { remove(0) }`
  （`apply` 与 `apply_coalesced`）；`crates/mw-shell/src/settings.rs` 的全局 `Store`；
  `crates/mw-shell/src/settings_ui.rs` 的设置中心。
- 状态：**已实现**（机器门绿；`stage-b` 139/278、`stage-cm` 58/116 两遍全过；
  人工门未开）

## 1. 现状（代码复核，先于改动）

1. 撤销栈上限是**编译期常量** `HISTORY_LIMIT = 500`：`Document` 没有字段，
   两条入栈路径都直接与常量比较。用户无法改成 100 或 2000。
2. `redo()` 把撤销的一步压回 `undo_stack` 时**不裁剪**（`document.rs` 的 `redo()`）；
   上限若降到 100，重做若干步后栈会超过 100——现在是常量且只有 `apply` 两条路裁剪，
   这个缺口被「最多 500」掩盖。
3. 设置中心已有 `Store`（`settings.json`）与四个组：外观 / 自动保存 / 导出默认值 / 笔记库路径；
   没有「编辑」组，也没有撤销步数。
4. `docs/02` 的 P0 行从 10-18 起就写着「可配 100–2000」，`docs/13` 也记着栈深固定；
   代码与合同不一致。这不是假想缺口：20k 文档上每步撤销都带一份 `ChangeSet` 快照，
   能不能把深度压低是用户能感知的内存 / 响应取舍。

## 2. 范围

1. **`Document` 自带上限**：新字段 `history_limit: usize`（不进序列化、
   不写 `.zmind`、不影响 `FORMAT_VERSION`），默认 `HISTORY_LIMIT = 500`；
   `pub fn history_limit()` / `pub fn set_history_limit(limit)`，
   `set` 时 clamp 到 `MIN_HISTORY_LIMIT = 100..= MAX_HISTORY_LIMIT = 2000`，
   并**立即裁剪**现有 `undo_stack`（丢最旧的），空栈/已相等时不做无谓搬移。
2. **三条入栈路径统一**：`apply`、`apply_coalesced` 的裁剪改读 `self.history_limit`；
   `redo()` 把重做的那一步压回 `undo_stack` 后同样裁剪，堵住「降上限 + 重做」越界。
3. **全局设置**：`settings::Store` 新增 `undo_limit: usize`，serde camelCase
   `undoLimit`，缺键回落 500；`undo_limit()` 取值时 clamp 100–2000。
4. **设置中心**：新增「编辑」组（自动保存之后），一行「撤销步数」滑轨
   （100–2000，步进 100）+ 当前值 + 一句说明：
   「默认 500 步；调小会立即丢弃较旧的撤销记录，无法恢复。重做同样受此上限约束。」
5. **应用时点**：保存设置后立即作用到当前画布与所有标签页的文档；
   之后新建 / 打开 / 导入 / 恢复的文档也在进入画布时套用同一上限——
   在 `Shell` 每帧的廉价同步里做（`usize` 相等就返回），不逐个改打开路径。
6. **验收旋钮**：`MW_UNDO_LIMIT=100` 启动时只改内存中的 `settings.undo_limit`
   （不落盘），给行为脚本与轨迹一个确定性靶子。
7. **轨迹**：`history limit=<n> depth=<undo>/<redo>`，只在生效或裁剪时打一条；
   `settings save undo_limit=<n>` 在保存设置时打一条。
8. **单测**：
   - `history_limit_defaults_and_clamps`（新建文档默认 500 + 上下界 100/2000）；
   - `lowering_history_limit_drops_oldest_and_undo_stops_there`（先 130 条再降到 100，
     栈深 100、最旧 30 条丢弃、调高上限也补不回来）；
   - `redo_cannot_push_history_past_limit_after_lowering`（150 条 → 撤 60 → 降到 100 →
     重做 60，栈深仍 100）；
   - `coalesced_apply_honors_history_limit`（合并入栈路径 101 条后仍是 100，合并路径不加深）；
   - `settings_roundtrip_and_clamp`（`undoLimit` 落盘键名 + 往返）；
   - `undo_limit_missing_uses_default_and_clamps`（缺键回落 500 + 越界取值 clamp）。
9. **金样**：`settings`（`MW_SETTINGS_DEMO=1` 打开设置中心，钉住「编辑」组与新滑轨），
   进 `stage-b` 门；`settings` 此前没有任何基线，本轮补上。
10. **文档**：`docs/02` §2 该行改口径（已实现 + 证据指针）；新增账本 M28 行；
    `docs/STATUS.md`、执行日志、`AGENTS.md`、`HANDOFF.md` 回填。

落地位置：`crates/mw-core/src/document.rs`（字段 / 三条路径 / 单测）、
`crates/mw-shell/src/settings.rs`（字段 / clamp / 单测）、
`crates/mw-shell/src/settings_ui.rs`（编辑组）、
`crates/mw-shell/src/main.rs`（保存设置时同步 + 每帧廉价同步）、
`crates/mw-shell/src/app_methods_1.rs`（`MW_UNDO_LIMIT`）、
`shots/golden/inputs/` 与 `scripts/x100-stage-b-gate.ps1`（`settings` 金样）。

## 3. 非目标

- 不改默认值：出厂仍是 500 步、出厂外观与既有基线零漂移。
- 不把撤销步数写进 `.zmind`（它是壳层偏好，不是文档内容；同一份文档换机器打开不受影响）。
- 不做「按文档覆盖」、不做撤销历史面板 / 快照管理。
- 不改 `ChangeSet` 结构、不做快照去重 / 增量压缩（内存优化另开轮次）。
- 不改 `Ctrl+Z` / `Ctrl+Y` / `Ctrl+Shift+Z` 的既有判别与合并语义。
- 不动快捷键面板、不顺手重排设置中心其他组。

## 4. 验收条款

### 4.1 单元

| # | 用例 | 期望 |
| --- | --- | --- |
| 1 | 新建文档 | `history_limit() == 500` |
| 2 | `set_history_limit(99)` / `(2001)` | 分别读回 100 / 2000 |
| 3 | 130 条命令后降到 100 | `undo_stack` 深 100，最旧 30 条不可撤销（文本仍是被丢弃后的状态） |
| 4 | 100 上限、100 条命令，undo 一次再 redo | 栈深仍 100，不越界 |
| 5 | `Store` 往返 + 越界 | `undo_limit = 2001` 读回后 `undo_limit() == 2000`；写盘读盘一致 |
| 6 | 老 `settings.json`（无 `undoLimit`） | 其余字段保留，`undo_limit()` 为 500 |
| 7 | 两条入栈路径 | `apply` 与 `apply_coalesced` 都按自定义上限裁剪 |

### 4.2 端到端

- `MW_SETTINGS_DEMO=1`：金样 `settings`，设置中心出现「编辑 / 撤销栈上限（步）」行，
  滑轨停在 500，说明文案一行。
- `MW_UNDO_LIMIT=100`：首帧轨迹 `history limit=100 depth=17/0`
  （样本文档启动时已有 17 条历史；17 < 100，同步没有清空历史）。
- 默认启动：沿用 500，既有场景零净漂移（设置中心此前不在任何现有场景里）。
- 降上限的裁剪由单测覆盖（130 条编辑配图代价过高，不为像素重复单测）；
  `settings-before.png` 用 `shots/v10/settings-snow/` 的旧版截图充作改前对照，
  只用于看「编辑」组此前不存在，不作为逐像素基线。

### 4.3 证据

`shots/x100/M28/`：`report.md`、`manifest.json`、
`settings-before.png`（v10 旧版设置中心，无「编辑」组）、
`settings-after.png`（改后「编辑」组，滑轨 500）、
`settings-limit-100.png`（`MW_UNDO_LIMIT=100` 时滑轨 100）、
`trace-limit-100.txt`（`history limit=100 depth=17/0`）、
金样在 `shots/golden/settings/`。

### 4.4 实测（2026-10-31）

- 单测：`cargo test --workspace -- --test-threads=1`（TEMP/TMP → `target/tmp`）
  **1081 passed / 0 failed / 1 ignored**（前 1076）。新增 5 条：
  `history_limit_defaults_and_clamps`、`lowering_history_limit_drops_oldest_and_undo_stops_there`、
  `redo_cannot_push_history_past_limit_after_lowering`、`coalesced_apply_honors_history_limit`
  （`mw-core`），`undo_limit_missing_uses_default_and_clamps`（`mw-shell`）；
  `settings::tests::store_roundtrips_through_disk` 补 `undo_limit: 1200` 与 `"undoLimit"` 断言。
  分项：`mw-core` 282、`mw-shell --bins` 615。
- 金样：`settings` 新建（sha256-12 `A34B7458CF0E`），设置中心「编辑」组滑轨停在 500；
  `MW_UNDO_LIMIT=100` 截图为 `settings-limit-100.png`，与 `settings-after.png` 只差
  **812 px**，包围盒 `(445,476)–(632,511)`（1620×1080）——正好是滑轨行，组标题/说明/其余四组零差异。
- 行为：`MW_UNDO_LIMIT=100` 首帧轨迹 `history limit=100 depth=17/0`
  （样本文档自带 17 条历史，17 < 100，同步没有清空历史）；此后值不变，不再打第二条。
- 每帧对账：活动画布先比 `history_limit()`，相等不写也不打轨迹；后台标签每帧一次
  `set_history_limit`（clamp + 栈长比较，无搬移），O(标签数) 且不遍历文档。
- 门：`scripts/x100-stage-b-gate.ps1` 2026-10-31 `13:08:03–13:15:40`，
  **139 场 / 278 次比较**两遍 `--tol 0` 全过（`MACHINE GATE PASS`），既有 138 场零净漂移；
  `scripts/x100-stage-cm-gate.ps1` 随后顺序复跑（见 §4.5）。
- 基线目录 199 → **200**；输入脚本仍 134（`settings` 无输入脚本）。

### 4.5 stage-cm 复跑与一次偶发

- 第 1 次顺序复跑 `13:16:41–13:19:49`：`fmt` / clippy / 概念图往返 / 两遍 golden（58×2）全过，
  但全量测试里 `mw-render` 的 `rendering_is_deterministic_and_dumps_a_picture` 在 1.5 倍
  缩放处报「两次渲染不一致」（同一进程连渲两帧）。该用例与撤销栈无关（本轮零渲染改动），
  单跑该用例 **1 passed**；`13:20:59–13:24:06` 顺序复跑整道门
  `tests passed=1081 failed=0`、**58/116 两遍全过**、`MACHINE GATE PASS`。
- 结论：这是一次负载下的 GPU 帧偶发，不是本条改动的回归；记在这里，不把第一次的红藏掉。

## 5. 风险 / 待实测

- 每帧同步所有标签页的 `usize` 相等即返回；标签页很多时也要保持 O(标签数)，
  不在这里做任何文档遍历。
- 降上限后旧记录**真的丢弃**（不可恢复），说明文案必须让用户知道；不弹二次确认。
- `redo` 裁剪会丢最旧的撤销——这是「上限」语义的自然结果，单测钉住。
- 滑轨步进 100 意味着 150 需要键入或微调；本轮不做数值格（设置中心仍是 egui 原生控件，
  与现有四组一致），若需要精确值再另开轮次。
- 实机 IME / 键盘手感不在本轮范围。
