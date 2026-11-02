# X16

## Scope

统一弹出菜单引擎：右键菜单、标题栏菜单与大纲页头菜单共用同一套条目模型、行绘制与
键盘状态机；补齐子菜单、边缘翻转、禁用项、快捷键列与右击输入原语。
菜单只发意图，不新增文档写路径，所有动作仍走既有 `panels::context::Action` /
`TitleMenuAction` / `OutlineAction`。

## Changes

- **新增 `menu` 模块**：`Entry<T>` 只保留 Action / Submenu / Separator / Section
  四类；`Menu<T>` 用 `Area` 独立成卡，支持最多 4 层子菜单路径；根卡默认右下展开，
  右侧不足翻左、下侧不足翻上，最后夹进内容区 8px 安全边；子菜单贴父行右侧、
  重叠 2px，右侧不足翻左，垂直方向按父行顶对齐并在溢出时上移。
- **统一行绘制**：行高 30px、行圆角 6px、图标列 16px、标签起 32px、快捷键右对齐
  距边 12px、`SMALL` 12px；带子菜单的行快捷键列左移 18px，chevron 固定右 10px。
  禁用行文字降 `faint`、图标降 `icon_off`，不响应悬停高亮与点击；分组标题左 12px。
  标题/大纲 Popup 菜单通过 `popup_row` 复用同一行绘制与同一份几何常量。
- **键盘状态机**：`↑↓` 循环、`Home/End` 首末、`→` 展开子菜单并落到第一个可用项、
  `←` 收起最深一层、`Enter` 执行或展开、`Esc` 先收子菜单再关整张；导航跳过禁用行、
  分隔线与分组标题；其它按键收起菜单但不吞事件。标题/大纲 `PopupNav` 支持
  `↑↓`、`Home/End`、`Enter`、`Esc`，高亮用强调淡底；同时移除了行上多余的重焦点环。
- **菜单内容迁移**：单节点菜单的复制/粘贴/检查器收进「样式」子菜单；多选节点菜单的
  六条对齐 + 两条分布收进「排列」；注解菜单的五条边界色收进「边界颜色」。
  其余菜单项、顺序与快捷键不变。
- **追踪点**：`menu_open`、`menu_nav`、`menu_submenu_open`、`menu_submenu_close`、
  `menu_activate`、`menu_click`、`menu_close reason=escape|outside` 与
  `popup_menu`，仅在 `MW_TRACE` 下写事件。
- **golden 原语与场景**：`golden_input` 新增 `rightclick x y`（真实 Secondary
  按下/抬起）；新增 `MW_CONTEXT_DEMO=1`，在固定节点上打开右键菜单并展开「样式」
  子菜单，作为第 15 个 golden 场景（ppp 1.5）。`menu` 需要 `--warmup 8`：
  菜单首帧到第 6 帧仍在做入场上色，warmup 3 会截到半透明中间态；
  warmup 6 与 8 已与 warmup 60 逐像素一致。`scripts/ci.ps1` 与
  `scripts/acceptance.ps1` 均已接入该场景。

## Verification

- `cargo fmt --all -- --check` 干净；`cargo clippy -p mw-shell --all-targets`
  仍只有 `export_pdf.rs` 3 条历史告警，新增 0。
- `cargo test --workspace -- --test-threads=1`：**718 passed / 0 failed**。
  菜单模块含 11 条单测：导航跳过分隔/分组/禁用、Enter 执行并关闭、禁用行永不触发、
  `→/←` 子菜单开关、Enter 在子菜单行只展开、根卡翻转与夹边、子菜单左右翻转、
  快捷键列避让、按行类型计高。
