# CM-23 · `㉓ .cxl` 互导证据包（2026-11-02）

> **一句话**：`.cxl` 互导的**代码已在**（`mw-io/src/cmap.rs` 548 行 + `dispatch.rs` 路由 + 4 条单测），
> 本轮把它测绘成**可复核的证据**：保真槽 / 互通视图 / 降级解析三层的字段对账、
> 往返断言设计、门表准入建议。**本轮不改任何代码**（并行会话正在改 `crates/`），
> 单测只**跑**不**改**。
>
> 状态：`进行中`（代码在、单测在，缺真 `.cxl` 样本与 CmapTools 真机验证）。
> 本文件只用 `未开始` / `进行中` / `延后`，不写「已完成」「已验证」。
> 行号是快照，每条引用都写出那一行的字面内容。

---

## 1. 范围与非目标

### 1.1 范围

| 做的事 | 产出 |
| --- | --- |
| 读通 `crates/mw-io/src/cmap.rs` 全文（写 / 读 / 降级 / 单测） | §2 实现现状 |
| 读通互导路由 `crates/mw-io/src/dispatch.rs` | §2 路由 |
| 跑 `cargo test -p mw-io` 并把原始输出落盘 | [`evidence-cargo-test-mw-io.txt`](evidence-cargo-test-mw-io.txt) |
| 逐字段对账：保真 / 可表达 / 降级丢失 | §3 |
| 设计往返断言（字段级，不是字节级） | §4 |
| 门表准入建议 | §5 |
| 未做与风险（含「本机无 CmapTools 真机」） | §6 |

### 1.2 非目标（本轮明确不做）

1. **不改 `crates/` 任何文件**——包括不改 `cmap.rs`、不加新单测、不修 §6 列出的缺陷。
2. **不跑 `cargo build --release` / `cargo golden`**——会和并行会话抢构建锁（`stage-*` 正式门要 LTO 重建）。
   因此本包**没有像素基线**，也不该有。
3. **不造真 `.cxl` 样本**：本机没有 CmapTools，编一个「CmapTools 导出的」文件就是伪造证据。
4. **不判定「互导可用」**：本轮只到**格式层**；「CmapTools 能打开且好看」这件事未验证（见 §6 第 1 条）。
5. 不改 `docs/STATUS.md`（本轮写作用域只有 `shots/x100/CM-23/`）。

---

## 2. 实现现状（入口 / 路由 / 单测名）

### 2.1 写：`Document` → CXL

| 步骤 | 出处（字面） |
| --- | --- |
| 入口 | `cmap.rs:30` `pub fn save_cxl(doc: &Document) -> Result<String, IoError> {` |
| 抽出概念图专属块 | `cmap.rs:31-35` `let block = mw_format::concept_map_block(&doc.nodes, doc.meta.layout.focus_question.clone(), doc.groups.clone(),);` |
| 概念清单 | `cmap.rs:72` `<concept id="{…}" label="{…}"/>` |
| 命题 → 短语 | `cmap.rs:95` `phrase_id: format!("lp-{}", p.id),`；`cmap.rs:93` 空短语回落到字面 "related to"（`unwrap_or_else` 里的默认值） |
| 两条连接 | `cmap.rs:115-126` `from-id="{t.from}" to-id="{t.phrase_id}"` / `from-id="{t.phrase_id}" to-id="{t.to}"` |
| 概念外观 | `cmap.rs:134-151` 只写 `x` / `y` / `font-size` / `background-color` / `border-color` |
| 命题外观 | `cmap.rs:172-176` `Off => "no"`、`Both => "yes"`、`_ => "if-to-concept"` |
| 保真槽 | `cmap.rs:186-196` `mw_format::doc_to_fidelity_json(doc, None)` → `<properties-list id="mindweaver"><property><key>doc</key><value>…` |
| 时间戳 | `cmap.rs:225-231` `format!("1970-01-01T00:00:{secs:02}Z")`（见 §6 第 5 条，格式有问题） |

### 2.2 读：CXL → `Document`

| 分支 | 出处（字面） |
| --- | --- |
| 入口 | `cmap.rs:274` `pub fn load_cxl(xml: &str) -> Result<Document, IoError> {` |
| ① 保真槽优先 | `cmap.rs:276-280` `if let Some(fidelity) = extract_fidelity(xml) { … return Ok(loaded.doc); }` |
| ② 降级 | `cmap.rs:282` `parse_cxl_interop(xml)` |
| 抠保真槽 | `cmap.rs:288` `let key_pos = xml.find("<key>doc</key>")?;` |
| 降级解析 | `cmap.rs:305` `fn parse_cxl_interop(xml: &str) -> Result<Document, IoError> {`；概念 `:340-346`、短语 `:347-353`、连接 `:354-360`、坐标 `:361-368`、聚焦问题 `:372-376` |
| 降级重建命题 | `cmap.rs:396-423` 「每个短语找 A→P 与 P→B 两条连接」 |
| 收口 | `cmap.rs:441` `fn build_concept_doc(block: …)`；标题硬编码 `:451` `title: "概念图".to_string(),`；主题硬编码 `:452` `theme: "snow".to_string(),` |

