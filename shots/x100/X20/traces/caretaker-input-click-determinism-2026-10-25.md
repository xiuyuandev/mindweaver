# X20 门后维护 · golden 点击原语确定性修复（2026-10-25）

结论：**归档的 X20 `narrow-menu` 证据在用归档脚本重放时不可复现**；根因不在产品
外观，而在 `MW_GOLDEN_INPUT` 的点击原语把「指针移入」和「按下」放在同一帧。
修复后，同一二进制、同一脚本、同一 warmup 重放，与归档实拍**逐字节一致**。

## 1. 现象（修复前）

二进制：`target/release/mw-shell.exe`，SHA256 `0f26bbb9…3dd72b`（与 X20 manifest 一致）。

```powershell
$env:MW_WINDOW_SIZE='640.0x460.0'
$env:MW_GOLDEN_INPUT='shots/x100/X20/inputs/narrow-menu.txt'
./target/release/mw-shell.exe --golden --ppp 1.5 --warmup 14 --dir <tmp> --update
```

- 连跑 3 次，截图 SHA256 均为 `26A6182B73D76D37375CE8571C535C99EE69163D751ABEA8AED09AE18104DEF3`；
- 与归档 `actual/narrow-menu-snow-640x460-p150.png`
  （`79C4E412AE15E166A201618C22FDCD8093DF99EF4E3FA15AA3C3F5B556810DBB`）
  逐像素比较：**325,013 px 不同（49.066%），最大通道差 221**，菜单根本没有打开；
- 同一重放若改成「先 `move 66 24`，隔一帧 `press`，再隔一帧 `release`」，
  立即打开菜单，且截图与归档图 **0 px 差异**。

修复前截图未单独归档（同一路径被后续成功重放覆盖），以三次一致的
SHA256 `26A6182B…04DEF3` 作为失败证据；修复后的图见 §4。

## 2. 根因

egui 的控件命中测试使用**上一帧登记的控件矩形**。`click/rightclick/middleclick`
原先把 `PointerMoved` 与 `PointerButton{pressed:true}` 放进同一帧；当指针此前不在
任何控件上（离屏脚本没有真实光标）时，这一帧的按下命不中控件，点击被丢弃。
真实鼠标总会先连续发送移动事件，所以日常使用不易察觉，但脚本回放会稳定丢点击。

X19 的 `middleclose` / `tabmenu` 脚本同帧点击曾生效，是因为它们读的是输入状态
或右击路径；标题栏菜单触发器（`egui::Popup::menu(&resp)`）依赖控件响应点击，
因此稳定受影响。

## 3. 修复

`crates/mw-shell/src/golden_input.rs`：

- `click / doubleclick / rightclick / middleclick` 先单独推进一帧 `PointerMoved`，
  下一帧才按下、再下一帧松开；
- `move` 原语共用新的 `move_step` helper；
- 单测改名并加断言：每个点击组的第一帧只能有 `PointerMoved`，下一帧必须是
  `PointerButton{pressed:true}`。

产品代码未改；外观、golden 基线、`.zmind` 格式均未动。

## 4. 修复后证据

重放命令同上（脚本文件一行未改）。trace：`caretaker-narrow-menu-replay-2026-10-25.trace`：

```text
golden_input_start steps=13
golden_input 4 move [66.0 24.0]
golden_input 5 pointer [66.0 24.0] Primary pressed=true
golden_input 6 pointer [66.0 24.0] Primary pressed=false
...
golden_input_done steps=13 applied=13
```

- 重放截图：`caretaker-narrow-menu-replay-2026-10-25.png`（960×690，ppp 1.50）；
- SHA256 `79C4E412AE15E166A201618C22FDCD8093DF99EF4E3FA15AA3C3F5B556810DBB`
  **与归档 actual 完全一致**（0 px 差异）。

X19 归档脚本回归（新原语多一帧，结果不变）：

- `tab-menu.txt` → `event menu_open anchor=62,65 entries=5`
- `middle-close.txt` → `event tab_close index=2 remaining=2 active=0`

## 5. 门槛与回归

- `cargo fmt --all -- --check`：干净
- `cargo clippy -p mw-shell --all-targets`：仅 `export_pdf.rs` 3 条历史告警，新增 0
- `cargo test --workspace -- --test-threads=1`：**747 passed / 0 failed**（测试名替换，条数不变）
- `scripts/x100-stage-b-gate.ps1`：**PASS**，23 组 golden × 2 次 `--tol 0`
  46/46 0 差异（含 `shell-narrow` / `shell-ink-compact` 两个脚本场景），
  结论行 `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`；
  日志 `caretaker-stage-b-gate-2026-10-25.log`
- release：`target/release/mw-shell.exe` 23,455,744 B，
  SHA256 `E67D79A6…03221F`（因源码改动重建，X20 manifest 里的旧哈希保持历史不动）

## 6. 未做

- 未改任何产品行为、视觉、golden 基线或证据实际图；
- 未开始 Stage C（X21+），X20 人工门仍需用户确认；
- 未提交 git。
