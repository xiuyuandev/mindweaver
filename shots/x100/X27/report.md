# X27 · 交互状态表（悬停 / 选中 / 主选 / 多选 / 键盘焦点 / 编辑 / 禁用 / 非法拖放）

> 合同：`docs/24` §阶段 C「X27 建立悬停、选中、主选、多选、键盘焦点、编辑、禁用和非法拖放状态表」。
> 设计来源：`docs/28` §4.3（颜色与状态令牌）、§4.5（动效 120/140ms）、§4.6（焦点与键盘）、
> §5.3（大纲状态表）、§5.6（大纲拖拽）；`docs/30` §4（检查器）、§5.3（概念节点三档）、
> §5.12（切换型交互要拍「撤下」）、§5.13（拖拽要拍「松手之后」）；`docs/02` §10（快捷键与禁用）。
> `docs/06` 只作外壳/令牌参考，**不当合同**（其全屏大纲旧规格已被 `docs/28` 取代）。
> 状态：**进行中**（本轮只做文档与只读审计：代码零改动、金样未拍、门未跑）。
> 状态词只用「未开始 / 进行中 / 延后」；本包不写「已完成 / 已验证」。

---

## 0. 一页结论

| 项 | 数 | 说明 |
| --- | --- | --- |
| 状态 × 表面单元格 | **24** | 8 状态 × 3 表面（画布 / 大纲 / 检查器） |
| 有实现 | 18 | 见 §2 逐格 |
| **缺失** | **3** | 画布「主选」、画布「键盘焦点」、大纲「键盘焦点」 |
| 部分 | 1 | 检查器「主选」（多选时只显示「N 个节点」，主选身份不可见） |
| 不适用 | 2 | 检查器「选中」（检查器是选中后的反映面）、检查器「非法拖放」（无拖放目标） |
| 与合同不一致 | **4** | I1 概念图悬停环宽、I2 概念图选中环宽、I3 选中时长、I4 焦点环透明度 |
| 合同缺口（需补定义） | **3** | G1 画布主选无定义、G2 画布键盘焦点无定义、G3 检查器主选不可见 |
| 预计新增金样 | 10 | §5 只列名字，**本包不拍图** |

三条硬不变量（X28/X36/X37 引用本表时按这三条走）：

1. **三条选中道互斥**：节点 / 关系线 / 注解任何时刻只许一条非空（M26 已收口，
   `SelectionLane` + `clear_foreign_selection`，轨迹 `selection_clear lane=… reason=…`）。
2. **hover 在 golden 里必须靠 `MW_GOLDEN_INPUT` 脚本**：离屏无脚本时每帧清空指针，
   不许「赌 hover」（`AGENTS.md` §7 第 6 条）。
3. **主选 ≠ 选中**：多选时「主选」是唯一被对象行/命令面板当作「当前对象」的那一颗；
   画布与检查器目前都没有把这一点画出来（G1/G3）。

---

## 1. 范围与非目标

### 1.1 范围（本轮交付）

1. 把 8 类状态在 3 种表面上的**实际取值**列成矩阵，逐格标出处（文件 + 那一行的字面）。
2. 标出**缺失格**与**与合同相反/不一致**的地方，各带证据条目名。
3. 给每格写**怎么判**（像素判据或轨迹判据），作为后续包（X28/X36/X37 与打磨轮）的验收底稿。
4. 只列预计 golden 场景**名单**，不拍图、不登记门。

### 1.2 非目标

- **不改 `crates/`**：本轮只读审计；并行会话正在改 `crates/`（含 `mw-io` 与画布底色）。
- 不拍 golden、不跑 `cargo`、不登记 `scripts/` 门、不动 `docs/STATUS.md`（Lead 回填）。
- 不新增状态机代码、不改字段、不改 `.zmind`。
- 不覆盖卡片页 / 笔记库 / 起始页（那是 X28/X36/X37 的表面；本表只立画布/大纲/检查器三面）。
- 不裁决 I1–I4 该改码还是改文档（决定权在 Lead；见 §6 R2）。

---

## 2. 状态表