### 2.3 路由（`.cxl` / `.cmap` 都进 CXL）

| 行为 | 出处（字面） |
| --- | --- |
| 扩展名判定 | `dispatch.rs:40` `"cxl" \| "cmap" => DocFormat::Cxl,` |
| 新文档默认扩展名 | `dispatch.rs:47-49` `if doc.meta.layout.kind == "conceptMap" { ".cxl" }` |
| 保存分支 | `dispatch.rs:63-66` `let xml = crate::cmap::save_cxl(doc)…; write_safe(path, xml.as_bytes())?;` |
| 加载分支 | `dispatch.rs:88-91` `crate::cmap::load_cxl(&xml)…` |

> 注意：概念图**新文档默认扩展名就是 `.cxl`**（`dispatch.rs:49`）。也就是说这条链路不是「可选功能」，
> 它已经在用户按 `Ctrl+S` 的默认路径上——这是本包值得优先收口的真正理由。

### 2.4 单测现状（实跑）

命令与完整输出见 [`evidence-cargo-test-mw-io.txt`](evidence-cargo-test-mw-io.txt)：

```powershell
$env:TEMP="…\target\tmp"; $env:TMP=$env:TEMP
cargo test -p mw-io -- --test-threads=1
```

结果：**27 passed / 0 failed**（lib 单测 23 条 + `tests/fidelity.rs` 集成 4 条 + doc-test 0 条）。

与 `.cxl` 直接相关的 4 条：

| 单测 | 出处 | 断言了什么（原文要点） |
| --- | --- | --- |
| `cmap::tests::cxl_roundtrips_fidelity_lossless` | `cmap.rs:507` | 聚焦问题、节点数 = 2、一条命题的短语 `需要` |
| `cmap::tests::cxl_interop_has_concepts_phrases_connections_and_focus` | `cmap.rs:522` | XML 里含 `<concept id="c-a" label="植物"/>`、`linking-phrase`、`from-id="c-b" to-id="lp-p1"`、聚焦问题 |
| `cmap::tests::cxl_degraded_parse_without_fidelity` | `cmap.rs:537` | 删掉 `<extra-properties-list>` 后仍能读到 ≥2 个概念 |
| `dispatch::tests::save_load_roundtrip_xmind_and_cxl` | `dispatch.rs:195` | 真落盘到临时目录，读回聚焦问题与节点数 |

> 覆盖评价：四条都只钉**「能打开、概念在、短语在」**。**没有任何一条**断言 groups / pinned /
> 线型线宽 / 备注 / 卡片 / 样式 / 箭头四档 / 标题 / 未知键。这正是 §4 要设计的部分。

---

## 3. 保真 / 可表达 / 降级丢失 · 逐字段对账

先立三个口径，否则「丢没丢」会说不清：

| 口径 | 含义 | 谁受影响 |
| --- | --- | --- |
| **A. 保真（织思 → 织思）** | 走保真槽，`.zmind` 模型字段原样回来 | 只有织思自己打开 |
| **B. 可表达（CmapTools 看得见）** | 互通视图里的标准 CXL 元素 | CmapTools 打开 |
| **C. 降级丢失（保真槽被外部工具抹掉后）** | 只剩 B，A 独有字段全丢 | CmapTools 用户改完另存再给织思 |

### 3.1 逐字段表

