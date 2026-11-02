# 并行会话占用标记 · 2026-11-01 18:35

> 本文件是**临时协调标记**，不是进度真相源（进度只看 `docs/STATUS.md`）。
> 用途：记录「某一个时间窗内，哪些文件被另一个代理占用」，避免两个会话互相覆盖。
> 解除条件见 §4；解除后本文件可以删掉或归档。

## 1. 观测到的事实（本机，2026-11-01）

| 时间 | 事件 |
| --- | --- |
| 18:20:23 | `crates/mw-shell/src/canvas.rs` 被写入：新增 `RelationshipSelection`（注释写明 **CM-17**：关系线多选，批量删 / 改箭头 / 改线型 / 改线宽） |
| 18:20:39 | `canvas/canvas_methods_2.rs`、`canvas_methods_3.rs` 被写入；此时 `self.relationship` 从 `Option<RelationshipSel>` 改成结构体，8 处 `.take()` / `.clone()` 调用点未同步，**`mw-shell` 编译失败** |
| 18:23:45 | 另一个 `cargo build -p mw-shell --tests` 启动（不是本会话发起的） |
| 18:26:50 | `docs/x100/polish-ledger.md`、`docs/x100/log/execution-log-2026-11.md` 被写入 |
| 18:32–18:33 | `canvas_methods_3.rs`、`canvas_methods_2.rs`、`panels/inspector.rs` 继续被写入；`cargo` 进程数一度到 4 |
| 18:14 | 卡片笔记相关（`note_view.rs` / `scroll_bar.rs`）最后一次被写，属 CE-01 收尾 |

### 1.1 顺带查清的一笔旧账：2px 纵向位移

本会话用 `mm-collapse` 复核时发现它的基线（**10-31 01:10**）与当前树差 **61 716 px / 3.527%**，
最大差 225，包围盒整窗。逐行比对定位到：底部状态栏的上边线在基线里是 `y=1056–1057`、
当前树是 `y=1054–1055`——**整页纵向差 2px**，画布视口高度随之变化、相机构图跟着移。
`05:14` 那次 stage-b 门是绿的，所以位移发生在 `05:14` 之后；这段时间里
`fonts.rs`（13:08）改过字号令牌，CE-01 报告 §9 也记着「若干场景重拍前有 1–2px 整体纵向位移，
逐条排查未归因」——两件事对得上。

19:23–19:26 并行代理正在**全量重拍基线**（`shots/golden` 下 100+ 张 `shell-ppp*.png`，
含根目录 `shell-ppp1.00/1.50/2.00`、`theme-*`、`library`、`outline`）。
本会话在它拍完之前不跑 golden，避免互相踩 `actual/` 与基线文件。

### 1.2 第二轮并行（19:33 起）

19:33–19:35：`points` 之外又在写 `start-*` / `tabs-*` 的 `actual|diff`（比对跑，不是重拍）。
19:36 前后：`Scene` 新增 `groups` 字段（概念分组方向），`mw-core` 进入半改状态，
`cargo test -p mw-shell` 报 `missing field groups in initializer of Scene`（**不是本会话的改动**）。
本会话因此暂停一切编译/金样，等它再次收口；此前 19:22 拍的两张 M32 基线是在它改 `Scene` 之前，
若它后续改变默认场景（`groups` 默认空应当不影响），需在它稳定后复跑确认。

写入方不是本 Claude 会话（本会话 18:18 启动，只读过这些文件）。
WorkBuddy 的数据库与文件索引在同一时段持续更新，判断为并行代理。

## 2. 本会话主动避让的文件

以下文件在并行代理收口前**只读不写**：

- `crates/mw-shell/src/canvas.rs`
- `crates/mw-shell/src/canvas/canvas_methods_1.rs` / `canvas_methods_2.rs` / `canvas_methods_3.rs`
- `crates/mw-shell/src/canvas/consts.rs` / `canvas/tests.rs`
- `crates/mw-shell/src/panels/inspector.rs` 与 `panels/inspector/`
- `crates/mw-core/src/command.rs` / `document.rs`（CM-17 的批量命令很可能落在这一带）
- `docs/x100/polish-ledger.md`、`docs/x100/log/execution-log-2026-11.md`、`docs/STATUS.md`

## 3. 本会话在避让期做的事
- 只读核对：`mw-shell` 单元测试 658 条曾全过（18:19，改动前）；CE-01 记录的
  `chrome::tests::shell_sources_do_not_hardcode_font_sizes` 与
  `menu::tests::height_counts_padding_and_row_kinds` 实测已绿。
- 全量测试 18:20 那次失败是并行改动导致的**编译失败**，不是测试失败。
- 可独立验证的落点（不依赖 `mw-shell`）：`mw-core/src/drill.rs` 新增
  `drill_toggle_command(doc, id)`（M32 双击折叠圆点「展开到下一级」的核心规划器），
  6 条单测全过（`cargo test -p mw-core --lib drill::`，18:40）。
  壳层双击识别/接线仍待 `canvas*` 释放；进度记在 `shots/x100/M32/report.md` §7。
  并行代理随后跑过 `cargo fmt`（只动了格式，测试不受影响）。

## 4. 解除条件

1. `cargo check -p mw-shell --tests` 通过；
2. `cargo test --workspace -- --test-threads=1`（`TEMP` 重定向）全绿；
3. `shots/x100/CM-17/`（或并行代理实际写的包）出现 `report.md` + `manifest.json`，
   且 `docs/STATUS.md` 回填了对应行。

三条都满足后：删掉本文件，或把它移进 `docs/archive/`。
