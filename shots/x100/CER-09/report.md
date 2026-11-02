# CER-09 · 大纲拖拽与折叠

> 状态：`已实现`（证据齐；非门包，待 G-B2-1 人工复核）· 批次 B2-A · 依赖：CER-08（已完成）
> 设计合同：[`docs/28`](../../../docs/28-内容体验重构规范.md) §5.6（拖拽）、§5.2/§5.3（握把/拖拽状态）
> 执行合同：[`docs/29`](../../../docs/29-内容体验重构工作包.md) §6（X64 范围并入）

## 1. 范围

对照 §5.6 逐项落地大纲拖拽：握把悬停延迟、三区落点、行幽灵、拖拽源降透明度、
非法落点提示、折叠节点自动展开、边缘自动滚动、Esc 取消、多选一次撤销、
跨父级移动后展开目标父级；并补 `outline-drag` 系列场景与 trace。

## 2. §5.6 契约落地（2026-10-26 代码实测）

| §5.6 项 | 开工前 | 本包处置 |
| --- | --- | --- |
| 悬停 120ms 后显示 6 点握把 | ❌ 悬停即时显示 | **补**：`GRIP_DELAY=120ms`；`grip_since` 记住同一行的悬停起点，`grip_ready()` 纯函数判定（选中即时） |
| 拖动超过 4px 才进入拖拽 | ⚠️ egui `drag_started` 阈值（约 6px，不可局部配置） | **记偏差**：不引入自定义阈值状态机；误触由 egui 阈值兜住（报告 §8） |
| 行幽灵 90% 不透明度 | ✅ `panel2` 90% + 圆角 6、无边框 | 不动（已符合） |
| 上/下 1/4 插入前/后，中 1/2 成子级 | ✅ 0.25/0.75 | 抽出 `drop_zone(rel)` 纯函数并加边界单测 |
| 目标自身/后代：禁止光标 + 红色 1px 线 | ⚠️ 有判定、线只覆盖上/下区，Child 区不画 | **补**：Child 非法落点改画红 1px 整行描边；光标 `NoDrop` |
| 悬停折叠节点 500ms 自动展开 | ✅ 500ms | 不动（注释里的旧 600ms 已改） |
| 视口上/下 24px 自动滚动 | ✅ 24px 边带（4→18px/帧） | 不动（已有单测） |
| `Esc` 取消拖拽，文档不变 | ❌ 实际未接（旧审计误记 ✅） | **补**：拖拽中消费 `Esc`，清 `dragging/drag_ids/drag_hover`，trace `outline_drag_cancel` |
| 松开后一条撤销记录 | ✅ 单条 `MoveNode` / 多选一条 `Batch` | 不动（`batch_is_single_history_entry_and_rolls_back_on_error` 覆盖） |
| 多选拖拽保持相对顺序 | ✅ `MoveMany`（`Command::Batch`） | 不动（`move_many_keeps_group_order_for_specific_and_appended_drops`） |
| 跨父级移动后展开目标父级 | ✅ `OA::Move` / `OA::MoveMany` 应用后 `toggle_collapsed` | 补 trace `outline_expand_parent`；行为脚本实证 |
| 拖拽源行 40% 不透明度（§5.3） | ❌ 未实现 | **补**：`Painter::set_opacity(0.4)`（含背景/文字/圆点），幽灵不受影响 |
| 成为子级目标整行 6% 底、无描边 | ✅ 15/255 ≈ 5.9%，无描边 | 圆角 5 → 6（与行半径一致），非法态改红描边 |

## 3. 实现清单

1. `crates/mw-shell/src/panels/outline.rs`
   - 常量 `GRIP_DELAY = 120ms`；纯函数 `grip_ready(selected, hovered_for)`、`drop_zone(rel)`。
   - `Outline::grip_since: Option<(String, Instant)>`：本帧只认「指针真正停住且未选中」的行；
     同一行保留起点，换行/离开重新计时；`focus_on` 时清空。
   - 行绘制：`painter.set_opacity(0.4)`（拖拽源）；握把条件改为 `grip_ready`。
   - 落点：`drop_zone` 取代内联三分支；Child 非法落点画红 1px 整行描边（合法 6% 底、圆角 6）。
   - 拖拽块：`Esc` 取消（`consume_key`，不再传给逐级退出链）。
   - trace：`outline_drag_start` / `outline_drag_spot` / `outline_drag_illegal` /
     `outline_drag_cancel` / `outline_drag_ghost`。
