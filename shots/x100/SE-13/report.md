# SE-13 · 检查器四项样式能力（椭圆 / 圆角 / 字重三档 / 对齐）

> 批次：P 轨补齐包（G-B3-2 第 4 条缺口）
> 设计合同：`docs/30` §4.2（必选能力清单 14 项）
> 状态：**已实现**（代码、测试、基线、门全绿；`已验证` 仍欠实机字体目检，与 SE-01–12 同口径）
> 授权原话（2026-11-02）：「继续开发，而且呢，你帮助我验收，不需要我决定。」
> 字段例外的决定记录：`docs/32` §15；文档同步：`docs/05` §2.3、`docs/30` §4.2、`docs/31` §5

## 1. 范围

G-B3-2 判词 4 剩下四项「延后」的检查器能力，本轮全部补上：

| # | 能力 | 目标 | 验收 |
| --- | --- | --- | --- |
| 1 | 形状：椭圆 | 思维导图节点也能选椭圆（概念图早已默认椭圆） | 形状段出现「椭圆」；选后画布与 SVG/PDF 同源（负半径哨兵） |
| 2 | 圆角 0–24 | 「圆角矩形 / 直角」的半径可调，不再由 `Shape` 三档写死 | 24×72 stepper（步 2）；改值一条撤销；圆角行 🜂 恢复继承 |
| 3 | 字重三档 400/500/600 | 取代「粗体开/关」二值；B 降为它的快捷方式 | 三段控件；`TextMeasurer` 按字重度量；SVG 写真实 `font-weight` |
| 4 | 文本对齐 左/中/右 | 节点文字三种水平锚点 | 三段控件；SVG 写 `text-anchor`；概念图 `center_text` 仍优先 |

## 2. 非目标

- 不改主题令牌与既有默认值：**缺键即老行为**，除「出现新控件的那几张检查器基线」外全部零漂移。
- 不动概念图的椭圆默认与 `cm-*` 专属主题逻辑（概念图仍走 `center_text` 居中）。
- 不加 700 档；`bold: true` 保留为旧文件兼容别名，**解析成 600**（见 §3 的「为什么不是 700」）。
- 不改 `.zmind` 版本号：新键全部可选、缺省不写。

## 3. 字段方案（数据模型例外）

`StyleOverride` 新增三个可选键（`docs/30` §4.2 已批准的能力；`docs/05` §2.3 已同步）：

```jsonc
{
  "radius": 12,          // 0–24；仅圆角矩形/矩形生效
  "align": "center",     // left / center / right
  "weight": 600          // 400 / 500 / 600
}
```

- 三个键都带 `skip_serializing_if`：没设置过就**不写键**，既有 `.zmind` 与字节级夹具一个字节都不变
  （`Relationship` 的线型/线宽/注释是同一套先例）。
- 缺键 = 老行为：`radius` 按 `Shape` 默认、`align` 左对齐（概念图仍走 `center_text`）、
  `weight` 回落 `bold ? 600 : 400`。
- `TextAlign::from_name` 接受大小写不敏感的三个词，不认识的词**当作没写**（不报错、不迁移）。
- 数值钳制在解析层：`radius` 夹 0–24、`weight` 夹 400–700、`align` 非法值 → `None`。
- `Shape::Ellipse` 已存在（CM-12），本轮只开放 `StyleOverride::shape` 的 `ellipse`/`underline` 映射与检查器入口。

### 3.1 为什么 `bold` 别名是 600 而不是 700

TYPO-01（第 65 轮）把画布粗体字面定成 **Segoe UI Semibold 600**，`dwrite::WEIGHT_BOLD` 一直是 600。
若这里把 `bold: true` 解释成 700，`resolve_all` 会把所有旧文档的粗体文字换成另一个字面，
**每一张既有基线都会漂**——「缺键即老行为」这条不变量优先。单测
`bold_is_a_semibold_alias_not_a_bold_seven_hundred` 咬住这一点。

## 4. 落地清单

### 4.1 模型与解析（`mw-core`）

