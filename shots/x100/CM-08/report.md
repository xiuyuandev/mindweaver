# CM-08 概念图证据包与 G-CM-2 · 报告

> 状态：**进行中**（写完 §1–§5 后动代码，§6 起为回填）
> 设计合同：`docs/30-外壳体验与概念图设计规范.md` §5（概念图全章，逐条判词见 §6）
> 执行合同：`docs/31-外壳体验与概念图工作包.md` §6（CM-08，规模 M）、§3（完成定义）、§8（golden 协议）
> 依赖：CM-06（概念 ↔ 笔记联动）、CM-07（大纲视图与导出），两者均已 `已实现`
> 开工 / 完成：2026-10-26

## 1. 起点（审计结果）

`docs/30` §5.10 列了 10 个验收场景。开工前全仓只有 7 张概念图基线，缺 3 张，
且 10 张场景**从来没有一次被同一个门覆盖过**——`scripts/x100-stage-b-gate.ps1` 里
概念图是顺带塞进去的 8 组（`cm-*`），没有判词、没有场景清单、没有挂账对账。

| 场景 | 开工前状态 | 缺什么 |
| --- | --- | --- |
| `cm-default` | 基线已有 `5b0d9dc4…` | — |
| `cm-narrow` | 基线已有 `fc7f31f1…`（1.5 DPI） | — |
| `cm-arrange` | 基线已有 `56bc4017…` | — |
| `cm-crosslink` | 基线已有 `f2e948d1…` | — |
| `cm-card-link` | 基线已有 `b7d11182…` | — |
| `cm-card-link-back` | 基线已有 `1ee059f4…` | — |
| `cm-outline` | 基线已有 `16ced6ab…` | — |
| **`cm-node-edit`** | **没有** | 概念就地改名没有钩子，也没有基线 |
| **`cm-link-create`** | **没有** | 拖出连线 → 空短语编辑态没有钩子，也没有基线 |
| **`cm-label-ime`** | **没有** | 短语编辑框的 IME 组合态没有钩子，也没有基线 |
| `cm-roundtrip` | 测试已有（`mw-format::concept_map_roundtrips_positions_and_relationships`） | 无（按 `docs/30` 口径它就是断言不是截图） |

**结论**：本包不是「再拍几张图」，是把 G-CM-2 的四类证据凑齐——10 场景清单 + 可复跑门脚本 +
`docs/30` §5 逐条判词 + 挂账对账。缺的三张必须补，否则「新建连线即进入短语编辑态」
（§5.9 的核心禁止项）与「IME 组合不提交」（中文四痛点）一条证据都没有。

## 2. 范围

| # | 交付 | 落点 |
| --- | --- | --- |
| 1 | 三个演示钩子，全部走**真实 Command / 真实入口**，不绕逻辑：`MW_CONCEPT_EDIT=<概念>` 进就地改名、`MW_CONCEPT_LINK=<from>\|<to>` 走 `commit_link`（鼠标松手那一步的同一函数）、`MW_CONCEPT_REL_EDIT=<短语>` 打开已有连线的短语编辑框 | `crates/mw-shell/src/canvas/canvas_methods_3.rs`、`app_methods_1.rs` |
| 2 | 三份输入脚本：`cm-node-edit.txt`（F2 → 打新词 → Enter）、`cm-link-create.txt`（等编辑框出现即截）、`cm-label-ime.txt`（注入 preedit，不提交） | `shots/golden/inputs/` |
| 3 | 三张新基线（`--update` 拍摄，各双跑 `--tol 0`） | `shots/golden/cm-node-edit|cm-link-create|cm-label-ime/` |
| 4 | G-CM-2 机器门脚本：10 场景 ×2 遍 `--tol 0` + fmt/clippy/tests/release，日志落 `target/x100/stage-cm-gate.log` | `scripts/x100-stage-cm-gate.ps1`（新） |
| 5 | `docs/30` §5 逐条判词 + 挂账对账 + 阶段报告 | 本报告 §6/§9、`docs/x100/stage-cm-report.md`、`shots/x100/CM-08/manifest.json` |

