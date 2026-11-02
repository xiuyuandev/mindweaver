# M19 · 全局搜索命中备注与链接文本（打磨轮 41）

- 日期：2026-10-31
- 依据：`docs/02` §4「查找」P0 ——「全文匹配节点标题+备注+链接文本；命中高亮并编号
  （3/17）；`Enter/Shift+Enter` 上/下一个；支持全拼/首字母、多音字（`docs/07` §7）」；
  `docs/06` §125 搜索条现状。
- 状态：**已实现**（机器门绿；实机键盘/IME 手感未核，人工门未开，不记 `已验证`）

## 1. 现状（代码复核，先于改动）

搜索条（`Ctrl+F`）的检索入口是 `Document::search`（`crates/mw-core/src/document.rs:2582`），
它只做一件事：

```text
self.search.hits(&self.nodes, &order, query)   // → crates/mw-core/src/search.rs:256
```

`SearchIndex::hits` 里逐节点只读 `node.text`（`search.rs:279` 原文、`:294` 拼音），
**备注 `node.note` 与链接 `node.link` 一次都没读**。壳层 `main.rs:2038` 的 `lookup` 另外
接了卡片 / 标签 / 路径三组结果，也同样没有「节点备注/链接」这一路。

后果（与 `docs/02` P0 的「全文匹配标题+备注+链接文本」直接冲突）：

- 备注里写着「没有 WebView，没有 HTML」的节点，搜 `html` **一条都不出来**——
  用户明明记得自己写过，却只能去逐个节点翻备注；
- 链接 `https://wgpu.rs` 同理，搜域名片段找不回挂链接的节点；
- 命中高亮与计数 `3/17` 因此也只对标题成立。

复现（改前，`MW_SEARCH_DEMO=1` 给「纯 Rust 原生」喂备注与链接，输入脚本只做
`Ctrl+F` + `text html` / `text https`）：

```text
脚本：wait 6 · key ctrl+f · wait 3 · text html · wait 6
改前截图：target/x100/M19/before-note/…png
          target/x100/M19/before-link/…png
画面：   搜索条里是「没有找到匹配的主题 / 试试全拼或首字母，如 xiangmu、xm」
```

## 2. 范围

1. **检索范围补全**（`mw-core::search`）：
   - 每个节点除 `text` 外再匹配 `note`（备注）与 `link`（链接）两个字段；
   - 三字段都支持「原文子串 / 全拼 / 首字母」，多音字规则不变；
   - 每个节点只出一条结果（取该节点三个字段里得分最高的一条），排序仍是「分数降序，
     同分按文档前序」。
2. **结果行的可读性**（`mw-shell`）：
   - `SearchHit` 增加 `field`（`text` / `note` / `link`）与 `excerpt`（非标题命中时的
     展示原文，已含省略号且高亮区间已折算到摘录内）；
   - 结果行标题显示**命中字段的摘录**（不是节点标题），右上角角标显示「备注」/「链接」；
   - 定位行显示「节点 · 节点标题 · 祖先路径」，用户一眼能看出这条备注属于哪颗节点；
   - 点结果 / `Enter` 与现在一致：选中该节点、相机居中、呼吸外环；不自动展开备注。
3. **检索摘要**：`SearchHit.excerpt` 由纯函数 `excerpt_around` 生成——把换行折成空格、
   以命中区间为中心截取 `24 + 命中 + 24` 个字符，两端加 `…`，并把区间平移到摘录坐标系。
   否则一条 2000 字的备注命中在第 1800 字，结果行开头永远是「为什么搜出来的」看不见。
4. **分数分层**（同一节点取最高分；跨节点排序）：

   | 字段 | 原文 | 全拼 | 首字母 |
   | --- | --- | --- | --- |
   | 标题 | 1000 | 700 | 500 |
   | 备注 | 650 | 520 | 400 |
   | 链接 | 600 | 480 | 380 |

   口径：标题命中永远压过备注/链接同类型命中；原文命中压过拼音/首字母。
5. **长字段上限**：备注/链接的**拼音索引**只建到 240 个字符（`FIELD_PINYIN_CAP`）；
   超过的部分仍能原文命中。理由：20k 文档 × 长备注全量展开多音字组合会炸内存与首查耗时，
   而长备注里的拼音搜索价值远低于原文搜索。
6. **缓存**：`SearchIndex` 的拼音键缓存按字段分成 `cache` / `note_cache` / `link_cache`
   三张（键仍是节点 id，值存「字段文本快照 + 拼音键」）。字段文本一变就重建该字段的键，
   与现在的标题缓存同一条规则。

## 3. 非目标

- **不改 `Ctrl+H` 替换范围**：`docs/02` 把「替换节点标题文本」写成 P2，备注/链接替换不在本轮；
  搜索条 `ReplaceAll` 仍只改标题。
- 不索引关系线短语、标签、卡片正文（卡片与标签已由壳层另外两路结果覆盖；
  关系线短语属于边不是节点，要单独立项）。
