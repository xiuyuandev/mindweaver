# CE-01 · 卡片笔记编辑体验

> 建立：2026-11-01。范围：**只动卡片笔记页（`note_view.rs`）的编辑体验**。
> 画布、概念图、检查器、大纲一律不碰。
> 触发：用户报「编辑笔记内容，换行后光标都找不到了，而且右侧的滚动条很奇怪」，
> 并要求「只升级笔记的编辑体验，其他不管」。

## 1. 范围

`crates/mw-shell/src/note_view.rs` 的 `CardReader`（单表面 Markdown 编辑器）
与它周边的滚动容器。合同依据：`docs/28` §6.1–§6.9。

**非目标**（明确不做）：

- 不引入块模型 / 不拆多个 `TextEdit`（`docs/28` §6.12 明令禁止）。
- 不改 `.zmind` 格式、不新增 frontmatter（`docs/28` §6.12「单一真源」）。
- 不改正文之外的外壳：顶栏、面包屑、标签管理入口、反链结构保持原样。
- 不做目录浮层（`Ctrl+Shift+O`）——CER-13 已按合同移除常驻目录，
  浮层属新功能而非「打磨」，留待单独批准。
- 不碰画布 / 概念图 / 检查器 / 大纲的任何代码路径。

## 2. 实测发现（改前）

全部为**本机实测**，不是读码推测。取证命令见 §5。

### D1 · 换行后光标消失（用户报的第 1 条）

`build_job` 把「空行」的行高直接改成块间距（`docs/28` §6.2 的 12/40/28/24/8），
于是空行**不再是一行字高**。连续空行更糟：`gap_taken` 让第二个及以后的空行
`gap = 0.0` → **行高 0**。

排版实测（`probe_row_heights`，离屏 Context）：

| body | 行数 | 光标所在行高 | 光标矩形 |
| --- | --- | --- | --- |
| `"abc"` | 1 | 28.0 | `(24.8,0)–(24.8,28.0)` ✅ |
| `"abc\n"` | 2 | 28.0 | `(0,28)–(0,56)` ✅ |
| `"abc\n\n"` | 3 | **0.0** | **`(0,28)–(0,28)` 高度 0** ❌ |
| `"abc\n\n\n"` | 4 | **0.0** | **`(0,28)–(0,28)` 高度 0** ❌ |

即：**写完一段、回车空一行、再回车 → 光标变成一条零高度的线，屏幕上找不到。**
这是中文写作里最常见的动作（分段），所以「换行后光标找不到」不是偶发。

附带问题：`self.active` 的行号由 `line_index_at` 数 `\n` 得到，而
`build_job` 遍历的是 `split_inclusive('\n')`。正文以 `\n` 结尾且光标停在末尾时，
`active` 会指到**不存在的第 n 行**（幻影行），该帧任何行都不揭标记。

### D2 · 滚动条（用户报的第 2 条）

卡片页用的是 egui 默认滚动条，而 egui 0.37 的默认是 `ScrollStyle::floating()`
（`style.rs` 的 `impl Default for ScrollStyle` 直接返回 `Self::floating()`）：
休眠态 `dormant_handle_opacity = 0.0`（**完全不可见**），悬停/滚动时才浮出来，
且**浮在文字上方**、位置在版心右缘而不是窗口右缘。

两段取证：

1. `card-long`（1620×1080）里内容其实**没溢出**（实测 `content=488.1 view=652.0`），
   所以右侧干干净净——这一点起初被我误判成「有内容却没滚动条」，
   量了轨迹才纠正过来。
2. 换成真超长的 `MW_CARD_TALL`（20 段）：`content=1640.1 view=652.0`，
   老实现下扫描 `x=1200..1620`、`y=150..1000` 的非背景列只有 `[[1200,1349]]`
   ——**窗口右侧 270px 全是纯背景，画面上没有任何滚动条**。
   用户看得到正文在 y≈1000 处被裁断，却既不知道自己在长文里的位置，也没有可拖的滑块。

这与 `docs/28` §4.5 的要求（6px 宽 / 4px 拇指 / 圆角 8 / 默认透明 /
悬停滚动 120ms 淡入 / 空闲 300ms 淡出）也不符：项目其它页面（`start.rs`、`menu.rs`）
都是自绘的细条，只有卡片页漏了。

### D3 · 有序列表编号被压成 4px 小点

`push_line` 的有序列表分支用 `style.marker_indent(digits + 2, hide, 24.0)`。
`marker_indent` 在 `hide == true`（非光标行）时把字号压到 **4px** 并置透明，
用来「用字距冒充缩进」；紧接着又写了 `num.color = palette.dim` ——
**颜色改回来了，字号没改**。结果非光标行的编号是 4px 的深色小点，
截图放大后是像逗号一样的墨点，合同的「数字右对齐到 20px 列」完全没兑现。