## 3. 验收场景与判词

| 场景 | 环境 | 判词（这张图要证明什么） |
| --- | --- | --- |
| `cm-default` | `MW_CONCEPT_DEMO=1` 1280×800 @1.0 | 命题网不是辐射树：浅色圆角概念、直线箭头、短语胶囊、自由摆放 |
| `cm-narrow` | 同上 640×460 @1.5 | 窄窗下节点不溢出、短语不被裁 |
| `cm-arrange` | `+MW_CONCEPT_ARRANGE=1` | 与 `cm-default` 构成前后对比：分层、不重叠、层距 ≥56 |
| `cm-crosslink` | `+MW_CONCEPT_CROSS=1 +MW_CONCEPT_SELECT=用户痛点` + 脚本 | 交叉连线存在且渲染稳定；`Shift+方向` 5 下 = 右下 24/16px 一条撤销 |
| `cm-card-link` | `+MW_CONCEPT_CARD=1` | 卡片页来源行 = `概念图 · 用户痛点` |
| `cm-card-link-back` | `+MW_CONCEPT_CARD_BACK=用户痛点` | 回跳落在画布选中态（同一个 `jump_to_source`） |
| `cm-outline` | `MW_CONCEPT_OUTLINE=1` | 命题网按 `relationships` 展开成多级 + 页头「按链接层级展开」+ 页脚「N 个概念」 |
| **`cm-node-edit`** | `MW_CONCEPT_SELECT=用户痛点` + 脚本 F2/文字/Enter | 改名走 `SetText` 一条命令；框宽随文本重算；无白框、文字不位移 |
| **`cm-link-create`** | `MW_CONCEPT_LINK=用户痛点\|风险与依赖` | 新线落 `AddRelationship` 后**立刻**进短语编辑态（空 → 「＋ 链接词」占位），线已在但短语未填 |
| **`cm-label-ime`** | `MW_CONCEPT_REL_EDIT=带来` + 脚本 preedit | 组合期间短语框显示组合串、原标签未被写坏、组合中不提交 |
| `cm-roundtrip`（测试） | `cargo test -p mw-format` | `kind` / `pos` / `relationships`（含 label/color）逐字段往返相等，版本未升 |

## 4. 非目标

- **不改 `.zmind`**：`FORMAT_VERSION`、`Node` / `NoteCard` 字段一律不动（`docs/30` §5.2）。
- **不改渲染与交互**：本包只加**演示钩子**与证据，不动节点/连线/布局的任何视觉参数——
  一改就是重拍别人的基线，那是 CM-02–CM-05 的范围。
- **不做**大纲建/删连线、交叉微弧：两条都是 §5 的已知缺口，本包**只做判词与挂账**，不偷做半个。
- 不做曲线连接线、不做自动互链、不做多概念图共存。
- 不碰用户 Xmind、不提交 git、不操作用户个人文件。

## 5. 风险

| 风险 | 处置 |
| --- | --- |
| 编辑态截图带**闪烁光标**，两次跑可能不同帧 | 先按「跑两次 `--tol 0`」实测；若漂，`cm-node-edit` 改成**提交后**截图（Enter 已按），`cm-link-create` / `cm-label-ime` 保留编辑态但用 `wait` 固定帧相位 |
| `MW_CONCEPT_LINK` 直接用坐标脚本拖拽会随相机 fit 漂 | 钩子调 `commit_link`（松手那一步的真实入口），指针几何不进基线；报告写明这是「跳过手势、不跳过逻辑」 |
| 新钩子写进生产代码路径 | 三个钩子都是 `pub fn` 的验收入口，只在 `MW_*` 环境变量存在时被调用，与 `MW_CONCEPT_CROSS` / `MW_CONCEPT_SELECT` 同口径；不进任何交互分支 |
| 门脚本与 `x100-stage-b-gate.ps1` 重复维护 | 概念图门只锁 10 个 `cm-*` 场景 + 工程质量四项，不重复 B 轨 38 组；两份脚本互不覆盖对方场景 |

