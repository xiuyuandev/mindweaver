# CER-33 · 主题 / DPI / 中文 / 性能总验收

- **日期**：2026-11-02
- **批次**：B2-D（`docs/29` §9）
- **状态**：**进行中** —— 36 张矩阵、大纲/卡片/库三条性能主项、卡片 IME 帧耗时、
  CJK 禁则两组测试、减少动画单测与 release 哈希均已落地；探针像素中性经 `library-long`
  零差异确认。仍未收口的原因：实机微软拼音与系统 DPI 切换欠人工，且新探针要等 CER-34
  按最终树重跑两遍 stage-b。本包不新增功能，只做总验收与缺陷修复。

---

## 1. 范围

`docs/29` §9 CER-33 的七项交付，逐项落点：

| # | 合同项 | 判据出处 | 本包要交的证据 |
| --- | --- | --- | --- |
| 1 | 四主题 × 1.0 / 1.5 / 2.0 DPI | `docs/28` §5.8、§6、§7 | 三界面各 12 张矩阵图 + 每张 SHA256（`matrix/<surface>/<theme>-<ppp>/`） |
| 2 | CJK 禁则 | `docs/28` §6.3 | 既有单测清单 + 三界面长中文换行基线复核 |
| 3 | 微软拼音 | `docs/28` §6.1 / §6.8 | 构造事件 IME 脚本（preedit / commit）+ 实机核对记录（人工） |
| 4 | 20k 大纲 | `docs/28` §5.8：首屏 ≤100ms、滚动 ≥55fps、折叠/展开 ≤16ms | release 探针日志 + stats 末行 |
| 5 | 20k 字符卡片 | `docs/28` §6.9：打开 ≤150ms、输入无 >50ms、IME ≤16ms | `MW_CARD_PERF` 逐帧日志 |
| 6 | 10k 卡片库 | `docs/28` §7.4：首屏 ≤100ms、滚动 ≥55fps | `library-long` stats + 检索上界单测 |
| 7 | 减少动画 / release / 缺陷 | `docs/28` §4.5、`docs/29` §9 | `reduce_motion` 证据、release 体积/哈希、缺陷清单 |

## 2. 非目标

- 不新增功能、不改 `.zmind` / `Node` / `NoteCard`；不借验收重做三界面。
- 不把「harness ppp」冒充 Windows 系统缩放：`cargo golden --ppp` 只改渲染缩放，
  真实系统 DPI 切换仍需实机人工核对（与 B2-C 的拼音欠账同一口径）。
- 不重拍无关基线；矩阵图是新证据，不进 `stage-b` 门。
- 不在 G-B2-4 未通过时宣布 B2 完成。

## 3. 现状盘点（开工前已有什么）

| 项 | 已有能力 | 位置 / 日期 |
| --- | --- | --- |
| 大纲 20k 矩阵 | **已有 12 张**（4 主题 × 3 DPI）+ perf 探针 + stats | `shots/x100/CER-11/matrix/`、`perf.txt`、`stats.log`（2026-10-26） |
| 大纲 20k 探针 | `outline_20k_frame_budget_probe`（`#[ignore]`，release 手动跑） | `crates/mw-shell/src/panels/outline.rs:6943` |
| 卡片 20k | 夹具 `MW_CARD_20K=1`、逐帧探针 `MW_CARD_PERF=<文件>`；2026-10-29 实测打开 5.5ms / 输入 p95 3.6ms / IME max 4.4ms | `crates/mw-shell/src/app_methods_1.rs:1360`、`shots/x100/CER-20/report.md` §4 |
| 卡片 20k 金样 | `card-20k` 已进 stage-b 门（ppp 1.0、1200×1000、warmup 12） | `scripts/x100-stage-b-gate.ps1:175` |
| 库 10k | `MW_LIBRARY_DEMO=long` = 10000 张；`library-long` 已进门；另有 `library-ppp1.0/1.5/2.0` 三 DPI | `crates/mw-shell/src/demo_notes.rs`、`scripts/x100-stage-b-gate.ps1:110` |
| 库检索上界 | `search_thousand_cards_is_bounded` 等单测 | `crates/mw-shell/src/note_store.rs` |
| 减少动画 | 系统开关 + 用户设置 + 金样三源合成；`reduce_motion` 下过渡 0ms | `crates/mw-shell/src/main.rs:118`、`settings.rs:14`、`canvas/motion.rs:19` |
| CJK 禁则 | `enforce_kinsoku` + DWrite 兜底后重新量宽；三界面共用；行首/行尾/整篇三组测试按禁则清单自动生成 | `crates/mw-core/src/text.rs:48-65`、`crates/mw-render/src/dwrite.rs:256`、`crates/mw-render/tests/text_measure.rs:32-72` |
| IME 构造事件 | `card-ime-preedit/commit`、`outline-edit-ime` 输入脚本 | `shots/golden/inputs/` |

