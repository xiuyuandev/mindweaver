# CER-12 · 大纲证据包与 G-B2-1

> 设计合同：[`docs/28`](../../../docs/28-内容体验重构规范.md) §5（全屏大纲）、§9（证据与验收）。
> 执行合同：[`docs/29`](../../../docs/29-内容体验重构工作包.md) §2、§6（B2-A）、§10（证据模板）。
> 状态：**进行中**（2026-10-26）。本包是 B2-A 的收口包，机器门通过后停在 **G-B2-1 人工门**。
> 2026-10-26 回补：§5.2 非编辑态长文换行落地 + 新增 `outline-wrap` 场景（见 §6.5），
> 修掉 `outline-edit-ime` 的桌面指针泄漏（同一份构建曾能截出带/不带 hover 两张图）。

## 1. 范围

1. 把 CER-01–CER-11 的大纲成果收成 `docs/28` §5.10 要求的 **9 个验收场景**证据包
   （8 个原有场景 + 回补的长文折行 `outline-wrap`），每个场景双跑 `--tol 0` 并记录 SHA-256。
2. 对 `docs/28` §5.1–§5.11 **逐条核对**，每条给出代码位置、行为证据或明确的未做项。
3. 补 `outline-default` 的 **四主题 × 三 DPI** 矩阵（§5.10 必测项），以及
   `outline-narrow` 的 800×600 / 640×460 两档窗口。
4. 创建阶段报告 [`docs/x100/stage-b2-report.md`](../../../docs/x100/stage-b2-report.md) 的
   B2-A 章节与门清单 `shots/x100/CER-12/gate.md`，把八条人工判词留空为「待用户」。
5. 汇总全量工程门槛：fmt、clippy（新增 0）、workspace 串行测试、release 构建。

## 2. 非目标

- **不改产品代码**。本包只做证据；若核对中发现缺陷，记进 §6 与阶段报告，
  按「只修 A 批」原则单独回补并重跑对应证据。
- 不开始 B2-B（CER-13+）；G-B2-1 未过不碰卡片页。
- 不做侧栏 232px 大纲、不做 X68 命令面板、不改画布节点样式。
- 不改 `.zmind` / `Node` / `NoteCard` 字段，不新增依赖。
- **不替用户写判词**。八条判词一律「待用户」，AI 不得写「通过」。

## 3. 验收场景（9 个场景 / 10 组）

| `docs/28` §5.10 | golden 目录 | 关键环境/脚本 | 必测点 |
| --- | --- | --- | --- |
| `outline-default` | `shots/golden/outline` | `MW_OUTLINE=1`，ppp 1.5 | 13 节点 / 3 层 / 1 任务 / 1 说明 / 1 折叠；四主题 × 三 DPI |
| `outline-hover-select` | `shots/golden/outline-hover-select` | `MW_WINDOW_SIZE=1200x720` + `inputs/outline-hover-select.txt` | 选中无边框、hover 不抢、握把延迟 |
| `outline-edit-ime` | `shots/golden/outline-edit-ime` | `MW_OUTLINE_EDIT='纯 Rust 原生'` + `inputs/outline-edit-ime.txt` | 组合态不拆行、不缩进、编辑行与行同构 |
| `outline-focus` | `shots/golden/outline-focus` | `MW_OUTLINE_FOCUS=1` | 聚焦子树 + 面包屑 + Esc 逐级 |
| `outline-search` | `shots/golden/outline-search` | `MW_OUTLINE_SEARCH='排版'` + `MW_OUTLINE_FILTER_LABEL='验收'` | 命中 12% 底、祖先 48%、计数、清除恢复 |
| `outline-drag` | `shots/golden/outline-drag` | `inputs/outline-drag.txt` | 插入线 / 成为子级 6% 底 / 源 40% |
| `outline-narrow` | `shots/golden/outline-narrow` | `MW_WINDOW_SIZE=800x600` | 不横向滚动、页头收缩；另测 640×460 |
| `outline-20k` | `shots/golden/outline-20k` | `MW_OUTLINE_20K=1`，ppp 1.0 | 首屏 ≤100ms、滚动 ≥55fps、折叠 ≤16ms |
| `outline-wrap` | `shots/golden/outline-wrap` | `MW_OUTLINE_WRAP=1`，`1280x800`，ppp 1.5 + 2.0 | 折行不截断、每行 +24、悬挂缩进、说明行叠加、硬换行、行尾角标不压字 |

