# C14 · 概念图语义收口（树入口封堵 / 命题可见 / 平行边）

> 设计依据：`docs/30` §5（概念图不是树：`parent_id = None` + `pos`，连接走 `relationships`）、
> `docs/32` §3（`Relationship.head` 四档）、`docs/30` §4（检查器分组/行/键盘规格）。
> 目标就一句话：**概念图里不许再漏出树语义，且概念自己能看到自己的命题。**

## 1. 范围（只改这五个文件）

| 文件 | 为什么在这一轮 |
| --- | --- |
| `crates/mw-core/src/document.rs` | 命令层的语义护栏（`MoveNode` / `AddNode` / `InsertSubtree` / `AddLinkedConcept`）、平行边、空短语 |
| `crates/mw-core/src/command.rs` | 只改文档注释：把「概念图里这些命令会被拒」写在命令自己身上 |
| `crates/mw-core/src/concept_layout.rs` | 只加注释：布局期去重 ≠ 数据期去重 |
| `crates/mw-shell/src/panels/inspector.rs` | 命题行/删除位的小工具、`Action` 复用 |
| `crates/mw-shell/src/panels/inspector/inspector_methods_1.rs` | 内容页「命题」分组、关系线页的数字键与空短语提示 |

**不动**：`canvas*.rs`、`app_methods_*.rs`、`main.rs`、`session.rs`、`mw-format/*`、`mw-render/*`、
`mw-core/{theme,scene,model}.rs`（各属别的队友 / lead）。

## 2. 非目标

- **不加字段**：不碰 `Node` / `Relationship` / `LayoutSettings` / `.zmind`，不升 `FORMAT_VERSION`。
  本轮所有改动都能被旧文件读出（旧文件只是没有「一个概念对多条边」这种数据而已）。
- 不重拍 golden：命题分组只在**概念图 + 概念选中 + 内容页**出现，默认基线场景（思维导图检查器）不受影响；
  真要看图得新开场景，属另一包。
- 不改大纲/导出把概念图展开成树的规则（`concept_tree.rs` 是只读消费端，本轮不动）。
- 不做「平行边在画布上错开绘制」——那是 `scene.rs`（lead 的范围），本轮只在 §7 记账。

## 3. 六条修法与取舍

### ① `MoveNode` 没有概念图护栏（最脏的一条）

现状：`Command::MoveNode` 把 `n.parent_id = Some(new_parent)`、`n.pos = None`
（`document.rs` 那句 `n.pos = None; // 挂回树内后不再是自由主题`）。概念图里跑一次，概念就变成树节点，
位置当场丢掉——而 `validate()` 的概念图分支只允许「根 + 自由概念」有 `pos`，所以这是**数据层静默降级**。
mw-ui 那条路径今天恰好被挡住，但壳层 `OA::Move` / `OA::MoveMany`（大纲拖动）是无条件下发的。

修法：`MoveNode` 在概念图里直接 `Err(CoreError::Invalid("概念图里概念不能挂成树节点"))`，
放在所有前置检查之前（先判文档类型，再判 id 存在），保证「文档没变、撤销栈没动」。

**同类会写 `parent_id = Some(..)` 的入口一起封**：
- `AddNode`：概念图里拒（见下）；
- `InsertSubtree`：概念图里拒，错误文案「概念图里不能粘贴子树」（粘贴概念图必须走 `InsertConcepts`）。

`AddNode` 的取舍——**拒绝，而不是「强行改成独立概念」**：
`AddNode` 的载荷里只有 `parent`，**没有 `pos`**。要强制成独立概念就必须凭空编一个坐标
（叠在父概念身上 / 塞到 (0,0)），那是替用户做了一个他没法撤销的排版决定；而且「忽略 `parent` 参数」会让
同一个命令在两种文档里语义不同，比报错更难解释。概念图本来就有两条对的入口：
`AddFloatingTopic`（自由摆放）与 `AddLinkedConcept`（建概念同时连线），壳层要新建概念走这两条。
错误文案「概念图里概念不能挂成树节点（用 AddFloatingTopic / AddLinkedConcept）」直接把下一步告诉调用方。

### ② `AddLinkedConcept` 没有自己的概念图护栏

现状：两个调用点（`canvas_methods_3.rs` 拖到空白、`canvas_methods_1.rs` 大纲 Tab）都在
`if !self.concept_map() { return … }` 之后，靠调用点自觉。命令层谁都能发，思维导图里发一次就造出
「`parent_id = None` 的浮点节点 + 一条边」——`validate()` 的思维导图分支只允许「自由主题或根」有 `pos`，
结果是**命令成功、文档立刻不自洽**。

