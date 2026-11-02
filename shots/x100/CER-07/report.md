# CER-07 · 大纲行内编辑与 IME

> 状态：已实现（§7 的行布局偏差已于 2026-10-25 修复，见 §10）· 开始 2026-10-25 · 批次 B2-A · 依赖：CER-06（已完成）
> 设计合同：[`docs/28`](../../../docs/28-内容体验重构规范.md) §5.4（编辑契约）、§4.6（IME）
> 执行合同：[`docs/29`](../../../docs/29-内容体验重构工作包.md) §6（X62 范围并入并加深）

## 1. 范围

对照 §5.4 编辑契约逐项审计 X20 既有实现，补齐缺口并把行为钉进单测。

## 2. §5.4 契约审计（2026-10-25，代码实测）

| 契约项 | 现状 | 依据 |
| --- | --- | --- |
| 单一活动编辑框 | ✅ | `self.editing: Option<Edit>`，同一时刻一个会话 |
| 编辑框与行同字体/字号/左边距 | ✅ | 编辑 overlay 复用 `row_font_id(density, depth, row)` 与同一 `text_x`（含 CER-05 的任务框位移） |
| 本地草稿 | ✅ | `Edit { draft, original }`，未变不写历史 |
| Enter 光标处拆分 / 行尾新建同级 | ✅ | caret 读 `TextEditState` 上帧范围，`sel_start != sel_end` 先删选区再拆 |
| 空行 Enter 退出并删空行 | ✅ | `empty` + Backspace/Delete → `leave_empty` |
| 行首 Backspace 并入上一同级 | ✅（超出契约） | `leave_merge` + Batch（改字+删除一条撤销） |
| Esc 还原原文 | ✅ | `esc` 分支走还原路径 |
| 失焦提交 | ✅ | 编辑会话失焦路径提交 |
| Ctrl+Enter 切换任务（不进编辑） | ✅ | `ctrl_enter` 在编辑态外处理；编辑态内 Ctrl+Enter 被吞（O08 占位） |
| IME 组合不拆行/不缩进/不提交 | ✅ | `ed.composing && !ime_commit` 期间吞掉 Enter/Tab/Esc/方向键/Backspace/Delete 全部结构键（含确认组合那一帧） |
| 一次提交一条撤销 | ✅ | 提交 = 单条 `SetText`；退格合并用 Batch |
| 编辑态 Ctrl+Z 只撤销文本 | ✅ | TextEdit 本地撤销；结构撤销在退出编辑后 |
| **Shift+Enter 行内换行** | ❌ | 现状：`outline_edit_soft_break_ignored`——被吃掉不提交（O08 占位）。§5.4 要求行内换行写入节点文本（节点文本支持多行）；实现需要编辑 overlay 支持多行显示与行高 +24/行（与 §5.2 换行规格联动） |
| **多行粘贴拆节点** | ❌ | 无 `Event::Paste` 拦截：多行文本进 singleline TextEdit 被展平。§5.4 要求按行拆成多个同级节点（首行进本行，余行成新同级），单行粘贴替换选中 |

结论：**契约 13 项中 11 项已落地且超出（退格合并 Batch）**，两项缺口都是
「编辑 overlay 的多行能力」：软换行需要多行显示，粘贴拆分需要行级命令序列。

## 3. 剩余实现计划

1. ~~**编辑 overlay 多行化**~~ ✅ **已实现（2026-10-25，见 §5）**。
2. ~~**多行粘贴拆节点**~~ ✅ **已实现（2026-10-25，见 §5 增量二）**。
3. ~~**测试**~~ ✅ `insert_at_chars` + `split_paste` 两组单测（10 断言）。
4. ~~**证据**~~ ✅ `outline-edit` / `outline-edit-ime` / `outline-edit-softbreak` /
   `outline-paste-lines` 四场景双跑 0 差异；微软拼音人工实测仍待用户配合时补记。
5. ring 场景四主题重拍：`wait 24` 已验证可消除帧竞态（三跑 0），待补拍。

## 4. 非目标