记号：**有** = 代码里有可见取值；**部分** = 有实现但不完整；**缺失** = 需要但代码里没有；
**不适用** = 该表面不存在这个状态。所有行号是快照，同时给出那一行的字面。

### 2.0 总览矩阵

| 状态 | 画布 | 大纲 | 检查器 |
| --- | --- | --- | --- |
| 悬停 | 有 | 有 | 有 |
| 选中 | 有 | 有 | 不适用 |
| 主选 | **缺失** | 有 | 部分 |
| 多选 | 有 | 有 | 有 |
| 键盘焦点 | **缺失** | **缺失** | 有 |
| 编辑 | 有 | 有 | 有 |
| 禁用 | 有（铬件级） | 有（菜单项级） | 有 |
| 非法拖放 | 有 | 有 | 不适用 |

### 2.1 悬停

| 表面 | 视觉取值 | 出处 | 状态 |
| --- | --- | --- | --- |
| 画布（思维导图节点 / 概念节点同一条路径） | 强调色**4% 淡底** + **1.5px 强调环**（环强度 = `hover_alpha × 0.55`，即 55%），120ms 淡入；**只在未被选中时画**（已选中的节点悬停仍走选中档） | `crates/mw-render/src/instances.rs:35` `pub const HOVER_FILL: f32 = 0.04;`；`:36` `pub const HOVER_RING_W: f32 = 1.5;`；`:620` `(cam.css_to_world(HOVER_RING_W), f.hover_alpha * 0.55)`；`:611` `let has_sel = \|idx: u32\| f.selected.iter().any(…);`；`crates/mw-shell/src/canvas/consts.rs:4` `HOVER_TRANSITION = design::motion::FAST`（0.12s，`mw-core/src/design.rs:737`） | 有 |
| 画布（折叠圆点 / 概要 / 边界，壳层覆盖层） | 圆点**放大到 1.3 倍**（`s = 1 + 0.3×alpha`）；概要/边界外扩 2px 描 **2px 强调色 55%**；均 120ms；正在选中/正在编辑的不重复画 | `shots/x100/M17/report.md` §3.2（实现见 `crates/mw-shell/src/canvas/hover.rs`，`:55` 注释「强调强度与节点悬停同一档（渲染层用 `hover_alpha × 0.55` 画环）」） | 有 |
| 大纲行 | `state.hover` = **主色 4%（暗色 6%）** 底 + 圆角 **6**；握把在悬停 **120ms** 后出现（选中行立即出现） | `crates/mw-shell/src/panels/outline.rs:2454` `let hover_bg = c32a(content_colors.hover); // 主色 4%`；`:47` `const GRIP_DELAY: std::time::Duration = std::time::Duration::from_millis(120);`；`:1265` `selected \|\| hovered_for.is_some_and(\|d\| d >= GRIP_DELAY)` | 有 |
| 检查器控件 | 悬停底 `ch.hover`（`ui_kit` 工具按钮：`if enabled && (resp.hovered() \|\| active)` 才铺底 + 手型光标）；色板/分段/stepper 走同一套 chrome | `crates/mw-shell/src/ui_kit.rs` `tool_button` 内 `if enabled && (resp.hovered() \|\| active) { … ch.hover … }` | 有 |

### 2.2 选中