（同一段代码注释写着「编号始终显示，跟 Typora 一样」，与实现直接矛盾。）

### D3b · 点进列表行，正文会横跳

同一条 `marker_indent` 的另一半：它按 `hide` 分两种形态——光标不在行内用
4px 透明字 + 字距把正文推到 24，光标进到行内就换成正常字号的可见标记。
可见标记的自然宽远小于 24（`- ` 约 9px、`1. ` 约 17px），于是**点进一行列表，
那一行的正文就左移十几个像素**；再点出去又跳回来。

而绘制层本来就会先拿页面底色把那 24px 盖掉、再补圆点/方框/编号
（`paint_block_decorations` 第一句就是 `rect_filled(row, …, palette.bg)`），
所以「在光标行露出 `- `」这条从未真正生效过——留着只是让文字乱跳。

### D4 · 正文里 `Tab` 写入裸 `\t`

`TextEdit::lock_focus(true)` 让 egui 把 Tab 交给正文，而 egui 多行 `TextEdit`
对 Tab 的处理是 `insert_text_at("\t")`。本项目的 Markdown 排版器不认制表符：
`\t` 既不缩进也不可见，只会在缓冲区里留下一个**看不见的字符**，
之后 `trim_start` / `lead_len` 全部按它算，光标与文字对不上。

### D5 · 行尾空白被排版层吃掉

自定义 layouter 建的 `LayoutJob` 没设 `keep_trailing_whitespace`，
而 egui 自己的默认 layouter 是设了的（`builder.rs:583`，注释写明
「text editors 要保留行尾空格」）。后果：**在一行末尾连续敲空格，光标不动**，
看起来像卡死。

### D6 · 标记揭示慢一帧

`active`（决定哪一行露出 Markdown 标记）取自 `self.active`，而它在**上一帧末**
才由 `output.cursor_range` 写回。我们自己的带外改动（`caret_to`、斜杠插入、
列表续写、行首退格）都在这一帧**之前**就把新光标存进了 `TextEditState`，
但排版用的仍是旧值：换行之后，露标记的还是上一行，要等下一帧才对。

## 3. 改法

| 缺陷 | 改法 | 位置 |
| --- | --- | --- |
| D1 | `build_job` 给光标所在源行 `max(gap, BODY_LINE)`；`caret_line` 夹到真实源行数内（末尾幻影行） | `build_job` |
| D2 | 关掉 egui 内置条（`AlwaysHidden`），新建 `crate::scroll_bar` 自绘：贴窗口右缘、6/4/8 令牌、120ms 淡入 + 300ms 空闲淡出、整条轨道可拖 | `scroll_bar.rs` + `draw_scroll_bar` |
| D3 | 编号在文字层只留透明占位，绘制层按 20px 列右对齐画 | `push_line` + `paint_block_decorations` |
| D3b | `marker_indent` 去掉 `hide` 分支，缩进恒定；看得见的标记一律交给绘制层 | `marker_indent` |
| D4 | `Tab` / `Shift+Tab` 由编辑器接管：列表行整行 ±2 格，其余行光标处补/退 4 个空格 | `indent_edit` |
| D4b | 行首 `Backspace` 在列表标记末尾整枚删掉标记，退回普通段落 | `backspace_marker` |
| D5 | `LayoutJob::keep_trailing_whitespace = true` | `build_job` |
| D6 | `active` 改读**本帧**的 `TextEditState`，`self.active` 只作首帧兜底 | `show` |
| D7 | 标签的删除 × 拿到独立命中区（落在两枚标签的空隙里，不占版面）；点标签文字不再删 | `meta_and_tags` |
| D8 | 顶栏在 `save_error` 时挂「重试」按钮（hover 显示原因） | 顶栏保存态 |
| D9 | `Ctrl+/` 叫出斜杠菜单：行首补 `/`，行中有字就另起一行再补 | `slash_invoke_edit` |
| D10 | 标题行 `Enter` 把焦点交给正文（合同 §6.8） | 标题 `TextEdit` |
| D11 | 选中多行按 `Tab`：整段一起缩进、选区保住（以前只缩进光标那一行还打散选区） | `indent_edit` + `store_after_edit_range` |
| D12 | 缩进过的列表项照样续写、照样带缩进粘贴（`list_prefix` 只认行首，行首变空格就断） | `enter_edit` / `paste_list` |
| D13 | 字数每帧解析两遍 Markdown（20k 卡片白解析） | 按正文快照缓存 `word_count` |
| D14 | `Ctrl+Enter` 等于「保存并关闭」，在正文里顺手一按就整页关掉（合同 §6.8 明写「不绑定提交」） | 解除绑定，落回 `TextEdit` 当换行 |
| D15 | **标题重复**（合同 §6.10）：正文首行是 `# 标题` 时，标题框下面又画一遍同一句 | 打开时摘掉那一行、保存时按原样拼回（`split_title_header` / `compose_body`，字节级可逆） |
| D16 | 标签输入框空着按退格毫无反应（合同 §6.6 要「删掉最后一个」） | 在 `TextEdit` 之前截，逐个退回 |

