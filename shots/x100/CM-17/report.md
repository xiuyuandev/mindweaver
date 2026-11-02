# CM-17 · 多选命题（批量删 / 改箭头 / 改线型 / 改线宽）

> 建立：2026-11-01 · 状态：**已实现**（单测绿；基线未拍，不记 `已验证`）
> 触发：CM-15 复审第 ⑳ 项「多选关系线」；CM-16 之后用户说「继续」，按建议先做这一块。
> 设计合同：[`docs/30`](../../../docs/30-外壳体验与概念图设计规范.md) §5.4.4。

## 1. 为什么这块不用动数据

`Canvas.relationship` 原来是 `Option<RelationshipSel>`——**单数**。所以「删掉这 5 条暂定命题」
只能一条一条删，而且每次删完选中态就没了，得重新找下一条。这是纯选择模型的问题，
`.zmind` 一个字节都不用改。

## 2. 交付

### 2.1 选择模型（`mw-shell/src/canvas.rs`）

```rust
pub struct RelationshipSelection { items: Vec<RelationshipSel> }
```

| 方法 | 语义 |
| --- | --- |
| `select_only(sel)` | 单选：清掉别的 |
| `toggle(sel)` | 加选 / 减选（已选中的再点一次 = 移除） |
| `primary()` | **最后选中的那条**（检查器单条页、右键勾、`F2` 都读它） |
| `contains(from, id)` | 强调绘制用它判断这条线要不要画 3px |
| `retain_alive(f)` | 场景重建后**逐条**清掉失效的线（不能一条不在就把整组带走） |

原来 45 处 `self.relationship` 的用法全部机械迁移：`= None` → `clear()`、`.is_some()` →
`!is_empty()`、`.as_ref()` → `.primary()`、`= Some(x)` → `select_only(x)`。

### 2.2 手势

- `Ctrl` / `Shift` + 点击另一条线 = 加选；再点一次 = 减选。
- 点空白 / 点节点 / 点注解 / `Esc` = 清空（沿用 §5.4 的三道互斥）。
- 加选时**不能先清空再选中**——这是改造里最容易写错的一处：原来的点击分支第一句就是
  `self.relationship.clear()`，加了 `rel_additive` 判断才没把刚选的抹掉。

### 2.3 批量命令（都是**一条 `Batch`，一次撤销**）

| 操作 | 命令 |
| --- | --- |
| 删除 | `Batch[DeleteRelationship × N]` |
| 箭头四档 | `Batch[SetRelationshipHead × N]` |
| 线型（实/虚） | `Batch[SetRelationshipDashed × N]` |
| 线宽三档 | `Batch[SetRelationshipWidth × N]`（画布方法；检查器里同款） |
| 反向 | `Batch[ReverseRelationship × N]`，选中集跟着换主人（`from→to` 变 `to→from`） |

单条时**不发 `Batch`**（直接发那一条），这样撤销文案仍然是「删除关系线」而不是「批量操作」——
`Command::label` 对 `Batch` 取第一条的文案，但少一层嵌套更干净。

### 2.4 界面

- 检查器：选中 ≥2 条时切「**N 条命题**」批量页（箭头 / 线型 / 线宽 / 删除 N 条 / 取消选择）。
  **不画当前值**：多选时各条的值可能不同，画勾会撒谎。
- 右键菜单：「删除 N 条关系线」；`编辑关系线标签` 在多选时禁用（一次改 5 条短语没有意义）。
- 画布：每条选中的线都画 3px 实色强调（与单选同一段绘制，只是集合更大）。

## 3. 验收

新增场景 `cm-rel-multi`（`MW_CONCEPT_MULTI_REL="支撑|带来"` + `MW_INSPECTOR=1`）：
两条线同时 3px 强调，检查器切「2 条命题」批量页。夹具走 `toggle`，锁的是真实加选路径。

**基线未拍**：本机 `cargo golden` 拿不到 wgpu 适配器（同 CM-14 §4.3）。

## 4. 证据

| 用例 | 钉住 | 结果 |
| --- | --- | --- |
| `relationship_selection_toggles_and_keeps_the_last_one_as_primary`（shell） | 单选/加选/减选、主选 = 最后一条、`contains`、`retain_alive` 逐条清理 | ok |
| `batching_relationship_style_is_one_undo`（core） | 三条线批量改线型只占**一条**撤销，一次 `undo` 全退回 | ok |

实跑：

- `cargo test -p mw-core --lib -- --test-threads=1` → **330 passed / 0 failed**
  （比 CM-16 的 323 多 7 条，其中 1 条是本包新增，其余来自并行会话）
- `cargo test -p mw-shell --bin mw-shell -- canvas::` → **136 passed**（+1）
- `cargo clippy -p mw-shell --all-targets` → 改动文件**新增 0**
  （顺手删掉了没人用的 `RelationshipSelection::iter`，并让强调绘制改用 `contains`）
- `cargo fmt` → 本包改动的文件干净（`mw-ui/src/drill_click.rs` 是并行会话正在改的，不碰）

## 5. 未做与风险

- **没拍基线**（环境限制）：`cm-rel-multi` 待 `-Update`。
- **框选命中关系线**没做：橡皮筋现在只框节点。要做需要给 marquee 加「线-矩形相交」判定，
  并且和「框节点」的结果合并——那是另一包。
- **Shift 范围选择**没做（从主选到点击处整段选中）。
- **批量页不显示当前值**：多选时值可能不同，画勾会撒谎；要做得先定义「混合态」的画法。
- 实机手感未核：加选时指针落在 1.5px 宽的线上本来就难，7px 命中环是既有口径，没动。
