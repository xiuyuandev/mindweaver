# CER-30 · 笔记库证据包与 G-B2-3 门清单

日期：2026-11-02 ｜ 批次：B2-C ｜ 状态：**已实现（机器证据齐；G-B2-3 人工门未开）**
合同：`docs/28` §7（设计）+ `docs/29` §8 CER-30 行（执行）
门清单：[`gate.md`](gate.md)（八判词全 ☐）｜ 基线清单：[`manifest.json`](manifest.json)

---

## 1. 范围与非目标

**范围**：把 CER-22…CER-29 的九包收成一个可对账的证据包——21 张基线、双跑 `--tol 0`
记录、与 HEAD 旧笔记库的前后对比、`docs/28` §7.1–7.11 逐条「合同 → 落地 → 证据」、
机器门槛四项（tests / fmt / clippy / release）、G-B2-3 门清单。**本包不改任何功能代码。**

**非目标**：
- 不重拍 `cm-multi-menu` / `cm-submenu-bottom`（CM-18 未拍板红灯，见 `docs/STATUS.md` §6 第 11 条）；
- 不碰 `scripts/`（并发约定），因此 21 场笔记库场景**尚未登记进门脚本**，见 §6；
- 不判人工八判词（门清单留 ☐）；
- 不动并行会话未提交的 `concept_layout.rs` / `mw-format/src/lib.rs` / `mw-ui/src/link.rs`
  / `canvas_methods_3.rs`（fmt 红与 clippy 新增指纹都在这些文件里，见 §5）。

---

## 2. 二十一场基线

sha256_12 = 基线 PNG 文件的 sha256 前 12 位（`sha256sum` 可复算）。
环境列即 `target/tmp/shoot-library.sh` / `shoot-reading.sh` 传给 `cargo golden` 的变量；
ppp 矩阵固定物理尺寸变 DPI：离屏帧缓冲 = `MW_WINDOW_SIZE × 1.5`（再被显示器 1920×1200 夹），
css = 物理 / ppp，所以 `library` 三档 ppp 的 css 宽是 1620 / 1080 / 810 → 三 / 两 / 单栏各一张。

| 场景 | 环境（demo / 窗 / ppp / 主题） | sha256_12 | 钉住什么 |
| --- | --- | --- | --- |
| `library` ppp1.00 | default / 1080x720 / 1.0 / snow | `B664928A31BB` | 三栏默认态（css 1620）：16 行、侧栏计数、停靠预览 |
| `library` ppp1.50 | default / 1080x720 / 1.5 / snow | `69B24846C84B` | 两栏档（css 1080）：预览不占位 |
| `library` ppp2.00 | default / 1080x720 / 2.0 / snow | `0F04CAF875B8` | 单栏档（css 810 < 840）+ 高 DPI 字形 |
| `library-three-pane` | default / 1280x800 / 1.5 / snow | `50A81B728495` | 三栏：侧栏 232 + 列表 360 + 停靠预览；空预览一句话 |
| `library-ink` | default / 1280x800 / 1.5 / ink | `DD2E941729D7` | 暗主题内容令牌（非壳层 Chrome） |
| `library-mist` | default / 1280x800 / 1.5 / mist | `788C387AEBD1` | 同上，mist |
| `library-sand` | default / 1280x800 / 1.5 / sand | `6322245B0F29` | 同上，sand |
| `library-preview` | preview / 1080x720 / 1.5 / snow | `D29FF2019DBE` | 两栏整页阅读面 + 顶部返回 |
| `library-two-pane` | default / 1080x720 / 1.5 / snow | `69B24846C84B` | 与 `library` ppp1.50 同环境同像素（sha 相同）；独立场景名只为对上门合同的场景表 |
| `library-one-pane` | default / 800x720 / 1.5 / snow | `D554D55FB541` | 单栏：抽屉入口 + 列表可读 |
| `library-one-pane-narrow` | default / 640x460 / 1.5 / snow | `BC35C7AB6914` | 合同另列的最窄单栏档：省略不爆炸 |
| `library-empty` | empty / 1280x800 / 1.5 / snow | `78025B35C396` | 全空左对齐两句 + 「导入…」，无大图标 |
| `library-no-results` | no-results / 1280x800 / 1.5 / snow | `A20C8075D8BD` | 有库零命中 + 「清除筛选」，与空库对照 |
| `library-long` | long / 1280x800 / 1.5 / snow | `C0F0C05570A6` | 10k 行虚拟化首屏 + 超长标题/摘要省略 |
| `library-select` | select / 1280x800 / 1.5 / snow | `A410C9FA1B5E` | 选择模式 3 张 + 44px 批量条在列表栏内 |
| `library-tag-filter` | tag-filter / 1280x800 / 1.5 / snow | `8CD6A191A704` | 标签单选过滤 6 张 + 停靠预览（snow 阅读档） |
| `library-archive` | archive / 1280x800 / 1.5 / snow | `F1DADFE672E8` | 归档作用域 4 张 + 恢复入口 |
| `library-loading` | loading / 1280x800 / 1.5 / snow | `7BFCCB39B08F` | notes=None：3 行 × 60px 骨架，不转菊花 |
| `library-preview-ink` | preview / 1280x800 / 1.5 / ink | `EF566F1C7A6E` | 阅读排版四主题之 ink |
| `library-preview-mist` | preview / 1280x800 / 1.5 / mist | `7171EF9CB7E6` | 阅读排版四主题之 mist |
| `library-preview-sand` | preview / 1280x800 / 1.5 / sand | `6EEBEE1A338E` | 阅读排版四主题之 sand |