| # | 字段 | 互通视图写成什么 | A 保真 | B 可表达 | C 降级回读 | 出处（文件:行 + 字面） |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `meta.title` | `<dc:title>` | 有 | 有 | **已修（本轮）**：有 `dc:title` 就用它，空/无才退回「概念图」 | 写 `cmap.rs` 的 `<dc:title>`；读 `dc:title` → `build_concept_doc(block, title)`（§7 第 1 条） |
| 2 | 文档 id / created_at | 无（仅保真槽） | 有 | 无 | 丢 | `mw-format/src/lib.rs:446-451` `FidelityDoc { doc_id, title, created_at, content }` |
| 3 | `theme` / `theme_accent` / `theme_overrides` | 无 | 有 | 无 | **丢 → 硬编码 `snow`** | `cmap.rs:452` `theme: "snow".to_string(),` |
| 4 | `layout.kind` | 无（回读时按概念图重建） | 有 | 无 | 重建为 `conceptMap` | `cmap.rs:456` `kind: "conceptMap".to_string(),` |
| 5 | `layout.focus_question` | `<dc:description>` | 有 | 有 | **有** | `cmap.rs:56`；降级读 `:372-376` |
| 6 | `layout` 其余键（`direction`/`edge_shape`/`edge_arrow`/`sibling_gap`/`level_gap`/`max_node_width`） | 无 | 有 | 无 | 丢（`..Default::default()`） | `cmap.rs:455-459` |
| 7 | 概念 `id` | `<concept id>` | 有 | 有 | 有 | `cmap.rs:72` |
| 8 | 概念 `text` | `<concept label>` | 有 | 有 | 有 | `cmap.rs:74`；降级 `:342` |
| 9 | 概念 `pos` | `<concept-appearance x y>`（`as i32` 截断） | 有 | 有（取整） | 有（缺则 `0,0`） | `cmap.rs:135-138`；降级 `:363-364`、`:425` |
| 10 | `pinned` | 无 | 有 | 无 | **丢 → `None`** | `cmap.rs:431` `pinned: None` |
| 11 | `style.font_size` | `font-size` | 有 | 有 | **丢**（降级只读 `id/x/y`） | 写 `:141-143`；降级 `:361-368` 无该属性 |
| 12 | `style.fill` / `style.stroke` | `background-color` / `border-color`（`r,g,b,a`） | 有 | 有 | **丢** | 写 `:144-149`；转换 `:234-252` |
| 13 | `style` 其余键（`shape`/`bold`/`italic`/`radius`/`align`/`weight`/`text_color`/`stroke_width`/`branch_color`/`apply_text_to_subtree`） | 无 | 有 | 无 | 丢 | `:140-150` 只写三项 |
| 14 | 命题（源/目标） | 概念 → 短语 → 概念两条 `<connection>` | 有 | 有 | 有 | `:112-128`；降级 `:396-423` |
| 15 | 命题 `label` | `<linking-phrase label>` | 有 | 有 | 有（空短语变成字面 "related to"） | `:89-93` 空短语的 `unwrap_or_else` 默认值 |
| 16 | 命题 `head` | **两条边各写一条** `connection-appearance`（`no`/`yes`/`if-to-concept`） | 有 | **四档全可表达** | **已修（本轮）**：降级解析回读 `connection-appearance`（含 `isBidirectional`）反推四档 | 写 `connection-appearance-list` 两条；读 `has_arrow(...)` → `match (arrow_in, arrow_out)`（§7 第 3 条） |
| 17 | `head == Backward`（反向命题） | **已修（本轮）**：箭头移到 `A→短语` 那条边（`-a` = `if-to-concept`、`-b` = `no`），与 `Forward` 不再同形 | 有 | **可表达** | 有（回读为 `Backward`） | 写 `match t.head { Backward => ("if-to-concept", "no"), ... }`；读 `(true,false) => ArrowHead::Backward` |
| 18 | 命题 `id` | 保真槽；互通视图里是 `lp-<id>` | 有 | — | **变**（`p1` → `lp-p1`） | `:95` `format!("lp-{}", p.id)`；降级 `:413` `id: pid.clone()` |
| 19 | 命题 `color` / `dashed` / `width` / `note` | 无 | 有 | 无 | 丢 | `:170-183` 只写 `arrowhead` |
| 20 | `groups`（概念分组） | **已修（本轮）**：明确不写（CXL 无分组元素），`save_cxl` 显式传空表 + 注释，死参数已删 | 有（保真槽 `content.groups`，本轮加单测钉住） | 无 | 丢 → 空 | 写 `concept_map_block(..., Vec::new())`；读 `doc_from_file_doc` 的 `file.groups.clone()`（§7 第 4 条） |
| 21 | 节点 `note` / `link` / `icon` / `labels` / `task` / `card_id` / `collapsed` / `image` | 无（`ConceptEntry` 没有这些键） | **已修（2026-11-02）**：读侧按 id 从兼容镜像 `nodes` 并回扩展字段（§7.5） | 无 | 丢 | 写 `mw-format/src/lib.rs` `nodes: doc.nodes.clone()`；读 `nodes_from_file_doc`（`concept_map::to_nodes` + 镜像合并） |
| 22 | 卡片 `cards`（笔记正文 + 元数据） | 无 | 有 | 无 | 丢 | `mw-format/src/lib.rs:424` `cards: doc.cards.clone(),` |
| 23 | 未知顶层键（`FileDoc.extra`）与节点未知键 | 无 | **丢** | 无 | 丢 | `mw-format/src/lib.rs:436` `extra: serde_json::Map::new(),` |
| 24 | 图片 / 附件字节（`.zmind` 的 `assets/`） | 无 | **丢** | 无 | 丢 | 保真槽是**单条 XML 属性值**，装不下 ZIP 条目 |
| 25 | `dcterms:created` / `modified` | 写 | — | **已修（本轮）**：`YYYY-MM-DDTHH:MM:SS.mmmZ`，`created` 取保真槽 `createdAt` | 仍不读回（CXL 元数据，不入模型） | 写 `iso8601_utc(ms)`；单测 `cxl_timestamps_are_iso8601_utc_and_ordered`（§7 第 2 条） |

