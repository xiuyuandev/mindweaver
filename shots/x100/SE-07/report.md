# SE-07 · 内容组：备注 / 任务 / 标签 / 链接 / 卡片（P 轨 E2 第三包）

> 设计合同：`docs/30` §4.1（内容组默认展开、组内行高 28）、§4.2（文本域 3 行起、超过 8 行可展开；
> 标签纯文字 `12.5/18` + 6px 色点、回车提交、**无 chip 墙**；卡片绑定三态并显示标题 + 来源）、
> §4.5（禁止彩色 chip 墙）、§4.6（`inspector-content` 场景）。
> 执行合同：`docs/31` §5 第 SE-07 行。依赖 SE-05（骨架与 `group()`）、CER-20（保存语义）。

## 1. 开工前审计（对着 `docs/30` §4.1 / §4.2 逐条查代码）

| 合同条目 | 现状 | 结论 |
| --- | --- | --- |
| 内容组**默认展开**（文本内容 / 任务 / 标签 / 链接 / 卡片） | 五组 `default_open` 全是 `false`——内容页打开时是一排收起的标题 | ❌ 与 §4.1 相反 |
| 备注**多行 3 行起、超过 8 行可展开** | `TextEdit::multiline(...).desired_rows(9)` 写死 9 行，永远那么大、也不会「展开」 | ❌ |
| 任务**三态**（未开始 / 进行中 / 完成）+ 可清除 | `note_body` 的「任务」组：三格 `seg` 56×24 + 清除图标 | ✅ |
| 标签**纯文字 12.5/18 + 6px 色点、无 chip 墙** | 在**「标记」页**，且是 `corner_radius(9)` + `panel2` 底的**胶囊 chip**，靠 `close_x` 删除 | ❌ 位置与形态都不符 §4.1 / §4.2 / §4.5 |
| 标签**输入框回车提交** | 单行框 + 「添加」按钮 + Enter 提交（`label_draft`） | ✅（逻辑可复用，随标签一起搬家） |
| 标签上限 | `MAX_LABELS`（5）到顶显示「最多 5 个标签」 | ✅ |
| 链接 **URL 行内编辑** | `link_body`：单行框 + host 预览 + 保存/打开/清除，Enter 提交、Esc 撤销 | ✅ |
| 卡片**三态**（无绑定 → 新建 / 关联已有；已绑定 → 标题 / 解绑） | 已实现，含「关联已有」候选列表（只列未被占用的卡） | ✅ |
| 已绑定**显示卡片标题 + 来源** | 标题 ✅、摘要 ✅、卡片自己的标签 ✅；**「来源」没有** | ❌ 缺一行 |
| `inspector-content` 场景 | 不存在；也没有能把检查器切到内容页的旋钮 | ❌ 本包的交付拍不到 |

**结论**：SE-07 的实质工作是三件——把「默认展开」按 §4.1 改回来；把标签从「标记」页搬到内容页并
从胶囊 chip 改成「6px 色点 + 纯文字」；给备注框做「3 行起、超过 8 行可展开」。卡片补一行「来源」。
其余（任务三态、链接行内编辑、卡片三态、标签上限与回车提交）已实现，本包只做核对与证据。

## 2. 非目标

- 不新增 `Node` / `NoteCard` 字段（`labels` / `task` / `link` / `card_id` 都已经在模型里）。
- 不做标签的批量（多选）编辑——`AddLabel` / `RemoveLabel` 收 `ids`，但内容页维持「单节点」语义
  （多选时提示先选一个），与备注、链接一致。
- 不做作用域切换（SE-08）、不做检查器键盘全路径与对比度测量（SE-09）。
- 不改「图片」组（它是既有能力，`docs/30` §4.1 的组清单里没列它；保持折叠、位置不变）。

## 3. 验收场景

1. **`inspector-content`**（1280×720 @1.5，`MW_INSPECTOR=1 MW_SELECT_DEMO=1 MW_INSPECTOR_TAB=note MW_CONTENT_DEMO=1`）：
   内容页五组**默认全开**；备注框 3 行起；两个标签是「6px 色点 + 纯文字」（无色胶囊）；
   任务三态里有选中；链接显示 host；卡片显示标题 + 来源。
2. **`inspector`**（样式页）：五组默认开合改动后**不应**影响样式页——重拍后确认漂移只在内容页相关像素。
3. **备注展开**：`note_rows()` 纯函数——1 行文本给 3 行、12 行文本给 8 行 + 「展开 · 共 12 行」，
   展开后给 12 行。
4. **标签往返**：`AddLabel` 后节点 `labels` 多一条、画布立即反映；`RemoveLabel` 后回原状；
   两条都在一条撤销内回到原点。

