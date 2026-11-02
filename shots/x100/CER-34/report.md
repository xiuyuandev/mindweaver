# CER-34 · 全量回归、重拍与 G-B2-4

- **日期**：2026-11-02
- **批次**：B2-D（`docs/29` §9）
- **依赖**：CER-33（证据包已完成）
- **状态**：**已实现** —— 全量回归、基线重拍、双跑 `--tol 0` 全部完成；
  G-B2-4 八判词按用户授权代判（原话见 §8），判词记录在 [gate.md](gate.md)。

---

## 1. 范围

`docs/29` §9 CER-34 的交付逐项落点：

| 合同项 | 本包做法 | 证据 |
| --- | --- | --- |
| 全量回归 | 正式脚本 `scripts/x100-stage-b-gate.ps1`：X11–X19 证据包、PS 语法、fmt、mw-shell clippy、串行全量测试、release 构建、173 场 `--tol 0` 两遍 | [stage-b-gate-2026-11-02-final.log](stage-b-gate-2026-11-02-final.log) |
| 受影响基线重拍并记录旧哈希/漂移 | `card-*` 36 场 + `cm-card-link` 因 F1/F2 有意重拍；`stage-cm` 3 场（2 张 CM-18 欠账 + 新 `cm-group`）；逐场旧/新哈希与漂移见 §5 | [hashes-final-2026-11-02.tsv](hashes-final-2026-11-02.tsv) |
| 双跑 `--tol 0` | stage-b 173 场 / 346 次比较、stage-cm 73 场 / 146 次比较，两遍逐像素零差异 | 两份门日志 |
| tests / fmt / clippy / release | 门内四步全绿；clippy 新增 0 | 门日志 |
| `docs/x100/stage-b2-report.md` | §13/§14 回填为最终结论 | 该文件 |
| `manifest.json` | 场景、矩阵、性能、构建号、哈希 | 本目录 |
| 文档回填 | `docs/STATUS.md` §1/§3.3、`docs/24`、`AGENTS.md`、`HANDOFF.md`、执行日志 | 见 §7 |
| 门清单 | [gate.md](gate.md)，八判词已按授权代判并逐条落依据 | 该文件 |
| 必要时复核 X20 外壳 | X11–X19 证据检查 PASS；X20 人工复核计入 G-B2-4 判词 1/7 | 门日志 |

## 2. 非目标

- **不新增功能**、不改数据模型；D 批只做一致性、联动、验收、修缺陷。
- **F1/F6 已由用户授权代为拍板**（2026-11-02，原话「继续开发，而且呢，你帮助我验收，
  不需要我决定。」）。决定与落地见 CER-31 §4 F1/F2/F4、CER-32 §5 F6：
  - F1 选 ①：卡片页正文向合同 640 对齐，不再把合同改成 720；
  - F2：卡片页配色改走 `content_metrics`，与预览/大纲同一条派生链；
  - F4：卡片页版心改走 `content::CARD.column`，窄窗左右 24；
  - F6 选 ②：库是归档状态唯一真相源，文档同步时保留库里的 `archived`。
- 不顺手重拍无关场景：36 场 `card-*` 与 `cm-card-link` 的重拍全部由 F1/F2 的整页换底引起，
  `cm-multi-menu` / `cm-submenu-bottom` 由 CM-18 已拍板的分组菜单引起。
- 不在 G-B2-4 通过前宣布 B2 完成或开始 Stage C。

## 3. 变更

产品代码（B2-D 的修复项，均为 CER-31/32 审计结论）：

- `crates/mw-shell/src/note_view.rs`：
  - `show()` 新增 `theme: &mw_core::Theme` 参数，内部取 `content::content_metrics(theme, ppp).colors`；
  - 卡片页整页底色从壳层 `Chrome::panel`（亮色纯白）改为 `surface.page`（雪主题 `#F4F1EB`）；
  - `MdPalette` 删除 `from_chrome`，改走 `from_content`（正文/次要/占位/强调/代码底/页面底同源，
    代码块底 = 正文色 6%，与笔记库预览一致）；
  - 版心从自写 `total >= 1040 ? 720 : clamp(total-96, 320, 640)` 改为 `content::CARD.column(total)`
    （宽窗 720 / 正文 640 / inset 40；窄窗左右 24）；标题与正文 `TextEdit` 的颜色改取 `palette.text`。