### 3.2 口径小结

逐行点数（共 25 行）：

- **A（织思 → 织思，保真槽在）**：**22 / 25 行保留**。三个例外：#23 未知顶层键、#24 附件字节、
  #25 时间戳（写进去了但从不读回）。#21 节点扩展字段已于 2026-11-02 修好（§7.5）。
- **B（CmapTools 看得见）**：**11 / 25 行**写进了标准元素——#1 标题、#5 聚焦问题、#7 概念 id、#8 文本、
  #9 坐标（取整）、#11 字号、#12 填充/边框色、#14 命题结构、#15 短语、#16 箭头（**只三档**）、
  #25 时间戳（**格式非法**）。
  **看不见**：#10 钉住、#13 样式其余键、#19 线色/虚线/线宽/注释、#20 分组、#21 备注/标签/卡片等、#17 反向箭头。
- **C（外部工具另存后再回织思）**：**修码后 8 / 25 行能回读**——#1 标题（本轮修）、#5 聚焦问题、
  #7 id、#8 文本、#9 坐标、#14 命题结构、#15 短语、#16/#17 箭头四档（本轮修）；
  另有 #18 命题 id **变形**（`p1` → `lp-p1`）。
  **其余 15 行全丢**：#3 主题、#10 钉住、#20 分组、#11/#12/#13 全部样式、#19 线属性、#21 节点扩展字段……
  这一档必须在用户手册里写明，否则用户会认为「织思丢了他的数据」。

> **诚实标注**：上表的「B 可表达」只表示**写进了标准元素**，**没有**用 CmapTools 真机打开验证过
> （本机未安装，见 §6 第 1 条）。

---

## 4. 往返断言设计（只设计，不改码）

### 4.1 为什么不能是「字节级」比较

`.cxl` 的字节里天然有三个非确定源：

1. `cmap.rs:45` `let modified = iso_ms(mw_core::now_ms());` —— 每写一次都变；
2. 保真槽里的 `created_at` 走 `mw_core::now_ms()`（`mw-format/src/lib.rs:449`）；
3. `HashMap` 遍历顺序（`doc.nodes` 是 `HashMap`），虽然 `from_nodes` 按 id 排序挡掉了大部分。

所以判据应该是**字段级结构相等**，不是字节相等。`docs/33` §3 的 `round_trip_is_byte_exact`
（`concept_map.rs:196`）也只是「按 id 排序后的序列化比较」，同一套思路。

### 4.2 两条断言的判定口径

**判据一：保真往返（A 档，必须有）**

```
doc → save_cxl → xml → load_cxl → back
断言：norm(back) == norm(doc)
其中 norm(d) = 按 id 排序后逐节点 serde_json::to_string(Node)，再拼 groups / cards / meta
```

要求覆盖：`meta.title`、`meta.theme(+accent/overrides)`、`meta.layout`（整结构，含 `focus_question`）、
`root_id`、每节点的 `text/pos/pinned/style/note/link/icon/labels/task/card_id/collapsed`、
每条命题的 `id/to/label/color/head/dashed/width/note`、`groups`、`cards`。

**判据二：降级往返（C 档，必须有，且要「显式断言丢了什么」）**

```
xml → 删掉 <extra-properties-list>…</extra-properties-list> → load_cxl → degraded
断言：
  ① 正向：概念数/文本/坐标、命题源目标与短语、focus_question 与原文档一致；
  ② 反向（关键）：degraded 里 pinned 全 None、groups 空、style 全 None、
     head 全 Forward、title == "概念图" —— 把「丢失」写成断言，才不会哪天悄悄变成「丢得更多」。
```

**判据三：互通视图的确定性（可选，建议）**

```
save_cxl(doc) 两次，除 <dcterms:modified> 那一行外必须逐字节相同
```

### 4.3 建议新增的单测名（只设计；本轮不改码）