## 4. 预计证据

- 基线：新 `inspector-content`；`inspector` 重拍并量化漂移。
- 单测：`note_rows` 的 3/8/展开三态；标签行几何（6px 点、行高 28、无底无色）；卡片来源三态。
- 两道门绿（stage-b 场景数 +1）；测试数只增不减。

## 5. 数据模型核对（`docs/31` §5 P0 风险行：新增能力有没有字段承载）

| 能力 | 字段 | 结论 |
| --- | --- | --- |
| 节点备注 | `Node.note: Option<String>` | 有 |
| 节点任务三态 | `Node.task: Option<TaskState>` | 有 |
| 节点标签 | `Node.labels: Vec<String>`（`MAX_LABELS = 5`） | 有 |
| 节点链接 URL | `Node.link: Option<String>` | 有 |
| 节点 ↔ 卡片 | `Node.card_id` + `Document.cards` | 有 |
| 卡片「来源」 | `NoteCard.source_doc` / `source_node` | 有（只差界面把它读出来） |

五项在模型里都已存在，**本包不需要任何新字段**，`docs/31` §5 的 P0 风险不触发。

## 6. 结果（实测）

### 6.1 交付项

| 合同条目 | 做完的样子 | 证据 |
| --- | --- | --- |
| §4.1 内容组默认展开 | 文本内容 / 任务 / 标签 / 链接 / 卡片五组 `default_open` 全部改真；分组标题右侧各带一句活摘要（空/已写、未开始·进行中·完成、`n 个`/无、host、卡片标题/未绑定） | `content_groups_are_open_by_default`；`inspector-content` 基线 |
| §4.2 备注 3 行起、超过 8 行可展开 | `note_rows(text, expanded)`：少于 3 行给 3 行，多于 8 行封顶 8 行并出「展开 · 共 N 行」按钮，展开后按实际行数给足；换节点 / 清空选中时自动收回 | `note_rows_starts_at_three_and_caps_at_eight` |
| §4.2 文本域**无白底无框**、靠 `hairline` 底线 | 备注框原来套着 `text_box_frame`（填色 + 1px 描边 + 6 圆角），等于把「纸上的一段字」画成输入框；改成 `Frame::NONE` + 一条 `hline`：备注区 260×66 css（3 行 × 18 + 上下内距 12），只剩底线。实测基线：面板内 `(248,249,250)` 填色像素 **40451 → 10023**（剩下的是卡片/链接那几块），备注区一条 **394 px** 的 hairline（y=541），左缘不再有竖框 | `inspector-content` 基线 + 像素统计（本次实测） |
| §4.2 标签纯文字 12.5/18 + 6px 色点 | 从「标记」页搬到内容页；胶囊底 + `corner_radius(9)` 删掉，只剩一个 `circle_filled(r=3.0)` 和一行 `12.5` 文字，右侧 14px 关闭叉；回车/「添加」提交、上限 5 | `labels_are_text_with_a_six_px_dot`（数半径恰为 3.0 的圆、且填色等于 `tag_color("渲染")`/`tag_color("性能")`）；基线里两颗色点是 9×9 设备像素（6 css） |
| §4.2 卡片显示标题 + 来源 | 新增来源行：`独立卡片` / `本文档 · 挂在主题上`（概念图写「概念」）/ `本文档 · 独立卡片` / `来自其它文档` | `card_source_says_where_the_card_comes_from` |
| §4.6 `inspector-content` 场景 | 新基线 + 两个验收旋钮 `MW_INSPECTOR_TAB=note`、`MW_CONTENT_DEMO=1`（五项内容全走 `Command` 灌进演示文档，与用户手点同一条撤销栈） | `shots/golden/inspector-content/shell-ppp1.50.png` |

### 6.2 途中撞见并修掉的两处回归（本包的真正价值）

**① 面板最小宽被内容顶大（288 → 316，且每帧 +2）。**
「添加」按钮用自然宽、输入框用 `available - 44`，而 `TextEdit::desired_width` **不含控件自身的左右内距**（8+8），
整行比可用宽多 16：`SidePanel` 的最小宽被内容抬到 316.65，而且每帧再长 2（316.65 → 326.65）。
用 `MW_INSPECTOR_GROUPS` 逐组关掉定位到「标签」这一组，改法是把宽度先算死再排：
`label_field_width(avail_after_gap)` 纯函数 + `ui.add_sized([44, 22])` + 内距常量由 `LABEL_MARGIN_X` 推出。
改后轨迹 `inspector w=288.00`，内容页与样式页同宽。

