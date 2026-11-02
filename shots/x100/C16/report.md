# C16 · 命题短语的空白提交口径（打磨轮 40）

- 日期：2026-10-31
- 依据：`docs/30` §5「禁止无链接短语的线团」、§5.4「链接短语」、§5.6「大纲改结构」；
  `docs/30` §5.9「未填不给连线落盘」；模型层 `mw-core` 的 `SetRelationshipLabel` 守卫
  （`crates/mw-core/src/document.rs:1413`「概念图的连接短语必填」）。
- 状态：**已实现**（机器门绿；实机鼠标/键盘手感未核，人工门未开，不记 `已验证`）

## 1. 现状（两处已复现的静默失败）

概念图里「没有短语的边」不构成命题（`docs/30` §5）。模型层已经堵住了「把已有短语清空」：

```text
document.rs:1413  if self.is_concept_map() && label ... empty { Err(Invalid("概念图的连接短语必填")) }
```

但壳层没接住这条拒绝，两个入口都会静默失败：

### 1.1 既有命题被清空：编辑器关掉、短语还在、什么也不说

`Canvas::rel_editor_ui`（`canvas_methods_3.rs:728`）把空串折成 `label: None` 直接发命令；
模型拒绝 → `apply_command` 返回 `false` → 编辑器已 `take()` 走、没有恢复，画面停在「短语没变」。
没有 toast、没有轨迹，用户只会觉得「回车没反应」。

复现（`shots/x100/C16/before-clear.png`，临时目录 `target/x100/c16-pre/` 拍的）：

```text
MW_CONCEPT_DEMO=1 MW_CONCEPT_REL_EDIT=带来
脚本：wait 8 · Ctrl+A · Delete · Enter · wait 6
轨迹：event rel-label <from> <id>        ← 命令发了
      （没有 toast、没有 reject；截图里「带来」原样还在、线仍是选中态）
```

### 1.2 新建连线按 Enter 留空：线团真的落进文档

同一段代码里，`blank_new` 只在「失焦且为空」时撤线（`canvas_methods_3.rs:717`）；
**按 Enter 留空**走的是提交分支 → `label: None` → 模型拒绝 → 返回 `false`，
但编辑器关掉了、线没有删。结果是一条 `w=0` 的无短语命题：

```text
MW_CONCEPT_DEMO=1 MW_CONCEPT_LINK=用户痛点|风险与依赖
脚本：wait 8 · Enter · wait 8
轨迹：event rel-edit-open ...                 ← 新线立起短语框
      event rel-label <from> <new-id>         ← 回车提交空串（被模型拒绝）
      event scene_rel idx=4 ... w=0           ← 线还在，短语宽度 0 = 线团
```

`shots/x100/C16/before-blank.png` 里能直接看到带「＋ 链接词」占位、没有短语的新线。

### 1.3 大纲侧同一口径的静默失败

`panels/outline.rs:3329` 对「既有短语被清空」仍然发 `Action::SetVia { label: None }`，
`Canvas::set_concept_phrase`（`canvas_methods_1.rs:1212`）照旧折成 `None` 发命令，
同样被模型拒绝、同样没有反馈。`docs/30` §5.6 第 388/398 行却还写着「空串 = 清掉短语，边保留」——
文档与模型相反，需要一起改。

## 2. 范围

1. **画布短语框**（`rel_editor_ui`）：把「提交判定」抽成纯函数 `phrase_commit`，四种结果：
   - `Unchanged`：一字未动，不发命令（原文原样，不压撤销栈）；
   - `Set(Some)`：正常提交；
   - `Set(None)`：**只在思维导图**里表示清空短语（模型不禁止）；
   - `DropEdge`：概念图 + 新建线 + 留空 → 删掉这条线、概念留下，提示「没填链接短语，这条命题已撤掉」；
   - `RejectEmpty`：概念图 + 既有命题 + 留空 → **不提交、保留原短语**，提示「概念图的连接短语不能为空」。
2. **提示通道**：`Canvas` 新增一次性 `notice`（与 `card_open` 同一种「画布置、壳层取」的字段），
   壳层在画布 `show()` 之后取走并走现有 toast（金样里不按墙钟过期，能进基线）。
3. **大纲短语框**（`OA::SetVia`）：概念图里 `label: None` 不再发命令，
   保留原短语 + 同一条提示，轨迹写 `outline_concept_via_reject`。
4. **文档对齐**：`docs/30` §5.4 / §5.6 两处「空串清掉短语」改成与模型一致的口径；
   账本 C5 行注明「空串清零」只对思维导图成立，新增 C16 行。

## 3. 非目标

