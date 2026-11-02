# M29 · 搜索匹配方式可切换（精确文本 / 拼音）（打磨轮 52）

- 日期：2026-10-31
- 依据：`docs/02` §3「查找」P0 行的验收条款——
  「**匹配方式可切换"精确文本/拼音"**；命题命中显示「概念 · 源 → 目标」并可跳转选中该边；
  命中折叠子树里的节点时先展开挡路祖先再跳转…」。
- 代码：`crates/mw-shell/src/panels/search.rs`（`SearchBar`、脚注、`handle_keys`）、
  `crates/mw-core/src/search.rs:460`（`SearchIndex::hits`）、`:547`（`relationship_hits`）、
  `crates/mw-core/src/document.rs:2628`（`Document::search`）、
  `crates/mw-shell/src/canvas/canvas_methods_1.rs:746`（`search_nodes` / `search_relationships`）、
  `crates/mw-shell/src/note_store.rs:252`（卡片检索）。
- 状态：**已实现**（报告与 manifest 齐；机器门绿；人工门未开）

## 1. 现状（代码复核，先于改动）

1. `SearchBar` 只有查询词，没有匹配方式的任何状态；`lookup(q)` 永远把同一个查询词交给
   `Document::search` / `search_relationships` / `notes.search`。
2. `SearchIndex::hits` 的 `pinyin_eligible` 只看查询词是不是 ASCII 字母数字：
   是就同时查原文、全拼、首字母，不是就只查原文。**用户无法选择**「只要原文」——
   搜 `xiangmu` 时原文里没有这四个字母，却会命中「项目」；反过来想精确找
   `xm` 这个字面串时，拼音命中会混进来。
3. `docs/02` 的这条 P0 验收从 10-18 起就存在，代码里从未有过开关；
   M19/M21/M22 补了备注/链接、替换、聚焦跳转，但都没有碰匹配方式。
4. `Ctrl+H` 替换只接受标题字面命中（`replace_target`），所以匹配方式只影响
   检索结果，不影响替换语义——但切到「精确文本」后，拼音命中消失，替换入口自然只剩字面行。

## 2. 范围

1. **`mw-core` 匹配方式**：`pub enum MatchMode { Pinyin, Text }`（`Default = Pinyin`，
   即现状：原文 + 全拼 + 首字母；`Text` 只做原文子串，大小写不敏感）。
   `SearchIndex::hits` 与 `relationship_hits` 增加 `mode: MatchMode` 参数；
   `MatchMode::Text` 时**不建也不读**拼音键缓存（`pinyin_eligible && mode == Pinyin`），
   原文分支原样保留。`Document::search` / `search_relationships` 保持现状签名并转发
   `MatchMode::Pinyin`；新增 `search_with_mode` / `search_relationships_with_mode`
   给搜索条用。
2. **壳层**：`Canvas::search_nodes` / `search_relationships` 增加 `mode` 参数；
   `NoteStore::search` 增加 `mode`（卡片标题沿用同一个索引）；
   标签与路径本来就是字面匹配，不受影响。
3. **搜索条 UI**：脚注右侧加两段式开关「精确文本 / 拼音」（每段约 46–52px，
   选中段 8% 强调底 + 强调色文字，与检查器作用域开关同一套语言）；
   点击立即重查并在当前行尽量留在同一结果上；脚注左侧提示不变，`Esc 关闭` 保留在最右。
