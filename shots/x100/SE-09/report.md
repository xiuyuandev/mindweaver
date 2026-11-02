# SE-09 · 检查器键盘与无障碍（P 轨 E2 第五包）

> 设计合同：`docs/30` §4.3（命令失败：面板内 28px 错误条 + toast，不静默吞掉）、
> §4.4（`Ctrl+Shift+I` 开合；面板内 Tab 依次进入分组控件；↑↓ 在色板内移动、**Enter 选择**；
> **数字键 1–9 直选**；Esc **先退出当前控件、再退出面板**；每个可改属性都有键盘路径）、
> §4.5（禁止项）。执行合同：`docs/31` §5 第 SE-09 行。依赖 SE-06 / SE-07。

## 1. 开工前审计（对着 `docs/30` §4.3 / §4.4 逐条查代码）

| 合同条目 | 现状 | 结论 |
| --- | --- | --- |
| `Ctrl+Shift+I` 开合检查器 | `Action::Inspector` 的出厂和弦是 **F9**（`shortcuts.rs` 的 `DEFAULTS` 表），`Ctrl+Shift+I` 没有任何绑定；面板关闭按钮的提示也写「关闭（F9）」 | ❌ 缺合同键 |
| Tab 进分组、Space 应用 | 焦点链已铺到分段 / 色板 / 分组标题，两条测试一路 Tab 到色板与「主题默认」并按空格；正文框、标签框有自己的焦点键 | ✅（Enter 那半见下） |
| 色板 ↑↓ 移动 | 打磨轮 24（SE-06）补过 ↑↓ 跨行 | ✅ |
| 色板 **Enter 选择** | 现在只有 Space 走 `resp.clicked()` 出口；Enter 没有接 | ❌ 缺一半 |
| **数字键 1–9 直选** | 没有 | ❌ |
| Esc **先退出当前控件、再退出面板** | 现在只有一步：自定义控件聚焦时按 Esc 直接 `Action::FocusCanvas`（一步退回画布） | ❌ 层级不对 |
| 命令失败：面板内 **28px 错误条** + toast | 只有 `main.rs` 里的 toast「修改未生效」，面板里没有任何错误条 | ❌ |
| 对比度 ≥ 4.5:1 | 没有针对检查器文字令牌的测量（`mw-core/tests/contrast.rs` 只覆盖画布主题层级） | ❌ 未测 |

**结论**：SE-09 的实质工作是四件——把出厂键改成 `Ctrl+Shift+I`（F9 降为「备用」动作，不删肌肉记忆）；
给色板补 Enter 与数字键 1–9；把 Esc 改成分两级退出；加面板内 28px 错误条；再补一条四主题的对比度测量。

## 2. 非目标

- 不做屏幕阅读器 / UIA 无障碍树（egui 本身不提供，属另一个量级）。
- 不改快捷键配置文件格式；只在 `DEFAULTS` 表里换一条、加一条。
- 不重排 Tab 序（SE-05/06/07 已定），只补缺的键位。
- 不做手柄/触控导航。

## 3. 验收场景

1. **`inspector-error`**（1280×720 @1.5）：`MW_INSPECTOR=1 MW_SELECT_DEMO=1 MW_INSPECTOR_FAIL=1`——
   走一次**真的失败路径**（发一条指向不存在节点的 `SetStyle`），面板里出现 28px 错误条、
   同时仍走 toast；基线锁住错误条的位置与高度。轨迹 `inspector_error shown=1 text=…`。
2. **数字键 / Enter**：单测——Tab 走到色板后按 `3` 直选第 3 格、按 Enter 应用，
   出来的必须是一条 `SetStyle`（`digits_pick_a_swatch_while_the_palette_holds_focus`）。
   输入脚本版（`inspector-keys`）不拍：Tab 步数随面板改动漂，拍出来锁不住，改用单测锁命令。
3. **Esc 两级**：判据抽成纯函数（`esc_step`）单测——控件握着焦点 → `Release`；
   刚松过（latch）→ `Exit`（回画布）；都没焦点 → `Ignore`（不抢画布的 Esc）。
   无头 harness 里 egui 的焦点在两次 `run_ui` 之间不保留，拍不出「跨帧两级」，接线靠实机核对。
4. **对比度**：单测——四套主题下检查器写字的两级（`text`/`dim`）对面板底色 ≥ 4.5:1。

## 4. 预计证据

- 新基线 `inspector-error` + `inspector_error` 轨迹；键盘路径用单测锁命令（见 §3 的取舍）。
- 单测：Esc 两级判据、数字键直选、四主题对比度（外加本轮把检查器里的文字从 `faint` 换成 `dim`，
  否则浅主题 2.37:1）。
- 两道门绿；测试数只增不减。

## 5. 数据模型核对（`docs/31` §5 P0 风险行）

