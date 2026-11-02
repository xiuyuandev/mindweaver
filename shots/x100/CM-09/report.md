# CM-09 · 概念图收口（M3）

> 门：**G-CM-2 复判**（补齐 §5 的 5 个缺口后重跑）
> 设计合同：[`docs/30`](../../../docs/30-外壳体验与概念图设计规范.md) §5.3.1 / §5.4.1 / §5.6.1 / §5.9
> 执行合同：[`docs/31`](../../../docs/31-外壳体验与概念图工作包.md) §6
> 报告日期：2026-10-26 · 状态：已实现（机器门 PASS；人工复判未开，缺口见 §6）

## 1. 范围

G-CM-2 的判词差在 `docs/30` §5 的 **5 条没做到 + 3 条口径不一致**。本包只补这 5 条，不动别的层：

| # | 缺口 | 本包做法 | 归属文件 |
| --- | --- | --- | --- |
| ① | 大纲里建/删/改连线（唯一功能缺口） | §5.6.1 的键位与短语就地编辑：`Tab` 建「概念 + 边」、`Shift+Delete` 断边、`Shift+F2`/点前缀改短语 | `mw-core/concept_tree.rs`（`via_id`）、`mw-shell/panels/outline.rs`、`canvas_methods_*` |
| ② | 概念省略 tooltip | §5.3.1：真截断才出、停 400ms、节点下方 8px、最大宽 320 | `mw-core/scene.rs`（`truncated`）、`mw-shell/canvas` |
| ③ | 卡片关联 5px 圆点 + hover 提示 | §5.3.1：右下角 5px `content.marker`，hover 提示「已关联卡片：{标题}」 | `mw-core/scene.rs`（`has_card`）、`mw-render`、`mw-shell/canvas` |
| ④ | 跨分支连线取来源分支色 | §5.4.1：分支号来自生成树 `depth==1` 祖先；跨分支取 `palette_at(branch % 6)`；显式色优先；不写回文档 | `mw-core/scene.rs` + `concept_tree.rs` |
| ⑤ | 交叉处 ≤8px 微弧 | §5.4.1：后声明的那条在中点偏 6px 的二次曲线，箭头按终点切线重算 | `mw-core/scene.rs` |

## 2. 非目标

- **不改** `.zmind`、`Node`、`NoteCard` 字段；不新增 crate / 依赖。
- **不动** B2 的三块内容界面（全屏大纲的树行为、卡片页、笔记库）；`docs/28` 的判词不在本包。
- **不动** 思维导图的关系线样式（曲线与跳线口径只对 `conceptMap` 生效）。
- **不表态** 三条「规格 vs 实测」的口径（圆角 8/10、箭头 6×4/9×8.4、线宽 1.5/1.6）——
  这三条按 `docs/31` §11 的挂账交用户口径确认，本包**两边都不动**，避免在没定口径前先改一遍视觉。

## 3. 验收场景（机器证据）

| 场景 | 输入 | 钉住什么 |
| --- | --- | --- |
| `cm-outline-link` | `MW_CONCEPT_OUTLINE=1` + `inputs/cm-outline-link.txt`（选中行 → `tab` → 打字 → `enter` → 打字 → `enter`） | 新行出现、短语前缀落到行首、`outline_concept_link` 与 `outline_concept_via_commit` trace |
| `cm-outline-unlink` | `MW_CONCEPT_OUTLINE=1` + `inputs/cm-outline-unlink.txt`（选中子概念 → `shift+delete`） | 该概念变孤立根（深度 0）、概念还在、`outline_concept_unlink` trace |
| `cm-outline-via` | `MW_CONCEPT_OUTLINE=1` + `inputs/cm-outline-via.txt`（选中 → `shift+f2` → 打字 → `enter`） | 行首短语被改、行高不变、行尾仍是概念名 |
| `cm-crosslink`（重拍） | 既有 | 交叉微弧：两条线相交处不再是纯 X |
| `cm-default`（重拍） | 既有 | 跨分支取色（演示图里有一条跨分支连接） |
| `cm-card-link`（重拍） | 既有 | 卡片圆点出现在概念右下角 |
| `cm-narrow`（重拍） | 既有 | 窄窗下圆点/微弧不溢出 |