- `crates/mw-shell/src/main.rs`：卡片阅读页调用点按当前画布主题取 `Theme` 传入 `show()`。
- `crates/mw-shell/src/note_store.rs`：`sync_document` 用文档份覆盖库之前先取库里已有的
  `archived` 并写回落盘对象（库里没有该卡时才用文档份的值）；新增单测
  `syncing_a_newer_document_card_keeps_the_library_archived_flag`。
- `crates/mw-core/src/design.rs`：新增单测
  `card_column_keeps_the_contract_text_width_at_every_breakpoint`（700 / 900 / 1200 / 500 四点）。

工具与格式：

- `scripts/golden-sweep.sh`：启动场景的 `env` 改绝对路径 `/usr/bin/env`。本机
  `~/.local/bin/env` 有一个 shim（exit 0、不执行任何命令），裸 `env` 会让 173 场全部假红
  （`PASS=0 FAIL=173`、错误详情为空）。这是本轮实测到的环境坑，已在脚本里注释。
- `cargo fmt --all`：把并行会话留下的未格式化文件与本次改动一起归位，无语义变化。

基线：

- `stage-b` 重拍 **36 场**（全部 `card-*` + `cm-card-link`，整页换底/正文变窄）；
- `stage-cm` 重拍 **3 场**（`cm-multi-menu` / `cm-submenu-bottom` 按 CM-18 最终形态；
  `cm-group` 首次生成基线）；门内 73 场两遍全过；
- CER-33 卡片矩阵 **12 张**（4 主题 × 3 DPI）按最终树重拍，`hashes.txt` 与接触表同步更新。

## 4. 验证

| 项 | 结果 | 证据 |
| --- | --- | --- |
| X11–X19 证据检查 | PASS | 门日志 |
| PowerShell 语法 | PASS | 门日志 |
| `cargo fmt --all -- --check` | PASS（门内与门后各一次） | 门日志 |
| mw-shell clippy 新增 | 0 | 门日志 |
| 全量测试 | **1279 passed / 0 failed / 1 ignored** | 门日志 |
| release 构建 | PASS；25,737,728 B，SHA256 `3811fb3011a23652df8cb2d9fb31b5ef172531be663656d373c381573cf61fa6` | 门日志 + 本报告 |
| golden 第 1 遍 | **173 / 173** 零差异 | 门日志 |
| golden 第 2 遍 | **173 / 173** 零差异 | 门日志 |
| stage-cm 门 | **73 / 73** 两遍（146 次比较）零差异 | [stage-cm-gate-2026-11-02.log](stage-cm-gate-2026-11-02.log) |
| 卡片宽度实测 | `card-long` 正文暗像素跨度 960 物理 px = **640 css px**（ppp 1.5）；`card-narrow` 同 | 像素统计脚本，CER-31 §5 |
| 配色实测 | 卡片页底色 `#F4F1EB`、正文 `#22262A`，与 `content_metrics` 一致 | 同左 |

定向测试（改动后、门之前）：

```text
cargo test -p mw-shell --bin mw-shell -- note_view   → 80 passed / 0 failed
cargo test -p mw-shell --bin mw-shell -- note_store  → 16 passed / 0 failed
cargo test -p mw-core  --lib design                  → 19 passed / 0 failed
cargo test -p mw-shell --bin mw-shell -- canvas      → 147 passed / 0 failed
```

## 5. 旧/新哈希表

完整 173 场见 [hashes-final-2026-11-02.tsv](hashes-final-2026-11-02.tsv)（列：场景 / 目录 / ppp /
旧 SHA256 / 新 SHA256 / 是否重拍 / 漂移）。摘要：

