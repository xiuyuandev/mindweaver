# C17 · 概念图命题短语进全局检索（打磨轮 42）

- 日期：2026-10-31
- 依据：`docs/30` §5.1/§5.4 —— 概念图是**命题网络**，边是「有向且带链接短语」的命题，
  短语就是这条边的语义正文；`docs/02` §4「查找」的全文匹配口径；
  M19 报告 §3 把「关系线短语不进节点检索，要单独立项」列为未做项。
- 状态：**已实现**（机器门绿；人工门未开，不记 `已验证`）

## 1. 现状（代码复核，先于改动）

搜索条（`Ctrl+F`）的检索入口仍是 `Document::search` → `SearchIndex::hits`
（`crates/mw-core/src/search.rs:303` 的 `pub fn hits`）。M19 之后它读节点的三个字段：

```text
node.text / node.note / node.link
```

**关系线（命题）一次都没读**。`Node.relationships`（`crates/mw-core/src/model.rs:81`
`pub struct Relationship`）里的 `label` 是概念图唯一必填的语义正文，但：

- 画布/大纲里写着「产品定位 —支撑→ 技术方案」的命题，搜 `支撑` **一条都不出来**；
- 结果只能落在节点上（`Group::Node`），即使搜到短语也说不清是哪条边；
- `docs/30` §5.4 的短语编辑（双击就地改、空串拒绝）做得越完整，
  「写完却搜不到」的缺口越明显。

复现（改前）：`MW_CONCEPT_DEMO=1` 打开示例概念图（模板里有一条 `产品定位 → 技术方案`
的命题，短语「支撑」），脚本 `Ctrl+F` → `text 支撑`：

```text
画面：搜索条里是「没有找到匹配的主题 / 试试全拼或首字母，如 xiangmu、xm」
改前轨迹：search_hits q=支撑 rows=0 note=0 link=0
改前截图：target/x100/C17/before/search-rel.png（整窗）、
          target/x100/C17/before/search-rel/shell-ppp1.00.png（1920×1200）
```

## 2. 范围

1. **检索命题短语**（`mw-core::search`）：
   - 新增 `RelationshipHit { from, id, to, score, kind, range, excerpt }`；
     身份是「源节点 id + 边 id」，`range` 指回**短语原文**，`excerpt` 是 ±24 字摘录；
   - 新增 `SearchIndex::relationship_hits(nodes, order, query)`：逐节点按文档前序，
     再按节点内 `relationships` 顺序扫；只检索 `label` 非空白的边；
   - 原文 / 全拼 / 首字母三档都支持，多音字规则与节点检索同一套；
   - 排序：分数降序，同分按源节点的文档前序 + 边在节点内的下标；
   - 分数分层：原文 560 / 全拼 450 / 首字母 350，位置扣分与 prefix/exact 加成沿用；
     与节点三字段分开排，壳层用分组表达先后，不靠跨类分数压人。
2. **结果行与跳转**（`mw-shell`）：
   - `Group` 增加 `Relationship`（标签「命题」），排在 `Node` 之后、
     `Card` 之前；现有卡片/标签/路径三组的顺序不变；
   - `HitRow` 增加 `rel: Option<RelRow>`（源节点 id + 边 id）；
     `Action` 增加 `JumpRelationship { from, id }`，点行 / `Enter` 都走它；
   - 结果行首行显示短语摘录（暖黄高亮命中字），右上角角标「命题」+ 命中类型；
   - 第二行显示「概念 · 源概念 → 目标概念」，回答「这条命题连着谁」；
   - `JumpRelationship` 调用 `Canvas::select_relationship_keep_focus(from, id)`：
     选中该边（沿用既有 `select_relationship` 的互斥：清节点选择、清注解选择），
     相机居中到两端概念盒中心的中点，**不抢键盘**（搜索框还开着，`Enter` 继续走下一行）。
3. **缓存**：`SearchIndex` 增加第四张拼音缓存 `rel_cache`，键是 `(from, id)`，
   值是「短语快照 + 拼音键」；短语一变就重建。超过 240 字（`FIELD_PINYIN_CAP`）
   只做原文匹配。`CACHE_LIMIT` 的整表清空同时覆盖四张缓存。
4. **轨迹**：`search_hits` 行追加 `rel=N`；跳转时写
   `search_rel_jump from=… id=…`，证明点击/回车真的选中了那条边。
5. **金样**：
   - `search-rel`：`MW_CONCEPT_DEMO=1`，`Ctrl+F` → `text 支撑`，
     结果行是「支撑」+ 角标「命题」+ 第二行「概念 · 产品定位 → 技术方案」；
   - `search-rel-jump`：同脚本 + `Enter`，画布出现该命题的选中环（相机居中），
     轨迹 `search_rel_jump from=<产品定位 id> id=<边 id>`。

## 3. 非目标

- **不把命题命中混进节点行**：关系线是边不是节点，不改变 `SearchHit` 的节点语义；
  `Ctrl+H` 替换仍只改节点标题，不改短语（`docs/02` 把替换范围写成 P2）。
- 不索引关系线的颜色、箭头方向、边界/概要/注解；只检索短语。
- 不改节点三字段的分数、摘录、缓存与结果行几何；`search-note` / `search-link`
  的像素基线不应漂移（轨迹行会多 `rel=0`，轨迹不进像素基线）。
- 不改搜索条宽度/行高/快捷键/IME 守卫；新增分组只在有命题命中时出现。
- 不做「只整理选中」、不做节点/关系线之外的第三种选中态。
- 不改 `.zmind` / `Node` / `Relationship` 字段，不做迁移。