| 文件 | 改动 |
| --- | --- |
| `src/model.rs` | `StyleOverride` 三个新键（`radius` / `align` / `weight`，均 `Option` + `skip_serializing_if`） |
| `src/theme.rs` | `WEIGHT_REGULAR/MEDIUM/SEMIBOLD/BOLD_ALIAS` 常量；`ResolvedStyle` 的 `radius` / `align` / `weight` 解析与钳制；`TextAlign` 三档 |
| `src/text.rs` | `TextMeasurer::measure(..., weight: u16, ...)`（原 `bold: bool`） |
| `src/scene.rs` | 度量调用改传 `st.weight`；`BoxKey` 增加 `weight`，换字重必须重测盒宽 |

### 4.2 渲染（`mw-render`）

| 文件 | 改动 |
| --- | --- |
| `src/dwrite.rs` | 文本格式缓存键 `(size, weight)`；`WEIGHT_NORMAL/BOLD` 常量与主题同值 |
| `src/text.rs` | `TextJob.weight` / `OutlinedText.weight`；`node_text_align` 支持 `TextAlign::Right`；图标/标签/备注等非节点文字统一走 `weight_of_bold` |
| `src/instances.rs` | `corner_style(style, rect)`：圆角覆盖只对 `rounded`/`rect` 生效；椭圆走 `ELLIPSE_CODE` 哨兵 |

### 4.3 检查器（`mw-shell/src/panels/inspector*`）

| 控件 | 形态 | 键 / 撤销 |
| --- | --- | --- |
| 形状 | 四段：圆角 / 直角 / 胶囊 / **椭圆** | 点一次一条 `SetStyle` |
| 圆角 | 28 高行 + 24×72 stepper（0–24，步 2）+ ↺ 恢复继承 | 只在圆角矩形/直角下出现 |
| 字重 | 三段 `400` / `500` / `600`，行首「字重」+ 已覆盖圆点 + ↺ | 点一次一条 |
| 粗体 B | 二值快捷方式：开 → 600、关 → 400（**写 `weight`，不再写 `bold`**） | 与波形同一条 |
| 下划线 U | B / I 同排的开关（`shape = underline` ↔ 清空） | 同上 |
| 对齐 | 三段「左 / 中 / 右」 | 同上 |

下划线从「形状第四段」搬到 B / I 同排，是因为形状行要让位给椭圆，而 240 面板下
**五段 12.5px 汉字格放不下**（每格 42px < 三字 37.5px + 边距）；下划线本来就是
「只有一条底线」的文字形态，与 B/I 同类。理由与几何都写进 `docs/30` §4.2；
单测 `the_weight_and_align_segments_fit_the_narrowest_panel` 钉住三段不顶宽。

### 4.4 导出同源（`mw-shell`）

- SVG：`font-weight` 取 `TextJob.weight`（≠400 才写），`text-anchor` 跟 `TextAlign` 走
  （`start` / `middle` / `end`）；圆角/椭圆走 `rect_elt` 的同一分支。
- PDF：文字走字形轮廓（`OutlinedText` 的路径），字重与对齐天然同源；圆角/椭圆走
  `mw_render::corner_style` 的同一出口。

## 5. 证据

### 5.1 单测（新增 6 条）

| 测试 | 钉住什么 |
| --- | --- |
| `theme::tests::bold_is_a_semibold_alias_not_a_bold_seven_hundred` | `bold` → 600、显式 `weight` 优先、缺键 400 |
| `theme::tests::the_new_style_keys_default_to_the_old_look` | 三键缺省 = 老行为 |
| `theme::tests::the_new_style_keys_are_clamped_and_parsed` | 0–24 / 400–700 / 三词对齐 |
| `scene::tests::the_box_cache_remeasures_when_only_the_weight_changes` | 盒缓存键含 `weight`（漏了字宽不重测） |
| `instances::tests::an_override_radius_only_applies_to_shapes_that_have_corners` | 圆角只对有角的形状生效；椭圆哨兵 |
| `export_svg::tests::the_new_style_keys_reach_the_svg` | `font-weight="600"` / `text-anchor="end"` / `rx="14"` / `<ellipse>` |
| `format::tests::the_new_style_keys_are_optional_and_default_to_the_old_look` | 缺键不写、旧文件缺键读成 `None` |
| `inspector::tests::the_weight_and_align_segments_fit_the_narrowest_panel` | 240 面板不顶宽 |

### 5.2 基线（15 场，全部双跑 `--tol 0` 零差异）

> **2026-11-02 19:05 更新**：本节的漂移数是**重拍前**口径（拍在「检查器组内行距 8 → 6」
> 这处外观改动之前），已被 §9.1 的 14 张重拍实测取代；§5.3 的门读数同理见 §9.2 / §9.3。
> 两处冲突时一律以 §9 为准。