修法：`AddLinkedConcept` 开头 `if !self.is_concept_map() { return Err(Invalid("只有概念图可以新建概念并连线")) }`。
`RetargetLinkToNewConcept` 是同一类（也是「建浮点概念 + 改边」），一并加同一护栏。

### ③ 同一对概念之间只许一条边

现状：`AddRelationship` 与 `SetRelationshipTarget` 各有一处
`any(|r| &r.to == to)` → `Invalid("两节点之间已存在关系线")`。这条是从树语义里带出来的（一棵树里两个节点只有一条路），
直接把「A 影响 B」+「A 包含 B」判死，与 CmapTools 口径相反。

修法：**概念图里删掉这两处检查，思维导图里留着**。
- 自连仍然禁（`from == to` →「不能把节点连接到自身」），这条与树无关，是自反命题的脏数据；
- 选中/编辑本来就按 `rel.id` 定位（`RelationshipSel { from, id }`），不依赖 `(from,to)` 唯一；
- **为什么思维导图保留**：`scene.rs` 的 `bow_crossings` 只对「真交叉且夹角 ≥12°」的线段换弯法，
  两条同起终点的边是近平行 + 端点相接 → 都不弯，画出来**完全重合**。思维导图里用户拖一条线到
  已连过的节点，会得到一条看不见的重复线：删掉看得见那条，线还在。这是明确的回退，不能放。
  概念图里平行边的语义是「两条不同的命题」，重合是绘制层欠的账（§7 记账），不是数据层的错。

### ④ 概念看不到自己的命题

现状：检查器只有选中**关系线**时才有关系页；选中概念时，出去的/进来的命题一条都看不见——
概念图用户必须靠画布上的线认字。

修法：概念图里选中**概念节点**（`parent_id == None`）时，「内容」页末尾新增「命题」分组
（垫在最后，不动 §4.1 给思维导图定的组序「文本内容 → 任务 → 标签 → 链接 → 卡片 → 图片」），两段：
- 出去的：`字形 短语 目标概念`，字形就是 `head` 的方向（`—`/`→`/`←`/`↔`）；
- 进来的：同样格式，第三段是**源概念**——边挂在源节点上，所以进来的行「宿主」与「另一端」是同一个 id；
- 箭头语义（`head.label()`，如「双向」）与整句读法放在**悬停说明**里：行宽只有 212，塞不下；
- 行点一下 = 选中这条关系线（复用 `Action::SelectRelationship { from, id }`，壳层已有落地）；
- 行尾 `×` = `Command::DeleteRelationship { from, id }`，一次撤销；
- 空态一句话：「还没有命题：从这颗概念拖线到另一颗概念，就是一条命题」；
- 分组摘要写 `出 n · 入 m`，默认展开（概念图里这是主信息，不该再点一下才看得见）；
- 行高走 `content_row`（28，§4.1），文字 12.5 / 摘要 11.5，颜色全用现有令牌（`text` / `dim`），
  **不引入新字号**；短语/概念名分别按 5 / 6 个汉字截断（`clip()`），超长部分进悬停——
  不截断的 Label 会把内容最小宽顶出去（SE-05 修过的「288 → 316 且每帧 +2」那类回归）；
- 行序按「另一端名字 + 短语」排序：`doc.nodes` 是 `HashMap`，迭代序不稳定，不排序行位与金样都不可复现。

### ⑤ 空短语留下一条没有短语的线

现状：关系线页把标签清空 → `SetRelationshipLabel { label: None }` → 线上没有连接短语。
概念图里一条「没有短语的线」不构成命题（CmapTools 里也没有这种东西），它就是「线团」的来源。

修法（**概念图里拒绝清空，但绝不堵新建**）：
- `document.rs`：概念图的 `SetRelationshipLabel { label: None }`（或全空白）→
  `Invalid("概念图的连接短语必填")`。这是数据层的硬口径，谁发都一样；
- `inspector_methods_1.rs`：关系线页在概念图里把输入框 hint 改成「连接短语（必填）」，
  草稿为空时不发命令、就地落回原短语，并在框下补一行 12 浅字提示——
  「就地」比「发出去被拒再弹 toast」少一次往返；
- **新建流程不受影响**：`AddRelationship` / `AddLinkedConcept` 建线时 `label: None` 照旧合法
  （拖出新线 → 立刻立起短语编辑框，用户当场打字），本轮不碰这条路径。
  换句话说：**建的时候可以没有短语，改的时候不能把它删掉**。

### ⑥ 关系线的箭头没有键盘路径