- 不改 `Document` / `.zmind` 的任何字段、不做迁移；`note` / `link` 本来就在模型里。
- 不改搜索条的布局/尺寸/键盘归属（`↑↓`/`Enter`/`Shift+Enter`/`Esc`/IME 守卫原样）。
- 不把备注摘录做成多行：结果行仍是两行（标题 + 定位），行长与行高不变。
- 缓存内存不做全局封顶（与现有 `CACHE_LIMIT` 同款：超过上限整表清空）。

## 4. 验收条款

### 4.1 纯函数 / 索引（单测，`mw-core`）

| # | 用例 | 期望 |
| --- | --- | --- |
| 1 | 标题不含、备注含 `html` | 命中，`field=note`、`kind=text`，区间指回备注原文 |
| 2 | 标题不含、链接含 `https` | 命中，`field=link`、`kind=text` |
| 3 | 备注含「没有」、查 `meiyou` | 命中，`field=note`、`kind=pinyin` |
| 4 | 备注含「没有」、查 `my` | 命中，`field=note`、`kind=initials` |
| 5 | 同一节点标题与备注都含查询词 | 只出一条，`field=text`（标题压过备注） |
| 6 | A 节点标题命中、B 节点备注原文命中 | A 排在 B 前 |
| 7 | 备注超 240 字、查询词在第 300 字 | 原文仍命中（拼音不建索引） |
| 8 | 备注改成另一段文字后再查旧词 | 不再命中（缓存随字段文本失效） |
| 9 | `excerpt_around` 命中在开头 | 无前缀省略号、区间从 0 起 |
| 10 | `excerpt_around` 命中在中部/尾部 | 两侧/单侧有 `…`、区间平移正确 |
| 11 | 备注含换行、命中在换行之后 | 换行折成空格、区间仍落在命中字上 |
| 12 | 空备注 / 纯空白备注 | 不产生 note 命中 |

### 4.2 端到端（轨迹 + 金样）

- `search-note`：`MW_SEARCH_DEMO=1 MW_NOTES=none`，脚本 `Ctrl+F` → `text html`。
  截图里结果行标题是备注摘录（`HTML` 有暖黄底），右上是「备注」，定位行是
  「节点 · 纯 Rust 原生 · 织思」；轨迹 `search_hits q=html rows=1 note=1 link=0`。
- `search-link`：同一夹具，脚本 `Ctrl+F` → `text https`。
  结果行标题 `https://wgpu.rs`，角标「链接」；轨迹 `rows=1 note=0 link=1`。
- 两张 `--tol 0` 双跑；stage-b 门加两场。
- 与改前截图对照：改前同脚本画面是「没有找到匹配的主题」，改后多出结果行。

### 4.3 既有基线零漂移

改动只落在「检索命中集合」与结果行绘制；现有 121 场 stage-b / 56 场 stage-cm
没有一场打开搜索条，理论上全部零重拍。若有一张漂移，先查原因再重拍。

## 5. 预计证据

| 类型 | 内容 |
| --- | --- |
| 单测 | 上表 12 条验收 → 11 条测试（`mw-core::search`；`excerpt_around` 三条） |
| 轨迹 | `search_hits q=… rows=… note=… link=…`、`golden_input` 全帧 |
| 金样 | `search-note` / `search-link`（`--tol 0` 双跑） |
| 像素 | 改前（无结果）vs 改后（结果行）同脚本对照；两张之间只差结果行 |
| 复现 | `target/x100/M19/before-note|before-link/` 两张改前截图 |

## 6. 实现（2026-10-31，先报告后代码）

### 6.1 `mw-core::search`

- `SearchHit` 增加两列：`field`（`text` / `note` / `link`，常量 `FIELD_TEXT/NOTE/LINK`）
  与 `excerpt: Option<SearchExcerpt>`。`range` 仍是**字段内**字符区间（备注/链接命中时指回
  备注/链接原文，不是摘录），`SearchExcerpt` 自带 `text` 与折到摘录坐标系的 `range`——
  两个坐标系分开存，测试才能一条断言「指回原文」、另一条断言「高亮在摘录里」。
- `excerpt_around(text, range)`：换行一换一折成空格（字符下标不漂）、命中前后各 24 字
  （`EXCERPT_CONTEXT`）、只有真截断才加 `…`、区间平移时把前缀 `…` 的 1 个字符算进去。
- 拼音缓存从一张拆成三张（`cache` / `note_cache` / `link_cache`），键仍是节点 id，
  值仍是「字段文本快照 + 拼音键」；`cache_keys()` 统一做快照比较与重建。
  备注/链接超过 240 字（`FIELD_PINYIN_CAP`）直接返回 `None`，只做原文匹配。
- `field_hit()` 把「原文 / 全拼 / 首字母」三档分数抽成 `base: [i32; 3]`：
  标题 `[1000, 700, 500]`、备注 `[650, 520, 400]`、链接 `[600, 480, 380]`，
  prefix/exact 加成与位置扣分沿用原规则。