- 重拍 11 场：`inspector`、`inspector-narrow`、`inspector-empty`、`inspector-shape`、
  `inspector-ink`、`inspector-mist`、`inspector-sand`、`inspector-content`、
  `inspector-multiselect`、`inspector-error`、`inspector-stepper-edit`。
- 新增 4 场（`MW_STYLE_DEMO=<k=v,…>` 经 `Command::SetStyle` 喂一份覆盖）：

| 场景 | 覆盖 | 画面里能看出什么 |
| --- | --- | --- |
| `inspector-weight` | `weight=600` | 选中节点文字换 Semibold、盒宽跟着变；字重三段停在 600 |
| `inspector-align` | `align=right` | 文字贴右侧内距；对齐三段停在「右」 |
| `inspector-radius` | `radius=24` | 圆角顶格；圆角 stepper 显示 24 |
| `inspector-ellipse` | `shape=ellipse` | 卡片画成椭圆；形状段停在「椭圆」 |

- **漂移核算**：新增场景相对 `inspector-shape` 的差异（用像素统计量口径）：

| 场景 | 差异像素 | 包围盒 |
| --- | --- | --- |
| `inspector-weight` | 68 918 | x 19–1895（画布整树因盒宽变化重排 + 面板） |
| `inspector-align` | 35 135 | x 19–1683（画布文字锚点 + 面板） |
| `inspector-radius` | 37 238 | x 19–1699（画布圆角 + 面板） |
| `inspector-ellipse` | 54 140 | x 19–1902（画布椭圆 + 面板） |

  控制变量：`inspector` 与 `inspector-shape` 的差异只有面板区（x ≥ 1506）——说明
  「没喂覆盖的检查器基线只动面板」，画布零漂移。

### 5.3 门与本地门槛

- `stage-b`：173 → **177 场 / 346 → 354 次比较**，两遍零差异（四张新场景已登记进
  `scripts/x100-stage-b-gate.ps1`）。
- `stage-cm`：**73 场 / 146 次比较**两遍零差异（概念图路径未改默认值）。
- `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets`（新增 0）、
  `cargo test --workspace -- --test-threads=1`（TEMP 重定向）。

> **2026-11-02 19:47 更新**：上面「两遍零差异」是 SE-13 代码面收口**当时**的读数。
> 矩阵层最新复跑（同一二进制、两遍）`stage-b` 178/178、`stage-cm` 73/73 零差异，
> 但**正式门脚本本身仍待补跑**——读数、假红判定与三条限制见 §9.2 / §9.3；测试总数见 §9.4。

## 6. 复审修掉的洞（2026-11-02 自查，se13-reviewer）

本轮收口前做了一次只读复审（独立 agent，零改动），查出 5 处真问题，**全部当期修掉**：
其中第 1 条补了新单测（会红），第 2–5 条由既有基线与编译守卫盯着——它们改的都是
「画布/导出/编辑器同源」的判据，任何一处写回旧判据都会在像素或单测上立刻显形。

| # | 位置 | 问题 | 修法 |
| --- | --- | --- | --- |
| 1 | `mw-core/src/theme.rs`（`apply_concept_defaults`） | 概念图换 `cm-*` 主题时把主概念改成 `bold = true`，**却没搬 `weight`**；渲染/导出只认 `weight`，于是「中心概念」掉回 400，而 PDF 伪粗与行内编辑器还按 `bold` 算——两处口径打架 | 改 `bold` 时同步 `style.weight`（无显式覆盖才动）；新单测 `concept_themes_move_the_primary_concept_to_the_semibold_weight` |
| 2 | `mw-shell/src/export_pdf.rs`（伪粗描边） | 判据仍是 `t.bold`，显式 `weight=400` 的层级粗体节点画布常规、PDF 多一道描边 | 判据改成 `weight >= WEIGHT_SEMIBOLD`；既有 `bold` 经别名仍是 600，导出零漂移 |
| 3 | `mw-shell/src/canvas.rs`（编辑环） | 环自己又写了一份 `match shape`，没用 `corner_style`：`radius=24` 的节点圆、环是直角，`radius=0` 反过来 | 环半径改走 `mw_render::corner_style`，与合成器同出口 |
| 4 | `mw-shell/src/canvas.rs`（行内编辑器） | F2 进编辑后字族看 `geom.bold`、锚点恒左内距：设过 500/600 或右对齐的节点，一按 F2 文字跳字重 + 跳位置 | 字族按 `style.weight` 选（≥600 走粗体族）、`TextEdit::horizontal_align` 跟 `style.align` |
| 5 | `mw-render/src/text.rs`（右锚点 / 图标字重） | 右锚点用 `text_x`（含图标宽）→ 有图标的节点右内距多出一个图标宽；图标按节点字重画会超出固定的 `字号+6` 槽 | 右锚点改用 `pad_x`；图标固定 `weight_of_bold(false)` |