现状：箭头四档（无 / 正向 / 反向 / 双向）只能鼠标点，「选中线 → 想改箭头」对纯键盘用户是死路。

修法：关系线页的四格支持数字键 `1`–`4`（`1` 无、`2` 正向、`3` 反向、`4` 双向，与四格从左到右同序），
走 `Action::Apply(Command::SetRelationshipHead{..})`，一次撤销一格。
- 只在「关系线页在屏上」且文本框没吃键盘时生效（`ui.ctx().egui_wants_keyboard_input()` 为假），
  不会抢短语输入；
- 数字键在壳层是空闲的：画布只吃 `Ctrl/Alt + 数字`（`canvas_methods_3.rs` 的 `cmd`/`alt` 分支），
  开始页的 1–6 只在起始页存在；
- tooltip 里写明快捷键（「无箭头 · 数字键 1」…），鼠标用户也看得见。

## 4. 验收场景（本轮全部是单测，不拍图）

`cargo test -p mw-core`（`TEMP`/`TMP` 指到 `target/tmp`）：

1. `move_node_is_rejected_in_concept_maps`：概念图里 `MoveNode` 返回 `Invalid`，
   **文档字节级不变**（`canon` 前后一致）、`undo_depth` 不变、`revision` 不变。
2. `add_node_is_rejected_in_concept_maps`：概念图里 `AddNode{parent: 根}` 被拒，文档不变；
   思维导图里同一命令照旧成功（钉住「只拦概念图」）。
3. `insert_subtree_is_rejected_in_concept_maps`：概念图里粘贴子树被拒，概念要粘贴得走 `InsertConcepts`。
4. `add_linked_concept_is_concept_map_only`：思维导图里 `AddLinkedConcept` 返回 `Invalid`，文档不变；
   概念图里同一条命令成功且一次撤销（已有用例保留）。
5. `parallel_propositions_between_the_same_pair_are_independent`：概念图里 A→B 建两条不同短语的边
   都成功；删掉其中一条，另一条还在、短语没变；撤销一次回到两条（一次撤销一条边）。
6. `self_link_is_still_rejected`：概念图与思维导图里 `from == to` 都仍是「不能把节点连接到自身」。
7. `clearing_the_phrase_is_rejected_in_concept_maps`：概念图里把短语清成 `None`/全空白被拒，
   文档不变；思维导图里清空照旧成功（思维导图的线本来就可以没有短语）。

`mw-shell` 是**纯 bin crate**（没有 lib target），命令是
`TEMP=$(pwd)/target/tmp TMP=$(pwd)/target/tmp cargo test -p mw-shell --bins -- --test-threads=1`：

8. `concept_maps_show_their_own_propositions`：概念图 + 选中概念，内容页画一帧，断言画出来的文字里有
   「命题」标题、摘要 `出 1 · 入 1`、出去的行 `→ 导致 乙`、进来的行 `→ 支撑 丙`；
   同一夹具换成思维导图后**一帧里没有「命题」二字**（概念图语义不许漏进树）。
9. `digit_keys_pick_the_arrow_head`：关系线页按 `3` 出
   `Action::Apply(Command::SetRelationshipHead { head: Backward })`，`1` → `Off`、`4` → `Both`；
   按当前那一档（`2`，默认正向）**不发命令**（钉住「不重发空命令」）。

## 5. 证据（实测，2026-10-30）

```
cargo build -p mw-core                      → 绿
cargo test  -p mw-core                      → 248 passed; 0 failed（含本轮新增 7 条）
                                               + tests/contrast.rs 7 passed; 0 failed
cargo build -p mw-shell                     → 绿
cargo test  -p mw-shell --bins -- --test-threads=1
                                            → 588 passed; 0 failed; 1 ignored
                                               （含本轮新增 2 条）
```

- 每条修法在 §3 里能对上「函数名 + 一句话」；
- 不新增字段、不升 `FORMAT_VERSION`：`git diff` 里 `model.rs` / `mw-format` 零改动；
- 本轮动了 5 个文件，另加 1 处 `Action` 的 `#[derive(Debug)]`（见 §7 第 4 条）。

## 6. 数据模型核对

| 能力 | 承载 | 结论 |
| --- | --- | --- |
| 多对多关系 | 已有 `Node.relationships: Vec<Relationship>`（边挂在源节点上，天然 M:N） | 有，不需要字段 |
| 平行边 | 同上，`Relationship.id` 唯一即可 | 有，只删检查 |
| 箭头四档 | `Relationship.head: ArrowHead`（已批准，默认 `Forward`） | 有 |
| 命题列表 | 由 `relationships` 现算，不落盘 | 有 |

