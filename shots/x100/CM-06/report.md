# CM-06 概念 ↔ 笔记联动 · 报告

> 状态：**已实现**（本包范围；G-CM-2 门未开）
> 设计合同：`docs/30-外壳体验与概念图设计规范.md` §4.2「卡片绑定」、§5.6「与笔记联动」
> 执行合同：`docs/31-外壳体验与概念图工作包.md` §6（CM-06，规模 M）
> 开工 / 完成：2026-10-26 · 依赖 CM-05

## 1. 起点（审计结果）

概念图的节点已经是 `Node`，`card_id` 字段天然可复用，但**联动只有半条腿**：

| 能力 | 审计时的现状 | 问题 |
| --- | --- | --- |
| 新建卡片 | 检查器「卡片笔记」区有「新建卡片笔记」→ `AttachNewCard` | 可用 |
| 解绑 | 「解除链接」→ `SetNodeCard { card: None }` | 可用，但文案与 §4.2 的「解绑」不一致 |
| **关联已有** | **没有入口** | §4.2 的三态里少了一态；用户只能新建，不能把库里已写的卡片挂上来 |
| 卡片页「来源」 | `doc_label` 写死 `"当前文档"` / `"另一份文档"` | §5.6 要求 `概念图名 · 概念名`；「当前文档」既不是名字也不告诉你这是概念图 |
| 卡片页回跳 | `JumpToNode` → `canvas.select_node` | 概念图节点是 floating，需验证真的能选中并居中（没用例） |
| 笔记库来源行 | `meta.push_str(" · 挂着主题")` | 只给了一个布尔事实，没给 `概念图名 · 概念名`；概念/主题不分 |
| 笔记库回跳按钮 | 「定位节点」 | 概念图上应叫「回到概念」，且要能选中 |
| 导出包来源 | `collection.rs` 写 `主题 · {文字}` | 概念图里写「主题」是错的语义 |
| 反查（卡片 → 概念） | `backlinks_for_card` 遍历全部节点 | 可用，无概念图用例 |

**结论**：不需要新字段、不需要 `.zmind` 版本变更——`Node.card_id` + `NoteCard.source_node` 已经够用，
缺的是**入口**（关联已有）、**措辞**（来源到底叫什么）、**回跳验证**。

## 2. 范围（本包做的五件事）

| # | 缺口 | 处置 | 落点 |
| --- | --- | --- | --- |
| 1 | 「关联已有」无入口 | 检查器卡片区未绑定时给「新建卡片 / 关联已有」两个动作；点「关联已有」内联展开当前文档里**未被别的节点占用**的卡片列表（最多 8 条，可滚），点一条发 `SetNodeCard` | `inspector.rs`（`Action::LinkCard`）、`inspector_methods_1.rs::card_section`、`main.rs` |
| 2 | 来源措辞 | `Document::display_title()`（`meta.title` 空则「未命名」）+ `Document::is_concept_map()`；`SourceLink` 增 `concept_map: bool`；卡片页渲染 `来源 [概念图] 概念图 · 用户痛点` | `mw-core/document.rs`、`app_methods_3.rs::card_reader_for`、`note_view.rs` |
| 3 | 笔记库来源行 | `CardSource{doc_label, node_text, concept_map}` 由壳层按当前文档算好，`show` 收 `Extras` 一起传；来源行改成 `概念图名 · 概念名` / `收件箱` / `另一份文档`；回跳按钮按类型切「回到概念 / 定位节点」 | `note_library.rs`、`app_methods_3.rs::card_sources` |
| 4 | 导出包语义 | `collection.rs` 概念图下写 `概念 · {文字}`，思维导图仍是 `主题 · {文字}` | `collection.rs` |
| 5 | 回跳无证据 | 新场景 `cm-card-link`（卡片页）+ `cm-card-link-back`（回跳后画布），两条共用 `Shell::jump_to_source`；trace `card-jump node=… concept=1` | `app_methods_1.rs`、`app_methods_3.rs`、`shots/golden/cm-card-link{,-back}/` |

## 3. 验收场景