另两处**校验后不改**，如实记账：
- **500/600 的字族落差**：egui 侧只有 `zhisi-bold`（Segoe Semibold + 雅黑 Bold）与 `zhisi-semibold` 两个族，
  Canvas 画布侧走 DWrite 的真实字重；行内编辑器 500 落回常规族。两者在 500 档有肉眼可见的小差异，
  与「编辑器与画布像同一段字」的目标只做到了 400/600 两档。
- **500ms 合并是「同批 id」粒度**：500ms 内先点字重再点对齐会并成一条撤销（SE-08 既有行为），
  「每项一次撤销」只在两次点击间隔 >500ms 时成立。要更细就得把手势键带上字段名，属 SE-08 口径调整，未动。

## 7. 未做 / 风险

- **实机字体字面**：500/600 是否落到 Segoe UI Semibold / 微软雅黑 Semibold 需实机目检；
  离屏只验 DWrite 查询到的字面与基线。与 SE-01–12 的「实机拼音/DPI」欠账同一口径，
  因此本轮记 `已实现`，不记 `已验证`。
- **右对齐的光学余量**：右侧同内距镜像后，边缘可能比左对齐少 1px 视觉余量；
  等宽字体下不可见，中文标点收尾时理论上存在。未调整，如实记账。
- **椭圆 + 多行文字**：思维导图节点选椭圆时，文字盒不变（与概念图同一约定），
  四角可能压到弧线；概念图的 1.22×1.16 外扩是概念图专属，本轮没有搬到思维导图节点上。
  若后续要补，应作为「形状外扩」统一项，不在检查器里特判。
- **`bold` 字段不再被检查器写入**：旧文档里残留的 `bold: true` 在设过 `weight` 之后被静默忽略
  （`weight` 优先），这是有意的优先级，不是丢失。
- **概念图不出「对齐」行**（复审修）：渲染层是「显式 `align` 优先于 `center_text`」，
  而 `docs/30` §5.3 的合同是概念居中。两个口径只能选一个，这里按合同做——概念图不给入口，
  于是不会有显式覆盖；从思维导图文档带过来的 `align` 若出现在概念图里，仍按显式覆盖优先（渲染层不改）。

---

## 8. 文档同步（2026-11-02 第二轮文档整顿）

> 交接里把这节叫作「§9 文档同步」；写的时候本报告只到 §7，所以取空号 **§8**——同一轮里「机器验证现状」后来占了 **§9**，两节正好 7 → 8 → 9 顺排，不留空号。
> 本轮只改三份入口文档（`AGENTS.md` / `HANDOFF.md` / `README.md`）里**已经过期的数字与状态**，
> 把会腐烂的快照换成「引用 `docs/STATUS.md`」，或换成本轮真实值；不动任何结论、不动代码、不动 `docs/STATUS.md`。

### 8.1 `AGENTS.md`（14 处）

