# TYPO-01 · 全局字体与排版打磨（打磨轮 65）

> 用户口径（2026-11-01）：「打磨所有页面的字体、字号、字间距等和文字相关产生的视觉效果，
> 目前的整个产品看着累，而且不够舒服，不够美观。」
> 方案与审计快照：[`docs/x100/typography-plan-2026-11-01.md`](../../../docs/x100/typography-plan-2026-11-01.md)。
> 本包**只动排版**：字体栈、字阶、字重、行高、文字颜色/对比度、随字走的几何；不动主题色板、
> 不动图标、不新增依赖、不碰 `.zmind` / `Node` / `NoteCard` 字段。

## 1. 做了什么

一句话：把「小字当家 + 灰得看不清 + 假字重 + 三套字阶」四件事一起收口。

1. **字体栈**：DirectWrite 改为 `Segoe UI → Microsoft YaHei UI → Microsoft YaHei → Noto Sans SC`
   （拉丁/数字拿 Segoe，中文按字形回退雅黑）；egui 侧注册真 `Segoe UI`、`Segoe UI Semibold`，
   新增 `zhisi-semibold`（拉丁 600 + 中文常规）与 `zhisi-bold`（拉丁 600 + 中文粗体）。
2. **字阶**：外壳六档 12/13/14/15/16/26；内容十四档收敛为整数档；画布三级 16、四级 15、
   摘要 14、命题短语 13；开始页/检查器/卡片页的私有字号表全部接回令牌。
3. **真字重**：删掉全部 `.strong()`（egui 0.37 只改颜色不改字重）与检查器的 0.4px 双描；
   `TextStyle::Heading`、内容 600/500、菜单/快捷键/列表标题改走 `zhisi-bold` / `zhisi-semibold`。
4. **颜色**：浅色外壳 `dim` `#6E7681 → #5C646F`、`faint` `#A3A9B2 → #656D78`；暗色补出真正的
   第三级 `faint #8B929B`；内容 `text_faint` 0.56/0.58 → 0.68/0.68（四套主题、page/panel 全部 ≥4.5）。
   标签不再用粉彩字，统一 6px 色点 + `ch.text`（8% 色底）。
5. **行高与几何**：大纲换行步进 24 → 26、根行 46、有备注 +22；菜单/命令面板行 32、快捷键 30、
   搜索行 46/页脚 30、库导航 32、状态栏 26、开始页品牌头 48、标签页 30、卡片页顶栏 44。
6. **中文禁则**：`dwrite::wrap_paragraph` 的避头尾回填改成「按首个可重排位置重切尾部 + 合法性复核」，
   Segoe 优先后仍保持全角标点不孤立、不来回振荡。

## 2. 关键落点与证据入口

| 主题 | 落地 | 证据 |
| --- | --- | --- |
| 画布字体栈与 600 字重 | `mw-render/src/dwrite.rs`：`FONT_CANDIDATES`、`WEIGHT_BOLD = 600` | `mw-render/tests/text_measure.rs`（family = `Segoe UI`，拉丁宽 26.981）；`shots/golden/canvas-text-ppp1.00.png` |
| 画布字号 | `mw-core/src/design.rs::text`（SUB 16 / LEAF 15）、`scene.rs`（SUMMARY 14 / REL_LABEL 13） | `design` 单测、`scene` 金样 |
| egui 字体族 | `mw-shell/src/fonts.rs`：`SEGOE` / `SEGOE_SEMIBOLD` / `BOLD` / `SEMIBOLD` | `fonts` 单测；`content_kit::font_for` 映射测试 |
| 外壳字阶 | `design::ui` 12/13/14/15/16/26；`chrome::Type` 只做别名 | `chrome` 单测（含源码扫描） |
| 内容字阶 | `design::content` 十四档：PAGE_TITLE 30/42、OUTLINE_TITLE 28/40、H1 22/33、LIST_TITLE 15/22/600、LIST_EXCERPT 13/19、LIST_META 12/18、NAV 14/21、TAG 12/18/600、CODE 14/22 | `design` 单测（单调、行高比 ≥1.4） |
| 对比度 | `chrome.rs` 浅色 dim/faint + 暗色 faint；`design::content::{LIGHT,DARK}_ALPHAS` 的 0.68 | `mw-core/tests/contrast.rs` 七条（page/panel/panel2 分表面下限）；`chrome` 单测 |
| 大纲行高 | `panels/outline.rs`：根 46、`row_layout_height` +26、`row_text_job` line_height 26 | `outline` 单测（46/32/26） |
| 卡片页 | `note_view.rs`：BODY_LINE 28、CODE 14/22、CODE_LANG 12、TOP_BAR 44、标签色点 | `card-*` 金样 |
| 笔记库 | `note_library.rs`：列表 15/13/12、导航 32、标签色点 + `ch.text` | `library-ppp*` 金样 |
| 搜索 | `panels/search.rs`：ROW 46、FOOTER 30、字段角标/页脚 12 | `search-*` 金样 |
| 检查器 | `panels/inspector.rs::geo`：40/44/30/28；文字 13/14/12 | `inspector-*` 金样与 geo 断言 |
| 菜单/命令/快捷键 | 行 32/32/30；标签 15/14 | `menu` 单测、`shortcuts-*` 金样 |
| 开始页 | `start.rs`：品牌 48、SUBTITLE 13、QUICK 15、ROW 14 + 12、KEYCAP 12、FOOTER 13 | `start-*` 金样 |
| 状态栏/标签页 | `main.rs`：STATUS_H 26；`design::component::TAB_H` 30 | `start`/`shell` 金样 |