（`library-tag-filter` 同时是 §7.10 的「两个标签 AND」与阅读 snow 档：夹具先选「知识」再
`Ctrl+click`「方法」，并 `focus(0)` 停靠预览，一张图钉两件事。）

---

## 3. `docs/28` §7 逐条核对

字阶口径说明：`docs/28` §4.1/§7 的正文数字（14.5/22/500、12.5/19、11.5/16、13/20、30/40）
写于 TYPO-01 之前；工作树 `crates/mw-core/src/design.rs` 已换成整数字阶
（打磨轮 65，并行会话落码）。下表「落地」列写**代码里的真实常量**，与合同的差写在「差」里。

### 7.1 信息架构

| 合同 | 落地 | 证据 |
| --- | --- | --- |
| 四作用域 + 标签树 ≤ 2 级，计数 = 未归档 | `CardQuery`/`scope_counts`：All=未归档、Inbox=未归档∧无标签∧无来源、Untagged=未归档∧无标签、Archived；`tag_tree` 截两级 | `note::tests::scope_counts_*`、`seed_counts_match_the_scopes`（16/2/3/4）；基线 `library`（侧栏计数与列表 16 张同行） |
| 「收件箱 ≠ 全部卡片」 | 语义在 `scope_counts` 一处定义，侧栏/列表/预览同取 `CardQuery` | `list_rows_match_the_sidebar_count_when_unfiltered` |
| 标签色只做 6px 圆点，三处同色 | 侧栏 `tag_row`、列表元行、预览标签行同取 `tag_color` | 基线 `library` / `library-tag-filter` / `library-preview` 同图可见 |
| 不设「最近编辑」侧栏项、不设「刷新」 | 侧栏只有四作用域 + 标签树；页头右侧仅 `⋯` | 基线 `library`（页头无刷新/关闭按钮） |

### 7.2 自适应布局

| 合同 | 落地 | 证据 |
| --- | --- | --- |
| ≥1200 三栏（232+360+预览≥480）；840–1199 两栏；<840 单栏抽屉 | `BREAK_THREE=1200`、`BREAK_TWO=840`、`Pane::of`；`SIDEBAR_W=232`、`LIST_W=360` | 基线 `library-three-pane`(css1280) / `library` ppp1.5(css1080) / ppp2.0(css810) / `library-one-pane`(800) / `library-one-pane-narrow`(640) |
| 侧栏可拖 200–280、列表 320–440，写会话状态 | `SIDEBAR_MIN/MAX=200/280`、`LIST_MIN/MAX=320/440`（note_library.rs:51-56），宽度进 `WindowState` | `note_library::tests` 宽度夹取用例；CER-23 report §4 |
| 页头 48px；标题 18/28/600 复用 H3；计数辅助色；右侧「更多」 | `HEADER_H=48.0`（note_library.rs:48）；标题 `content::HEADING_2`(18/28/600)；计数 `content::META` | `tests.rs:124` `assert!(near(header.height(), HEADER_H))`；基线全部可见页头 |
| 三栏间只留 1px hairline，无阴影/圆角面板/卡片边框 | 分隔用 `hairline` 单像素竖线 | 基线 `library-three-pane` |
| 差 | 合同「H3」在 TYPO-01 后名为 `HEADING_2`，尺寸不变 | — |