## 4. 执行计划（待树静默）

### 4.1 矩阵（36 张）

新脚本 `scripts/x100-cer33-matrix.ps1`，按 `scripts/x100-outline-20k.ps1` 的隔离方式
（`APPDATA`/`LOCALAPPDATA` 指到 `target/x100/`），三个表面各跑 4 主题 × 3 DPI：

| 表面 | 环境 | 窗口 |
| --- | --- | --- |
| 大纲 20k | `MW_OUTLINE=1 MW_OUTLINE_20K=1` | 1280×800 |
| 卡片 20k | `MW_CARD_20K=1` | 1200×1000 |
| 库 10k | `MW_LIBRARY_DEMO=long` | 1280×800 |

产物：`shots/x100/CER-33/matrix/<surface>/<theme>-<ppp>/`，每目录一张
`shell-ppp<0.00>.png`；每个表面一份 `hashes.txt`（theme / ppp / SHA256 / 路径）。
断言只做「文件存在、尺寸非零、哈希写入」，不做跨主题像素比较（主题本来就不同）。

**已跑（2026-11-02 10:1x）**：3 表面 × 4 主题 × 3 DPI = **36 张，全部非空**；
`hashes.txt` 各 12 行。接触表 `matrix/{outline,card,library}-contact.png` 由 36 张原图拼成，
逐张目视结论见 §4.5。脚本首跑因缺 UTF-8 BOM 被 Windows PowerShell 5.1 按 ANSI 解析失败，
补 BOM 后通过（与 `scripts/` 其余脚本一致）。

### 4.2 性能

| 场景 | 命令（TEMP/TMP → `target/tmp`） | 预算 |
| --- | --- | --- |
| 大纲 20k | `cargo test --release -p mw-shell --bin mw-shell outline_20k_frame -- --ignored --nocapture` | 首屏 ≤100ms / 滚动 p95 ≤16ms / 折叠 ≤16ms |
| 卡片 20k | `MW_CARD_20K=1 MW_CARD_PERF=<log> MW_GOLDEN_INPUT=shots/golden/inputs/card-typewriter.txt cargo golden --ppp 1.0 --warmup 24 --dir target/x100/cer33-card-perf --update` | 打开 ≤150ms / 输入无 >50ms / IME ≤16ms |
| 库 10k | `MW_LIBRARY_DEMO=long MW_STATS=1 MW_STATS_LOG=<log> cargo golden --ppp 1.0 --warmup 60 --dir target/x100/cer33-lib-perf --update` | 首屏 ≤100ms / UI p95 ≤16ms |

记录口径必须写明：release 构建、窗口尺寸、ppp、预热帧数；离屏 harness 无 vsync，
所以「滚动 ≥55fps」用 **UI p95 ≤16ms** 判，不用 wall fps 冒充。

**库首屏探针已补（2026-11-02）**：`trace::library_frame_ms`（`MW_LIBRARY_PERF=<文件>`）
在 `Shell::ui` 的 `library_ui` 外层计时，与 `MW_CARD_PERF` 同形；首行就是库页第一次上屏。
10k 夹具 `MW_LIBRARY_DEMO=long` 只建内存元数据、不读磁盘，所以该行**不含冷启动索引**。
滚动脚本 `shots/golden/inputs/library-10k-scroll.txt`（指针落在列表栏后连发四帧触控板滚动）
滚动到第 160+ 张，截图 `target/x100/cer33-lib-perf/` 可证列表确实在动。