| 位置 | 改动前 | 改动后 |
| --- | --- | --- |
| 文首「更新」行（19:48 又改一次，见 §8.5） | `stage-b` 173 / 346、`stage-cm` 73 / 146「两遍零差异」、测试 1279 | 指向 `docs/STATUS.md` §1 + 本轮快照（测试 1313 / 0、`stage-b` 登记 178 场 / 356 次、`stage-cm` 73 / 146、基线 254、输入脚本 152），**并写明 19:05 矩阵实跑 177/178、70/73 不是全绿** |
| §2 标题 | 当前断点（2026-10-30） | 当前断点（2026-11-02） |
| §2「阶段」 | X100 Stage B2 内容体验重构（进行中） | **Stage C（X21+）**：B2 已收口并过四道人工门；当前 X21 / X21b；下面保留 B2 各批收口记录 |
| §2 M 轨 CM-14 | 「基线一张都没重拍……`cm-multi-menu` 与 `cm-arrow-menu` 必漂」 | 基线状态与最近一次 `stage-cm` 实跑（含红灯）以 `docs/STATUS.md` §1 为准 |
| §2 M 轨 G-CM | G-CM-1 未开、G-CM-2 人工未通过（未复判） | **G-CM-1 / G-CM-2 均「通过 4/4」（2026-11-02 授权代判）**，判词见 `shots/x100/CM-05/gate.md`、`CM-09/gate.md`；门通过 ≠ `已验证` |
| §2 P 轨 G-B3-2 | 判为 4 / 5，缺字重 / 对齐 / 圆角 / 椭圆四项 | **复判 5 / 5 通过（2026-11-02 授权代判）**，四项由 SE-13 补齐、检查器组内行距改合同值 6 |
| §2「机器门现状」（19:48 又改一次，见 §8.5） | `stage-b` / `stage-cm`「全绿，零差异」+ 两份 log | 改成分「**登记场次**（178 / 356、73 / 146、基线 254、输入 152）」与「**最近一次实跑不是全绿**（177/178、70/73，逐盏红灯与并行会话归因建议）」，并让读者以 STATUS §1 为准 |
| §2「Stage C 冻结」条 | Stage C（X21+）冻结，直到 G-B2-4 通过 | **已解锁**（G-B2-4 2026-11-02 通过），当前 X21 / X21b |
| §7 第 9 条 | 「本节以前写死的『65 场零差异』『2 个场景缺基线』」 | 改成「以前写死过的场次数字都已是历史」，并把纪律补成「注明日期 / 是整门还是单场景复跑」 |
| §7 第 10 条 | 「G-B2-2 八判词一条都没判」 | 已于 2026-11-02 授权代判通过（门过 ≠ `已验证`） |
| §8 第 4 条 | `stage-b` 173/346、`stage-cm` 73/146 两遍零差异 + 36 场重拍 | 指向 `docs/STATUS.md` §1 与 `shots/x100/CER-34/report.md`，不再复制场次 |
| §8 第 5 条 | G-B3-2 判为 4 / 5；「CM-10 收口 + G-CM-2 复判；补开 G-CM-1」 | 全部收口：G-B3-1 5/5、G-B3-2 复判 5/5、G-CM-1 / G-CM-2 通过 4/4，附五份判词路径 |
| §8 第 7 条 | 「下一轮从 X21 相机 / 底色 / 纹理 / 缩放 / 滚轮锚点开始」 | X21 与 X21b **已落地**（证据 `shots/x100/X21/`、`X21b/`），下一项只看 STATUS §5 |
| §8「不要做」第 1 条 | 「跳过 B2 人工门直接开始 Stage C」 | 改成通用纪律：不跳过未通过的人工门开始下一批 / 下一阶段（门过没过按 STATUS §1 与该门 `gate.md`） |
| §8 末尾 CM-18 条 | `stage-cm` 现为 73 / 73 两遍零差异 | 登记场次与最近一次实跑（含红灯）见 `docs/STATUS.md` §1 |

### 8.2 `HANDOFF.md`（9 处）

| 位置 | 改动前 | 改动后 |
| --- | --- | --- |
| 文首「更新」行 | 2026-11-01 打磨轮 54；`stage-b` / `stage-cm` 连跑全绿 | 2026-11-02 19:26：B2 收口、八道门授权代判通过、Stage C 解锁、X21 / X21b 落地中；**数字与红灯只看 STATUS §1**；带授权原话 |
| §1「当前」 | 「Stage C 已解锁，下一项是 X21」 | 八道门均通过（2026-11-02）；X21 / X21b 落地中；进度 / 测试数 / 红灯只查 STATUS §1 |
| §5 文件地图三行 | `stage-b-report`「人工未通过」、`stage-b2-report`「B2-B 章进行中」、`stage-cm-report`「人工未通过」 | 改中性描述 + 「当前门状态见 `docs/STATUS.md` §1」；`stage-cm-report` 注明 G-CM-1 / G-CM-2 已于 2026-11-02 判「通过 4/4」 |
| §6 接手第 4 步 | 「下一项是 X21」 | X21 / X21b 已落地 |
| §7 顶部补记 | 「下一项是 Stage C 的 X21」 | X21 / X21b 亦已落地，下一步只看 STATUS §5 |
| §7.1 整节 | 「三十秒状态（本机实测 2026-10-31 13:24）」+ 1081 测试 / 58·116 / 139·278 / clippy 36 条 + 测试数分项 | 改成「状态与数字只查 STATUS §1，本节不留快照」的两列表（去哪看 / 怎么复跑），快照全删 |
| §7.3 撤销栈 | 「上限仍是硬编码 500（未做设置项）」 | 已可配（M28）：`Document.history_limit` 默认 500、clamp 100–2000、不写 `.zmind`；设置中心「编辑」组滑轨 |
| §7.5 第 5 条「未做」 | 缩略图 / 力导向布局 / **钉住** / **只整理选中** | 只留缩略图 / 力导向布局；钉住与只整理选中划掉（打磨轮 53 / C18 已补） |
| §7.7 第 6 条 | 「或按 STATUS §5 回到人工门走验收——**别抢开 Stage C**」 | 标 ✅ 2026-11-02 全通过（G-B3-2 复判 5/5）；Stage C 已解锁，顺序只看 STATUS §5 |

