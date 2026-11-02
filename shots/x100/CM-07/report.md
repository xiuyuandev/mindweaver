# CM-07 概念图大纲视图与导出 · 报告

> 状态：**已实现**（本包范围；G-CM-2 门未开）
> 设计合同：`docs/30-外壳体验与概念图设计规范.md` §5.6「与笔记联动」大纲一段、§5.8「导出与往返」
> 执行合同：`docs/31-外壳体验与概念图工作包.md` §6（CM-07，规模 L）
> 开工 / 完成：2026-10-26 · 依赖 CM-05（交互已落地）、CM-06（联动已落地）

## 1. 起点（审计结果）

概念图的节点是「浮动概念」——`parent_id = None`，`children` 恒空（`docs/30` §5.2 明确禁止用
`children` 表达概念图层级）。全仓有三处**按 `children` 展平文档**的地方，概念图进去全哑：

| 位置 | 代码 | 概念图下的实际表现 |
| --- | --- | --- |
| 全屏大纲 `panels/outline.rs::rebuild` | 从 `root_id` 起按 `children` 压栈展开，其余浮动节点当「自由主题」平铺 | 根一行 + 其余概念全在 depth 0 平铺，**层级全丢**（命题网被画成一列） |
| `mw-io::export_markdown` | 递归 `node.children` | 只导出主概念一行，**其余概念全丢** |
| `mw-io::export_opml` | 递归 `node.children` | 同上；OPML 里只剩一个 `<outline>` |
| `mw-io::export_outline` / `export_freemind` | 同上 | 同上（这两条不在 `docs/31` CM-07 的交付项里，但同属「导出只有一行」的哑，一并收口） |

PNG/SVG/PDF 三条离屏导出**不需要改**：它们吃 `Scene`（`docs/30` §5.8「零新增导出路径」），
`crates/mw-shell/src/export_pdf.rs` / `export_svg.rs` 里没有任何 `node.children` 遍历，
只有 `scene` 的 `has_children` 画折叠点。审计用 `grep -rn "children" export_*.rs` 确认。

**结论**：缺的不是渲染，是**一张「命题网 → 树」的投影**。这张投影必须只有一个实现，
大纲和四种文本导出共用；否则大纲里看到的层级和导出文件里的层级会各说各话。

## 2. 范围（本包做的四件事）

| # | 缺口 | 处置 | 落点 |
| --- | --- | --- | --- |
| 1 | 没有「命题网 → 生成树」的投影 | 新纯函数模块 `mw_core::concept_tree::concept_rows(doc)`：从 `root_id` 起沿 `relationships` 前序展开，**环取首次访问边**（已访问即跳过，不会再入栈）；没被任何边指到的概念各自成为一棵 depth 0 的根（按 `(y, x, id)` 排，与「左上即阅读顺序」同口径）；非概念图返回空表 | `crates/mw-core/src/concept_tree.rs`（新）、`lib.rs` |
| 2 | 大纲把概念平铺成一列 | `Outline::rebuild` 分叉：概念图走 `concept_rows`，行表带 depth/parent/兄弟下标；折叠仍读 `Node.collapsed`（显示态，不写树语义）；聚焦按生成树的子树截取 | `panels/outline.rs` |
| 3 | 大纲里的树语义按键会往命题网里写 `children` | 概念图下 Enter 改成「新建概念」（`AddFloatingTopic`，落点在被选概念下方 56px），Tab / Shift+Tab / Ctrl+Shift+↑↓ 与拖拽一律不接（trace 记 `outline_tree_key_ignored`） | `panels/outline.rs`、`app_methods_2.rs`、`canvas_methods_1.rs` |
| 4 | 页头/页脚没有「这是概念图」的标注 | 页头面包屑接 `· 概念图 · 按链接层级展开`；页脚把「N 个主题」换成「N 个概念」 | `app_methods_2.rs`、`panels/outline.rs` |
| 5 | 四种文本导出在概念图下只剩主概念 | `export_markdown` / `export_opml` / `export_outline` / `export_freemind` 全部改走同一张生成树；Markdown 文首加 `> 概念图 · 按链接层级展开`，OPML 标题后缀同上，FreeMind 加 XML 注释；**链接短语随行导出**（`{短语} → {概念}`），否则导出的层级丢掉命题语义 | `crates/mw-io/src/export.rs` |

## 3. 验收场景

