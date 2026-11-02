# CER-17 · 块级输入与 Markdown 粘贴

> 设计合同：`docs/28` §6.3（块级标记揭示、即时转换、粘贴规则）；
> 执行合同：`docs/29` §7 CER-17（`段落 Enter、标题 Enter 转正文、空列表退出、列表续项、任务切换、
> 粘贴 Markdown 保留结构、粘贴纯文本按当前块处理；一次撤销`）。
> 状态：**`已实现`**（不记 `已验证`——G-B2-2 人工门未开；实机微软拼音仍未核对，与 CER-16 共用 IME 门）。

## 1. 范围

| # | 合同条目 | 落地 | 证据 |
| --- | --- | --- | --- |
| ① | 段落 Enter | `TextEdit` 原生换行 | `card-default` 等既有基线 |
| ② | 标题行 Enter 转正文 | `enter_edit` 对标题行返回 `None`（新行自然不带 `#`） | `enter_on_a_heading_or_paragraph_is_not_taken_over` |
| ③ | 空项退出列表 | `enter_edit`：整行只剩前缀 → 前缀退掉、行变空段落 | `enter_on_empty_bullet_exits_the_list`、`card-enter-exit` |
| ④ | 列表续项 | `list_prefix` + `enter_edit`：无序归一成 `- `、有序 +1、任务项续成未勾选 | `card-enter-continue`、`task_item_continues_as_an_unchecked_task` |
| ⑤ | **任务框点击切换** | `task_flip` + `paint_block_decorations` 命中矩形 | `card-task-click`（本轮新增）、§4 |
| ⑥ | **粘贴 Markdown 保留结构** | `clip_has_markdown_structure` + `splice_structured` | `card-paste-md`（本轮新增）、§5 |
| ⑦ | 粘贴纯文本按当前块处理 | `paste_list` 补同型前缀 | `card-paste-list`（CER-16 基线）、`card-paste-plain-task`（本轮对照，不进基线） |
| ⑧ | 一次撤销 | `store_after_edit` 写 `auto_undo` 槽 | `card-paste-undo` 与 `card-paste-none` 逐字节相同 |

本轮做的是 ⑤ ⑥ 两格，以及给整个包补上独立证据目录（此前证据散在 golden 与逐轮日志里）。

## 2. 非目标

- 不改 `NoteCard` 字段、`.zmind`、正文存储路径。
- 不新增「粘贴为纯文本 / 粘贴为 Markdown」的菜单项或快捷键——判断按内容自动做。
- 不做表格、公式、Mermaid 的结构化粘贴（`docs/28` §6.2 无这些块）。
- 不把结构化粘贴改造成「导入文档」：段落里粘贴仍交给 `TextEdit` 原生插入。

## 3. 实现

### 3.1 任务框点击切换（⑤）

原状：`paint_block_decorations` 早就返回了命中 `(字符下标, 是否已勾选)`，调用点也写了「翻一格、把光标挪到行尾」，
**但没有实拍、没有单测、没有 trace**，所以 `docs/STATUS` 记的是「未做」——这一轮把它坐实并收进证据。

1. 抽出纯函数 `task_flip(text, at) -> Option<(新正文, 行尾下标)>`（`note_view.rs`）：
   - 自己重新定位行首 / 行尾，再核一遍「整行确实是任务项」且 `at` 正好落在方框那一格；
   - 坐标算错、行被改过、点在别的字符上一律返回 `None`，**绝不**误改正文；
   - `' ' ⇄ 'x'`，大写 `X` 也算勾上（写回统一成小写 `x`，与 `task_state` 的读法一致）。
2. 调用点改成先 `task_flip` 再落盘，并在 `MW_TRACE` 下记两行：
   - `task_box_click line=2 checked=false at=8 y=272.1`（命中的是哪一行、原状态、字符下标）；
   - `card_task_toggle at=8 checked=false line_end=14`（真的翻了、光标落在行尾）。
3. 光标挪到行尾是刻意的：接着打字就是继续这一条，不会挤进 `[ ]` 里。

### 3.2 结构化粘贴（⑥）

原状：`paste_list` 对**所有**多行粘贴一视同仁地补前缀。这在纯文本下是对的（合同⑦），
在成稿 Markdown 下会把结构弄坏：`- 甲` 变 `- - 甲`，`## 乙` 变 `- ## 乙`。