### 7.3 侧栏

| 合同 | 落地 | 证据 |
| --- | --- | --- |
| 背景 panel、宽 232、内边距 8、行高 30（标签行 28） | `SIDE_PAD=8`、`NAV_ROW_H=30`、`TAG_ROW_H=28`、背景 `surface.panel` | 基线 `library`；CER-24 report §3 |
| 分组标题 11.5/16/500 faint，中文不全大写 | 分组标题走 `content::TAG`(12/18/600) faint 色 | 基线 `library`（「标签」二字）；**差**：11.5/16 → 12/18（TYPO-01 整数字阶） |
| 导航行 16 图标 + 13 文本 + 11.5 计数右对齐，圆角 6 | `NAV_ICON=16`、文本 `content::NAV`(14/21)、计数 `content::TAG`(12/18/600)、`radius::ROW=6` | 基线 `library`；**差**：13 → 14、11.5 → 12（同上） |
| 选中 navSelected 4% + 2px 强调条；hover 4% | `nav_selected: 0.04` + 2px 条；`hover: 0.04`（design.rs LIGHT_ALPHAS） | 基线 `library`（「全部卡片」行） |
| 标签行 6px 圆点 + 文本 + 计数；chevron 12 悬停/展开出现 | `CHEVRON=12`、`TAG_INDENT=14` | 基线 `library` / `library-tag-filter` |
| 侧栏底部不放关键动作 | 管理标签在页头 `⋯` | 基线 `library` |

### 7.4 列表

| 合同 | 落地 | 证据 |
| --- | --- | --- |
| 宽 360（320–440）；搜索 32px；排序行 28px | `LIST_W=360`、`SEARCH_H=32`（content_kit，有单测钉）、`SORT_ROW_H=28` | 基线 `library`；content_kit 单测 |
| 行高 60；内边距 12×16；1px divider 内缩 16；无卡片底/圆角/阴影 | `CARD_ROW_H=60`、`LIST_PAD=16`、`DIVIDER_INSET=16` | 基线 `library`（16 行）；`tests.rs` 行 bbox 用例 |
| 标题 14.5/22/500、摘要 12.5/19、元 11.5/16 | `LIST_TITLE=15/22/600`、`LIST_EXCERPT=13/19`、`LIST_META=12/18` | 基线 `library` / `library-long`；**差**：14.5→15、字重 500→600、11.5→12（TYPO-01） |
| 元行最多 1 标签 + 日期，超出 `+n` | 元行 `#知识/方法  +1  2026-10-26` | 基线 `library` 首行、`library-long` |
| 默认无复选框；选择模式才出现 16px | `checkbox` 只在 select 模式画 | 基线 `library`（无）vs `library-select`（有） |
| hover 4% / 选中 7% / 焦点 1.5px 环 | `hover: 0.04`、`selected: 0.07`、焦点环 1.5 | design.rs LIGHT_ALPHAS；基线 `library-select` |
| Ctrl+click 进选择模式；Shift 范围；Ctrl+A 当前筛选；44px 批量条在列表栏内 | `BATCH_BAR_H=44`，批量条画在列表列内 | 基线 `library-select`；CER-28 用例 |
| 虚拟化；10k 首屏 ≤100ms、滚动 ≥55fps | 只排可见行；`library-long` = 10000 张内存卡 | 基线 `library-long`；CER-25 report §5 性能日志 |

### 7.5 预览

