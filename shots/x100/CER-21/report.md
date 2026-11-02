# CER-21 · B2-B 证据包与 G-B2-2

> 执行合同：`docs/29` §7 CER-21；设计合同：`docs/28` §6（卡片笔记页）、§9（证据与验收）。
> 交付：11 类验收场景齐备、`docs/28` §6 逐条核对、机器判据汇总、**门清单**（[gate.md](gate.md)）。
> 状态：**`已实现`**。门本身**未开**——八判词一条都没判，AI 不代写。

## 1. 本包做了什么

1. 盘点 `docs/28` §6.11 的 11 类场景，补齐缺的两类：
   - `card-code`：同页放代码块 + 行内码 + 链接 + 图片语法（新夹具 `MW_CARD_CODE`）。
   - `card-selection`：点进正文 → `Ctrl+A` 全选 → `Shift+←` 收三格；
     为脚本新增 `sclick`（Shift+点击）动词，跨块选区动线可脚本化。
2. 11 类场景全部登记进 stage-b 门（**79 场 / 158 次比较**，`--tol 0` 双跑）。
3. `docs/28` §6.1–6.12 逐条核对（§3），把「没做」的条款写清楚（§4）。
4. 写门清单 [gate.md](gate.md)：八判词逐条给证据入口与「已知缺口」，
   结论列留空给人工。

## 2. 11 类验收场景（`docs/28` §6.11）

| 场景 | 基线 | sha256_12 | 必测点 |
| --- | --- | --- | --- |
| `card-empty` | `shots/golden/card-empty` | `7E5A2665439E` | 空状态不空洞、可直接写 |
| `card-default` | `shots/golden/card-default` | `8645FCEDB4D1` | 排版节奏、对比度 |
| `card-code` | `shots/golden/card-code` | `4538E9B095ED` | 代码块容器、行内码、链接；图片只有语法（见 §4） |
| `card-tags` | `shots/golden/card-tags` | `FC2792EF8CEF` | 标签纯文字、来源、反链安静 |
| `card-focus` | `shots/golden/card-focus` | `94AA5DE08CBA` | 非当前块 35%、无遮罩 |
| `card-typewriter` | `shots/golden/card-typewriter` | `69D425ABFE83` | 光标 42%（探针 `ratio=0.420`） |
| `card-slash` | `shots/golden/card-slash` | `2BDD4F12CB41` | 280 宽 / 行高 30 / 最多 8 行 |
| `card-ime-preedit` / `-commit` | `shots/golden/card-ime-*` | `836E4F35358E` / `D9B029206B67` | 组合不提交、不拆块、行距不动 |
| `card-selection` | `shots/golden/card-selection` | `4B6FE15E5DF4` | 跨块选区、收格后锚点不跳 |
| `card-conflict` | `shots/golden/card-conflict` | `F6136E31812B` | 行内提示条不遮正文 |
| `card-narrow` | `shots/golden/card-narrow` | `60DBB34D81A1` | 800×600 版心收缩 |

`card-ime` 在合同里是一个场景，实现上拆成 preedit / commit 两帧（同一脚本的两端），
两张基线都进门；`card-selection` 的选区底色实测 32 319 px（跨块），收格后文本不变。

## 3. `docs/28` §6 逐条核对

