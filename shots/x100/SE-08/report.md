# SE-08 · 作用域与批量编辑（P 轨 E2 第四包）

> 设计合同：`docs/30` §4.1（分组标题右侧的「本节点 / 含子树」两段）、§4.3（500ms 内同类连续操作
> 合并为一条撤销；改动即时反映，不设「应用」按钮）、§4.6（`inspector-multiselect` 场景）。
> 执行合同：`docs/31` §5 第 SE-08 行。依赖 SE-06（样式组控件）、SE-07（内容组分组与行几何）。

## 1. 开工前审计（对着 `docs/30` §4.1 / §4.3 / §4.6 逐条查代码）

| 合同条目 | 现状 | 结论 |
| --- | --- | --- |
| 作用域是**分组标题右侧**的「本节点 / 含子树」两段 | 作用域存在，但是在样式页**顶上单独一行**：`作用` + 两段 `当前节点`(72×24) / `含子树`(64×24)（`style_body` 开头那个 `ui.horizontal`） | ❌ 位置与文案都不符 |
| 作用域**只对样式类属性生效**（颜色/字号/边框/形状），不作用于备注/链接/标签 | 只有 `style_cmd` 读 `self.style_subtree`；备注/链接/标签各走自己的 `Command` | ✅ |
| 500ms 内同类连续操作合并为一条撤销 | 每次点击/拖动步进各发一条 `Batch`（`style_cmd` → `wrap`）。连续点色板 5 次就是 5 条撤销 | ❌ 没有时间窗合并 |
| 批量后画布即时反映 | `apply_command` 后当场重建 `Scene`，画布同帧反映 | ✅ |
| 多选时检查器显示作用域切换 | 多选有 `items.len() > 1` 的分支（对象行「N 个节点」），但作用域那行在多选时也照样在顶上 | 部分 |
| `inspector-multiselect` 场景 | 不存在；也没有能把演示文档选中三颗的旋钮（只有 `MW_SELECT_DEMO` 单颗） | ❌ 本包交付拍不到 |

**结论**：SE-08 的实质工作是三件——把作用域从「顶上单独一行」搬到**分组标题右侧**并按合同改成
「本节点 / 含子树」；给 `SetStyle` 一类操作加 **500ms 合并窗口**；补 `MW_SELECT_MULTI` 旋钮与
`inspector-multiselect` 基线。其余（作用域只作用于样式、即时反映）已实现，本包只做核对与证据。

## 2. 非目标

- 不做「按 id 反选 / 只改子树里某些节点」的复杂作用域；两段就是全部。
- 不给内容类属性（备注 / 标签 / 链接 / 任务）加作用域——合同明说不作用。
- 不改 `StyleOverride` / `Node` 字段；合并窗口只是撤销栈行为。
- 不做「保存为样式」、不做批量删除/移动（那是画布与大纲的事，§4.5 禁止）。

## 3. 验收场景

1. **`inspector-multiselect`**（1280×720 @1.5，`MW_INSPECTOR=1 MW_SELECT_DEMO=multi MW_WINDOW_SIZE=1280x720`）：
   三颗节点选中（对象行「3 个节点」）、样式组标题右侧有「本节点 / 含子树」两段、当前在「本节点」。
2. **作用域往返**：单测——`style_cmd` 在 `style_subtree=true` 时下发的 id 集合 = 选中节点 ∪ 子树，
   且同一改动对每个节点各一条 `SetStyle`，整体一条 `Batch`（一次撤销）。
3. **合并窗口**：单测——同一手势键在 500ms 内连发三条 `SetStyle`，撤销栈只多一条；
   超过 500ms 或换手势键则各算一条。
4. **即时反映**：基线里选中态的填充色即为刚改的颜色（不需要「应用」）。

## 4. 预计证据

- 基线：新 `inspector-multiselect`；`inspector` 因作用域搬家重拍并量化漂移。
- 单测：作用域 id 集合、合并窗口（含边界 499/501ms）、手势键变化即断。
- 两道门绿；测试数只增不减。

## 5. 数据模型核对（`docs/31` §5 P0 风险行：新增能力有没有字段承载）

| 能力 | 承载 | 结论 |
| --- | --- | --- |
| 作用域 | `Command::SetStyle { ids, .. }` 已经是多 id；`InspectorItem.subtree` 已带子树快照 | 有 |
| 合并窗口 | 撤销栈 `Document::apply_coalesced(cmd, merge)`（CER/M8 已有） | 有 |
| 多选 | `Canvas::selection` 与 `Inspector.items` 已支持多选 | 有 |

三项都不需要新字段，`docs/31` §5 的 P0 风险不触发。

## 6. 结果（实测）

### 6.1 交付项

