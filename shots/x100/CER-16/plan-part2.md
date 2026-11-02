# CER-16 后半 · 即时转换与块级键盘行为（待做）

> 本文件写于 2026-10-28 06:15。§3 纯函数、§4 两张实拍、以及「一次 `Ctrl+Z` 撤回转换」
> 均已落地（见 `report.md` §6 与 §10）；**仍缺实机 IME 门**（与 CER-17 合判）。
> 前半（片段级标记揭示）已 `已实现`，见同目录 `report.md`。

## 1. 先认清一件事：缓冲区就是 Markdown

卡片正文的 buffer 存的是 **Markdown 源文**（`note_view.rs` 的 `draft_body`），不是富文本。
因此「输入 `#` + 空格 → 变成标题」这件事**几乎是免费的**：用户敲出来的 `# ` 本身就是标题的源文，
渲染层当场就按标题排版了。所以 `docs/28` §6.3 那条「即时转换」真正缺的不是「转换」，而是四件配套：

1. **一次 `Ctrl+Z` 撤回转换**——现在没有「转换」这个动作，自然也没有要撤回的东西；
   若将来做「`* ` 归一成 `- `」这类规范化，就必须让它成为一条可撤销的编辑。
2. **空列表项 `Enter` 退出列表**（§6.8）：在 `- ` 后面直接 `Enter`，应当把这一行清成空段落，
   而不是留下一个空列表项、更不是留下 `- `。
3. **列表项 `Enter` 续写**：在有内容的列表项末尾 `Enter`，新行应自动带 `- `（有序列表带下一个序号）。
4. **标题行 `Enter` 转正文**：在 `## 标题` 末尾 `Enter`，新行不应继承 `##`
   （egui 只继承前导空白，所以现在**已经是对的**，但要写测试钉住）。

## 2. 落点（都在 `crates/mw-shell/src/note_view.rs`）

| 要改的地方 | 现在长什么样 | 备注 |
| --- | --- | --- |
| body `TextEdit` 之前 | 只算 `active = if self.body_focused { self.active } else { None }` | 在这里拦 `Enter`：`ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))`；**先判断 IME**（这一帧有 `Ime::Commit` 就不拦） |
| 正文变化钩子 | 两处 `self.draft_body != self.saved_body` | 转换后的文本走同一条「脏」判定，不要新增第二条 |
| 光标写入 | `self.active = Some((line, col_in_line(&self.draft_body, idx)))` | 转换后要显式写回新光标（字符索引），否则下一帧 `active` 停在旧列 |

## 3. 纯函数先行（无窗口可测）

```rust
/// `Enter` 在当前行尾按下时的编辑结果。返回 None 表示「不接管，交给 TextEdit」。
fn enter_edit(text: &str, caret: usize) -> Option<(String, usize)>;
```

规则：

- 光标不在行尾 → `None`（行中回车就是普通换行）。
- 当前行是空列表项（`- ` / `- [ ] ` / `1. ` 之后无内容）→ 删除该行前缀，返回新文本与新光标。
- 当前行是列表项且有内容 → 插入 `\n` + 同型前缀（有序列表序号 +1）。
- 其它 → `None`。

单测清单：空无序项 / 空有序项 / 空任务项 / 有内容无序项 / 有内容有序项（`9.` → `10.`）/ 标题行 / 普通段落 / 光标在行中。

## 4. 验收（做完要补的）

- ✅ 两张 golden 场景已拍：`card-enter-continue` / `card-enter-exit`（对照 `card-enter-nox` 不进基线）；
  `MW_GOLDEN_INPUT` 用 `key Enter`，`--warmup` 要够到光标稳定。
- IME 门：**必须与 CER-17 一起判**（`docs/29` 第 242 行）。组合态下 `Enter` 属于输入法，不能被抢。
- 门/CI 数字按当时快照更新（当前：56 场 / 112 次比较、875 测试、clippy 37 / 新增 0）。