2. `crates/mw-shell/src/app_methods_2.rs`
   - `OA::Move` / `OA::MoveMany` 展开目标父级处补 `outline_expand_parent` trace。

## 4. 非目标

- 不做自定义 4px 拖拽阈值状态机（egui 默认近似，偏差留档）。
- 不做拖拽动画曲线；不动 `Command::MoveNode` / `MoveMany` 语义。
- 不重做握把几何（§5.2 的 16×20 盒归 CER-05；当前 2×3 点阵维持既有外观，见 §8）。
- 不改 `.zmind` / `Node` / `NoteCard` 字段与版本。

## 5. 验证

### 5.1 行为（`MW_TRACE`，证据在 `traces/`）

| 脚本 | 期望 | 实测 |
| --- | --- | --- |
| `outline-drag.txt` | 拖到「中文排版」中部 = Child 预览 | `outline_drag_start` + `outline_drag_spot row="中文排版" zone=Child` |
| `outline-drag-drop.txt` | 松手落一条 `MoveNode`（父级换到中文排版） | `outline_move_drag … parent=… index=None` |
| `outline-drag-cancel.txt` | `Esc` 取消，无落盘 | `outline_drag_cancel`；**无** `outline_move_drag` |
| `outline-drag-expand.txt` | 落到折叠节点后自动展开 | `outline_move_drag` + `outline_expand_parent id=…` |
| `outline-drag-illegal.txt` | 落进后代 = 非法 | `outline_drag_spot row="wgpu 四层合成器"` + `outline_drag_illegal` 每帧 |

### 5.2 视觉（ppp 1.5，`--tol 0`）

| 场景 | 窗口 | 结果 |
| --- | --- | --- |
| `outline-hover-select` | 1200×720 | **逐字节等于旧基线** `0031d764…`（握把延迟只改出现时机，稳定帧不变） |
| `outline-drag`（新增） | 1080×720 | `99cf601f…`，源行 40% + 目标 6% 底 + 幽灵 |
| `outline-drag-illegal`（新增） | 1080×720 | `1776a778…`，红 1px 整行描边 + NoDrop |

- 旧基线留档：`shots/x100/CER-09/baseline-before/outline-hover-select-ppp1.50.png`
  （SHA-256 `0031d764b7896170cff01d19f5ebc3ec6dd8bbc2bba504e97fdc519a124557a7`，
  与重拍后逐字节相同，漂移 0 px / 0.0000%）。
- 未受影响场景逐字节不变：`outline`、`outline-focus`、`outline-compact`、`outline-narrow`、
  `outline-edit`、`cm-outline` 各跑一次 `--tol 0` 全 0 差异。
- 三个新/改场景各连跑两次 `--tol 0` 均 0 差异（6/6）。
- 已加入 `scripts/x100-stage-b-gate.ps1`（38 → 41 组比较）。

### 5.3 工程

- `cargo fmt --all -- --check` 干净。
- `cargo test --workspace -- --test-threads=1` → **815 passed / 0 failed**（+2：`drop_zone` 分带、握把延迟）。
- `cargo clippy -p mw-shell --all-targets` → 仅 `export_pdf.rs` 3 条历史告警，新增 0。
- `cargo build --release -p mw-shell` 通过。
- `cargo test -p mw-format --test x100_fixtures -- --test-threads=1` → 2 passed（旧文件读取/往返）。
- 撤销：`batch_is_single_history_entry_and_rolls_back_on_error` 随全量测试通过；
  单行拖拽发一条 `MoveNode`，多选发一条 `Batch`。
