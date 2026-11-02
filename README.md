# 织思 · MindWeaver

> 仅面向 Windows 的**纯 Rust 原生**思维导图工具。免费开源、本地优先、中文优先。
> 不使用 WebView / HTML / CSS / JS / Electron / Tauri —— 从数据模型到界面与排版，整条栈都是 Rust。

*界面截图与验收证据不随仓库分发（体积考虑）；本地 `cargo run -p mw-shell --release` 可查看实机界面。*

## 为什么做这个

中文用户常用的思维导图工具里，同时做到「大文档不卡」「中文输入不打架」「数据完全在本地」「界面不玩具」的很少。织思把四件事当作硬指标：

- **快**：Rust 数据核心 + wgpu 自绘画布 + DirectWrite 排版；5,000 节点流畅缩放平移，20,000 节点可打开。
- **稳**：3 秒自动恢复点、原子保存、滚动备份；文件格式是开放的 ZIP + JSON（`.zmind`）。
- **美**：4 套精修内置主题（含一套真调过的深色）、悬停工具条、克制动效、Windows 11 原生窗口质感。
- **中文好**：原生 IME 行内编辑、CJK 禁则换行、中英混排、全拼/首字母/多音字检索。

## 当前状态（2026-11-02，X100 Stage C）

> **诚实状态**：核心闭环可用。X20 机器门通过、**人工视觉门未通过**（用户判定全屏大纲、
> 卡片笔记页、笔记库三个内容界面不合格），因此插入了 **Stage B2 内容体验重构（CER-01–CER-34）**；
> 2026-11-02 B2 收口：四道 B2 人工门与平行轨的 G-B3-1、G-B3-2、G-CM-1、G-CM-2 均按用户授权代判通过
> （授权原话：「继续开发，而且呢，你帮助我验收，不需要我决定。」），**Stage C（X21+）已解锁**，
> 当前在 X21 / X21b。
> 设计合同见 [docs/28](docs/28-内容体验重构规范.md) / [docs/30](docs/30-外壳体验与概念图设计规范.md)，
> 执行合同见 [docs/29](docs/29-内容体验重构工作包.md) / [docs/31](docs/31-外壳体验与概念图工作包.md)。
> **门过 ≠ `已验证`**：实机微软拼音与系统 DPI 两项人工签字仍欠。
> **当前做到哪、门绿没绿、下一步是什么：只看 [docs/STATUS.md](docs/STATUS.md)。**
> 历史终验只作证据，不再决定下一步。
>
> 测试数、门场次、golden 基线与红灯、Clippy 基线等动态数字以 **[docs/STATUS.md](docs/STATUS.md)** 为准；
> 本文只保留定性结论，不复制会过期的数字。

| 项 | 状态 |
| --- | --- |
| 版本 | **v0.1.0 开发版 · X100 Stage C（X21/X21b）进行中**；阶段、门与断点只看 [docs/STATUS.md](docs/STATUS.md) §1–§5，本文不复制会过期的状态明细 |
| 测试 | 串行全量 **见 [docs/STATUS.md](docs/STATUS.md) §1**（本文不复制会过期的数字）。**裸跑前先把 `TEMP`/`TMP` 指到 `target/tmp`**，否则写盘用例会因 `os error 5` 假红 |
| 视觉基线 | X20 完成 25 场景外壳矩阵（四主题 × 四档 DPI × 五种窗口 + 开始页/菜单/弹窗/标签/大纲/检查器）；**只证明外壳稳定，不证明内容界面好看**。截图证据不随仓库分发 |
| Clippy | `cargo fmt` 干净；`cargo clippy --workspace --all-targets` 新增 0（与 `scripts/clippy-baseline.txt` 指纹比对） |
| CI | `scripts/x100-stage-b-gate.ps1`（Stage B/B2）与 `scripts/x100-stage-cm-gate.ps1`（概念图）；两道门必须顺序跑（并发会互踩 `actual/` `diff/` 目录），场次与结论只看 [docs/STATUS.md](docs/STATUS.md) §1 |
| 构建 | 最近一次 release `target/release/mw-shell.exe` 见 `HANDOFF.md`；换机后运行 `cargo build -p mw-shell --release` |
| 发布包 | 发布 zip 不随仓库分发；打包/安装/签名说明见 `docs/archive/20-签名与发布.md` |
| 性能门 | 预算口径见 [docs/08-性能与兼容性要求.md](docs/08-性能与兼容性要求.md)，实测数字只看 [docs/STATUS.md](docs/STATUS.md) §1；`scripts/perf.ps1` 是预算门（本文不复制实测值） |
| 仓库体积 | 入库约 **10.4MB / 876 个文件**（不含 `target/`、golden 基线图与截图证据，规则见 `.gitignore`） |
| Git | `master` 单提交；仓库只含源码、文档、脚本与测试 |