1. `clip_has_markdown_structure(clip)`：任意一非空行行首带块标记就算成稿。
   `starts_with_block_marker` 认：`# `…`###### `（`#` 后必须跟空格，`#######` 七级不算）、
   `> `、`- `、`* `、`+ `、`1. `…`999. `（点后必须跟空格）、围栏 ` ``` ` / `~~~`、表格 `|`、分隔线 `---`。
   行首空白先 `trim_start`，所以缩进列表也算。
2. 命中成稿就换成 `splice_structured`：**当前行原地不动**，粘进来的 Markdown 自立成块插在光标处——
   光标前有字补一个换行、光标后还有字再补一个换行（行尾只剩 `\r` 时省掉后一个，CRLF 不重复换行）。
3. 纯文本路径一行没动：段落里仍返回 `None`（交给 `TextEdit`），列表里仍补同型前缀。
4. `MW_TRACE` 下把这次粘贴记一行：
   `card_paste at=14 structured=true clip="…" body="…"`（`clip` 截 48 字符、`body` 截 96 字符）。

## 4. 证据 · 任务框点击

### 4.1 单测

| 测试 | 钉住什么 |
| --- | --- |
| `clicking_a_task_box_flips_only_the_box` | `- [ ] 待办一` 翻成正体小写 `[x]`；其余行一字不动；光标落在行尾 |
| `task_flip_rejects_anything_that_is_not_the_box` | 点列表符号 / 正文 / 非任务行、缩进行下标错一格 → 全部 `None`（不改正文） |

`note_view::tests` 58 → **60** 条全绿（本轮再加 2 条粘贴测试，见 §5.1）。

### 4.2 基线 `card-task-click`（新，已登记进 stage-b 门）

夹具 `MW_CARD_TASK=1`，正文 `第一段\n\n- [ ] 待办一\n- [x] 待办二\n\n尾段\n`；
脚本 `shots/golden/inputs/card-task-click.txt` 在第三行方框中心 `(547, 272)` 点一下（ppp 1.0）。

trace（`target/x100/cer17/trace-click-run1.txt`）：

```text
event task_box_click line=2 checked=false at=8 y=272.1
event card_task_toggle at=8 checked=false line_end=14
```

像素（1800×1200，`--tol 0` 连跑两次 0 差异，sha256_12 = `5C2DC8BBEE76`）：

| 对照 | 差异 | 包围盒 |
| --- | --- | --- |
| 不输入 → 点方框 | 1788 px（0.083%） | x[539..1725] y[26..289] |
| 不输入 → 点正文（同帧数对照） | 528 px（0.024%） | x[539..619] y[219..289] |
| 点正文 → 点方框 | 1747 px（0.081%） | x[540..1725] y[26..289] |

方框 16×16（x540..555）内的颜色构成，把「翻过去了」和「只挪了光标」分开：

| 帧 | 第一条方框（y264..280） | 第二条方框（y294..309，原 `[x]`） |
| --- | --- | --- |
| 不输入 | 强调色 0 px / 描边 44 px | 强调色 186 px / 描边 0 px |
| 点正文（对照） | 强调色 0 px / 描边 44 px（**没动**） | 强调色 186 px / 描边 0 px |
| 点方框 | **强调色 186 px / 描边 0 px** | 强调色 186 px / 描边 0 px（没动） |

`186 px = 16×16 − 对勾笔画`，与「已勾选实心框」的形状一一对应；第二条框的 186 在三种帧里恒定，
说明翻的是被点中的那一条，不是整行整段一起动。

`y[26..38]`、`x[1571..1725]` 那一小块差异只在**正文被标脏**时出现（点方框标脏、点正文不标脏；
两张粘贴对照组同样出现），对应顶栏右侧的保存状态区域。

## 5. 证据 · 结构化粘贴

### 5.1 单测

| 测试 | 钉住什么 |
| --- | --- |
| `pasting_markdown_keeps_its_own_structure` | 行尾粘 → 原行不动、整段自成块；行首粘 → 原行让到下一行；行中粘 → 前后各断开一次 |
| `structured_clip_is_recognized_by_its_block_markers` | 8 种成稿判 true；`#标题没有空格`、`####### 七级`、`1.甲`、`-甲`、纯段落判 false |