| 场景 | 输入 | 判词 |
| --- | --- | --- |
| `cm-outline`（golden） | `MW_CONCEPT_DEMO=1 MW_CONCEPT_OUTLINE=1` | 全屏大纲按「用户痛点 → 竞品访谈 → …」展开成多级；页头写 `大纲 · 概念图 · 概念图 · 按链接层级展开`（文档名恰为「概念图」时不重复类型字）；页脚写「N 个概念」；无树线、无分支色块 |
| 生成树单测（core） | 见 §6 | 环不死循环；首次访问边定父；孤立概念当根；树文档返回空 |
| 导出快照（io） | 概念图夹具 | Markdown 文首带标注、层级与短语齐全；OPML 标题带标注、嵌套正确；四种格式都不再只剩一行 |
| 交互 trace（人工/脚本） | 大纲里按 Tab / Enter | Tab 不写 `children`（trace `outline_tree_key_ignored`）；Enter 新建一个概念并落到行内编辑 |

## 4. 非目标

- **不改 `.zmind`**：`kind` 字符串与 `pos` / `relationships` 原样；不新增字段、不升版本。
- **不新增导出路径**：PNG/SVG/PDF 一行代码都没动，仍吃 `Scene`。
- **不给大纲加「加边/删边」**：`docs/30` §5.9 要求「新建连线即进入短语编辑态，未填不给线落盘」，
  大纲里没有短语输入的位置。加边仍在画布（拖出连线）。这一条写进 §9 挂账。
- 不做概念图的多根合并、不做自动互链、不做「大纲里改链接短语」。
- 不碰 Xmind、不提交 git、不操作用户个人文件。

## 5. 风险

| 风险 | 处置 |
| --- | --- |
| 生成树顺序不确定（`HashMap` 迭代序） | 根的排序键固定为 `(root_id 优先, y, x, id)`；出边按 `relationships` 声明顺序；单测里对同一份文档连算两次比对 |
| 环让 DFS 无限展开 | 出栈即标记已访问，入栈前查 `visited`；单测里造 A→B→C→A + 自环 |
| 大纲重构会让 31 组旧基线漂 | 树文档走的仍是原分支（分叉在 `is_concept_map()` 之后），旧 `outline*` 场景不碰；只新增 `cm-outline` |
| 概念图里 `Node.collapsed` 被当成树折叠 | 折叠只影响行表可见性，`Toggle` 仍发 `SetCollapsed`（显示态），不写 `children` |

## 6. 验证

| 项 | 结果 |
| --- | --- |
| 新单测（core，5 条） | `concept_tree`：① 按声明顺序展开（深度/父/短语逐字段）；② 环 A→B→C→A 不回环、每概念一行、两次结果一致，且命令层拒绝自环；③ 两条边指向同一概念时**先到者定父**；④ 孤立概念各自成根并按 `y`/`x` 排；⑤ 树文档返回空表 |
| 新单测（shell，2 条） | `outline::concept_rows_follow_the_relationship_tree`（行表深度 0/1/2、子数、叶子无 chevron）；`export_svg::concept_map_exports_through_the_same_scene`（概念文字 / 自由概念 / 链接短语都进 SVG = 「零新增导出路径」真的零） |
| 新单测（io，1 条） | `concept_map_exports_follow_the_generated_tree`：Markdown 文首标注 + `## 因为 → 竞品访谈` + `### 导致 → 留存低` + 回边不重复展开；纯文本逐字节相等；OPML 标题带标注且嵌套正确；FreeMind 注释 + 嵌套 |
| 反例（golden） | 把 `self.concept_map = doc.is_concept_map()` 临时改成 `false`（等同回到「按 `children` 摊平」）→ `cm-outline` 报 **13812/2304000 像素（0.599%）**，包围盒 `x[588..776] y[161..398]`；恢复后 0 差异。基线真的守着「层级从连线来」 |
| 反例（io） | 把 `concept_label` 的短语摘掉（`Some(_via) => text`）→ `concept_map_exports_follow_the_generated_tree` 立刻断在 `## 因为 → 竞品访谈`；恢复后转绿 |
| `cargo fmt --all -- --check` | 干净 |
| `cargo clippy -p mw-shell --all-targets` | 新增 0（现存 4 条仍是 `export_pdf.rs` 3 + `panels/outline.rs:3084` 1，均为本包之前既有；新写的 `concept_tree.rs` / `export.rs` 零告警） |
| `cargo test --workspace -- --test-threads=1` | **813 passed / 0 failed**（805 + 8：5 条 core + 2 条 shell + 1 条 io） |
| 机器门 | `MACHINE GATE PASS`：**38 组比较 ×2 遍 `--tol 0` 全 0 差异**（`target/x100/stage-b-gate.log`，终局 `04:27:25`） |
| 旧基线 | `cm-default` `5b0d9dc4…`、`cm-card-link` `b7d11182…`、`outline*` / `library` 三档全部**逐字节未变**——概念图分支在 `is_concept_map()` 之后才分叉，树文档走的还是原路 |
| 新基线 | `cm-outline` = `16ced6ab…`（1920×1200 @1.0），连跑两次 `--tol 0` 0 差异 |

## 7. 场景与证据