- 行为实拍（全部在最终构建上重跑，`PP=1.5`）：
  - **右键与键盘遍历**（`inputs/menu-rightclick.txt`）：真实 Secondary 点击
    `(430,288)` 打开 19 条目菜单（`menu_open anchor=430,288 entries=19`）；
    11 次 `↓` 依次落到 新建子主题 → 新建同级主题 → 剪切 → 复制 → 删除 →
    编辑文字 → 新建卡片笔记 → 添加概要 → 添加边界 → 分离为自由主题 → 样式，
    禁用「粘贴」与全部分隔线/分组被跳过；`→` 展开样式子菜单
    （`menu_submenu_open key=right label=样式`）。
  - **关闭层级**（`menu-escape.txt`）：`Esc` 只收子菜单
    （`menu_submenu_close key=escape level=1`），再次 `Esc` 关整张
    （`menu_close reason=escape`）。
  - **外部点击**（`menu-outside.txt`）：画布空白处主键点击
    （`menu_close reason=outside`），不误触任何命令。
  - **执行命令**（`menu-enter.txt`）：`End` 高亮「聚焦」，
    `Enter` 发出 `menu_activate key=enter label=聚焦`；画布确实进入聚焦模式：
    面包屑「织思 > 纯 Rust 原生」、只显示子树、缩放 128%，菜单本身不改文档。
  - **标题菜单键盘**（`menu-title.txt`）：视图菜单 `↓` 两次，
    `popup_menu key=down highlight=Some(0)` → `Some(1)`，`count=9`；
    高亮为强调淡底，没有额外焦点环。
- 金样矩阵：15 个场景 × 2 次 `--tol 0` 全部「✓ 无超容差差异」（30/30，
  `traces/golden-verify.log`）。既有 14 个场景的 SHA256 与 X15 完全一致
  （shell@1.5 `811c6d36…679cf925`、library@1.5 `561fa6ce…1b0d1939`、
  selected@1.5 `0ef28dfb…4bcc833a`、outline `4242cc77…3fb21b`），
  X16 新增菜单场景带来 0 漂移；菜单基线 `fbfaf8ce…3342610`（`traces/hashes.txt`）。
- 构建：`target/release/mw-shell.exe` **23,236,608 B**，
  SHA256 `1f2dcaf0…8da5470ecc`。

## Known gaps

- 大纲页头三个菜单（折叠/密度/更多）已共享 `PopupNav`，但只有标题「视图」菜单
  有脚本化键盘实拍；大纲菜单由单测与代码路径覆盖，没有单独截图/trace。
- 子菜单鼠标悬停展开路径会写 `menu_submenu_open via=hover`，开发时人工确认过，
  但没有脚本断言；证据里锁的是键盘 `→` 路径。
- 菜单在深色/雾青/暖砂主题下的配色没有单独金样；菜单场景只覆盖默认纸感主题。
- 根菜单四条屏幕边的翻转/夹边由纯几何单测覆盖；运行时实拍只证明了下方不足时的上翻。
- 高菜单上翻后夹到内容区顶边，会覆盖应用内菜单栏。这是窗口级浮层的原生行为
  （与系统/Xmind 右键菜单一致），本轮不做画布矩形裁剪。

## 接手复核（2026-10-24 23:11，心跳超时接手）

前一会话在 23:05 完成金样复核后中断，心跳停在 22:33；代码、基线、manifest 与
HANDOFF/PLAN 已落盘，但缺 `traces/tests.txt` 与心跳。接手只做复核与补齐，
未改代码、未重拍基线：

- `cargo fmt --all -- --check` 干净；`cargo clippy -p mw-shell --all-targets`
  仍只有 `export_pdf.rs` 3 条历史告警（`traces/clippy.txt`）。
- `cargo test --workspace -- --test-threads=1`：718 passed / 0 failed
  （`traces/tests.txt`）。
- `scripts/ci.ps1 -SkipClippy` 连跑两遍均 `CI OK`，每遍 15/15 场景
  `--tol 0` 无差异（`traces/ci-pass1.log`、`traces/ci-pass2.log`），合计 30/30；
  所有场景按脚本隔离 `APPDATA`。release 二进制未变（`1f2dcaf0…8da5470ecc`）。
- 复跑教训：`library` 必须走 `ci.ps1` 的临时 `APPDATA` + `MW_LIBRARY_DEMO=1`；
  在默认 `APPDATA` 下会读到真实笔记库，@1.0/1.5/2.0 分别漂移
  2.480% / 4.692% / 6.358%（两轮数字完全一致），不是渲染回归。
- **未解决**：`scripts/ci.ps1` 的 `MaxClippyWarnings = 37` 已过期——
  `cargo clippy --workspace --all-targets` 当前 61 条（mw-core 38、mw-shell 3 等），
  脚本会在 golden 之前中止。本轮按 AGENTS 口径以 `-p mw-shell`（新增 0）为门槛，
  并用 `-SkipClippy` 跑完 golden；预算校准需要单独确认哪些历史告警可接受。
