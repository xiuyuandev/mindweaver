# SE-06b · 检查器数值格：可键入 + 方向键步进（打磨轮 29）

> 设计合同：`docs/30` §4.2「数值：stepper 24 高、宽 72 右对齐：`- 值 +`；字号 12–96、边宽 0–8」
> 与 §4.4「**每个可改属性都有键盘路径**」。执行合同：`docs/31` §5 第 SE-06 行（样式组）。
> 本包是 SE-06 的续一轮（与 `M12b`…`M12e` 同一命名习惯）——不是 `docs/31` 里的新包。

## 1. 开工前审计

| 现状 | 结论 |
| --- | --- |
| `stepper()`（`inspector.rs`，`- 值 +` 三格自绘） | 只有两个按钮：`−`/`+` 各走一步（字号步 2、边宽步 1） |
| 中间的数值 | **只是一段被 `painter.text` 画上去的字**，不是控件、不可聚焦、点它什么都不会发生 |
| 键盘路径 | `−`/`+` 本身能被 `Tab` 走到、空格能按（§4.4 的底线成立）；但**想从 14 改到 48 要按 17 次** |
| 方向键 | 没有。色板有 ←→↑↓，数值格没有 |
| 直接键入 | 没有。`docs/30` §4.2 只写了 stepper，§4.4 却要求「每个可改属性都有键盘路径」——对数值来说，**敲数字就是那条路径** |

**结论**：这是一个真缺口——数值属性有「可点」但没有「可敲」。Xmind 里字号框点进去就能直接输入，
本项目要连点十几下。本包只做这一件事：把数值格做成「点一下变输入框、敲数字直接进编辑、
`↑`/`↓` 单步、`Enter` 提交、`Esc` 取消」。

## 2. 非目标

- 不改 stepper 的几何（24 高、72 宽、`−`/`+` 20 宽）与步长（字号 2、边宽 1）。
- 不碰「圆角」——它在模型里没有字段（`docs/30` §4.2 的第四项，SE-06 已记 `延后`）。
- 不做拖拽调值（`drag to scrub`）：离屏 harness 拍不到、手感也没法验，留作以后。
- 不改色板、分段、开关、备注/标签/链接任何一处。
- 不改任何数据模型。

## 3. 验收场景

1. **`inspector-stepper-edit`**（新基线）：内容页/样式页里字号数值格进入编辑态——格内是一段
   带底线的数字、光标在末尾，其余行不动。
2. **`inspector` 等十张既有基线零漂移**：非编辑态下数值格画的东西一个字都不变（本包的关键约束）。
3. **键入提交**：焦点在数值格上敲 `4` `8` `Enter` → 发出一条 `SetStyle { font_size: Some(48) }`；
   `Esc` → 撤销草稿、不发命令、数字回到原值。
4. **方向键单步**：`↑`/`↓` 在数值格上等价于 `+`/`−` 一次（字号 ±2、边宽 ±1，端点夹取）。
5. **越界与脏输入**：键入 `5` `Enter`（字号下限 12）→ 夹到 12；键入 `abc` / 空 → 不发命令、退回原值。
6. **一次编辑一条撤销**：连续键入只发一条命令（不按墙钟合并，编辑期本身就是一个手势）。

## 4. 预计证据

- 新基线 1 张（编辑态）；既有十张零漂移。
- 单测：`parse_stepper_input()`（脏输入、越界、小数）、方向键 → 与按钮同值、编辑态开/关的状态机。
- 轨迹：`inspector_stepper row=… edit=1 value=…`（进/出编辑各一条）。
- 两道门绿；测试数只增不减。

## 5. 结果（实测）

### 5.1 交付项

| 验收场景 | 做完的样子 | 证据 |
| --- | --- | --- |
| 数值格**可点、可敲** | 中间那格成了可聚焦控件（`k_val = FocusKey::indexed(base, 2)`）：点一下变输入框（空草稿 + 当前值做 `hint`），焦点在格上时敲数字 0–9 当场开编辑、草稿就是那个数字 | `typing_a_digit_opens_the_field_with_that_digit` |
| 方向键单步 | `↑`/`↓` 在数值格上等价于点一次 `+`/`−`（字号 ±2、边宽 ±1，端点夹取 `stepped`） | `arrow_keys_step_a_number_like_the_buttons_do`（↑ → 18、↓ → 14，16 起步） |
| `Enter` 提交 / `Esc` 取消 / 点别处提交 | `Enter` 发值；`Esc` 撤草稿不发命令；点到别处（丢焦点）按提交算，与画布行内编辑同一套手感 | `enter_commits_the_typed_value_and_escape_throws_it_away` |
| 越界与脏输入 | `parse_stepper_input()`：空 / 非数字 / `NaN` 一律不发命令退回原值；越界夹进区间 | `typed_numbers_are_clamped_and_garbage_is_ignored`（"5"→12、"300"→96、"abc"/"" → None） |
| 非编辑态**零漂移** | 不编辑时画的还是原来那一段字；新增的命中区/焦点环只在「握焦点」时才画（金样无焦点） | `inspector-stepper-edit` 与 `inspector-shape` 的差**只有编辑格**（见 §5.3） |
| 一次编辑一条撤销 | 编辑期本身是一个手势：提交发一条 `SetStyle`；`sync()` 在换选中/文档变了时把草稿作废 | 单测 + `style_cmd` 沿用 SE-08 的合并撤销 |

