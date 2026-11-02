# CER-01 · 内容表面令牌与度量

> 状态：已实现 · 开始 2026-10-25 · 批次 B2-A · 依赖：无
> 设计合同：[`docs/28`](../../../docs/28-内容体验重构规范.md) §4（§4.1–§4.6）
> 执行合同：[`docs/29`](../../../docs/29-内容体验重构工作包.md) §6

## 1. 范围

把三个内容表面（全屏大纲、卡片笔记页、笔记库）的共同语言收敛成 **唯一一份**
代码令牌，后续 CER-02 起的组件与页面只能从这里取值。

本包交付：

1. `mw_core::design::content`：
   - 字阶：页面标题 30/40/600、大纲页标题 28/38/600、H2 22/32/600、
     H3 18/28/600、H4 16/26/600、大纲行 16/26/400、阅读正文 16/28/400、
     列表标题 14.5/22/500、列表摘要 12.5/19/400、列表元信息 11.5/16/400、
     行内说明/导航 13/20/400、辅助信息 12/18/400、小标签 11.5/16/500、
     代码 13.5/22/400。
   - 版心：大纲 800/760、卡片页 720/640、笔记库预览 680/600；页面顶/底留白
     32/64、48/96、40/64；窄窗左右 32（大纲/预览）、24（卡片页）。
   - 状态透明度：hover 4%/6%、selected 7%/12%、navSelected 4%/8%、
     editing 3%/8%、hairline 8%/12%、divider 6%/10%（亮/暗）。
   - 圆角：行/标签 6、代码块/斜杠菜单/图片 10、弹窗 18。
   - 动效：hover 120ms、select 140ms、浮层 160ms、聚焦滚动 180ms；
     滚动条淡入 120ms、空闲 300ms 淡出。
   - 滚动条：6px 宽、4px 拇指、圆角 8。
   - 语义色推导：`surface.page`、`surface.panel`、`text.primary`、
     `text.secondary`（主色 70%）、`text.faint`（主色 48%，仅占位/禁用）、
     `accent`、`link`（亮色下自动压到 ≥4.5:1）、状态填充、hairline/divider、
     6 颗标签点。
   - 纯函数 `content_metrics(theme, dpi) -> ContentMetrics`：返回 1 物理像素的
     hairline（以 point 表示）、焦点环、滚动条与语义色；不读全局、不写文件。
2. 测试：字阶单调、行高比例、字重白名单、版心上限与窄窗行为、状态透明度、
   四主题对比度 ≥4.5、link 对比度、焦点色 ≥3:1、DPI 非法值回退。
3. 令牌 dump：`shots/x100/CER-01/tokens.txt`（四主题 × 三档 DPI 数值表）。
4. 文档校准：`docs/28` §4.3 明确 `text.faint` 只用于占位/禁用、12px 元信息用
   `text.secondary`；新增 `link` 令牌的对比度约束。`docs/16` §3.6 的页面级
   48/64/96 已在规划轮写入，本包只核对不重复改。

## 2. 非目标

- 不接线任何界面：不改大纲、卡片页、笔记库、开始页、画布。
- 不新建 `ContentPage` / `PageTitle` 等组件（CER-02）。
- 不改 `chrome::Type`、`Tone` 或现有外壳令牌；不删除旧常量。
- 不重拍任何 golden；本包预期 0 视觉差异。
- 不改 `.zmind`、`Document`、`Node`、`NoteCard` 或命令层。
- 不新增依赖、crate、线程、构建工具。
- 不实现 tag 颜色分配策略、不做主题编辑器的内容表面预览。

## 3. 验收场景

| 场景 | 输入 | 期望 |
| --- | --- | --- |
| `type_scale` | 14 个角色 | 字号单调不减；行高在 1.30–1.80；字重仅 400/500/600 |
| `measure` | 大纲/卡片/预览 | `page_max > text_max`；顶/底留白与 28 §4.2 一致 |
| `column_wide` | 大纲 1000、卡片 900、预览 900 | 版心不超过上限，水平居中，正文不超过上限 |
| `column_narrow` | 大纲 700、卡片 700、预览 700 | 左右留白 32/24/32，正文不溢出，inset 不为负 |
| `state_alpha` | 亮/暗 | 数值与 28 §4.3 一致；暗色不低于亮色；均在 0–0.2 |
| `contrast` | snow/ink/mist/sand | 正文、次要文字、链接对 page/panel ≥4.5；强调色对 page ≥3 |
| `selected_text` | 四主题 | 选中/悬停填充上的正文仍 ≥4.5 |
| `metrics_dpi` | 1.0/1.25/1.5/2.0、0、NaN | hairline 恒为 1 物理像素；非法 DPI 回退 1.0 |
| `tokens_dump` | 四主题 × 三 DPI | 生成稳定文本表，供证据包归档 |

## 4. 旧基线

本包不改绘制路径，**受影响 golden 为空**。开工前基线以 `HANDOFF.md` 为准：

- 31 组比较 / 27 个目录，连跑两次 `--tol 0` 均 0 差异。
- 测试 765 passed / 0 failed；fmt 干净；`mw-shell` clippy 3 条历史告警。
- 本包完成后要求：既有 31 组仍 0 差异；测试数增加但失败 0；新增 clippy 0。

## 5. 变更

