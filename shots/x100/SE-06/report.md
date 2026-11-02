# SE-06 · 样式组：颜色 / 文本 / 边框与形状（P 轨 E2 第二包）

> 设计合同：`docs/30` §4.2（色板 8 列 24px、第 9 格自定义虚线圆、无颜色斜杠格、stepper 24 高 72 宽、对齐三段、字号 12–96、边宽 0–8、圆角 0–24、形状三档）。
> 执行合同：`docs/31` §5 第 SE-06 行。依赖 SE-05（骨架与 `group()`）。

## 1. 开工前审计（对着 `docs/30` §4.2 与合同条目逐条查代码）

| 合同条目 | 现状 | 结论 |
| --- | --- | --- |
| 色板 **8 列 × 24px 圆点，列距 8** | `swatches()` 用 17px 格、点半径 6、列距 3，列数按可用宽度**自动算**（11 个色 → 6+5 两排） | ❌ 尺寸与列数都不符 |
| 选中点 **2px 强调环 + 1px 内白** | 选中画 8.0 环（2.0 粗）+ 内描边 `ink.well` | 部分 ✅（环有、内白是主题 well 色不是纯白） |
| 第 9 格「自定义」**虚线圆** | 色板下面**单独一整行**：「自定义填充色…」/「自定义分支线色…」/「自定义文字色…」三处 | ❌ 正是 §4.5 禁止的「整行自定义」 |
| 「无颜色」**斜杠格** | 没有；清除覆盖只能点左侧标签（隐藏的知识） | ❌ |
| 数值：stepper **24 高、宽 72 右对齐** `- 值 +` | 字号是一个 `MenuButton`（九档菜单）；边宽 / 圆角**根本没有控件** | ❌ |
| 字号 **12–96** | 菜单只有 9 档（12/14/16/18/20/24/28/32/40） | ❌ 区间不符 |
| 边框宽度 **0–8** | `StyleOverride.stroke_width` 字段存在、`instances.rs` 按它描边，但检查器里没有任何入口 | ❌ 字段有能力、界面没有 |
| 圆角 **0–24** | `StyleOverride` **没有圆角字段**（`Shape::Rounded/Capsule/Rect` 三档写死在主题里） | ⛔ 数据模型不允许（见 §2） |
| 文本对齐三段（图标） | `StyleOverride` **没有对齐字段**，`Scene` 也没有对齐参数 | ⛔ 数据模型不允许 |
| 字重 **400/500/600** | `StyleOverride.bold: Option<bool>` 只有粗/不粗；检查器是 B / I 两个开关 | ⛔ 数据模型不允许（三档要新字段） |
| 形状：矩形 / 椭圆 / 圆角矩形 | `Shape` 四档：`Rounded/Rect/Capsule/Underline`，**没有椭圆**；四段控件已接 `SetStyle.shape` | 部分 ✅（椭圆缺） |
| 每项走 `Command` | 现有 `style_cmd()` 已按「一节点一条 `SetStyle`」批量下发（一条撤销） | ✅ |

**结论**：`docs/30` §4.2 的样式组里，**色板、stepper（字号/边宽）、边框颜色与宽度**能在现有数据模型内做完；
**圆角、文本对齐、字重三档、椭圆形状**需要 `StyleOverride` 加字段，按 `AGENTS.md` §8「不改 `Node`/`StyleOverride`/`.zmind`
字段（已批准例外除外）」本轮不动，记为 `延后` 并写清方案（§5）。

## 2. 非目标

- 不加 `StyleOverride` 字段（`radius` / `align` / `weight` / `shape=ellipse`）——需批准后另开一轮，方案见 §5。
- 不做作用域「本节点 / 含子树」两段切换（SE-08），现有「文字样式应用到子树」开关保持原样。
- 不做色板键盘全路径（SE-09 的 ↑↓/Enter/1–9）。
- 不改 `Theme` 调色板内容、不改 `.zmind`、不动主题文件。

## 3. 验收场景

1. **`inspector`**（1280×720 @1.5，`MW_INSPECTOR=1 MW_SELECT_DEMO=1`）：色板 8 列 × 24、列距 8；
   第 9 格是虚线圆（自定义）与斜杠格（无颜色）；字号与边宽是 24 高 72 宽的 stepper。
2. **`inspector-narrow`**（240）：色板按可用宽度降到 7 列（216 版心 → `floor((216+8)/32)`），不顶宽；stepper 不挤压标签。
3. **`cm-arrow-inspector`**（概念图 + 检查器）：重拍后确认概念图侧零漂移（只有检查器列变）。
4. **命令**：点「无颜色」→ `SetStyle{fill: None}`（或对应字段清空）；拖 stepper → 一条撤销；
   字重/对齐/圆角/椭圆在界面上**不出现**（没有假控件）。

