# X20 门后维护 · 弹窗/菜单下的标签条输入隔离（2026-10-25）

> 范围：Stage B 修复，不改变 X20 门结论，不越过人工门，不开始 Stage C。
> 触发：看护代理接手时心跳停在 06:47（>15 分钟），按 `docs/24`
> 「X20 门后维护 · 模态弹窗下的标签条输入隔离验收口径」执行。

## 1. 缺陷

标签条的两条输入路径读**裸指针/滚轮**，不经过 egui 的图层命中：

- 滚轮：`ctx.input(|i| i.pointer.interact_pos())` 判断落点后直接换算横向滚动；
- 重排：`press_origin` + `interact_pos` + 4px 阈值自己起手，不看顶层浮层。

于是前台有暗幕/菜单时，标签条仍在背后收输入：

- 打开「新建文档」对话框后滚轮仍滚动标签条 100px；
- 按下拖动仍会重排标签：`tab_reorder from=2 slot=5 active=0`，同时活动
  文档被换掉（`focus canvas`）。配合按下标存储的
  `confirm_close_tab: Option<usize>`，后续关闭确认可能落到错误标签上，
  直接丢失未保存内容；
- 文件菜单（Memory popup）不覆盖标签条，滚轮同样会穿过去。

画布本身已被 `blocks_canvas()` 正确拦截，问题只在标签条自建的两条输入
路径。

## 2. 修复

`crates/mw-shell/src/app_methods_2.rs:852` 在 `tab_bar` 入口补前台判定：

```rust
let foreground = self.overlay_state().top_modal().is_some()
    || self.dialog_demo
    || egui::Popup::is_any_open(ui.ctx());
if foreground {
    drag = None; // 取消在途拖拽
}
let interactive = !foreground
    && self.tab_menu.is_none()
    && self.context_menu.is_none();
```

`interactive` 现在同时门控滚轮换算、重排起手（`:888`、`:962`、`:979`）
与加号按钮（`:1141`）。前台出现时在途拖拽被取消，不会跨模态延续。

用 `Popup::is_any_open` 而不是「任意 Area」：它只反映 Memory 类弹窗
（标题菜单、下拉），提示气泡走 `OpenKind::Open` + hover，不注册进
`Memory::popups`，因此悬停提示盖住标签条时不会把标签条冻住。

## 3. 修复前证据

9 标签、640×460、ppp 1.5、`MW_DIALOG_DEMO=1`、warmup 16。

- 滚轮脚本 `target/x100/probe-modal-wheel/modal-wheel.txt`
  （`wait 4` / `hover 300 65` / `scroll 0 -100` / `wait 12`）：
  trace `caretaker-tabs-modal-isolation-before-wheel-2026-10-25.trace`
  第 12 行 `tab_wheel dx=-100.00`，第 14 行
  `tab_scroll x=100.0 content=1006.0 view=600.0`。
  实拍 `caretaker-tabs-modal-isolation-before-wheel-2026-10-25.png`：
  960×690 / 107,839 B / SHA256 `9AE4ADD3…A8688B`。
- 拖动脚本 `target/x100/probe-modal-drag/modal-drag.txt`
  （`wait 4` / `press 300 65` / 三次 `move` 到 520 / `release` / `wait 8`）：
  trace `caretaker-tabs-modal-isolation-before-drag-2026-10-25.trace`
  第 15 行 `tab_reorder from=2 slot=5 active=0`，第 16 行 `focus canvas`
  ——标签顺序与活动文档都被改掉。
  实拍 `caretaker-tabs-modal-isolation-before-drag-2026-10-25.png`：
  960×690 / 106,795 B / SHA256 `901B2361…70B3FBE`。
- 修复前后差异：
  - 滚轮 12,106 px = 1.828%，最大差 154，包围盒
    `x[1..899] y[72..122]`，全部落在标签条；
  - 拖动 2,767 px = 0.418%，最大差 208，包围盒
    `x[354..791] y[88..495]`，除标签条外还包含暗幕后的画布——活动文档
    确实被换掉。

## 4. 修复后证据

- 滚轮：`caretaker-tabs-modal-isolation-after-wheel-2026-10-25.trace`
  全文件 0 条 `tab_wheel` / `tab_scroll`；实拍
  `caretaker-tabs-modal-isolation-after-wheel-2026-10-25.png`
  960×690 / 108,398 B / SHA256 `01ACE0A2…C896648`。
- 拖动：`caretaker-tabs-modal-isolation-after-drag-2026-10-25.trace`
  0 条 `tab_reorder`；实拍与滚轮后图逐字节相同
  （SHA256 `01ACE0A2…C896648`），可同时证明未重排、未切换活动文档。
- 菜单：1080×720 下脚本 `target/x100/probe-menu-wheel/menu-wheel.txt`
  点开「文件」后悬停标签条并滚轮；
  `caretaker-tabs-menu-passthrough-2026-10-25.trace` 0 条标签事件，
  实拍 `caretaker-tabs-menu-passthrough-2026-10-25.png`
  1620×1080 / 221,502 B / SHA256 `87DEB4DE…C18B1A`，菜单保持打开。
- 新基线：第 30 组 golden `shots/golden/tabs-modal-input/`
  （960×690 / 108,398 B / SHA256 `01ACE0A2…C896648`），脚本
  `shots/golden/inputs/tabs-modal-input.txt` 在同一场景里先滚轮再拖动，
  21 帧、warmup 24；双跑 `--tol 0` 0 差异，
  `caretaker-tabs-modal-input-golden-2026-10-25.trace` 0 条标签事件。

## 5. 回归

- 新场景已接入 `scripts/ci.ps1` 与 `scripts/x100-stage-b-gate.ps1`；
  阶段门由 25 个目录 / 29 组比较升为 **26 个目录 / 30 组比较**。
- `scripts/x100-stage-b-gate.ps1` 的 clippy 新增告警白名单补入
  `app_methods_2.rs` / `tab_strip.rs`，本次维护动过的文件不会有漏网新告警。
- 既有 golden（`tabs` / `tabs-overflow` / `tabs-wheel` / `tabs-wheel-line`
  及其余）在阶段门中复跑 `--tol 0` 0 差异；本轮比较全部不带 `--update`，
  `shots/golden/` 下 07:00 之后只有新基线有 mtime 变化，旧基线未被改写。
- 完整门槛数字见第 6 节。

## 6. 机器门复跑

- 2026-10-25 07:15:26–07:17:07 完整阶段门：X11–X19 证据包 PASS、全部
  `x100-*.ps1` 语法 PASS、fmt 干净、mw-shell clippy 仅 `export_pdf.rs` 3 条
  历史、**761 tests / 0 failed**、30 组 × 2 = **60/60** 全 0 差异；结论行
  `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`
  （finished 2026-10-25 07:17:07 +08:00）。
- release 构建（门内 `cargo build --release -p mw-shell`）：
  `23,485,952 B` / SHA256 `38EBF196…41F27DA`。
- 日志：`caretaker-stage-b-gate-tabs-modal-2026-10-25.log`。

## 7. 未做

- 文件菜单只在修复后实拍，没有留下修复前 trace；穿菜单行为由同一裸输入
  路径推断，未做 A/B 对比。
- 只实拍了新建文档对话框这一种 `top_modal()`；其余模态共享同一谓词，
  未逐个实拍。
- 提示气泡覆盖标签条时仍允许滚动，这是有意行为，未做 golden 固定。
- 键盘焦点/激活路径未改（标题菜单触发器只有 `Sense::click`）。
- 没有真实鼠标/触控板采样，脚本是协议级注入。
