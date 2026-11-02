# X12 · 矢量图标系统

> 日期：2026-10-24 · 阶段：Stage B · 状态：已验证（Stage B 人工门在 X20）
>
> 参考：Xmind 26.5.1107 的图标密度与线宽只作为观感依据；几何全部自绘，未复制任何专有素材。

## Scope

把菜单、工具条、页头、检查器、卡片页和画布悬停条里的图标收成一套：
同一 16 单位网格、同一屏幕空间线宽、同一命中区与悬停色，清掉 `▾ ✕ ↺ ⋯ ＋ ‹ ›` 字形按钮。

## Changes

- `mw-core::design::icon` 新增令牌：盒 16 / 小号 14 / 网格 16 / 主描边 1.5 /
  次描边 1.1 / 窗口控制 1.2 / 圆点 1.5 / 命中区 24。
- 新增 `mw-shell::icon_paint`：`IconPrim` 纯几何（线、矩形、圆、折线、多边形）+
  `paint_icon_sized`。`size` 只缩放几何，描边保持屏幕空间恒定，125%–200% 不糊不跳。
- `Icon` 从 20 个扩到 30 个：新增 `ChevronLeft/Right/Down`、`Close`、`More`、`Plus`、
  `Back`、`Levels`、`Density`、`Focus`；所有图标共用一套描边与轨迹语义名。
- 菜单与工具条不再各写一份 match：`paint_menu_icon` / `paint_tool_icon` 统一委托。
- 字形控件替换：
  - 检查器字号下拉 `▾` → 文字 + 矢量 chevron 原子（`MenuButton`）；
  - 主题编辑器删除预设 `✕`、检查器四处 `↺` → `ui_kit::icon_button`；
  - 演示翻页 `‹ ›` → `ChevronLeft/Right`；
  - 画布悬停 `＋ / ⋯` → `Plus / More`；
  - 大纲页头 7 个手绘符号（返回/搜索/层级/密度/聚焦/画布/更多）→ 统一图标。
- `ui_kit::icon_button` 统一小图标按钮：命中区、悬停淡底、手型、正文/次要色切换，
  图标占按钮 62%；`close_x` / `plus` 改为它的薄封装。
- 窗口控制（最小化/最大化/还原/关闭）改用 `STROKE_CHROME` 1.2，保留 Windows 原生细线。
- 防回退测试：扫描 `mw-shell/src`，拒绝 `Button::new("✕")`、`Button::new("↺")`、`"▾"` 回流；
  另有 16 单位边界、撤销/重做方向、chevron 镜像、导航图标完整性 4 项几何测试。

## Verification

- 单元：`cargo test -p mw-shell icon_paint::` 5/5；`cargo test -p mw-core --lib design::tests` 11/11。
- 回归：`cargo test --workspace -- --test-threads=1` → **694 passed / 0 failed**（证据 `traces/tests.txt`）。
- 工程：`cargo fmt --all -- --check` 干净；`cargo clippy -p mw-shell --all-targets`
  → `export_pdf.rs` 3 条历史告警，新增 0（证据 `traces/clippy.txt`）。
- 视觉漂移（重拍前，容差 0）：
  - shell @1.0/1.5/2.0：379 / 595 / 818 px（0.022% / 0.034% / 0.047%）；
  - library @1.5：60 px（0.003%）；outline @1.5：1247 px（0.071%）；
  - outline-focus @1.5：1246 px（0.071%）；card-slash @1.5：266 px（0.015%）。
  - 包围盒全部落在工具条、页头图标与关闭按钮区域，无布局位移。
- 重拍并双跑 0 差异（13 个场景 × 2 次，证据 `traces/golden-verify.log`）：
  shell 1.0/1.5/2.0、library 1.0/1.5/2.0、theme ink/mist/sand @1.5、
  outline @1.5、outline-focus @1.5、card-slash @1.5、outline-compact @1.5。
- 基线：`shots/golden/shell-ppp1.50.png` SHA256 `180b9b78…14752d`；
  实拍副本 `actual/shell-ppp1.50.png`、`actual/outline-ppp1.50.png`。

## Known gaps

- 窗口最小化/最大化/还原仍是独立几何（遵循 Windows 1.2px 约定），只统一了线宽令牌。
- 大纲折叠三角、画布折叠圆点、跨度/连线手柄是图形而非图标，未并入 `Icon`。
- `shots/golden/o03`、`o04`、`outline-edit` 是历史交互基线，不在 CI 内；本轮未重拍，
  若未来复跑需按对应 `MW_GOLDEN_INPUT` 脚本整体刷新。
- 图标没有多状态色板（正常/悬停/禁用由调用方传色）；状态色统一在 X13/X14 组件层收口。
