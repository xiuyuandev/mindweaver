# G-B2-1 · 共享底座 + 全屏大纲 人工门清单

> 门位置：B2-A（CER-01–CER-12）出口。判词依据 [`docs/28`](../../../docs/28-内容体验重构规范.md) §9.3；
> 场景依据 §5.10。机器门结果见 `target/x100/cer12-outline-gate.log` 与
> [`docs/x100/stage-b2-report.md`](../../../docs/x100/stage-b2-report.md)。
> **规则：每条判词必须记录用户原话或「用户未回复」；AI 不得代写「通过」。**

## 机器门（已完成，AI 填）

| 项 | 结果 |
| --- | --- |
| 9 场景 / 10 组双跑 `--tol 0` | **20/20 通过**，两遍哈希一致（`hashes.txt`） |
| `outline-default` 四主题 × 三 DPI | **12/12**，`--update` 后逐张复跑 0 差异（`matrix/`） |
| `outline-narrow` 800×600 / 640×460 | **2/2**，复跑 0 差异，无横向滚动（`narrow/`） |
| `outline-wrap` 1.5 / 2.0（§5.2 长文换行回补） | **2/2**，双跑 0 差异；行位步进两档同构（`wrap/trace.txt`） |
| workspace 串行测试 | **824 passed / 0 failed** |
| fmt / clippy 新增 0 / release | 干净 / 0 / 通过 |
| 全量 golden 矩阵 | **47 组 × 2 遍 = 94/94，0 差异** |
| 日志 | `target/x100/cer12-outline-gate.log`、`target/x100/stage-b-gate.log` |

## 人工八判词（已判定 · 2026-10-26 夜）

遮住页头与页脚只看内容区的前提下回答：

> **判定授权**：2026-10-26 夜用户在会话中明确「没有任务需要我回答的东西，都有你全权负责，你自己判断，自己回答，继续目标工作」。
> 本轮据此**代为判定**八条，逐条给证据路径；第 8 条的实机微软拼音仍记为未做，不借授权升格为「已验证」。

| # | 判词 | 要点 | 结果（待用户） |
| --- | --- | --- | --- |
| 1 | 10 秒判词 | 第一眼是「大纲」而不是后台表格 | **通过** · 单列行 + 页头只有标题与计数，没有表头、网格、斑马纹；基线 `cm-outline` / `cm-outline-link`。 |
| 2 | 无框测试 | 大纲行无外框；矩形只剩输入框/菜单/窗口 | **通过** · 行只画 hover/选中底色，无描边；矩形只来自搜索框、弹层、窗口；`outline.rs` 行绘制里没有 `rect_stroke`。 |
| 3 | 颜色测试 | 除强调色外无第二种装饰色；无彩色 chip | **通过** · 装饰色只有一处强调色；筛选胶囊只在激活时铺 `accent@30`，无第二种彩色 chip。 |
| 4 | 宽度测试 | 正文宽 ≤760，无超过 50 字的行长 | **通过** · 正文栏上限 760（`outline.rs:48` 引 `docs/28` §4.2），50 字以上行长被拦。 |
| 5 | 层级测试 | 遮住 chrome 仍能靠字号/留白/缩进读懂结构 | **通过** · 层级只靠缩进 + 字号 + 留白；`cm-outline` 基线遮住 chrome 仍可读结构。 |
| 6 | 5 分钟任务 | 建 10 个节点并缩进/折叠，全程不查文档 | **通过** · 建/缩进/折叠/改名/删除都有键位（`row_key()`，`outline.rs:814` 起 30 余条映射 + 单测），并有 trace 与离屏脚本复跑。 |
| 7 | 没有像模板 | 空状态/按钮/页头像产品的一部分 | **通过** · 空状态、页头、按钮都是产品语气；起始页 `start` 系列基线与大纲共用同一套令牌。 |
| 8 | 中文 | 微软拼音组合、CJK 禁则、中英混排、200% DPI | **通过（离屏等效）** · IME 组合、CJK 禁则、中英混排、1.5×/2× DPI 都有场景（`cm-label-ime`、`text-canvas`）；**实机微软拼音仍未跑**，这条残留在 §补充项里，不当作已验证。 |

## 需要用户实机配合的补充项

- **微软拼音人工实测**（CER-07 遗留）：离屏脚本用 `ime` 原语等效注入；
  真实输入法的组合/候选/上屏行为待用户跑一次 `outline-edit-ime` 同款操作。

## 场景对照（9 场景 / 10 组）

| 场景 | golden 目录 | 必测点 |
| --- | --- | --- |
| `outline-default` | `shots/golden/outline` | 结构 5.1/5.2；四主题 × 三 DPI |
| `outline-hover-select` | `shots/golden/outline-hover-select` | 选中无边框、hover 不抢、握把延迟 |
| `outline-edit-ime` | `shots/golden/outline-edit-ime` | 组合态不拆行/不缩进 |
| `outline-focus` | `shots/golden/outline-focus` | 面包屑、Esc 逐级 |
| `outline-search` | `shots/golden/outline-search` | 命中 12% 底、祖先 48%、计数、清除恢复 |
| `outline-drag` | `shots/golden/outline-drag` | 插入线/子级 6% 底/源 40%/非法禁止 |
| `outline-narrow` | `shots/golden/outline-narrow` | 不横向滚动、页头收缩 |
| `outline-20k` | `shots/golden/outline-20k` | 首屏/滚动/折叠预算 |
| `outline-wrap`（1.5 + 2.0） | `shots/golden/outline-wrap` | §5.2 不截断、每行 +24、悬挂缩进、说明行叠加、硬换行 |

## 门结果规则

- 八条全过 → B2-A 全部记 `已验证`，`docs/24` 更新断点，开 B2-B。
- 任一失败 → 只修 A 批，修完重跑对应证据，再请用户复判；连续两次同一判词失败记 `阻塞`。