**已经能做的事**：新建/打开/保存/另存为、多标签、自动保存与崩溃恢复；五种布局（思维导图 / 向右逻辑 / 组织结构图 / 鱼骨图 / 时间轴）；节点增删改、拖拽改父、Ctrl+拖拽复制、拖到空白分离自由主题、多选、折叠、撤销重做、剪贴板；**全屏大纲（F8）**、检查器、搜索、命令面板、右键菜单、演示模式；**四套主题** + 自定义强调色 + 逐节点样式；**单表面所见即所得卡片笔记**（无阅读/编辑模式切换）+ **独立卡片库**（三栏、标签体系、批量管理、反链、全文搜索）；主动回忆、知识视图、学习统计、合集导出；图片附件与粘贴/拖放；恢复中心、文件锁与冲突提示、设置中心；新建子节点后视图自动跟随；导入 Markdown / OPML / FreeMind / XMind / 缩进文本；导出 PNG / SVG / PDF / Markdown / 大纲文本 / OPML / FreeMind。

**明确还没做好**：五款输入法的人工签字；部分极端 DPI / 多屏组合的人工验证；PDF 文字是矢量轮廓（不可选/不可搜索，节点图片未嵌入）；卡片图片尚未进入卡片笔记；安装器代码签名（钩子与文档已就绪，受信任证书未采购）。

## 快速开始

**前置**：Windows 10 1809+ / Windows 11，x64；Rust（MSVC 工具链）+ Visual Studio Build Tools + Windows SDK。
**不需要** Node.js，也**不需要**安装 XMind。

```bash
# 运行（release；debug 下 DirectWrite 与合成器慢到测不出手感）
cargo run -p mw-shell --release        # 等价于 cargo shell

# 测试与质量门槛
cargo test --workspace -- --test-threads=1 # 串行全量；先设 TEMP/TMP=target/tmp（见 HANDOFF §3.1）
powershell -File scripts\ci.ps1        # fmt + clippy 基线 + test + 全部 golden
powershell -File scripts\x100-stage-b-gate.ps1 # X100 Stage B 一键机器门
powershell -File scripts\perf.ps1      # 20k/10k 性能预算门（预算口径见 docs/08）
cargo clippy --workspace --all-targets # 要求新增 0
cargo fmt --all -- --check
cargo core-test                        # 只跑逻辑层，不开窗口

# 视觉回归
cargo golden --tol 0                   # 壳层基线逐像素比对
cargo golden --update --warmup 5       # 有意改外观后重拍壳层基线
MW_GOLDEN_UPDATE=1 cargo test -p mw-render --test offscreen -- canvas_matches_golden_baseline
MW_GOLDEN_UPDATE=1 cargo test -p mw-render --test text_render -- text_canvas_matches_golden
```

常用环境变量：`MW_OPEN=<path.zmind>` 启动即打开文档；`MW_HOME=0` 用内置夹具文档（跳过开始页）；`MW_WINDOW_STATE=off` 不恢复窗口状态；`MW_TRACE=<file>` 输出焦点轨迹；`MW_STATS` 输出帧统计。

> 更换 GPU / 驱动 / DPI 后 golden 可能漂移。**先量化差异、确认是环境而非回归，再重拍**；重拍命令与判定纪律见 [HANDOFF.md §1.3](HANDOFF.md)。

## 10 分钟上手

1. **新建**：开始页「新建空白图」或 `Ctrl+N`。
2. **长分支**：选中中心主题，`Tab` 建子节点直接打字；`Enter` 建同级。
3. **改结构**：拖动节点到另一个节点改父；拖到空白变自由主题；`Ctrl+Z` 撤销。
4. **全屏大纲**：`F8` 整块进入结构编辑，`Enter` 同级、`Ctrl+Enter` 子级、`Tab`/`Shift+Tab` 缩进升级，`Esc` 回画布。
5. **写卡片**：选中节点 → 命令面板「打开卡片」→ 直接所见即所得编辑、自动保存；`Esc` 关闭。
6. **管理卡片**：命令面板「卡片库」→ 三栏浏览、按标签筛选、多选批量归档/导出。
7. **找回来**：`Ctrl+F` 搜节点/卡片/标签；`Ctrl+K` 命令面板。
8. **复习**：命令面板「主动回忆」按到期日复习；「知识视图」看标签关系。
9. **导出**：`Ctrl+E` 选 PNG/SVG/PDF；`Ctrl+P` 直接 PDF。
10. **设置**：命令面板「设置」改自动保存间隔、默认导出值、笔记库路径。

