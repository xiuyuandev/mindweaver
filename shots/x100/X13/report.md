# X13 · 组件库 v2

> 日期：2026-10-24 · 阶段：Stage B · 状态：已验证（Stage B 人工门在 X20）
>
> 参考：Xmind 26.5.1107 的控件密度与状态对比只作为观感依据；几何、颜色与交互全部自绘，
> 未复制任何专有素材。

## Scope

把散落在笔记库、检查器、设置、主题编辑器、大纲页头和标签条里的按钮、图标按钮、
胶囊、分段、开关、输入框、下拉框、标签页与菜单壳收进 `mw-shell::ui_kit`：
几何读 `mw_core::design::component`，状态色读 `Chrome`，调用方只传语义。

## Changes

- `mw-core::design::component` 新增组件令牌：按钮内边距/圆角、胶囊、大纲按钮高度、
  工具按钮与命中区、分段内外圆角与间隙、开关尺寸/滑钮/内缩、焦点环偏移与线宽；
  配套单测校验尺寸关系（滑钮 + 双侧内缩 = 轨道高度、分段外圆角大于内圆角等）。
- `ui_kit` v2 组件：
  - `text_button` + `ButtonTone`：主/次/平面/危险四态，禁用统一退到 `faint`；
  - `icon_button` / `close_x` / `plus`：命中区、悬停淡底、手型、矢量图标；
  - `pill`、`outline_button`、`tool_button` + `ToolButtonState`；
  - `chip` + `ChipPaint`：检查器强调色芯片，支持混合态与伪粗体；
  - `segmented`：等宽分段，选中块用面板色 + 细描边，返回点击序号；
  - `switch` / `switch_row`：36×20 轨道、16px 滑钮、`FAST` 动效、整行可点；
  - `input_frame` / `text_input`：统一输入框表面与内边距；
  - `select` + `SelectSpec`：统一下拉框，替代裸 `egui::ComboBox`；
  - `menu_frame`：菜单壳统一圆角、描边与 `elevation::POPOVER` 阴影；
  - `tab`：选中强调淡底 + 强调墨色、脏标记实心圆点、焦点环与键盘激活；
  - `activated` / `paint_focus_ring`：自绘控件的 Enter/Space 激活与键盘焦点环。
- 迁移旧实现：
  - `note_library` 删除本地 `Btn` / `flat_button` / `pill`，排序下拉改 `select`，
    搜索框放大镜由 `"⌕"` 字形换成 `Icon::Search` 矢量图；
  - 开始页 `ghost_button` → `outline_button`；
  - 检查器 `seg` / `toggle` 的绘制委托给 `chip`，两处复选框 → `switch`；
  - 大纲页头工具按钮 → `tool_button`（菜单开合由 `Popup::is_id_open` 判断），
    大纲/画布切换 → `segmented`；
  - 导出预设、主题编辑器基底、笔记排序三处 ComboBox → `select`；
  - 设置页导出/降动效复选框 → `switch_row`，笔记目录 → `text_input`；
  - 主题编辑器「分支线收细」与颜色槽开关 → `switch`；
  - 标签条从 `selectable_label` + `"● "` 字形迁到 `ui_kit::tab`，脏标记改画圆点。
- 防回退：源码扫描拒绝在 `ui_kit` 外重新出现 `egui::ComboBox`、`.checkbox(`、
  本地 `flat_button` / `ghost_button` / `pill`、`"⌕"` 与标签条
  `selectable_label(i == active`。
- 组件测试 9 项：色调映射、分段矩形无缝铺满、滑钮不出轨、按钮最小命中高、
  开关点击只切换一次、分段返回正确索引、标签脏点宽度、标签点击激活、组件单一实现扫描。

## Verification

- 单元：`cargo test -p mw-shell ui_kit::tests` → **9 passed / 0 failed**。
- 回归：`cargo test --workspace -- --test-threads=1` → **704 passed / 0 failed**
  （证据 `traces/tests.txt`）。
- 工程：`cargo fmt --all -- --check` 干净；`cargo clippy -p mw-shell --all-targets`
  → `export_pdf.rs` 3 条历史告警，新增 0（证据 `traces/clippy.txt`、`traces/fmt.txt`）。
- 视觉漂移（重拍前，容差 0）：
  - library @1.0/1.5/2.0：3128 / 7005 / 11246 px（0.179% / 0.400% / 0.643%），
    包围盒集中在搜索行与排序控件；左列、卡片列表、预览区逐像素一致。
  - 其余 10 个场景（shell 1.0/1.5/2.0、theme ink/mist/sand、outline、
    outline-focus、card-slash、outline-compact）均为 0 px。
- 仅重拍 library @1.0/1.5/2.0；随后 13 个场景 × 2 次 `--tol 0` 全部
  “✓ 无超容差差异”（证据 `traces/golden-verify.log`）。
- 基线：`shots/golden/library/shell-ppp1.50.png` SHA256 `561fa6ce…b0d1939`；
  shell `180b9b78…14752d`、outline `ebce4e2f…919c2f` 未变。
  实拍副本 `actual/shell-ppp1.50.png`、`actual/library-ppp1.50.png`、
  `actual/outline-ppp1.50.png`。

## Known gaps

- 标题栏、应用菜单、文档标题、保存状态与主工具栏留给 X14；画布悬浮条与缩放条留给 X15。
- 统一菜单引擎（子菜单、边缘翻转、快捷键列、键盘导航）留给 X16；弹窗系统留给 X17。
- 标签条只在 ≥2 个标签页时出现，当前 golden 矩阵没有多标签场景；本轮以
  点击/宽度单测覆盖，多标签视觉与保存状态在 X19 补 golden。
- 设置页与主题编辑器不在 golden 矩阵内，本轮迁移由编译、单测与人工代码检查保障。
- 提示仍使用 egui 默认 hover 延迟与样式，待 X16 统一菜单/提示引擎时收口。