| 条款 | 判据 | 实现 / 证据 | 结论 |
| --- | --- | --- | --- |
| §6.1 结构 | 40px 顶栏只放导航与状态、标题是正文 H1、元信息 12/18、标签 13/20 纯文字、版心 720/640 | `note_view.rs` 顶栏与元信息行；`card-default`、`card-tags` | 达成（**目录浮层未做**，见 §4.1） |
| §6.2 正文排版 | 四级标题、段落、列表、任务、引用、行内码、代码块、分隔线、链接、反链 | CER-15 整表；`card-blocks`、`card-code` | 达成（**图片未渲染**，见 §4.2） |
| §6.3 单一表面与语法揭示 | 块标记只在光标块露出；行内标记片段级；即时转换 + 一次撤销；粘贴规则 | CER-16/17；`card-reveal*`、`card-instant*`、`card-paste-*` | 达成 |
| §6.4 专注 / 打字机 | 35% alpha 无遮罩；光标 42%、180ms | CER-19；`card-focus`、`card-typewriter` + `MW_TW_PROBE` | 达成 |
| §6.5 斜杠菜单 | 280 / 行高 30 / ≤8 行 / 10 条 / `POPOVER` / IME 守卫 | CER-18；`card-slash*` | 达成（**`Ctrl+/` 未接线**，只能靠行首 `/` 触发，见 §4.3） |
| §6.6 标签、来源、反链 | 纯文字标签、来源可回跳、反链列页尾、断链删除线 | CER-14/15；`card-tags` | 达成 |
| §6.7 保存、冲突、只读 | 去抖 800ms、`保存中…/已保存`、行内冲突条、只读 | CER-20；`card-saving`、`card-conflict`、`card-readonly`、`card-autosave` | 达成（文档内卡片仍走模态，见 §4.4） |
| §6.8 键盘 | `Ctrl+S`、`Ctrl+Z/Y`、`Alt+Z`、`Alt+Shift+Z`、`Ctrl+Shift+O`、`Ctrl+/`、`Esc`、`Tab` | `note_view.rs` 键表 | **部分达成**：`Ctrl+S` / `Alt+Z` / `Alt+Shift+Z` / `Esc` 有；`Ctrl+Y` 重做、`Ctrl+Shift+O` 目录、`Ctrl+/`、`Tab` 焦点移动**未做/未验证**（§4.3） |
| §6.9 长文与性能 | 20k 打开 ≤150ms、输入无 >50ms、IME ≤16ms、块级缓存、图片懒加载 | CER-20 探针：5.5 / p95 3.6 / max 4.4 ms | **部分达成**：三条时间判据达标；块级缓存未上（不需要）；图片懒加载无从谈起（图片没渲染） |
| §6.10 禁止项 | 顶栏不重复标题、无常驻目录/工具栏、无分栏预览、无彩色 chip、无外框/阴影、正文 16/28 | `card-default`、`card-tags`、`card-code` | 达成 |
| §6.11 验收场景 | 11 类 + 必测点 | §2 表 | 达成 |
| §6.12 实现边界 | 单一真源、不整页切换控件、语法揭示底线、跨块编辑底线、撤销一次、性能 | 全页单一 `TextEdit`（不拆块）+ 派生 Markdown 排版；文档内卡片 flush 为 `Command::Batch` | 达成（跨块复制的内容正确性只有推理，见 §4.5） |

## 4. 已知缺口（门判词不得把这些算作通过）

1. **目录浮层 `Ctrl+Shift+O` 未做**：`Heading` 解析与测试还在，UI 侧的 240px 浮层没实现；
   `docs/28` §6.1 的「目录默认不常驻，快捷键打开」目前只有前半句。
2. **图片不渲染**：`![示意](shot.png)` 只作为 Markdown 文本参与排版，没有解码、没有懒加载；
   §6.2 的「图片最大宽 100%、圆角 10」与 §6.9 的「图片懒加载」都无从验证。
3. **键盘缺口**：`Ctrl+/`（斜杠菜单快捷键）、`Ctrl+Y`（重做）、`Tab`（标签/正文/反链之间移动焦点）
   未实现或未验证；`Tab` 在斜杠菜单里只用于「选中当前项」。
4. **文档内卡片的冲突仍是模态**：`conflict_ui` 是既有文档保存路径；合同 §6.7 要的行内条
   本轮只做在库内卡片上。
5. **跨块复制的内容正确性**没有机器证据：单表面 = 正文缓冲，`Ctrl+C` 复制的是缓冲子串，
   需要实机剪贴板核对一次（建议并入 G-B2-2 第 6 条 5 分钟任务）。
6. **实机微软拼音**仍未核对（CER-07 遗留，全员共用）。
7. **块级缓存未上**：20k 实测远低于预算（打开 5.5ms、输入 p95 3.6ms），所以没做；
   如果真实文档出现卡顿，这是第一处要补的（§6.12 把它列为性能手段）。

## 5. 机器判据汇总

- stage-b 门：**79 场 / 158 次比较**，`--tol 0` 双跑 0 差异（`target/x100/stage-b-gate.log`）。
- 全量测试：**912 passed / 0 failed / 1 ignored**（`docs/STATUS.md` §1）。
- fmt 干净；clippy 指纹 37 / 新增 0；release 构建通过（golden 跑的就是 release）。
- 逐包证据：`shots/x100/CER-13/` … `CER-20/`；本包 `shots/x100/CER-21/`（report + gate + manifest）。

## 6. 变更记录

- 2026-10-29 01:0x：CER-21 收口（证据包 + 门清单）。新增 `card-code` / `card-selection` 两个夹具与
  两张基线、`sclick` 脚本动词；11 类场景齐；`docs/28` §6 逐条核对落到 §3，缺口 7 条写进 §4；
  门 77 → **79 场 / 158 次比较**。G-B2-2 仍未开，八判词待人工。