- `hits()` 逐节点按「标题 → 备注 → 链接」顺序 offer，**同分保留先来的**（标题优先）；
  最终胜出的备注/链接命中才生成摘录。空备注 / 纯空白备注直接跳过。

### 6.2 `mw-shell`

- `main.rs` 的结果映射：备注/链接命中的首行用 `hit.excerpt.text`、高亮用
  `excerpt.range`；第二行路径拼成「节点标题 · 祖先路径」（顶层节点只有标题）；
  标题命中保持原样（首行节点文本、第二行祖先路径）——既有搜索画面因此零漂移。
- `panels/search.rs` 的 `HitRow` 增加 `field`；右上角角标按「字段角标 + 命中类型角标」
  依次右对齐绘制（备注命中 + 全拼 = 「备注 拼音」），只有一枚时占位仍是原来的 52px。
- `MW_SEARCH_DEMO=1` 通过 `Command::SetNote` / `Command::SetLink` 给样例节点
  「纯 Rust 原生」写入备注（含 `HTML`）与链接（`https://wgpu.rs`）；
  `MW_NOTES=none` 关掉本机卡片库，避免真实笔记里的命中污染基线。
- `MW_TRACE` 新增一行：`search_hits q=… rows=… note=… link=…`，只在结果重算时写。

### 6.3 金样 / 脚本

- 新基线 `shots/golden/search-note`（sha256 前 12：`1487137406A2`）与
  `shots/golden/search-link`（`F220A31D72A9`），`ppp 1.0`，输入脚本 23 帧。
- `scripts/x100-stage-b-gate.ps1` 增加两场；每场由 `Run-GoldenScene` 分配隔离 APPDATA
  （`target/ci-appdata/gate-search-*`），不会去写用户真实的 `%APPDATA%\织思`。

## 7. 证据（机器）

### 7.1 单测

`cargo test -p mw-core search`：**20 passed / 0 failed**（含本轮 11 条，覆盖 §4.1 的 12 条验收：
备注原文/链接/备注全拼/备注首字母/标题压过备注/跨节点排序/超 240 字原文仍命中/
缓存随字段文本失效/摘录开头/摘录中部/换行折叠/空白备注）。

### 7.2 轨迹

```text
event search_hits q=html rows=1 note=1 link=0
event search_hits q=https rows=1 note=0 link=1
```

（`target/x100/M19/trace-note.txt`、`trace-link.txt`）

### 7.3 像素

同一脚本、同一夹具，改前 vs 改后：

| 场景 | 差异像素 | 包围盒 | 说明 |
| --- | --- | --- | --- |
| `search-note` | **29 422** | `x515..1106 y80..221` | 只在搜索卡片内：空状态 → 一行摘录 |
| `search-link` | **27 349** | `x515..1106 y80..221` | 同上 |

画布与其余铬件 0 px 差异——改动没有溢出结果行。

### 7.4 门

- `stage-b`：**PASS**，2026-10-31 `03:17:23–03:26:07`（含 fmt / mw-shell clippy
  子集 / 全量测试 1041 / 两遍 golden），**123 场 / 246 次比较**，pass 1 / pass 2
  各 123 比较全过，`MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`。
  日志 `target/x100/M19/stage-b-gate.log`。
- `stage-cm`：**PASS**，2026-10-31 `03:26:19–03:29:39`（fmt / mw-shell clippy 子集 /
  概念图 `.zmind` 往返 / 全量测试 1041 / 两遍 golden），**56 场 / 112 次比较**，
  pass 1 / pass 2 各 56 比较全过，`MACHINE GATE PASS; HUMAN G-CM-2 REVIEW IS STILL REQUIRED`。
  日志 `target/x100/M19/stage-cm-gate.log`。

### 7.5 既有基线零漂移

`stage-b` 登记 121 → **123** 场，既有 121 场两遍全过、零重拍；新增两场
`search-note` / `search-link` 为 `--tol 0` 双跑。`stage-cm` 56 场顺序复跑
（见 §7.4），本轮没有概念图代码改动。

## 8. 未做 / 风险

- **`Ctrl+H` 替换仍只改标题**：`ReplaceAll` 用 `replace_all` 逐节点替换 `node.text`；
  备注/链接替换不在本轮（`docs/02` 把它归 P2）。
- **关系线短语 / 标签 / 卡片正文不进节点检索**：卡片与标签已由壳层另外两路结果覆盖；
  关系线是边不是节点，要单独立项。
- **摘录固定单行**：结果行仍是两行、行高 44 不变；长备注只看命中处 ±24 字。
- **超过 240 字的备注/链接不做拼音匹配**：原文仍可命中；这是内存/首查耗时的取舍，
  已写进 `docs/07` §7。
- **实机 IME / 键盘手感未核**：与 M4 同一缺口；金样只走注入事件。
- **20k 文档 × 全备注首查成本未单独计时**：拼音键有 240 字上限、缓存按字段拆分；
  原文匹配仍是每节点一次，长备注文档的输入延迟没有实测数据。