| 文件 | 变更 | 动外观/数据/格式 |
| --- | --- | --- |
| `crates/mw-core/src/design.rs` | 新增 `content` 模块：14 档 `TypeStyle` 字阶、`Measure`×3 版心（OUTLINE/CARD/PREVIEW）、`ContentAlphas` 亮/暗两套、`ContentColors` 语义色推导（`page = ThemeUi.bg`、`link` 亮色自动压暗到 ≥4.5:1、`tag_dots` 取主题分支色）、`composite` 非预乘合成、`focus`/`radius`/`motion`/`scrollbar` 令牌、`ContentMetrics` 与纯函数 `content_metrics(theme, dpi)`；新增 5 个测试函数 | 否 |
| `crates/mw-core/examples/content_tokens.rs` | 新增证据 dump 示例：字阶/版心/版心实算/四主题语义色/状态透明度/DPI 与滚动条/标签点七张表 | 否 |
| `shots/x100/CER-01/report.md`、`tokens.txt`、`manifest.json` | 本证据包 | 否 |
| `docs/28` §4.3、§11 | 校准：`text.faint` 仅占位/禁用、12px 元信息改用 `text.secondary`、新增 `link` 令牌与 ≥4.5:1 约束、明确 `surface.page` 取 `ThemeUi.bg` 不随 `canvas_bg`（前序会话随本包完成） | 否（文档） |
| `docs/16` §3.6 | 页面级留白 `48/64/96` 允许项（规划轮已写入；本包核对无误，未重复改） | 否（文档） |

未接线任何界面：`design::content` 在 `mw-shell` / `mw-render` 中零引用（grep 验证），
绘制路径不变，golden 基线预期逐字节不变。未新增依赖、crate；未改 `.zmind`、
`Document`、`Node`、`NoteCard` 或命令层。

## 6. 验证

**行为证据**

- `cargo test --workspace -- --test-threads=1` → **770 passed / 0 failed**
  （开工基线 765 + 本包新增 5：`content_type_scale_is_monotonic_and_uses_three_weights`、
  `content_measures_centre_a_bounded_column`、`content_alphas_match_the_contract`、
  `content_colors_keep_text_readable_on_all_themes`、`content_metrics_snap_hairline_and_ignore_bad_dpi`）。
  覆盖 §3 全部 9 个验收场景：字阶单调/行高比例/字重白名单、版心上限与宽窄窗实算、
  状态透明度亮暗对照、四主题正文/次要/链接对比度 ≥4.5、强调色 ≥3:1、选中底上文字可读、
  hairline=1 物理像素、非法 DPI 回退、纯函数幂等。

**视觉证据**

- 本包不改绘制路径，受影响 golden 为空（§4 旧基线声明成立）。
- 令牌数值表 `tokens.txt` 已归档；重新生成
  `cargo run -p mw-core --example content_tokens` 与归档副本**逐字节一致**（确定性验证）。
- 四主题数值表关键结论：snow/ink/mist/sand 正文对 page 均 ≥11.6:1、次要文字
  ≥4.89:1、链接 ≥4.61:1（mist/sand 强调色直接作链接不达标，`link_color` 已压暗生效）。

**回归证据**

- 31 组 golden / 27 个目录双跑 `--tol 0` → 全 0 差异（机器门重跑日志
  `target/x100/cer-01-gate-rerun.log`；上一次运行 pass 2 已 PASS，pass 1 因
  `mw-shell.exe` 被占用 os error 5 环境性失败，与代码无关）。
- 未跑旧文件往返/撤销重做：本包未触碰文档模型与命令层，无可回归对象。

**工程证据**

- `cargo fmt --all -- --check` → 干净。
- `cargo clippy -p mw-shell --all-targets` → 新增 0（mw-shell 仅 `export_pdf.rs`
  3 条历史告警；`mw-core`/`mw-render`/`mw-io` 依赖 crate 在冷缓存重 lint 时会显示
  若干既有告警，均不在本包触碰的 `design.rs`，门脚本口径下不出现）。
- release 构建：机器门内通过。
- `scripts/x100-stage-b-gate.ps1` 全量重跑 → PASS（证据 manifest、fmt、release、
  golden 双跑、clippy、全量串行测试）。

## 7. 未做 / 已知偏差

- `design::content` 目前**没有任何 UI 消费者**：按计划属 CER-02 起的接线工作，
  本包只交付令牌与度量。
- `tag_dots` 直接取 `ThemeUi.palette` 前六支，未单独做「低饱和/暗色提亮」调校；
  亮/暗主题各自的 palette 已按主题校过色，但 6px 圆点的最终观感待 CER-24 接线时人工复核。
- 令牌 dump 为手动运行 example 生成，未挂进门脚本；CER-02 的 `content-kit` golden
  会成为令牌漂移的常驻回归。
- 滚动条淡入/淡出（120ms/300ms）只有常量与度量结构，行为待 CER-02 组件实现。

## 8. 风险

- `link` 是 28 §4.3 未列出的推导令牌，用于满足链接文字 ≥4.5:1；必须在
  CER-02 起统一使用，禁止页面各自把 accent 当链接色。
- 令牌与未来组件的唯一真相源是 `design::content`；若 CER-02 出现第二份
  字号/内距常量，按回归处理。
- 本包只做数值与推导，审美仍需 G-B2-1 人工判词。

## 9. 人工判词

非门包，不填。