| 表面 | 视觉取值 | 出处 | 状态 |
| --- | --- | --- | --- |
| 画布（节点） | 强调色**8% 淡底** + **2px 强调环**，外扩 `SELECT_RING_GAP = 2.5`；180ms 淡入（`SELECT_TRANSITION = BASE`）；环/底都用 `corner_style`，椭圆节点画椭圆环 | `instances.rs:34` `SELECT_FILL = 0.08`；`:30` `SELECT_RING_W = 2.0`；`:31` `SELECT_RING_GAP = 2.5`；`:622` `(cam.css_to_world(SELECT_RING_W), f.select_alpha)`；`canvas/consts.rs:5` `SELECT_TRANSITION = design::motion::BASE`（0.18s） | 有 |
| 画布（关系线 / 注解） | 关系线选中 2px 强调描边；注解（概要/边界）**2px 强调色矩形**圈住跨度，跨度手柄在两端 | `crates/mw-shell/src/canvas/canvas_methods_2.rs:392` 注释「注解选中环：每条都画一圈 2px 强调色」；`:2279` 注释「选中环是圈住跨度的 2px 强调色矩形」 | 有 |
| 大纲行 | `state.selected` = **强调色 7%** 底 + 圆角 6，无边框 | `outline.rs:2453` `let sel_fill = c32a(content_colors.selected); // 强调色 7%`；`mw-core/src/design.rs:388` `selected: with_alpha(accent, a.selected)` | 有 |
| 检查器 | 不适用：检查器是「被选中对象」的反映面，自身不画选中态；对象行 44 高显示标题 + 副行（单选 `一级主题 · N 个子主题`，多选 `3 个节点`） | `crates/mw-shell/src/panels/inspector/inspector_methods_1.rs:325` `format!("{} 个节点", self.items.len())`；`docs/30` §4.1 对象行 | 不适用 |

### 2.3 主选（多选里「当前那一颗」）

| 表面 | 视觉取值 | 出处 | 状态 |
| --- | --- | --- | --- |
| 画布 | **无区分**：`instances` 里所有选中节点同一档（2px / 8%），渲染层不读 `mul sel.primary()`；主选只影响命令落点（如 `Ctrl+L` 源节点、结构移动起点） | 反证：`crates/mw-render/src/instances.rs` 全文无 `primary`；主选读取点 `canvas/canvas_methods_1.rs:206` `let start = self.sel.primary();`、`:626` `let Some(from) = self.sel.primary() else {` | **缺失** |
| 大纲 | 有：主选 7% 强调底，非主选 **5%**（强调色 alpha 13/255） | `outline.rs:2456` `let multi_fill = Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 13);` + 注释「多选非主选 5%（§5.3 多选行）」 | 有 |
| 检查器 | 部分：单选时对象行显示该节点标题；多选时副行只说「N 个节点」，**主选是谁看不到**（也不能从对象行切换主选） | `inspector_methods_1.rs:297` 注释「多选时副行固定说「N 个节点」——那才是这一刻真正需要知道的事」；`:325` 同上 | 部分 |

### 2.4 多选

| 表面 | 视觉取值 | 出处 | 状态 |
| --- | --- | --- | --- |
| 画布 | 每颗选中节点各画一圈 2px + 8% 底（数量无上限）；多节点拖拽时额外画**并集虚线框**（`ghost` 只在 >1 节点时画）；状态栏写 `已选 N 个主题` | `instances.rs` 同上；`instances.rs` 幽灵段注释「只在多根/多节点时才画」；`crates/mw-shell/src/app_helpers.rs:204` `n => Some(format!("已选 {n} 个主题"))`、`:202` `Some(format!("已选：{}", shorten(text, 18)))`（单选） | 有 |
| 大纲 | 多选每行 5% / 主选 7%；行尾 `⋯` 菜单在多选时切成 **5 项**（单行 12 项） | `outline.rs` 同上；`outline.rs:300` `pub(crate) fn row_menu_multi(selected: &[String], target: &str) -> bool`；`shots/x100/M25/report.md`（单行 12 项 / 多行 5 项） | 有 |
| 检查器 | 显示「3 个节点」+ 混合态控件 + 改动按各节点自己的覆盖合并成一条 `Batch` 下发；作用域两段「本节点 / 含子树」在分组标题右侧 | `inspector.rs:12` 注释「多选时控件显示「混合」态：改动按每个节点**各自的覆盖**合并后批量下发」；`shots/x100/SE-08/report.md` §4（作用域位置/文案/500ms 合并） | 有 |

### 2.5 键盘焦点（Tab/方向键进入时与鼠标选中**不同**的那一态）

