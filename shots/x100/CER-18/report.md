# CER-18 · 斜杠菜单（`已实现`）

> 执行合同：`docs/29` §CER-18 一行；设计合同：`docs/28` §5（浮层几何与阴影）。
> 状态词只有 `未开始 / 进行中 / 已实现 / 已验证 / 阻塞 / 延后`。本包收尾时才能改成 `已实现`。

## 1. 范围

卡片页正文里行首打 `/` 弹出的插入菜单。合同判据（`docs/29` CER-18）：

| 项 | 合同 | 现状（开工前实测） |
| --- | --- | --- |
| 宽度 | 280px | **252px**（偏窄） |
| 行数 | 最多 8 行 | **不封顶**，10 条时高 312px |
| 行高 | 30 | 30 ✓ |
| 圆角 | 10 | 10 ✓ |
| 阴影 | `POPOVER` 令牌 | 手搓 `Shadow{offset:[0,8],blur:24}` |
| 条目 | 标题 1/2/3、无序/有序/任务、引用、代码、分隔线、图片 | 缺**有序列表**、缺**图片** |
| 过滤 | 按 label/别名/拼音过滤 | ✓（`slash_matches`） |
| 键盘 | ↑↓ / Enter / Esc | ✓ |
| IME | **不拦截** | ✗ 组合态按 Enter 会被菜单吃掉 |
| 光标 | 插入后落在该落的地方 | ✓（代码块落围栏中间） |

## 2. 非目标

- 不做子菜单、不做分组标题（合同没有）；
- 不做图标体系（条目仍用 `label + hint` 两段文本）；
- 不改 `Node` / `NoteCard` / `.zmind`；菜单只产生 Markdown 文本。

## 3. 验收场景（都进 stage-b 门）

| 场景 | 判据 |
| --- | --- |
| `card-slash` | 菜单 280 宽、8 行可见、第 9 条起可滚；阴影换 `POPOVER` 后整体漂移量记录 |
| `card-slash-filter` | 脚本输入 `/h2` 后只剩「标题 2」，且高亮行是它 |
| `card-slash-scroll` | 脚本按 8 次 `ArrowDown` 把高亮推到底，菜单滚动、高亮仍完整可见 |
| `card-slash-ime` | 组合态（preedit）下按 Enter **不**插入、菜单不关 |

## 4. 证据（收尾时回填）

- 几何断言：`SLASH_W` / `SLASH_ROW_H` / `SLASH_RADIUS` / `slash_visible_rows` 单测；
- 实拍：上面 4 张 + 与旧 `card-slash` 的逐像素漂移量；
- 门：stage-b 63 → 67 场（+4），126 → 134 次比较。

## 5. 未做 / 风险

（收尾时回填）

## 6. 实现与实测（2026-10-28 第 13 轮）

### 6.1 改了什么

| 项 | 改法 |
| --- | --- |
| 宽度 | 252 → **280**（`SLASH_W`） |
| 行高 | 30（`SLASH_ROW_H`），并**把 `item_spacing.y` 归零** |
| 行数 | `slash_visible_rows` 封顶 8，超出进 `ScrollArea` |
| 圆角 | 10（`SLASH_RADIUS`） |
| 阴影 | 改走 `ui_kit::menu_frame`，即 `design::elevation::POPOVER` 令牌 |
| 条目 | 8 → **10**：补「有序列表」（`1. `）与「图片」（`![图片]()`，光标落在 `![` 之后） |
| IME | 菜单键盘块加守卫：本帧有 `Event::Ime` 就不截 ↑↓/Enter/Tab |

### 6.2 量出来的真问题：行距 38 而不是 30

第一版改完量像素，发现相邻文字带间隔 **57 设备像素 = 38 CSS**，不是 30。
原因是 egui 默认 `item_spacing.y = 8` 叠在 30 的行高外面；于是「最多 8 行」的菜单实际只露出 6 行半，
第 7 行还被视口切掉字。把浮层内 `item_spacing.y` 归零后重拍：

| 量 | 期望 | 实测（ppp 1.5） |
| --- | --- | --- |
| 行高 | 30 CSS | 高亮带 45 设备像素 = **30.0 CSS** |
| 行宽 | 268 CSS（280 − 2×6 内边距） | 高亮带 402 设备像素 = **268 CSS** |
| 可见行数 | 8 | **8 条文字带**，节距 45 设备像素 = 30 CSS |
| 浮层高 | 252 CSS（8×30 + 12） | 填充段 ≈ 254 CSS（余量为 1px 边与 `POPOVER` 阴影溢出） |

### 6.3 漂移

`card-slash` 是有意重拍：**91208 px = 5.213%**，包围盒 `(260,513)-(722,1013)`，最大差 184。
旧基线是 252 宽 / 8 条 / 手搓阴影，新的是 280 宽 / 10 条 / `POPOVER`，菜单整体变宽变高，属于合同要求的变化。

