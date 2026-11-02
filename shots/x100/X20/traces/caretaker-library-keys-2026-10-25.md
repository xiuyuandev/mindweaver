# 门后维护 ⑭ · 笔记库页在模态弹窗下的键盘归属（2026-10-25）

> 结果：**已实现 · 待人工随 X20 一并验收**。这是 2.11 末段留下的
> 「卡片库/阅读页等页面型浮层未扩面」，属于 Stage B 修复，不是新一轮，
> 不改变 X20 门结论，也不越过人工门。

## 缺口

笔记库页自绘整页，键盘直接读 `ctx.input`/`consume_key`：Esc、↑↓、Enter
都不经过 egui 焦点系统。模态弹窗（设置/移出确认/正在打开，以及 QA 的
`MW_DIALOG_DEMO`）在栈顶时，`egui::Modal` 只挡指针与 Tab 巡游，挡不住
这些裸键。1080×720、ppp 1.5、`MW_LIBRARY_DEMO=1 MW_DIALOG_DEMO=1` 实拍：

- 弹窗开着按 `↑`：右侧预览从「键盘色板验证」切到「视图跟随」，
  即暗幕背后的库焦点被移动。
- 再按 `Enter`：直接打开「视图跟随」的卡片阅读页，弹窗被绕过。

这与 2.9/2.10/2.11 的「浮层是第一键盘主人」冲突；在真实场景里
（设置/移出确认上）同样会误开卡片。

## 规则

- 笔记库页的裸键只在没有前台模态时归库：`page_keys_owned(top_modal,
  dialog_demo)`——模态栈顶或演示弹窗开着时，Esc / ↑↓ / Enter 整段不处理。
- 移出确认框自己的 Esc/Enter 仍由 `dialog` 壳处理；库页在下面保留状态。
- 不改库页布局/滚动/预览；无模态时行为与 X20 基线逐像素一致。

## 验收

- 纯函数单测 `page_keys_yield_to_foreground_modals`：模态/演示弹窗任一
  在栈顶都不归库页。
- 新脚本 `shots/golden/inputs/library-modal-input.txt`（`wait 8 →
  ArrowDown → wait 2 → Enter → wait 8`），以及对照脚本
  `target/x100/probe-library-modal/ctl-{open,down}.txt`。
- 修复前：`ArrowDown` 后预览变成「视图跟随」；`Enter` 后打开卡片阅读页
  （102,116 B）。对照图 244,728 B 证明弹窗确实盖在库上。
- 修复后：`ArrowDown` 后预览仍是「键盘色板验证」；`Enter` 落到弹窗默认
  「确定」并关闭弹窗（trace `dialog demo closed` / `dialog demo action 1`），
  库页留在原处（264,292 B）。
- 阶段门：31 组 × 2 = 62/62 全 0 差异、765 tests / 0 failed、fmt 干净、
  mw-shell clippy 仅 3 条历史；基线未被改写。

## 证据文件

- `caretaker-library-keys-context-2026-10-25.png/.trace`（弹窗 + 库对照）
- `caretaker-library-keys-down-before-2026-10-25.png/.trace`（修复前 ↑）
- `caretaker-library-keys-before-2026-10-25.png`（修复前 Enter 后打开阅读页）
- `caretaker-library-keys-down-after-2026-10-25.png/.trace`（修复后 ↑）
- `caretaker-library-keys-after-2026-10-25.png/.trace`（修复后 Enter 归弹窗）
- `caretaker-stage-b-gate-start-menu-library-2026-10-25.log`
- 脚本：`shots/golden/inputs/library-modal-input.txt`