| 表面 | 视觉取值 | 出处 | 状态 |
| --- | --- | --- | --- |
| 画布 | **没有可见焦点环**。画布只通过 `wants_keys` 抢/放键盘（`F6` 交出），焦点归属的唯一提示是状态栏在面板握键盘时写 `键盘：面板 · F6 收回`；画布握键盘时没有对应文字，也没有环 | `canvas/canvas_methods_2.rs:11` `let (rect, response) = canvas_area(ui, available, self.wants_keys);`；`:179` `if response.has_focus()`（只用于按键路由，不是绘制）；`canvas/canvas_methods_1.rs:1930` 注释「`F6` 交出键盘：画布退出焦点环，`Tab` 才走得到检查器里的强调色色板」（此处「焦点环」指 Tab 焦点链，不是画出来的环）；`app_methods_2.rs:1824` `egui::RichText::new("键盘：面板 · F6 收回")` | **缺失** |
| 大纲 | 没有 1.5px 焦点环。行绘制只有三种填色：`editing` / `selected` / `hovered`；键盘进入大纲时把 egui 焦点放到当前行（注释「进大纲就把焦点环放在当前行上」），但**这一态没有画出环**，键盘与鼠标两种来源在像素上不可分 | `outline.rs:2453-2456` 三个 fill 常量 + `:3209` `painter.rect_filled(row_rect, row_radius, edit_fill)`；`outline.rs` 全文无 `paint_focus_ring`；`app_methods_2.rs:2182` 注释「进大纲就把焦点环放在当前行上：不用先按一下方向键才知道键盘归谁」 | **缺失** |
| 检查器 | 有：`paint_focus_ring` 画 **1.5px 强调色**、外偏移 **1px**、外部描边；色板/分段/stepper/文本域各自调用 | `crates/mw-shell/src/ui_kit.rs:229-243` `pub fn paint_focus_ring(…){ if resp.has_focus() { painter.rect_stroke(rect.expand(design::component::FOCUS_RING_OFFSET), …, Stroke::new(design::component::FOCUS_RING, ch.accent), StrokeKind::Outside); } }`；`mw-core/src/design.rs:658/660` `FOCUS_RING_OFFSET = content::focus::OFFSET` / `FOCUS_RING = content::focus::RING`；`:421-425` `RING 1.5 / OFFSET 1.0 / ALPHA 0.80`；调用点 `panels/inspector.rs:934/1025/1123/1173` | 有 |

> 合同要求：`docs/28` §4.6「鼠标点击显示『选中』，键盘 Tab/方向键显示『焦点环』；两者不同时伪装成同一种状态」
> + `docs/30` §5.3「选中 = 1.5px 强调描边 + 8% 底 + 焦点环 1.5px；键盘焦点 = 1.5px 环画在节点外 2px」。
> 画布与大纲目前都没有实现「焦点环」这一态 → §3 的 I2/I5 与 G2。

### 2.6 编辑

| 表面 | 视觉取值 | 出处 | 状态 |
| --- | --- | --- | --- |
| 画布（节点就地编辑） | 合成器对 `editing` 节点**跳过**自己的环（否则与 egui 编辑器重影）；egui 在节点盒**外侧**画 **2px 强调色环**，环心距盒 `SELECT_RING_GAP + ring_w × 0.5`，与选中环共用几何常量；椭圆节点画椭圆环；半径走同一个 `corner_style` 出口 | `instances.rs:615` `// 行内编辑时强调环由 egui 编辑器画，合成器再画一圈会重影。` + `if f.editing == Some(*idx) { continue; }`；`crates/mw-shell/src/canvas.rs:1948-1950` `let ring_w = mw_render::SELECT_RING_W; let ring_off = mw_render::SELECT_RING_GAP + ring_w * 0.5;`、`:1951` `let ring_stroke = egui::Stroke::new(ring_w, color32(theme.ui.accent));` | 有 |
| 大纲行 | `state.editing` = **强调色 3%** 底、无边框、光标可见、握把隐藏；行内编辑走同一字阶 | `outline.rs:2455` `let edit_fill = c32a(content_colors.editing); // 强调色 3%`；`:3209` `painter.rect_filled(row_rect, row_radius, edit_fill)`；`docs/28` §5.4 | 有 |
| 检查器 | 有：stepper 数值格可键入（点/`Enter`/敲数字进编辑，`↑↓` 步进，`Enter` 提交 `Esc` 取消）；备注/链接是 `TextEdit`；失败时面板内 **28px 错误条** + toast；`Esc` 两级（先退控件再退面板） | `inspector.rs:111-112` `/// 面板内的错误条（§4.3「命令失败：面板内 28px 错误条 + toast」）`、`pub const ERROR_H: f32 = 28.0;`；`shots/x100/SE-06b/report.md`（stepper 编辑 + `MW_STEPPER_EDIT`）、`shots/x100/SE-09/report.md`（`esc_step` 两级、`inspector-error`） | 有 |