## 4. 预计证据

- 轨迹：`swatches row=… cols=… cell=24`、`stepper name=… value=…`、`style cmd=…`。
- 基线：`inspector` / `inspector-narrow` 重拍；`cm-arrow-inspector` 重拍。
- 单测：色板网格（8 列 / 240 降列）、stepper 夹取与步进、无颜色格的命令、边框色命令。
- 两道门绿；测试数只增不减。

## 5. 数据模型缺口（延后项与方案）

| 缺什么 | 现模型 | 方案（待批准） |
| --- | --- | --- |
| 圆角 0–24 | `Shape` 三档写死 | `StyleOverride.radius: Option<f32>`（0–24，缺键=按 `Shape` 默认）；`theme::resolve` 落到 `NodeStyle.radius`，`instances.rs` 用它算圆角，`scene.rs` 指纹里混入 |
| 文本对齐 | 无 | `StyleOverride.align: Option<Align>`（left/center/right），落 `NodeStyle.align`，`Scene` 排字时按它定锚点 |
| 字重三档 | `bold: Option<bool>` | `StyleOverride.weight: Option<u16>`（400/500/600），缺键回落 `bold ? 700 : 400`；`bold` 保留为兼容别名 |
| 椭圆 | `Shape` 四档 | `Shape::Ellipse`（`"ellipse"`），`instances.rs` 画椭圆；旧文件无此键不受影响 |

四条都满足「缺键即老行为、不升 `FORMAT_VERSION`、无迁移」，但要改 `StyleOverride`/`Shape` 两个公共类型，
按 `AGENTS.md` §8 需先批准——本轮不擅自扩。

## 6. 结果（2026-10-30 回填）

### 6.1 做完了什么

| 合同条目 | 落地 | 证据 |
| --- | --- | --- |
| 色板 8 列 × 24px、列距 8 | `swatch_grid(可用宽)`：288 面板 8 列、240 面板 7 列，格恒 24、距恒 8 | `swatch_grid_keeps_eight_columns_until_the_panel_gets_narrow`；`inspector` / `inspector-narrow` 基线 |
| 选中 2px 强调环 + 1px 内白 | 环画在 r11（2px）+ r9.5（1px 白），压在 8px 列距里不压邻居 | 同上 |
| 第 9 格「自定义」虚线圆 | `dashed_circle()` 12 段短弧，点开就地展开 egui 取色器，不再占整行 | `palette_ends_with_custom_then_none` |
| 「无颜色」斜杠格 | 斜杠格 = 清覆盖回继承；继承态自己带 2px 强调环 | 同上 + 命令测试 |
| stepper 24 高 × 72 宽 `− 值 +` | 字号 12–96 步 2、边宽 0–8 步 1，两端能停住 | `stepper_stops_at_the_contract_ends`；`inspector-shape` 基线 |
| 每项走 `Command` | 每个动作一条 `SetStyle`，一次撤销；多选合批 | `style_edits_merge_into_each_nodes_own_override`、`branch_color_is_a_per_node_override_that_clears_back_to_inheritance` |

### 6.2 本轮新改的四件事

1. **`palette_cells()` 抽纯函数 + 新单测**。次序（主题色 → 自定义 → 无颜色）以前只活在
   `swatches()` 的局部 `Vec` 里，没人钉；一旦有人把「无颜色」插到前面，像素上第 9 格就不再是
   虚线圆。现在 `palette_ends_with_custom_then_none` 对四个主题逐个断言。
2. **色板方向键补齐 ↑↓**。原来只有 ←→（同排移动）。`docs/30` §4.3 写的是「↑↓ 在色板内移动」，
   雪主题的色板正是两行（8 色一行、自定义+无颜色一行），只给 ←→ 时第二行走不到。
   现在 ←→ 同排、↑↓ 跨行，两端夹取。键鼠仍共用一个出口（`Sense::click()` 的 Space/Enter）。
3. **`stepper()` 8 参数 → `StepperRow` 结构**。八个参数触发 clippy `too_many_arguments`
   （基线里没有这条，属于「新增必须为 0」），把「显示什么 / 从哪个值加减 / 区间 / 步长」
   收进一个小结构体，签名回到 5 个。
