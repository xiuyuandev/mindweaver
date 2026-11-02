# CM-18 · 概念分组（`groups` 顶层键）

> 建立：2026-11-01 · 状态：**已实现**（单测绿；基线未拍，不记 `已验证`）
> 触发：CM-15 复审第 ⑰ 项 → 用户 2026-11-01 拍板「批准分组 v1」。
> 合同：[`docs/30`](../../../docs/30-外壳体验与概念图设计规范.md) §5.5.1；字段论证：[`docs/32`](../../../docs/32-概念图数据结构提案.md) §13。

## 1. 为什么必须动格式

CmapTools 的分组是「把若干概念框在一起」。现有三种东西都替代不了：

| 候选 | 为什么不行 |
| --- | --- |
| `Boundary` | 只圈「同一父级的直接子节点」；概念图里所有概念都是浮点根 |
| `Node.parent_id` | 那是树语义，会推翻「概念图不用 parent/children 表达关系」 |
| 某个概念的 `style` | 组是**多个概念**之间的关系，不是单节点属性 |

所以按 `docs/30` §5.2 的流程先拍板，再记**第二个字段例外**：顶层键 `groups`。

## 2. 交付

### 2.1 数据

```rust
pub struct ConceptGroup { id, label, members: Vec<String>, color: Option<String> }
// Document.groups / FileDoc.groups（顶层；空数组不写，缺键 = 没有分组）
```

- **组框矩形不进文件**：由「成员包围盒 + `GROUP_PAD`(24)」在 `Scene` 里算。
  派生量写进文件会和成员位置打架（成员一动，存下来的矩形就过期）。
- 不升 `FORMAT_VERSION`、不做迁移；旧版本读新文件只会忽略这个键。

### 2.2 命令（各一条撤销）

| 命令 | 守卫 |
| --- | --- |
| `CreateGroup { members, label }` | 仅概念图；成员 ≥ 2；成员已属别组 → 先从旧组摘掉；摘完只剩 1 个成员的组**整组消失**（`validate` 要求 ≥ 2）；空标签回落「分组」 |
| `DeleteGroup { id }` | 解组：只删框，概念一个不删 |
| `SetGroupLabel { id, label }` | 空串回落「分组」 |
| `SetGroupColor { id, color }` | 颜色格式校验同关系线 |
| `MoveGroup { id, dx, dy }` | 只挪「浮点且有坐标」的成员；一条命令一次撤销 |

`ChangeSet` 新增 `before_groups` / `after_groups`（与 `cards` 同款整份快照），
`DeleteNodes` 顺手把被删概念从 `members` 摘掉并快照分组——撤销时组和概念一起回来。

### 2.3 场景与绘制

- `SceneGroup { id, label, rect, color, label_at }`；`Scene.groups` 只在概念图里非空。
- 画法：圆角 12、1.5px、6/4 虚线（与关系线虚线同一节奏）、6% 淡底；
  名字 12px 画在**框上沿外侧**（框里是概念，字进去会压盒子）。
- 导出三处同源：wgpu（`rect_dashed`）、SVG（`rect_elt` + `stroke-dasharray`）、
  PDF（`fill_round` + `q…[6 4] 0 d…Q`）。

### 2.4 交互

- **建组**：概念多选菜单首行「创建分组」。
- **点框线**：选中组内全部概念（复用现有多选拖动，**不新增拖动类型**）。
  命中只认框线（`BAND`），框内空白仍归画布——否则框选会被吃掉。
- **右键框线**：「选中组内概念 / 解组（保留概念）/ 分组颜色（5 色）」。
- 悬停框线**不给**反馈（不抢概念的悬停提示）。

## 3. 验收

新增场景 `cm-group`（`MW_CONCEPT_GROUP="用户痛点|增长策略"`）：两颗概念被圆角虚线框圈住，
框上方写「分组 1」。夹具走「选中两颗 → 创建分组」这条真实路径。

**基线未拍**：本机 `cargo golden` 拿不到 wgpu 适配器（同 CM-14 §4.3）。
另有 `cm-multi-menu` / `cm-submenu-bottom` 因多选菜单多一行而漂（见 `docs/STATUS.md` §6 第 11 条）。

## 4. 证据

| 用例 | 钉住 | 结果 |
| --- | --- | --- |
| `groups_wrap_members_and_ungroup_keeps_the_concepts`（core） | 空标签回落、一个概念只属一组（旧组摘完只剩 1 人则整组消失）、解组不删概念、撤销还回来 | ok |
| `deleting_a_concept_prunes_groups_and_drops_the_ones_left_too_small`（core） | 删概念后组被剪枝；撤销把组一起还回来 | ok |
| `moving_a_group_moves_every_member_in_one_step`（core） | 整组平移只占**一条**撤销，成员同步位移 | ok |
| `groups_are_concept_map_only`（core） | 思维导图里建组被拒 | ok |
| `a_group_box_wraps_its_members_with_padding`（scene） | 组框包住每个成员、空标签回落、名字在框外 | ok |
| `groups_are_written_only_when_present`（mw-format） | 没有分组时整个键不写；有则写 `groups`/`members` | ok |

实跑：

- `cargo test -p mw-core --lib -- --test-threads=1` → **336 passed / 0 failed**
- `cargo test -p mw-format --lib -- --test-threads=1 groups` → **1 passed**
- `cargo check -p mw-shell --all-targets` → 通过
- clippy / fmt：改动文件无新增（见 §5 的环境说明）

## 5. 未做与风险

- **没拍基线**（环境限制）：`cm-group` 待 `-Update`；`cm-multi-menu`、`cm-submenu-bottom` 一并重拍。
- **重命名没做**：v1 名字只有「分组 N」。加行内改名要再引一个编辑态（`GroupEdit`）+ 双击手势，
  单独一轮做，别夹在这批里。
- **拖成员进出组**没做：靠「建组时摘旧组 + 删概念时剪枝」维持「一个概念只属一组」。
- **组框手动缩放**没做：矩形是派生的，要支持缩放得引入 `rect` 覆盖（`docs/32` §13 已列为下一版）。
- **嵌套组**没做。
- 组框颜色只有菜单里 5 档，检查器没有分组页（没有分组选中道）。