## 3. 测试

- 新增/更新：`design` 字阶与几何断言、`contrast` 分表面 AA、`content_kit::font_for` 500/600 映射、
  `chrome` 禁止 `.strong()` 与裸字号扫描、`menu`/`search`/`inspector`/`outline`/`note_view` 几何。
- mw-render：`text_measure` 12 条、`glyph` 4 条（CJK 直测面改用雅黑，图集测试不经过 DWrite 回退）；
  `dwrite` 的避头尾回填新增多字标点重切与合法性复核。
- 全量（串行、TEMP 重定向）：`1202 passed / 0 failed / 1 ignored`（2026-11-01 20:12，23 套件；
  含并行会话在本包执行期间新增的 drill / 卡片用例）。

## 4. 金样与门

- 外观有意改动，`stage-b` 全矩阵按 `--tol 0` 重拍；`mw-render` 两张离屏基线
  （`canvas-text-ppp1.00`、`text-canvas`）同步重拍。
- `stage-b`：**153 场 / 306 次比较**（本包重拍时脚本登记数；较 `docs/STATUS.md` 第 59 轮的 145
  多出 8 场，来自打磨轮 60/61/62/64 与并行会话新增的 drill/card 场景，不是本包新增）。
  重拍与复核结果见 §6。
- `stage-cm`：**72 场 / 144 次比较**（含此前一直缺基线的 7 张 CM-14 场景，本轮一并补齐）。
  重拍与复核结果见 §6。
- 混合排版决策门：`canvas-text-ppp1.00.png` 的「中文Rust混排」显示基线一致、全角标点仍是中文字形、
  拉丁/数字已用 Segoe —— 不退回「雅黑优先」。证据图在 `shots/golden/canvas-text-ppp1.00.png`。

## 5. 未做 / 偏差 / 风险

- **未做**：`typography-plan` §5.3 列的 10 张新基线没有逐个另起场景；本包用既有 153 场全矩阵重拍
  覆盖同一批页面（`typo-*` 专项场景仍缺，记缺口）。人工目检（100/125/150/200% DPI、微软拼音、
  长中文标题换行）**未做**；M4 实机 IME 仍是阻塞项。
- **偏差 1**：内容 `text_faint` 计划 0.65/0.66，实际 **0.68/0.68**——0.65 在雾青/暖砂的 page/panel
  只有 4.28–4.45，达不到 4.5；已回写方案 §D6。
- **偏差 2**：计划新增 `fonts::regular/medium/emph` 三个包装函数，实际直接用
  `FontFamily::Name(fonts::BOLD/SEMIBOLD)` + `content_kit::font_for`，少一层没有调用方的抽象。
- **偏差 3**：计划新增 `note_view::tag_ink`，实际按方案允许的「最简：色点 + `ch.text`」落地，
  卡片页与库页标签都不用粉彩字；`tag_color` 只做圆点/浅底。