**不需要任何新字段**，P0 风险不触发。

## 7. 未做 / 风险（必须让 lead 知道的四条跨范围账）

1. **平行边在画面上完全重合**（`scene.rs` / `mw-render`，lead 的范围）：
   `bow_crossings` 的 `segments_cross` 对「端点相接」和「夹角 <12° 的近平行」都返回 false，
   所以一对概念之间的两条边是两条一模一样的直线，两枚短语标签叠在同一个中点。
   数据层放开后，概念图里「A 影响 B」+「A 包含 B」会读不出来。**建议**：在 `scene.rs` 的
   `relationships` 构建后按 `(from, to)` 分组，给同组第 k 条边一个 `k * 12px` 的法向偏移
   （或直接复用 `bowed_between` 的两档弯法）。这条不在我的写入范围，本轮只记账。
2. **大纲拖拽在概念图里会弹「修改未生效」**（`app_methods_2.rs` 的 `OA::Move` / `OA::MoveMany`，第三个队友的范围）：
   `MoveNode` 加了护栏之后，概念图里在大纲上拖行会稳定失败并弹 toast。理想修法是壳层在概念图里
   不发 `MoveNode`（概念的「上下移」没有语义，要么禁用这条手势，要么改走 `MoveConcept` 的排序语义）。
   数据层现在**至少不会把概念悄悄变成树节点**——这是本轮要保证的底线。
3. **画布上「把短语清空」现在会走失败分支、但提示是通用的**（`canvas_methods_3.rs:710` 的 `rel_edit` 提交、
   `canvas_methods_1.rs:1188` 的 `set_concept_phrase`，都是别人的范围）：
   这两处提交时都用 `(!trimmed.is_empty()).then(|| …)`，概念图里清空就变成 `SetRelationshipLabel { label: None }`
   → 现在会被核心层拒（`Invalid("概念图的连接短语必填")`）。壳层 `main.rs` 的 `IA::Apply` 失败只显示
   「修改未生效：这条命令被文档拒绝了」，用户看不出是「短语必填」。
   **建议**：这两处在概念图里若 `trimmed.is_empty()` 就**不发命令**（跟检查器关系线页同一个做法），
   或在 `main.rs` 里把 `CoreError::Invalid` 的原文挂到错误条上。
   检查器一侧已经做到「就地落回、不发命令、框下补一行『连接短语必填』」，没有这个问题。
4. **`Action` 补了 `#[derive(Debug)]`**（`panels/inspector.rs`，我的范围）：
   新单测要断言「这一帧发了哪条命令」，失败信息里得能打出 `Action`；不给 `Debug` 时
   `cargo build` 是绿的、**只有 `cargo test` 目标红**（`inspector.rs` 新测试里的 `{other:?}`）。
   变体里只有 `Command` / `String` / `usize` / `bool`，都能派生，没有引入 `Debug` 实现负担。

另：真机输入脚本、golden 一律不做（命题分组不进默认基线），机器门与 `docs/STATUS.md` 数字由 lead 跑。

## 8. 执行记录（改到别人既有用例的部分，必须记账）

1. **三条 mw-core 既有用例按新语义重写夹具**（不是放宽断言，断言反而更严）：
   - `add_linked_concept_undoes_in_one_step`、`retarget_link_to_new_concept_undoes_in_one_step`：
     原来在思维导图文档里跑 `AddLinkedConcept` / `RetargetLinkToNewConcept`，现在这两条命令被
     「只有概念图可以…」挡住，夹具改成 `concept_doc(&[("根", 0.0, 0.0)])`——**测的仍然是「一次撤销复原全部」**；
   - `move_concept_only_works_in_concept_maps`：原来用 `add()` 在概念图里建一棵带父级的子节点
     （`AddNode` 现在被拒），改成先在临时思维导图文档里建好子节点、再切到概念图；
     保留的断言仍是 `MoveConcept` 在思维导图里被拒。
2. **命令纠正**：`mw-shell` 是纯 bin crate，没有 lib target——`cargo test -p mw-shell --lib` 会直接
   `error: no library targets found`。正确口径是
   `TEMP=$(pwd)/target/tmp TMP=$(pwd)/target/tmp cargo test -p mw-shell --bins -- --test-threads=1`。
3. **并行跑会抖**：`cargo test -p mw-shell --bins` 用默认多线程时，`canvas::tests` 两条与
   `sr02_tests::autosave_backoff_grows_and_caps` 偶发失败，**单独跑与 `--test-threads=1` 全跑都过**，
   与本轮改动无关（不在我的写入范围，记在这里供 lead 参考）。