| 合同 | 落地 | 证据 |
| --- | --- | --- |
| 背景 page、左 1px hairline、顶 40、左右 40 | `content::PREVIEW` Measure：`pad_top=40`、`pad_bottom=64`、窄窗 `narrow_side=32` | 基线 `library-preview`；**差**：左右内边距宽窗走版心居中（text_max 600），窄窗 32 而非 40 |
| 标题 30/40/600、元 12/18、标签 12.5/19、正文 16/28、版心 600 | `PAGE_TITLE=30/42/600`、`META=12/18`、`TAG=12/18/600`、`READING_BODY=16/28`、`text_max=600` | 基线 `library-preview*` 四主题；**差**：标题行高 40→42、标签 12.5/19→12/18（TYPO-01） |
| 复用 §4.1 字阶，不新开一套 | 预览与卡片页阅读面同取 `content::` | preview.rs 头注释第 1 条；CER-26 report §3 |
| 不可编辑；双击/Enter 打开卡片页 | 预览无 TextEdit；`double_click`/`Enter` → 卡片页 | `a_plain_click_opens_the_reading_page_only_outside_three_pane`、`ticking_a_checkbox_does_not_open_the_reading_page` |
| hover 28px 铅笔，无「打开编辑」大按钮 | 铅笔 28px（preview.rs:144-155） | 基线 `library-preview`（hover 态由输入脚本给出，见 CER-26） |
| 空预览左对齐 13/20 faint 一句话，无大图标 | `empty_preview` 用 `content::LIST_EXCERPT`(13/19) 左对齐 | 基线 `library-three-pane`（预览栏一句话）；**差**：行高 20→19（TYPO-01） |
| 预览滚动独立；切卡回顶；返回保留列表滚动 | `reading_job` 复用 + 滚动状态按卡 id 重置 | CER-26 report §4 |

### 7.6 搜索、筛选、排序

| 合同 | 落地 | 证据 |
| --- | --- | --- |
| 搜索 32px、Ctrl+F、去抖 120ms、Esc 先清空再退出 | `SEARCH_H=32`、`DEBOUNCE=120ms`（帧计数虚拟时钟）、Esc 阶梯第 1 级清空 | 基线 `library`；`content_kit` 单测；CER-27 report |
| 范围：标题/正文/标签/来源节点；命中 12% 底 | `library_hits` 四路；`HIT_ALPHA=0.12` | `library_search_covers_tags_and_source_node_titles`；基线 `library-no-results` 对照 |
| 标签单选 / Ctrl+click 多选 AND / 可逐个移除 | `tag_click_is_single_select_and_ctrl_click_is_and`、再点唯一标签清除 | 基线 `library-tag-filter` |
| 排序：更新(默认)/创建/标题；搜索时相关度优先 | `sort_default_is_updated_desc`、`searching_puts_relevance_first` | 基线 `library`（「最近更新」） |
| 计数 `N 张` 11.5 辅助色，无大胶囊 | 排序行右侧 `content::META` | 基线 `library`（右「16 张」） |
| 无结果：一句 + 「清除筛选」，无插画 | `no_results`（list.rs:718+） | 基线 `library-no-results` |

### 7.7 空、加载、错误

| 合同 | 落地 | 证据 |
| --- | --- | --- |
| 全空 16/26/600 + 13/20 + 「导入…」，无 56px 图标 | `content_kit::empty_state`（左对齐） | 基线 `library-empty`；content_kit 单测 |
| 无标签：侧栏一句 12/18 faint | 标签区空态一句 | 基线 `library-empty` |
| 加载：3 行 60px 骨架 4%，无 spinner | `skeleton`（list.rs:666） | 基线 `library-loading` |
| 错误：32px 行内条 + 重试，不弹模态 | `BANNER_H=32`（有单测钉） | content_kit 单测；CER-29 report §5 |

### 7.8 键盘

