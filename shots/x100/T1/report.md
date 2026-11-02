# T1 · 抽壳层令牌、零像素漂移

> 施工依据：`docs/35-品牌与全局主题系统.md` §7（第一包施工单）
> 状态：`已实现`（**待跑 golden 门**：`--tol 0` 全量两遍尚未执行，见 §6）

---

## 1. 范围

把散落在壳层各处的**颜色 / 图标字号字面量原值**搬进 `mw_core::design::shell`，
只改引用路径、**不改任何取值**。目标判据：golden 逐像素零差异。

## 2. 非目标

- 不合并重复语义（如 `warn` 的四支色、"面板白" 的三种来源）——那是 T2。
- 不改字号阶梯、圆角档、描边宽度、间距（那是 T2/T3）。
- 不动 `theme.rs` 四套主题、`contrast.rs` 七条硬门。
- 不动被并发会话占用的文件（见 §5）。

## 3. 改了哪些文件

| 文件 | 改法 |
| --- | --- |
| `crates/mw-core/src/design.rs` | 新增 `pub mod shell`（内含 `pub(crate) mod primitive` + `pub use primitive::*`）；`icon` 补 `GLYPH_MEDIUM` / `GLYPH_LARGE`；新增字节钉死单测 |
| `crates/mw-shell/src/chrome.rs` | `c32` 改 `const fn`；新增 `c32a`（保 alpha）；`chrome_of` / `Glyph` / `Tone::of` 全部改引用令牌；新增 `c32a_keeps_alpha` 单测 |
| `crates/mw-shell/src/main.rs` | `CLOSE_HOT` 常量改引用 `shell::CLOSE_HOT` |
| `crates/mw-shell/src/note_view.rs` | `tag_color` 的 8 支调色板 → `shell::tag::*`；测试里的 `MdPalette` 默认值 → `shell::markdown::*` |
| `crates/mw-shell/src/stats.rs` | 性能浮层底与字色 → `shell::stats::*` |
| `crates/mw-shell/src/app_methods_2.rs` | 演讲者备注浮层、演示条底/描边/阴影 → `shell::overlay::*` |
| `crates/mw-shell/src/panels/inspector.rs` | 检查器 `well` 的两种情况与「偏低」警示色 → `shell::inspector::*` |

## 4. 验收场景

1. `cargo fmt --all -- --check` 通过。
2. `cargo clippy --workspace --all-targets` 与 `scripts/clippy-baseline.txt` 比对，新增 0。
3. `cargo test --workspace -- --test-threads=1`（`TEMP`/`TMP` 指向 `target/tmp`）全绿；
   新增两条：`shell_tokens_match_the_old_literals`（23 组旧 hex 逐项断言）、`c32a_keeps_alpha`。
4. **待跑门**：`stage-b` 178 场 / 356 次比较、`stage-cm` 73 场 / 146 次比较，
   `--tol 0` 连跑两遍零差异（写本包结论前必须补）。

## 5. 未做与风险

- **golden 门未跑**（用户指令：另一会话正在重拍带箭头的基线，此刻跑「零差异」拿到的是别人的快照）。
  本包结论暂记 `已实现`，**不记 `已验证`**。
- `design::shell` 对外是 `pub`：`mw-shell` 是另一个 crate，`pub(crate)` 在跨 crate 场合不可见。
  为满足「`mod primitive` 保持 `pub(crate)`」，内部用 `pub(crate) mod primitive` + `pub use primitive::*`
  对外暴露同一个路径，primitive 模块本身不可被下游命名。
- §7 第 17 组原名 `shell::syntax`，但该 8 支色的唯一消费点是 `note_view::tag_color`（标签色盘），
  与语法高亮无关 → 改名 `shell::tag`，值一字未动。
- §7 第 19 组把 `from_rgba_unmultiplied(24,27,31,232)` 拆成 `SCRIM` + `SCRIM_ALPHA`，消费端仍走
  `from_rgba_unmultiplied` 语义（`c32a`），**没有**改成预乘路径。

### 5.1 T1b 待迁（被并发占用）