完整操作见 [用户手册](docs/15-用户手册.md)。

## 下载与安装

- **便携版**：运行 `scripts\package.ps1` 生成 `dist\zhisi-v0.1.0-win64.zip`，解压后直接运行 `MindWeaver.exe`。
- **按用户安装**（无需管理员）：`powershell -File scripts\install.ps1`，会复制到 `%LOCALAPPDATA%\Programs\MindWeaver` 并注册 `.zmind` 关联；双击 `.zmind` 直接打开。
- **卸载**：`powershell -File scripts\install.ps1 -Uninstall`，移除程序目录、文件关联与卸载项；不碰文档与笔记库。

## 项目结构

```
crates/
  mw-core/    领域模型、命令、撤销、布局、场景图、主题解析、文本策略、检索
  mw-render/  wgpu 四层合成器、DirectWrite 排版、字形图集、离屏导出
  mw-shell/   eframe/egui 壳层、画布交互、面板、菜单、标签、文件对话框
  mw-ui/      与后端无关的界面状态与命中辅助
  mw-format/  .zmind 容器（ZIP + JSON）、原子保存、备份
  mw-io/      Markdown / OPML / FreeMind / XMind 导入与多格式导出
  mw-markdown/ 卡片笔记 Markdown 解析（纯 CPU、无 GUI，阅读/预览/导出共用一棵 AST）
  mw-golden/  壳层截图 golden harness
  mw-probe/   性能与启动里程碑探针
docs/         设计文档（见下）
scripts/      PowerShell 真实输入验收脚本（ASCII-only）
shots/        golden 基线、参考图与验收截图（默认不入库，见 .gitignore）
```

依赖方向严格单向：`mw-shell → mw-render / mw-core / mw-format / mw-io / mw-markdown`；`mw-core` 零 UI 依赖，是数据唯一权威。画布、离屏 golden 与 PNG/SVG/PDF 导出共用同一份 `Scene`，不存在第二套排版几何。

## 文档