### 2.7 禁用

| 表面 | 视觉取值 | 出处 | 状态 |
| --- | --- | --- | --- |
| 画布（铬件：工具条 / 右键菜单 / 命令面板） | 禁用按钮**不可点**（`Sense::hover()` 而非 `click`）、图标色 `ch.icon_off`、仍给 `tip` 提示；禁用菜单项由 `Item::row(…, false)` 表达；工具条另有 `on_disabled_hover_text("先在画布上选中一个主题")` 这一类原因提示 | `ui_kit.rs` `let sense = if enabled { Sense::click() } else { Sense::hover() };`、`let ink = if active { ch.accent } else if !enabled { ch.icon_off } else { ch.text };`；`app_methods_2.rs:873` `None => items.push(Item::row("折叠", "/", A::ToggleCollapse, false)),`；`:3163` `.on_disabled_hover_text("先在画布上选中一个主题")` | 有（铬件级） |
| 大纲行 | 行本身无禁用概念；禁用发生在**行菜单项**（如根主题禁剪切/删除、无子节点时「折叠」禁用）与「更多」按钮的显隐 | 同上 `Item::row(…, false)`；`shots/x100/M25/report.md`（单行 12 项：画布专属项不收） | 有（菜单项级） |
| 检查器 | 有：无选中 → 不显示属性控件、只留一句话 + 文字动作（`selected.is_empty()` 早退）；属性「已覆盖」时小标题旁一颗圆点 + `↺` 恢复继承，悬停提示「已覆盖（↺ 恢复继承）」；命令失败 → 28px 错误条 | `inspector/inspector_methods_1.rs:54` `if selected.is_empty() { … return; }`；`:1235` `/// 样式区的小标题 + "已覆盖"圆点 + 一键恢复。返回是否点了恢复。`、`:1256` `resp.on_hover_text("已覆盖（↺ 恢复继承）");`；`inspector.rs:112` `ERROR_H = 28.0` | 有 |

### 2.8 非法拖放

| 表面 | 视觉取值 | 出处 | 状态 |
| --- | --- | --- | --- |
| 画布 | 拖到自身/后代等非法落点：光标 `CursorIcon::NotAllowed`；落点指示转成**警示色**（`dt.illegal` 走红色/警告分支）；轨迹 `drop_zone node=… zone=… illegal=true at=(x,y)` | `canvas/canvas_methods_2.rs:803-805` `} else if self.moving.as_ref().is_some_and(Moving::illegal) { … CursorIcon::NotAllowed`；`:1731` `"drop_zone node={} zone={} illegal={} at=({:.0},{:.0})"`；`canvas/canvas_methods_3.rs:2498` 注释「`dt.illegal` 只是上一帧的显示判定；`plan_moves` 会按当前 roots 再守一遍」；`crates/mw-shell/src/canvas.rs:354` `fn illegal(&self) -> bool { self.drop.is_some_and(\|d\| d.illegal) }`、`:1669` `let src = if dt.illegal {` | 有 |
| 大纲 | 拖到自身/子树：光标 `CursorIcon::NoDrop`；插入线转 `ch.warn` 色（合法时 `accent`）；不执行落点；轨迹 `outline_drag_illegal` | `outline.rs:4531` `ui.ctx().set_cursor_icon(egui::CursorIcon::NoDrop);`、`:4530` `crate::trace::event("outline_drag_illegal");`、`:4562` `if drag_illegal { ch.warn } else { accent }`、`:4538` `let line = if drag_illegal {` | 有 |
| 检查器 | 不适用：检查器里没有拖放目标（作用域/分段/色板都是点击） | `docs/30` §4.2 控件清单无拖拽语义 | 不适用 |