---

## 6. `docs/30` §5 逐条判词

判词只有三种：**做到** / **部分**（做了什么、缺什么）/ **没做到**。这一节就是 G-CM-2 的人工判词底稿。

### 6.1 §5.1 定义、§5.2 数据契约

| 条款 | 实测 | 判词 |
| --- | --- | --- |
| 允许多根/交叉/成环的网络 | `concept_rows` 环取首次访问边（`MAX_DEPTH=4096`）；`cm-crosslink` 有交叉；环单测在 `mw-core` | 做到 |
| `LayoutSettings.kind = "conceptMap"`，不新增字段、不升版本 | `cm-roundtrip`（`cargo test -p mw-format concept_map_roundtrips_positions_and_relationships`）断言 `kind` / `pos` / `relationships.label` / `color` 逐字段相等 | 做到 |
| 概念 = `parent_id = None` + `pos`；主概念也带 `pos`（CM-04 例外条款） | `validate` 按 `kind` 放行，`SetLayout` 切走时清掉 | 做到 |
| 关系 = `Node.relationships`（有向 + 短语 + 颜色），不用 `children` 表达层级 | 大纲/导出都走 `concept_rows`，`children` 在概念图恒空 | 做到 |

### 6.2 §5.3 节点

| 条款 | 实测 | 判词 |
| --- | --- | --- |
| 圆角矩形，**圆角 8** | `Shape::Rounded` → `NODE_RADIUS = design::radius::MD = **10**` | **部分**：形状对，圆角是 10 不是 8（差 2px，`docs/30` 与令牌体系二选一，得先定口径） |
| 宽 = 文本 + 28，夹 96–240；高 36 / 双行 52 | `CONCEPT_PAD_X=14`、`MIN_W=96`、`MAX_W=240`、`H1=36`、`H2=52`，`concept_boxes_follow_the_node_contract` 单测 | 做到 |
| 文本 14/20 居中、最多 2 行、超出省略 | `CONCEPT_FONT=14`、`center_text=true`、`lines.take(2)` + 补 `…` | 做到 |
| 超出省略**并给 tooltip** | 全仓无该 tooltip | **没做到** |
| 主概念 15/22 600 + 8% 强调底 | `PRIMARY_FONT=15`、`bold=primary`、`panel.mix(accent, 0.08)` | 做到 |
| 悬停 1px 强调描边 **+ 4% 底** | `HOVER_RING_W` 环有；节点填充只取 `st.fill`，无悬停底纹 | **部分**：环做到，底纹没做 |
| 选中 1.5px 强调描边 + 焦点环 | `SELECT_RING_W` + `SELECT_RING_GAP` 环在盒外 | 做到（见下一行） |
| 选中 **8% 底** | 同上，无选中底纹 | **部分**：环做到，底纹没做 |
| 卡片关联右下 5px 圆点 + hover 提示「已关联卡片：xxx」 | 渲染层与 `SceneNode` 都没有 `card_id`；画布也没画这个点 | **没做到**（联动本身走检查器/卡片页/笔记库，见 §6.5） |
| 层级提示：非主概念不缩进，不靠字号递减 | 概念图所有非主概念同 `14/20`，位置即层级 | 做到 |

### 6.3 §5.4 连线与链接短语