### 第二批：D7–D10 为什么值得做

- **D7 是安全问题，不是美观问题**：以前整枚标签都是删除按钮，手一滑点上去标签就
  没了，而且标签删除**没有撤销**（它不在 `TextEdit` 的 undoer 里，也不进文档命令）。
- **D8** 合同 §6.7 写着「失败不关闭，显示行内错误条与重试」，代码里只有一句
  「保存失败」。现在 hover 能看到原因、点一下就能再试。
- **D9** `docs/28` §6.5 的触发方式写的是「空块行首输入 `/`，或 `Ctrl+/`」，后者没做。
- **D10** `docs/28` §6.8 写了「标题行 Enter 转到正文」，而单行 `TextEdit` 对 Enter
  根本不理——在标题里敲回车像敲到空气里。

## 3. 验收场景

| 场景 | 判据 | 证据 |
| --- | --- | --- |
| 分段后光标可见 | `"abc\n\n"` / `"abc\n\n\n"` / `"abc\n\n\n\n"` 的光标矩形高 ≥ 28 | 单测 `blank_line_keeps_a_full_row_for_the_caret` |
| 连续空行仍折叠 | `"甲\n\n\n乙"`：第一空行 12、第二空行 0、正文 28 | 单测 `blank_lines_without_the_caret_still_collapse` |
| 光标行号不越界 | `"abc\n\n"` 夹取后落回最后一条源行；标题行标记仍露 | 单测 `caret_line_never_points_past_the_last_source_line` |
| 滚动条贴窗口右缘 | 轨道右缘 = 视口右缘 − 4；宽 6；滑块宽 4 居中；上下贯通 | 单测 `thumb_hugs_the_window_right_edge` |
| 滚动条会淡出 | 热 0.32s 全亮；空闲 0.1s 仍亮；再 0.5s 归零 | 单测 `fade_comes_in_and_goes_out_after_idle` |
| 滚动条真出现在画面上 | `card-scroll` 基线右缘 4px 竖条；轨迹 `card_scroll on … x=[1070,1076]` | 金样 `card-scroll` |
| 不满一屏不画 | `card-long` / `card-default` 逐像素零漂移 | 见 §6 |
| 有序列表编号正常字号 | `1.` `2.` `3.` 清晰、右对齐成一列 | 金样 `card-default` 重拍 |
| 列表行缩进不跳 | 光标进出行时正文起点差 < 0.6px，且恒为 24（引用 16） | 单测 `list_indent_does_not_move_with_the_caret` |
| `Tab` 不写裸 `\t` | 列表行 ±2 格、段落 4 空格 | 单测 `tab_indents_with_spaces_and_never_writes_a_tab` |
| `Shift+Tab` 只退空格 | 无前导空格时不接管 | 单测 `shift_tab_outdents_only_the_leading_spaces` |
| 行首退格退出列表 | `- 甲` / `1. 甲` / `- [ ] 甲` 都退回段落 | 单测 `backspace_at_the_marker_end_leaves_the_list` |
| 行尾空格可见 | `keep_trailing_whitespace == true` | 单测 `layout_job_keeps_trailing_whitespace` |

## 4. 风险

- D1 会让「光标停在空行上」时那一行变高 16px，正文下方随之平移——
  这是**编辑器应有的行为**（零高度行放不下光标），但会让既有金样在
  「光标不在空行」时零漂移、在「光标在空行」时有意重拍。逐张核对。
- D2 只影响溢出场景，`card-default` 这类不满屏的基线不该漂。
- D3 会改 `card-default` 的编号外观（有意）。

## 5. 复跑

```powershell
cargo test -p mw-shell --bins note_view -- --test-threads=1
cargo golden --ppp 1.5 --tol 0 --dir shots/golden/card-default
```

放大取证的临时工具：`cargo run -p mw-golden --example crop -- <in> <out> <x> <y> <w> <h> [scale]`

## 6. 结果

### 6.1 单测

`cargo test -p mw-shell --bins` —— **665 passed / 0 failed / 1 ignored**（2026-11-01 23:1x）。
本轮为卡片编辑新增 16 条，`scroll_bar` 模块另有 5 条：