| 场景 | 输入 | 判词 |
| --- | --- | --- |
| `cm-card-link` | `MW_CONCEPT_DEMO=1 MW_CONCEPT_CARD=1` | 卡片页打开，来源行显示 `概念图 · 用户痛点`，带「概念图」角标；关闭后画布选中的是「用户痛点」概念框 |
| 检查器（人工） | 选中一个未绑定的概念 | 出现「新建卡片笔记 / 关联已有」；点「关联已有」列出候选卡片；选中后画布立即出现卡片态 |
| 笔记库（人工） | 打开笔记库，看这张卡的预览 | 来源行是 `概念图 · 用户痛点`，按钮是「回到概念」 |

## 4. 非目标

- **不做 G-B2-2 要求的笔记库整体重排**（三栏、来源面包屑的版式规格）：那是 `docs/28` 的正文，
  本包只改「来源这一行写什么」，不动其布局与基线。
- 不做卡片页里的「解绑」入口：解绑在检查器与笔记库侧，卡片页只显示来源。
- 不做跨文档回跳（`JumpToNode` 在别的文档里仍 toast「来源主题不在当前文档里」）——
  跨文档要换会话 + 恢复视图，属 SE 轨范围。
- 不新增 `Command` 变体：`SetNodeCard` / `AttachNewCard` 已够。
- 不改 `.zmind` 版本与字段；不动 `Node` / `NoteCard` 结构。

## 5. 风险

| 风险 | 处置 |
| --- | --- |
| 检查器内联列表会把面板撑高 | 列表限高 168px（6 条）内部滚动，展开一次就收起 |
| 笔记库基线（`library` 场景）会因来源行变字而漂 | 先看该场景是否存在；存在则按 `docs/29` §5 重拍并连跑两次 `--tol 0`，在 §7 写漂移量 |
| 概念图 `select_node` 依赖 `scene.find` | `scene` 对概念图有节点，中心点用 `scene.rect`，不依赖父子；用场景 + trace 钉住 |

## 6. 验证

| 项 | 结果 |
| --- | --- |
| 新单测（core） | `display_title_falls_back_and_marks_concept_maps`（空白标题 → 未命名；切布局后 `is_concept_map` 跟着走，不粘住）、`linkable_cards_exclude_cards_taken_by_other_nodes`（别人占的卡片不进候选；自己挂着的那张仍可选 = 换绑；`limit` 与「最近更新优先」）、`concept_card_backlink_names_the_concept`（概念图下的反查取名，`card=false`） |
| 新单测（shell） | `card_source_phrase_is_doc_then_node`（`文档名 · 节点名`；文档名未知返回 `None`；两种文档类型短语形状一致） |
| 反例（core） | 去掉 `linkable_cards` 里的 `n.id != node_id` → `for_a.contains(&old.id)` 立刻断红（说明「换绑也要能选回原卡」这条真在守）；恢复后转绿 |
| 反例（shell） | 去掉 `CardSource::phrase` 的空名检查 → 返回 `Some(" · 用户痛点")`，断言红在「跨文档时名字未知」；恢复后转绿 |
| 反例（golden） | 拿掉 `jump_to_source` 里的 `canvas.select_node(id)` → `cm-card-link-back` 报 **13729/2304000 像素超差（0.596%）**、exit 1；恢复后 0 差异。基线真的守着「跳回来得选中谁」 |
| `cargo fmt --all -- --check` | 干净 |
| `cargo clippy -p mw-shell --all-targets` | 新增 0（`note_library::show` 一度 8 参数触发 `too_many_arguments`，已用 `Extras` 打包；现存 4 条仍是 `export_pdf.rs` 3 + `panels/outline.rs` 1） |
| `cargo test --workspace -- --test-threads=1` | **805 passed / 0 failed**（801 + 4：3 条 core + 1 条 shell） |
| 机器门 | `MACHINE GATE PASS`：**37 组比较 ×2 遍 `--tol 0` 全 0 差异**（`gate-summary.txt`，终局 **03:34:25**） |
| 旧基线 | `cm-default` `5b0d9dc4…`、`cm-narrow` `fc7f31f1…`、`cm-arrange` `56bc4017…`、`cm-crosslink` `f2e948d1…`、`library` `561fa6ce…`、`card-slash` `3e3cd883…` **逐字节未变**——来源行只对 `sourceNode` 有值的卡片生效，演示卡片都没有，所以三档缩放的笔记库基线一格不漂 |
| 新基线 | `cm-card-link` = `b7d11182…`、`cm-card-link-back` = `1ee059f4…`（均 1920×1200 @1.0），各双跑 `--tol 0` 0 差异 |