| 文档 | 内容 |
| --- | --- |
| **[AGENTS.md](AGENTS.md)** | **AI 自主开发指南**：给下一任 AI 的自动驾驶手册，先读它 |
| **[HANDOFF.md](HANDOFF.md)** | **当前接手总入口**：构建、硬约束、证据命令和当前阻塞 |
| **[docs/STATUS.md](docs/STATUS.md)** | **当前状态唯一真相源**：做到哪、门绿没绿、下一步 |
| [PLAN.md](PLAN.md) | 已归档存根；旧 P/V/O/R/WP 路线见 `docs/archive/` |
| **[X100 升级路线](docs/24-X100升级路线.md)** | 以本机 Xmind 为参考的 100 轮核心体验重建计划；含 Stage B2 断点 |
| **[内容体验重构规范](docs/28-内容体验重构规范.md)** | **B2 设计合同**：大纲/卡片页/笔记库的共同公理、几何、排版、状态与验收判词 |
| **[内容体验重构工作包](docs/29-内容体验重构工作包.md)** | **B2 执行合同**：34 个工作包、4 批、4 道人工门、证据模板与风险 |
| [X100 验收规范](docs/26-X100完成定义与验收规范.md) | 行为/视觉/回归/工程四类证据 |
| [数据与命令审计](docs/27-X100数据与命令审计.md) | `.zmind` 兼容、样式/多画布扩展和迁移风险 |
| **[用户手册](docs/15-用户手册.md)** | **新用户 10 分钟上手**：画布、检查器、卡片、复习、导入导出、数据位置、FAQ |
| [贡献指南](CONTRIBUTING.md) | 环境、质量门、架构边界、包施工协议、golden 重拍纪律 |
| [变更记录](CHANGELOG.md) | 2026-10-21 前的变更快照（P01–P80、视觉重塑 V、第二班 S01–S16）；已停止逐轮更新，X100 逐轮见 `docs/x100/log/` |
| [docs/00-文档索引](docs/00-文档索引.md) | 三分钟索引、阅读顺序、文档维护纪律 |
| [docs/01-项目目标与适用范围](docs/01-项目目标与适用范围.md) | 定位、目标用户、v1 目标与非目标 |
| [docs/02-功能需求清单](docs/02-功能需求清单.md) | 功能矩阵、快捷键规格 |
| [docs/03-技术选型与开发环境](docs/03-技术选型与开发环境.md) | 现行纯 Rust 栈、环境、命令、测试矩阵 |
| [docs/04-系统架构与模块划分](docs/04-系统架构与模块划分.md) | 架构、crate 职责、数据流、线程模型、ADR 索引 |
| [docs/05-数据结构与文件格式](docs/05-数据结构与文件格式.md) | 内存模型、`.zmind` 格式、样式级联 |
| [docs/06-界面布局与交互设计](docs/06-界面布局与交互设计.md) | 布局、令牌、画布手势、面板规格 |
| [docs/07-中文支持专项设计](docs/07-中文支持专项设计.md) | IME、排版、字体、拼音检索验收标准 |
| [docs/08-性能与兼容性要求](docs/08-性能与兼容性要求.md) | P1–P15 预算与实测结果 |
| [docs/10-可扩展方向与演进路线](docs/10-可扩展方向与演进路线.md) | 1.x 方向与永久非目标 |
| [docs/archive/12-历史注记-WebView路线](docs/archive/12-历史注记-WebView路线.md) | 已废弃 WebView 路线的墓碑（不要按它实现） |
| [docs/13-纯Rust原生重构方案](docs/13-纯Rust原生重构方案.md) | ADR-010/011、逐轮实现日志（v3–v7）；**历史冻结**，按小节查 |
| [docs/archive/11-实现状态.md](docs/archive/11-实现状态.md) | 历史快照（2026-10-19） |
| [docs/archive/14-项目交接说明.md](docs/archive/14-项目交接说明.md) | 历史交接记录（旧数字不代表当前状态） |
| [docs/archive/22-检查器2.0设计.md](docs/archive/22-检查器2.0设计.md) | 历史设计输入，由 X41–X50 重新验收 |
| [docs/archive/23-大纲2.0设计.md](docs/archive/23-大纲2.0设计.md) | 历史设计 + O01–O03 实施记录 |

**阅读顺序**：`AGENTS.md` → `HANDOFF.md` → `docs/00` → `docs/24` → `docs/28` → `docs/29`；要动渲染或布局再加 `docs/04` / `docs/05` / `docs/13`。`docs/06` 的全屏大纲旧规格、`docs/24` 的 X61–X70/X81–X82 旧范围已被 B2 取代；232px 侧栏大纲仍有效。

## 关于 XMind

Xmind 26.5.1107 是 X100 的视觉/交互参考，不是构建依赖。参考图与版本 manifest 不随仓库分发；
没有 Xmind 的机器照样能开发、测试和重拍本项目 golden。
动态采样只允许隔离临时配置和空白文档，**不得自动化点击或修改用户正在使用的 Xmind 实例**。

## 许可

本项目采用 **MIT OR Apache-2.0** 双许可，可任选其一：

- [MIT](LICENSE-MIT)
- [Apache-2.0](LICENSE-APACHE)
- 说明见 [LICENSE](LICENSE)

提交贡献即表示同意按上述双许可发布。

## 参与开发

- 动手前先读 [HANDOFF.md](HANDOFF.md) 的硬约束与坑；外观改动必须走 golden 重拍协议。
- 当前主线是 **Stage C（X21+）**：B2 的设计/执行合同（[docs/28](docs/28-内容体验重构规范.md) / [docs/29](docs/29-内容体验重构工作包.md)）已收口；动画布与外壳时读 [docs/30](docs/30-外壳体验与概念图设计规范.md) / [docs/31](docs/31-外壳体验与概念图工作包.md)；从哪一项继续以 [docs/STATUS.md](docs/STATUS.md) §5 为准（**不是**从 CER-01 开始）；不要跳过人工门，也不要抢做未过门的下一阶段。
- 讨论范围以 [docs/01 §5 非目标](docs/01-项目目标与适用范围.md) 为界：不做协作、云同步、账号、移动端、网页版；B2 也不做表格/公式/Mermaid、云、AI、卡片彩色标签墙。
- 当前最重要的三件事：① 按 `docs/STATUS.md` §5 推进 Stage C（X21/X21b 余项与后续轮次）；② 每批末端人工验收，门未过不推进；③ 五款输入法与双屏/极端 DPI 的人工签字仍欠账。详见 [docs/STATUS.md](docs/STATUS.md)。
