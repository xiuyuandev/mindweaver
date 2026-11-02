# CER-11 · 大纲虚拟化与 20k

> 状态：`已实现`（证据齐；非门包，待 G-B2-1 人工复核）· 批次 B2-A · 依赖：CER-05–CER-10
> 设计合同：[`docs/28`](../../../docs/28-内容体验重构规范.md) §5.8（行虚拟化、20k 预算、窄窗、DPI）
> 执行合同：[`docs/29`](../../../docs/29-内容体验重构工作包.md) §6（X70 范围并入）

## 1. 范围

全屏大纲 20k 行：只排视口 ±10 行、折叠/展开增量更新、远距离跳转不依赖滚轮；
给出首屏/滚动/折叠预算的 release 证据、三 DPI × 四主题矩阵，并保证既有场景逐字节不变。

## 2. 审计（2026-10-26，release 实测）

开工前 `Outline::show` 在 `ScrollArea::show` 里对**每一行**做 `allocate_space` + `ui.interact`
+ `focus.note`；`is_rect_visible` 只省了绘制，没省布局与焦点登记。20k 实测单帧
**p50 1090.93ms / p95 1119.19ms**。定位到两处：

1. `FocusMemory::note` 用 `Vec<FocusKey>::contains` 去重 —— 每帧 2 万行时 O(n²)
   （400M 次含 String 比较），单这一项就占约 1.05s。
2. 全表布局/交互本身：修掉 (1) 后仍 **p50 14.78ms / p95 16.56ms**，已贴 16ms 预算。

`rebuild` 分相位（`MW_OUTLINE_PROFILE=1`）：stats 0.6ms、`resolve_all` 13.0ms、
行构造 15.9ms，合计 **29.5ms** —— 单次折叠若走全量重建必然超 16ms。

## 3. 实现

1. **焦点登记 O(1)**：`FocusMemory::Entry` 增加 `seen: HashSet<FocusKey>`，
   `begin_frame` 同时清空 `known`/`seen`，`note` 改为 `seen.insert` 命中才 push。
2. **行虚拟化**（`panels/outline.rs`）：
   - `ScrollArea::show_viewport` 取内容坐标视口；用 `row_layout_height(row, row_h, edit_lines)`
     逐行累加，算出 `first..last`，**上下各留 10 行 overscan**。
   - 上方 `ui.add_space(pre)`、下方 `ui.add_space(total - y)` 把整表高度补齐，
     滚动条范围不变；只有 `first..last` 的行会 `allocate_space`/`interact`/`note`/绘制。
   - 目标行在视口外时（搜索跳转、`reveal`）改用 `ScrollArea::vertical_scroll_offset`
     直接定位（内容闭包里排队 `scroll_with_delta` 这一帧取不到，实测视口不动）；
     进视口后仍由 `scroll_to_me` 居中。
3. **折叠/展开增量**：
   - `Outline.styles` 缓存 `resolve_all` 的结果；`collect_rows()` 抽成 `rebuild` 与
     「补行」共用的展平函数。
   - `Outline::apply_collapse(doc, theme, id, collapsed)`：折叠删 `depth >` 的行段；
     展开用缓存样式补回子树行并 `splice`；随后重建 `visible`、`self.rev = doc.revision`
     让本帧 `sync` 跳过全量重建。`OA::Toggle` 处理器调用它，trace `outline_collapse_incremental`。
   - 筛选态（`rows_all`）与概念图退回全量重建（见 §7）。
4. `MW_OUTLINE_20K=1`（4000 分支 × 4 叶 = 20001 行）、`MW_OUTLINE_SCROLL_TO=文字`
   两个实拍钩子；`MW_OUTLINE_PROFILE=1` 保留 rebuild 分相位诊断。

## 4. 非目标

- 不改 `.zmind`/`Node`；不为 20k 改数据模型。
- 不做行高缓存前缀表（每帧 O(n) 累加约 0.1ms，低于缓存失效的复杂度收益）。
- 不做筛选态/概念图的增量折叠（见 §7）。

## 5. 验证

### 5.1 release 探针（`cargo test --release … outline_20k_frame -- --ignored`）

`shots/x100/CER-11/perf.txt`：

```
outline 20k: first_screen=23.0ms scroll p50=0.11ms p95=0.13ms max=0.38ms
collapse=0.17ms expand=0.17ms laid_rows=52
```

| 预算（docs/28 §5.8） | 实测 | 结论 |
| --- | --- | --- |
| 首屏 ≤100ms | 行表重建 + 可见表 **23.0ms** | PASS |
| 滚动 ≥55fps（单帧 ≤16ms） | UI **p95 0.13ms**、max 0.38ms | PASS |
| 单次折叠/展开 ≤16ms | 折叠 **0.17ms** / 展开 **0.17ms** | PASS |
| 只排视口 ±10 行 | **52 行**（视口约 31 行 + 上下各 10 + 1） | PASS |

探针断言跑在常规测试里（`#[ignore]`，不拖慢全量），任何一项超预算会直接失败。

