# M21 · Ctrl+H 逐个替换与替换预览（打磨轮 44）

- 日期：2026-10-31
- 依据：`docs/02` §4「查找替换」P2——「替换节点标题文本，**逐个/全部**；**替换预览**」；
  `docs/06` 搜索条（`Ctrl+F`）一节的同族交互。
- 状态：**已实现**（机器门绿；人工门未开，不记 `已验证`）

## 1. 现状（代码复核，先于改动）

`Ctrl+H` 的替换行早已存在（`panels/search.rs`）：输入框 + 「全部替换」按钮。
逐条对照 `docs/02` §4 的三项要求：

| 要求 | 改前 | 缺口 |
| --- | --- | --- |
| 替换节点标题文本 | `SA::ReplaceAll` 用 `search_nodes` 取标题命中，`replace_all` 只做字面替换 | 只覆盖「全部」 |
| 逐个替换 | 无按钮、无快捷键；`Ctrl+Enter` 是「全部替换」（焦点在替换框时） | **缺** |
| 替换预览 | 无：替换框里敲字，结果行一个字都不变 | **缺** |

另外两处手感问题：

- `Ctrl+H` 展开替换行后，焦点仍留在**搜索框**（`self.grab = true` 的既有行为），
  用户打完 `Ctrl+H` 直接敲字，字进了搜索词——多数编辑器是焦点进替换框。
- 结果行看不出「这条能不能替换」：备注/链接/拼音命中都在列表里，
  但 `replace_all` 对它们静默跳过（拼音没有可替换位置、备注/链接不在标题上）。

改前实测（`target/x100/M21/before/`）：同一脚本
`Ctrl+F → text 排版 → Enter → Ctrl+H → text 排布` 之后，替换行展开、替换框里是「排布」，
结果行仍是「中文排版」，**没有预览**；轨迹里也没有任何逐条替换事件（`trace.txt`）。

## 2. 范围

1. **纯函数**（`mw-core::search`）：
   `replace_char_range(text, range, to) -> Option<String>`——按**字符**区间替换
   `[start, end)`，越界 / `start > end` / 非字符边界返回 `None`；空 `to` = 删除命中段。
   与 `replace_all` 同一语义（只做字面替换，不做拼音替换）。
2. **替换行**（`panels/search.rs`）：
   - `Ctrl+H` 展开时焦点进**替换框**；再按 `Ctrl+H` 收起、焦点回搜索框。
   - 两个按钮：「替换」（逐个，宽 52）在左、「全部替换」（宽 72）在右。
   - 快捷键：`Ctrl+Enter` = 替换当前；`Ctrl+Shift+Enter` = 全部替换。
     原先「焦点在替换框时 `Ctrl+Enter` = 全部」的映射改掉——同一个键不再看焦点换语义。
   - **预览**：替换行展开且当前行是「节点标题 + 字面命中」时，结果行第二行
     `→ {替换后的标题}`（强调色）；其他行仍显示定位信息。
   - 逐个替换后当前行跟着走：被替换的**行仍命中**（同一节点还有第二处）就前进一行、
     到底绕回；**不再命中**就按原下标夹取。连续 `Ctrl+Enter` 不会卡在同一行反复替换。
3. **壳层**（`app_methods_2.rs`）：
   `SA::ReplaceOne { id, text }` → 一条 `Command::SetText`（一次撤销只回这一步）、
   轨迹 `search_replace_one id=… chars=…`、选中并居中该节点（**不抢键盘**，搜索框继续可用）；
   模型拒绝（例如超长）时 toast 而不是静默。选中前先 `node_visible`：节点不在当前视图
   （折叠/聚焦模式）就不动相机，避免“替换成功但视角被拽走”。
4. **可替换判定**只有一个口径（面板纯函数 `replace_target`）：
   `Group::Node` + `field == text` + `kind == text` + `range` 存在。
   备注/链接命中（字段不是 `text`）、拼音/首字母命中（`kind` 不是 `text`）、
   卡片/标签/路径行（不是节点标题）都不给替换入口。
5. **金样**：
   - `search-replace`（Warmup 58）：`Ctrl+F → text 排版 → Enter → Ctrl+H → text 排布`，
     锁住「替换行展开 + 当前行预览 `→ 中文排布` + 页脚新提示」。
   - `search-replace-one`（Warmup 68）：同一脚本再 `Ctrl+Enter`，锁住「标题已替换、
     相机居中、搜索框/替换行仍开着（焦点不跳回画布）、toast 已替换 1 处」。
6. **夹具**：不新增；用样例文档的「中文排版」（`排版 → 排布` 唯一标题命中）。
   替换后路径行也不再命中，结果归零显示「无结果」——正好验证空列表下按钮禁用。

## 3. 顺带修掉（同一轮验证中发现）

`card-autosave` 在 2026-10-31 08:44 的门里红了 56 像素：包围盒 `x[639..645] y[167..175]`，
最大差 92。根因不是 M21，而是自动保存把卡片「更新于」写成**墙钟**，而 `format_date`
按 UTC 切日——本地 08:00 一过，基线里的 `2026-10-30` 就变成 `2026-10-31`，门每天会红一次。
处理：`note_view::mark_saved` 改走 `card_now_ms()`，金样可用 `MW_CARD_NOW_MS` 钉住这一刻；
`card-autosave` 场景钉到演示时间戳 `1792800000000`（2026-10-24 UTC）后重拍基线。
产品行为不变（不设环境变量时仍读墙钟），只是金样不再随日期腐烂。

## 4. 非目标