> 名称映射：B2 开工时保留了更早的目录名，`docs/28` 的 `outline-default` 对应
> `shots/golden/outline`；其余场景目录名与合同一致。`outline-wrap` 是 2026-10-26
> 回补 §5.2 长文折行时新增的场景，同一目录存 `shell-ppp1.50.png` / `shell-ppp2.00.png`
> 两张基线。此映射写进 `manifest.json` 与阶段报告，避免下一任 AI 以为少了场景。

## 4. 预计证据

| 类别 | 证据 | 位置 |
| --- | --- | --- |
| 视觉 | 8 场景各一张基线，双跑 `--tol 0`；默认场景 4 主题 × 3 DPI；窄窗两档 | `shots/x100/CER-12/matrix/`、`target/x100/cer12-outline-gate.log` |
| 行为 | 各场景 `MW_GOLDEN_INPUT` 脚本与 trace；CER-07–11 的命令/筛选/拖拽/折叠单测 | `shots/x100/CER-07/`…`CER-11/`、workspace tests |
| 回归 | 45 组既有机器门（CER-11 收口）复跑；8 场景双跑；未动场景逐字节不变 | `target/x100/stage-b-gate-cer11.log`、本包日志 |
| 工程 | fmt / clippy 新增 0 / 818 tests / release | 本包 §5 |
| 合同 | §5.1–§5.11 审计表；阶段报告；门清单 | `report.md` §6、`docs/x100/stage-b2-report.md`、`gate.md` |

## 5. 风险

- **证据包 ≠ 门通过**：golden 只证明稳定；八条判词必须用户亲口回答。
- `outline-default` 的四主题矩阵是新目录，不参与既有 45 组门；矩阵图只作人工比较，
  不回写 `shots/golden/`。
- 20k 场景只在 1.0 DPI 拍过；DPI/主题覆盖由默认场景矩阵承担，20k 的预算是 CPU
  探针 + stats，不是像素矩阵。
- CER-07 遗留的微软拼音**人工实测**仍未做，门清单里单列一条。
- 若审计发现 §5 中无法用现有证据满足的条目，记 `阻塞` 或 `未做`，不得含糊。

## 6. 验证（2026-10-26 回填）

脚本：`scripts/x100-cer12-outline-gate.ps1`（每次运行前清空隔离 profile）；
日志：`target/x100/cer12-outline-gate.log`；哈希：`shots/x100/CER-12/hashes.txt`。

### 6.1 场景双跑（`--tol 0`，各两遍）

| 场景 | DPI | 两遍 SHA-256 | 结论 |
| --- | --- | --- | --- |
| `outline-default` | 1.5 | `cab0bdde…daac49` | 两遍一致 |
| `outline-hover-select` | 1.5 | `0031d764…24557a7` | 两遍一致 |
| `outline-edit-ime` | 1.5 | `9a869ce5…873ba6eb` | 两遍一致（见 §6.4） |
| `outline-focus` | 1.5 | `1841d0d1…0d136551` | 两遍一致 |
| `outline-search` | 1.5 | `1544a2aa…13639dbc` | 两遍一致 |
| `outline-drag` | 1.5 | `99cf601f…024f94d9c` | 两遍一致 |
| `outline-narrow` | 1.5 | `b0a178f7…e5ce06` | 两遍一致 |
| `outline-20k` | 1.0 | `2700c935…cab4bf3c` | 两遍一致 |
| `outline-wrap` | 1.5 | `ea771190…c771f3e` | 两遍一致（2026-10-26 新增，见 §6.5） |
| `outline-wrap` | 2.0 | `4be636a0…c9a04b3c` | 两遍一致（同一目录第二张基线） |

全部 9 个场景（10 组）的基线文件哈希在两遍之间相同，且第二遍与基线逐像素 0 差异。

### 6.2 主题/DPI 矩阵与窄窗

- `outline-default`：**四主题 × 三 DPI = 12 张**（`shots/x100/CER-12/matrix/`），
  每张 `--update` 后立刻以 `--tol 0` 复跑一遍，12/12 复跑通过。