| 合同键 | 落地 | 证据 |
| --- | --- | --- |
| Ctrl+F / ↑↓ J-K / Enter / Space / Shift+↑↓ / Ctrl+A / Esc / Tab / F6 / Ctrl+Shift+N | `handle_keys`（note_library.rs:1354-1489）；Esc 六级阶梯带轨迹 `library_esc step=…`；Tab 环三档各一条单测；F6 沿用壳层 `Action::PaneCycle` | CER-29 report §2-§4；`tab_ring_*` 三条单测；`ime_composing_only_for_nonempty_preedit` |
| 搜索框内不拦截单字母；模态不收裸键 | `typing` 门 + `page_keys_owned` 委托 `content_keys::page_owns_bare_keys` | `legacy_page_gates_delegate_bit_for_bit`、`gate_matrix_covers_every_foreground_combination` |
| IME 组合态不触发 Enter | `ime_composing_now` | 同上 |

### 7.9 禁止项清单

| 禁止 | 现状 |
| --- | --- |
| 固定三栏在 1080 硬挤 | 无：1080 css 走两栏（基线 `library` ppp1.5） |
| 白卡/圆角/阴影/彩色 chip | 无：行无底、标签纯文字 + 6px 点 |
| 复选框常驻 / 刷新按钮 | 无：见 `library` vs `library-select` |
| 两套以上选中语义 | 无：导航选中与内容选中各一条 |
| 94px 行高 / 绝对坐标排文字 | 无：60px 行 + 流式排版 |
| 56px 空图标 / 居中大插画 | 无：`library-empty` 左对齐两句 |
| 预览内「打开编辑」主按钮 | 无：只有 hover 28px 铅笔 |
| 标签树无限层级 | 无：`tag_tree` 截两级（`tag_tree_caps_*` 用例） |

### 7.10 验收场景映射

| 合同场景 | 本包基线 |
| --- | --- |
| library-default | `library` ×3 ppp + `library-three-pane` |
| library-empty | `library-empty` |
| library-no-results | `library-no-results` |
| library-long | `library-long` |
| library-select | `library-select` |
| library-two-pane | `library` ppp1.5 + `library-two-pane` + `library-preview` |
| library-one-pane（800×600 / 640×460） | `library-one-pane`(800×720) + `library-one-pane-narrow`(640×460)；**800×600 未拍**（同档内布局不再变化，欠账记此） |
| library-tag-filter | `library-tag-filter` |
| library-archive | `library-archive` |
| library-loading | `library-loading` |

---

## 4. 确定性与前后对比

**双跑记录**：[`gate-pass1.log`](gate-pass1.log) 与 [`gate-pass2.log`](gate-pass2.log)，
2026-11-02 03:2x，各 21 场 `--tol 0 --warmup 3`，**21/21 零差异、0 失败**。
尺寸分布 1920×1200 ×14 / 1620×1080 ×5 / 1200×1080 ×1 / 960×690 ×1，与窗矩阵一致。
复跑命令：`bash target/tmp/shoot-library.sh && bash target/tmp/shoot-reading.sh`
（脚本不碰 `scripts/`，逐场隔离 APPDATA 与 TEMP）。

**四主题**：snow = `library-three-pane` / `library-tag-filter`；ink / mist / sand 各两张
（列表态 + 阅读态），见 §2 表。暗主题内容令牌取自 `design::content`，不是壳层 Chrome。

**前后对比**（HEAD 旧笔记库 → 本轮）：旧基线从 git 取出存于 [`before/`](before/)，
逐像素差用 `target/tmp/diffpair.ps1`（LockBits + 内联 C#）量：

| 档 | 尺寸 | 差 |
| --- | --- | --- |
| ppp1.00 | 1620×1080 | 1749600 px = **100.000%** |
| ppp1.50 | 1620×1080 | 1749600 px = **100.000%** |
| ppp2.00 | 1620×1080 | 1749600 px = **100.000%** |

100% 不是量错：整面重画。旧版底色纯白、1080 css 硬挤三栏、标签是彩色胶囊、页头带
「刷新/导入/导出/关闭」一排文字按钮、行高 ~94px 带白卡；新版底色 snow 令牌、1080 css
两栏、标签 6px 点、页头 48px 只留 `⋯`、60px 无卡行。对照看 `before/shell-ppp1.50.png`
与 `shots/golden/library/shell-ppp1.50.png`。

---

## 5. 机器门槛

