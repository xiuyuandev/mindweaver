# CM-01 概念图数据契约与文档类型 · 报告

> 状态：**已实现**（G-CM-1 门未开；CM-02 的渲染依赖本包的 `kind` 契约）
> 设计合同：`docs/30-外壳体验与概念图设计规范.md` §5.1 §5.2 §5.7 §5.8
> 执行合同：`docs/31-外壳体验与概念图工作包.md` §6
> 开始：2026-10-26

## 1. 范围

确认「概念图不新增字段、不升版本」这条路能走通，并用测试把契约钉死：

- `LayoutSettings.kind = "conceptMap"`（自由字符串，`crates/mw-core/src/model.rs` 已支持）。
- 概念 = 浮动 `Node`（`parent_id = None` + `pos`）。
- 关系 = `Node.relationships`（有向 + `label` + `color`）。
- `.zmind` 往返后 `pos` 与 `relationships` 逐字段相等。
- `validate()` 通过；未知 `kind` 不崩。

## 2. 非目标

- 不改 `FORMAT_VERSION`、不新增字段、不写迁移脚本。
- 不实现概念图画布渲染（CM-02）、连线与短语编辑（CM-03）、整理布局（CM-04）。
- 不改 `Command` 语义（复用 `AddFloatingTopic` / `AddRelationship` / `SetNodePos`）。

## 3. 实现记录

结论：**这条路走得通，一行格式代码都不用改。**

- `LayoutSettings.kind` 本来就是自由字符串，`"conceptMap"` 直接可用（`model.rs` 无枚举约束）。
- 概念节点用 `parent_id = None` + `pos`：`Document::validate()` 已接受「带 `pos` 的自由节点」，
  `AddFloatingTopic` / `SetNodePos` 两条命令承担写入，不需要新命令。
- 关系用 `Node.relationships`：`AddRelationship` 已校验 `from != to`、两端存在、文字长度、
  且拒绝重复连线；`Relationship { id, to, label, color }` 四字段都在 `.zmind` 里。
- `FORMAT_VERSION` 未动、`Node` / `NoteCard` 字段未动、无迁移脚本。

新增测试 `concept_map_roundtrips_positions_and_relationships`（`mw-format`）：

| 断言 | 内容 |
| --- | --- |
| `kind` | 存 `"conceptMap"`，读回来仍是 `"conceptMap"` |
| `pos` | `(-132.0, 47.5)` 往返后逐字段相等 |
| `parent_id` | 仍是 `None`（概念没被挂成子节点） |
| `relationships` | `to` 指向对方、`label = Some("解决")`、`color = None`（不偷偷补默认色） |
| `validate()` | 往返后结构合法 |

## 4. 验收

| 项 | 结果 |
| --- | --- |
| `cargo test -p mw-format concept_map` | 1 passed |
| 未知 `kind` | `LayoutKind::from_kind` 回落 `Mindmap`，不 panic（既有行为，未改） |

## 5. 未做 / 风险

- **`doc_order()` 与 `validate()` 的「单根树」假设**：概念图的浮动概念会被 `doc_order`
  按「根 + 自由主题」的顺序遍历到，本轮实测没有崩；但若将来有人在概念图里
  用 `children` 表达层级，`resolve_all` 的环保护会挡住重复引用，不会死循环。
- 概念图的大纲视图需要「按 `relationships` 生成树且环安全」，属 CM-07，本包不做。
- 没有对 `kind` 做「已知值白名单」校验：未知字符串一律回落思维导图，这是既有行为，
  改它会影响所有旧文档的打开路径，不动。