- 不替换备注、链接、命题短语、卡片标题：`docs/02` 只要求标题；备注/链接命中在替换行里
  仍可跳转，只是没有替换入口。
- 不做正则、大小写敏感、整词匹配、范围内替换（当前/选中子树）等高级选项。
- 不改检索命中集合与排序；不做替换前的「命中列表预演」弹窗。
- 不改 `.zmind` / `Node` 字段；替换走既有 `SetText` 命令与撤销栈。
- 不把预览画到**所有**结果行：预览只跟当前行，避免替换行展开后整列都在闪。

## 5. 验收条款与结果

### 5.1 纯函数

| # | 用例 | 期望 | 结果 |
| --- | --- | --- | --- |
| 1 | 中文标题 `中文排版`，区间 `[2,4) → 排布` | `中文排布` | PASS |
| 2 | 空替换串 | 删除命中段（`中文排版 [2,4) → ""` = `中文`） | PASS |
| 3 | `start > end` / 越界 / 非字符边界 | `None`，不 panic | PASS |
| 4 | 首尾区间 | 前缀 / 后缀替换正确，其余字符原样 | PASS |

单测：`replace_char_range_replaces_a_cjk_slice`、`replace_char_range_rejects_invalid_ranges`。

### 5.2 可替换判定（面板）

| # | 行 | 期望 | 结果 |
| --- | --- | --- | --- |
| 1 | 节点标题 + 原文命中 | `Some(新标题)` | PASS |
| 2 | 节点标题 + 拼音/首字母命中 | `None`（没有可替换的字面区间） | PASS |
| 3 | 备注 / 链接命中 | `None`（字段不是标题） | PASS |
| 4 | 卡片 / 标签 / 路径 / 命题行 | `None` | PASS |

单测：`replace_target_only_accepts_title_literal_hits`、
`replace_current_refuses_pinyin_and_note_rows`。

### 5.3 当前行推进

| # | 用例 | 期望 | 结果 |
| --- | --- | --- | --- |
| 1 | 被替换行不再命中 | 按原下标夹到剩下的行 | PASS |
| 2 | 同一节点还有第二处（行仍命中） | 前进到下一行 | PASS |
| 3 | 已在末行 | 绕回第一行，不卡住 | PASS |
| 4 | 结果顺序变了 | 按 id 找新位置算「下一行」，不信旧下标 | PASS |
| 5 | 改查询词 | 清掉待推进状态 | PASS |

单测：`replace_current_emits_one_command_and_parks_the_next_row`、
`settle_after_refresh_moves_past_the_replaced_row_and_wraps`、
`replace_current_starts_at_the_first_row_and_clears_on_query_change`。

### 5.4 端到端（轨迹 + 金样）

- 轨迹（`trace.txt`）：`Ctrl+H` 后 `focus search-replace-input`；`Ctrl+Enter` 后
  `search_replace_one … chars=4 before_chars=4`（记录的是整条标题的字符数）、
  `toast 已替换 1 处`、`search_hits q=排版 rows=0`；**没有** `focus canvas`。
- 改前（`target/x100/M21/before/`）：同脚本无 `search_replace_one`，替换框里的字只影响
  下一次查询，结果行不出现 `→ …` 预览。
- 金样：`search-replace` / `search-replace-one` 进 `stage-b` 门，`--tol 0` 两遍零差异。
- 像素：改前 → `search-replace` 差 **3964 px（0.1720%）**，包围盒 `(697,72)–(1228,348)`
  只在搜索卡片内；`search-replace` → `search-replace-one` 差 **78247 px（3.3961%）**，
  覆盖结果列表、画布标题、相机与 toast（相机居中导致整幅重排，属预期）。

### 5.5 键盘与焦点

- `Ctrl+H` 展开 → 替换框获得焦点；收起 → 搜索框获得焦点。
- `Ctrl+Enter` = 替换当前，`Ctrl+Shift+Enter` = 全部替换；焦点在哪个输入框都一样。
- 逐个替换后焦点不跳走（轨迹无 `focus canvas`）；`Esc` 仍关闭搜索条。

## 6. 机器门

| 项 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --workspace --all-targets` | 与 `scripts/clippy-baseline.txt` 比对，新增 0（`app_methods_2.rs` / `panels/search.rs` 无告警） |
| 全量串行测试 | **1065 passed / 0 failed / 1 ignored**；`mw-shell` 603、`mw-core` 单测 278（含 search 29） |
| `stage-cm` | **58 场 / 116 次比较**，两遍 `--tol 0` 全过（`08:55:46–08:58:52`，最终代码复跑） |
| `stage-b` | **126 场 / 252 次比较**，两遍 `--tol 0` 全过（08:48:29–08:55:25，新增两场 + `card-autosave` 定钟） |

## 7. 未做 / 风险

- 空替换串 = 删除命中段，与「全部替换」一致；如果用户只想清空替换框，别按 `Ctrl+Enter`。
- 逐个替换只覆盖**字面**标题命中；拼音命中仍可跳转但不能替换（与 `replace_all` 同口径）。
- 若替换后的文本仍包含查询词（如 `aa → a`），该行会继续留在结果里；当前行按「下一行」
  前进，连续按会绕圈，这是与「全部替换」不同的语义，属预期。
- 替换后相机居中会改变取景，`Ctrl+Z` 只回标题不回相机（与既有节点跳转一致）。
- 实机 IME / 键盘手感未核（与 M4 同一缺口）；`Ctrl+Shift+Enter` 在中文输入法下的归属未实机验证。
- `MW_CARD_NOW_MS` 只服务金样，不写进产品文档；正常启动不读它。