| 条款 | 实测 | 判词 |
| --- | --- | --- |
| 直线段 + 端点箭头 | `straight_between`（概念图分支），思维导图仍 `curve_between` | 做到 |
| 箭头 **6×4** | 实测长约 9、半宽 4.2（总宽 8.4） | **部分**：形状对，尺寸比文档大一档 |
| 线宽 **1.5**（屏幕空间恒定） | 关系线走 `relations` 实例，1.6 CSS px | **部分**：屏幕空间恒定做到，值差 0.1px |
| 默认颜色 = `text.secondary` | `theme.ui.dim` | 做到 |
| 跨分支连接用来源分支色（≤6 色循环） | `unwrap_or(theme.ui.dim)`——未设置 `color` 时统一 dim，没有按来源分支取色 | **没做到** |
| 短语胶囊 20px 高、`surface.page` 底、无描边、11.5/16 secondary | 编辑框与标签绘制都吃 `LIST_META`、无描边、底色 `canvas_bg` | 做到 |
| 线在标签处**断开 8px** | 单条 path 不切段，靠标签底色盖住（视觉等效，不是几何断开） | **部分**：效果等效，实现不同（`label_w = 文本 + 12`） |
| 空短语画 12px「＋ 链接词」faint | 渲染层空标签占位 | 做到（`cm-link-create` 的底图里能看到） |
| 双击就地编辑：本地草稿 / Esc 还原 / Enter 提交 / IME 组合不提交 | `rel_editor_ui`：`ed.ime` 时不吃 Enter；`cm-label-ime` 锁组合态 | 做到 |
| 交叉处轻微弧度（曲率 ≤8px） | 没有弧度代码，交叉就是两条直线 | **没做到** |
| 允许有向环，布局与遍历要能处理 | `concept_tree` 环安全；`concept_layout` DFS 丢回边 | 做到 |

### 6.4 §5.5 布局、§5.7 命令

| 条款 | 实测 | 判词 |
| --- | --- | --- |
| 拖动即定位写 `pos`；概念图拖到别的概念上不改结构 | `commit_concept_move` 只发 `MoveConcept` | 做到 |
| 重叠给 1px 强调红边提示（0.25s） | `overlap_flash` | 做到 |
| 整理布局：分层 + 父重心 + 等距 + 24 轮松弛，整体左对齐，一次命令可撤销 | `mw_core::concept_layout::arrange` + `Batch`；`cm-arrange` 与 `cm-default` 是前后对比 | 做到 |
| 层距 ≥56、层内 ≥24、不重叠 | `LAYER_GAP=56` / `NODE_GAP=24` + 单测 | 做到 |
| 概念图不参与树的 `fit`，用所有节点包围盒 | `concept_root_shift` + `request_fit` | 做到 |
| 全部变更走 `Command` 与撤销栈 | `AddFloatingTopic` / `SetText` / `AddRelationship` / `RemoveRelationship` / `SetRelationshipLabel` / `MoveConcept` / `DeleteNodes` | 做到 |
| 方向键 = 摆位 1px / `Shift` 8px，整组一条 `Batch` | `nudge_concepts` + `cm-crosslink` 脚本（5 下 → 右下 24/16px） | 做到 |
| 主概念可删，根重挂到剩下最靠左上的概念 | `DeleteNodes` 放行 `root_id`，`ChangeSet.before_root/after_root` | 做到 |
| 右键菜单不带树语义 | 三套概念图专属菜单，`concept_menu_has_no_tree_entries` 一类单测 | 做到 |

### 6.5 §5.6 与笔记联动

| 条款 | 实测 | 判词 |
| --- | --- | --- |
| 卡片绑定三态（新建 / 关联已有 / 解绑），`card_id` 同语义 | 检查器「内容」组，落地 `SetNodeCard` | 做到（解绑入口挂在检查器，画布上没有，见 §8） |
| 反查：笔记库来源显示 `概念图名 · 概念名` 并可点回 | `CardSource.phrase()` + `Shell::jump_to_source()`；`cm-card-link` / `cm-card-link-back` | 做到 |
| 大纲按 `relationships` 生成树展开（环取首次访问边）+ 页头标注 | `cm-outline` | 做到 |
| **「让用户能像编辑思维导图一样用大纲改结构」** | 大纲里只能新建概念（Enter）、改名、删除；**Tab/Shift+Tab/拖拽一律不接，也不能建/删连线** | **没做到**（CM-07 已挂账：缺一个「大纲行尾就地输短语」的设计，`docs/30` §5.9 要求先有短语编辑态才允许落线） |

