# SE-03 · 起始页键盘、响应式与概念图入口（P 轨 E1 第三包）

> 设计合同：`docs/30` §3.2（断点）、§3.5（键盘与焦点）、§3.6（概念图入口）；执行合同：`docs/31` §4。
> 依赖 SE-01、SE-02（都已实现）。

## 1. 开工前审计（对着合同逐条查代码）

| 合同条目 | 现状 | 结论 |
| --- | --- | --- |
| 断点 ≥900 / 640–899 / <640（单点判定 + 单测） | `start::layout(avail)` 返回 `Layout`，`breakpoints_follow_the_three_tiers` 类单测已在 | ✅ 已在 |
| `1`–`6` 直选模板 | `home_bare_keys_owned` 分支里 `Num1..Num6` → `home_template(n-1)` | ✅ 已在 |
| `Ctrl+N` 新建思维导图、`Ctrl+O` 打开、`Esc` 返回 | 同上分支（`Action::New` / `Action::Open` / `Escape`） | ✅ 已在 |
| 窗口级 `Enter` 新建只在没有控件聚焦时生效 | `ctx.memory(|m| m.focused()).is_none()` 守卫 | ✅ 已在 |
| 出现开始页自动聚焦「新建思维导图」、从菜单进入也回该卡 | `focus_pending` / `request_new_focus` | ✅ 已在 |
| `Enter`/`Space` 等价点击 | `ui_kit::activated`（卡片、行、按钮都走它） | ✅ 已在 |
| **Tab 序：快速开始四卡 → 最近文件行 → 清除按钮 → 模板六卡 → 提示行** | `recents_section` 把「清除列表」**画在区段标题行里**，注册顺序早于最近文件行 → Tab 先到清除按钮 | ❌ **顺序反了**（SE-01 的已知缺口，仍原样） |
| **`Ctrl+Shift+N` 新建概念图** | 快捷键表里没有这个动作（`shortcuts::Action` 只有 `New`/`NewTab`…） | ❌ **未绑定** |
| 主作用卡焦点环画 **2px 内侧浅色环**（避免同色叠同色） | `quick_card` 和其他卡一样走 `paint_focus_ring`（外侧强调色环） | ❌ **缺** |
| 窄窗不横向溢出、文字不压边（640×460） | `start-scroll` / `start-tab-focus` 就在 640×460 拍；`layout` 三档单测覆盖 | ✅ 已有基线（本轮补一条断言：卡片宽度下限不超可用宽） |
| 概念图入口四项（快速开始卡 / 模板第 6 张 / 文件菜单 / 快捷键） | 前两项已有（SE-02 的 `start-concept` 锁了 hover）；文件菜单有；快捷键缺 | ❌ 快捷键同上 |

结论：SE-03 缺三件事——**Tab 序**、**`Ctrl+Shift+N`**、**主卡内侧焦点环**；其余条款已在，
证据缺「键盘轨迹」这一份（`start-tab-focus` 只锁了焦点环长什么样，没锁 Tab 走到第几个控件）。

## 2. 非目标

- 不改断点数值、不改卡片尺寸与间距（§3.2 已经在）。
- 不做检查器/卡片页的键盘（SE-05+）。
- 不改「从菜单进入开始页」的返回语义。

## 3. 验收场景

1. **Tab 序**：`MW_GOLDEN_INPUT` 脚本连按 `Tab` 8 次，轨迹里 `focus` 行的顺序必须是
   `开始页 · 新建思维导图` → 新建概念图 → 打开 → 笔记库 → 最近文件第 1 行 → … → `开始页 · 清除最近文件`
   → `开始页 · 模板 思维导图`。**清除按钮必须排在最近文件行之后**。
2. **`Ctrl+Shift+N`**：开始页按一次 → 文档变成概念图（`kind = conceptMap`），轨迹 `home new concept`；
   在文档里按 → 新开一张概念图标签页。
3. **主卡内侧环**：焦点在「新建思维导图」卡上时，强调色环画在卡**内侧 2px**（外侧不再叠加），
   像素上卡外 1px 环内不应出现强调色——用 `start-tab-focus` 重拍对照。