对本轮列出的禁用文件做了逐文件扫描（`from_rgb(0x` / `from_rgba_unmultiplied` / `from_gray` /
`from_white_alpha` / `from_black_alpha`），**没有**发现属于 §7 搬运表 23 组的字面量：

| 文件 | 扫描结果 |
| --- | --- |
| `crates/mw-render/src/instances.rs` | 无命中 |
| `crates/mw-render/src/hit.rs` | 无命中 |
| `crates/mw-shell/src/export_svg.rs` / `export_pdf.rs` / `export.rs` | 无命中 |
| `crates/mw-core/src/scene.rs` / `document.rs` / `search.rs` | 无命中 |
| `crates/mw-shell/src/canvas*.rs` | 只有两处**通用**转换（`canvas.rs:1765/2093` 把 `Rgba`→`Color32`），不是令牌字面量 |

结论：**T1b 清单为空**；被并发占用的文件不需要为 T1 改动。

## 6. 证据（2026-11-02 本机实测）

### 6.1 fmt

```
rustfmt --edition 2021 crates/mw-core/src/design.rs \
        crates/mw-shell/src/app_methods_2.rs crates/mw-shell/src/panels/inspector.rs   # exit 0
cargo fmt --all -- --check                                                              # exit 0
```

过程：先只 `rustfmt` 本轮改的 3 个含格式 diff 的文件（`design.rs` / `app_methods_2.rs` /
`panels/inspector.rs`，**都是本轮自己的改动**）。中途 `--check` 还报过 `geom.rs` / `cmap.rs` /
`fidelity.rs` / `dwrite.rs` / `text.rs` / `canvas/tests.rs` 的 diff——全在别人的在飞文件里，
按纪律不碰；并行会话随后自己格式化了它们，**最终整仓 `--check` exit 0**。

### 6.2 clippy

```
cargo clippy -p mw-core --all-targets          # exit 0，design.rs 零告警
cargo clippy --workspace --all-targets         # exit 0；告警全落在基线既有文件里
cargo clippy -p mw-shell --all-targets         # 对本轮 6 个文件零命中
```

`cargo clippy --workspace --all-targets` 的告警（`mw-render` 的 `offscreen.rs` / `compositor.rs` /
`glyph.rs` / `hit.rs` / `instances.rs` / `text.rs`、`mw-core` 的 `concept_layout.rs` /
`concept_tree.rs` / `layout.rs` / `scene.rs` / `theme.rs`、`mw-core/examples/perf_scene.rs`）
**一处都不在本轮改的 7 个文件里**，属 `scripts/clippy-baseline.txt` 既有项 → 新增 0。

> 过程记录：中途 `crates/mw-shell/src/export_pdf.rs:1511` 曾有并行会话的未提交类型错误
> （`tree_edge_outline(e, 1.0)` 期望 `&SceneEdge`、得 `SceneEdge`），该文件在禁用清单内不能修，
> 一度让 `mw-shell` 编译中断、clippy 与 `--workspace` 测试都跑不了。并行会话随后修好了它
> （期间我一行没动该文件），下面 6.3 的实测是在修好之后跑的。

### 6.3 test

```
cargo test -p mw-core --lib                                     # 384 passed / 0 failed
cargo test --workspace -- --test-threads=1                      # 1356 passed / 0 failed / 1 ignored
cargo test -p mw-shell c32a_keeps_alpha                         # 1 passed / 0 failed
```

- `shell_tokens_match_the_old_literals`：**通过**（44 支色 + 11 个 alpha + 两个图标字号逐字节断言）。
- `c32a_keeps_alpha`：**通过**（`c32a` 保 247、`c32` 抹成 255、两条路径结果不等）。
- 1356 是**本次实测**；它会随并行会话加用例变化，数字口径仍以 `docs/STATUS.md` §1 为准。

### 6.4 golden（待跑门）

**未跑**。用户指令：另一会话正在重拍带箭头的基线，此刻跑「零差异」拿到的是别人正在改的快照。

判据（等协调信号后执行，两遍都必须零差异、不许放 tol）：