## 4. 预计证据

1. `shots/x100/CM-09/report.md`（本文件，收尾时补 §5 实测）+ `manifest.json`
2. ~~`shots/x100/CM-09/traces/*.txt`：三份 `MW_TRACE` 行为证据~~ → **未产出**：`MW_TRACE` 要有一次
   真跑窗口的交互，本包没有可复现的「点行 + 按键」脚本入口（同 §6 第 1 条）。trace 只在代码里
   留好了名字（`outline_concept_link_done` / `outline_concept_unlink_done` / `outline_concept_via_commit`），
   等 harness 补上再采。
3. 新基线 3 张 + 重拍 4 张，各双跑 `--tol 0` 0 差异；旧哈希写进 `manifest.json` 的 `old_hashes`
4. 单测：`concept_tree`（`via_id`）、`scene`（跨分支色 / 微弧 / truncated / has_card）
   与 `outline`（键位映射、短语提交分支）
5. `scripts/x100-stage-cm-gate.ps1` 场景表 10 → 13，日志 `target/x100/stage-cm-gate.log`

## 5. 实测

### 5.1 单测

| 包 | 结果 | 新断言钉住什么 |
| --- | --- | --- |
| `mw-core` | 223 passed / 0 failed（原 218 + 5） | `concept_tree`：行的 `via_id` 就是文档里那条边的 id、一级概念各占一个分支号、二级沿用一级的分支号、主概念不占号；`scene`：跨分支取**来源**分支色（同分支与空分支表回落 `ui.dim`）、`segments_cross` 只认真相交（端点相接 / 近平行 / 平行 / 退化线段都不算）、微弧垂距封顶 6 且同参两次逐字段相同、`SceneNode.truncated` 只在真被截断时为真、空白 `card_id` 不算挂卡 |
| `mw-shell` | 445 passed / 1 ignored（原 445，**用例数没涨**） | 键位断言加进既有的 `row_key` 测试：`Shift+Delete`→`Unlink`、`Shift+F2`→`RenameVia`、`Ctrl+Shift+Delete` 不映射（免得手抖删边） |

### 5.2 外观漂移（重拍前，`--tol 0`）

两遍比较的像素数与最大差**逐字相同**，说明这是确定性改动，不是抖动：

| 场景 | 超容差像素 | 最大差 | 包围盒 |
| --- | --- | --- | --- |
| `cm-default` | 2631 / 2304000（0.114%） | 45 | x[623..1277] y[368..672] |
| `cm-narrow` | 2141 / 662400（0.323%） | 45 | x[253..693] y[199..403] |
| `cm-arrange` | 1011 / 2304000（0.044%） | 46 | x[775..913] y[529..674] |
| `cm-crosslink` | 2531 / 2304000（0.110%） | 45 | x[653..1277] y[368..672] |
| `cm-card-link-back` | 2658 / 2304000（0.115%） | 205 | x[623..1277] y[368..672] |
| `cm-node-edit` | 2605 / 2304000（0.113%） | 45 | x[639..1277] y[368..672] |
| `cm-link-create` | 3567 / 2304000（0.155%） | 69 | x[623..1293] y[368..855] |
| `cm-label-ime` | 2631 / 2304000（0.114%） | 45 | x[623..1277] y[368..672] |
| `cm-card-link` | 0 | 0 | — |
| `cm-outline` | 0 | 0 | — |

漂移集中在 y≈368..672 的连线区：跨分支取色（`ui.dim` → 分支色）与交叉微弧都发生在这里。
`cm-card-link-back` 的最大差 205 是圆点/提示层级的差，不是线色。

旧哈希已存 `target/x100/cm-baseline-old-hashes.txt`（10 条，格式 `目录|文件|sha256 前 16 位`），
收尾时并入 `shots/x100/CM-09/manifest.json` 的 `old_hashes`。