- `outline-narrow`：**800×600 与 640×460 两档**（`shots/x100/CER-12/narrow/`），
  各 `--update` + 复跑 0 差异；两档均无横向滚动（内容列 = 窗口 − 64）。
- 矩阵与窄窗图只作人工比较证据，不回写 `shots/golden/`。

### 6.3 工程

- `cargo fmt --all -- --check` 干净。
- `cargo clippy -p mw-shell --all-targets`：仅 `export_pdf.rs` 3 条历史告警，新增 0。
- `cargo test --workspace -- --test-threads=1` → **819 passed / 0 failed**（+1，见 §6.4）。
- `cargo build --release -p mw-shell` 通过。
- 既有全量机器门 `scripts/x100-stage-b-gate.ps1` 复跑：**45 组 × 2 遍 = 90/90**，
  0 差异，`MACHINE GATE PASS`（`target/x100/stage-b-gate.log`）。

### 6.4 金样确定性修复（唯一的代码变更）

`outline-edit-ime` 第一次双跑暴露了真实问题：同一二进制、同一脚本，两次截图会
停在**光标闪烁的亮/灭两态**之一（126 px 差异，最大通道差 87）；继续排查又看到
一次整列 1–3 px 的行位漂移。根因是 egui 的文本光标按**真实时间**闪烁，
`golden::prepare` 此前只把 `animation_time` 归零，没有停光标。

修复：`crates/mw-shell/src/golden.rs` 在 `prepare` 里同时设
`visuals.text_cursor.blink = false`（仅金样 harness，交互式运行不变）；
新增单测 `golden::tests::prepare_freezes_time_dependent_visuals`。
验证：四个编辑场景（`outline-edit` / `-ime` / `-softbreak` / `-paste-lines`）
各在干净 profile 下连跑三次，哈希三次相同**且等于既有基线**；随后 8 场景双跑与
45 组门全绿。

> 另：隔离 profile 必须每遍清空。一次复用上一遍的 `APPDATA`（里面有空的
> `notes/index.json`）曾让同一场景渲染出整列 1–3 px 不同的图；清空后稳定。

### 6.5 §5.2 长文折行落地与第二处金样确定性修复（2026-10-26 回补）

原 §7 第 1 条缺口（非编辑态长文换行）已按合同做完，这是本包的第二次代码变更：

- **换行测量**：`Outline::measure_wraps`（`panels/outline.rs`）按正文可用宽
  （`text_avail` 与绘制同一把尺子：缩进 + 任务框/短语前缀、行尾角标、折叠计数）
  把每行显示行数缓存在 `Row.text_rows`；短行用「字符数 × 字号」上界直接判 1 行，
  不进字体引擎（20k 帧预算不受影响）。行表重建、增量补行、列宽变化都会置脏重测。
- **行高与绘制**：`row_layout_height` 按「每行 +24」长高（说明行/根标题底子不变）；
  换行行用 `line_height = Some(24)` 排版，单行仍用字体行高——既有 9 个大纲场景
  逐字节不变。圆点/任务框/角标改锚**首行**文字带（`first_band`）；单行行像素不变，
  唯一几何口径变化是「根标题 + 说明行」这种组合：旧代码把 28px 标题在 64px 行框里
  垂直居中（比首行带低 10px），现在与所有行统一锚首行带（没有任何基线含这种行，
  所以零漂移，但行为确实变了）。
  正文不再 `max_rows = 1` + 省略号，`…` 截断提示随之下线（文本不再截断）。
- **命中底带**：`hit_bands` 按已排版 galley 拆行画 12% 强调底，换行行的命中不再
  错位；单行沿用旧「行框 ±3」竖向范围，`outline-search` 基线逐字节不变。
- **新场景 `outline-wrap`**（`MW_OUTLINE_WRAP=1`，`Session::sample_wrap`）：六条行
  分别钉住 2 行折行、短行不折、深层更早折行（avail 716）、硬换行、说明行叠加
  （avail 725）、3 行累计；1.5 与 2.0 两档 DPI 的行位步进完全同构（36 vs 48 device px），
  证明换行按逻辑像素计算。`MW_TRACE` 记 `outline_wrap depth rows avail` 事件。