| 场景 | 基线哈希 | 字节 |
| --- | --- | --- |
| `card-slash` | `45B5C6D5024D` | 121756 |
| `card-slash-filter` | `839B0D27D41F` | 125772 |
| `card-slash-scroll` | `CF09AA3CA59F` | 140491 |

### 6.4 单测（`note_view` 43 → 45）

- `slash_menu_geometry_matches_the_contract`：280 / 30 / 10 / 8 与 `slash_visible_rows` 的封顶行为；
- `slash_items_cover_the_contract_list`：10 条齐全、有序列表插入 `1. `、图片插入 `![图片]()` 且光标落在 `![图片` 之后；
- 顺带把原来按**下标**取项的测试改成按 label 查（下标会随条目增删漂，`todo` 从 4 变 5 就是这么漂的）。

### 6.5 踩坑：`cargo test` 不更新 `mw-shell.exe`

改完代码只跑了 `cargo test`，然后直接调 `./target/release/mw-shell.exe` 拍图，
结果新图与旧基线**逐像素相同（diff 0）**——因为跑的是**旧二进制**。
差点把「菜单没变」当成结论写进报告。规矩：手工拍图前先 `cargo build --release -p mw-shell`。

## 4. 证据（实测）

| 门 | 结果 |
| --- | --- |
| stage-b 门 | **PASS**，65 场 / 130 次比较（两遍），`finished=2026-10-28 13:35:07 +08:00` |
| `cargo fmt --all -- --check` | 干净 |
| `cargo test --workspace` | **893** 通过 / 0 失败 |

三张新场景两遍跑逐字节相同（见 §6.7 的表）。
## 7. 未做 / 风险

- **IME 不拦截只有代码守卫，没有真机证据**：离屏脚本造不出真实输入法，OS 层的候选键本来也不会送到应用。
  该条记为「已实现，未验证」，与 CER-07 的真机 IME 核对一起做。
- 条目仍只有 `label + hint` 两段文本，没有图标（合同没要求，刻意不做）。
- 菜单不跟随窗口缩放重算位置：每帧按光标重算，窗口极小时可能贴边（现在只有下/右两个方向的夹取）。

### 6.6 顺带修掉一个环境假失败（与本包无关，但挡路）

跑全量单测时 `mw-shell` 56 条、`mw-format` 8 条**写盘用例**全红，报 `os error 5 拒绝访问`，
位置都是 `create_dir_all(<用户 Temp>/...)`。同一台机器上用 PowerShell 手建目录却是成功的——
是本机用户 `%LOCALAPPDATA%\Temp` 对**测试进程**拒写（`demo_notes.rs` 早就记过这个坑）。

处置：

- `scripts/ci.ps1` 与 `scripts/x100-stage-b-gate.ps1` 启动时把 `TEMP`/`TMP` 指到仓库内 `target/tmp/ci-temp`；
- `mw-format` 的单测与集成测试改成「仓库 `target/tmp` 优先、Temp 兜底」，跑 `cargo test` 不再看本机 Temp 脸色。

证据：把 `TEMP` 指到仓库内后，`mw-format` 18/18、`mw-shell` 的 `window_state:: trace:: autosave::` 22/22 全过。
这属于**测试环境**问题，不是代码回归；CI 的绿灯不该被本机 Temp 权限左右。

### 6.7 第二次踩坑：`scroll_to_me` 的动画拍出两张不一样的帧

第一版为了「高亮行永远可见」调了 `resp.scroll_to_me(...)`。门里立刻炸了：`card-slash-filter` **两遍各自失败**
（第 1 遍差 **9915 px**、第 2 遍差 **8269 px**，包围盒都是 `x[319..720] y[545..573]`，正是一行的高度）。

原因：`scroll_to_me` 不是「跳过去」而是「滚过去」——egui 会补一段动效，两遍采样落在动效的不同帧上。
改成自己算偏移、不带动效：

```rust
let keep_in_view = (*sel).saturating_sub(rows.saturating_sub(1)) as f32 * SLASH_ROW_H;
ScrollArea::vertical().vertical_scroll_offset(keep_in_view)
```

语义也更准：只有高亮**越过最后一可见行**时才滚动（选第 1 条时不动）。改完三张各跑两遍，**逐字节相同**：

| 场景 | 第 1 遍 | 第 2 遍 |
| --- | --- | --- |
| `card-slash` | `45B5C6D5024D` | `45B5C6D5024D` |
| `card-slash-filter` | `839B0D27D41F` | `839B0D27D41F` |
| `card-slash-scroll` | `CF09AA3CA59F` | `CF09AA3CA59F` |

这条和 CER-17 的「悬停保持 400ms」是同一类：**动效会在 golden 里变成噪声**，要么等它停，要么别用它。