| # | 建议测试名 | 钉住什么 | 现状 |
| --- | --- | --- | --- |
| 1 | `cxl_fidelity_roundtrip_preserves_groups_pinned_and_line_styles` | #10 / #19 / #20 | 缺 |
| 2 | `cxl_fidelity_roundtrip_preserves_node_note_labels_card_and_style` | #21 / #22 / #13 | 缺 |
| 3 | `cxl_fidelity_roundtrip_preserves_layout_and_title` | #1 / #3 / #6 | 缺 |
| 4 | `cxl_degraded_parse_drops_the_documented_fields_explicitly` | §4.2 判据二的反向断言 | 现测试只查 `nodes >= 2` |
| 5 | `cxl_interop_view_omits_groups_and_pinned_but_keeps_geometry` | #10 / #20 在 XML 里**不出现** | 缺 |
| 6 | `cxl_interop_view_cannot_express_backward_arrowhead` | #17 的不可表达性（写成文档事实） | 缺 |
| 7 | `cxl_save_is_deterministic_except_modified` | 判据三 | 缺 |
| 8 | `cxl_degraded_parse_keeps_title_from_dc_title` | #1 —— **当前会红**（标题硬编码），属待修缺陷 | 缺 |
| 9 | `cxl_timestamps_are_valid_iso8601` | #25 —— **当前会红**，属待修缺陷 | 缺 |
| 10 | `cxl_fidelity_slot_preserves_unknown_top_level_keys` | #23 —— **当前会红**，属待修缺陷 | 缺 |

> #8–#10 是「先修后加」的类型：写测试是为了让缺陷可见，不是本轮能修的（`crates/` 不许动）。

---

## 5. 门表准入建议

### 5.1 结论：**保真不进像素门，入口进 1–2 场**

`stage-b` / `stage-cm` 是**截图矩阵**（每场一张 `shell-ppp*.png` 比像素）。
`.cxl` 往返是**文件层**的事，用截图证明不了保真——硬塞进去只会得到「点开菜单、弹个 toast」这种假证据。

| 层 | 建议 | 理由 |
| --- | --- | --- |
| **保真** | 走 `mw-io` 集成测试（`crates/mw-io/tests/`），**不进**截图门 | 字段级 diff 是唯一说得清的判据 |
| **入口可达** | 进 `stage-cm` **1 场**：概念图另存为 `.cxl` 后提示条正常 | 证明 `dispatch.rs` 那条默认路径真的有 UI 出口 |
| **降级可读** | 走单测（§4.3 #4），**不进**门 | 没有 CmapTools 就没有真样本，像素门无法覆盖 |

### 5.2 具体场景名建议（若要进 `stage-cm`）

| 场景名 | 输入 | 钉住什么 |
| --- | --- | --- |
| `cm-save-cxl` | 概念图 → `Ctrl+Shift+S` → 文件名 `x.cxl` → 确认 | 保存成功提示（不弹错、不落回到 `.zmind`） |
| `cm-open-cxl` | 打开上一步写出的 `.cxl`（走 `MW_GOLDEN_INPUT` 脚本或启动参数） | 打开后画布概念/命题/短语与导出前一致（**像素级**只证明「画出来了」，不证明保真） |

> 若只准加一场：加 `cm-save-cxl`；它证明的是**默认扩展名链路**，风险最高的那一段。

### 5.3 但门表不是本轮的门槛

按 `docs/31` 的包纪律，CM-23 若要立项，需要：① 报告（本文件）；② 证据；③ 门表条目；④ `docs/STATUS.md` 回填。
本轮只交 ① 与 ② 的一半（缺真样本与 CmapTools 真机），所以 CM-23 状态记 `进行中`，**不能记 `已实现`**。

---

## 6. 未做 / 风险

1. **本机没有 CmapTools 真机 —— 互导只做到格式层**（必须写，且是最重要的一条）。
   §3 的「B 可表达」全部由代码阅读得出，**没有一条**经过 CmapTools 打开验证；
   `cmap.xsd` 的元素名/属性名是否与真实 CmapTools 版本一致（尤其 `linking-phrase-appearance`、
   `connection-appearance.arrowhead` 的取值域）**未核**。唯一能终结这条的是「用户从 CmapTools 导一份 `.cxl` 给我」。
2. **没有真 `.cxl` 样本**：`shots/golden/` 下没有任何 `.cxl`，`scripts/*.ps1` 也不提 `cxl`。
   现在的「互通视图」正确性都是从 `cmap.rs` 的三条单测反推的，属于自产自销。
3. **`tests/fidelity.rs` 没有 `.cxl` 一行**：它覆盖 OPML / FreeMind / Markdown / XMind 的十样本往返
   （`crates/mw-io/tests/fidelity.rs` 的 4 条测试），**概念图 `.cxl` 完全不在矩阵里**。
   另外该文件头注释仍写着「XMind 目前只有导入器，没有导出器」，而 `save_xmind` 早已存在——注释过期，
   容易让下一棒误判覆盖范围。
