# X14 · 标题栏 / 应用菜单 / 文档标题 / 保存状态 / 主工具栏

> 日期：2026-10-24 · 阶段：Stage B · 状态：已验证（Stage B 人工门在 X20）
>
> Xmind 26.5.1107 只用于观感参照；几何、颜色、图标与菜单全部自绘，未复制任何专有素材。

## Scope

把顶部 56px 旧栏替换为 48px 标题栏：品牌、五个应用菜单、撤销/编辑工具簇、文档标题、
保存状态芯片与 46×48 窗口按钮同排；菜单行、状态芯片与工具按钮统一吃
`mw_core::design::component` 令牌。保存状态从「已保存 / 未保存」两态细分为
本地优先 / 未保存到文件 / 已保存 / 未保存四态，从未落盘的新文档不再被误标成“已保存”。

## Changes

- 组件令牌：`TITLEBAR_H=48`、`WINDOW_CONTROL_W=46`、`MENU_TRIGGER_H=30`、
  `MENU_ROW_H=30`、`SAVE_CHIP_H=22`、`TOOL_GAP=4`、`TOOL_GROUP_SEP=24`，
  并补尺寸关系单测（标题栏高于菜单触发、窗口按钮不窄于 40、状态芯片低于菜单行）。
- 标题栏：
  - 品牌区 26px 徽标 + 产品名；窗口窄于 900px 时五个菜单折成「更多」，
    窄于 720px 时只留徽标；工具簇保持 28×28 视觉 / 32×32 热区。
  - 文档标题右对齐，`Type::BODY`；保存状态芯片 22px 高，圆点 + 文案；
    芯片优先于标题，剩余空间不足 120px 先丢标题，芯片也放不下才整组留白。
  - `more` 菜单展开时触发块用 accent 淡底，菜单之间、菜单与工具、工具组之间
    用 1px hairline 断行；窗口按钮 46×48，按下深一档。
- 保存状态：`SaveState::{Home, Untitled, Saved, Dirty}` 由
  `home / has_canvas / dirty / has_path` 推导；未落盘 = 「未保存到文件」，
  已落盘且无改动 = 「已保存」，有改动 = 「未保存」；仅前两者可点保存。
  新增 `save_state_distinguishes_untitled_saved_and_dirty` 单测。
- 应用菜单：行高 30px、圆角 6px、`Type::BODY` 文案、图标 16px、勾选与快捷键
  右对齐；分隔线统一 `stroke::HAIRLINE`；菜单壳复用 X13 `ui_kit::menu_frame`。
- 主工具栏：`ui_kit::tool_button` 的启用色从 `ch.dim` 提到 `ch.text`，
  禁用仍为 `ch.icon_off`；撤销/重做与编辑命令、适应画布之间各留分组线。
- 金样 harness：离屏且没有 `MW_GOLDEN_INPUT` 时，每帧注入 `Event::PointerGone`。
  隐藏窗口此前可能继承桌面光标位置，同一份构建会截出“带 hover”的基线；
  本次因此暴露出 `library@1.5` 返回按钮 hover、`library@2.0` 选中框 hover
  两张旧基线，已一并重拍。`--shown` 与脚本回放不受影响。
- 测试去抖：`note_store::tests::search_thousand_cards_is_bounded` 在整包串行跑时
  偶发 132ms（单跑稳定通过）；改为预热后取三次最好值，保留 100ms 上限，
  仍能拦住 O(n²) 退化。

## Verification

- 单元 / 回归：`cargo test --workspace -- --test-threads=1` → **706 passed / 0 failed**
  （证据 `traces/tests.txt`）。
- 工程：`cargo fmt --all -- --check` 干净；`cargo clippy -p mw-shell --all-targets`
  → `export_pdf.rs` 3 条历史告警，新增 0（证据 `traces/fmt.txt`、`traces/clippy.txt`）。
- 视觉漂移（重拍前，容差 0）：
  - shell @1.0 / 1.5 / 2.0：58178 / 114024 / 160797 px
    （3.325% / 6.517% / 9.191%）；标题栏 48px 与工具簇重排使整条顶栏及画布起点位移。
  - theme ink / mist / sand @1.5：143254 / 112861 / 113062 px
    （8.188% / 6.451% / 6.462%）；同因，主题色本身无回归。
  - outline / outline-focus / outline-compact @1.5：
    119978 / 105833 / 118689 px（6.857% / 6.049% / 6.784%）。
  - library @1.5 / @2.0：3728 px（0.213%）/ 264 px（0.015%），
    包围盒分别落在返回按钮、首张卡片选择框，是旧基线里的鼠标 hover；
    由离屏指针清空修复暴露，非本轮 UI 改动。
  - library @1.0、card-slash @1.5：0 px。
- 重拍全部 13 个场景（11 张实际变化，library@1.0 与 card-slash@1.5 输出不变）；
  随后 13 个场景 × 2 次 `--tol 0` 全部“✓ 无超容差差异”
  （证据 `traces/golden-measure.log`、`golden-rebaseline.log`、`golden-verify.log`）。
- 菜单开合：`MW_GOLDEN_INPUT` 脚本 `wait 6 / click 120 27 / wait 6`
  打开「文件」菜单并留证 `actual/titlebar-file-menu-ppp1.50.png`
  （脚本 `traces/titlebar-file-menu.txt`）。
- 关键基线 SHA256：
  - shell@1.5 `7828931b…f4e6793c1`；shell@1.0 `c155a79e…5e91e`；
    shell@2.0 `8a5cceac…68aa445`。
  - library@1.5 `3844221c…42177ea`；theme-ink `4024a5f3…38dcc53`；
    outline `4242cc77…3fb21b`。全集见 `traces/hashes.txt`。
- 构建：`target/release/mw-shell.exe` **23,191,040 B**，
  SHA256 `e211b944…615f4312`。

## Known gaps

- 统一菜单引擎（子菜单、屏幕边缘翻转、快捷键列对齐、键盘导航）留给 X16；
  当前菜单开合只由脚本截图与点击单测守护，未进 golden 基线。
- 弹窗系统留给 X17；标题栏菜单里的「关于 / 文档信息」等仍用现有弹层。
- 窄窗只在组件层验证了标题/芯片取舍（`title_cluster_drops_title_before_save_chip`），
  golden 矩阵没有 640×460 标题栏基线；下一轮可在验收脚本补窄窗实拍。
- 工具簇没有溢出菜单：窗口再窄时仍固定显示 7 个工具，小于最小窗宽 640 的情况不存在。
- 离屏清空指针后，hover 态不再进入 golden；hover 仍靠 `MW_GOLDEN_INPUT` 的
  `hover/click` 脚本与组件单测覆盖。
