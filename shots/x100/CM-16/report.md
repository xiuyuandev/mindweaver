# CM-16 · 关系线线型、线宽与命题注释

> 建立：2026-11-01 · 状态：**已实现**（单测绿；基线未拍，不记 `已验证`）
> 触发：用户 2026-11-01 追问「你确定概念图该有的功能都有？」→ CM-15 复审列出 8 项缺口 →
> 用户拍板「先做线型 + 线注释」。
> 设计合同：[`docs/30`](../../../docs/30-外壳体验与概念图设计规范.md) §5.4.3；
> 字段论证：[`docs/32`](../../../docs/32-概念图数据结构提案.md) §12.1 / §12.2。

## 1. 为什么这两项要动数据

`Relationship` 原先只有 `id / to / label / color / head`。线型（实/虚）、线宽、注释三样
都是**每条命题自己的属性**，画在屏上而不落盘等于「导出就丢」——用户没法交付。
所以这次必须加字段，也就必须先改 `docs/30` §5.2 那条「禁止新增字段」的硬约束：
**用户拍板后**在 §5.2 记了例外（三个可选键），字段方案与兼容性写在 `docs/32` §12。

## 2. 交付

### 2.1 数据与命令（`mw-core`）

| 字段 | 类型 | 默认 | 落盘 |
| --- | --- | --- | --- |
| `Relationship.dashed` | `bool` | `false` | 默认不写（`skip_serializing_if`） |
| `Relationship.width` | `Option<f32>`（css px，0.5–6.0） | `None` = 跟随主题 1.5 | 默认不写 |
| `Relationship.note` | `Option<String>`（上限 `MAX_NOTE_LEN`） | `None` | 默认不写 |

三条命令，各占一条撤销记录：

- `SetRelationshipDashed { from, id, dashed }`，撤销文案「改为虚线 / 改为实线」；
- `SetRelationshipWidth { from, id, width }`，撤销文案「修改线宽 / 恢复默认线宽」；
- `SetRelationshipNote { from, id, note }`，撤销文案「编辑命题注释 / 清除命题注释」。

守卫：线宽必须有限且在 `0.5..=6.0`（否则 `Invalid("线宽要在 0.5–6 之间")`）；
注释空串归一成 `None`（与 `SetNote` 同口径）；注释超长复用 `NoteTooLong`。
`ReverseRelationship` 现在把这三个属性**原样带走**（反的是方向，不是「这是不是暂定命题」）。

### 2.2 场景与渲染（`mw-core` / `mw-render`）

- `SceneRelationship` 增加 `dashed: bool` 与 `width: f32`（css px）；`blend_scenes` 插值线宽、
  线型取目标帧的值（离散量不插）。
- 虚线节奏是**世界单位**常量 `REL_DASH = [6.0, 4.0]`（`mw_core::scene`）：`Scene` 必须与缩放无关，
  而虚线是几何——SVG / PDF 照同一节奏画，屏幕与导出才逐段对得上。
- wgpu 侧不把虚线塞进 `Scene`：`push_dashed_curve` 在渲染层把折线切成实线段，
  `Scene` 仍是一条完整曲线（命中判定、包围盒不受影响）。切段逻辑抽成纯函数
  `dash_walk(len, phase, dash) -> (Vec<(f32,f32)>, phase)`，**相位跨折线拐点连续**。

### 2.3 导出

| 格式 | 线宽 | 虚线 |
| --- | --- | --- |
| PNG | `cam.css_to_world(r.width)` | 渲染层切段 |
| SVG | `stroke-width` | `stroke-dasharray="6 4"` |
| PDF | `w` | `[6 4] 0 d`（写在 `q … Q` 里，不污染后面的线） |

### 2.4 界面

- 检查器「关系线」页：**线型**两段（实线 / 虚线）、**线宽**三档（细 1.0 / 标准 / 粗 2.5）、
  **注释**多行框（失焦提交，多行里 Enter 是换行）。