4. **`MW_INSPECTOR_GROUPS` 预置分组开合**（证据专用）。「形状与文字」默认折叠，不预置的话
   边框/边宽/形状/字号四行永远进不了镜头——本轮 SE-06 自己交付的控件，三张老基线一张都拍不到。
   新旋钮启动时把 `键=0/1` 塞进内存；`WindowState::seed_inspector_group` **只写内存不落盘**
   （写完后把 `saved` 对齐），所以拍金样不会顺手改掉用户自己的 `window.json`。
   新单测 `seeded_groups_stay_in_memory_and_never_touch_the_file` 钉住这条。

### 6.3 主题色板个数：一个必须写下来的既有事实

`Theme.palette` 各主题长度不一：**雪 8 个**，墨夜 / 雾 / 沙各 **6 个**。

- 后果：`docs/30` §4.2 的「8 列 × 24px …… 第 9 格是自定义虚线圆」只在雪主题字面成立；
  另外三个主题 6 色 + 2 格 = 8 格，一行就装下，自定义落在第 7 格。
- 本轮**不动**主题令牌：`palette` 同时喂着标签色、分支色等其它面，统一成 8 色会牵动
  主题基准与一大批金样，属于设计决定而不是样式组的收尾。记为待决定项（§7）。
- 新单测把事实写死：`palette` 长度必须 ≤ 8（超过就会把自定义格顶到第二行），
  且「主题色之后第一格是自定义、末格是无颜色」。

### 6.4 基线（`--tol 0`，每张连跑两遍零差异）

| 场景 | 环境 | 结果 |
| --- | --- | --- |
| `inspector` | 1280×720 @1.5，`MW_INSPECTOR=1 MW_SELECT_DEMO=1` | 重拍，漂移 **29434 px / 1.419%**，bbox `x1502..1877 y523..975`（只在检查器版心内，画布 0 px） |
| `inspector-narrow` | 同上 + `MW_INSPECTOR_W=240` | 重拍，漂移 **28147 px / 1.357%**，bbox `x1574..1902 y523..975` |
| `inspector-empty` | 只开检查器、不选中 | **0 px**（没选中就没有样式组，反证改动只在样式组内） |
| `inspector-shape` 🆕 | 同上 + `MW_INSPECTOR_GROUPS=style/color=0,style/shape=1` | 新基线；与 `inspector` 差 43933 px，bbox `x1489..1918 y420..1044` 全在面板内 |
| `inspector-ink` 🆕 | 同上 + `MW_THEME=ink` | 新基线（深色下色板/分组标题/stepper 的对比度） |
| `inspector-mist` 🆕 | 同上 + `MW_THEME=mist` | 新基线 |
| `inspector-sand` 🆕 | 同上 + `MW_THEME=sand` | 新基线 |
| `cm-arrow-inspector` | 概念图 + 箭头菜单 + 检查器 | **0 px**（概念图侧零漂移，不需要重拍） |

`§3` 第 2 条开工时写的「色板降到 6 列」是估数，实测 **7 列**（240 − 2×12 = 216 版心，
`floor((216+8)/32) = 7`），测试与基线都按 7 钉住。

### 6.5 门与测试

- `cargo fmt --all -- --check`：干净。
- `cargo clippy --workspace --all-targets`：指纹 **当前 37 / 基线 38 / 新增 0**（少报的那条是
  `mw-render` 的缓存未重报，不算新增）。
- `cargo test --workspace -- --test-threads=1`（TEMP/TMP 指向 `target/tmp`）：
  **948 passed / 0 failed / 1 ignored**。
- `scripts/x100-stage-b-gate.ps1`：**PASS**，golden matrix 两遍各 **114 比较**（原 110 场景 + 4 张新基线 = 228 比较）。
- `scripts/x100-stage-cm-gate.ps1`：**PASS**，concept golden 两遍各 48 比较。

## 7. 未做与风险

- **延后（数据模型不允许，待批准）**：圆角 0–24 / 文本对齐三段 / 字重三档 / 椭圆形状，方案见 §5。
  界面上**没有假控件**，这四项在检查器里不出现。
- **待决定**：四个主题的 `palette` 长度不一（雪 8 / 其余 6）。要不要统一成 8 色，是主题令牌层面的
  决定（会牵动主题金样与标签/分支颜色），不在 SE-06 内改。
- **未核**：实机键盘手感（↑↓ 跨行、Tab 序）与实机微软拼音——本机离屏金样只能证明像素，不能证明手感，
  所以本包记 `已实现`，不记 `已验证`。
- 色板的数字键 1–9 直选（`docs/30` §4.3 另一条）仍归键盘包（SE-09/SE-10），本轮没做。