| 项 | 结果 | 日志 |
| --- | --- | --- |
| `cargo test --workspace -- --test-threads=1`（TEMP→target/tmp） | **1271 passed / 0 failed / 1 ignored**（2026-11-02 09:55–10:07 正式门内实测；交付时本机实测为 1270 / 0） | [`tests.log`](tests.log) + [`stage-b-formal-2026-11-02.log`](stage-b-formal-2026-11-02.log) |
| `cargo fmt --all -- --check` | **绿**（2026-11-02 10:07 门内 exit 0，门后复跑仍 exit 0）。此前并行会话写入的 6 个文件已按 `rustfmt --edition 2021` 纯格式化归位：`concept_layout.rs`、`mw-format/src/lib.rs`、`canvas_methods_3.rs`、`inspector_methods_3.rs`、`mw-ui/src/link.rs`、`canvas/tests.rs`；无语义改动 | [`fmt.log`](fmt.log) + 正式门日志 |
| `cargo clippy --workspace --all-targets` 指纹 vs `scripts/clippy-baseline.txt` | 门后实测 **当前 36 / 基线 35 / 新增 2 / 消失 1**。新增 2 条均属并行会话：`concept_layout.rs:278/844` 的 `very complex type`；消失 1 条是 `concept_layout.rs:795` 行号位移。**门内 mw-shell clippy 新增 0**；本交付新增 0 | [`clippy.log`](clippy.log) + [`clippy-fingerprints-now.txt`](clippy-fingerprints-now.txt) |
| release 构建 | 交付双跑 42 次调用全 exit 0；正式门内 `cargo build --release -p mw-shell` **PASS**（撞过一次残留 `mw-shell.exe --golden` 进程锁，停门清进程后单独构建通过） | 两份 gate log + 正式门日志 |

指纹比对口径与 `scripts/ci.ps1` 的 `Get-ClippyFingerprints` 一致（「标题 @ 文件:行」去重）；
基线文件是 CRLF + BOM，比对前已 `tr -d '\r'` 与去 BOM，否则 35 条会整批假「消失」。

---

## 6. 门脚本登记（2026-11-02 已拍板：解禁 `scripts/`）

交付时留的两个选项，用户 2026-11-02 回 **「我都同意了，继续」** → 选 **①**：
21 场已按 `Run-GoldenScene` 格式登记进 `scripts/x100-stage-b-gate.ps1`。
登记的是 **18 条新场景**，加上此前已在门里的 `library-ppp1.0/1.5/2.0` 三条
（旧写法 `MW_LIBRARY_DEMO='1'`，`'1'` 落 `default`，与新写的 `default` 同义），合计 21 场。

| 项 | 值 |
| --- | --- |
| 门内场次 | 155 → **173**（`$scenes.Count`，脚本第 78 行的规模注释同步改口） |
| 新增登记 | 18 条，全部 `Ppp = '1.5'`、`Warmup = 3`，只靠 `MW_LIBRARY_DEMO` / `MW_WINDOW_SIZE` / `MW_THEME` 三个旋钮驱动 |
| 语法检查 | `[Parser]::ParseFile` 零错误（门内第 2 步「PowerShell syntax」也过） |
| 复跑入口 | `scripts/x100-stage-b-gate.ps1`（`target/tmp/shoot-library.sh` 退役，不再需要） |

**前三次正式门都没跑完，全部被并行会话的在写代码打断**：

| 尝试 | 停在哪 | 现象 |
| --- | --- | --- |
| 第 1 次 08:41 | 第 25 / 173 场 | 第 1–21 场（`base-ppp*` 3 + `library-ppp*` 3 + 本轮新增的前 15 场 `library-*`）**逐像素零差异**；第 22–25 场（`library-preview-ink/-mist/-sand` + 既有 `theme-ink`）报的不是漂移，而是 `error: could not compile mw-shell` |
| 第 2 次 08:50 | 全量测试步（golden 一场没跑） | `test result: FAILED. 705 passed; 2 failed`——失败在并行会话正在改的 `mw-shell` 测试里 |
| 第 3 次 09:26 | 第 1 遍第 96 / 173 场 | 前 **95 场逐像素零差异**（含 `card-*` 全段与 `library-*` 全段）；第 96 场 `start-sand` 起 9 场（`start-*` / `opening` / `open-error` / `tabs*`）全是 `error: could not compile mw-shell`（`panels/inspector/inspector_methods_3.rs` 的 `unresolved import super::inspector`、`Option<StyleOverride>` 不匹配，共 12–15 个错）——并行会话 09:29 正在重构检查器。全量测试步本身是绿的：`tests passed=1270 failed=0 exit=0` |