## 4. 验收条款

### 4.1 纯函数 / 索引（单测，`mw-core`）

| # | 用例 | 期望 |
| --- | --- | --- |
| 1 | 短语「解决」、查 `解决` | 命中，`kind=text`，`range=[0,2]`，`from/to` 指对两端 |
| 2 | 短语「解决」、查 `jiejue` | 命中，`kind=pinyin` |
| 3 | 短语「解决」、查 `jj` | 命中，`kind=initials` |
| 4 | 短语为空 / 纯空白 | 不产生命题命中 |
| 5 | 短语改成另一段文字后再查旧词 | 不再命中（缓存随短语文本失效） |
| 6 | 短语超 240 字、查询词在第 300 字 | 原文仍命中，拼音不建索引 |
| 7 | 两条命题同分 | 按源节点文档前序、再按边下标稳定排序 |
| 8 | 长短语命中在中部 | 摘录两侧有 `…`、区间折到摘录坐标系 |
| 9 | 短语含换行 | 折成空格、区间仍落在命中字上 |

### 4.2 壳层（单测，`mw-shell`）

| # | 用例 | 期望 |
| --- | --- | --- |
| 10 | 命题行 `action_for` | 产出 `JumpRelationship { from, id }` |
| 11 | 分组排序 | `Node < Relationship < Card < Tag < Path` |

### 4.3 端到端（轨迹 + 金样）

- `search-rel`：`MW_CONCEPT_DEMO=1`，`Ctrl+F` → `text 支撑`。
  截图里结果行首行「支撑」（命中字暖黄底），右上角「命题」，第二行
  「概念 · 产品定位 → 技术方案」；轨迹
  `search_hits q=支撑 rows=1 note=0 link=0 rel=1`。
- `search-rel-jump`：同脚本 + `Enter`。画布上该命题出现选中环，轨迹
  `search_rel_jump from=… id=…`；与 `search-rel` 的差异只应出现在画布选中环
  与相机位置（两张各拍一张，`--tol 0` 双跑）。
- 两场进 stage-cm 门（概念图夹具），`--tol 0` 双跑零差异。
- 与改前截图对照：改前同脚本是「没有找到匹配的主题」，改后多出命题结果行。

### 4.4 既有基线零漂移

- 只改「检索命中集合 + 壳层结果组」；现有 123 场 stage-b / 56 场 stage-cm
  没有一场打开搜索条命中命题。若有一张漂移，先查原因再重拍。

## 5. 证据

| 类型 | 内容 |
| --- | --- |
| 单测 | 上表 11 条（`mw-core::search` 9 条、`mw-shell` 2 条） |
| 轨迹 | `search_hits … rel=N`、`search_rel_jump from=… id=…`、`golden_input` 全帧 |
| 金样 | `shots/golden/search-rel`（150 140 B）、`shots/golden/search-rel-jump`（150 397 B） |
| 像素 | 改前（无结果）vs 改后（命题结果行 + 选中环）同脚本对照 |
| 复现 | `target/x100/C17/before/…png` 改前截图；轨迹 `target/x100/C17/trace-rel*.txt` |

## 6. 实测（2026-10-31 收工回填）

- 单测：`mw-core::search` 29 条通过（含新增 9 条）；`mw-shell` 搜索面板 18 条通过
  （含新增 2 条）；全量工作区 **1052 passed / 0 failed / 1 ignored**（串行，
  `TEMP`/`TMP` → `target/tmp`；mw-core 273 + contrast 7，mw-shell 595）。
- 静态门：`cargo fmt --all -- --check` 干净；`cargo clippy --workspace --all-targets`
  指纹 **36/36，新增 0**（与 `scripts/clippy-baseline.txt` 比对）。
- 轨迹：改前 `search_hits q=支撑 rows=0 note=0 link=0`；改后
  `search_hits q=支撑 rows=1 note=0 link=0 rel=1`；`Enter` 后
  `search_rel_jump from=01a18e74-226a-741f-bfbc-302d122ab93e
  id=01a18e74-226a-741f-bfbc-3035d6d707b5 ok=1`，轨迹里焦点仍是 `search-input`
  （搜索框没被画布抢走）。
- 像素：改前 vs `search-rel` 差 **27 216 px**，包围盒 x665..1256 / y80..221
  （只在结果卡片内）；`search-rel` vs `search-rel-jump` 差 **71 547 px**，
  包围盒 x210..1505 / y80..966（画布选中环 + 相机居中）。
- 机器门：stage-cm **PASS**，2026-10-31 `04:31:44–04:36:32`，**58 场 / 116 次比较**
  零差异（`target/x100/C17/stage-cm-gate.log`）；stage-b **PASS**，
  `04:36:37–04:43:01`，**123 场 / 246 次比较**零差异
  （`target/x100/C17/stage-b-gate.log`）。既有 56 场概念图与 123 场 stage-b
  没有一张漂移，`search-note` / `search-link` 也未动。

## 7. 未做 / 风险

- 实机 IME / 键盘手感未核（与 M4 同缺口）。
- 20k 概念图 × 大量命题的首查耗时未实测；先按 240 字上限与缓存控制。
  缓存只省拼音键构造，每帧仍会扫一遍全部边。
- 命题结果不参与 `Ctrl+H` 替换；替换范围仍是节点标题。
- 关系线短语的「精确文本/拼音模式切换」不做——搜索条按 `docs/06` 不切模式。
- 命题组没有单独的空状态/折叠；与节点组共用同一个结果列表。