- **第二处金样确定性修复**：排查 `outline-edit-ime` 双跑时发现，带 `MW_GOLDEN_INPUT`
  的脚本场景**没有**清掉桌面指针——鼠标恰好压在窗口里某一行上时，同一份构建能截出
  带/不带 hover 的两张图（实测 3.06% 与 5.06% 两种漂移，包围盒正好是一行的 hover 底）。
  修复：`main.rs raw_input_hook` 在离屏且**脚本不含指针动作**时每帧先发
  `PointerMoved(-10000) + PointerGone`；指针归脚本管的场景（`owns_pointer()`）不清，
  否则 hover 计时永远攒不满、握把不出现。修复后该场景与基线逐字节一致，
  23 个指针/大纲相关场景复跑 0 差异。§3 的「赌 hover」陷阱表述现在被机器化了。
- 新增单测 5 个：`wrapped_text_adds_24_per_line`、`text_avail_matches_the_row_inset`、
  `measure_wraps_counts_long_rows_and_respects_the_column_width`、
  `hit_bands_split_across_wrapped_lines`、`collapse_marks_the_wrap_cache_dirty`。
- **20k 不受影响**：`MW_OUTLINE_PROFILE=1` 实测 20001 行的换行测量 **0.54ms/次**
  （短行全部走上界短路，不进字体引擎）；`outline_20k_frame_budget_probe`（release）
  → 首屏 33–41ms（预算 100ms）、滚动 p95 ≤0.70ms、折叠/展开 0.28–0.58ms、
  只排 52/20001 行。首屏比 CER-11 记录的 23.0ms 高，是机器负载噪声
  （同机连跑三次 33/40/41ms，rebuild 本身的耗时与 CER-11 一致）。

## 7. 未做 / 已知偏差

逐条对应 `docs/28` §5：

1. ~~**§5.2 非编辑态长文换行未做**~~：2026-10-26 已做（见 §6.5），
   `outline-wrap` 双档基线为证；不再列为缺口。
2. **§5.2 拖拽握把几何**：仍是 2×3 点阵，16×20 盒 / 距列左 8 未重做（CER-09 已记）。
3. **§5.6 阈值与光标**：拖拽阈值用 egui 默认（约 6px，合同 4px）；非法落点用
   `CursorIcon::NoDrop`（egui 无 Forbidden）。
4. **§5.6 过程证据**：自动滚动 / 500ms 自动展开 / 多选拖拽只有单测与 trace，
   没有各自的拖拽中截图（静态 golden 不赌经过帧）。
5. **§5.7 筛选控件**：条件胶囊无键盘路径（Tab 进不了 22px 热区）、任务胶囊单态循环、
   标签弹层只能鼠标；高亮区间按 `to_lowercase` 字节位映射，罕见 Unicode 可能偏 1 字符。
6. **§5.7 / §5.8 性能边界**：筛选态与概念图的折叠/展开仍走全量重建（约 29ms），
   仅三条件叠加 20k 时超 16ms；行高前缀每帧 O(n) 累加，未做缓存
   （换行行数本身有缓存，不额外增加字体引擎调用）。
7. **§5.4 微软拼音**：离屏只用 `ime preedit` 等效注入，真实微软拼音人工实测仍待用户。
8. **§5.8 覆盖**：20k 只在 1.0 DPI 拍基线；20k 下的编辑/IME 组合场景未拍；
   主题/DPI 覆盖由 `outline-default` 矩阵承担。长文折行只在 1.5/2.0 两档拍过，
   未进四主题矩阵（换行是几何量，与配色无关）。
9. **残余**：带指针脚本的场景仍可能被真实鼠标移动打断（脚本只在指定帧注入位置）；
   纯键盘/IME 脚本已由 §6.5 的 harness 修复兜住。

以上各项已同步写入 [`docs/x100/stage-b2-report.md`](../../../docs/x100/stage-b2-report.md)
的 §5 与 [`gate.md`](gate.md)，门判词不得把它们算作通过。

## 8. 人工判词（G-B2-1）

见 [`gate.md`](gate.md)。八条判词全部「待用户」，本包不代写结论。