```powershell
powershell -File scripts/x100-stage-b-gate.ps1 -Tol 0     # 178 场 / 356 次比较
powershell -File scripts/x100-stage-cm-gate.ps1 -Tol 0    # 73 场 / 146 次比较
```

### 6.5 逐字节搬运的独立旁证

`docs/35` §7.1 的 23 组旧字面量与本轮 `shell::*` 的取值逐项对齐（PANEL2 `F2F3F5`、
SEP/BORDER/BORDER_HOT `E8EAEE`/`E6E9ED`/`D8DDE4`、EDGE `969696`、TEXT `3E444C`、
DIM `5C646F`、FAINT `656D78`、HOVER `F1F3F6`、ICON_OFF `C6CBD2`、WARN `B46A1E`、
dark FAINT `8B929B`、dark WARN `E9A05C`、tone `E0E4EA`/`ECEFF3`/`6B581C`/`FFE7A3`/
`FFE9A8`/`4A3500`/`212`/`242`、CLOSE_HOT `C42B1C`、tag 八色、markdown 五色、
SCRIM `(24,27,31,232)`、INK `E8EBEF`、overlay `(18,21,26,228)`/`(255,255,255,22)`/
`(26,30,36,238)`/`(255,255,255,20)`/`80`、inspector `18`/`18`/`C05A4A`）。

## 7. T2 输入：本轮发现的重复同义值（**只记录，未合并**）

按用户第 5 条要求，搬迁中发现的第二份同义值一律不合并，列在这里交给 T2：

| 位置（那一行长什么样） | 与哪支令牌同值/近义 | 备注 |
| --- | --- | --- |
| `app_methods_3.rs:1522` · `Color32::from_rgb(0xC4, 0x2B, 0x1C)` | 与 `shell::CLOSE_HOT` **同值第二份** | 关闭按钮的另一处绘制路径 |
| `app_methods_2.rs:238` · `.color(Color32::from_rgb(0x9A, 0xA2, 0xAD))` | 演讲者备注正文灰，无令牌 | 与 `:303` 同值（同文件两处） |
| `app_methods_2.rs:244` · `.color(Color32::from_rgb(0xE9, 0xEC, 0xF1))` | 备注标题近白，无令牌 | 与 `:297` 同值（同文件两处） |
| `app_methods_2.rs:250` · `.color(Color32::from_rgb(0x7F, 0x8A, 0x99))` | 备注次要灰，无令牌 | — |
| `app_methods_3.rs:1524` · `Color32::from_rgb(0x2B, 0x33, 0x40)` | 深底，无令牌 | — |
| `app_methods_4.rs:79` · `.color(Color32::from_gray(110))` | `from_gray` 系列第三份 | 与 `tone::OFF_LIGHT`/`CHIP_LIGHT` 同一族写法 |
| `main.rs:1363` · `.rect_filled(rect, 8.0, Color32::from_white_alpha(22))` | 与 `shell::overlay::NOTES_STROKE` **同值**（`255,255,255,22`） | 不同浮层，需 T2 判是否同一语义 |
| `main.rs:1366` · `Color32::from_gray(92)` | 无令牌 | — |
| `main.rs:1370` · `Color32::from_rgb(0xE6, 0xEA, 0xF0)` | 近 `shell::light::SEP`（`E8EAEE`）与 `EDGE`，**差 2 个色阶** | 注意：**不是同值**，T2 合并会改像素，须先量 |

另外两条口径提醒（会影响 T2 怎么做）：

1. `chrome_of()` 的暗色分支仍直接取 `ThemeUi`（`panel / panel2 / sep / border / text / dim / hover / accent`），
   只有 `faint / icon_off / warn` 用了 `shell::dark::*`。语义层收口时不能简单把这些也搬成常量，
   否则会切断 `theme.rs` 的"多主题跟随"。
2. 半透明令牌现在是 **alpha 常量 + 由它派生的 `Rgba::rgba(...)`**（SCRIM / NOTES_BG / NOTES_STROKE /
   BAR_BG / BAR_STROKE）。T2 若改成 `Semantic` 结构字段，必须保持"改 alpha 常量即改整块"这一条，
   否则会重新分叉。