- 不做行内编辑之外的检查器/备注编辑路径。
- 不改 `Document`/`Command` 语义（拆分/合并/粘贴全部走既有命令）。
- 不做 CER-08 的结构键盘表（Tab/Shift+Tab 等已存在，本包只保证编辑态让位）。

## 5. 变更（2026-10-25 软换行 + 粘贴拆节点增量）

| 文件 | 变更 | 动外观 |
| --- | --- | --- |
| `crates/mw-shell/src/panels/outline.rs` | 新增纯函数 `insert_at_chars`（字符区间插入）与 `split_paste`（多行粘贴按行拆，行尾空白/末尾空行忽略）；`Shift+Enter` 在光标处插入 `\n`（本地草稿，不产生命令），光标经 `TextEditState::store` 推到新行行首；编辑行高按草稿行数 `32 + 24*(n-1)` 增长；编辑 overlay `singleline`→`multiline`；`Edit.paste_tail` 字段；**`paste_into_edit`**（窗口级拦截：多行首行按选区进草稿 + 余行入库、单行转 Text 事件还给 TextEdit、非编辑态原样还原）；`Action::PasteLines { id, text, siblings }`；提交分支把 paste_tail 检查提到 commit_add 之前（Enter 在行尾的粘贴不丢余行）；+2 单测 | **是（编辑态）** |
| `crates/mw-shell/src/app_methods_2.rs` | `window_clipboard` 顶部加大纲编辑粘贴钩子（在节点粘贴路径之前摘走 Paste）；`apply_outline_action` 新增 `PasteLines` 臂：`Command::Batch [SetText + N×AddNode]` 插到本行之后，一次撤销 | 否 |

## 6. 验证（软换行 + 粘贴拆节点）

- **行为**：workspace **783 tests / 0 failed**（`insert_at_chars_covers_caret_and_selection`
  + `split_paste_only_splits_multiline`：多行才拆/末尾空行忽略/CRLF/行尾空白）。
- **视觉/trace**：四个编辑场景人工过目 + 双跑 0 差异——
  - `outline-edit`：编辑态 3% 强调底 + 光标 + 文本长度下划线，无框；
  - `outline-edit-ime`：`ime preedit 拼音` 注入，组合下划线可见（结构键被吞——
    既有门的实拍；行布局一处待复核，见 §7）；
  - `outline-edit-softbreak`：End + Shift+Enter 后行高 32→56、光标落第二行
    行首（像素验证）、trace `outline_edit_soft_break caret=9`；
  - `outline-paste-lines`：三行粘贴 → trace 链 `outline_paste_window caret=9
    tail=2` → `outline_commit paste_tail=2` → `outline_paste_lines siblings=2`；
    终帧：本行变为「纯 Rust 原生第一行」，**第二行/第三行作为新同级插入**，
    计数 13→15，一条 Batch 一次撤销。
- **回归**：机器门 golden 矩阵双跑（`target/x100/cer-07-final-golden.log`）。
- **工程**：fmt 干净；clippy 触碰文件新增 0；release 通过。

## 7. 未做 / 已知偏差

- **ime 场景的行布局待复核**：组合态下编辑行显示为两行（首行空 + 下划线、
  文本在第二行）且确定性复现——组合门行为正确（结构键被吞、候选文本带组合
  下划线），但多行 TextEdit 的组合排布为何占两行需对照 select_all/caret 初始态
  查明；不影响组合不拆行契约，留 CER-07 收尾或 CER-08 一并核。
- **编辑态 ↑↓ 仍为"提交并走行"**（X20 语义）；docs/28 §5.5 要求编辑态移动
  光标——多行编辑落地后行为差异变大，列入 CER-08 结构键盘表时一并裁决。
- 微软拼音人工实测待用户配合时补记（离屏脚本用 `ime` 原语等效注入）。
- 多行文本的**非编辑态**显示仍是单行省略（`wrap.max_rows=1`）——完整换行
  显示与 CER-11 动态行高一起做；编辑中已按草稿行数完整显示。
- ring 场景四主题：`wait 24` 已验证消除帧竞态，待补拍。

## 8. 风险

- 多行 overlay 与 CER-11 虚拟化的行高计算耦合：先做，虚拟化时复用同一
  行高函数，避免两套度量。