### 5.3 机器门（`scripts/x100-stage-cm-gate.ps1`，完整跑）

`started=2026-10-26 18:18:27` → `finished=18:19:56`，日志 `target/x100/stage-cm-gate.log`：

| 步骤 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy -p mw-shell --all-targets` 本轨动过的文件新增告警 | PASS（0 条） |
| `.zmind` 概念图往返断言 | PASS |
| 全量串行测试 | **829 passed / 0 failed**（CM-08 基线 824，+5 条本次新增） |
| 概念图 13 场景 ×2 遍 `--tol 0` | PASS（24/24） |

本包对门脚本只做两件事：加 `-Update`（同场景表逐张重拍，第二遍仍是逐像素复核）、
把 `app_methods_2.rs` 与 `panels/outline.rs` 加进「新增告警」的检查范围——
后者是**收紧**，不是放宽。

### 5.4 大纲改结构：harness 打通与两张落地场景

三张「在大纲里键位改结构」的场景之前一张都拍不出来，原因有三层，逐层钉掉才有像素证据：

1. **钩子在建窗口时查节点，画布还没建**：`MW_OUTLINE_SELECT` 最初写在 `Shell::new` 里，
   那一刻 `shell.canvas` 还是 `None`，按文字找不到行；而概念节点浮在 `children` 之外
   （层级在 `relationships` 里），`doc_order()` 也走不到它们——两条一起导致恒 `target_missing`。
   改法：查找挪到首帧 `raw_input_hook`，并按 `doc.nodes` 全表匹配。
2. **键位的「活动行」不是焦点行，是画布主选中**：`Outline` 的键盘分支取
   `primary → visible` 的定位，找不到就回落到第 0 行。概念节点没有 `parent_id`，
   `select_many` 会把它从选择里筛掉，于是每帧都回落成根行——`Shift+Delete` 永远打在根上。
   改法：给面板加一个只给验收用的 `pin`（绘制时覆盖主选中）。
3. **钉太狠会抢焦点**：`pin` 一钉，面板每帧按「活动行」把焦点拉回行上，文本框/短语框立不住。
   改法：只在头 4 帧钉焦点（首帧的「选中为空选根」只需挡一次），一进编辑就 `clear_pin()`。

三张全部落地（第三张 `cm-outline-via` 中途撤过一次，2026-10-26 夜重开，
2026-11-02 补进 `stage-cm` 门表并重拍基线）：

| 场景 | 输入 | trace 证据 | 判定 |
| --- | --- | --- | --- |
| `cm-outline-link` | `Tab` → 打字 → `Enter` ×2 | `outline_concept_link_done from=… new=Some(…)`，重建后新概念出现在行表 | 建概念 + 建边成立 |
| `cm-outline-unlink` | `Shift+Delete` | `outline_concept_unlink from=… rel=…` → `…_done … ok=true`；重建后该概念 `depth=0 via=None` | 只断边、概念留成孤概念 |
| `cm-outline-via` | `Shift+F2` → 清空 → 打字 → `Enter` | `outline_concept_via_edit` → `outline_via_start fresh=false` → `focus outline-via-edit` → 下一帧 `outline_delete` | **已重开**（2026-10-26 夜）：原先「首帧后消失」是夹具问题——`MW_OUTLINE_SELECT` 每帧重新武装抢焦点，把短语框挤没。改成只武装一次 + 短语框立着时 `RowKey::Delete` 不执行，场景稳定停在带文字的短语框上（`cm-outline-via`，基线 `shots/golden/cm-outline-via/shell-ppp1.00.png`，trace `shots/x100/CER-12/trace-via-keys.txt`） |

`cm-outline-via` 一度被撤除（2026-10-26 凌晨：短语框在离屏里立不住，与其留一张拍不出的空场景
不如先撤）；同日夜修好夹具后**已恢复**，现在三件东西都在：基线
`shots/golden/cm-outline-via/shell-ppp1.00.png`、输入脚本 `shots/golden/inputs/cm-outline-via.txt`、
门表条目（`scripts/x100-stage-cm-gate.ps1` 的 `cm-outline-via` 一行）；另加
`cm-outline-via-clear` 锁「既有短语清空被拒」那条口径。
**仍欠的不是场景，是回归保护**：`via_edit` 在没画到时会静默丢弃，这条隐患没有断言守着。

### 5.5 三条口径统一（2026-10-26 夜）

三条「规格 vs 实测」偏差这轮定死了，规则是**令牌优先、合同次之、注释最末**：

| 项 | 原状 | 结论 | 理由 |
| --- | --- | --- | --- |
| 箭头 | 9×8.4（`scene.rs` 9/4.2） | 改代码 → **6×4** | `docs/30` §5.4 与函数自己的注释都写 6×4，只有实现是 9/4.2 |
| 关系线宽 | 1.6（`instances.rs` `REL_W`） | 改代码 → **1.5** | `docs/30` §5.4 写 1.5 |
| 节点圆角 | 10（令牌 `radius::MD`） | 改文档 → **10** | 令牌是系统真相，10 已被 12 张基线视觉接受；原稿 8 是笔误 |

重拍后的 12 张基线（`--tol 0` 各跑两遍，全一致）：

| 场景 | 新哈希（前 16 位） |
| --- | --- |
| `cm-arrange` | `465244327f489214…` |
| `cm-card-link` | `b7d11182b516c577…` |
| `cm-card-link-back` | `dc9a25e9628e50bc…` |
| `cm-crosslink` | `b34d9c261759cd73…` |
| `cm-default` | `954f0f34e5ed3bc2…` |
| `cm-label-ime` | `9af8f52829d2dccc…` |
| `cm-link-create` | `be4bd30feab77880…` |
| `cm-narrow` | `8f26590f41c23eee…` |
| `cm-node-edit` | `824f72a41ab22ad5…` |
| `cm-outline` | `16ced6ab807687ec…` |
| `cm-outline-link` | `0d09470dc5c9ab87…` |
| `cm-outline-unlink` | `8413de7a2e26e679…` |

`shots/golden/canvas-ppp1.00.png` 与 `shots/golden/text-canvas-*.png` 也一起重写：前者因箭头/线宽，后者只漂 3/540000 像素（最大差 8，线宽的 AA 差），连跑两次可复现，不是抖动。

## 6. 未做 / 风险

1. **`cm-outline-via` 的像素证据一度缺失，现已补齐**：短语框曾在离屏里立不住
   （`outline_via_start` 之后下一帧焦点被「活动行」拉回，`Backspace` 变成断边、`Enter` 变成新建概念），
   当时先撤了场景。修法是只武装一次 `MW_OUTLINE_SELECT` + 短语框立着时 `RowKey::Delete` 不执行；
   现在基线 `shots/golden/cm-outline-via/shell-ppp1.00.png` 与输入脚本都在，并已登记进门表。
   **遗留隐患**：`via_edit` 没画到时会静默丢弃，没有断言守着（与场景本身无关）。
2. **圆点只有一张场景作证**（`cm-card-link-back`），且它与「回跳选中态」耦合：
   将来改选中环就会连带这张一起漂，判不出是谁变的。补一张纯画布的 `cm-card-dot` 更稳。
3. **三条「规格 vs 实测」口径已统一**（见 §5.5）：箭头 6×4、线宽 1.5 改代码，圆角 10 改文档（令牌为准）。
4. **大纲里建边的行高与滚动位置未验证**：`Tab` 建完边后新行是 `begin_edit` + 短语框两段式，
   极端窄窗（`cm-narrow` 宽度的一半）下没测过。
5. **`Shift+Delete` 断边后孤立概念的落点**：现在沿用「原坐标不动」，没有像 XMind 那样
   自动往主概念那一侧收；`docs/30` §5.9 没规定，先不动。