4. **窄窗**：沿用 640×460 的两张基线；补一条单测：任何断点下卡片宽度 ≥ 180 且不超可用宽。

## 4. 预计证据

- 轨迹：`shots/x100/SE-03/tab-order.txt`（8 次 Tab 的 focus 顺序）、`ctrl-shift-n.txt`（kind 与轨迹）
- 基线：`start-tab-focus` 重拍（内侧环）、`start-keys`（新：1–6 选模板后的画面？不做，改由轨迹证明）
- 单测：Tab 序不变式（纯函数：把「焦点顺序清单」抽成可测的列表）、`shell_shortcuts_do_not_collide` 类的绑定唯一性、
  卡片宽度下限
- 两道门绿

## 5. 风险

- Tab 序改的是**注册顺序**：把清除按钮挪到行之后，视觉位置必须还是标题行右边（用 `ui.put(rect)` 定位），
  否则就是「为了键盘顺序把按钮挪走」——那不可接受。
- `Ctrl+Shift+N` 与用户自定义键位表共存：新动作要进 `Action::ALL`/默认表，并过唯一性单测。

## 6. 结果（打磨轮 21，2026-10-29/30）

| 项 | 结果 |
| --- | --- |
| Tab 序 | `start::recents_section` 改为「标题行按尺寸占位（`Sense::hover`，不是控件）→ 行画完后 `ui_kit::text_button_at` 在原矩形 `interact`」。轨迹 `tab-order.txt`：16 次 Tab 的顺序 = 新建思维导图 → 新建概念图 → 打开文件 → 卡片笔记库 → 最近文件×3 → **清除最近文件** → 模板×6 → 绕回新建思维导图；清除按钮排在最近文件行之后 ✅ |
| `Ctrl+Shift+N` | `shortcuts::Action::NewConcept`（默认 `Ctrl+Shift+N`）；开始页 → `home new concept kind=conceptMap` + toast「已新建概念图」（`ctrl-shift-n.txt`）；文档里 → `home new concept source=shortcut`（`ctrl-shift-n-doc.txt`）。**判别顺序修正**：`NewConcept` 必须排在 `New` 之前——egui 的 `matches_logically` 只查「模式要 Shift 而没按」，按下 Ctrl+Shift+N 同样满足 Ctrl+N 的模式；修之前轨迹里记的是「已新建思维导图」（真撞过）。单测 `the_shifted_chord_is_swallowed_by_the_plain_one_unless_judged_first` 把这条规则钉住；`ctrl-n.txt` 证 Ctrl+N 仍归新建思维导图 |
| 主卡内侧环 | 焦点在「新建思维导图」卡上时环画在**卡内侧 2px**：环 = (47,111,94) 落在卡内 1–2px，卡外 4 方向全是页面底色 (244,241,235)，0 个强调色像素在外（`ring-probe.txt`）；外侧 `paint_focus_ring` 不再叠加 |
| 窄窗 | 新单测 `cards_stay_between_180_and_the_page_width`：10 个断点值上卡片 ≥180 且行宽不超页宽 |
| 顺带修（金样确定性） | 金样运行**不再读用户机器上的 recent.json**（`MW_HOME` 场景统一走空列表 + `MW_START_DEMO` 播种）：`start-empty`/`start-narrow` 原来跟着本机真实列表漂——同一份代码只因真实列表多了几条，空态基线整页对不上（SE-03 实测 173100 px 漂移，修完只剩焦点环 2300 px） |
| 视觉漂移 | 本轮唯一**有意**的外观改动 = 主卡内侧环。受影响基线 8 张（start / start-empty / start-skeleton / start-concept / start-ink / start-mist / start-sand / start-narrow）：修完 `scope_builder` 挤高度的第一版后，全部只差主卡焦点环（start 系列 2300 px = 卡矩形 x[517..734] y[196..274]；start-narrow 7018 px = 卡行 y[260..389]）；全部按 `--update` 重拍并连跑两遍 `--tol 0` |

**一句话读法**：SE-03 补的是「不碰鼠标也能把开始页走完」——Tab 序归位、Ctrl+Shift+N 直达概念图、
主卡焦点环画在卡内不糊边；顺带把两张基线从「跟着本机 recent.json 漂」改成确定性的。