4. **保真槽装不下附件**：`assets/` 是 ZIP 条目，保真槽是**单条 XML 属性值**（`cmap.rs:192-194`），
   图片/附件字节必然丢。当前节点 `image` 还只是占位（`mw-core/src/model.rs` 的 `pub image: Option<String>`，
   注释「图片/附件占位（P41；真正加载在 P53）」），所以**今天还没有真实痛感**，但这是设计上的硬边界，要写进文档。
5. **`dcterms:created` / `modified` 时间戳格式非法**：`cmap.rs:230` `format!("1970-01-01T00:00:{secs:02}Z")`，
   其中 `secs = now/1000` ≈ 1.79e9，写出来是 `1970-01-01T00:00:1794258000Z` —— 既不是合法 ISO 8601，
   日期也永远是 1970。注释自称「CmapTools 不据此计算」，但严格校验的 XML 消费者可能直接拒收。
   而且 `created` 与 `modified` 都是「现在」（`iso_ms(_now: i64)` **传参未用**），创建时间语义也没有。
6. **降级路径丢标题**：`dc:title` 写进了 XML（`cmap.rs:54`），但降级解析从不读它，回读标题恒为 `"概念图"`
   （`cmap.rs:451`）。这是 §4.3 #8 那条「先修后加」的测试要钉的。
7. **空短语被写成字面 `related to`**（`cmap.rs:93`）：织思里「没有短语」的命题，在 CmapTools 里变成
   `related to` 的**真短语**；再降级读回来就是 `Some("related to")`。语义从「空」变成「有值」，
   与 `docs/30` §5.9「命题必须有短语」的口径不一致，需要在互通视图里定个说法（留空 / 用 `related to` 并标注）。
8. **`head == Backward` 不可表达**（`cmap.rs:175` `_ => "if-to-concept"`）：反向命题在 CmapTools 里
   与正向长得一样。要么在互通视图额外用 `isBidirectional` 表达，要么在文档里明说「反向只在织思内有效」。
9. **未知顶层键不进保真槽**（`mw-format/src/lib.rs:436` `extra: serde_json::Map::new()`）：
   一份含未来字段的 `.zmind` 存成 `.cxl` 再从保真槽读回，未知键**消失**。这与 `docs/32` 反复强调的
   「新版本写的字段被旧版本打开再存不会消失」是同一个保真原则，这里是**破例**，应记账。
10. **单测覆盖薄**：4 条 `.cxl` 相关测试只钉「能开、概念在、短语在」；`#8/#9/#10` 那三条缺陷今天没有任何测试守着。

---

## 7. 本轮修码（2026-11-02）

> 触发：lead 判「§6 里的三个缺陷 + `groups` 从未遍历」**不是格式层限制，是这一轮就该修的 bug**。
> 写作用域：`crates/mw-io/src/cmap.rs`、`crates/mw-io/tests/fidelity.rs`（新增 `.cxl` 用例 + 改过期头注释）；
> `crates/mw-io/src/dispatch.rs` 只在需要时动 `.cxl` 路由那一段。
> **禁改**：`crates/mw-core/**`、`crates/mw-render/**`、`crates/mw-shell/**`、`docs/`、`scripts/`。
> 状态：`进行中`（修完并跑过测试，但**仍未**用 CmapTools 真机验证，仍不记 `已实现`）。

### 7.1 修的四条（对应 §6 编号）

| # | 缺陷 | 改法 | 新增/改动的单测 |
| --- | --- | --- | --- |
| 1 | 降级路径丢标题（§6 第 6 条） | `parse_cxl_interop` 增加 `<dc:title>` 捕获，`build_concept_doc` 改成**接受标题参数**：有 `dc:title` 就用它，没有（或空/全空白）才退回「概念图」 | `cxl_degraded_parse_keeps_title_from_dc_title`、`cxl_degraded_parse_falls_back_to_default_title` |
| 2 | 时间戳非法（§6 第 5 条） | 删掉 `iso_ms(_now)` 那个假格式化；新增 `iso8601_utc(ms)` 手写公历换算（`civil_from_days`，**不引依赖**），输出 `YYYY-MM-DDTHH:MM:SS.mmmZ`。`dcterms:created` 取保真槽 JSON 里的 `createdAt`（原来 `iso_ms(0)` 传参未用），`dcterms:modified` 取 `now_ms()` | `cxl_timestamps_are_iso8601_utc_and_ordered` |
| 3 | 反向箭头不可表达（§6 第 8 条 / §3 第 #17 行） | 箭头写到**它所在的那条 connection 上**：`Off` = 两条都 `no`；`Forward` = `-a` 无 / `-b` `if-to-concept`；`Backward` = `-a` `if-to-concept` / `-b` 无；`Both` = 两条 `yes`。降级解析新增 `connection-appearance` 回读（含 `isBidirectional`），按「哪条边有箭头」反推四档 | `cxl_arrowhead_slots_encode_direction`、`cxl_interop_parse_reads_arrowhead_direction_back` |
| 4 | `groups` 传了参数却从不遍历（§6 第 9 条的近亲） | **明确不写进互通视图**（CXL 没有分组元素）。`save_cxl` 改为显式传空表并加注释说明「分组只走保真槽」；删除死参数用法，保真由新单测钉住 | `cxl_fidelity_roundtrip_preserves_groups_pinned_and_line_styles` |