| 组 | 场次 | 旧 → 新 | 漂移 | 原因 |
| --- | --- | --- | --- | --- |
| `card-*`（含 `card-ime-*` / `card-20k` / `card-code`） | 35 | 全部重拍 | **92.585%–99.995%** | 整页底色白 → 纸色；正文 720 → 640 |
| `cm-card-link` | 1 | 重拍 | **99.965%** | 同一卡片页作为概念图来源跳转的落地页 |
| `cm-multi-menu` | 1 | 重拍 | 2.913%（CM-18 已拍板） | 概念多选菜单首行「创建分组」 |
| `cm-submenu-bottom` | 1 | 重拍 | 16.288%（CM-18 已拍板） | 菜单变高后子菜单被下沿夹取 |
| `cm-group` | 1 | **首次生成** | —（无旧基线） | CM-18 概念分组 |
| 其余 134 场 | 134 | 未重拍（旧 = 新） | 0 px | — |

旧哈希取自重拍前的基线文件；`cm-multi-menu` / `cm-submenu-bottom` 的旧哈希取 `HEAD` 中的
版本（`f216c63a…` / `706de39d…`），新哈希为 `c2be1dca…` / `596f4258…`；`cm-group` 新哈希
`f385caaa…`。逐场漂移数字来自 `--tol 0` 的首次比对（`target/tmp/c34-drift-before-update.log`）。

## 6. 已知缺口 / 风险

1. **实机微软拼音未核对**（B2-A 第 8 条起的一笔欠账）：本轮所有中文证据仍是离屏等效或构造事件；
   因此 CER-22–34 即使门过也只记 `已实现`，不记 `已验证`。
2. **系统 DPI 缩放切换未测**：CER-33 矩阵只覆盖 harness `--ppp` 1.0/1.5/2.0。
3. **5 分钟任务没有实机录屏**：判词 6 按合同路径 + 单测 + 基线证据判，实机操作仍需用户复看。
4. **`card-jump-collapsed` 专项金样未拍**：F8 的折叠/聚焦揭示路径由 M20/M22 的 Document 级测试
   与 `cm-card-link-back` 兜底，没有「来源在折叠子树」的实拍。
5. **卡片页图片仍不渲染**（B2-B 已知缺口）：`card-code` 里的图片语法只显示文字。
6. **`docs/28` §4.2 与实现现已一致**（正文 640），但 `docs/28` 之外的旧报告里仍可能引用
   「卡片 720」的历史数字——以本轮 CER-31/CER-34 为准。
7. 离屏 harness 无 vsync：性能只按 UI p95 判，不看墙钟帧率（CER-33 §4.2 口径）。
8. **概念图剩余项**（不属 B2-D）：嵌套概念、命名样式、`.cxl` 互导仍待拍板/未做（CM-15）。

## 7. 回填清单

- [x] `shots/x100/CER-34/manifest.json`（场景、哈希、门槛、构建号）
- [x] `shots/x100/CER-34/gate.md`（G-B2-4 八判词 + 判词记录）
- [x] `docs/x100/stage-b2-report.md` B2-D 章节与 G-B2-4 结论
- [x] `docs/STATUS.md` §1/§2/§3.3/§5/§6/§9
- [x] `docs/24` B2-D 行、`docs/00` 如有需要
- [x] `AGENTS.md` / `HANDOFF.md` 顶部断点
- [x] `docs/x100/log/execution-log-2026-11.md` 追加本轮

## 8. 人工判词

用户 2026-11-02 原话：**「继续开发，而且呢，你帮助我验收，不需要我决定。」**
据此按 G-B2-1 / G-B2-3 的先例代判，逐条依据见 [gate.md](gate.md)「判词记录」。
**门结论：G-B2-4 通过（2026-11-02 授权代判，8 条）**；但 CER-22–34 仍记 `已实现`，
不记 `已验证`（实机微软拼音与系统 DPI 两项欠账，与 G-B2-1/G-B2-3 同一口径）。