### 6.6 §5.8 导出与往返、§5.9 禁止项

| 条款 | 实测 | 判词 |
| --- | --- | --- |
| `Scene` 是几何唯一真相源，PNG/SVG/PDF 零新增导出路径 | `export_svg` 有一条守卫测试防第二条路径；本包没动这三个文件 | 做到 |
| Markdown/OPML 按生成树导出并标注「概念图」 | `export_markdown` / `export_opml` / `export_outline` / `export_freemind` 共用 `concept_rows` | 做到 |
| `.zmind` 往返逐字段相等 | `cm-roundtrip` | 做到 |
| 禁止把概念图渲染成辐射树 | `cm-default` / `cm-narrow` | 做到 |
| 禁止无短语的线团：新建连线即进短语编辑态，未填不落盘 | `commit_link` → `start_rel_edit(blank_new=true)`；留空/Esc 撤线；`cm-link-create` 锁的就是这个态 | 做到 |
| 禁止曲线/贝塞尔、自动互链、自动生成概念 | `straight_between` 只在概念图；无自动互链代码 | 做到 |
| 禁止画布之外的第二套工具条 | 概念图沿用画布工具条 | 做到 |
| 禁止改 `.zmind` 版本或新增字段 | `FORMAT_VERSION` 未动 | 做到 |

**小结**：§5 共 48 条可判条款，**做到 37 条、部分 6 条、没做到 5 条**。
没做到的五条是：概念省略的 tooltip、卡片关联圆点、跨分支取色、交叉微弧、以及
**大纲改结构**（这是唯一一条「功能缺口」，其余四条是外观缺口）。
按 `docs/31` §3，G-CM-2 的人工判词**不能算通过**——门要过，得先把这五条补掉或改文档。

## 7. 证据

### 7.1 机器证据

| 项 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | 干净 |
| `cargo clippy -p mw-shell --all-targets` | 本轨动过的文件（`canvas.rs` / `canvas/` / `app_methods_1.rs` / `main.rs`）**新增 0**；工作区既有 **27 条**（`mw-core` / `mw-render` / `mw-io` / `export_pdf.rs` / `panels/outline.rs:3084`），全部落在本包未改动的文件，不计 |
| `cargo test --workspace -- --test-threads=1` | **813 passed / 0 failed** |
| `cargo build --release -p mw-shell` | 通过 |
| `.zmind` 往返 | `PASS concept map .zmind roundtrip` |

### 7.2 golden（10 场景 ×2 遍 `--tol 0`）

门：`powershell -File scripts/x100-stage-cm-gate.ps1`，日志 `target/x100/stage-cm-gate.log`
（2026-10-26 05:20:48 → 05:21:27，`MACHINE GATE PASS`）。

| 场景 | DPI | warmup | 新哈希（前 16） | 旧哈希 |
| --- | --- | --- | --- | --- |
| `cm-default` | 1.0 | 6 | `5b0d9dc4da7c680e` | 同（逐字节未变） |
| `cm-narrow` | 1.5 | 6 | `fc7f31f1a4c49138` | 同 |
| `cm-arrange` | 1.0 | 6 | `56bc40170f3de479` | 同 |
| `cm-crosslink` | 1.0 | 20 | `f2e948d1b9ac0eca` | 同 |
| `cm-card-link` | 1.0 | 6 | `b7d11182b516c577` | 同 |
| `cm-card-link-back` | 1.0 | 6 | `1ee059f441784f90` | 同 |
| `cm-outline` | 1.0 | 6 | `16ced6ab807687ec` | 同 |
| **`cm-node-edit`** | 1.0 | 20 | `2c390c94d6dbd179` | 新增 |
| **`cm-link-create`** | 1.0 | 10 | `98b6729cf088c199` | 新增 |
| **`cm-label-ime`** | 1.0 | 18 | `40cd7202a2646b3e` | 新增 |