4. **键盘**：`Ctrl+Shift+M` 在搜索条内切换匹配方式（与 `Ctrl+H` 同一层，排在输入法组合判定之后）。
   **原计划用 `Ctrl+P`，实测两次都行不通**：`shortcuts::DEFAULTS` 里 `Ctrl+P` 是「导出 PDF」，
   `window_shortcuts()`（`main.rs:1448`）排在 `search.handle_keys()`（`main.rs:1509`）之前；
   第一版按 `Ctrl+P` 直接弹出导出对话框（轨迹 `overlay-double 2 Export`，没有 `search_mode`）。
   改成 `Ctrl+Shift+P` 仍是同样结果——egui 的 `matches_logically` 允许**额外**按住 Shift，
   `Ctrl+Shift+P` 同样满足 `Ctrl+P` 的模式（`NewConcept` 先于 `New` 判也是这个原因的反向用法）。
   最终选 `Ctrl+Shift+M`：默认表里没有 `Ctrl+M`，这条不会在窗级被抢走。
   轨迹 `search_mode mode=text|pinyin rows=<n>`，同时保留既有 `search_hits …` 行。
5. **默认与零漂移**：默认仍是「拼音」（现状），所以结果集不变；但脚注多出开关，
   既有**八张**搜索基线（stage-b 6 张：`search-note`/`search-link`/`search-collapsed`/
   `search-replace`/`search-replace-one`/`search-focus-jump`；stage-cm 2 张：
   `search-rel`/`search-rel-jump`）会**有意重拍**——差异只应在脚注右侧；
   另新增 `search-text` 一张，合计九张搜索基线。
6. **单测**：
   - `text_mode_ignores_pinyin_and_initials`（`xm` / `xiangmu` 在 Text 下查不到「项目」，原文 `项目` 能查到）；
   - `text_mode_still_matches_notes_links_and_relationships`（字段原文命中不受影响）；
   - `text_mode_does_not_build_pinyin_keys`（同一索引先 Text 查 `xm` 再 Pinyin 查，结果正确且缓存按需建立）；
   - `search_mode_defaults_to_pinyin_and_is_case_insensitive`（Text 下 `HTML`/`html` 同结果）；
   - `search_bar_toggles_mode_from_the_keyboard`（纯逻辑：`toggle_mode` 往返，重置当前行）。
7. **金样**：新增 `search-text`（`MW_NOTES=none` + 输入脚本 `Ctrl+F → 输入 zw → Ctrl+Shift+M`；
   `zw` 是样例文档「中文排版」的首字母，拼音模式命中 1 行，精确文本下原文没有这两个字母、
   结果归零、脚注开关高亮「精确文本」）；既有八张搜索基线按新脚注重拍。
8. **文档**：`docs/02` 该验收行改口径（已实现 + 证据指针）；账本 M29 行；
   `docs/STATUS.md`、执行日志、`AGENTS.md`、`HANDOFF.md` 回填。

落地位置：`crates/mw-core/src/search.rs`（枚举 / 两个入口 / 单测）、
`crates/mw-core/src/document.rs`（转发 + 单测）、
`crates/mw-shell/src/panels/search.rs`（状态 / 开关 / 键盘 / 轨迹）、
`crates/mw-shell/src/main.rs`（lookup 传 mode）、
`crates/mw-shell/src/canvas/canvas_methods_1.rs`、`crates/mw-shell/src/note_store.rs`、
`scripts/x100-stage-b-gate.ps1` 与 `shots/golden/inputs/search-text.txt`。

## 3. 非目标

- 不改标签 / 路径检索（它们本来只有字面匹配）。
- 不做「全词匹配 / 正则 / 大小写敏感」第三档；本轮只有两档，与 `docs/02` 的措辞一致。
- 不改替换语义：`Ctrl+H` 仍只换标题字面命中。
- 不改默认值：出厂仍是「拼音」，既有用户升级后结果集不变。
- 不做匹配方式持久化（它是单次搜索的临时选择，关掉搜索条即回默认）。
- 不动搜索排序 / 分组 / 摘录 / 跳转逻辑。

## 4. 验收条款

### 4.1 单元

