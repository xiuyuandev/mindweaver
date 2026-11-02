# X11 · 设计令牌 v7

> 日期：2026-10-24 · 阶段：Stage B · 状态：已验证（Stage B 人工门在 X20）
>
> 参考：Xmind 26.5.1107 的密度与层级只作为数值依据；未复制任何专有素材。

## Scope

把字号、行高、间距、圆角、描边、阴影、控件几何与动效收敛到 `mw-core::design` 单一来源，
清掉壳层里第二套字阶和裸字号，并给后续 X12–X20 提供可断言的令牌基线。

## Changes

- `mw-core::design` 新增：
  - `ui` 壳层字阶（11/12/13/14/16/25），与画布 `CANVAS_*` 明确分离、共享行高规则；
  - `control` 控件几何（命中 24、图标按钮 22、行高 28、输入 30、按钮内距 10×5）；
  - 圆角七档（4/6/8/10/12/14/18），阴影新增 `DIALOG`；
  - 动效新增 `ease_out_cubic`、减少动画时的 `duration`、场景位移时长带 0.20–0.34s。
- `mw-shell::chrome::Type` 改为 `design::ui` 的别名，删除第二套数字。
- `chrome::install_visuals`、`ui_kit::modal`、画布悬停/选中常量、场景动画全部改读令牌。
- 修正一处真实不一致：渲染层节点圆角 10、壳层选中环按 8 计算；现统一为 `radius::MD`（10）。
  同一令牌也被 SVG/PDF 导出使用。
- 新增防回退测试：扫描 `mw-shell/src`，禁止 `FontId::proportional(<数字>)` 或
  `FontId::monospace(<数字>)` 再次出现。

## Verification

- 行为/单元：`cargo test -p mw-core design::tests` 10/10；`cargo test -p mw-shell chrome::tests` 6/6。
- 回归：`cargo test --workspace -- --test-threads=1`（数字见 manifest 与 HANDOFF 快照）。
- 工程：`cargo fmt --all -- --check`；`cargo clippy -p mw-shell --all-targets` 新增 0。
- 视觉：本轮只统一取值与来源，预期除选中环圆角 8→10 外无外观变化；golden 双跑结果见
  `manifest.json`。若出现漂移，按协议重拍并记录字节差。

## Known gaps

- 仍有约 40 处装饰性小圆角（1–4px 进度条、色点）保持裸数字；它们不是组件级圆角，
  等 X13 组件库统一。
- 菜单、开始页、卡片库的字号已走 `Type`，但间距仍是逐处 4/8/12 混写；X13/X18 收口。
- `chrome_of` 的浅色灰阶尚未改由主题 `ui` 令牌派生；这是 X14 外壳重做的一部分。