**实测（release，2026-11-02 10:1x–10:3x）**：

| 场景 | 命令 / 日志 | 实测 | 预算 | 判 |
| --- | --- | --- | --- | --- |
| 大纲 20k | `outline-20k-probe.log` | 首屏 **34.1ms**；滚动 p50 **0.18** / p95 **0.51** / max **0.73ms**；折叠 **0.32ms** / 展开 **0.30ms** | ≤100 / ≤16 / ≤16 / ≤16 | **过** |
| 卡片 20k | `card-20k-perf.log`（typewriter 37 帧） | 打开 **12.688ms**；p50 **0.635** / p95 **0.927** / max **12.688ms**；无 >50ms 帧 | ≤150 / 无 >50 | **过** |
| 卡片 20k · IME | `card-20k-ime.log`（preedit / commit 16 帧） | 打开 **12.351ms**；preedit 帧 **3.876ms**、commit 帧 **1.255ms**；全帧 p95 **2.362ms** | IME ≤16 | **过** |
| 库 10k | `library-10k-perf.log` + `library-10k-stats.log` | 首屏 **12.971ms**；p50 **0.147** / p95 **0.760** / max **12.971ms**；stats UI p50 **0.10** / p95 **0.13–0.14ms**（含滚动帧） | ≤100 / UI p95 ≤16 | **过** |

stats 末两行的 wall p95 是 18.0ms、UI p95 是 0.13–0.14ms：离屏 harness 无 vsync，
wall 间隔被合成器节流，按事先写定的口径只认 UI 帧耗时（`docs/08` §8）。
卡片 20k 的档位与 `CER-20` 的 5.5ms 不同源于夹具/帧计入口径（本轮连打开帧一起记），
预算仍以 150ms 判。

### 4.3 中文与减少动画

- IME：跑上述构造事件脚本，记录 preedit / commit 帧耗时与文字落位；
  **实机微软拼音仍需人工**（与 CER-16/17/20 的同一欠账合并）。
- CJK 禁则：复核 `enforce_kinsoku` 单测 + 从矩阵里挑「长中文段落」三张图目视；
  不新增断行算法。
- 减少动画：`settings` 基线与单测（`demo::reduce_motion_lands_on_the_target_immediately`）
  + 代码审计「所有过渡都读 `motion_reduced`」；不在四主题下重复拍动画帧。

**已跑（2026-11-02）**：

- CJK 禁则：`cargo test -p mw-core --lib text` → **35 passed / 0 failed**；
  `cargo test -p mw-render --test text_measure` → **12 passed / 0 failed**。
  钉住的行为包括 `no_start_punctuation_never_begins_a_line`、
  `no_end_bracket_never_ends_a_line`、`hanging_dash_refills_the_rest_of_the_paragraph`、
  `prose_corpus_keeps_every_line_legal`、`placeholder_measurer_also_obey_kinsoku`。
- 矩阵里的中文长文（卡片页 20k、库 10k 行标题与摘要、大纲 20k）逐张目视未见
  行首标点、行尾开括号或整段溢出。
- 减少动画：`cargo test -p mw-shell --bin mw-shell reduce_motion -- --test-threads=1`
  → `demo::tests::reduce_motion_lands_on_the_target_immediately` **1 passed / 0 failed**；
  过渡路径统一读 `motion_reduced`（`settings` / `canvas/motion.rs` / `main.rs` 三源合成）。
- IME 构造事件已跑（见 §4.2 表）；**实机微软拼音与系统 DPI 切换仍欠人工**。

### 4.4 工程与缺陷

- `cargo build --release -p mw-shell`：记录 exe 体积与 SHA256。
- 跑完矩阵后按主题/DPI 逐张目视，缺陷进本报告 §5；能修的在本包修，
  修了会让既有基线漂移的（如 F1/F2）留给 CER-34 一起重拍。

**release**：本轮量了两个时点，因为并行会话 10:25–10:30 改过 `mw-format`：