| 能力 | 承载 | 结论 |
| --- | --- | --- |
| 快捷键 | `shortcuts::DEFAULTS` 表 + 用户配置（`Action::id()` 稳定键） | 有（加一条 `InspectorAlt`） |
| 错误条 | 面板内的临时状态（`Inspector.error: Option<(String, f64)>`），不落盘 | 有 |
| 数字键直选 | 只是把键位映射到已有的 `SwatchPick` | 有 |

不需要新字段，P0 风险不触发。

## 6. 结果（实测）

### 6.1 交付项

| 合同条目 | 做完的样子 | 证据 |
| --- | --- | --- |
| §4.4 `Ctrl+Shift+I` 开合 | `Action::Inspector` 的出厂键改为 `Ctrl+Shift+I`，老的 `F9` 降为新增的 `Action::InspectorAlt`（「检查器（备用）」），两者都调 `toggle_inspector`；快捷键面板、速查、命令面板提示不变（F9 仍在） | `shortcuts.rs` 的 `hints_follow_the_configured_chord` 补一条双键断言 |
| §4.4 Tab 进分组、Space 应用 | 既有（SE-05/06 已铺，`tab_walks_onto_the_accent_chips_and_space_applies_one` 等在守） | 既有测试 |
| §4.4 色板 ↑↓ 移动 | 既有（打磨轮 24） | 既有 |
| §4.4 **Enter 选择** | 聚焦的色板上按 Enter = 激活当前格（与 Space 同一出口） | `digits_pick_a_swatch_while_the_palette_holds_focus`（同时覆盖数字键） |
| §4.4 **数字键 1–9 直选** | 只在色板自己握焦点时吃键（面板别处敲数字不改颜色）；`1–9` 映射到色板前 9 格 | 同上 |
| §4.4 **Esc 两级** | `esc_step(holding, latch)`：控件 → `Release`（只松焦点）；刚松过 → `Exit`（回画布）；都没焦点 → `Ignore`（不吃画布的 Esc）。面板里再按 Tab 回到控件时计数重置 | `escape_goes_through_the_control_before_the_panel` |
| §4.3 **28px 错误条** | `Inspector.error` + `error_bar()`：28 高、warn 8% 底、警示点、文字 `12.5`、右侧 14px 关闭；壳层在 `apply` 失败时 `report_error`（toast 照旧）、成功时 `clear_error`；不按墙钟过期，点掉/改成功即消 | `inspector-error` 基线 + `inspector_error shown=1` 轨迹 |
| §4.4 对比度 ≥ 4.5:1 | `every_theme_keeps_inspector_text_readable`：四主题 `text`/`dim` 对 `panel` 全部 ≥ 4.5 | 见 §6.3 的发现与处理 |

### 6.2 途中发现并处理的一处真问题（对比度）

浅主题下检查器的弱文字令牌 `faint` 对面板底色只有 **2.37:1**——摘要、提示语、空态文案全在这条线上，
远低于 AA。逐格换色（动主题令牌会掀翻全应用的基线）与把字加深（把层级拍平）之间，本轮取** contained **的改法：
检查器里的**文字**一律改用 `dim`（对 `panel` ≥ 4.5），`faint` 只留给定线、色点、开关轨道这些非文字；
主题令牌本身不动。同时量出 `dim` 在 `panel2`（卡片候选列表的底）上是 **4.14:1**，差 AA 一点——
测试里单独立了一条 ≥ 4.0 的下限，缺口记在 §7，不在本轮动令牌。

### 6.3 测试

新增 3 条（`panels::inspector` 现 959 条全绿）：

- `digits_pick_a_swatch_while_the_palette_holds_focus` — Tab 到色板后 `3` 直选、出 `SetStyle`。
- `escape_goes_through_the_control_before_the_panel` — Esc 两级的四格判据。
- `every_theme_keeps_inspector_text_readable` — 四主题 `text`/`dim` ≥ 4.5（panel）与 ≥ 4.0（panel2 下限）。
- （`shortcuts.rs` 的既有测试补了 `InspectorAlt` 一行断言。）

### 6.4 机器判据

{{GATE}}

### 6.5 漂移

{{DRIFT}}

## 7. 未做与风险

| 项 | 状态 | 说明 |
| --- | --- | --- |
| `dim`@panel2 = 4.14:1 | 记账 | 卡片候选列表的摘要在下沉一层的底上，差 AA 0.36。要修就得动主题令牌（掀全应用基线）或把那几行换成 `text`；两者都超出本包，先立 4.0 下限盯着 |
| `inspector-keys` 输入脚本场景 | 改单测 | Tab 步数随面板改动漂，输入脚本锁不住；改用单测直接锁「数字键/Enter 出命令」（§3 的取舍） |
| Esc 两级的跨帧演示 | 无头拍不出 | egui 焦点在两次 `run_ui` 之间不保留；接线靠实机核对，不记 `已验证` |
| 屏幕阅读器 / UIA | 非目标 | egui 不提供，量级完全不同 |