---

## 3. 现状与合同不一致项清单

每条都带**证据条目名**（已有基线 / 代码位置 / 既有报告），便于 X28/X36/X37 引用时直接复核。

### 3.1 A 类：代码与合同不一致（需 Lead 决定改码还是改文档）

| ID | 合同 | 现状 | 证据条目 | 影响面 |
| --- | --- | --- | --- | --- |
| **I1** | `docs/30` §5.3「悬停：**1px** 强调描边 + 4% 底」 | `HOVER_RING_W = 1.5`（画布与概念图共用一条路径），实际 1.5px | 代码 `mw-render/src/instances.rs:36`；无「概念节点悬停」专项基线（现有 `cm-rel-hover` 拍的是关系线，不是节点） | 概念图 / 思维导图节点；改 1.5→1.0 是外观改动，需重拍所有含悬停节点的基线 |
| **I2** | `docs/30` §5.3「选中：**1.5px** 强调描边 + 8% 底 + 焦点环 1.5px」 | `SELECT_RING_W = 2.0`；且没有独立焦点环（选中即唯一一档） | 代码 `instances.rs:30`；基线 `selected`、`cm-parallel-select-a/b`、`mm-rel-select-arrow` | 同上，且与 §2.5 的 G2 同源 |
| **I3** | `docs/28` §4.5「悬停 120ms；选中 **140ms**」 | 悬停 `FAST = 0.12` ✓；选中 `BASE = 0.18` = **180ms** ✗ | 代码 `canvas/consts.rs:4-5`、`mw-core/src/design.rs:737/739`；无能量时长的基线（金样只截终态） | 属设计系统令牌问题：140 是否也管画布选中环，需合同侧澄清；改令牌会牵动所有用 `BASE` 的过渡 |
| **I4** | `docs/28` §4.6「焦点环：1.5px 强调色 **80%**，外偏移 1px」 | `ui_kit::paint_focus_ring` 用 `ch.accent`（不透明）；80% 只定义在 `content::focus::ALPHA` 里，`ui_kit` 没读它 | 代码 `ui_kit.rs:240`；基线 `inspector`、`outline-row-menu`（键盘态未专项拍） | 检查器全部控件 + 大纲行菜单；像素差很小（alpha 80% vs 100%），但对比度门（SE-11）与「焦点环 ≥3:1」都读这条 |

### 3.2 B 类：合同缺口（文档没定义，X27 需要补）

| ID | 缺口 | 现状 | 建议（交给 Lead 拍板） |
| --- | --- | --- | --- |
| **G1** | 画布上「主选 vs 非主选」怎么画，没有任何文档写过 | 全画布所有选中节点同一档（2px / 8%） | 二选一：① 补合同（主选 2px + 8%、非主选 1.5px + 5%，与大纲 7%/5% 同构）；② 明确「画布不区分主选」，并在 `docs/30` 写清理由 |
| **G2** | 画布「键盘焦点」没有任何视觉定义（`docs/28` §4.6 与 `docs/30` §5.3 都只对内容表面/概念节点说话） | 画布无环，只有 `F6` 交键盘时的状态栏字样 | 补一条：画布握键盘时状态栏写 `键盘：画布`（零像素风险），或画 1.5px 视口内环（需新基线） |
| **G3** | 检查器多选时主选身份不可见、也不可切换 | 副行固定「N 个节点」 | 补一条：对象行右侧加「主选：<标题>」副行，或允许点对象行循环主选；两者都要动检查器布局，属 X28 范围 |

---

## 4. 验收场景草案（每格怎么判）

统一纪律：**hover / 点击必须走 `MW_GOLDEN_INPUT` 脚本**；时长类状态用「帧数」不用墙钟
（`M16` 已把金样改成帧计数虚拟时钟）；每张新基线都要 `--tol 0` 双跑。