- **机器门**：`powershell -File scripts/x100-stage-b-gate.ps1` →
  `MACHINE GATE PASS`，**41 组 × 2 遍 = 82/82 全 0 差异**，815 tests，
  finished 2026-10-26 12:19:04 +08:00（`target/x100/stage-b-gate-cer09.log`）。

### 5.4 门检附带修复：`start-tab-focus` 旧基线

首次跑门时只有 `start-tab-focus` 失败（稳定复现，与本包代码无关）：旧基线是**旧开始页的
「还没有最近文件」空状态**，而场景定义早已是 `MW_START_DEMO=1`（三条演示最近文件）。
按 §5 记录后重拍，并给脚本补了收尾 `move 24 690`（脚本存在时 harness 不清桌面指针，
悬停 wash 会随桌面光标漂进基线）：

- 旧：`ad05385ea4c334076db367c5cc7499d9099d8e0f78b46f2c682f50f6c7c4b35b`
  （留档 `baseline-before/start-tab-focus-ppp1.50.png`）
- 新：`8fc6018a55feece083eed70655d86a046e9e0ba0f4fd9170a91e7ae66b7dcc6b`
- 漂移：**11.9331%（79,045 px，最大差 221）**，bbox `(41,175)–(903,318)` —— 正好是
  「最近文件」段；原因见上，不是外观回归。重拍后双跑 `--tol 0` 0 差异。

## 6. 变更

- `crates/mw-shell/src/panels/outline.rs`：握把延迟、拖拽源透明度、`drop_zone`、Child 非法红框、
  Esc 取消、拖拽 trace。
- `crates/mw-shell/src/app_methods_2.rs`：`outline_expand_parent` trace。
- `shots/golden/inputs/outline-hover-select.txt`：hover 带 160ms 保持（稳定帧落在握把已出现之后）。
- 新增 `shots/golden/inputs/outline-drag{,-drop,-cancel,-expand,-illegal}.txt`。
- 新增 `shots/golden/outline-drag/`、`shots/golden/outline-drag-illegal/` 基线。
- `scripts/x100-stage-b-gate.ps1`：补三个大纲拖拽/悬停场景。
- `shots/golden/inputs/start-tab-focus.txt`：收尾把指针移出窗口；同场景基线按 §5 重拍
  （门检发现的旧开始页空状态基线，见 §5.4）。

## 7. 未做 / 遗留

- 4px 阈值仍由 egui 默认（约 6px）近似（§2 已记账）。
- 握把 16×20 盒 / 距列左 8 的几何未重做（§5.2 归 CER-05，当前 2×3 点阵）。
- 自动滚动、自动展开、多选拖拽只用既有单测 + trace 覆盖，未各拍一张拖拽中截图
  （静态 golden 不赌经过帧）；`outline-drag` 的 6 帧拖拽过程由 trace 与单帧终态共同固定。
- 拖拽落点预览未画「插入位置的行首圆点」以外的装饰（按 §5.6 仅线 + 圆点）。

## 8. 风险 / 偏差

- **偏差 ①**：4px 拖拽阈值未自建状态机；egui `drag_started` 在 click+drag 组合下要移动约 6px。
- **偏差 ②**：禁止光标用 `CursorIcon::NoDrop`（egui 无 `Forbidden` 变体），语义等价。
- 握把延迟依赖真实时间；golden 脚本用 `hover … 160` 把截图固定在阈值之后，
  连跑两次 0 差异。真实低速机器若一帧 >120ms，握把会提前出现，不影响正确性。
- `outline-hover-select` 基线沿用 1200×720；当前默认窗口是 1080×720，本包未顺手改窗口
  （避免把布局重排混进这次漂移）。
- `start-tab-focus` 旧基线是开始页 v2 之前的产物（空最近文件状态），门检时按 §5 重拍，
  漂移 11.93% 全部落在「最近文件」段；与本包大纲代码无关，但整道门因此才转绿。

## 9. 人工判词

非门包，不填；B2-A 末端的 G-B2-1 一并复核。
