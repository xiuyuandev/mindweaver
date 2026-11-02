# CER-03 · MW_TRACE 行为证据

复跑 X20 门后维护的既有行为脚本（脚本源在 `shots/golden/inputs/`，本包
`scripts/` 有同名副本），验证 `content_keys` 同源门改造后行为逐位不变。
所有脚本跑在 release 二进制上，环境变量与 X20 记录一致。

## 1. library-modal.trace — 笔记库 × 模态

```
MW_LIBRARY_DEMO=1 MW_DIALOG_DEMO=1 MW_WINDOW_SIZE=1080x720
MW_GOLDEN_INPUT=shots/golden/inputs/library-modal-input.txt
MW_TRACE=shots/x100/CER-03/traces/library-modal.trace
--golden --update --ppp 1.5 --warmup 24 --dir shots/x100/CER-03/traces/library-modal
```

判据（X20 修复后的既定行为，改造后必须原样复现）：

- `focus dialog action 1 · 确定`：焦点在弹窗默认按钮（弹窗拥有键盘）。
- 步骤 9 `key Down`：**无**库页预览变化、无焦点移动——裸键被 `page_keys_owned`
  门挡住（门现在委托 `content_keys::page_owns_bare_keys`）。
- 步骤 13 `key Enter` → `event dialog demo closed` + `event dialog demo action 1`：
  Enter 归弹窗默认「确定」，不落库页。
- 末帧 PNG（`library-modal/shell-ppp1.50.png`）：弹窗已关、库页保留、
  预览仍停在「键盘色板验证」。

## 2. start-menu-input.trace — 开始页 × 标题栏菜单 × 裸键 `1`

```
MW_HOME=1 MW_START_DEMO=1 MW_WINDOW_SIZE=1440x1000
MW_GOLDEN_INPUT=shots/golden/inputs/start-menu-input.txt
MW_TRACE=shots/x100/CER-03/traces/start-menu-input.trace
--golden --update --ppp 1.0 --warmup 24 --dir shots/x100/CER-03/traces/start-menu-input
```

判据：步骤 18 `key 1` 之后**无**模板创建/文档事件（X20 修复前 `1` 会在菜单
背后新建模板），焦点保持在开始页（`focus 开始页 · …`）。

## 3. start-menu-esc.trace — 开始页 × 标题栏菜单 × Esc

```
MW_HOME=1 MW_START_DEMO=1 MW_WINDOW_SIZE=1440x1000
MW_GOLDEN_INPUT=shots/golden/inputs/start-menu-esc.txt
MW_TRACE=shots/x100/CER-03/traces/start-menu-esc.trace
--golden --update --ppp 1.0 --warmup 24 --dir shots/x100/CER-03/traces/start-menu-esc
```

判据：`event popup_menu key=escape`——Esc 归菜单并收起，开始页不清空。

## 4. 门矩阵单测

`content_keys::tests`（`cargo test -p mw-shell content_keys`）：

- `gate_matrix_covers_every_foreground_combination`：32 种前台组合 × 2 判定。
- `legacy_page_gates_delegate_bit_for_bit`：两个旧函数委托后真值表逐位一致。
- `ime_composing_only_for_nonempty_preedit`：非空 Preedit 才算组合态。
- `tab_cycle_walks_slots_and_wraps_both_ways`：Tab 顺序首尾循环。