| 合同条目 | 做完的样子 | 证据 |
| --- | --- | --- |
| §4.2 作用域 = **分组标题右侧**两段 | 「颜色」与「形状与文字」两组的标题行右侧各画「本节点 / 含子树」两段（11.5/16），选中那段给 8% 强调底 + 强调色文字；有开关的组**不再画右摘要**（两个都靠右，288 宽放不下）；点开关的那一下不折叠分组 | `the_scope_switch_lives_in_the_group_header`（画一帧：有「本节点」「含子树」、没有「作用」「当前节点」）；`inspector-multiselect` 基线 |
| §4.3 500ms 内同类连续操作合并一条撤销 | 新增 `style_gesture_key()`（只有 `SetStyle` / 全 `SetStyle` 的 `Batch` 有手势键 = 排序去重后的 id 集）与 `same_gesture(key, since≤500ms)`；`Canvas::apply_command_coalesced` 走 `Document::apply_coalesced`。不是样式命令（换主题/写备注/删除）会把窗口关掉，后面的编辑不被粘上 | `style_gestures_merge_only_on_the_same_ids_within_500ms` |
| 作用域只对样式类属性生效 | 旧实现就是这样（只有 `style_cmd` 读 `style_subtree`），本轮核对后未动 | `subtree_styles_include_self_and_descendants_once`（既有） |
| 批量后画布即时反映 | `apply_command` 当帧重建 `Scene` | `inspector-multiselect` 基线 |
| §4.6 `inspector-multiselect` 场景 | `MW_SELECT_DEMO=multi`：选中「纯 Rust 原生」再补选它的父级与第一个子级（对象行「3 个节点」） | `shots/golden/inspector-multiselect/shell-ppp1.50.png` |

### 6.2 命令细节（本轮唯一的结构改动）

- 「作用 / 当前节点 / 含子树」那一行**删掉**；两段进分组标题，走 `group_scoped()` →
  `group_header(..., Some(subtree), ...)`，返回 `(开合, 作用域)`。
- 点在开关上的那一下不算点行：`resp.clicked() && !scope_rect.contains(pointer)`——
  否则想切作用域却把组收起来。
- 合并窗口在 `main.rs` 的 `IA::Apply` 分支：`style_gesture_key(&cmd)` →
  与 `Shell.style_gesture`（上一次的手势键 + 时刻）比；同键且 ≤500ms 就走
  `apply_command_coalesced`，否则普通 `apply`。壳层记录的手势键只有样式命令才有。

### 6.3 测试

新增 2 条（`panels::inspector` 现 956 条全绿）：

- `the_scope_switch_lives_in_the_group_header` — 画一帧，断言「本节点」「含子树」在、
  旧文案「作用」「当前节点」不在。
- `style_gestures_merge_only_on_the_same_ids_within_500ms` — 手势键四态（单节点 / 双节点 /
  排序去重后的 Batch / 混入删除则 None）+ 窗口边界（0/500ms 并、501ms 断、换作用域断、
  上一件事不是样式编辑断）。

### 6.4 机器判据

- 全量：`cargo test --workspace -- --test-threads=1`（TEMP 重定向）**956 passed / 0 failed / 1 ignored**（954 → 956，+2）；fmt 干净；clippy 指纹与基线比**新增 0**。
- stage-b 门：**PASS，116 场 / 232 次比较（两遍各 116），2026-10-30 06:29 跑完**，`--tol 0`；`inspector-multiselect` 首次进门。
- stage-cm 门：**PASS，48 场 / 96 次比较，06:32 跑完**；`cm-arrow-inspector` 零漂移。

### 6.5 漂移

作用域从页顶搬进分组标题后，样式页六张基线**按预期重拍**（旧的「作用」行没了，组体整体上移约 32 css px）：

| 场景 | 漂移 | 包围盒 |
| --- | --- | --- |
| `inspector` | 59055 px / 2.848% | x[1506..1901] y[335..1043] |
| `inspector-narrow` | 56253 px / 2.713% | x[1578..1901] y[335..1043] |
| `inspector-shape` | 43983 px / 2.121% | x[1489..1917] y[335..1043] |
| `inspector-ink` | 63195 px / 3.048% | x[1506..1901] y[335..1043] |
| `inspector-mist` | 62522 px / 3.015% | x[1506..1901] y[335..1043] |
| `inspector-sand` | 62521 px / 3.015% | x[1506..1901] y[335..1043] |

六张的包围盒全在检查器面板内（画布侧 0 px）；`inspector-empty`（无选中走空状态，没有那行）与
`inspector-content`（内容页不画样式组）零漂移。重拍后 `--update` 覆写，门里连跑两遍。

## 7. 未做与风险

| 项 | 状态 | 说明 |
| --- | --- | --- |
| 作用域开关的键盘路径（Tab 进开关、←→ 切换） | 未做 | §4.4 是 SE-09（检查器键盘与无障碍）的合同，本轮不做 |
| 「含子树」与「文字样式应用到子树」并存 | 设计张力 | 前者是检查器的**临时**批量（改完就结束），后者是节点上**持久**的 `StyleOverride.apply_text_to_subtree`（文字色继承）。两个开关在同一页，含义不同但措辞接近；要不要合并语义得先在 `docs/30` 定，本轮不动模型 |
| 合并窗口与撤销栈的交互 | 只测了纯函数 | `apply_coalesced` 的栈顶合并在 `mw-core` 已有测试；App 层的 500ms 窗口没有无头 `App` 可测，靠 `same_gesture` 的边界单测 + 实机核对 |
| 实机手感 | 未核 | 连点 stepper 的撤销次数、真手拖面板时开关是否跟随，都不记 `已验证` |