### 7.2 `crates/mw-io/src/cmap.rs` 的改动点（改后行号会漂，写清楚改了什么）

1. **`save_cxl` 顶部重排**：先算 `fidelity`（原来在写 map 之后才序列化），因为时间戳要用它的 `createdAt`。
   `dcterms:created` / `dcterms:modified` 改成 `iso8601_utc(...)`。
2. **`concept_map_block` 的第三个实参**：从 `doc.groups.clone()` 改成 `Vec::new()` + 注释
   「分组不进互通视图，只走保真槽」，消掉「传进 block 却没人读 `block.groups`」的死代码。
3. **`connection-appearance-list`**：从「只给 `c-<lp>-b` 写一个 arrowhead」改成**两条连接各写一条**，
   四档映射见 7.1 第 3 条。
4. **`parse_cxl_interop`**：新增三类捕获 —— `dc:title` 文本、`connection` 的
   `id`（原来只留 `from/to`）、`connection-appearance` 的 `id`/`arrowhead` 与 `isBidirectional`；
   重建命题时按「a 边有箭头 / b 边有箭头」推 `ArrowHead`（两条都无 → `Off`，无 appearance 记录时保持
   原默认 `Forward`，与旧行为一致）。
5. **`build_concept_doc(block, title)`**：标题参数化；`theme` 仍硬编码 `snow`（互通视图没有主题槽，
   保真槽里有——这一条不改，属 §7.4 的已知边界）。
6. **`iso_ms` 删除**，换成 `iso8601_utc`；新增测试用的 `parse_iso8601`（`#[cfg(test)]`）。

### 7.3 `crates/mw-io/tests/fidelity.rs` 的改动

1. 头注释那句「XMind 目前只有导入器，没有导出器」**已过期**（`export_xmind` 早已存在，
   同文件的 `ten_samples_survive_xmind_roundtrip_for_its_supported_fields` 就在用它）——改成事实陈述，
   并写明 `.cxl` 两条用例各自比什么。
2. 新增概念图样例 `concept_sample(i)`（3–6 概念、四档箭头、钉住/样式/分组按 `i` 变化）与两条用例：
   - `ten_concept_samples_survive_cxl_fidelity_roundtrip`：保真槽在时逐节点/逐命题比全字段，
     **唯一豁免**是「节点扩展字段」——见 §7.5，那是 `mw-format` 的读取侧缺陷，
     **2026-11-02 已修**（`nodes_from_file_doc` 按 id 并回镜像扩展字段），该豁免已删除；
   - `ten_concept_samples_survive_cxl_degraded_parse`：抹掉保真槽后只比互通视图真能表达的
     （标题 / 聚焦问题 / 概念数与文本 / 坐标 / 命题终点·短语·箭头四档）。

### 7.5 本轮修码时新发现的一条越界缺陷（**2026-11-02 已修**）

**现象**：`.cxl` / `.xmind` 的保真槽里**写着**节点的备注 / 链接 / 图标 / 标签 / 折叠 / `card_id` /
任务 / 图片 / 边界 / 概要（`build_file_doc` 里 `nodes: doc.nodes.clone()`），但**读不回来**。

**根因**（一行代码）：`crates/mw-format/src/lib.rs` 的 `doc_from_file_doc`：

```rust
let nodes = match &file.concept_map {
    Some(block) => concept_map::to_nodes(block),   // ← 概念图走这里
    None => file.nodes.clone(),
};
```

`ConceptEntry` 只有 `id / text / x / y / pinned / style`，所以走 `to_nodes` 重建节点时，
上面那些字段全部丢失——**哪怕保真槽里的 `content.nodes` 明明带着它们**。
`.zmind` 不走这个函数（`doc_from_file_doc` 的唯一调用点是 `doc_from_fidelity_json`），
所以受害面是**开放格式（`.cxl` / `.xmind`）的概念图**，不影响 `.zmind`。