| 时点 | 体积 | SHA256 | 说明 |
| --- | --- | --- | --- |
| 2026-11-02 10:23 | 25,618,944 B（24.43 MiB） | `ed2ee62107bfb77a3df931a7a4804064d5053f1cf8def245abf20b2ed7531788` | 含 `MW_LIBRARY_PERF` 探针；矩阵、大纲/卡片/库性能、卡片 IME 都用它 |
| 2026-11-02 10:40 | 25,714,688 B（24.52 MiB） | `4178826b6634d3ac16eb2f36098ff81de220d59923a85e284389971d2bf9d35a` | 合入并行会话的 `mw-format` 改动后的最终树；`library-long` 零差异比较用它 |

`logs/release-hash.txt` 记录的是后一次。CER-34 全量回归时会按最终树再重记一次。

### 4.5 矩阵目视结论（AI 目视，不是人工判词）

按 `matrix/{outline,card,library}-contact.png` 逐格看 36 张：

- **大纲 20k**：四主题 × 三 DPI 的层级线、缩进、行尾角标一致；ink 主题无浅底串色；
  2.0 下字阶与命中区域按比例放大，无裁切。
- **卡片 20k**：版心在各 DPI 下保持居中、两侧留白对称；ink 正文与背景对比足够；
  mist/sand 的纸色与字色同族；滚到中段的段落无行首禁则违例。
- **库 10k**：1.0/1.5/2.0 分别落三栏 / 两栏 / 单栏，侧栏标签树、列表三行式、
  预览空状态均无重叠；ink 主题侧栏与列表的层级差可辨；`10000 张` 计数正确。
- **未发现新的确定性缺陷**；三张接触表作为目视底稿留在 `matrix/`。

## 5. 未做 / 已知偏差

- **已交**：36 张矩阵（含目视底稿）、大纲/卡片/库三条性能主项 + 卡片 IME 帧耗时、
  CJK 禁则两组测试、减少动画单测、release 体积与 SHA256、`MW_LIBRARY_PERF` 探针与
  10k 滚动脚本。探针的像素中性已用 `library-long` 既有基线比较确认：**零差异**。
- 实机微软拼音未核对（沿用 B2-C 欠账）；本轮 IME 只是构造事件。
- 系统 DPI 缩放切换未测，矩阵只覆盖 harness `--ppp`。
- 离屏无 vsync：帧率判据用 UI 帧耗时，与实机 vsync 表现可能不同。
- 10k 库夹具只建内存元数据，首屏数字**不含磁盘索引**；冷启动含索引另记。
- `MW_LIBRARY_PERF` 是新增探针代码（env 门控、不改绘制），**CER-34 仍需重跑
  stage-b 两遍**按最终树背书；本轮 `library-long` 零差异只覆盖一个场景。

## 6. 风险

| 风险 | 说明 |
| --- | --- |
| 36 次 `cargo golden` 串行耗时 | 本轮已完成（每次重查源码，整轮约 4 分钟）；并行会话若中途改 `mw-shell` 仍会让下一轮作废 |
| 并行会话改 `mw-format` 截断收尾 | 2026-11-02 10:25 起 `crates/mw-format/src/lib.rs` 处于 `concept_map` 未导入的中间态，卡住卡片 IME 与减少动画两条收尾；不在本包修，等树静默 |
| 夹具写入真实库 | 矩阵必须带 `APPDATA`/`LOCALAPPDATA` 隔离；本轮隔离在 `target/x100/cer33-*`，未碰真实笔记库 |
| 主题色在 DPI 2.0 下对比度 | SE-11 的对比度门是 1.0 DPI 计算的；2.0 只影响物理像素，不改变颜色比值，矩阵用于目视确认 |
| 缺陷修复与 CER-34 重拍耦合 | F1/F2 一旦在本包修，`card-*` 基线必须同批重拍，不能留「半新半旧」 |
| 新探针改变二进制 | `MW_LIBRARY_PERF` 只多一次 `Instant::now` 与 env 门控的写文件，不参与绘制；仍需在树静默后跑一遍 `library-long` 既有基线比较 + CER-34 两遍门背书 |

## 7. 人工判词

CER-33 不是门包（门包是 CER-34 / G-B2-4），本报告不代写判词。