### 8.3 `README.md`（5 处）

| 位置 | 改动前 | 改动后 |
| --- | --- | --- |
| 小节标题 | 当前状态（2026-10-30，X100 Stage B2） | 当前状态（2026-11-02，X100 Stage C） |
| 状态块 | 「Stage C 冻结」「B2-B 等 G-B2-2 人工判词」 | B2 收口、八道门 2026-11-02 授权代判通过（附原话）、Stage C 已解锁、当前 X21 / X21b；补「门过 ≠ `已验证`」 |
| 状态表「版本」行 | X100 Stage B2 进行中 | X100 Stage C（X21/X21b）进行中 |
| 「参与开发」第 2 条 | 「当前主线是 Stage B2 内容体验重构 + 打磨轮……不要先做 Stage C」 | 当前主线是 Stage C（X21+）；B2 合同已收口，改画布/外壳时读 `docs/30` / `docs/31`；顺序只看 STATUS §5；不抢做未过门的下一阶段 |
| 「参与开发」末条 | 三件事第①件「按 B2 把大纲做成幕布式……」 | 第①件改成「按 STATUS §5 推进 Stage C（X21/X21b 余项与后续轮次）」 |

### 8.4 体检与边界

- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts/doc-hygiene.ps1` → **通过**：
  `文档体检通过：56 篇 / 271 条相对链接 / 9 条过期规则`，退出码 0；收尾又跑一次，仍是 56 篇 / 271 条链接。
  （链接数从上一轮的 259 涨到 271，是并发会话正在补 `docs/31` 与 `shots/` 的结果，不是本轮新增。）
- 未改：`docs/STATUS.md`（Lead 负责）、`crates/`、`scripts/`、其他 `docs/` 文件、任何结论性内容。
- **故意保留**的旧数字：`AGENTS.md` §2「打磨轮」逐轮清单、§8 第 3 条打磨轮清单、`HANDOFF.md` §7.7 逐轮 ✅ 行、
  §7.2/§7.3 的 2026-10-31 现场正文——它们都带日期，属于历史记录，不是当前状态；删掉反而丢掉漂移账。
- **遗留（本轮没动）**：
  - `AGENTS.md` §4 第 4 项仍写「**当前批次**的合同：B2 设计 / 执行」；B2 虽已收口，这两份仍是设计与执行口径的来源，
    改写会牵动阅读顺序，留待 Lead 决定是否换成「B2 合同（已收口，查历史用）+ Stage C 合同在哪」。
  - `AGENTS.md` §2「打磨轮」段落把 1–67 轮全列一遍（约 40 行），本身不是数字腐烂，但已经很长，
    更适合按 `docs/00` §4 的纪律挪进 `docs/x100/log/`；本轮不动结构。
  - `README.md` 状态表「构建」行「最近一次 release 见 HANDOFF.md」：HANDOFF 已不再写 release 字节数，
    这句应改成「换机后自行 `cargo build -p mw-shell --release`」，同样留给下一刀。

### 8.5 19:48 追加修订（矩阵全绿，撤掉「不是全绿」的写法）

Lead 19:35–19:47 用同一二进制（mtime 19:32:30，场景表与环境同正式门）重跑矩阵：
`stage-b` **178 场 × 2 遍 = 356/356**、`stage-cm` **73 场 × 2 遍 = 146/146** 全部零差异（MATRIX PASS）。
19:05 那次的 `stage-b` 177/178 与 `stage-cm` 70/73 因此判定为**并发假红**：并行会话正在 `rustc`
构建并跑 `mw-shell`（单场 16.5s）抢 CPU/GPU，harness 超时；`mm-rel-link-created` /
`mm-summary-hover` / `card-autosave` / `search-text` / `mm-zoom-wheel` 单场景复跑逐个通过，
19:20–19:30 那两遍各 10 场失败且**互不相同**；真漂移只有 `cm-inspector` / `cm-inspector-pinned`
（检查器组内行距 8→6，只漂面板区），已重拍并复核零差异。

据此改了三处（§8.1 表里两行 + `HANDOFF.md` 文首）：

- `AGENTS.md` 文首：「但最近一次矩阵实跑不是全绿（19:05：177/178、70/73）」→「矩阵层面已全绿
  （19:35–19:47，356/356、146/146，MATRIX PASS）」+ 并发假红的归因；
- `AGENTS.md` §2「机器门」段：同样的替换，并把三条限制写全；
- `HANDOFF.md` 文首：加一行 19:48 口径（矩阵全绿 + 假红归因 + 正式门待补）。

**仍然并列保留的限制（不许只写绿）**：

1. 这两遍跑的是**已构建二进制**，不是正式门脚本本身（脚本会被并行会话的源码改动触发约 4 分钟
   LTO 重建），**正式门整跑待对方落笔后补**；
2. 全量测试 1313 / 0 / 1 ignored、`cargo fmt`、门内 clippy 是**分别实测**，不是一次门脚本的产出；
3. `已验证` 仍欠实机拼音与系统 DPI——矩阵绿不等于实机验收。

本报告 §5.3 的「19:05 更新」小块与 §9.2 / §9.3 的 19:05 读数按时间**留档不改**（那两处归 SE-13 作者
维护）；现在对外口径以本节为准。

---

## 9. 机器验证现状（2026-11-02 19:05 初跑 · 19:47 复跑）

> 本节是**收口后补记**的机器面读数，补的是 §5.2 / §5.3 写完之后才发生的事：
> ①「检查器组内行距 8 → 6」这处外观改动落地，检查器基线因此重拍了一批；
> ② 矩阵在全量场次上重跑了一遍。**凡与 §5.2 / §5.3 冲突，以本节为准。**

### 9.1 14 张检查器基线重拍（有意外观改动）

行距这次改动**不是顺手调的**：`docs/30` §4.1 结构表最后一行写的是
「组内 | 上下 8、左右 12；控件行高 28；**行间距 6**」，实现此前是 8，本轮按合同改成 6。
面板区因此整片位移——这是**有意外观改动**，不是渲染回归，但必须按 §5 的规矩量化＋重拍。

重拍清单 **14 张**：`inspector` / `inspector-narrow` / `inspector-shape` / `inspector-ink` /
`inspector-mist` / `inspector-sand` / `inspector-content` / `inspector-multiselect` /
`inspector-error` / `inspector-stepper-edit` / `inspector-weight` / `inspector-align` /
`inspector-radius` / `inspector-ellipse`。
（`inspector-empty` 未列入：空状态没有样式行，行距改动不触及它的画面。）

相对各自旧基线的箱内像素差（`--tol 0`）：

| 基线 | 差异像素 | 占比 | 包围盒读法 |
| --- | --- | --- | --- |
| `inspector` | 43 820 | 2.113% | 只在面板（x ≥ 1506） |
| `inspector-narrow` | 43 847 | 2.115% | 只在面板 |
| `inspector-shape` | 35 150 | 1.695% | 只在面板 |
| `inspector-ink` | 78 488 | 3.785% | 从左边缘起 |
| `inspector-mist` | 77 480 | 3.736% | 从左边缘起 |
| `inspector-sand` | 77 448 | 3.735% | 从左边缘起 |
| `inspector-content` | 78 158 | 3.769% | 从左边缘起 |
| `inspector-multiselect` | 43 316 | 2.089% | 只在面板 |
| `inspector-error` | 37 594 | 1.813% | 只在面板 |
| `inspector-stepper-edit` | 35 241 | 1.700% | 只在面板 |
| `inspector-weight` | 63 770 | 3.075% | 从左边缘起 |
| `inspector-align` | 63 463 | 3.061% | 从左边缘起 |
| `inspector-radius` | 63 591 | 3.067% | 从左边缘起 |
| `inspector-ellipse` | 67 092 | 3.236% | 从左边缘起 |

两个包围盒口径别读混：

- **面板-only 的 6 张**（`inspector` / `-narrow` / `-shape` / `-multiselect` / `-error` /
  `-stepper-edit`）：bbox 全在 x ≥ 1506，即**画布一个像素没动**，漂移全部来自行距 8 → 6。
- **带主题/样式覆盖的 8 张**（`-ink` / `-mist` / `-sand` / `-content` / `-weight` /
  `-align` / `-radius` / `-ellipse`）：bbox 从左边缘（x=19）起——因为 SE-13 改的是
  **画布节点本身**（字重改度量 → 整树重排；对齐改锚点；圆角/椭圆改轮廓），
  这是这批能力**第一次上屏**，不是漂移异常。

重拍后逐个复核：**同一二进制、`--tol 0`，14 张全部零差异。**

### 9.2 全矩阵（`stage-b`，2026-11-02 19:35–19:47 复跑）

- 口径：`scripts/golden-matrix.ps1`，场景表与环境与正式门一致，**同一个二进制**（mtime **19:32:30**）。
- 结果：`stage-b` **178 场 × 2 遍 = 356 次比较，全部零差异**。
- **19:05 那几盏红是并发假红，不是代码回归**：19:20–19:30 那一轮两遍各出现 **10 场互不相同**的失败，
  当时并行会话正在 `rustc` 构建并跑 `mw-shell`（`card-instant-undo` 单场耗时 **16.5 s**），
  CPU/GPU 抢占把 harness 拖到超时；`mm-rel-link-created` / `mm-summary-hover` / `card-autosave` /
  `search-text` / `mm-zoom-wheel` 等单场景复跑**逐个通过**。
- **但这两遍不等于「正式门绿」**，限制照写：
  1. 跑的是**已构建二进制**；正式门脚本要等并行会话落笔后再补跑——源码一动就触发约
     **4 分钟 LTO 重建**，所以本轮没有为了补一份正式门日志去动源码。
  2. 全量测试 **1313 passed / 0 failed / 1 ignored**、`cargo fmt --all -- --check`、门内 `clippy`
     是**分别实测**的，不是一次整跑（见 §9.4）。
  3. `已验证` 仍欠**实机微软拼音 + 系统 DPI**（§9.5）。

### 9.3 `stage-cm`

- 同一个二进制、同样两遍：**73 场 × 2 遍 = 146 次比较，全部零差异**。
- `cm-inspector` / `cm-inspector-pinned` 的漂移是**真的**（检查器组内行距 8 → 6 的合同改动，
  与 §9.1 同源），**已重拍并复核零差异**；漂移只在面板区（x ≥ 1634）。
- `mm-summary-hover` 早前那次「悬停高亮丢失」与 §9.2 同属**并发假红**，单场景复跑通过。
- 正式门同样**待补跑**（§9.2 的三条限制对 `stage-cm` 一样适用）。

### 9.4 测试与格式

- 全量测试：**1313 passed / 0 failed / 1 ignored**（`--test-threads=1`，
  `TEMP`/`TMP` 指向 `target/tmp`）。
- `cargo fmt --all -- --check`：**干净**。

### 9.5 这两件事别混

1. **功能判词**：G-B3-2 第 4 条（检查器能力清单齐全）已由 **4/5 复判为 5/5 通过**
   （2026-11-02，授权代判）——这判的是「四项能力到底有没有做、有没有落盘/上屏」。
2. **机器门**：矩阵层已全绿——2026-11-02 19:35–19:47，同一二进制（mtime 19:32:30）两遍，
   `stage-b` **178/178 场、356/356 次比较**、`stage-cm` **73/73 场、146/146 次比较**全部零差异
   （19:05 那批红已判定为并发假红，见 §9.2）。但**正式门脚本本身仍待补跑**（限制见 §9.2），
   所以既不能写成「正式门绿」，也不能写成「已验证」。两者不互相替代，
   **判词通过不能替代门绿，门绿也不等于实机验收**。

**「门通过 ≠ 已验证」**：实机微软拼音与系统 DPI 仍是欠账，本轮状态照旧记 `已实现`，不记 `已验证`。