**为什么当时没修**：修点在 `mw-format/src/lib.rs`，不在当轮写作用域（lead 只给了
`cmap.rs` / `tests/fidelity.rs` / `dispatch.rs` 的一小段）。建议改法（任选其一）：
① `file.nodes` 非空时优先用 `file.nodes.clone()`，`concept_map` 块只作 `.zmind` 兼容镜像；
② 或 `to_nodes` 之后把 `file.nodes` 里同 id 节点的扩展字段合并回来。
改完把 `cxl_fidelity_writes_node_extras_but_does_not_read_them_back` 与
`ten_concept_samples_survive_cxl_fidelity_roundtrip` 里的「豁免」一起删掉，改成断言保留。

**2026-11-02 落地**：按 ② 实现——新增 `mw_format::nodes_from_file_doc`，
`concept_map::to_nodes` 之后按 id 把兼容镜像 `nodes` 的扩展字段并回来（概念形状仍以块为准，
镜像里多出的树节点不塞进概念图）。三条断言同时改为「必须保留」：
`cmap::tests::cxl_fidelity_reads_node_extras_back`、`tests/fidelity.rs` 的
`ten_concept_samples_survive_cxl_fidelity_roundtrip`，以及新增的
`mw-format` 单测 `concept_block_is_the_authority_and_the_mirror_only_supplies_extras`。
另补一条 `.xmind` 保真槽往返单测 `save_then_load_keeps_every_field_through_the_fidelity_slot`。

**今天的账**：§3 第 #21 行从「写得出、读不回」改回「保真槽有」（`.zmind` 从来不受影响）。

### 7.6 结果（逐条：已修 / 未修）

| # | 条目 | 状态 | 说明 |
| --- | --- | --- | --- |
| 2 | 时间戳非法 | **已修** | `iso_ms` → `iso8601_utc(ms)`（手写公历换算，无新依赖）；`created` 取保真槽 `createdAt`；`parse_iso8601` 单测证明「写得出、解析得回、两者互逆」 |
| 1 | `dc:title` 读回 | **已修** | `parse_cxl_interop` 捕获标题 → `build_concept_doc(block, title)`；无/空/全空白才退回「概念图」 |
| 3 | 反向箭头不可表达 | **已修** | 箭头写到它所在的边：`-a`/`-b` 两条 `connection-appearance`；降级解析按「哪条边有箭头」反推四档，`Forward` 与 `Backward` 的 XML 不再同形 |
| 4 | `groups` 死代码 | **已修** | `save_cxl` 显式传空表 + 注释「分组只走保真槽」，死参数用法删除；保真由新单测钉住 |
| 5 | 节点扩展字段读不回（§7.5） | **已修（2026-11-02）** | `mw-format` 新增 `nodes_from_file_doc`：块为准 + 按 id 并回兼容镜像的扩展字段；三条测试改为断言保留，另补 `.xmind` 保真槽往返一条 |

**实跑**（原始输出：[`evidence-cargo-test-mw-io-fix.txt`](evidence-cargo-test-mw-io-fix.txt)）：

```
cargo check -p mw-io --all-targets        → exit 0
cargo test -p mw-io -- --test-threads=1   → 38 passed / 0 failed
                                            （lib 32 + tests/fidelity.rs 6 + doc-test 0）
```

新增单测 **11 条**（`cmap.rs` 9 条 + `tests/fidelity.rs` 2 条；后者原有 4 条不变，文件共 6 条）。

### 7.4 改完仍然成立的两个限制（不能因为修了 bug 就当成「互导可用」）

1. **本机没有 CmapTools 真机**：§3 的「B 可表达」仍然只是「写进了标准元素」，
   没有一条经过 CmapTools 打开验证。
2. **互通视图仍不表达**：`groups`、`pinned`、线色/虚线/线宽/命题注释、备注/标签/卡片、样式其余键、
   主题（`snow` 硬编码在降级路径）。这些只活在保真槽里。
3. ~~**保真槽的读取侧仍有缺口**（§7.5）：节点扩展字段写得出、读不回，修点在 `mw-format`，
   本轮没动。~~ **已修（2026-11-02）**：`mw_format::nodes_from_file_doc` 按 id 并回镜像扩展字段，
   节点扩展字段现在随保真槽往返。

---

## 8. 复跑方式（只读 + 只跑测试）

```powershell
# 单测（本轮修码后实跑；原始输出见 evidence-cargo-test-mw-io-fix.txt）
$env:TEMP="C:/Users/douha/Pictures/思维导图rust/target/tmp"
$env:TMP=$env:TEMP
cargo test -p mw-io -- --test-threads=1

# 只跑 .cxl 相关
cargo test -p mw-io cxl -- --test-threads=1
cargo test -p mw-io save_load_roundtrip_xmind_and_cxl -- --test-threads=1
```

> 本轮**没有**跑 `cargo build --release` 与 `cargo golden`（会与并行会话抢构建锁）。
> 所以本包不含像素基线，`status` 记 `进行中`。