### 5.2 基线 `card-paste-md`（新，已登记进 stage-b 门）

脚本 `shots/golden/inputs/card-paste-md.txt`：点进待办行 → `End` → 粘 `## 小节\n- 乙\n- 丙`。

trace（`target/x100/cer17/trace-md-run1.txt`）：

```text
event card_paste at=14 structured=true clip="## 小节\n- 乙\n- 丙" body="第一段\n\n- [ ] 待办一\n## 小节\n- 乙\n- 丙\n- [x] 待办二\n\n尾段\n"
```

原行 `- [ ] 待办一` 原样保留，粘进来的三行一个前缀都没加、也没并进原行。

对照 `card-paste-plain-task`（同位置、同帧数，粘 `乙\n丙`；注意 CER-16 那张同名的
`card-paste-plain.txt` 是「段落夹具」的对照，本轮没有覆盖它）：

```text
event card_paste at=14 structured=false clip="乙\n丙" body="第一段\n\n- [ ] 待办一乙\n- [ ] 丙\n- [x] 待办二\n\n尾段\n"
```

续行补成 `- [ ] 丙`——纯文本按当前块处理（含任务项续写不带勾选的约定），与合同⑦一致。

像素（shoot 脚本实测，1800×1200）：

| 对照 | 差异 | 包围盒 |
| --- | --- | --- |
| 不输入 → 粘纯文本 | 5506 px（0.255%） | x[539..1725] y[26..497] |
| 不输入 → 粘 Markdown | 6411 px（0.297%） | x[539..1725] y[26..557] |
| 粘纯文本 → 粘 Markdown | 6067 px（0.281%） | x[540..1583] y[28..557] |

两版的包围盒高度不同（497 / 557）：纯文本只多两行、成稿多出一个二级标题 + 两条列表，
行高不同——与 trace 里的正文结构一致。

## 6. 机器门与本地门槛

- stage-b 门：**71 场 / 142 次比较**（本轮 +2 场：`card-task-click`、`card-paste-md`），
  两张新基线各 `--update` 后连跑两次 `--tol 0` 均 0 差异，其余场景零漂移（见 `docs/STATUS.md` §1 日志）。
- `cargo fmt --all -- --check` 干净；`cargo clippy --workspace --all-targets` 指纹 **37 / 新增 0**。
- 工作区测试见 `docs/STATUS.md` §1（本轮 +4 条）。

## 7. 未做 / 风险

1. **实机微软拼音**：本轮全部是构造事件（`Event::Paste` / `Paste` + `Ime`），真机输入法组合态仍未核对，
   与 CER-16 共用一个 IME 门；在这条闭合前 CER-16 / CER-17 都不许记 `已验证`。
2. **段落里粘成稿不成块**：段落行上粘 Markdown 仍由 `TextEdit` 原生插入（光标所在行的前后文会粘连），
   只有列表行接管。若要「随处粘都成块」，得把接管范围扩到段落——那是 CER-20 的编辑管线，不在本轮。
3. **成稿判定是启发式的**：只要有一行像块标记就整段按成稿处理。粘 `价格 - 说明` 这类行首恰好是 `- `
   的表格化纯文本时也会被判成列表——宁可不动结构，也不猜用户意图。
4. **写回统一小写**：点已勾选的 `- [X] 大写` 会变成 `- [ ]`（读的时候两种都认，写回只写 `x`）。
5. **命中矩形外扩 4px**：方框本身 16×16，可点区域 24×24，与 `docs/28` §5.2 的 14×14 框规格略有出入
   （现实现 16×16，B2 期间未调整，记在 CER-15 的已知偏差里）。
6. **点击也标脏**：翻勾选会写 `dirty_since`，顶栏出现保存状态变化；这是有意的（正文确实变了），
   但会让「点一下就出」的截图与不输入的参照帧在顶栏右侧差一小块。

## 8. 变更记录

- 2026-10-28：CER-17 收口。新增 `task_flip` / `clip_has_markdown_structure` / `splice_structured` /
  `starts_with_block_marker` / `ellipsis`；两处 trace（`task_box_click`、`card_task_toggle`、`card_paste`）；
  单测 +4（`note_view` 58 → 60）；两张新基线进门（71 场 / 142 次比较）；
  新建本目录与 `manifest.json`。状态由 `进行中` → `已实现`（实机 IME 未核对）。
