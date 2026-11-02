# M18 · `Ctrl+X` 多节点删除确认（打磨轮 38）

> 依据：`HANDOFF.md` §7.5③「**`Ctrl+X` 多节点不弹删除确认**：目前剪切直接走，撤销可救，但不一致」。
> 状态：**先验证后改**——读码看这条结论可能已经过期；本轮用轨迹 + 金样钉住真实行为。
> 如果行为已正确，本轮不改产品代码，只补证据与回归；如果不正确，再改。

## 1. 要回答什么

1. 选中**带子节点**的节点按 `Ctrl+X`，是否和 `Delete` 一样弹「删除 N 个节点？」确认框？
2. 弹框后按 `Esc` 或「取消」，文档是否**一个节点都没少**（剪切不该偷偷先删）？
3. 叶子节点按 `Ctrl+X` 是否仍然一键剪切（不破坏最高频动线）？
4. 这条路径有没有金样/轨迹钉住？——**目前没有**：`delete_confirm_plan` 只有一条纯函数单测，
   `apply_clipboard(Cut)` / `cut_action()` 的确认行为没有端到端证据。

## 2. 读码结论（先于任何改动）

- `Canvas::cut_action()`（`canvas_methods_1.rs` 剪切菜单入口）最后一行是 `self.delete_selected()`。
- `Canvas::apply_clipboard(ClipboardChord::Cut)`（`Ctrl+X` 的窗口级和弦）同样是
  `copy_selected()` + 写系统剪贴板 + `self.delete_selected()`。
- `delete_selected()`（`canvas_methods_3.rs`）在 `delete_confirm_plan(&doc, &ids)` 返回 `Some(count)`
  时只设置 `confirm_delete = Some(DeleteConfirm { ids, count })`，**不删任何节点**；
  叶子（无子节点）才直接 `delete_now(&ids)`。
- 因此 HANDOFF §7.5③ 的「剪切直接走」**与当前代码不符**，极可能是旧结论。
  但「读码正确」不等于「真机正确」：需要一条真实按键轨迹，证明 `Ctrl+X` 确实到达这条分支、
  确认框确实画出来、点取消后文档没变。

## 3. 非目标

- 不改剪切/删除的产品语义（确认规则、默认按钮、撤销粒度都保持现状）。
- 不改 `mw_render::Frame`、不改 `.zmind`、不改 `Node` 字段。
- 不新增依赖、不新增 crate。

## 4. 验收与证据（实测）

| 判据 | 结果 |
| --- | --- |
| 带子节点 `Ctrl+X` 挂起确认、不删节点 | **通过**：轨迹 `canvas_click hit=<目标>` → `key ctrl+X` → `delete-confirm pending=3 selected=1` → `dialog id=id_2993`；金样 `mm-cut-confirm` |
| 确认框可取消且文档不变 | **通过**：`Esc` 后轨迹 `delete-confirm cancel count=3 reason=esc`；`mm-cut-cancel` 与「只选中、不剪切」对照**逐像素 0 px** |
| 叶子 `Ctrl+X` 仍一键剪切 | 既有纯函数单测 `delete_confirm_plan`（叶子 → `None`）覆盖；`cut_action` / `apply_clipboard(Cut)` 都走同一个 `delete_selected()`，没有第二条规则 |
| 既有行为零漂移 | 见 §5 门记录 |

像素差：`mm-cut-confirm` vs 只选中 = **2 304 000**（整屏，确认框 + 全屏遮罩）；
`mm-cut-cancel` vs 只选中 = **0**；`mm-cut-confirm` vs `mm-cut-cancel` = 2 304 000。详见 `diff.txt`。

## 5. 实际改动与门

1. **产品行为零改动**。`Ctrl+X` 本来就走 `delete_selected()` 的确认分支，HANDOFF §7.5③ 是旧结论。
2. `canvas_methods_3.rs` 加三条 `MW_TRACE` 事件（只写轨迹，不参与逻辑）：
   `delete-confirm pending=N selected=M`、`delete-confirm accept count=N`、
   `delete-confirm cancel count=N reason=esc`（按钮/关闭走同名前缀，无 reason 字段）。
3. 新增 `shots/golden/inputs/mm-cut-confirm.txt` / `mm-cut-cancel.txt`；
   新增基线 `shots/golden/mm-cut-confirm` / `mm-cut-cancel`（`--tol 0`，重建后复跑仍 0 差异）；
   `scripts/x100-stage-b-gate.ps1` 登记两场（119 → **121** 场）。
4. 机器门：`stage-b` **121 场 / 242 次比较**（`01:38:52–01:46:51`，pass 1 / pass 2 全过，
   `MACHINE GATE PASS`；全量测试 **1025 passed / 0 failed**）；`stage-cm` 51 场 / 102 次比较随后复跑。
   `cargo fmt --all -- --check` 干净；全量 clippy 复跑见 `docs/STATUS.md` §1（基线 36、新增 0）。

## 6. 未做 / 风险

- 实机键盘（真实 winit `Ctrl+X`）与金样注入的按键路径不完全相同；离屏证明的是事件到达后的行为。
- 「多选后剪切」与「单节点带子树剪切」走同一条 `delete_selected()`；本轮拍后者，
  多选路径由同一函数覆盖，但没有单独拍图。
- 确认框「删除」按钮的接受路径没有单独金样（会真删节点）；`delete_now` 已有既有测试与撤销覆盖。