| 格 | 脚本要点 | 判据（像素或轨迹） |
| --- | --- | --- |
| 画布悬停 | `hover <节点中心>` → `wait 8`（120ms @60fps ≈ 7.2 帧） | 节点盒内取样点强调色 alpha ≈ 4%；盒边外 0..2px 带内强调像素**宽度 = 1.5px**（与选中态的 2px 带可比）|
| 画布悬停（时序） | 同一脚本出两张：`wait 3` 与 `wait 8` | 3 帧那张底/环强度应明显低于 8 帧那张（证明 120ms 淡入，不是瞬切） |
| 画布选中 | `click <节点>` → `wait 12`（180ms） | 8% 底 + 2px 环；与悬停那张的差应只在「环宽 +0.5px、底 +4pp」 |
| 画布主选 | 视 G1 裁决：若按建议 ①，`click A` → `Shift+click B` → `click B`（主选挪到 B） | 像素差只落在 A/B 两圈环上；轨迹应有 `select primary=<id> count=2`（**新轨迹，未实现**） |
| 画布多选 | `click A` → `Shift+click B` → `Shift+click C` | 三圈环都在；状态栏文本 `已选 3 个主题`；再 `Shift+click B` 撤下后只剩两圈（`docs/30` §5.12 的「撤下」口径） |
| 画布键盘焦点 | `key F6`（交面板）→ `key F6`（收回） | 轨迹 `canvas_focus owner=canvas\|panel`（**未实现**）；状态栏字样切换 |
| 画布编辑 | `key F2`（进编辑）→ `wait 6` | 环心在盒外 `SELECT_RING_GAP + ring_w×0.5 = 3.5`（css px）、宽 2px 不透明强调色；同一节点的选中态环心在 2.5 —— 两张的相对位置差就是判据 |
| 画布禁用 | 无选中右键 → 菜单截图 | 禁用行文字/图标走在 `ch.icon_off`；与启用行比，覆盖率下降但行高不变（防「禁用=隐藏」） |
| 画布非法拖放 | 拖节点到自身/后代 `press/move/release` | 轨迹 `drop_zone … illegal=true`；落点线颜色 = warn 色（红），**不是**强调色 |
| 大纲悬停 | `hover <行>` → `wait 8` | 行底 4% 主色；握把像素在 8 帧出现、3 帧不出现（`GRIP_DELAY=120ms`） |
| 大纲选中 | `click <行>` | 7% 强调底 + 圆角 6，无边框 |
| 大纲主选/多选 | `click A` → `Shift+click B` | A=7%、B=5%（差 2pp 强调色，可数像素）；多选时行菜单 5 项 |
| 大纲键盘焦点 | `key F6` 进大纲后**不点鼠标** | 目前**判不了**（无环）→ 与 G2/I5 一起补；先立轨迹 `outline_focus row=<id> source=key` |
| 大纲编辑 | `key F2` | 3% 强调底 + 握把消失 + 光标可见 |
| 大纲禁用 | 根主题行右键 | 「剪切/删除」禁用（不可点、色降档） |
| 大纲非法拖放 | 拖到自身子树 | 轨迹 `outline_drag_illegal`（**已有**）；插入线 warn 色；光标 NoDrop（截图看不到，靠轨迹取证） |
| 检查器悬停 | `hover <色板格/stepper>` | 悬停底 `ch.hover` 出现 |
| 检查器多选 | `MW_SELECT_DEMO=multi` | 副行「3 个节点」+ 作用域两段（已有基线 `inspector-multiselect`） |
| 检查器键盘焦点 | `Tab` 进入，不点鼠标 | 1.5px 环（当前 100% 不透明；若按 I4 改 80%，像素会小一档） |
| 检查器编辑 | stepper 点一下 / `MW_STEPPER_EDIT` | 编辑格出草稿 + `Enter` 提交、`Esc` 取消（已有基线 `inspector-stepper-edit`） |
| 检查器禁用 | 无选中 / 已覆盖 / 失败 | 空状态一句话（`inspector-empty`）、`↺` 圆点、28px 错误条（`inspector-error`） |

---

## 5. 预计 golden 场景名单（只列名字，本包不拍）