### 5.2 端到端 stats（golden 模式下滚动 200 帧）

`shots/x100/CER-11/stats.log`（`MW_STATS=1`，20k，1280×800）：

```
frame  fps   wall_p50  wall_p95  ui_p50  ui_p95  visible  total
60     56.2  16.40     25.82     0.63    0.95    33       20001
120    58.1  16.45     22.55     0.68    1.10    33       20001
180    58.7  16.44     21.59     0.72    1.81    33       20001
```

- UI（布局+绘制命令生成）p95 **≤1.81ms**；wall 帧率 **56.2–58.7fps**（离屏无 vsync，
  wall 受读回/调度影响，取“≥55”作下限证据；正式桌面以 UI 预算为准）。
- `visible=33 / 20001`：端到端确认只处理视口内的行。

### 5.3 视觉

| 场景 | 结果 |
| --- | --- |
| `outline-20k`（新增） | `2700c935…`，顶部 20k 行 |
| `outline-20k-deep`（新增） | `50f2152f…`，`MW_OUTLINE_SCROLL_TO=分支 0150`（全表第 751 行） |
| 三 DPI × 四主题 | `shots/x100/CER-11/matrix/{snow,ink,mist,sand}-{1.0,1.5,2.0}/` 共 12 张 + `hashes.txt` |
| 未受影响 | `outline` / `outline-focus` / `outline-compact` / `outline-narrow` / `outline-edit` / `outline-search` / `outline-search-labels` / `outline-hover-select` / `outline-drag` 逐字节 0 差异 |

两个 20k 场景各连跑两次 `--tol 0` 0 差异；已加入 `scripts/x100-stage-b-gate.ps1`
（43 → 45 组比较）。

### 5.4 工程

- `cargo fmt --all -- --check` 干净。
- `cargo test --workspace -- --test-threads=1` → **818 passed / 0 failed**（+1 增量折叠一致性）。
- `cargo clippy -p mw-shell --all-targets` → 仅 `export_pdf.rs` 3 条历史，新增 0。
- `cargo build --release -p mw-shell` 通过。
- 机器门：见 §5.5（回填）。

### 5.5 机器门

`powershell -File scripts/x100-stage-b-gate.ps1 -Log target/x100/stage-b-gate-cer11.log`：

- golden matrix **45 组 × 2 遍 = 90/90 通过，0 差异（`--tol 0`）**，含新增 `outline-20k`、
  `outline-20k-deep`。
- `tests passed=818 failed=0 exit=0`。
- `MACHINE GATE PASS`；G-B2-1 人工门仍未开（B2-A 收口见 CER-12）。

## 6. 变更

- `crates/mw-shell/src/panels/outline.rs`：`show_viewport` 虚拟化、`row_layout_height`、
  `collect_rows` 展平、`apply_collapse` 增量、`styles` 缓存、`MW_OUTLINE_PROFILE` 诊断、
  探针测试 `outline_20k_frame_budget_probe`（ignored）。
- `crates/mw-shell/src/panels/focus.rs`：`seen: HashSet` 去重（O(n²) → O(n)）。
- `crates/mw-shell/src/session.rs`：`sample_20k()` 性能夹具。
- `crates/mw-shell/src/app_methods_1.rs`：`MW_OUTLINE_20K` / `MW_OUTLINE_SCROLL_TO` 钩子。
- `crates/mw-shell/src/app_methods_2.rs`：`OA::Toggle` 走增量折叠。
- `scripts/x100-outline-20k.ps1`（新）：stats + 12 张矩阵。
- `shots/golden/inputs/outline-20k-scroll.txt`（stats 用滚动脚本，不作为 golden）。

## 7. 未做 / 遗留

- **筛选态与概念图的折叠/展开仍走全量重建**（约 29ms），超过 16ms 预算；只在
  「20k + 筛选/概念图 + 折叠」三者叠加时出现。修法需要把筛选态行表也做子树增量，
  留到 CER-12 或 Stage C 前处理，本包如实记录。
- 行高前缀每帧 O(n) 累加（20k 约 0.1ms），未做前缀缓存。
- 探针只测 UI 构建 CPU；GPU 上屏时间由 stats 的 wall 帧率兜底（离屏无 vsync，需注意口径）。
- 未做 20k 下的编辑/IME 场景（属 CER-07/CER-12 的组合矩阵）。

## 8. 风险

- `apply_collapse` 与全量重建的一致性由单测逐字段（id/depth/index/parent/last/guide_mask）
  钉住；若将来 `Row` 加字段，必须同步更新该断言。
- 折叠展开展开子树时用缓存样式；若某条命令只改样式不改 revision 会读到旧缓存——
  当前所有样式命令都会推进 revision 并触发全量重建。
- 20k 夹具用 2 万条 `Command::AddNode` 构建（release ~0.5s），只用于实拍；
  真实文件打开路径仍走 `.zmind` 反序列化（`scripts/perf.ps1` 另行覆盖冷启动）。

## 9. 人工判词

非门包，不填；G-B2-1 一并复核。