- **不改模型**：`SetRelationshipLabel` 的守卫原样保留（它是防止线团的最后一道门）。
- 不改 `blank_new` 的 Esc 语义（仍然直接撤线，不提示——用户知道自己按了 Esc）。
- 不改 IME 组合期 Enter 归输入法的判定。
- 不改思维导图里「空串清掉短语」的行为。
- 不新增数据结构/字段/依赖；不动 `.zmind`。
- 不处理「撤销把空短语的线恢复回来」这种模型外的边角（Esc 撤线本来就有同样性质）。

## 4. 验收条款

### 4.1 纯函数判定（单测）

`phrase_commit(concept_map, text, opening, blank_new)`：

| 概念图 | 新建线 | 草稿（去空白） | 原短语 | 期望 |
| --- | --- | --- | --- | --- |
| 是 | 否 | 与原短语相同 | 带来 | `Unchanged` |
| 是 | 否 | 与原短语相同（两侧多空格） | 带来 | `Unchanged` |
| 是 | 否 | 空 | 带来 | `RejectEmpty` |
| 是 | 否 | 空格串 | 带来 | `RejectEmpty` |
| 是 | 是 | 空 | — | `DropEdge` |
| 是 | 是 | 空格串 | — | `DropEdge` |
| 是 | 是 | 新词 | — | `Set(Some("新词"))` |
| 是 | 否 | 新词（两侧带空格） | 带来 | `Set(Some("新词"))`（提交前去空白） |
| 否 | 否 | 空 | 带来 | `Set(None)`（思维导图仍可清空） |

### 4.2 端到端（轨迹 + 金样）

- `cm-phrase-clear`：打开「带来」短语框 → 全选删除 → Enter。
  轨迹必须是 `rel-label-reject reason=empty`，**没有** `rel-label`；截图里短语还在，底部有提示条。
  与 `cm-link-create` 的差别只在提示条与被选中态。
- `cm-phrase-blank`：新建「用户痛点 → 风险与依赖」连线 → Enter 留空。
  轨迹必须是 `rel-label-reject reason=blank_new`，随后的 `scene_rel` 不再出现 `w=0` 的那条；
  截图回到 5 条命题 + 提示条。
- 两张 `--tol 0` 双跑；stage-cm 门加两场。

### 4.3 既有基线零漂移

改动只落在「空串提交」分支：既有脚本要么输入非空短语（`cm-rel-curve` 输入「引起」），
要么停在编辑态不提交（`cm-link-create` / `cm-link-drag-pointer` / `cm-label-ime`）。
51 张既有概念图基线应零重拍。

## 5. 预计证据

| 类型 | 内容 |
| --- | --- |
| 单测 | `phrase_commit` 七行判定表（含空格串） |
| 轨迹 | `rel-label-reject reason=empty` / `reason=blank_new`、`outline_concept_via_reject`、`toast bad=0 text=…` |
| 金样 | `cm-phrase-clear` / `cm-phrase-blank`（两张 `--tol 0` 双跑） |
| 像素 | `cm-phrase-blank` 与 `cm-default` 的差只在提示条（线团已消失）；`cm-phrase-clear` 与 `cm-label-ime` 的差只在提示条与被选中态 |
| 复现 | 改前两张截图与轨迹，放在本目录 |

## 6. 实现

1. **画布短语框**（`canvas/canvas_methods_3.rs`）
   - 新增 `enum PhraseCommit { Unchanged, DropEdge, RejectEmpty, Set(Option<String>) }`
     与纯函数 `phrase_commit(concept_map, text, opening, blank_new)`。
   - `rel_editor_ui` 的提交路径改成一次 `match`：
     - `Unchanged` → 只关框，不发命令（不压撤销栈、不标脏）；
     - `RejectEmpty` → 框关掉、短语原样保留，`notice = Some(("概念图的连接短语不能为空", false))`，
       轨迹 `rel-label-reject reason=empty`；
     - `DropEdge` → 走 `finish_new_link(remove_edge=true)` 撤掉刚建的线，概念留下，
       提示「没填链接短语，这条命题已撤掉」，轨迹 `rel-label-reject reason=blank_new`；
     - `Set` → 原 `apply_command(SetRelationshipLabel)` 路径。
   - 思维导图（`concept_map == false`）的 `Set(None)` 保持原行为：空串 = 清掉短语。
2. **提示通道**（`canvas.rs` / `canvas_methods_1.rs` / `main.rs`）
   - `Canvas` 新增一次性 `notice: Option<(String, bool)>`，与 `card_open` 同为「画布置、壳层取」；
   - 壳层在 `canvas.show()` 之后 `take_notice()`，再 `set_toast(text, bad)`。
     画布因此不需要知道提示条长什么样。