## 7. 场景与证据

| 文件 | 内容 |
| --- | --- |
| `shots/golden/cm-card-link/` | `MW_CONCEPT_DEMO=1 MW_CONCEPT_CARD=1`：概念图模板 + 「用户痛点」挂一张卡片，直接开卡片页。锁：来源行 `来源 概念图 · 用户痛点`、顶栏「回到概念」、日期固定 `2026-10-24` |
| `shots/golden/cm-card-link-back/` | 同一份数据 + `MW_CONCEPT_CARD_BACK=用户痛点`：走真实 `jump_to_source`，卡片页关闭、画布选中该概念（状态栏「已选 · 用户痛点」） |
| `cm-card-link.trace.txt` | `event card-jump node=… concept=1` + `focus canvas`——回跳真的走到了壳层落地 |
| `cm-card-pick.png` | 检查器「内容」页实拍：`这个概念还没有卡片笔记。` / `新建卡片笔记` `收起候选` / 候选块里只有「竞品访谈 · 两家」（另一张挂在「风险与依赖」上，**没出现**） |
| `gate-summary.txt` | 37 组双跑 PASS 与完成时刻 |

## 8. 落点清单

| 层 | 改动 |
| --- | --- |
| `mw-core` | `Document::layout_kind()` / `is_concept_map()` / `display_title()` / `linkable_cards(node, limit)` |
| 检查器 | `Action::LinkCard{node,card}`；`card_section` 三态（新建 / 关联已有 / 打开 + 解绑）；`card_pick_list` 限高 168px、最多 8 条、只列没被占的；措辞按 `is_concept_map()` 在「主题/概念」之间切 |
| 卡片页 | `SourceLink.concept_map`；来源行 `{文档名} · {节点名}`；文档名恰为「概念图」时不画类型角标（避免 `概念图 概念图 · 用户痛点`）；按钮「回到概念 / 回到节点」 |
| 笔记库 | `CardSource{doc_label,node_text,concept_map}` + `phrase()`；`show` 收 `Extras`；来源行 `· 来源 {文档名} · {节点名}`；按钮「回到概念 / 定位节点」；toast 按类型换词 |
| 回跳 | `Shell::jump_to_source(&str)`——关浮层 + 选中 + 交还焦点 + `trace card-jump`，卡片页与笔记库**共用同一个落地点** |
| 导出包 | `collection::collect` 概念图写 `概念 · x`，思维导图仍 `主题 · x` |

## 9. 未做 / 偏差（必须留在纸面上）

1. **笔记库版式没动**：`docs/28` 的三栏 + 来源面包屑属 G-B2-2，本包只改「来源这一行写什么」。
   改完 `library` 三档基线逐字节未变，说明边界没越。
2. **卡片页里没有解绑入口**：解绑留在检查器（「解绑」）与笔记库侧。卡片页是全屏阅读面，多一个破坏性按钮不值得。
3. **跨文档回跳仍只给提示**：`jump_to_source` 返回 false 时 toast「来源概念不在当前文档里」。
   要真跳得换会话 + 恢复视图，属 SE 轨。
4. **检查器头部类型字仍写「分支主题」**：概念图上应该是「概念」。卡片区已按类型换词，
   头部标题属检查器重做（SE 轨）范围，未在 CM 轨拆改。
5. **`MW_CONCEPT_CARD` 用 `replace_session` 造初始文档**：走 `AttachNewCard` 会把
   `updated_at` 刷成墙钟，卡片页的「更新于」每天漂一天、基线天天红。夹具口径与
   `Template::session()` 一致（从零构造初始文档，新文档没有撤销历史），`validate()` 兜底。
   代价：它同时设了 `MW_CONCEPT_CROSS` 时，后跑的 `replace_session` 会把交叉连接盖掉——
   两个变量不同时用（现有场景确实不同时用）。
6. **候选列表只列「当前文档里没被占的卡片」**：库里的孤儿卡片（跨文档）不在候选里。
   跨库关联要等笔记库侧的能力，本轮不做。
7. **`Ctrl+A` 之外没有「批量绑卡」**：一次一个节点，与 `docs/30` §5.6「与思维导图同一套机制」一致。
8. **`cm-card-link-back` 的选中环在浅色主题下偏淡**：沿用 CM-02/CM-03 的选中样式，未在 CM-06 调整。