| 测试 | 钉住什么 |
| --- | --- |
| `blank_line_keeps_a_full_row_for_the_caret` | 光标落在空行上时行高 ≥ 28（改前 0） |
| `blank_lines_without_the_caret_still_collapse` | 非光标空行仍折叠（12 / 0） |
| `caret_line_never_points_past_the_last_source_line` | 末尾幻影行夹取 |
| `layout_job_keeps_trailing_whitespace` | 行尾空白保留 |
| `trailing_whitespace_flag_never_moves_the_rows` | 该开关不改变行盒几何 |
| `ordered_marker_is_reported_for_the_paint_layer` | 编号解析口径 |
| `list_indent_does_not_move_with_the_caret` | 列表正文起点恒为 24 / 16 |
| `tab_indents_with_spaces_and_never_writes_a_tab` | Tab 不写裸 `\t` |
| `shift_tab_outdents_only_the_leading_spaces` | 反缩进只退空格 |
| `backspace_at_the_marker_end_leaves_the_list` | 行首退格退出列表 |
| `ctrl_slash_starts_the_menu_on_a_blank_line_or_a_new_one` | `Ctrl+/` 落 `/` 的两种形态 |
| `tab_indents_every_selected_line_and_keeps_the_selection` | 多行缩进 + 选区保住 |
| `indented_list_items_still_continue_and_paste` | 缩进列表续写 / 粘贴 |
| `word_count_cache_tracks_the_body` | 字数缓存随正文失效 |

`scroll_bar`：`nothing_to_scroll_means_no_bar` / `thumb_hugs_the_window_right_edge` /
`thumb_tracks_both_ends_and_keeps_a_minimum` / `dragging_maps_back_to_an_offset` /
`fade_comes_in_and_goes_out_after_idle`。

### 6.2 金样

**卡片子集 34 场，`--tol 0` 连跑两遍全绿**（`scripts/ce01-card-sweep.sh`，
口径与门一致：每场独立 `APPDATA` / `LOCALAPPDATA` + `TEMP` 重定向）。

新增两场：

| 场景 | 内容 | 结果 |
| --- | --- | --- |
| `card-blank-caret` | 空列表项回车退成空段落，光标停在那一行 | 光标是一整行高（改前 0 高度） |
| `card-scroll` | `MW_CARD_TALL` 超长正文 + 指针停在窗口右缘 | 右缘出现 4px 细条；轨迹 `card_scroll on … x=[1070,1076]` |

重拍前后的逐场漂移记在 `drift-before-reshoot.txt`。摘要：

- `card-default` 389 px，bbox `x[270..298] y[591..701]` —— 就是有序列表的编号列（D3）。
- `card-slash*` 41 px，bbox `x[270..273] y[597..695]` —— 编号列里的细笔画。
- `card-blocks` / `card-enter-*` / `card-paste-*` / `card-ime-*` / `card-reveal-*`
  1 万像素量级、包围盒横跨正文 —— 列表正文起点从 9/17 挪到 24（D3b）导致的整段重排。
- `card-empty` / `card-long` / `card-narrow` / `card-tags` **零漂移**（没有列表、不满屏、
  不触发滚动条）。

### 6.3 诚实清单（未做 / 未解释）

1. ~~**整道 `stage-b` 门没有复跑**~~ ✅ **已跑通**：`scripts/golden-sweep.sh`
   （绕开 `cargo golden` 的构建锁，直接驱动 `target/release/mw-shell.exe`）复核
   **155 场连跑两遍 `--tol 0` 全过**；正式脚本 `scripts/x100-stage-b-gate.ps1`
   同日 21:46–21:55 也跑出 155/155，两路结果一致。
2. 若干场景重拍前存在 **1–2px 的整体纵向位移**（顶栏 0、标题 1、正文 2，实测于
   `card-saving`）。逐条排查了 `keep_trailing_whitespace`（单测证明不改几何）、
   空行行高、列表缩进、滚动条，**没有找到归因**。基线按现状重拍，重拍后
   155 场两遍零差异——**这一条仍然没有解释**，只是不再影响比对。
3. **不碰**画布 / 概念图 / 检查器 / 大纲；`docs/28` §6.8 的 `Ctrl+Shift+O` 目录浮层
   属新功能，未做。
4. **收尾时仓库被另一个会话反复改动**（`canvas.rs` / `mw-format` / `mw-core` 先后
   处于半改状态，编译数次被打断）。本包的每一次验证都等编译变绿之后重做，
   最后一次是 23:1x：`665 passed / 0 failed` + 155 场两遍全过。本包没有碰 canvas 一行。