3. **大纲同口径**（`app_methods_2.rs` 的 `OA::SetVia`）
   - 概念图 + 文本去空白为空 → 不发命令，`set_toast("概念图的连接短语不能为空", false)`，
     轨迹 `outline_concept_via_reject from=… rel=… reason=empty`；
   - 思维导图仍发给 `set_concept_phrase`（空串清空短语）。
4. **顺带修掉的键盘归属**（`panels/outline.rs`，金样实拍发现）
   - 短语编辑框打开时 `editing_active` 只认行文本编辑，`Ctrl+A`/`Delete` 会同时漏进大纲列表：
     改短语的同时把 6 行全选中（旧基线 `cm-outline-via-clear` 129480 px 差异的根因）。
   - 改成 `self.editing.is_some() || self.via_edit.is_some()`：短语框开着时大纲不抢键。
5. **文档**：`docs/30` §5.4 / §5.6 / §5.6.4 / §5.10 与两场金样表改成「既有命题不可清空，
   空提交 = 拒绝 + 提示；空短语只允许作为新建边的瞬态」。

## 7. 证据

### 7.1 单测

- `crates/mw-shell/src/canvas/tests.rs::blank_phrase_commit_follows_the_concept_map_contract`
  九条断言覆盖 §4.1 判定表（含空格串、`Set(None)` 只对思维导图）。
- `cargo test -p mw-shell --bins`：593 通过 / 0 失败 / 1 忽略（含本包新测）。

### 7.2 轨迹（`shots/x100/C16/`）

| 文件 | 关键行 |
| --- | --- |
| `trace-phrase-clear.txt` | `rel-label-reject from=… reason=empty` → `toast bad=false text=概念图的连接短语不能为空` |
| `trace-phrase-blank.txt` | `rel-edit-open … blank_new=true` → `rel-label-reject … reason=blank_new` → toast；后续 `scene_rel` 无 `w=0` |
| `trace-outline-via-clear.txt` | `outline_via_close … blank=true … fresh=false` → `outline_concept_via_reject … reason=empty` → toast |

改前对照（`before-*-trace.txt`）：只有 `rel-label`，没有 reject、没有 toast；blank 场景的 `scene_rel` 带 `w=0`。

### 7.3 金样（`--tol 0` 双跑）

| 基线 | 输入 | warmup | 钉住 |
| --- | --- | --- | --- |
| `cm-phrase-clear` | `cm-phrase-clear.txt`（24 帧） | 26 | 既有短语清空被拒：短语还在、只多提示条 |
| `cm-phrase-blank` | `cm-phrase-blank.txt`（18 帧） | 20 | 新建边空提交撤线：回到 5 条命题 + 提示条 |
| `cm-outline-via-clear` | `cm-outline-via-clear.txt`（24 帧） | 26 | 大纲同口径：选中 1 行、短语还在、提示条 |

### 7.4 像素

| 对比 | 差异 | 位置 |
| --- | --- | --- |
| `before-clear.png` → `cm-phrase-clear` | 10760 px | 仅提示条 `x16..375 y1148..1177` |
| Esc 对照（临时 `target/x100/C16/esc`）→ `cm-phrase-clear` | 10760 px | 同上：只差提示条 |
| `cm-default` → `cm-phrase-blank` | 12695 px（画布 0 / 铬件 12695） | 画布逐像素相同 = 线团没了 |
| `before-blank.png` → `cm-phrase-blank` | 14937 px | `x16..1355 y408..1177`（线 + 提示条） |
| `cm-outline` → `cm-outline-via-clear` | 34416 px | = 23768（键盘焦点行底色/拖手）+ 10648（提示条，同 Esc 对照） |

### 7.5 机器门

- `scripts/x100-stage-cm-gate.ps1`：**PASS**（2026-10-31 `02:44:54–02:48:15 +08:00`），
  fmt / clippy 子集 / 全量测试 **1030 passed / 0 failed** / 两遍矩阵 **56 场 / 112 次比较**全过，
  既有 53 场零漂移。日志 `target/x100/stage-cm-gate.log`。
- `scripts/x100-stage-b-gate.ps1`：**PASS**（2026-10-31 `02:49:47–02:56:11 +08:00`），
  两遍各 121 比较全过、1030 测试、fmt / clippy 子集绿；大纲键盘归属改动未让任何 `outline-*` /
  `mm-*` / `card-*` 基线漂移。日志 `target/x100/stage-b-gate.log`。

## 8. 风险与未做

- 实机微软拼音下「全选删除 + 回车」的按键序列未核（M4 同缺口）。
- 撤销可以把刚撤掉的空短语线恢复回来——模型层没有「概念边必须有短语」的全局不变量，
  这个问题在 Esc 撤线路径上早已存在，本轮不扩大处理。
- 提示条文案只做了一条；多语言没有体系，沿用现状中文。