| 文件 | 内容 |
| --- | --- |
| `shots/golden/cm-outline/` | `MW_CONCEPT_OUTLINE=1`：示例概念图 + 直接进全屏大纲。锁：页头 `大纲 · 产品定位 · 概念图 · 按链接层级展开`、右上「6 个概念」、行首短语 `解决 → 用户痛点` / `转化 → 增长策略`（二级）/ `支撑 → 技术方案` / `带来 → 风险与依赖`（二级）/ `参照 → 竞品对比`、页脚「6 个概念 · 0 个已完成」 |
| `shots/x100/CM-07/manifest.json` | 场景、哈希、阈值、命令清单 |
| `target/x100/stage-b-gate.log` | 38 组双跑 PASS 与完成时刻 |
| 生成树 / 导出单测 | 见 §6 前四行（跑法：`cargo test -p mw-core concept_tree`、`-p mw-io concept_map_exports`、`-p mw-shell concept_rows_follow`） |

## 8. 落点清单

| 层 | 改动 |
| --- | --- |
| `mw-core` | 新模块 `concept_tree.rs`：`concept_rows(doc) -> Vec<ConceptRow{id, depth, parent, via}>`（前序、环取首次访问边、孤立概念成根、非概念图返回空）、`root_order()`；`lib.rs` 重导出 |
| `mw-io` | `concept_label()`（`{短语} → {概念}`）、`concept_children()`（邻接表）、`CONCEPT_NOTE`；`export_markdown` / `export_outline` / `export_opml` / `export_freemind` 四条都在概念图下改走生成树；Markdown 文首 `> 概念图 · 按链接层级展开`，OPML 标题后缀，FreeMind XML 注释 |
| 大纲 | `Row.via`；`Outline.concept_map`；`Outline::rebuild_concept`（生成树行表 + 生成树父链面包屑 + 折叠裁子树 + 聚焦截子树）；`rebuild` 分叉；Enter → `Action::NewConcept`；Tab / Shift+Tab / Ctrl+Shift+↑↓ / 拖拽在概念图下不接（`trace outline_tree_key_ignored`）；行首淡色短语前缀（实测字宽让位，选区/编辑框/省略自动跟随） |
| 壳层 | `Action::NewConcept` 落地（`canvas.add_concept_below` → `AddFloatingTopic`，落点在被选概念下方 56px，然后行内编辑）；`OutlineHeader.concept`（页头 `· 概念图 · 按链接层级展开`、右上「N 个概念」）；页脚按类型换「个概念」；`MW_CONCEPT_OUTLINE=1` 演示入口 |
| 画布 | `add_floating_world` 抽出 `insert_floating_world()`（返回 id、不动选择/编辑态），新增 `add_concept_below()` |
| 门脚本 | `scripts/x100-stage-b-gate.ps1` 增 `cm-outline`（37 → 38 组比较） |

## 9. 未做 / 偏差（必须留在纸面上）

1. **大纲只加「点」，不加「边」**：`docs/30` §5.6 期望「像编辑思维导图一样用大纲改结构」，
   本包只做到 Enter 新建概念、F2/双击改名、Delete 删概念；**Tab/Shift+Tab 建/删连线没做**。
   原因：§5.9 要求「新建连线即进入短语编辑态，未填不给线落盘」，大纲里没有短语输入的位置，
   硬做会产出没有链接词的线（正是 §5.9 禁止的「线团」）。G-CM-2 判词里「大纲可编辑结构」
   这一条**按未完成记**，挂到下一包（需要一个「大纲行尾就地输短语」的设计）。
2. **聚焦语义借用生成树**：概念图大纲的 `Alt+Enter` 聚焦是**视图级**截取（生成树子树），
   不是文档结构；退出聚焦不写文档。这与树一致，但概念图的「子树」是投影，不是真结构。
3. **孤立概念在导出里是并列的多个顶层**：Markdown 会出现多个 `#`，OPML/FreeMind 会出多个平级
   `outline`/`node`。这是「不为孤立概念编造父级」的代价，已写进报告而不是悄悄挂到主概念下。
4. **链接短语只在 Markdown/OPML/FreeMind/纯文本里进 `text`**：`.zmind` 才是唯一无损面；
   文本格式没有「边」这种结构，属有损导出（`docs/05` §7 口径）。
5. **`Node.collapsed` 在概念图上仍然生效**：折叠只是行表可见性，`Toggle` 仍发 `SetCollapsed`；
   但画布上概念图不画折叠点，所以只能在 (概念图) 大纲里折——两处状态不会打架，但没有统一入口。
6. **短语前缀与长标题的挤压**：行文字宽减去前缀宽度后可能提前省略；没有做「前缀也省略」的
   二级策略（前缀本来就短，实测最长 4 字）。