- **风险**：画布三级/四级 +1px 会让部分节点变高，已在全矩阵重拍中接受；若人工门认为节点过胖，
  回退点只有 `design::text::CANVAS_SUB/LEAF` 两个常量。
- **并行工作树**：本包执行期间另一会话仍在改 `canvas*.rs` / `inspector.rs` / `note_view.rs`，
  并在 19:28–19:55 反复重建；正式门脚本每次 `cargo golden` 都会按新源码重编，曾两次撞上
  它未完成的编译。最终证据用**冻结二进制**复刻门环境（§6），不是 `scripts/x100-*.ps1` 的原脚本。

## 6. 复跑记录

**为什么不用原门脚本**：`scripts/x100-stage-b-gate.ps1` / `stage-cm-gate.ps1` 的每场都走
`cargo golden`，会按当时源码重编。并行会话在 19:28–19:55 持续改 `note_view.rs` / `document.rs`
并重建，门脚本一次因编译中断、一次会混入与本包无关的新代码。因此把 **19:20:42 构建的
`target/release/mw-shell.exe`** 冻结为 `target/tmp/mw-shell-typo.exe`（19:30:15），
用 `target/tmp/run_golden.py` 逐场复刻两份门脚本的 **153 / 72 个场景定义**：同一 `--ppp`、
`--warmup`、场景 env、`--tol 0`，且每场独立 `APPDATA` / `LOCALAPPDATA` / `TEMP`。
该二进制包含本包全部排版改动、M31 与 drill 场景。

**stage-b（153 场 / 306 次比较）**

| 步骤 | 时间 | 结果 |
| --- | --- | --- |
| 重拍全部基线 | 19:36:09–19:40:31 | 153/153 写入，0 失败（`target/tmp/typo-golden/update-b/summary.json`） |
| 复核 1（verify-c） | 19:40:50–19:47:08 | 151/153；`card-scroll` 差 0.131%（只在滚动条 6 px 竖带）、`mm-cut-confirm` 100%（基线抓成无确认框） |
| 复核 2（verify-d） | 19:48:03–19:54:41 | 152/153；`mm-cut-confirm` 同一张脏基线复现 |
| 复核 3（verify-e） | 19:57:46–20:01:53 | **153/153 零差异** |
| 复核 4（verify-f） | 20:02:06–20:05:46 | **153/153 零差异** |

瞬态两场的处理：各自单独复跑两次均 0 差异；根因是并行构建负载下启动帧不稳，
`mm-cut-confirm` 的旧基线误抓成「未弹确认框」（均值 245.5，正确态均值 171.7）。
19:57:35 用同一冻结二进制重拍该场（带全屏遮罩 + 确认框，轨迹仍为
`delete-confirm pending=3 selected=1` → `dialog id=id_2993`），随后两次复核均 0 差异。
`card-scroll` 未重拍，复核 3/4 均 0 差异。

**stage-cm（72 场 / 144 次比较）**

| 步骤 | 时间 | 结果 |
| --- | --- | --- |
| 重拍全部基线 | 20:06:01–20:07:21 | 72/72 写入，0 失败；补齐 7 张缺失的 CM-14 基线（`cm-link-menu` / `cm-link-mode` / `cm-link-two` / `cm-rel-reverse` / `cm-phrase-suggest` / `cm-rel-style` / `cm-rel-multi`） |
| 复核 1（verify-cm1） | 20:07:47–20:09:18 | **72/72 零差异** |
| 复核 2（verify-cm2） | 20:09:44–20:11:12 | **72/72 零差异** |

**mw-render 离屏基线**：`canvas-text-ppp1.00.png` 与 `text-canvas` 已按
`MW_GOLDEN_UPDATE=1 cargo test -p mw-render --test ...` 重拍（18:10），随后对应测试通过。

**未产出**：`target/x100/stage-b-gate.log` / `stage-cm-gate.log` 未由原脚本重建——
并行会话停止后应各跑一次 `powershell -ExecutionPolicy Bypass -File scripts/x100-stage-b-gate.ps1`
与 `...stage-cm-gate.ps1`，确认与本节 450 次比较同为零差异。