- 右键关系线 →「线型」子菜单（实线 / 虚线，当前档打勾）——不用先打开检查器。
- **画布上不给注释画角标**：命题已经有一条线和一枚短语胶囊，再加一颗点会吃掉
  「线在标签处断开 8px」的留白。注释只在检查器里读。这是刻意取舍，写进 §5.4.3。

## 3. 验收

新增场景 `cm-rel-style`（`MW_CONCEPT_REL_STYLE=带来` + `MW_INSPECTOR=1`）：
「带来」那条线是 2.5 粗的虚线（6/4 节奏），检查器同时显示「虚线 / 粗 / 2026Q3 的数据」。
夹具走三条**真命令**，所以这张图同时锁住数据、`Scene` 与检查器。

**基线未拍**：本机 `cargo golden` 拿不到 wgpu 适配器（同 CM-14 的 §4.3）。
门脚本对缺基线会 `SKIP golden …` 并计数，不会把「没拍」算成通过。

## 4. 证据

| 用例 | 钉住 | 结果 |
| --- | --- | --- |
| `relationship_line_style_and_note_are_undoable_and_normalised`（core） | 三属性各自可撤销、空注释归一成 `None`、越界/NaN 线宽拒绝、退到线宽前虚线仍在 | ok |
| `reverse_relationship_swaps_ends_keeps_head_and_undoes_in_one_step`（core，已扩） | 反向后虚线/注释原样带走 | ok |
| `relationship_line_style_uses_defaults_and_writes_nothing_when_unset`（mw-format） | 默认值四个键都不落盘；缺键读回默认；设了就原样写 | ok |
| `dash_tests::*`（mw-render，4 条） | 短段一段实线、相位跨段连续、长线节拍不漂、退化的 `[0,4]` 不死循环 | ok |
| `a_dashed_relationship_exports_a_dash_array`（shell） | 虚线写进 SVG（`stroke-dasharray="6 4"`） | ok |

实跑：

- `cargo test -p mw-core --lib -- --test-threads=1` → **323 passed / 0 failed**（+1）
- `cargo test -p mw-shell --bin mw-shell -- canvas::` → **135 passed**；`export_svg::` → **14 passed**（+1）
- `cargo test -p mw-render --lib -- dash` → **4 passed**。
  另：`cargo test -p mw-render --lib`（全量）是 **28 passed / 3 failed**，三条失败全是
  `glyph.rs` 的「系统里没有可用的中文字族」——本机（无桌面会话）拿不到 DirectWrite 字体，
  与本次改动无关；`instances.rs` 的 28 条全过。
- `cargo test -p mw-format --lib -- relationship_line_style` → **1 passed**
- `cargo fmt --all -- --check` → 干净
- `cargo clippy -p mw-shell -p mw-core -p mw-render --all-targets` → 改动文件**新增 0**
  （`instances.rs` 的 `items after a test module` 已把测试模块挪到文件末尾修掉）

## 5. 未做与风险

- **没拍基线**（环境限制，见 CM-14 §4.3）：`cm-rel-style` 待 `-Update`；
  `cm-arrow-menu` 因为多了「线型」子菜单会再漂一次，重拍时一并处理。
- **虚线节奏是世界单位**：缩到 20% 时 6 世界单位 ≈ 1.2 css px，看起来接近实线。
  这是为了 `Scene` 与缩放无关、导出与屏幕同源的取舍，已写进 §5.4.3。
- **线宽只给三档**：不做连续滑杆；要加档先改 §5.4.3。
- **注释不进检索**：`Ctrl+F` 不搜注释（只搜标题/备注/链接/命题短语）。要不要搜是产品决定，未做。
- 检查器的「注释」多行框**没有** IME 专项脚本；它与备注框同一套 `TextEdit`，
  但「组合期间失焦提交」这条路径没有实机核对。