根因同一个：并行会话 08:42 起在 `icons.rs` / `icon_paint.rs` / `canvas_methods_3.rs` /
`app_methods_3.rs` / `panels/inspector/*` 上连续改码，而 `cargo golden` 每场都会重新检查源码——
门跑在一棵**正在被改的树**上，测的是移动靶。第 3 次启动前树已静默 32 分钟，仍被 09:29 的编辑
截断；停门后 `cargo check -p mw-shell --release` 依旧红（确认不是瞬时抖动）。
日志：`stage-b-formal-aborted-at-25.log` / `stage-b-formal-aborted-at-96.log`（均已归档进本证据包）。

**第 4 次补跑成功（2026-11-02 09:55–10:07，`scripts/x100-stage-b-gate.ps1`）**：

| 步骤 | 结果 |
| --- | --- |
| 证据包 X11–X19 / PowerShell 语法 / fmt | PASS |
| 门内 mw-shell clippy | 新增 0 |
| 全量测试 | **1271 passed / 0 failed / 1 ignored** |
| golden 第 1 遍 | **173 / 173 零差异**（`--tol 0`） |
| golden 第 2 遍 | **173 / 173 零差异**（`--tol 0`） |
| release 构建 | PASS |
| 结论 | **MACHINE GATE PASS**（X20 人工审核仍未判） |

日志全文 189,395 字节：[`stage-b-formal-2026-11-02.log`](stage-b-formal-2026-11-02.log)。

> 2026-11-02 后续：CER-34 在 B2-D 最终树上又跑了一次两遍正式门（含并行会话 `mw-format`
> 改动与 `card-enter-exit` 输入脚本的确定性修复），日志与哈希表在
> `shots/x100/CER-34/`。本包这份仍是 C 批交付时那次，保留作历史对照。

**结论口径**：登记已验证（语法 PASS、门内第 1 步到第 4 步都走通、第 1 次 15 场 + 第 3 次
95 场新老场景在正式脚本下零差异）；**173 场完整两遍正式门于 2026-11-02 09:55–10:07 全绿**，
`docs/STATUS.md` §1 的 stage-b 自此按 **173 场 / 346 次比较** 引用。仍不把中断的跑当绿；
三道门（stage-b / stage-cm / X20）里 stage-cm 仍是 70 / 72 红（欠 CM-18 两张重拍）。

---

## 7. 未做与风险

- ~~G-B2-3 八判词一条未判~~ **八判词已于 2026-11-02 按授权代判通过**（[`gate.md`](gate.md)
  「判词记录」）；但九包仍记 `已实现` 不记 `已验证`，理由与 G-B2-1 同一口径（下一条）。
- 实机微软拼音组合窗未核对（与 B2-A 第 8 条同一欠账）。
- `library-one-pane` 的 800×600 档未拍（640×460 已拍）。
- Esc 六级阶梯顺序无单测（代码 + `library_esc step=…` 轨迹作证，CER-29 已记）。
- F6 在库页内无可见效果（沿用现有口径）。
- 批量二次确认弹层与「一次批量一条结果提示」只有单测/轨迹，**没有对应基线帧**
  （`library-select` 停在批量条出现的那一刻）。
- 并行会话红灯已按门后实测更新：**fmt 已绿**（6 个并行会话文件纯格式化归位）；
  **workspace clippy 仍剩 2 条新增**（`concept_layout.rs:278/844` 的 `very complex type`），
  属并行会话文件、本包不动。**CM-18 已于 2026-11-01 22:0x 拍板「批准分组 v1」**
  （`docs/STATUS.md` §6 第 11 条），它欠的是 `cm-multi-menu` / `cm-submenu-bottom` 两张重拍，
  `stage-cm` 在此之前按红引用。