| # | 用例 | 期望 |
| --- | --- | --- |
| 1 | Text 模式搜 `xm` / `xiangmu` | 不命中「项目」 |
| 2 | Text 模式搜 `项目` / `HTML` / `html` | 命中且大小写不敏感 |
| 3 | Text 模式 | 备注、链接、命题短语的原文命中仍生效 |
| 4 | Text 后切 Pinyin | `xm` 立即命中；两次查询共用同一索引 |
| 5 | 搜索条状态 | `toggle_mode` 往返、切换后当前行清空等待重查 |

### 4.2 端到端

- `MW_NOTES=none` + 输入脚本：Ctrl+F → 输入 `zw`（拼音命中 1 行）→ `Ctrl+Shift+M` 切「精确文本」→
  结果区显示「没有找到匹配的主题」，脚注开关高亮在「精确文本」；轨迹 `search_mode mode=text rows=0`
  且 `search_hits q=zw rows=0`。
- 切回「拼音」：同一次运行再按一次 `Ctrl+Shift+M` 应恢复 `search_hits q=zw rows=1`；
  `Enter` 仍能跳出并定位（复用 M20/M22 路径）。
- 既有八张搜索基线重拍后，差异只落在脚注右侧开关区域（逐像素 bbox 记账）：

  | 场景 | 差异像素 | 占比 | 包围盒（1.0 ppp） |
  | --- | --- | --- | --- |
  | `search-note` | 2028 / 1749600 | 0.116% | x[915..1016] y[162..181] |
  | `search-link` | 2028 / 1749600 | 0.116% | x[915..1016] y[162..181] |
  | `search-collapsed` | 2028 / 2304000 | 0.088% | x[1065..1166] y[162..181] |
  | `search-replace` | 2028 / 2304000 | 0.088% | x[1065..1166] y[332..351] |
  | `search-replace-one` | 2028 / 2304000 | 0.088% | x[1065..1166] y[210..229] |
  | `search-focus-jump` | 2028 / 2304000 | 0.088% | x[1065..1166] y[162..181] |
  | `search-rel` | 2028 / 2304000 | 0.088% | x[1065..1166] y[162..181] |
  | `search-rel-jump` | 2028 / 2304000 | 0.088% | x[1065..1166] y[162..181] |

  八张都是同一个约 101×20 的开关区域，卡片其余部分与画布 0 像素差；
  重拍后逐张 `--tol 0` 复跑零差异。

### 4.3 证据

`shots/x100/M29/`：`report.md`、`manifest.json`、
`search-before.png`（改前脚注）、`search-after-text.png`（改后精确文本空结果）、
`search-after-pinyin.png`（切回拼音命中）、`trace-mode.txt`、
金样在 `shots/golden/search-text/`。

## 5. 风险 / 待实测

- 脚注高度 28px：两段开关按 20px 高、圆角 5px 画，不挤压左侧提示；
  560px 宽下「↑↓ 切换 · Enter 定位 · Shift+Enter 反向」约 250px，右侧开关 + Esc 约 170px，
  中间余量足够；窄窗（`width` 取 `central.width()-24`，最小 240）时右侧可能挤压，
  需要按可用宽度决定是否缩成「文本 / 拼音」两字。
- 键位已实测纠偏：`Ctrl+P` 是全局「导出 PDF」，搜索条抢不到（`window_shortcuts` 先消费）；
  `Ctrl+Shift+P` 因 egui 逻辑匹配允许额外 Shift、仍会命中 `Ctrl+P` 的模式，同样不行。
  最终 `Ctrl+Shift+M`（无全局 `Ctrl+M`）轨迹正常。后续若把切换键做进快捷键设置，需要让搜索条在
  `window_shortcuts` 之前拿到键盘，或给这条动作单独开一个「局部绑定」口子。
- 输入法组合期 `Ctrl+Shift+M` 归输入法（沿用组合期挂起）。
- 切到「精确文本」后拼音键不再建立，切回时首查有一次性重建成本；
  20k 文档的首次重建延迟本轮只做逻辑验证，不承诺毫秒数。
- 实机 IME / 键盘手感不在本轮范围。