**新增（10）**：`mm-node-hover`、`mm-node-hover-early`（120ms 时序的早帧）、`mm-multi-primary`（等 G1）、
`mm-node-edit-ring`、`mm-drag-illegal`、`outline-multi-primary`、`outline-key-focus`（等 G2）、
`outline-drag-illegal`、`menu-disabled`、`inspector-focus-ring`。

**复用已有（17）**：`selected`、`outline-hover-select`、`mm-toggle-hover`、`mm-summary-hover`、
`mm-boundary-hover`、`mm-rel-select-arrow`、`mm-rel-select-delete`、`mm-summary-select`、
`mm-boundary-select`、`cm-rel-hover`、`cm-parallel-select-a`、`cm-parallel-select-b`、
`inspector`、`inspector-multiselect`、`inspector-empty`、`inspector-stepper-edit`、`inspector-error`、
`shortcuts-default`、`shortcuts-disabled`（尾两项为「禁用」态的证据）。

> 登记门、拍基线、跑双跑都**不在本包**：本轮只交表与判据。

---

## 6. 未做 / 风险

### 6.1 未做（如实记账）

1. **代码零改动**：3 个缺失格与 4 条不一致全部只记录，未落码（本轮不许改 `crates/`）。
2. **金样零拍摄**：§5 的 10 个新场景一个都没拍，也没登记 `scripts/`；因此本表所有「像素判据」都还没被实拍验证过。
3. **无 cargo / 无门**：未跑 `fmt` / `clippy` / `test` / `golden`，两道门状态未变。
4. **概念图节点悬停/选中无专项基线**：I1/I2 只能靠代码常量判定，缺「概念节点悬停」这张图（现有 `cm-rel-hover` 是关系线）。
5. **深色主题未核**：`docs/28` §4.3 的 `state.hover` 暗色 6%、`state.selected` 暗色 12% 与代码 `with_alpha(text, a.hover)` / `with_alpha(accent, a.selected)` 的对应关系只按亮色口径核过一遍。
6. **其他表面的状态未进表**：卡片页（`card-selection`）、笔记库（`library-select`）、起始页（`start-missing` 的缺失态）各有自己的状态，待 X28/X36/X37 引用本表时再扩列。
7. **`M7b` 套索/框选的「预览中」态**只在画布内属于选中预览，未单列一行（它属于「拖拽中」，`docs/30` §5.13 口径另管）。

### 6.2 风险

| ID | 等级 | 内容 | 缓解 |
| --- | --- | --- | --- |
| R1 | 高 | 并行会话正在改 `crates/`（`mw-io`、画布底色），本表引用的行号与常量随时会漂 | 每格同时给「那一行的字面」；X28 前重新用 grep 复核一次常量值 |
| R2 | 中 | I1–I4 属外观改动，改一条要重拍所有含悬停/选中/焦点的基线 | 先让 Lead 裁决「改码 or 改文档」，再按 `docs/29` §5 批量重拍 |
| R3 | 中 | G1/G2 若补新视觉，会新增状态档 → 需要新轨迹 + 新基线，可能推高 stage-b 场次 | 优先选零像素方案（状态栏文字/轨迹） |
| R4 | 中 | 时长类判据（120ms / 180ms）只能靠「两张早帧对照」，金样不测动画曲线 | 只判「早帧明显弱于终帧」，不承诺精确曲线 |
| R5 | 低 | 大纲键盘焦点「判不了」会被误读成「已实现」 | 本表与 §3 已写明缺失；引用方须带这句限制 |
| R6 | 低 | 检查器「主选不可见」在多选批量编辑时可能让用户误改错对象 | 待 G3 决定；本轮仅在表里标「部分」 |

---

## 7. 引用与写作边界

- 本包只写 `shots/x100/X27/`；未改 `crates/`、`docs/`、`scripts/`、`docs/STATUS.md`、其他 `shots/x100/*`；未 commit。
- 只读命令：`grep` / `sed -n` / `ls`；未执行任何 `cargo` 或 golden 命令。
- 证据条目名一律指 `scripts/x100-stage-b-gate.ps1` / `scripts/x100-stage-cm-gate.ps1` 里登记的
  `Name = '…'`（基线目录同名），代码位置指文件 + 那一行的字面。