- 粘贴拆分的撤销粒度：N 个节点是 N 条命令；`docs/28` §5.4 只对"提交"要求
  一条撤销，粘贴序列按现有 Batch 能力尽量并批。

## 9. 人工判词

非门包，不填。

## 10. 修复记录（2026-10-25 · 用户实拍反馈「输入时文字上方有一条下划线」）

**现象**：进入行内编辑后，2px 强调下划线出现在文字**上方**，文字整体比非编辑态
低一行；`outline-edit` 与 `outline-edit-ime` 两张基线都拍到了这个错误态（§7 记为
「ime 场景行布局待复核」的那条，实为同一根因）。

**根因**：CER-07 把编辑 overlay 从 `singleline` 换成 `multiline` 时没有约束行数。
`egui::TextEdit::multiline` 的 `desired_rows` 默认 **4**，配合 `.align(LEFT_CENTER)`
与 `frame(NONE)`：单行草稿的 widget 高度被抬到 4 行，内容再被垂直居中推到第 2 行；
而下划线按行几何画在 `text_rect.bottom() - 1`，于是线留在第 1 行底部、文字掉到下面。
副作用不止错位——编辑行被撑高约 66px，其后 13 行整体下移。

**修复**：`crates/mw-shell/src/panels/outline.rs:2293` 给编辑 overlay 加
`.desired_rows(1)`。行高仍由 `32 + 24*(草稿行数-1)` 决定（`outline.rs:2284` 起注释
记录约束），多行软换行不受影响；下划线几何与 `text_rect` 不变。

**证据**（`--ppp 1.5`，`--tol 0`，`MW_OUTLINE=1 MW_OUTLINE_EDIT='纯 Rust 原生'`）：

| 场景 | 修复前 | 修复后 | 判读 |
| --- | --- | --- | --- |
| `outline-edit` @1.5 | `7653fa4e…c6249fb` | `6226fff5…2cfc7e0d` | 下划线回到文字下方 3px，文字与非编辑态同位 |
| `outline-edit` @1.0 | `900932fd…896ae0cb` | `c29a5596…76da60f0` | 同上 |
| `outline-edit-ime` | `ac989196…978dfbb3` | `9a869ce5…73ba6eb` | 组合串不再占两行：单行「纯 Rust 原生拼音」+ 组合下划线 |
| `outline-edit-softbreak` | `eab02712…7b0f7e7` | `b5b13076…61007f24` | 两行行高保持 56，下划线仍在文字下方 |
| `outline-paste-lines` | `9504f14f…48788b` | `7d3a5936…3e7756` | 行为不变（13→15 主题、余行成同级），仅编辑行少掉的 66px 让后续行上移 |

- **漂移量**：`outline-edit` 2.165%（包围盒 x[263..1398] y[238..1032]）、
  `-ime` 5.251%、`-softbreak` 5.000%、`-paste-lines` 5.059%（y[530..604]）。
  包围盒覆盖整列正文，来源就是编辑行少掉的那 66px 造成的后续行整体上移，
  不是新增视觉元素。
- **稳定性**：五个场景（含 @1.0）连跑两次 `--tol 0` 全 0 差异；在门脚本的隔离
  `APPDATA/LOCALAPPDATA` 下复跑同样 0 差异。
- **回归（未重拍，实测 0 差异）**：`outline`、`outline-focus`
  （`MW_OUTLINE_FOCUS=1`）、`outline-compact`（`MW_OUTLINE_DENSITY=compact`）、
  `outline-narrow`（`MW_WINDOW_SIZE=800x600`，1200×900）、`outline-hover-select`
  （`MW_WINDOW_SIZE=1200x720` + 脚本，1800×1080）。改动只在编辑分支内，
  非编辑态行度量不变。
- **工程**：workspace **784 tests / 0 failed**；`cargo fmt --all -- --check` 干净；
  release 构建通过。clippy 的 `mw-shell` 告警为 `export_pdf.rs` 历史 3 条 +
  CER-09 在途拖拽代码 1 条（`panels/outline.rs:2903` collapsible `if`），
  本包未新增。

**§7 第 1 条作废**：多行 TextEdit 组合排布的疑问即本条根因，已闭环。