**② 标签行高 30（合同要 28，行距因此 38）。**
`ui.horizontal` 按全局 `spacing.interact_size.y` 要高度，而这个值在本项目是 `design::control::INPUT = 30`，
所以行高 30、两行之间再叠 8 的行距。金样里两颗色点相距 38 css px，肉眼上「标签一行怎么这么松」就是这么来的。
改法：新增 `content_row()`，用 `allocate_ui_with_layout` 的显式 `desired_size` 钉死 28；色点间距 38 → 36（28 + 8）。
`content_rows_are_twenty_eight_high` 同时量了 `ui.horizontal` 仍是 30——陷阱还在，这条测得出东西。

两处都由「先量再改」发现：机器门是绿的，问题只在像素与轨迹里。

### 6.3 测试

新增 6 条（`panels::inspector` 现 31 条全绿）：

- `note_rows_starts_at_three_and_caps_at_eight` — 1 行给 3、12 行给 8、展开给 12。
- `card_source_says_where_the_card_comes_from` — 四态文案（含概念图写「概念」）。
- `content_groups_are_open_by_default` — 画一帧，断言备注框、任务三态、标签组的内容真的出现在画面里。
- `labels_are_text_with_a_six_px_dot` — 递归收集 `Shape::Circle`，半径必须恰为 3.0，且填色是标签色。
- `the_label_row_fits_the_panel` — 240/288/360 × 四种行距下「输入框 + 按钮 + **真的**内距 + 间距」都装得进这一行。
  **这条先被证明会红**：把 `LABEL_PAD_X` 改回 0，测试当场失败。
  （第一版拿 `ui.set_max_width(288)` 量面板宽，被证明恒真——`set_max_width` 会夹住 `min_rect`，已删。）
- `content_rows_are_twenty_eight_high` — `content_row` 给 28，`ui.horizontal` 给 30。

### 6.4 机器判据

- 全量：`cargo test --workspace -- --test-threads=1`（TEMP 重定向）**954 passed / 0 failed / 1 ignored**（948 → 954，+6）；fmt 干净；clippy 指纹与基线比**新增 0**（本轮一度新增 2 条 `cloned_ref_to_slice_refs`，改 `std::slice::from_ref` 后归零）。
- stage-b 门：**PASS，115 场 / 230 次比较（两遍各 115），2026-10-30 03:27 跑完**，`--tol 0`；`inspector-content` 首次进门，`inspector` 等七张老基线零漂移；日志 `target/x100/stage-b-gate.log`。

### 6.5 漂移

- `inspector-content`：新基线（1920×1080 @ ppp 1.5，warmup 8）。重拍两次：一次在修完面板宽度回归后，一次在补上「文本域无框」之后；两次都双跑零差异。
- `inspector` / `inspector-narrow` / `inspector-empty` / `inspector-shape` / `inspector-ink` / `inspector-mist` / `inspector-sand`：**0 px**（门两遍全过即证）。
- 画布侧：stage-b 全矩阵 115 场零差异，说明本轮所有改动都收在检查器面板内。

## 7. 未做与风险

| 项 | 状态 | 说明 |
| --- | --- | --- |
| 实机微软拼音在标签框/备注框里输入 | 未核 | 本包只做了离屏等效（金样无 IME 事件）。不记 `已验证`，与 M4 同一条债 |
| 真手拖面板 240↔360 时标签行是否始终装得下 | 只做了算术 | `the_label_row_fits_the_panel` 覆盖 240/288/360 三档，但没有真实拖动脚本；窄档另有一张 `inspector-narrow` 基线 |
| 备注「展开 / 收起」这一路 | 单测覆盖，未实拍 | 演示备注只有 3 行，进不了 >8 行分支；要拍得再造一个长备注场景，留作下一轮候选 |
| 卡片「关联已有」候选列表 | 沿用旧形态 | §4.2 只要求「三态 + 标题 + 来源」，候选列表的重排不在本包 |
| 任务三态、链接按钮行的行高 | 仍是旧几何（24 / 自然高） | 本轮只把**标签行**钉到 28；组内其它行要不要统一，得先跟 §4.1「组内控件行高 28」对齐后再一起改，留作下一轮 |
| §4.1「组内行间距 6」 | 仍是全局 `item_spacing.y = 8` | 合同写的是 6，面板里现在沿用全局 8（每行之间多 2px）。这属于 SE-05 的组内几何，动它会让所有检查器基线整体下移，**不在本轮夹带**，记作下一轮候选 |
| 图片组 | 未动 | `docs/30` §4.1 的组清单里没有它；保持折叠、位置不变 |
| 标签批量编辑（多选） | 非目标 | 内容页维持单节点语义，多选时提示「标签只能对单个主题编辑」 |