旧七张**逐字节未变**——本包没碰渲染与交互，只加了三个钩子。

### 7.3 交互证据（trace）

| 场景 | trace 关键行 |
| --- | --- |
| `cm-node-edit` | `concept-hook ran=true` → `golden_input 9 text "与场景"` → `golden_input 10 key Enter` → 节点文字变「用户痛点与场景」，盒宽随之变宽 |
| `cm-link-create` | `rel-edit-open origin=(0.0,48.0) world=(-136.5,33.0) screen=(762.7,649.1) zoom=1.237` → `concept-hook ran=true`；新线已落盘、短语框为空（「＋ 链接词」） |
| `cm-label-ime` | 同上（`screen=(1140.0,779.0)`）+ `golden_input 11 ime preedit "拼音"`：组合串进框、原短语「带来」未被改写 |

### 7.4 反例（这两条是能失效的，不是摆设）

1. **IME 组合真的进了图**：同一环境、同一 warmup，一次不带脚本、一次带 `ime preedit 拼音`，
   `--tol 0` 报 **383 px 差（0.017%）**，包围盒 `x[1076..1100] y[765..781]`（24×16，正好两个字），
   **其余像素一致**——说明组合串只改了自己那一小格，原标签没被写坏。
2. **warmup 必须覆盖落定**：`cm-node-edit` 用 `--warmup 4` 拍的基线，换成 `--warmup 20` 复跑报
   **146 px 差、最大差 1**，包围盒 `x[589..1358] y[411..884]`（亚像素抖动铺满整张图）。
   用同一 warmup 重拍后两次 `--tol 0` 均 0 差异——基线对帧数敏感这件事被钉住了。
3. **钩子必须在布局落定后跑**：第一版钩子在 `Shell::new` 里直接调 `start_rel_edit`，
   编辑框跑到 `(215,115)` 之外（起点 `last_rect` 还是菜单栏没占位的那一版）。改成
   「画布 `show()` 之后 + 等 2 帧」才落在线段中点。

## 8. 还剩什么（G-CM-2 要过必须补的五条）

| # | 缺口 | 归属 | 备注 |
| --- | --- | --- | --- |
| 1 | **大纲里建/删连线**（连短语编辑态） | CM-07 挂账 → 需要一个新包 | 唯一的功能缺口；`docs/30` §5.9 要求先有短语编辑态，大纲行尾目前没有输入位 |
| 2 | 概念省略时的 tooltip | §5.3 | 省了字却看不到全句，等于把信息藏了 |
| 3 | 卡片关联右下 5px 圆点 + hover 提示 | §5.3 | 画布上看不出哪个概念挂着笔记 |
| 4 | 跨分支连线取来源分支色（≤6 循环） | §5.4 | 现在全部 dim，交叉时读不出从哪来 |
| 5 | 交叉处 ≤8px 微弧 | §5.4 | 两线穿心时看不清是不是一个交点 |

另有 6 条「部分」（圆角 10 vs 8、箭头尺寸 9×8.4 vs 6×4、线宽 1.6 vs 1.5、标签断口是等效实现、
悬停底纹、选中底纹）：它们不影响「这是不是一个命题网」，但不改文档就得改代码——**不能两边都不动**。

## 9. 纪律与边界

- 未提交 git；未操作用户 Xmind；未碰个人文件。
- 未改 `.zmind` 版本与字段、未改 `Node` / `NoteCard`。
- 生产代码只多了三个验收钩子（`Canvas::start_concept_edit` / `create_concept_link` /
  `edit_concept_phrase`）+ `Shell::concept_hook` 一次性字段；只有对应 `MW_*` 环境变量存在时才走。
- 新增两处 trace（`concept-hook ran=`、`rel-edit-open ...`）——都是验收用的打点，不进交互路径。