### 5.2 实现要点

- `StepperEdit { row, draft, fresh, pinned }`（`Inspector.stepper_edit`）：同一时刻只开一格，
  换选中/文档变了就作废；`fresh` 是「这一帧刚开」的标记（egui 的焦点只能在控件存在之后请求），
  `pinned` 是金样种子（`MW_STEPPER_EDIT=行键:草稿`）——不抢焦点、也不因「没焦点」自己提交，
  离屏 harness 里焦点与光标闪烁都会让像素不稳。
- 编辑态用 `ui.new_child(max_rect)` 画（见 §5.3 ②），非编辑态照旧 `painter.text`。
- 轨迹：`inspector_stepper row=… edit=1`（开）、`edit=0 input="…" value=…`（收）。

### 5.3 途中撞见并修掉的两处问题

**① 编辑期会把下一行顶下去。** 第一版用 `ui.put(mid, TextEdit)`：`put` 把父光标推到输入框下沿，
输入框比 24px 的一格高一两像素，**「粗体/斜体」那行被顶下去**——基线里下面的行整体下移
（diff 带 y 620..634，205 px），开始打字、下面的行抖一下。改成 `ui.new_child(UiBuilder::new().max_rect(mid))`：
只在一个别人定好的矩形里画，不碰布局。改后 diff 收敛到编辑格本身。

**② 开编辑那一帧就自己提交了。** 第一版的判据是 `commit = enter || (!esc && resp.lost_focus())`，
而开编辑那一帧输入框还不存在、`lost_focus` 恒真——点一下还没打字，值就被原样提交了一遍。
改成开编辑帧不计（`was_fresh` 在清掉之前先取），金样种子再走 `pinned` 单独排除。

### 5.4 测试与机器判据

- 新增 4 条（`panels::inspector` 36 → **40**）：
  `typed_numbers_are_clamped_and_garbage_is_ignored`、`arrow_keys_step_a_number_like_the_buttons_do`、
  `typing_a_digit_opens_the_field_with_that_digit`、`enter_commits_the_typed_value_and_escape_throws_it_away`。
- 全量：`cargo test --workspace -- --test-threads=1`（TEMP 重定向）**963 passed / 0 failed / 1 ignored**（959 → 963）；
  fmt 干净；clippy 与基线比新增 0——第一轮门里 `let...else` 被判一条 `question_mark`（编辑分支的
  `let Some(e) = edit.as_mut() else { return None; }`），改 `let e = edit.as_mut()?;` 后归零。
- stage-b 门：**PASS，118 场 / 236 次比较（双跑，2026-10-30 08:0x 跑完）**，`--tol 0`；
  `inspector-stepper-edit` 首次进门，`inspector` 等十张老基线零漂移。stage-cm 门 PASS（48/96）。
  日志 `target/x100/stage-b-gate.log`、`target/x100/stage-cm-gate.log`。

### 5.5 漂移

- `inspector-stepper-edit`：新基线（1920×1080 @ ppp 1.5，warmup 8）。与 `inspector-shape`（同一页、
  同一节点、同一分组开合，只差「字号在编辑态」）的差 **406 px**，包围盒精确等于编辑格：
  数字文本带 y 572.7..584.7 + 1px 强调色底线 y 589.5（约 80 px 宽）——**布局零位移**。
- 既有十张检查器基线与全部画布/起始页/卡片基线：门两遍全过即证 0 px。

## 6. 未做与风险

| 项 | 状态 | 说明 |
| --- | --- | --- |
| 「点到别处提交」这一路 | 只做了判据，未实拍 | 丢焦点按提交算是刻意的（与画布行内编辑同一套手感）；单测钉了「有焦点 + Enter」与「Esc」两条 |
| 拖拽调值（drag to scrub） | 非目标 | 离屏 harness 拍不到、手感没法验；Xmind 的拖拽调值也非首选交互 |
| 编辑态的光标闪烁 | 已规避 | 金样种子 `pinned: true` 不抢焦点，格内不闪光标，基线稳定 |
| 多选「混合」时键入 | 提交应用到**全部选中** | `StepperRow.value` 是「混合」时的基准值；提交走 `style_cmd` 的批量路径（与点 `+` 同一条） |
| 实机微软拼音 | 未核 | 数值格只收数字键，IME 组合期理论上不干扰；不记 `已验证`，与 M4 同一条债 |
