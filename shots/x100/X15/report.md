# X15

## Scope

画布上下文工具条、右下缩放条与底部状态区重做；画布浮层统一屏幕空间几何与投影。
不新增文档写路径，所有动作继续走既有 `Canvas` / `Shell` 方法。

## Changes

- **新增 `canvas_chrome`**：工具条与缩放条的纯几何函数、统一浮层投影
  `float_shadow()`（`design::elevation::PANEL`）与 `FLOATING_H = 32`、`TOOLBAR_H = 36`
  两个组件令牌；几何单测钉住居中、面包屑避让、窄画布不画、四块命中区互不重叠。
- **上下文工具条**：有选区且非编辑/阅读/浮层时，画布顶部居中浮出 276×36 玻璃条；
  三组动作 `撤销/重做 | 加子/同级/删除 | 卡片/更多`，28×28 按钮、32 命中、
  禁用态由 `canvas.can_undo()/can_redo()/can_delete()/selected_count()` 每帧读取；
  「更多」以按钮下缘为落点弹节点菜单（`Canvas::request_node_context_at`）。
  工具条矩形在 `canvas.show` 之前登记为 overlay，点击不穿透成框选。
- **悬停工具条**：保留现有 ＋/⋯ 几何与悬停保持区，只把投影换成与浮层同一档
  `float_shadow()`；不新增第二套动作。
- **缩放条**：右下角 154×32 玻璃条，− / 百分比（点击复位 100%）/ + / 分隔线 / 适应；
  四块命中区 24px 高互不重叠，百分比 12px，悬停淡底、手型与提示沿用
  Ctrl+- / Ctrl++ / Home；画布小于 320×220 时不画。
- **状态区**：底部 24px。左起保存状态点 + 文案（四态，与标题栏芯片同源
  `SaveState`）、主题数、选中摘要（单选 `已选：名称` 截断 18 字符，多选计数）；
  右侧当前主题名（强调点 + 名称），键盘交给面板时显示 `键盘：面板 · F6 收回`；
  缩放数字从状态栏移除。大纲视图仍只显示 `大纲视图 · F8 返回画布`。
- **金样确定性二次修复**：离屏无脚本时先注入画外 `PointerMoved(-10000,-10000)`
  再 `PointerGone`。`PointerGone` 只清 `latest_pos`，当帧真实桌面指针仍会写进
  `interact_pos`，`Response::hovered()` 继续按桌面鼠标算——此前 library 基线里
  的返回按钮/卡片 hover 就是这么来的。修复后 library@1.0 与 @2.0 直接回到
  X14 哈希，只有旧基线本身带 hover 的 library@1.5 需要重拍。
- **新钩子**：`MW_SELECT_DEMO=1` 选中「纯 Rust 原生」并 `request_fit()`，
  作为第 14 个 golden 场景；harness 有 `MW_GOLDEN_INPUT` 时允许画布悬停态，
  无脚本时仍关闭。

## Verification

- `cargo fmt --all -- --check` 干净；`cargo clippy -p mw-shell --all-targets`
  仍只有 `export_pdf.rs` 3 条历史告警，新增 0。
- `cargo test --workspace -- --test-threads=1`：**711 passed / 0 failed**
  （新增 4 条浮层几何单测 + 1 条选中摘要单测）。
- 漂移测量（`traces/golden-measure.log`）：
  - shell @1.0 / @1.5 / @2.0：3748 px（0.214%）/ 8131 px（0.465%）/ 14250 px（0.814%），
    最大差 208，包围盒只在底部状态区与右下缩放条（y 929..1074）。
  - theme-ink / mist / sand：4533 / 7506 / 7491 px（0.259% / 0.429% / 0.428%），
    同样只落在状态区与缩放条。
  - outline、outline-focus、card-slash、outline-compact：0 px。
  - library@1.0 / @2.0：修复前 0 px / 102847 px（真实指针 hover 污染）；
    修复后 0 px / 0 px，@2.0 回到 X14 哈希 `2a3f134b…f67a0a53`。
    library@1.5：3728 px（0.213%），旧基线返回按钮带 hover，移除后重拍。
  - selected：新场景，无旧基线。
- 重拍 14 个场景（`traces/golden-rebaseline.log`），随后 **14 场景 × 2 次**
  `--tol 0` 全部「✓ 无超容差差异」（`traces/golden-verify.log`，28/28）。
- 悬停工具条实拍：`MW_GOLDEN_INPUT` 脚本 `wait 6 / hover 427 289 900 / wait 6`
  （`traces/hover-trace.log`），两次运行 PNG 哈希一致
  `e1edb70d…73f8c44f4`（`actual/hover-ppp1.50.png`）。
- 工具条交互实拍：
  - 「更多」：`click 651 78`，trace `canvas-tool-more`，节点菜单落在按钮下缘
    （`actual/toolbar-more-ppp1.50.png`）。
  - 「添加子主题」：`click 517 78`，trace `canvas-tool-child`，新增节点进入行内编辑，
    标题栏变「未保存」、状态区变 `未保存 | 14 个主题 | 已选：未命名主题`
    （`actual/toolbar-add-child-ppp1.50.png`）——动作确实走了既有 Command 路径。
- 关键基线 SHA256（`traces/hashes.txt`）：
  - shell@1.5 `811c6d36…679cf925`；shell@1.0 `4ba64bde…ac433c4`；
    shell@2.0 `6fe22428…7cce31a0`。
  - selected@1.5 `0ef28dfb…4bcc833a`；library@1.5 `561fa6ce…1b0d1939`；
    theme-ink `d0b03c81…910e0fdd`；outline 未变 `4242cc77…3fb21b`。
- 构建：`target/release/mw-shell.exe` **23,219,712 B**，
  SHA256 `22071fa2…00c5e29f9`。

## Known gaps

- 悬停工具条只在 `MW_GOLDEN_INPUT` 脚本下可复现；真实桌面悬停没有实拍证据，
  由几何单测与脚本金样共同覆盖。
- 工具条只有 More / AddChild 两条交互实拍；Undo/Redo/Delete/Card 走同一分发臂，
  由单测与命令路径保证，未逐条截图。
- 窄画布不画工具条只有几何单测（<276+24px），没有 640×460 全窗实拍；
  与 X14 的窄窗标题栏缺口合并，留给 X20 外壳视觉门一起补。
- 统一菜单引擎（子菜单、边缘翻转、快捷键列、键盘导航）仍留给 X16；
  本轮「更多」只验证落点与既有节点菜单。
- 缩放条百分比在 golden 里是相机真实值（selected 为 85%），没有逐档缩放实拍；
  点击复位/适应由既有 `Canvas` 方法与几何单测覆盖。
