# PLAN §4 执行记录归档（2026-10-19 – 2026-10-28）

> 来源：`PLAN.md` §4「执行记录」，原文逐字搬运。
> 用途：逐轮改动的漂移量、基线与命令记录。当前状态看 [`docs/STATUS.md`](../../STATUS.md)。

---

> 时间戳说明：本节早期若干条的时间写得跑到了实际时刻前面（出现过 `05:2x` 这类没到的钟点），
> 是当时误记。从下面第一条起，时间一律以 `Get-Date` 复核为准。


- 2026-11-02 19:0x · **第 68 轮 · SE-13 复审修复 + X21（相机 / 缩放读数 / 滚轮锚点）+ X21b（纹理单源 / 导出同源）**。
  （编号接第 67 轮；第 65–67 轮记在 `execution-log-2026-11.md`，本文只接最新一条。）
  **SE-13 复审四处修复**：① `mw-core` 的 `apply_concept_defaults` 换 `cm-*` 主题时把主概念改成 `bold = true`
  却没搬 `weight`，渲染/导出只认 `weight` ⇒「中心概念」掉回 400——改 `bold` 时同步 `style.weight`
  （无显式覆盖才动），新单测 `concept_themes_move_the_primary_concept_to_the_semibold_weight`；
  ② PDF 伪粗判据还是 `t.bold`，显式 `weight=400` 的层级节点画布常规、PDF 却多一道描边——判据改
  `weight >= WEIGHT_SEMIBOLD`（`bold` 经 600 别名仍是 600，导出零漂移）；③ 编辑环自己又写了一份
  `match shape`，`radius=24` 的节点画成圆、环还是直角——环半径改走 `mw_render::corner_style`，与合成器同出口；
  ④ 行内编辑器按 F2 后字族看 `geom.bold`、锚点恒左内距，设过 500/600 或右对齐的节点一进编辑就跳字重 + 跳位置——
  字族按 `style.weight` 选（≥600 走粗体族）、`TextEdit::horizontal_align` 跟 `style.align`。
  复审第 5 条（右锚点用 `text_x` 多算一个图标宽、图标按节点字重画会超出固定槽）一并修掉。
  **检查器组内行距 8 → 6**（合同值），14 张检查器基线按新行距重拍，`--tol 0` 实测漂移 **1.695–3.785%**，
  逐张核对：漂移**只落在检查器面板区**（x ≥ 1506），画布区零漂移。
  **X21 相机 / 缩放读数 / 滚轮锚点**：缩放收成一条路——一个入口 `zoom_step_at` + 一个倍率常量 `ZOOM_STEP`
  + 一个读数函数 `format_zoom` + 一条轨迹 `zoom_anchor` + 编辑期锁 `zoom_blocked`；`format_zoom` 先四舍五入
  再判档（`pct ≥ 10` 整数档、`< 10` 一位小数、`NaN/±∞` 印 `—`，`0.8545001` 必须读 85% 不许二次舍入）；
  **锚点语义不变**（缩放条/命令面板 = 视口中心，键盘/滚轮/pinch = 画布内指针，浮层不算画布指针），
  所以**零外观改动**、既有基线一张不重拍。`docs/06` §5 那条一直没实现的「Shift 切换轴向」落成纯函数
  `wheel_pan(scroll, shift)`（纵向优先：`y ≠ 0` 时纵向搬横轴并清零 `y`）。新金样 `mm-zoom-wheel`
  （`shots/golden/inputs/mm-zoom-wheel.txt` + `shots/golden/mm-zoom-wheel/shell-ppp1.00.png`，
  登记为 `stage-b` 第 178 场，warmup 30），**首拍即 `--tol 0` 零差异**，轨迹 `zoom_anchor src=wheel`
  反复出现且 `drift=(0.00,0.00)`。
  **X21b 纹理单源 / 导出同源**：点阵令牌收进 `Theme::canvas_dots()`（间距 28 / 半径 1.1 唯一定义在
  `mw_core::theme`，`mw-render` 里那两份常量已删），画布合成、SVG 的 `<pattern id="zdots">` 与 PDF 的
  `/PatternType 1` 平铺图案读同一份；**「PDF 一个点都不画」的缺口关闭**；Lead 驳回的外观两项已回退
  （`snow` 底色仍 `#FCFBF8`、三套亮色主题仍不铺点阵、格距仍 28）⇒ **零外观改动**，无基线重拍；
  所谓「SVG 点阵错半格 bug」**经核实不存在**（`<pattern>` 默认原点 + `cx=14` 本来就在格心，
  改了反而错半格），已用 AC-9 断言把相位钉住。
  证据：`stage-b` 正式门 20:15 **`MACHINE GATE PASS`**（`target/x100/stage-b-gate.log`）：
  X11–X19 证据包 PASS、PowerShell 语法 PASS、`cargo fmt` PASS、`mw-shell` clippy 新增 0、
  全量串行测试 **1328 passed / 0 failed / 0 ignored**、golden 178 场 × 2 遍 = **356 次比较零差异**；
  `stage-b` 登记 **178 场 / 356 次比较**、`stage-cm` 登记 **73 场 / 146 次比较**；
  golden 基线目录 **254**、输入脚本 **152**。
  **门的另一半（不许写成「两道门全绿」）**：`stage-cm` 正式门 20:19 **`MACHINE GATE FAIL`**
  （`target/x100/stage-cm-gate.log`，20:15:14–20:19:32）：门内 `.zmind` 往返 PASS、全量测试 PASS
  （1335/0）、golden 73 场 × 2 遍 = **146 次比较零差异**；**红只有 `fmt` 与 `clippy`，而且全落在
  并行会话正在改的文件上**——`crates/mw-core/src/geom.rs:168`、`crates/mw-shell/src/canvas/tests.rs:3453`
  未格式化，`crates/mw-shell/src/canvas/canvas_methods_3.rs:588:30` 一条新增 clippy 警告。
  所以口径是「**`stage-b` 门绿了；`stage-cm` 门没绿，红在别人在飞的代码上**」，等对方收尾后重跑
  `x100-stage-cm-gate.ps1`；门结论以 `docs/STATUS.md` §1 为准。
  **19:05 那批红灯的归因（保留，不许反过来当成门结论）**：那次矩阵 `stage-b` 177/178、
  `stage-cm` 70/73 是**并发干扰下的假红**——并行会话正在 `rustc` 构建并跑 `mw-shell`（单场 16.5s）
  抢 CPU/GPU、harness 超时；`mm-rel-link-created` / `mm-summary-hover` / `card-autosave` /
  `search-text` / `mm-zoom-wheel` 单场景复跑逐个通过，19:20–19:30 那次两遍各 10 场失败还**互不相同**；
  19:35–19:47 冻结同一二进制（mtime 19:32:30）跑矩阵两遍 = 356/356 + 146/146 零差异，先把红定性为抢占。
  真实漂移只有 `cm-inspector` / `cm-inspector-pinned`（检查器组内行距 8→6，只漂面板区），
  已重拍，矩阵与门内均零差异。
  **工程事实**：`mm-zoom-wheel` 首拍时误在**没有重定向 `APPDATA` / `LOCALAPPDATA`** 的环境里拍，
  把「自动保存失败」红条一起拍进了基线；正确环境与正式门一致（每场独立 `target/ci-appdata/<场景名>[-local]`，
  `scripts/golden-matrix.ps1` 的 `Invoke-Scene` 逐场设置并还原）。手工拍任何 golden 之前，先把这两个变量重定向。
- 2026-11-02 16:1x · **SE-13 · 检查器四项样式能力（椭圆 / 圆角 / 字重三档 / 对齐）**。G-B3-2 第 4 条
  判词缺的四项全部补上：`StyleOverride` 新增三个可选键 `weight` / `align` / `radius`
  （都带 `skip_serializing_if`，缺键即老行为，不升 `FORMAT_VERSION`——决定记在 `docs/32` §15），
  `shape` 开放 `ellipse` / `underline` 两个既有枚举值的映射；`TextMeasurer::measure` 的
  `bold: bool` 换成 `weight: u16`（度量与绘制走同一个字重，盒宽跟着字面走），`BoxKey` 加
  `weight` 字段——漏了它就会出现「字重换了、盒宽还是旧的」。
  **`bold: true` 解析成 600 而不是 700**：TYPO-01 定的粗体字面是 Segoe UI Semibold 600，
  `dwrite::WEIGHT_BOLD` 一直是 600；取 700 会让所有旧文档的粗体换成另一个字面，
  每一张既有基线都漂——「缺键即老行为」优先。
  检查器侧：形状行四段改成 **圆角/直角/胶囊/椭圆**，下划线搬进 B / I 同排当 **U** 开关
  （240 面板放不下五个 12.5px 汉字格：每格 42px < 三字 37.5px + 边距），新增「字重」三段
  `400 / 500 / 600`（B 降为它的快捷方式：开 600 / 关 400，不再写 `bold`）与「圆角」24×72 stepper
  （0–24、步 2、只对有角的形状出现），「对齐」三段「左 / 中 / 右」。
  **基线漂移核算**（`--tol 0`，全部双跑零差异）：15 场涉及检查器——11 场重拍
  （`inspector` / `-narrow` / `-empty` / `-shape` / `-ink` / `-mist` / `-sand` / `-content` /
  `-multiselect` / `-error` / `-stepper-edit`），4 场新增（`inspector-weight` / `-align` /
  `-radius` / `-ellipse`，夹具 `MW_STYLE_DEMO=<k=v,…>` 经 `Command::SetStyle` 喂覆盖）。
  新场景相对 `inspector-shape` 的差异：字重 68 918 px（盒宽变了，整树重排）、对齐 35 135、
  圆角 37 238、椭圆 54 140；**控制变量**：没喂覆盖的 `inspector` 与 `-shape` 只差面板区
  （x ≥ 1506）——画布零漂移，说明既有文字渲染逐像素未变。
  单测 8 条（级联与钳制 3、盒缓存键 1、圆角映射 1、SVG 同源 1、序列化可选 1、三段不顶宽 1）；
  全量 **1287 passed / 0 failed / 1 ignored**；fmt 净；clippy 新增 0。证据 `shots/x100/SE-13/`。
  **诚实标注**：500/600 是否真落到雅黑 Semibold 需实机目检；椭圆 + 多行文字的四角压弧问题
  没有搬概念图的 1.22×1.16 外扩，如实记在报告 §6。
- 2026-11-01 12:1x · **第 64 轮 · M31 「设为根主题」**。`docs/02` §2 与账本 M25 挂着的
  「未做：设为根主题（P2）」落地。`ChangeSet` 早有 `before_root` / `after_root` 快照
  （概念图删主概念重挂根用的），所以换根做成**一条命令、一次撤销**：
  `Command::SetRoot { id }`——新根摘出原父级（原父级的概要跨度同步收口）、
  旧根剩下的整棵树追加到新根孩子末尾、`root_id` 换新、自由主题提根交出 `pos`；
  概念图 / 已是根 / 未知 id 三种拒绝路径各回各的错。壳层入口：画布节点右键菜单
  「设为根主题」（排在「分离为自由主题」之后）与大纲行菜单（单选、非根行），
  落地后 `fit_view()` 一次——新根按布局落在视口外最容易被当成「点了没反应」（M6b 同类）。
  单测 4 条（结构+撤销/重做、从孙节点提根祖先链留旧根那侧、自由坐标、拒绝路径）；
  新基线 `mm-set-root`（warmup 20，菜单 `Home + 11×↓`，sha256-12 `B95E11237D8F`）进 stage-b。
  顺带：M31 新增节点菜单行会让 `menu` / `menu-narrow` / `outline-row-menu` 三张基线整页重排，
  已按现状重拍。证据 `shots/x100/M31/`。**未做**：快捷键；确认框（不必要，一步可撤销）；
  旧根插入位置固定末尾；实机键鼠节奏。
- 2026-11-01 11:3x · **第 62 轮 · M7b 框选的 Alt 减选与自由套索**。账本 M7 的「Alt 减选与套索没拍」
  先读实现再拍——读出来的语义与直觉不同：空白处 **Alt+拖 = 自由套索**（`DragKind::Lasso`），
  而 `subtract` 与 `Lasso` 都读按下时的 `alt`，所以**套索在真实手势里恒为减选、不能建立新选择**；
  矩形框选带 Alt 在指针上不可达；`subtract` 优先于 `additive`；圈空不清空选择。
  抽纯函数 `canvas::apply_lasso`（**行为零变化**），松手分支加轨迹
  `marquee picked=N selected=M path=lasso`，单测 +2（`lasso_pick_maps_the_polygon_through_the_camera`、
  `lasso_subtract_only_removes_looped_nodes`）。两张金样：`mm-lasso`（停在松手前，锁多边形，
  `870979712E6C`）、`mm-marquee-alt`（先框 3 颗再套索减 1 颗，末帧 2 个选中环，`209ADC44E298`），
  `--tol 0` 双跑零差异并进 stage-b；空组圈中两颗的结算轨迹另存
  `shots/x100/M7b/trace-lasso-commit.txt`。证据 `shots/x100/M7b/`。
  **未做**：套索不能建选 / 无实时预览 / `docs/06` §5「Alt+拖拽=记录布局偏移」与实现相反——
  三者都要先改合同，记在报告 §6。
- 2026-11-01 11:1x · **第 63 轮 · C21 三车道以上平行命题的等距避让**。C15 的车道偏移是
  常量 `min(len/6,24)` 法向 + 切线常量 24，短语加宽换不来间距；先写会红的场景级单测量得：
  3 车道竖边相邻短语净空 **4.00**（目标 24）、水平短边 **0.00（真重叠）**、短语 36→108 宽
  净空仍是 4.00。新增纯函数 `concept_layout::proposition_label_offsets(label_widths, tangent)`：
  ≥3 条按每条短语盒在切线上的投影半宽 `(w/2)|ux|+(h/2)|uy|` 累加（+24），镜像侧取大、
  中车道为 0；1/2 条走旧公式**逐位一致**（`cm-parallel-select-a/b`、`cm-crosslink` 零漂移）。
  `scene.rs` 只做「收车道 → 量宽 → 查表」。壳层旋钮 `MW_CONCEPT_PARALLEL` 从布尔改成
  **数量**（值是再补几条，`=1` 与 C15 完全一致，`=2` 凑出 3 条车道），新基线 `cm-parallel-3`
  （`--tol 0` 双跑零差异，sha256-12 `6BA2BF3A32FE`）进 stage-cm。单测 +10（`mw-core` lib 306 → **316**）。
  证据 `shots/x100/C21/`。**未做**：法向间距仍可能 < 20（标签可能伸到端点外）；
  法向也参与分配会改车道曲线，需单独批准；实机高 DPI 未核。
- 2026-11-01 10:5x · **第 61 轮 · SE-12 起始页内容模板**。`docs/02` §1 挂了半个月的
  「4 个内容模板待补」落地：`TEMPLATES` 6 → 10（结构 1–6 不动，新增 空白/读书笔记/会议记录/产品规划，
  全部 `Shape::Tree` + `mindmap`，图标复用现有矢量），模板区加两行**非控件**组标题
  「结构模板 / 内容模板」（`TPL_GROUP_H=18`、下距 8、组距 14），网格仍是同一张
  （3/2/1 列、同尺寸同间距；不选 4 列是因为会把已验收的六张卡从 285 压到 211）。
  数字键直选扩到 `1–9/0`（`start::template_hotkey(idx)` 纯函数，0 = 第 10 张），
  首页数字键循环改按 `TEMPLATES::len()` 生成。单测 +4（`mw-shell --bins` 631 → **635**）。
  11 张 `start*` 基线整页重拍后同一构建二次 `--tol 0` **零差异**；
  试过的新场景 `start-templates-content` 因离屏 harness 无视 `MW_WINDOW_SIZE`
  （固定 1620×1080/1920×1200 物理画布）与 `start` **逐字节相同**，已删。
  证据 `shots/x100/SE-12/`。**未做**：实机 125%/150% 字形、空白模板空根的视觉未人工核对。
- 2026-11-01 10:2x · **第 60 轮 · M6b 结构移动的边界反馈**。账本 M6 的两条粗糙：①「跨父级多选
  `Alt+↓` 什么都不动」被新单测 `batch_move_cross_parent_moves_each_within_its_own_parent`
  **证伪**（`batch_structure_command` 本来就逐颗按各自父级换位）；② 真正缺的是边界上的**提示**。
  新增纯函数 `canvas::move_block_reason(doc, id, delta) → Root|First|Last|None`，
  `move_selected` 在 `move_command` 为 `None` / `move_step_many` 返回 `false` 时给 `notice`
  （根主题不能移动 / 已经是第一个 / 已经是最后一个 / 选中的主题已经在边界），
  轨迹 `structure_move_blocked delta=… selected=… reason=…`。新基线 `mm-move-blocked`
  （点第一个子主题 → `Alt+↑`，与不发键的对照帧只差底部提示条 10 760 px，bbox `x[16..375] y[1028..1057]`）。
  门内抓到 `mm-move-multi` 的真相：旧脚本实际只框到根主题，锁的是「根主题按 `Alt+↓` 静默不动」，
  已改为框选两颗可动子主题并重拍。单测 +2（**1124 → 1126**）。**门**：10:31 那次 stage-b 跑到
  pass 2 的 `start` 时撞上本轮并行的 SE-12 改稿（编译取了半成品），整次作废；
  第 60/61 两轮与 M7b/C21/M31 的整门复跑结果见 `docs/STATUS.md` §1。证据 `shots/x100/M6b/`。
- 2026-11-01 05:20 · **第 59 轮 · M28b 撤销上限调低的提示**。M28 记着「降上限后旧记录
  不可恢复且无二次确认」。本轮不弹模态，在设置中心滑轨下方就地加 **28px 警告行**：
  纯函数 `settings::undo_limit_dropped(old, new, depth)`（`new < old 且 depth > new`
  才算真正会丢，否则 0），文案写明步数；`dropped == 0` 时不占位。轨迹
  `settings warning dropped=27 depth=127 limit=100`（点根 + 110 次 Enter 把深度堆到 127，
  草稿上限 100），图中警告色像素 377、bbox `(577,546)–(774,556)`。
  **金样踩到并修掉两个坑**：① `MW_SETTINGS_DEMO` 开局就开对话框，后续 110 次 Enter
  全被它吃掉（默认动作还把上限真写成 100）→ 新增帧控 `MW_SETTINGS_AFTER=<n>`，
  第 240 帧（脚本 234 帧之后）再弹；② 对话框里「笔记库当前路径」取自 APPDATA，
  换个 appdata 目录基线就漂 47 px → 路径行改显示**草稿**的 `s.notes_root()`
  （改路径即时预览，也更合理），并加旋钮 `MW_SETTINGS_NOTES_DIR` 固定成 `C:/织思笔记`；
  两个不同 appdata 目录各跑一遍 `--tol 0` 全过。单测六档；测试
  **1124 passed / 0 failed / 1 ignored**；stage-b **145/290**（`04:57:57–05:08:54`）
  两遍全过；stage-cm **64/128** 首跑 pass 1 在 `cm-submenu-bottom` 差 **2 px**
  （max 7，单跑 5/5 全过、pass 2 全过），整门重跑后 `05:14:45–05:17:55` 两遍全过；
  基线目录 211 → **212**、输入脚本 139 → **140**。证据 `shots/x100/M28b/`。
- 2026-11-01 04:24 · **第 58 轮 · M30b 缩略卡双击定位节点**。M30 只做了「拖视口框」，
  而 `overview.rs` 头部注释早写明「双击缩略卡定位某个节点应走 `reveal_center`」。
  新增 `mw_ui::overview::node_at(scene, world, pad)`（盒内容差命中，多颗取盒中心最近），
  壳层 `minimap_panel` 处理 `double_clicked`：命中 → `select_node` + `reveal_id`，
  轨迹 `minimap locate id=… hit=1`；空白 → `hit=0`、不选中、相机按普通点击搬视口框。
  容差 = 卡上 4 css px / `map.scale`。新基线 `mm-minimap-locate`：双击卡片中心命中根
  「织思」，对 `mm-minimap` 差 **274 314 px**（bbox `(14,58)–(1788,1194)`，选中环 + 相机居中），
  双跑 `--tol 0` 零差异。**门内插曲**：首跑 pass 1 在 `cm-default`（不走本轮路径）差
  **1 px**（max 82、bbox `(1219,392)`），pass 2 与单跑三连全过，判定 GPU AA 偶发，
  整门重跑后 **144/288 两遍全过**（`04:12:16–04:19:32`）。单测
  `node_at_hits_the_box_and_honours_the_pad`；测试 **1123 passed / 0 failed / 1 ignored**；
  stage-cm **64/128**（`04:21` 起顺序复跑）两遍全过；基线目录 210 → **211**、
  输入脚本 138 → **139**。证据 `shots/x100/M30b/`。
- 2026-11-01 03:52 · **第 57 轮 · M29b 搜索脚注开关的窄窗布局**。M29 的 `narrow` 分支
  （「精确文本」→「文本」）从未实拍，40/40/62 与右让 74 都是行内常量、没有单测。本轮抽出
  `mode_toggle(footer, narrow)` 纯函数 + 6 个常量（宽 62+40 / 窄 40+40、右让 74 给 Esc、
  左留 64 给提示），绘制改走纯函数，并补轨迹 `search_footer width=… narrow=… toggle=[l,r]`
  （布局变化才记一笔，不每帧刷）。新基线 `search-narrow`：640×700 + 检查器 288 → 中央 383、
  卡片 **359**（<420），轨迹实测 `width=359 narrow=1 toggle=[218,298]`；**harness 的 `--ppp 1.0`
  会让逻辑宽=物理宽（卡片仍 560、`narrow=0`），必须 `--ppp 1.5` 才拍得到**，基线为
  `shell-ppp1.50.png`（960×1050）。宽窗同轨迹 `width=560 narrow=0 toggle=[434,536]`，
  八张既有搜索基线零漂移。单测 `mode_toggle_fits_the_footer_at_every_width` 覆盖
  240/326/420/560 四档（最窄 240 时左界 86 ≥ 64）。测试 **1122 passed / 0 failed / 1 ignored**；
  stage-b **143/286**（`03:42:14–03:49:31`）、stage-cm **64/128** 顺序复跑两遍全过；
  基线目录 209 → **210**、输入脚本 137 → **138**。证据 `shots/x100/M29b/`。
- 2026-11-01 03:31 · **第 56 轮 · C20 概念图大纲行菜单（`⋯` 与行右键）**。`docs/30` §5.6 要求
  概念图「像编辑思维导图一样用大纲改结构」，键盘路径（Tab / Enter / Shift+Delete / Shift+F2）
  在 CM-09 已有，但 M25 把概念图行菜单**有意跳过**（`outline_row_menu_skipped reason=concept_map`），
  可发现入口为零。现在概念图大纲行也有同一枚 16px `⋯`（24px 动作道沿用 M25 的尺）与行右键菜单：
  新建概念并连线（Tab）/ 新建概念（Enter）/ 编辑连接短语（Shift+F2）/ 断开进入的边（Shift+Delete）/
  重命名概念（F2）/ 复制（Ctrl+C）/ 删除概念（Delete）；多选只给「复制 / 删除概念」。
  全部复用 `OA::LinkConcept` / `NewConcept` / `EditVia` / `UnlinkConcept`，无新命令；
  无进来边的行禁用改短语与断边（`Outline::row_edge` 判定）。
  新基线 `cm-outline-menu` / `cm-outline-menu-cancel`：菜单帧对无菜单对照 **25 848 px**
  （bbox `(561,153)–(1358,437)`），取消帧 3 284 px（只是选中行焦点/底色，轨迹没有
  `outline_row_action`）；`cm-outline-via` / `-via-clear` / `-unlink` 因选中行出现 `⋯`
  各差 **36 px**、bbox 同为 `x[1336..1351] y[166..169]`，三张有意重拍。
  动作等价轨迹：右键 → `outline_row_menu … concept=1 entries=9` → `menu_nav` ×3 →
  `outline_row_action kind=EditVia`。测试 **1121 passed / 0 failed / 1 ignored**（23 套件）；
  stage-cm **64/128**（`03:19:30–03:22:45`）、stage-b **142/284**（`03:23:17–03:30:32`）两遍全过；
  基线目录 207 → **209**、输入脚本 135 → **137**。证据 `shots/x100/C20/`。
- 2026-11-01 02:50 · **第 55 轮 · M30 画布缩略卡（小地图）接回画布**。`docs/02` §4 P1 / `docs/06` §4 的
  「小地图」在视觉 v2 重写时整块丢失：`mw_ui::overview` 纯几何层（608 行、20 余条单测）与
  `canvas_chrome::minimap_layout` / `minimap_wheel_factor` 都在，**没有任何绘制/交互入口**；
  `Shell` 的 `show_minimap` / `minimap_collapsed` 两个字段上一会话只加了一半（构造缺字段，
  工作树 `E0063` 编译不过）。本轮接回：右下 190×130、缩放条上方 8px；节点代理块按分支色、
  关系线 0.8px、视口框淡强调底 + 1px 强调边；卡内点击/拖动走 `center_from_thumb` 钳制后
  `pan_to`，悬停滚轮一档缩放（`zoom_around_world`）；`–` 折叠、34×26 角标点击展开；
  命令面板与顶栏「视图」菜单开关，状态落 `window.json`（出厂关，既有 140 场零漂移）。
  折叠命中区 18 → **24**（`docs/13` §4.19 的 17×17 旧账），视觉图标仍 12px。
  **门内抓到的两处附带修复**：① `zoom_around_world` 的锚点公式把 `(at − center)/k` 写成
  `× k`，缩放时锚点反向漂（`zooming_around_a_world_point_pins_it_on_screen` 咬住）；
  ② `tests/fixtures/x100/.autosave/` 残留一份比正本新的自动保存，`mm-cut-confirm` /
  `mm-cut-cancel` 一开夹具就弹「恢复点」框（3.012% / 0.924% 红）——删残留、金样在 harness 下
  不再读文档旁恢复点、`.gitignore` 补 `*.zmind.lock`，两场复跑 `--tol 0` 全过。
  新基线 `mm-minimap`（对不开缩略卡的对照帧 27 080 px，bbox `(1710,988)–(1907,1125)`）与
  `mm-minimap-collapsed`（1 184 px，bbox `(1866,1092)–(1907,1125)`），互差 26 482 px；
  两张各双跑 `--tol 0` 0 差异。测试 **1120 passed / 0 failed / 1 ignored**（23 套件）；
  stage-b **142/284**（`02:33:26–02:40:49`）、stage-cm **62/124**（`02:41:09–02:44:34`）两遍全过，
  基线目录 205 → **207**、输入脚本仍 135。证据 `shots/x100/M30/`。
- 2026-11-01 00:25 · **第 54 轮 · C19 概念图检查器属性（概念元信息、钉住开关与局部整理入口）**。
  `docs/30` §4.2 / §5.5 规定「概念图与思维导图共用同一检查器，但内容随对象类型切换；提供局部排布与固定控制」。
  此前概念图在检查器仍残留树语义（对象行显示中心主题/一级主题、样式页带「含子树」作用域），且缺少钉住开关。
  1. 模型与检查器元信息：`is_concept_pinned(&self, id)` 与 `concept_proposition_count(&self, id)`（进出命题总计，自环不双计）。
     单选概念识别为「概念」，已钉住前置「已钉住」，追加「N 条命题」、「已挂卡片」、「N 个标签」，屏蔽子主题计数；
     多选概念识别为「N 个概念」，展示「全部已钉住」或「M 个已钉住」。
  2. 样式页作用域隔离：概念图隐藏分组标题右侧「本节点 / 含子树」切换，屏蔽「文字样式应用到子树」。
  3. 概念排布分组与钉住开关：新增「概念排布」分组（键 `concept/layout`，默认开，活摘要），
     内嵌 28px `switch_row`「钉住位置」轨道开关与说明（单选 `SetConceptPinned`，多选 `Batch` 一次撤销）；
     组内提供「整理选中概念」/「整理全部布局」快捷操作按钮。
  4. 证据与门：5 条单测（core 1 条 + shell 4 条），测试 1109 → **1114 passed / 0 failed / 1 ignored**；
     修复 `document.rs` line 1362 clippy 告警，基线 36 → **35** 条；
     新基线 `cm-inspector`（`DDEDFEFA1445`）与 `cm-inspector-pinned`（`B19304716E8F`），
     两图差 **4 659 px**（画布钉住徽标 1,632 px、检查器元信息/摘要/开关 3,027 px）；
     stage-cm 登记 62 场 / 124 次比较两遍全过，stage-b 登记 140 场 / 280 次比较两遍全过；
     基线目录 203 → **205**、输入脚本仍 135。证据 `shots/x100/C19/`。
  **未做**：钉住状态不进 `.zmind` 持久化（内存交互态）；实机高 DPI/手感未核。

- 2026-10-31 23:25 · **第 53 轮 · C18 概念图「钉住」概念与「只整理选中」局部布局**。
  `docs/30` §5.5 把整理布局定义为「全局或局部整理，允许用户钉住某些概念位置」。
  此前 `arrange_concepts` 只有全局整张整理，所有概念强制覆盖为 DAG 自动位置，
  用户辛苦拖出的空间摆盘在整理后全被冲散。
  1. 模型层：新增 `Command::SetConceptPinned { id: String, pinned: bool }`，
     只在 `concept_map` 模式下允许，整步一次撤销收回，树节点拒绝；
     纯函数 `concept_layout::arrange_scoped(input, fixed)` 允许传入固定掩码，
     固定节点保持 `current[i]` 严格不动，但仍参与其他可移动概念的拓扑分层与重心吸附计算。
  2. 壳层交互：单选概念右键菜单补「钉住位置 / 取消钉住位置」；
     多选右键补「整理选中概念」、「整理全部布局」、「钉住/取消钉住位置」；
     全局整理时保护钉住节点，局部整理时未选中及已钉住节点统一保护；
     整理完毕弹 toast 提示整理概念数量并提示可 Ctrl+Z 复原。
  3. 视觉反馈：在覆盖层（不改 `mw_render::Frame` ABI）绘制钉住徽标（节点右上角 5.5px 强调色圆点徽章）。
  4. 证据与门：6 条单测（`concept_layout` 4 条 + `document` 2 条），测试 1103 → **1109 passed / 0 failed / 1 ignored**；
     新基线 `cm-pin`（`F5BB4AA8DBC8`，117px 徽标）与 `cm-arrange-pinned`（`8C2D3960524A`，
     钉住节点保持在 (280,40) 不动，其余概念重新拓扑重排，与 `cm-arrange` 差 42 569 px）；
     既有右键菜单场景 `cm-multi-menu` 与 `cm-submenu-bottom` 有意重拍；
     修复 `scripts/x100-stage-cm-gate.ps1` 脚本 warmup（24 帧）与 UTF-8 BOM；
     stage-cm 登记 60 场 / 120 次比较两遍全过（`23:16–23:20`），
     stage-b 登记 140 场 / 280 次比较两遍全过（`23:20–23:30`）；
     基线目录 201 → **203**、输入脚本仍 135。证据 `shots/x100/C18/`。
  **未做**：钉住状态不进 `.zmind` 持久化（内存交互态）；检查器未设钉住开关；实机键鼠手感未核。

- 2026-10-31 22:08 · **第 52 轮 · M29 搜索匹配方式可切换（默认拼音 / 精确文本可切）**。
  `docs/02` §3 的 P0 验收条款「匹配方式可切换"精确文本/拼音"」自 10-18 起就存在，
  代码里 `pinyin_eligible` 只看查询词是否为 ASCII 字母数字：搜 `xiangmu` 必命中「项目」，
  搜 `xm` 时原文没有这两个字母却混入拼音命中，用户无法选择「只要精确文本」。
  现在 `mw-core::search` 新增 `MatchMode::Pinyin`（默认，现状：原文 + 全拼 + 首字母）与
  `MatchMode::Text`（只查原文子串，大小写不敏感）；`SearchIndex::hits` 与 `relationship_hits`
  传入 `mode`，Text 模式下**不建也不读**拼音键缓存；`Document::search_with_mode` /
  `search_relationships_with_mode` 透传给壳层。
  `SearchBar` UI 脚注右侧加两段式开关「精确文本 / 拼音」（选中段 8% 强调底 + 强调色文字），
  点击立即重查并在当前行尽量留在同一结果；键盘路径 `Ctrl+Shift+M` 在搜索条内切换
  （实测 `Ctrl+P` 与 `Ctrl+Shift+P` 均被全局「导出 PDF」吃掉，egui 逻辑匹配允许额外 Shift）；
  轨迹 `search_mode mode=text|pinyin rows=<n>` 与 `search_hits`。
  新基线 `search-text`（`F6A2CE8274B3`，搜 `zw` 拼音命中 1 行，切精确文本命中 0 行）；
  既有八张搜索基线（`search-note` / `search-link` / `search-collapsed` / `search-replace` /
  `search-replace-one` / `search-focus-jump` / `search-rel` / `search-rel-jump`）因脚注开关有意重拍，
  每张差 **2 028 px**、bbox 严格在脚注开关区域。
  新增 5 条单测（core 4 + shell 1），测试 1098 → **1103 passed / 0 failed / 1 ignored**；
  stage-b **140/280**、stage-cm **58/116** 两遍全过；基线目录 200 → **201**、输入脚本 134 → **135**。
  证据 `shots/x100/M29/`。

- 2026-10-31 13:26 · **第 51 轮 · M28 撤销栈上限可配（100–2000）**。
  `docs/02` §2 的 P0 行从 10-18 起写着「栈上限默认 500 步（可配 100–2000）」，
  代码里却是编译期常量 `HISTORY_LIMIT = 500` 与两处写死的裁剪；`redo()` 压回撤销栈时
  根本不裁剪——上限若能调低，「降上限 + 重做」就会把栈撑过界。这不是假想缺口：
  20k 文档每步撤销都带一份 `ChangeSet` 快照，深度是用户能感知的内存 / 响应取舍。
  现在 `Document` 自带上限：新字段 `history_limit`（默认 500，clamp 100–2000，
  不进序列化、不写 `.zmind`、不动 `FORMAT_VERSION`），`set_history_limit()` 立即裁剪
  现有撤销栈（丢最旧，空栈 / 已相等不做无谓搬移），`apply` / `apply_coalesced` / `redo`
  三条入栈路径统一读实例上限。`settings::Store` 新增 `undoLimit`（camelCase，缺键回落 500，
  取值 clamp）；设置中心在「自动保存」之后新增「编辑」组：一条 100–2000、步进 100 的滑轨
  + 当前值 + 一句「调低会立即丢掉最旧的记录，无法恢复；重做同样受此上限约束」。
  保存设置立即作用到画布与所有标签页，此后新建 / 打开 / 导入 / 恢复的文档进画布时
  由 `Shell::ui()` 每帧的 `sync_history_limit()` 套用（`usize` 相等即返回，O(标签数) 不遍历文档）。
  验收旋钮 `MW_UNDO_LIMIT=100`（只改内存不落盘）给行为脚本确定性靶子；轨迹
  `history limit=<n> depth=<undo>/<redo>`（只在生效或裁剪时一条）与 `settings save undo_limit=<n>`。
  像素账（1620×1080，`--tol 0`）：设置中心此前没有任何基线，本轮补 `settings`
  （`A34B7458CF0E`，`MW_SETTINGS_DEMO=1`）；改后滑轨 500 ↔ `MW_UNDO_LIMIT=100` 差 **812 px**，
  包围盒 `(445,476)–(632,511)`，只在「撤销步数」这一行。既有 138 场零漂移。
  测试 1076 → **1081**（5 条新：默认与 clamp、降上限丢最旧且不可补回、降上限后重做不越界、
  合并入栈路径同样裁剪、`undoLimit` 落盘往返与缺键回落）；stage-b **139/278**
  （`13:08:03–13:15:40`）、stage-cm **58/116**（`13:20:59–13:24:06`）顺序复跑两遍全过；
  stage-cm 首跑（`13:16:41`）栽在 `mw-render` 的 `rendering_is_deterministic_and_dumps_a_picture`
  GPU 抖动上（本轮未改渲染），隔离复跑通过后整门重跑全绿，已如实记入报告与 `STATUS`。
  基线目录 **200**、输入脚本 **134**。证据 `shots/x100/M28/`。
- 2026-10-31 12:51 · **第 50 轮 · M27 快捷键禁用与配置持久化收口**。
  `docs/02` §10 从 10-18 起写着「快捷键可查看全部并禁用单项冲突项；禁用状态持久化于配置」，
  脚注⑥ 还把查看/改绑/冲突/持久化一并记成未做。代码复核：前四样早已有
  （`Bindings::load` 读覆盖表、`set` 冲突检测、面板 28 行），真正缺的是**禁用**——
  `entries` 是 `Vec<(Action, Chord)>` 没有未绑定态，被输入法占用的键只能改到别的键，
  `shortcuts.json` 写 `"none"` 会被当坏条目静默退回出厂。
  现在 `entries` 改 `Vec<(Action, Option<Chord>)>`；`chord()/hint()/consume()` 对 `None`
  分别返回 `None`/空串/`false`；新增 `disable`（已禁用返回 `false`）、`is_disabled`、
  `enable`（恢复出厂键，被别的启用项占用时返回占用者并保持禁用）；冲突只在启用中的和弦之间判，
  禁用项不再占位，`set()` 会顺带启用；`save_to` 写 `"none"`、`load_from` 把 `"none"`/空串读成
  未绑定（两遍载入先禁后改绑，坏值仍退回出厂），`reset()` 全部重新启用。
  面板每行补「禁用 / 启用」，底部一句话反馈，说明文案点明「被输入法占用时可禁用，
  重新启用恢复出厂键；画布编辑键不在此列」。轨迹
  `shortcut set/reject/disable/enable/enable_reject/reset`；验收旋钮
  `MW_SHORTCUTS_DEMO=1|id,id`（只写内存）。
  行为脚本（隔离 APPDATA）：点第三行「打开」的「禁用」→ `"open":"none"` 落盘；
  重启进程后面板显示「未绑定/启用」，再点「启用」→ `"open":"Ctrl+O"`。
  像素账（1620×1080，`--tol 0`）：改前→默认态差 **45 504 px**（说明文案两行 + 新增按钮列）；
  默认态↔禁用态差 **3 557 px**，包围盒 `(815,379)–(1075,698)`，只在保存/撤销/重做三行。
  新基线 `shortcuts-default`（`F76BCA820852`）、`shortcuts-disabled`（`E32C45210BE9`），
  各连跑两遍零差异；测试 1073 → **1076**（+3）；`stage-b` 136/272 → **138/276**
  （`12:41:58–12:51:06`）、`stage-cm` **58/116**（`12:51:57–12:55:03`）随后顺序复跑，均两遍 `--tol 0` 全过，
  既有 136 场零净漂移；基线目录 197 → **199**、输入脚本仍 134（两场靠环境变量打开面板）。
  文档：`docs/02` §10 脚注⑥ 改回与代码一致。证据 `shots/x100/M27/`。
- 2026-10-31 12:21 · **第 49 轮 · M26 选中互斥（节点 / 关系线 / 注解）**。
  `HANDOFF` §7.5 第 5 条「节点与关系线选中互斥」：三条选中道大部分入口已互斥，
  但 `navigate()`（方向键遍历）、框选/套索松手、`select_ids()`、`select_annotation()`
  只改自己那条道。复现（release，默认示例）：建线（自动选中关系线）→ `ArrowRight` ×2
  → `Delete`，轨迹同时出现 `delete relationship …` 与 `delete-confirm pending=4 selected=1`
  ——一次按键跨两道，线消失并立起 4 节点子树确认。
  新增 `SelectionLane { Node, Annotation, Relationship }`、`foreign_lanes(keep, rel, ann, nodes)`
  （返回除 keep 外必须清掉的道）、`delete_lane(rel, ann, nodes)`；`Canvas::clear_foreign_selection`
  统一清字段，只在实际清掉时写 `selection_clear lane=… reason=…`。补上四个漏口：
  方向键遍历（`navigate`）、框选（`marquee`）、套索（`lasso`）、`select_ids`；
  `select_annotation` 返回「是否有变化」并在内部清节点/关系线，`select_relationship[-_by_label]`
  改用同一助手。`Delete` 改按 `delete_lane` 的单道分派（关系线 → 注解 → 节点），
  落地写 `delete_dispatch lane=…`。
  证据：`mm-rel-select-arrow`（改前/改后差 **1 628 px**，全在关系线包围盒
  `710,515–1094,619`）、`mm-rel-select-delete`（差 **163 px**，确认框外露线段
  `1045,517–1095,526`）；另跑三条轨迹：只选关系线 `Delete` → `delete_dispatch lane=relationship`
  + `delete relationship` 且无确认框；点概要 → `selection_clear lane=node reason=select-annotation`，
  `ArrowRight` → `selection_clear lane=annotation reason=navigate`；关系线选中时框选 →
  `selection_clear lane=relationship reason=marquee`。
  单测 `selection_lanes_keep_exactly_one_lane`、`delete_press_reaches_exactly_one_lane`；
  全量 **1073**；`stage-b` 134/268 → **136/272**（`12:13:36–12:21:00`）、
  `stage-cm` **58/116**（`12:21:23–12:24:34`）两遍 `--tol 0` 全过，既有 134 场零净漂移。
  文档：`docs/06` §5、`docs/30` §5.4 补「选中互斥 / Delete 只删当前道」。证据 `shots/x100/M26/`。
- 2026-10-31 11:57 · **第 48 轮 · M25 大纲行「更多」`⋯` 与行右键菜单**。
  `docs/06` §6 从外壳 v2 起写着「大纲行的「…」仍是占位」：全屏大纲键盘路径齐全，
  指针用户却没有行级入口。行尾补**固定 24px 动作道**（16px `⋯` 距列右 8，悬停/选中出现；
  编辑与浮层开着时隐身但**占位不收回**——否则指针一进行，长标题会当场重排），
  备注/卡片圆点左移让位（右-34.5），`text_avail` / `text_right` 与绘制同减 `MORE_LANE`。
  `⋯` 与行右键共用一张菜单：单行 12 项（新建子/同级、剪切/复制/粘贴/删除、编辑文字、
  折叠/展开、聚焦），多行 5 项（折叠/移入同一父级/复制/删除）；画布专属项
  （关系线/卡片/概要/边界/分离/样式）不收——大纲上看不到结果。菜单键盘复用 `menu` 引擎、
  进 `overlay_state().context`（菜单开着 `text x` 后 `outline_edit_start*` 计数 0）；
  `Esc` 只关菜单，焦点回行。概念图大纲没有树级动作（`Enter` 是新建概念），
  本轮不出 `⋯`、不弹菜单，只落 `outline_row_menu_skipped reason=concept_map`。
  **证据阶段抓到两处骗人的快捷键标签**：`新建子主题` 抄了画布的 `Tab`（大纲 `Tab` 是缩进）、
  `折叠` 写 `/`（大纲是 `Alt+.`）；改成留空与 `Alt+.`，`outline-row-menu` 因此重拍一次。
  **有意重拍**（行尾动作道）：`outline` / `outline-compact` / `outline-search` / `-labels` /
  `outline-drag` / `-illegal` 各 **128 px（0.007%）**（备注点左移）、`outline-hover-select`
  **272 px（0.014%）**（悬停 + 选中两行）、`outline-wrap` **37 166 px（1.613%）** /
  `-2x` **47 089 px（2.044%）**（文字可用宽窄 24px，长标题提前换行）；
  `outline-focus` / `outline-20k*` / `cm-outline` 零漂移，概念图大纲三场回到 HEAD 基线。
  新基线 `outline-more`（`2D5B63C450D1`）、`outline-row-menu`（`85DCFE6ACF75`）、
  `outline-row-menu-add`（`674953F39EB4`）。测试 1069 → **1071**；stage-b **134/268**
  （`11:46:59–11:54:07`）、stage-cm **58/116**（`11:54:12–11:57:16`）两遍全过。证据 `shots/x100/M25/`。
- 2026-10-31 10:47 · **第 47 轮 · M24 边界/概要的键盘路径**。
  `docs/02` §10 脚注⑤ 从 2026-10-18 起写着「`Ctrl+B/]` 仍无快捷键」：概要/边界只有右键菜单，
  菜单第二列还是空的——纯键盘用户加完一批子主题后想圈边界，只能伸手去右键。
  补 `Ctrl+B` = 添加边界、`Ctrl+]` = 添加概要，仅思维导图（概念图没有注解语义，
  守卫先返回，连按两键 0 条轨迹、画布不动）。`take_exact_key` 精确匹配：`Ctrl+Shift+B`
  不建边界，无修饰的 `B` / `]` 仍是落字即编辑（`type_to_edit` 在前）。
  失败不静默：`annotation_shortcut_error` 把「没选」与「选了根」分开，提示
  「先选中主题，再添加边界/概要」与「边界只能圈同一父级下的主题 / 概要只能覆盖同一父级下的主题」，
  轨迹 `annotation_add_failed kind=… reason=no-selection|no-parent`；成功走菜单同款
  `add_boundary_action` / `add_summary_action`（一次撤销），补轨迹
  `annotation_add kind=… targets=N parent=…`。右键菜单「添加概要/添加边界」补 `Ctrl+]` / `Ctrl+B`。
  新基线 `mm-boundary-key`（Warmup 22，sha256-12 `8AD212179A5B`）与 `mm-summary-key`
  （Warmup 26，`B5EA2970F477`）；坐标按落定相机量（首帧 `scene_node` 是 fit 前的，别拿来点）。
  对照：边界帧 vs 「只选中禁则换行」**10 579 px / 0.6047%**、概要帧 vs 「两颗多选」
  **4 954 px / 0.2832%**、两张互差 **24 576 px / 1.4047%**。`menu` / `menu-narrow`
  因快捷键列有意重拍：**581 px / 0.0332%**（bbox `(819,651)–(873,709)`）、
  **579 px / 0.0874%**（`(820,484)–(874,542)`），其余区域 0 px。
  2 条单测；测试 1067 → **1069**；fmt 干净、clippy 基线 36 条新增 0；
  stage-b 129/258 → **131/262**（`10:32:15–10:39:19`）、stage-cm **58/116**
  （`10:41:01–10:44:09`）两遍全过、概念图场景零漂移；基线目录 190 → **192**、
  输入脚本 127 → **129**。证据 `shots/x100/M24/`。
  **未做**：实机微软拼音/键盘未核（`Ctrl+]` 在部分 IME 里是全半角切换）；
  单颗子主题也能建概要（与菜单一致），未额外提示；不自动进概要文字编辑。
- 2026-10-31 10:23 · **第 46 轮 · M23 思维导图插入关系线**。
  `docs/02` §10 脚注⑤ 与 `docs/06` §5/§6 都写着关系线只有右键入口；改前节点菜单的
  概念图行也没有任何快捷键提示，画布上按 `Ctrl+L` 无反应。参照 Xmind 的两步式交互补上：
  思维导图 + 恰好单节点选中时 `Ctrl+L` 进入插入态——源节点描 2px 强调环、指针变十字、
  一条虚线从源节点中心跟随指针；点另一节点落地，只发**一条** `Command::AddRelationship`
  （`label: None`），成功后选中新建的关系线并 toast「已插入关系线 · Ctrl+Z 撤销」
  （检查器可改颜色/箭头/文案）。点源节点/空白或 `Esc` 取消；同一对重复时轨迹
  `rel_link_failed from=… to=… error=…` + 红 toast「两节点之间已存在关系线」，不落地。
  纯函数 `rel_link_step(source, hit)`（`Idle/Commit/Cancel`）与 `esc_plan` 的两级 `Esc`
  （插入态 → 关系线选择 → 聚焦 → 选择）各配单测；`consume_key` 不吃 Shift/Alt 变体，
  `Ctrl+Shift+L` 不会误触发；概念图入口（拖出连线）不受影响（先判 `concept_map`）。
  节点右键菜单 PRO 行补「插入关系线 Ctrl+L」。
  新基线 `mm-rel-link`（Warmup 20，sha256-12 `E7E0682C9B43`）与 `mm-rel-link-created`
  （Warmup 22，`2EC078FE25E8`）；改前 vs 预览 **43 869 px / 2.5074%**
  （画布 33 109、toast 10 760），预览 vs 落地 **38 459 px / 2.1982%**（新关系线 + 检查器）。
  `menu` / `menu-narrow` 因节点菜单多一行**有意重拍**：`132 584 px / 7.578%`、
  `39 286 px / 5.931%`（bbox 只在菜单内）；`menu-narrow` 高度不够，菜单出现滚动条，
  「聚焦」落到折叠线以下——这是按真实菜单内容重拍的代价，已记进报告与账本。
  1 条新单测 + 重写 Esc 用例；测试 1066 → **1067**；fmt 干净、clippy 基线 36 条新增 0
  （首跑 stage-cm 在 `canvas_methods_2.rs` 报 `clippy::if_same_then_else`——`linking` 与
  `rel_link.is_some()` 两个相同 `Crosshair` 分支合并；stage-b 的 clippy 子集不含 canvas
  文件，所以此前没暴露）；stage-b 127/254 → **129/258**（`10:04:10–10:12:12`）、
  stage-cm **58/116**（`10:17:02–10:21:51`）两遍全过、既有场景零漂移；
  基线目录 188 → **190**、输入脚本 125 → **127**。证据 `shots/x100/M23/`。
  **未做**：切换标签页不取消插入态；落地后不自动进文案编辑；取消路径（`Esc`/空白）
  未进金样；实机 IME/键盘未核。
- 2026-10-31 09:25 · **第 45 轮 · M22 聚焦视图外的搜索跳转**。
  M20 收尾留了一条「聚焦模式仍不可达，只 toast「这个主题在当前视图之外」，不替用户退出聚焦」；
  但点搜索结果本身就是显式动作，留在原视图等于检索在聚焦态下失效。改前用
  `MW_FOCUS_DEMO=中文排版` 播夹具聚焦、再 `Ctrl+F → wgpu → Enter`：轨迹只有
  `search_hits rows=1` → `search_jump … ok=0` → 红 toast，画布与相机不动。
  `canvas` 新增纯函数 `focus_exit_levels(doc, focus, id)`：无聚焦 / 目标本来可见 /
  目标不在文档 → `None`，否则返回要退的层数；`select_node_keep_focus` 在
  `scene.find` 失败时先按它 `focus_to(0)`（纯视图态，不进撤销栈），轨迹
  `search_focus_exit id=… levels=…`，随后照旧 `reveal_commands` 展开挡路祖先（一条
  `Command::Batch`）再选中 + 居中。`SA::Jump` 在跳转后对比 `focus_path()`，
  退出过聚焦就 toast「已退出聚焦并跳转」；节点不存在等真不可达仍是红 toast。
  新基线 `search-focus-jump`（Warmup 48，sha256-12 `D271AB2E4183`）；改前图
  （`shots/x100/M22/before.png`）→ 新基线 **189 421 px / 8.2213%**，bbox
  `(8,56)–(1918,1194)`：搜索卡片区域 **0 px**、面包屑消失 5 156 px、画布区约 172 000 px、
  左下提示条 11 390 px。1 条单测；测试 1065 → **1066**；fmt 干净、clippy 基线 36 条新增 0；
  stage-b 126/252 → **127/254**（`09:14:32–09:21:18`）、stage-cm **58/116**
  （`09:09:47–09:14:25`）两遍全过、既有场景零漂移；基线目录 187 → **188**、
  输入脚本 124 → **125**。证据 `shots/x100/M22/`。**未做**：聚焦层级不随撤销恢复；
  实机 IME/键盘未核。
- 2026-10-31 09:05 · **第 44 轮 · M21 Ctrl+H 逐个替换与替换预览**。
  对着 `docs/02` §4 的 P2「替换节点标题文本，逐个/全部；替换预览」查代码：替换行只有
  输入框 + 「全部替换」，`Ctrl+Enter`（焦点在替换框时）也走全部；结果行没有预览；
  `Ctrl+H` 展开后焦点仍在搜索框，接着敲字会进搜索词；备注/链接/拼音命中在列表里，
  但 `replace_all` 对它们静默跳过。改前脚本 `Ctrl+F → 排版 → Enter → Ctrl+H → 排布`
  的轨迹没有逐条替换事件，结果行仍显示「中文排版」（`target/x100/M21/before/`）。
  `mw-core::search` 新增 `replace_char_range(text, [start,end), to)`：按字符区间替换，
  越界 / `start > end` / 非字符边界返回 `None`，空替换串 = 删除命中段（2 条单测）。
  面板新增 `Action::ReplaceOne`、`replace_target` / `replace_target_fields`
  （只接受 `Group::Node` + `field == text` + `kind == text` + `range` 存在）、
  「替换」（52×26）/「全部替换」（72×26）两个按钮与 `Ctrl+Enter` / `Ctrl+Shift+Enter`；
  `Ctrl+H` 展开时 `grab_replace` 把焦点送进替换框，收起时回搜索框。
  预览只跟当前行：第二行 `→ 替换后的标题`（强调色），其他行保留定位信息。
  当前行推进用 `pending_advance = (被替换 id, 原下标)`：刷新后该 id 仍在结果里就取
  `(pos+1) % len`（到底绕回），不在了就 `at.min(len-1)`（按原下标夹取）——
  修掉第一版「替换后当前行不动、连按 `Ctrl+Enter` 反复替换同一节点」。
  壳层 `SA::ReplaceOne` 走一条 `Command::SetText`（一次撤销只回这一步），轨迹
  `search_replace_one id=… chars=… before_chars=…`；选中前先 `node_visible`，
  节点在视图外就不动相机；在视图内用 `select_node_keep_focus`（不设 `refocus_canvas`），
  轨迹因此不再出现替换后的 `focus canvas`。新基线 `search-replace`（Warmup 58，
  sha256-12 `8A6F28044B44`；改前→改后 **3964 px / 0.1720%**，bbox `(697,72)–(1228,348)`
  只在搜索卡片）与 `search-replace-one`（Warmup 68，sha256-12 `7BCAAB250C9F`；
  与上一场差 **78 247 px / 3.3961%**：结果清空、标题改写、相机居中、toast）。
  门里同时暴露 `card-autosave` 按 UTC 跨日腐烂：自动保存把「更新于」写成墙钟
  （`mark_saved` → `mw_core::now_ms()`），本地 08:00 一过日期从 10-30 变 10-31，
  红 56 px（bbox `x[639..645] y[167..175]`）。`note_view` 新增 `card_now_ms()`，
  金样用 `MW_CARD_NOW_MS=1792800000000` 钉在演示时间戳，重拍该基线。
  测试 1058 → **1065**（`mw-core` 2 条 + `mw-shell` 面板 5 条）；fmt 干净、
  clippy 基线 36 条新增 0；stage-b 124/248 → **126/252**（`08:48:29–08:55:25`）、
  stage-cm **58/116**（`08:55:46–08:58:52`）两遍全过、既有场景零漂移；
  基线目录 185 → **187**、输入脚本 122 → **124**。证据 `shots/x100/M21/`。
- 2026-10-31 06:35 · **第 43 轮 · M20 搜索跳进折叠子树**。
  对着 `docs/02` §4「查找」查代码：`Document::search` 遍历 `doc_order()`，折叠子树里的节点
  照样进结果；可画布场景只排可见节点（`layout.rs` 在 `n.collapsed` 时跳过子树），
  `Canvas::select_node_keep_focus` 在 `scene.find` 返回 `None` 时直接 `return`。
  用夹具 `MW_COLLAPSE_DEMO=中文排版` 复现：改前轨迹只有
  `search_hits q=禁则换行 rows=1 note=0 link=0 rel=0`，其后 8 帧没有任何跳转事件；
  画布上「中文排版」仍折叠、没有选中。
  `mw-core::document` 新增 `Document::collapsed_ancestors(id)`：从直接父级往上只收
  `collapsed == true` 的祖先，近 → 远返回、不含自身，`seen` 挡父链损坏造成的环。
  `mw-shell::canvas::reveal_commands` 把它反成根 → 近的 `SetCollapsed { collapsed: false }`；
  `select_node_keep_focus` 改为返回 `bool`：`scene.find` 失败时先应用一条
  `Command::Batch`（撤销栈只多一条，`Ctrl+Z` 一次全收回），再重新 `find`、成功才选中 +
  相机居中 + glow；展开后仍找不到（聚焦模式把节点挡在 `layout_root` 之外）返回 `false`。
  轨迹新增 `search_reveal id=… expanded=N`（展开路径）与 `search_jump id=… ok=1/0`
  （`SA::Jump`），失败时 toast「这个主题在当前视图之外」，**不自动退出聚焦**。
  夹具 `MW_COLLAPSE_DEMO=<节点文字>` 启动时走 `Command::SetCollapsed` 折叠指定节点
  （默认样例里选「中文排版」）。
  测试 1052 → **1058**（`mw-core` 3 条：近→远顺序、跳过展开祖先与自身、节点不存在；
  `mw-shell` 3 条：根→近 + 一条 Batch 一次 undo 全收回且子树回到 Scene、可见链/不存在节点为空、
  可达性只认布局子树（折叠不影响、聚焦时根在外））。
  新增基线 `search-collapsed`（`--tol 0` 双跑，sha256-12 `37AF90C2C865`）；
  改前 → 改后差 **252 082 px**（bbox `x1..1809 y17..1194`，画布整体平移 + 子树展开 + 选中环）；
  分区核对：搜索卡片 `x600..1320 y45..225` **0 px**、标题栏 132 px 是撤销按钮由灰变亮。
  基线目录 184 → **185**、输入脚本 121 → **122**。
  `stage-b` 123/246 → **124/248**（`07:14:21–07:20:56`）、`stage-cm` **58/116**
  （`07:07:48–07:12:31`）两遍全过，既有场景零漂移；fmt 干净、clippy 基线 36 条新增 0
  （`document.rs` 插入代码让既有 `unneeded late initialization` 从 1313 平移到 1337，
  只改了这一行）。证据 `shots/x100/M20/`。
  **未做**：聚焦模式仍不可达（只 toast，不替用户退出）；跳转展开会入撤销栈；
  实机 IME 与键盘手感未核（与 M4 同一缺口）。
- 2026-10-31 05:05 · **第 42 轮 · C17 概念图命题短语进全局检索**。
  对着 `docs/30` §5.1/§5.4 审代码：关系线是「有向且带链接短语的命题」，但 `Ctrl+F` 只走
  `node.text/note/link`；`MW_CONCEPT_DEMO=1` 里「产品定位 —支撑→ 技术方案」搜 `支撑`
  改前轨迹 `search_hits q=支撑 rows=0 note=0 link=0`，一条都不出。
  `mw-core::search` 新增 `RelationshipHit { from, id, to, score, kind, range, excerpt }` 与
  `SearchIndex::relationship_hits`：逐节点按文档前序、边按节点内 `relationships` 下标扫，
  只检索非空白 `label`；原文 560 / 全拼 450 / 首字母 350，位置罚 6 与 prefix/exact 加成沿用；
  同分稳定排序保持文档前序 + 边下标。第四张缓存 `rel_cache`，键 `(from, id)`、值「短语快照 +
  拼音键」，`FIELD_PINYIN_CAP = 240`，`CACHE_LIMIT` 清表时一并清掉；`Document::search_relationships`
  暴露给壳层。
  壳层：`Group::Relationship`「命题」排在节点后、卡片前；`HitRow` 加 `rel: Option<RelRow>`，
  结果行角标「命题」、第二行「概念 · 源 → 目标」；`Action::JumpRelationship { from, id }`
  调 `Canvas::select_relationship_keep_focus`——沿用 `select_relationship` 的互斥选中，
  相机居中到两端概念盒中点，**不抢键盘**。`MW_TRACE search_hits` 加 `rel=`，
  跳转写 `search_rel_jump from=… id=… ok=1`（实测焦点仍停在 search-input）。
  测试 1041 → **1052**（`mw-core::search` 9 条 + `panels::search` 2 条：两端/区间、全拼/首字母、
  空白、缓存失效、240 字封顶、同分顺序、摘录折行与换行、动作与分组顺序）。
  新增基线 `search-rel` / `search-rel-jump`（`--tol 0` 双跑）；改前 → `search-rel` 差
  **27 216 px**（bbox `x665..1256 y80..221`，只动结果卡片）；两张新基线互差 **71 547 px**
  （bbox `x210..1505 y80..966`，画布选中环 + 相机）；基线目录 182 → **184**、
  输入脚本 119 → **121**。
  `stage-cm` 56/112 → **58/116**（`04:31:44–04:36:32`）、`stage-b` **123/246**
  （`04:36:37–04:43:01`）两遍全过，既有场景零漂移；fmt 干净、clippy 基线 36 条新增 0。
  证据 `shots/x100/C17/`。
  **未做**：命题短语不参与 `Ctrl+H`（替换仍只改标题）、不索引颜色/箭头/概要/注解、
  每帧仍扫全部边（只缓存拼音键）、20k 概念图 × 大量命题的首查延迟未实测。
- 2026-10-31 03:31 · **第 41 轮 · M19 搜索命中备注/链接**。
  对着 `docs/02` §4 的 P0 检索口径查代码：`SearchIndex` 只喂标题，备注/链接命中为零；
  而壳层早已另有卡片/标签/路径三路，唯独缺文档字段。扩 `mw-core::search`：备注/链接各自
  缓存键串与原文快照（`note_cache` / `link_cache`），拼音只建到 `FIELD_PINYIN_CAP = 240` 字、
  更长的仍可原文命中；多字段命中取分数最高一条（标题 1000·700·500 / 备注 650·520·400 /
  链接 600·480·380，位序罚 6，前缀 +120）；`SearchHit` 新增 `field` 与 `excerpt`，
  非标题命中给命中处 ±24 字单行摘录（换行 1:1 压空格、拼音命中折回原文区间、两端 `…`
  只在截断时出现）。
  壳层：`HitRow` 带字段与摘录，结果行第二行前置「备注 / 链接」角标；`MW_TRACE search_hits`
  加 `note=` / `link=` 计数。金样旋钮 `MW_SEARCH_DEMO` 给「纯 Rust 原生」播种
  `html` 只出现在备注、`https` 只出现在链接的固定文本，`MW_NOTES=none` 关掉本机笔记库
  （否则卡片正文里的 html 会多出一行，基线锁不住文档命中）。
  测试 1030 → **1041**（`search::tests` 11 条：字段映射/排序/缓存失效/摘录折行与省略号）；
  新增基线 `search-note` / `search-link`（`--tol 0` 双跑，分别差 **29 422 / 27 349 px**，
  包围盒 `x515..1106 y80..221` 只在搜索卡片内）；基线目录 180 → **182**、输入脚本 117 → **119**。
  `stage-b` 121/242 → **123/246**（`03:17:23–03:26:07`）、`stage-cm` **56/112**
  （`03:26:19–03:29:39`）两遍全过，既有场景零漂移；fmt 干净、clippy 基线 36 条新增 0。
  证据 `shots/x100/M19/`。
  **未做**：`Ctrl+H` 仍只替换标题（`docs/02` 归 P2）、关系线短语不在节点检索内（下一轮 C17）、
  长备注 20k 扫描延迟未单独计时。
- 2026-10-31 02:56 · **第 40 轮 · C16 命题短语的空白提交口径**。
  复核 `docs/30` §5.4「空短语」与模型守卫 `SetRelationshipLabel` 时发现两边相反：模型对概念图
  `label: None` 一律 `Invalid`，壳层却以为「空串 = 清掉短语」。端到端复现出两处静默失败：
  ① `MW_CONCEPT_REL_EDIT=带来` 全选删除 + `Enter`——命令被拒、编辑器关掉、短语原样、无任何反馈；
  ② `MW_CONCEPT_LINK=用户痛点|风险与依赖` 直接 `Enter`——编辑器关掉但线还在，`scene_rel` 里出现
  `w=0` 的无短语命题（真线团）。大纲 `Shift+F2` 清空同样无声。
  改法：画布提交判定抽成纯函数 `phrase_commit`（`Unchanged / RejectEmpty / DropEdge / Set`），
  既有命题空提交保留原短语 + toast，新建边空提交撤线 + toast，思维导图仍可 `Set(None)`；
  新增 `Canvas.notice` 一次性通道（与 `card_open` 同构）交给壳层提示条；大纲 `OA::SetVia`
  概念图空串直接拒绝 + 同一提示；`docs/30` §5.4 / §5.6 / §5.6.4 / §5.10 同步改成模型口径。
  **金样实拍发现第三个 bug**：短语框开着时 `editing_active` 不认 `via_edit`，`Ctrl+A` 漏进大纲
  选中全部 6 行；修正后 `cm-outline-via-clear` 与 `cm-outline` 的差从 129480 px 降到 34416 px
  （= 23768 键盘焦点行 + 10648 提示条），与 Esc 对照只差提示条。
  测试 1029 → **1030**（`blank_phrase_commit_follows_the_concept_map_contract` 九条）；
  stage-cm 53/106 → **56/112**（`02:44:54–02:48:15`）、stage-b **121/242**
  （`02:49:47–02:56:11`）两遍全过，`MACHINE GATE PASS` ×2；fmt 干净、clippy 子集新增 0。
  新增基线 `cm-phrase-clear` / `cm-phrase-blank` / `cm-outline-via-clear`（`--tol 0` 双跑）；
  基线目录 177 → **180**、输入脚本 114 → **117**。证据 `shots/x100/C16/`。
  **未做**：实机微软拼音（与 M4 同缺口）、撤销可把线团恢复（模型无全局不变量，不扩大处理）。
- 2026-10-31 02:25 · **第 39 轮 · C15 平行命题短语避让与独立点选**。
  `HANDOFF` §7.5⑤ 记着「平行命题在画布上的独立点选未做（现在靠检查器『命题』组进）」；按 TDD 先写
  会红的单测，撞出 C14 车道的两个真缺陷：① 车道按**源节点**分组，`A→B` 与 `B→A` 各自成「组内一条」，
  两条都走零偏移直线、整条重合（修复前 `label_at` 距离 **0.00**）；② 法向扇开对**竖边**无效——
  两个 36 宽短语中心只差 24，横向重叠 **12 世界单位**。修法：按无向对跨 `doc_order()` 一次收齐车道、
  新增 `CONCEPT_LABEL_STAGGER = 24` 沿规范方向切线错开、`bow_crossings` 跳过车道 Quad（否则按弦线
  重弯会把偏移抹掉）。`owner_ids` 只取一次，输出逐像素不变。
  `mw-render` 补 `parallel_propositions_are_individually_hittable`；`scene_rel` 轨迹输出每条关系线的
  `start/end/mid/label/w`；`MW_CONCEPT_PARALLEL=1` 在示例图「用户痛点 → 增长策略」补第二条命题「推动」。
  两场金样 `cm-parallel-select-a/-b` 分别点 (541,570)/(509,609)，轨迹选中 idx=3/idx=4 两个不同 id；
  两图差 **2 528 px**、bbox `x500..549 y409..765` 只在车道竖带。测试 1025 → **1029**；
  `stage-cm` 51/102 → **53/106**（`02:12:17–02:17:21`）、`stage-b` **121/242**
  （`02:18:44–02:25:06`）两遍全过，`MACHINE GATE PASS` ×2；fmt 干净、clippy 36/36 新增 0。
  证据 `shots/x100/C15/`。**未做**：实机鼠标点车道的体感、三条以上车道的短语越界、端点汇合区歧义。
- 2026-10-31 01:52 · **第 38 轮 · M18 Ctrl+X 删除确认**（先验证后改，产品行为零改动）。
  `HANDOFF` §7.5③ 记着「`Ctrl+X` 多节点不弹删除确认」；读码发现 `cut_action()` 与
  `apply_clipboard(ClipboardChord::Cut)` 最后一行都是 `delete_selected()`，而 `delete_selected()`
  在 `delete_confirm_plan` 返回 `Some(count)` 时只挂 `confirm_delete`、不删节点——旧结论不成立。
  新增三条 `MW_TRACE`：`delete-confirm pending=3 selected=1`、`delete-confirm cancel count=3 reason=esc`、
  `delete-confirm accept count=N`。两场金样（tiny-cn.zmind）：`mm-cut-confirm` 点「目标」(654,433)
  （落定 `scene_node rect=(596,401,116,65)` 的中心）→ `Ctrl+X`；`mm-cut-cancel` 同脚本 + `Esc`。
  像素账：确认框带全屏遮罩，与「只选中、不剪切」对照整屏 **2 304 000 px** 不同；
  **取消后与该对照 0 px**，证明剪切没有在确认前偷偷删节点。
  `stage-b` 119 → **121** 场；测试仍 **1025**；`stage-b` 两遍 242 次比较（`01:38:52–01:46:51`）、
  `stage-cm` 51/102（`01:47:40–01:50:25`）全过，`MACHINE GATE PASS` ×2；fmt 干净、clippy 36/36 新增 0。
  证据 `shots/x100/M18/`。**未做**：实机 winit `Ctrl+X` 路径、多选剪切单独拍图、「删除」按钮接受路径。
- 2026-10-31 01:22 · **第 37 轮 · M17 悬停反馈**（折叠圆点 / 概要 / 边界）。
  三个目标原来只有命中、没有指针反馈；`HANDOFF` §7.5② 曾判断「要改 `mw_render::Frame` ABI」，
  复核发现不需要：`Frame` 只带节点 id，指针位置在壳层本来就有，反馈画在 egui 覆盖层即可。
  新增 `canvas/hover.rs`：`HoverDetail{Toggle,Summary,Boundary}` + `paint_hover_detail`——
  圆点按 1.3× 重绘（120ms `design::motion::FAST`；`camera.chrome(8,4.5)` 保持屏幕恒定，
  放大后光晕/横杠/竖杠一起画）、概要（花括号 + 摘要主题盒）与边界画 2px 强调色描边（55% 强度）；
  受与节点悬停同一组门（编辑/拖拽/框选/输入阻塞时不画）。轨迹新增 `hover_detail` 与
  `scene_summary` / `scene_summary_brace` / `scene_boundary` / `scene_toggle`。
  三张单测：目标映射、1.3× 端点夹取、圆点半径随缩放恒定（200% 时 9px）。
  **证据方法**：悬停与「无指针」差 15 879 px（含节点悬停底 + 动作条），改用同帧「指针在节点本体」
  作对照，`mm-toggle-hover` 只差 377 px、bbox 恰好是 24×24 圆点；概要 1 484 px、边界 2 748 px。
  **顺带修证据陷阱**：`mm-collapse` / `mm-collapse-mid` / `mm-toggle-edge` / `mm-toggle-miss` 的点击坐标
  自第 19 轮启动取景改动后全部点空，四张基线锁的是「没折」；按落定相机改为 (522,429)/(522,438)/(522,441)
  重拍，旧新 `mm-collapse` 差 108 025 px、中点与终态差 62 619 px（旧记 27 913 是伪中间帧）。
  新增基线 `mm-toggle-hover` / `mm-summary-hover` / `mm-boundary-hover`；测试 1022 → **1025**；
  `stage-b` 119 场 / 238 次比较（`01:11:26–01:17:44`）、`stage-cm` 51 场 / 102 次比较
  （`01:18:22–01:20:55`）两遍全过，`MACHINE GATE PASS` ×2。证据 `shots/x100/M17/`。
  **未做**：实机鼠标淡入手感（60Hz/高刷）未核；只拍 ppp 1.0。
- 2026-10-31 01:00 · **第 36 轮 · 补齐八份 manifest**（纯文档，不改代码/基线/门）。
  `CM-12` / `SE-11` / `CM-13` / `M15` / `C14` / `SR-01` / `SR-02` 七份按各自 `report.md` 回填，
  另补 `M16`；结构照 `shots/x100/C9/manifest.json`。约定：`_note` 标明补记、不新增结论；
  无外观改动的包 `scenes: []` 且写原因；历史数字（如 SR-01 写作时未跑成的两条 mw-shell 导出单测）
  原样标注，不按今天的总数改写成「当时已跑」。八份 JSON 解析校验通过。
  回填后更新 `HANDOFF.md` §7.4/§7.5⑥/§7.7③：清单缺口关闭；`docs/STATUS.md` §6 不再列这项。
- 2026-10-31 00:40 · **第 35 轮 · M16 金样按帧确定性**（修 `stage-b` 第 1 遍 `outline-*` 抖动）。
  两场失败现场：`outline-drag-illegal` 2.827% / 49465 px、`outline-hover-select` 2.751%；
  10 次空载/负载复跑都逐字节等于基线，说明只在「GPU 刚起、帧耗抖动」的窗口出现。
  根因两层：① 金样按固定帧数截帧，而大纲握把 120ms / 拖拽展开 500ms 读 `Instant` 墙钟、
  egui 滚动落定读 `stable_dt`；② 离屏进程仍可能收到宿主 `PointerGone` / 真实键入。
  改法：`GoldenInput::advance_clock()` 帧计数虚拟时钟（普通帧 +1/60，`sleep` 冻结、到期补名义时长）、
  壳层每帧写 `RawInput::time` 与 `predicted_dt = 1/60`、harness 下 `retain(!is_host_input)` 剥离
  指针/键盘/IME/剪贴板/焦点/AccessKit 事件、`outline.rs` 两处计时改 `ui.input(|i| i.time)`。
  视觉零漂移：`outline-drag-illegal` / `outline-hover-select` / `outline-20k-deep` 与基线逐字节一致
  （sha256 前缀 `1776a77897f6` / `0031d764b789` / `50f2152f9c11`）。测试 1021 → **1022**
  （新增 `clock_is_frame_based_and_sleep_adds_nominal_time`）；fmt 干净、clippy 36/36 新增 0。
  `stage-b` 连跑两次（`00:24:06–00:30:48`、`00:33:57–00:40:10`），每次 pass 1 / pass 2 各 118 比较，
  **四遍矩阵 472 次比较零差异**，`MACHINE GATE PASS` ×2（日志 `target/tmp/gate-b-run{1,2}.log`）。
  清理：`shots/golden/outline-*/actual|diff` 六场 12 张旧失败图——重点两场留
  `shots/x100/M16/failure-2026-10-30/`，其余移 `target/tmp/stale-golden-artifacts-2026-10-31/`。
  证据 `shots/x100/M16/`。**未做**：无「旧码红 → 新码绿」A/B；失败图标题「4|」未真机重放。
- 2026-10-30 · **第 34 轮 · C14 平行命题分车道**：同一对概念之间允许多条命题后，画布上仍画成同一条直线，
  第二条点不中。`scene.rs` 按无向对 `(min,max)` 分组、组内序号给法向偏移 `gap = min(24, len/6)`、
  二次贝塞尔 `bowed_straight()`；单条命题走零偏移分支，既有单命题基线零变化。
  坑：法向必须在规范方向取，否则 `A→B` / `B→A` 相消。三条单测（锚点分离 ≥8 / 必须走曲线 / 单命题直线）；
  `mw-core` **248 passed**（+7）。报告原本写明默认基线不受影响，故本轮未重拍 golden。证据 `shots/x100/C14/`。
- 2026-10-30 · **第 33 轮 · CM-13 关系线箭头屏幕恒定**：箭头从 `Scene` 写死的 6×4 世界单位改成
  `screen_arrow()` 以尖端为锚等比放大到 ≥7 css px（`k >= 1`），30% 缩放下 1.8 → 7 css px；
  `relationship_bounds` 同步用放大后的三角形。漂移只在箭头像素：100% 档 `k ≈ 1.17`、每箭头 6–14 px，
  50% `k ≈ 2.33`，30% `k ≈ 3.89`；概念图相关基线按 `--tol 0` 重拍（场次以两道门日志为准）。
  证据 `shots/x100/CM-13/`。
- 2026-10-30 · **第 32 轮 · SE-11 对比度达标 + 令牌归一**：`ui.dim` 在 `panel2`/`hover` 上 4.31–4.36:1
  → 逐主题调深（snow `#67706c→#646d6a` 4.34→4.53、mist 4.36→4.56、sand 4.31→4.51，ink 不变）；
  `faint` 2.64–2.82:1 → alpha 0.48→0.56（暗 0.50→0.58，仍严格淡于 `dim`），四底最差 3.20–3.50；
  令牌归一（`CANVAS_LEAF`、焦点环、删零引用 `PAD_X/PAD_Y`）。新增 `contrast.rs` 7 条硬门（全绿）。
  报告定性为「不改布局几何，只在文字颜色像素上可能漂移」；相关基线随门重拍。证据 `shots/x100/SE-11/`。
- 2026-10-30 · **第 31 轮 · CM-12 概念节点改椭圆**：概念图无显式覆盖时默认 `Shape::Ellipse`；
  渲染用负圆角哨兵 `ELLIPSE_CODE = -1000`（`RectInst` 仍 68 字节，WGSL `sd_ellipse`）、
  命中改椭圆方程、SVG 原生 `<ellipse>`、PDF `k=0.5522848` 四段贝塞尔、编辑环 `egui::Shape::Ellipse`。
  盒子按比例外扩（宽 `+min(0.22w,24)`、高 `+min(0.16h,8)`）而文字盒不动，换行/省略阈值逐字节不变；
  `concept_boxes_follow_the_node_contract` 重写为椭圆契约并同图验证显式 `rounded` 不外扩。
  概念图基线全面重拍（外形变了）；证据 `shots/x100/CM-12/`。
- 2026-10-30 13:40 · **第 30 轮 · CM-11 概念图复制语义 + 选中/悬停底色**（`docs/30` §5.2/§5.3/§5.7）。
  审计发现概念图的复制/粘贴、`Ctrl+D`、`Ctrl+拖拽` 仍走树路径：`paste_under` 会把概念挂成子节点，
  `duplicate_command` 按兄弟插入，`commit_concept_move` 完全忽略 `Moving.copy`；同时 §5.3 的
  「选中 8% / 悬停 4% 强调底」从未落地，只画了环。
  改法：新增 `Command::InsertConcepts`（只概念图成立；拒绝父子结构、非有限坐标、已占用卡片；
  边界/概要清空；关系线只保留可解析目标；一批一条撤销），复制/粘贴/`Ctrl+D`/`Ctrl+拖拽` 统一产出
  **独立浮动概念**，保留内部命题、位置整体偏移（粘贴与复制 32/44）、卡片解绑。渲染层在画环之前先铺
  `SELECT_FILL=0.08` / `HOVER_FILL=0.04` 的强调底。
  视觉：26 个 shell 场景 + `canvas-ppp1.00` + 3 张 `offscreen-zoom*` 重拍；代表 diff
  `cm-crosslink 7763 px / 0.337% / max 16 / bbox (466,376)-(610,430)`、`mm-drag-child 11592 / 0.663% /
  max 14 / bbox (536,601)-(716,666)`、`selected 12181 / 0.696% / max 9`。全部只在节点盒内。
  回归：测试 963 → **968**（+5）；fmt 干净、clippy 新增 0；stage-b **118/236**（13:36:57）、
  stage-cm **49/98**（新增 `cm-copy-paste` 后复跑）双跑 `--tol 0` 全过。顺手修 X11–X16/X20 的
  `docs/16` 归档路径；四个拖拽/折叠中间态脚本末尾加 `sleep 160`，消除高负载下的吸附/过渡双态。
  证据 `shots/x100/CM-11/`。
- 2026-10-30 07:45 · **第 29 轮 · 检查器数值格可键入 + 方向键步进**（`docs/30` §4.2 + §4.4，SE-06 的续一轮）。
  对着合同查代码发现的是「一半」：stepper 的 `−`/`+` 是能点的，但**中间那格只是 `painter.text` 画的一段字**——
  不可聚焦、点它没反应、没有方向键、不能键入；字号从 14 改到 48 要点 17 下。§4.4 写的是「每个可改属性都有键盘路径」，
  对数值来说那条路径就是**敲数字**。本轮把中间那格做成：点一下（或 `Enter`）变输入框、焦点在格上敲 0–9 当场开编辑、
  `↑`/`↓` 与 `+`/`−` 等价、`Enter` 提交、`Esc` 取消、点别处按提交算；`parse_stepper_input()` 管脏输入与越界
  （"5"→12、"300"→96、"abc"/"" → 不发命令）。
  **途中撞到两个只有拍图才看得见的问题**：① 第一版用 `ui.put` 画 `TextEdit`，`put` 把父光标推到输入框下沿，
  输入框比 24px 的一格高一点，「粗体/斜体」整行被顶下去（diff 带 y 620..634、205 px）——改 `ui.new_child(max_rect)`，
  只在一个别人定好的矩形里画、不碰布局；② 开编辑那一帧 `resp.lost_focus()` 恒真，点一下还没打字就把原值提交了一遍——
  改成先取 `was_fresh` 再清标志，金样种子另走 `pinned`（不抢焦点、不自动提交，离屏才稳）。
  编辑态的数字用 `TextEdit::align(RIGHT_CENTER)` 与不编辑时同一条右对齐基线（点进去数字不「跳」到左边）。
  新基线 `inspector-stepper-edit`（`MW_STEPPER_EDIT=inspector/style/size:48`），与 `inspector-shape` 的差
  **372 px 且包围盒精确等于编辑格**（数字带 + 1px 强调色底线），布局零位移。测试 959 → **963**。
  stage-b 门 118 场 / 236 次比较、stage-cm 48 场 / 96 次比较（两遍全过）。证据 `shots/x100/SE-06b/`。
- 2026-10-30 07:26 · **第 28 轮 · P 轨 SE-10（检查器证据包与 G-B3-2 门清单）**。**本包不改一行功能代码**，
  只把 SE-05–SE-09 收成一份能过人工门的东西。做的四件事：
  ① `docs/30` §4.1–§4.6 逐条回填「合同 → 落地 → 证据」总表（22 行），每条都能顺着指到基线或单测；
  ② 盘 `docs/30` §4.2 的**必选能力 14 项**：**10 已实现 / 1 部分 / 3 延后**——部分是「形状缺椭圆」，
  延后是「字重 400/500/600 / 文本对齐 / 圆角」，三项都要往 `StyleOverride`（`Node.style`）加字段，
  按 `docs/27` 属数据模型变更、需单独批准，所以 G-B3-2 第 4 条判词必须由人拍板，不替人写「通过」；
  ③ 十张检查器基线逐张写清钉住什么（含 `cm-arrow-inspector`，它是 CM 轨的、每次检查器改动都波及）；
  ④ 写完 `gate.md`（五判词全留 ☐ + 每条看哪张图）与 `manifest.json`。
  顺带记下两条工程债：§4.1「组内行间距 6」现在仍是全局 8（改它十张基线整体下移，留作下一轮）、
  `dim` 在 `panel2` 底上 4.14:1 差 AA 0.36。
  机器判据：测试 **959**、stage-b **117 场 / 234 次比较**（2026-10-30 07:17 双跑全过）、
  stage-cm **48 场 / 96 次比较**（07:19 双跑全过），与 SE-09 同数——本包确实没动功能。证据 `shots/x100/SE-10/`。
- 2026-10-30 07:24 · **第 27 轮 · P 轨 SE-09（检查器键盘与无障碍）**。对着 `docs/30` §4.3/§4.4：
  出厂键从 `F9` 改 **`Ctrl+Shift+I`**（新增 `Action::InspectorAlt` = F9「检查器（备用）」，两者同义，
  快捷键面板/速查/命令面板里的 F9 提示仍然成立）；色板补 **Enter 选择**与**数字键 1–9 直选**
  （只在色板自己握焦点时吃键——否则在面板别处敲数字会把颜色改掉）；**Esc 两级**：控件 → 先松焦点、
  面板留着，再一下才回画布；判据抽成纯函数 `esc_step`，「都没焦点就不吃这一下」避免抢画布的 Esc。
  无头 harness 里 egui 的焦点不跨 `run_ui` 保留，两级跨帧拍不出来，这条只做纯函数单测 + 实机核对。
  新增 **28px 错误条**（`Inspector.error` + `report_error`/`clear_error`）：`apply` 失败时面板里贴条、
  toast 照旧，成功即消；不按墙钟过期（金样稳定）。新基线 `inspector-error`，旋钮 `MW_INSPECTOR_FAIL=1`
  走**真的失败路径**（发一条指向不存在节点的 `SetStyle`），轨迹 `inspector_error shown=1 text=…`。
  **对比度量出一个真问题**：浅主题 `faint` 对面板底色只有 **2.37:1**（摘要/提示/空态全用它），
  改法是检查器里的**文字**一律换 `dim`（对 panel ≥ 4.5），`faint` 只留给定线/色点/开关轨道；
  `dim` 在 `panel2`（卡片候选列表的底）上是 **4.14:1**，差 AA 0.36，单测单立 ≥4.0 下限并记账。
  十张检查器基线因 faint → dim 重拍；`cm-arrow-inspector` 因同一改动漂 562 px / 0.024%（面板内）也重拍。
  **顺手治了三个「一忙就飘」的输入脚本**：`outline-hover-select` 的保 hold 从 400 → 900ms
  （悬停淡入彻底饱和）、`mm-drag-before` 在 press 与两次 move 之间各加 2 帧、`mm-new-cancel` 的
  每步都多留几帧（原来是 `wait 1`）。三张基线重拍后**与旧基线逐像素 0 差**——等待只治了抖动、没有改画面，
  之后各连跑两遍零差异。
  **诚实记录**：SE-09 的两轮机器门里，pass 1 先后飘过 `mm-drag-before` / `mm-new-cancel` /
  `outline-hover-select`（都是按住/悬停的真实时间脚本，与检查器无关），pass 2 都是干净的；
  根治办法是把脚本的等待给足，已按上面改掉。
  测试 956 → 959；stage-b 117 场 / 234 次比较、stage-cm 48 场 / 96 次比较。
  证据 `shots/x100/SE-09/`。
- 2026-10-30 06:20 · **第 26 轮 · P 轨 SE-08（作用域与批量编辑）**。对着 `docs/30` §4.1/§4.2/§4.3/§4.6：
  作用域原先是样式页顶上单独一行「作用 / 当前节点 / 含子树」（72×24 + 64×24 两段），合同要的是
  **分组标题右侧**两段「本节点 / 含子树」——搬进 `group_header`（新增 `scope: Option<bool>`），
  选中那段给 8% 强调底 + 强调色文字；有开关的组不再画右摘要（两个都靠右，288 宽放不下）；
  点在开关上的那一下不折叠分组（`resp.clicked() && !scope_rect.contains(pointer)`），
  否则想切作用域却把组收起来。新增 `group_scoped()`，颜色与「形状与文字」两组改走它。
  补 §4.3 的 **500ms 合并撤销**：`style_gesture_key()` 只给 `SetStyle` / 全 `SetStyle` 的 `Batch`
  出手势键（排序去重后的 id 集），`same_gesture()` 判同键 + ≤500ms，`Canvas::apply_command_coalesced`
  落到 `Document::apply_coalesced`；换主题 / 写备注 / 删除这类命令会把窗口关掉，后面的编辑不被粘上。
  证据侧新增 `MW_SELECT_DEMO=multi`（选中「纯 Rust 原生」+ 父级 + 第一个子级）与新基线 `inspector-multiselect`。
  样式页六张**按预期重拍**（组体整体上移约 32 css px）：`inspector` 59055 px / 2.848%、`-narrow` 56253 / 2.713%、
  `-shape` 43983 / 2.121%、`-ink` 63195 / 3.048%、`-mist` 62522 / 3.015%、`-sand` 62521 / 3.015%，
  包围盒全在面板内（画布 0 px）；`inspector-empty` 与 `inspector-content` 零漂移。
  测试 954 → 956；stage-b 116/232（2026-10-30 06:29，双跑）、stage-cm 48/96（2026-10-30 06:32，双跑） 双跑全过。证据 `shots/x100/SE-08/`。
  **未做**：作用域开关的键盘路径（§4.4 归 SE-09）；检查器临时作用域与节点上持久的
  `StyleOverride.apply_text_to_subtree` 两个「子树」开关并存，语义要不要合并得先在 `docs/30` 定。
- 2026-10-30 03:30 · **第 25 轮 · P 轨 SE-07（检查器内容组 v2）**。对着 `docs/30` §4.1/§4.2/§4.5 把内容页做完：
  五组分从「全 `false`」改成**默认展开**，各带一句活摘要（空/已写、未开始·进行中·完成、`n 个`/无、host、卡片标题/未绑定）；
  备注 `note_rows(text, expanded)`：3 行起、超过 8 行封顶 8 行 + 「展开 · 共 N 行」，换节点或清空选中自动收回；
  标签从「标记」页搬到内容页，删掉 `corner_radius(9)` 胶囊底与 `panel2` 底，只剩 **6px 色点 + 12.5 文字**（§4.5 禁 chip 墙）；
  卡片补一行「来源」（独立卡片 / 本文档 · 挂在主题上 / 本文档 · 独立卡片 / 来自其它文档）；
  备注文本域按 §4.2 从「填色 + 1px 描边 + 6 圆角」改成**无底无框 + 一条 hairline 底线**
  （面板内 `(248,249,250)` 填色像素 40451 → 10023，备注区只剩 y=541 那条 394px 的线）。
  **两处只有量才看得见的回归（机器门当时是绿的）**：① 「添加」按钮用自然宽、`TextEdit::desired_width` 又**不含控件自身 8+8 内距**，
  整行比可用宽多 16 → 面板最小宽被内容顶到 **316.65 且每帧 +2**（→ 326.65）；用 `MW_INSPECTOR_GROUPS` 逐组关掉定位到标签组，
  改 `label_field_width()` 先算宽再排 + `add_sized([44, 22])`，轨迹回到 `inspector w=288.00`。
  ② `ui.horizontal` 按全局 `spacing.interact_size.y = control::INPUT = 30` 要高度 → 标签行 30 高、行距 38（金样里两颗色点相距 38）；
  新增 `content_row()` 用 `allocate_ui_with_layout` 的显式 `desired_size` 钉死 28，色点间距 38 → 36。
  两条都配了会红的单测；`the_label_row_fits_the_panel` 的第一版拿 `ui.set_max_width` 量面板宽，被证明恒真（`set_max_width` 夹住 `min_rect`），已删重写。
  证据侧新增 `MW_INSPECTOR_TAB=style|note|mark` 与 `MW_CONTENT_DEMO=1`（五项内容全走 `Command`，与手点同一条撤销栈）、
  新基线 `inspector-content`。测试 948 → **954**；stage-b **115/230**（03:27）、stage-cm 48/96（03:30）；
  `inspector` 等七张老基线零漂移。证据 `shots/x100/SE-07/`。**未核**：标签框/备注框里的实机微软拼音。
- 2026-10-30 02:30 · **第 24 轮 · P 轨 SE-06（检查器样式组 v2）**。对着 `docs/30` §4.2 把样式组重做，六条 ❌ 全清：
  色板从「17px 格 / 点半径 6 / 列距 3 / 列数自动算」改成 **8 列 × 24px、列距 8**（288 面板 8 列、240 面板 7 列）；
  选中改画 **2px 强调环（r11）+ 1px 内白（r9.5）**，压在 8px 列距里不压邻居；
  **第 9 格「自定义」虚线圆**（12 段短弧）点开就地展开取色器——删掉填充/分支线/文字**三处整行「自定义…」**；
  补 **「无颜色」斜杠格**（清除覆盖回继承，继承态自带 2px 环）；字号 12–96（步 2）与边宽 0–8（步 1）换成
  **24 高 × 72 宽 `− 值 +` stepper**；边框色板接同一套网格。每项仍是一条 `SetStyle`，一次撤销。
  开工审计里判 `延后` 的四项（圆角 0–24 / 文本对齐 / 字重三档 / 椭圆）在 `StyleOverride` 与 `Shape` 里没有字段，
  本轮不动、界面也不出假控件，方案写进 `shots/x100/SE-06/report.md` §5。
  **两个真缺口（本轮补的）**：① 色板只有 ←→ ——「雪」主题色板是两行（8 色 + 自定义/无颜色），
  第二行键盘根本走不到；补 ↑↓ 跨行，两端夹取，键鼠仍共用 `Sense::click()` 一个出口。
  ② **「形状与文字」默认折叠**，所以 `inspector` / `inspector-narrow` / `inspector-empty` 三张老基线
  一张都拍不到本包交付的边框/边宽/形状/字号四行——等于交付的东西没上过镜。加 `MW_INSPECTOR_GROUPS`
  （`key=0,key=1`）启动预置分组开合，配套 `WindowState::seed_inspector_group`：**只写内存、`saved` 对齐后不落盘**，
  拍金样不会改用户自己的 `window.json`；新单测 `seeded_groups_stay_in_memory_and_never_touch_the_file` 钉住。
  顺手把 `stepper()` 的 8 参数收进 `StepperRow`（clippy `too_many_arguments` 新增必须为 0）。
  **发现的既有事实**：`Theme.palette` 雪 8 色、墨夜/雾/沙各 6 色 → 「第 9 格是自定义」只在雪主题字面成立；
  统一成 8 色要动主题令牌并连累主题金样与标签/分支颜色，不在本包内改，记进 `report.md` §7 待决定项，
  新单测把「palette ≤ 8 且次序固定」写死。
  证据：`inspector` 漂 **29434 px / 1.419%**（bbox `x1502..1877 y523..975`，画布 0 px）、
  `inspector-narrow` **28147 px / 1.357%**（`x1574..1902 y523..975`）、`inspector-empty` **0 px**、
  `cm-arrow-inspector` **0 px**（概念图侧零漂移，无需重拍）；新增 `inspector-shape`（`MW_INSPECTOR_GROUPS=style/color=0,style/shape=1`，
  与 `inspector` 差 43933 px 且 bbox 全在面板内）、`inspector-ink` / `-mist` / `-sand` 四场。
  `report.md` §3 开工时写的「色板降到 6 列」是估数，实测 **7 列**（216 版心 → `floor((216+8)/32)`），已改正、测试按 7 钉住。
  测试 944 → **948**（新增 `palette_ends_with_custom_then_none`、`seeded_groups_stay_in_memory_and_never_touch_the_file`）；
  fmt 干净、clippy 指纹 基线 38 / 重报 37 / **新增 0**；stage-b 门 **114/228**（02:23:49）、stage-cm **48/96**（02:26:21）双跑全过。
  **未做**：色板数字键 1–9 直选（归 SE-09/SE-10 键盘包）；实机键盘手感与实机微软拼音仍未核，本包只记 `已实现`。
`docs/30` §4.1–§4.5 逐条对代码，七条 ❌：面板宽 260（合同 288）、可拖 220–520（合同 240–360）、头行/对象行没有固定高度、分段控件是 28 高**描边胶囊**（合同：30 高无描边 + 8% 底 + 1.5px 底条）、分组无 chevron 不可折叠也没有记忆、组内左右 16（合同 12）、无选中时直接铺文档属性（合同：一句话 + 文字动作）。
  做法：`PANEL_W=288`/`MIN=240`/`MAX=360` + `clamp_width`；`geo` 常量 40/44/30/28/8；`segment()` 重写为无描边 + 底条；  `group_header()` 画 chevron（两段线）与右摘要、组间 8px hairline；`window_state.inspector_groups` 存 `键=0/1`（只记用户动过的组，  标记组默认折叠）；`doc_body` 空状态；`app_methods_2::inspector_panel` 给画布留 360。
  **开发中撞到的真回归（已修）**：「形状」四段写死 `Vec2::new(66.0, 26.0)`，内容最小宽 = 16+4×66+12 = **292**，egui 把面板顶到 292——  轨迹实测 `want=240.00 actual=293.00`，用户拖到 240 也不生效；而旧的「每帧把 actual 写回 `inspector_w`」逻辑把这个 293 记成用户的选择，  于是「拖到 240 → 下次打开变 293」。修法：`shape_seg_w(available)` 按版心均分（下限 44）+ 只在 `dragged/drag_stopped` 回写；  修后轨迹 `inspector w=240.00`。
  证据：三张新基线 `inspector` / `inspector-narrow` / `inspector-empty`（1920×1080 @ ppp 1.5，`--tol 0` 双跑零差异）；  轨迹 `inspector w=288.00 head=40 obj=44 seg=30 bar=1.5 group=28 gutter=12`（每次运行一行）；  组内左右 16 → 12 让检查器所有行左移 4px（有意为之，已量化）。`cm-arrow-inspector` 因检查器整列重排漂 **95242 px（4.134%）**，  `-Update` 重拍后双跑零差异。测试 939 → **944**（mw-shell bin 537 → 538），fmt 干净、clippy 新增 0（`group_header`/`group_with` 的 8 参按仓库惯例挂 allow）；stage-b 门 **110/220**（01:33:55）、stage-cm **48/96**（01:45:23）双跑全过。
- 2026-10-30 00:42 · **第 22 轮 · P 轨 SE-04（起始页证据包与 G-B3-1）**。按 `docs/30` §3.1–§3.7 逐条审代码
  （几何/字号/断点/视觉/键盘/四入口/禁止项），结论：只有 §3.4「最近文件缺失」与合同相反——旧行为是启动时
  `Recent::prune_missing()` 把文件已不在盘上的那一行**删掉**，合同要的是**留行 + 标题 secondary + 副行「文件已移动」+ 点它走打开失败态**；
  全仓当时连「文件已移动」四个字都没有。做法：`Recent::Entry.missing`（`#[serde(skip)]`，`recent.json` 语义不变）、
  `Recent::mark_missing()` 在载入/触摸时算一次（不每帧 stat）、删掉 `prune_missing`；`start::recents_card` 缺失行
  改用 secondary 标题 + 副行「文件已移动」，轨迹打 `recents row missing title=… rect=…`；harness 用 `MW_RECENTS_MISSING=<n>` 播种缺失。
  新基线 `start-missing`（stage-b 门第 107 场）：与 `start` 逐像素比 **1082 px，bbox `x562..627 y441..470`**，正好是第二条行的标题与副行，
  行外 0 差异；点击脚本（`start-missing-click.txt`，坐标取自稳定后的行矩形 `526,433 868x44` 的中心）走到
  `open_start` → `open_fail from_recent=1`，不是静默也不是删行。单测换血：`missing_files_are_pruned` →
  `missing_files_stay_in_the_list_and_get_marked`（语义反转）；测试仍 **939**，fmt 干净、clippy 新增 0。
  stage-b 门 **107 场 / 214 次比较双跑全过**（2026-10-30 00:40）；门清单 `shots/x100/SE-04/gate.md`（五判词逐条留 ☐，不代写结论）。
  **未做**：运行中被删的文件要等下次载入才标出来；「在文件夹中显示」「重新定位」这类补救动作合同没写，没做；
  `scripts/ci.ps1` 的 start 场景没加 `start-missing`（ci 用独立 APPDATA，本来就不读用户列表）。
- 2026-10-30 00:42 · **第 21 轮 · P 轨 SE-03（起始页键盘、响应式与概念图入口）**（与第 22 轮同一钟点回填）。
  审计发现三处与 `docs/30` §3.5/§3.6 不符：① **Tab 序反了**——「清除最近文件」画在最近文件区段标题行里，
  注册顺序早于行，键盘先落它；② **`Ctrl+Shift+N` 根本没绑定**，概念图的第四个入口只在纸面上；
  ③ **主卡焦点环画在卡外**，与主卡 8% 强调底叠成同色，等于没有环。
  做法：① 标题行只按尺寸占位（`Sense::hover`，不是控件），行画完后 `ui_kit::text_button_at` 在原矩形 `ui.interact` 注册
  ——位置与顺序各归各位；**弯路如实记**：第一版用 `ui.scope_builder(max_rect=…)` 放按钮，它会把子矩形的高度
  分配进父布局，整页下移 42%，改成 `allocate_exact_size` 占位 + `interact` 才不动版式；② 新增
  `shortcuts::Action::NewConcept`（默认 `Ctrl+Shift+N`），**判别必须排在 `New` 之前**——egui 的 `matches_logically`
  只查「模式要 Shift 而没按」，按下 Ctrl+Shift+N 同样满足 Ctrl+N，顺序反了就变成新建思维导图（真撞过，轨迹里写着
  「已新建思维导图」），单测 `the_shifted_chord_is_swallowed_by_the_plain_one_unless_judged_first` 钉住；③ 主卡环改画卡内 2px。
  证据 `shots/x100/SE-03/`：`tab-order.txt`（16 次 Tab = 四卡 → 最近文件×3 → 清除 → 模板×6 → 绕回新建思维导图）、
  `ctrl-shift-n.txt`（`home new concept kind=conceptMap`）、`ctrl-shift-n-doc.txt`（文档内新开概念图标签）、
  `ctrl-n.txt`（Ctrl+N 仍归新建思维导图）、`ring-probe.txt`（环 (47,111,94) 落在卡内 1–2px，卡外四方向 0 个强调色像素）。
  顺带修**金样确定性**：`start-empty` / `start-narrow` 原来跟着本机真实 `recent.json` 漂（同一份代码只因真实列表多了几条，
  空态基线整页对不上，实测 173100 px），harness 统一走空列表 + `MW_START_DEMO` 播种，修完只剩焦点环 2300 px。
  新单测 `cards_stay_between_180_and_the_page_width`（10 个断点）；测试 937 → **939**。
  外观漂移：8 张起始页基线（`start` / `start-empty` / `start-skeleton` / `start-concept` / `start-ink` / `start-mist` /
  `start-sand` / `start-narrow`）按 `-Update` 重拍，之后双跑 `--tol 0` 零差异。
- 2026-10-29 23:34 · **第 20 轮 · P 轨 SE-02（起始页视觉与状态）**。开工先审计：对着 `docs/30` §3.3/§3.4
  逐条查代码，结论是**视觉条款早在 SE-01 就落地了**（hairline 卡底无阴影、主卡 8% 强调底 + 强调色图标、
  六个结构图标、最近文件时间 secondary、空状态左对齐两行、`every_icon_stays_inside_the_sixteen_unit_grid`
  覆盖 37 个图标）；真缺的是「3 行加载骨架」与三组状态/主题证据（空状态、悬停、四主题）。
  做法：① `recent::Recent::load_async`（后台线程读盘）+ `pending()`（占位空列表，`file` 仍指向默认位置，
  先到的 `touch` 不丢）；`App` 每帧收结果并记 `recents loaded n=`；② `start::recents_skeleton` 画 3 行骨架，
  用 `recents_height(3)` 与三行真列表**同高**（到货不跳段），整块 `Sense::hover` 不吃点击，加载中不显示「清除列表」；
  ③ 金样要确定：harness 仍同步读，`MW_RECENTS_PENDING_FRAMES=<n>` 假装没读完，`start-skeleton` 取帧在预算之内；
  ④ `quick_card` 把卡片矩形打进轨迹（`quick rect concept 743,199 211x72`），悬停脚本不用猜坐标。
  新基线 6 张：`start-empty` / `start-skeleton` / `start-concept` / `start-ink` / `start-mist` / `start-sand`（106/212 场）。
  证据 `shots/x100/SE-02/`：骨架 vs 真列表 px>8=12386、bbox (536,345)-(1387,514) 只在最近文件卡内；
  指针压概念图卡 px>8=12766、bbox=(743,199)-(953,270) 正好是卡面、卡外 0 差异；四主题整页均色
  ink(32,35,40) / mist(239,244,241) / sand(242,237,228) / default(244,241,236)。
  新增单测 2 条（异步读与占位列表），测试 935 → **937**；stage-b 门 100/200 → **106/212** 全过，fmt 干净、clippy 新增 0。
  **未做**：真实运行里骨架只在读盘那一瞬间存在（本机 recent.json 很小，通常不到一帧），大列表的手感没量过；
  SE-03 的键盘与断点不在本包。
- 2026-10-29 23:00 · **打磨轮 19 · M13：启动时的取景用的是没落定的视口**。第 18 轮收尾时发现：同一份包围盒
  `(-346,-246,738,498)`，首帧 `fit` 算出 `zoom=1.2369`（视口 `1280×728`，默认窗口），窗口被
  `sized_to_monitor` 拉到 `1920×1128` 之后应是 `1.5`（`FIT_MAX_ZOOM` 钳位）——「打开即适应」一直比它该有的小一档。
  做法：`App::start_refit_frames`（60 帧预算）+ `Canvas::fit_viewport`（上一次取景用的视口）；
  启动阶段**只在视口和上次取景时不一样**时 `request_fit()`，`set_camera` 也记一份（阅读模式/场景夹具的相机不被覆盖）；
  `fit` 轨迹补 `vp=`。证据 `shots/x100/M13/`：轨迹从「一次 `fit`、`zoom=1.2369`」变成
  「`1280×728 → 1.2369`、`1920×1128 → 1.5`」两次；新 `cm-default` 与同一文档手动 `Ctrl+1` **整图 0 像素差**。
  **漂移 60 个文件 / 58 个场景**（31 张 `cm-*`、27 张 `mm-*`、`shell-ppp1.00/2.00`），全部是「打开就有文档、
  靠自动取景」的场景，没有一张固定相机的场景被改动；`ci.ps1 -Update` 与两道门 `-Update` 重拍后，
  stage-b 100/200、stage-cm 48/96 双跑全过，测试 935、fmt 干净、clippy 新增 0。
  **弯路如实记**：第一版「头 12 帧每帧都重取」把「窗口第几帧落定」写进了结果——`cm-arrow-menu-click`、
  `cm-focus-question-hover` 两张基线在两遍之间自己不同；改按「视口变了才重取」后消失。
  另：`scripts/x100-stage-b-gate.ps1` 补上 `-Update` 开关（此前只有 stage-cm 有，stage-b 的 100 场只能手拍）；
  重拍时两次撞上 `os error 1224`（写基线时文件被映射占用），那两张没写成，但第二遍比对全过说明它们本来没漂。
  **未做**：60 帧只是预算（更慢的机器没测）；这一轮没有单测（`start_refit_frames` 挂在 `App::update` 帧循环上，
  单测没有无头构造 `App` 的路径）。
- 2026-10-29 21:01 · **打磨轮 18 · C7：撤销整理布局时连取景一起还回来**。账本第 10 条：整理会 `fit` 到新布局，
  撤销只还原位置，用户按 `Ctrl+Z` 后看到「原来的散布 + 整理后的取景」。做法：`Canvas` 记
  `arrange_undo_depth`（整理落地时的栈深）与 `arrange_redo_fit`；撤销时比对撤销前的栈深，踩到的正是整理那一条
  就在 `rebuild()` 之后 `request_fit()`，重做同样 `fit` 并把栈深重新挂上。**不存旧相机**：整理可能跑在首帧
  `fit` 之前，那一份相机不是用户看到的取景；取景按撤回来的位置重算才是说得清的定义。
  证据（`shots/x100/C7/`）：`undo-fit-trace.txt` 记 `concept-arrange n=6 moved=6` → `undo label=摆放概念 left=12`
  → `undo fit=1` → `fit zoom=1.5000 center=(23.00,3.00) bounds=(-346.0,-246.0,738.0,498.0)`；
  `pixel-diff.txt` 记「整理 + 一次 `Ctrl+Z`」与「同一文档手动 `Ctrl+1`」的**画布行 100–1099 最大差 0**，
  全图 1828 px（>8）只在顶栏（缩放读数 124% vs 150%、撤销可用态）与左下提示条；`cm-arrange-undo` 重拍后双跑
  `--tol 0` 零差异。新增 `fit zoom= center= bounds=` 轨迹（`canvas_methods_2::fit_bounds`）。
  **顺带修掉一处确定性问题**：提示条原先按 2.4s 墙钟过期，离屏跑得快就进基线、跑得慢就不在——
  `cm-arrange-undo` 第一版基线就是带着它拍下来的。现在 harness 里不按墙钟过期（`app_methods_3::toast_bar`），
  这张图也第一次给了提示条像素覆盖。机器：**935 测试**、fmt 干净、clippy 新增 0、stage-b 100/200 与 stage-cm 48/96 全过。
  **本轮没有新单测（如实记）**：`arrange_concepts`/`undo` 要真实 `Canvas`（wgpu 渲染态），单测里没有无头构造路径。
  **发现未做**：首帧 `fit` 用的是没落定的视口——同一份包围盒 `(-346,-246,738,498)`，`cm-default` 首帧算出
  `zoom=1.2369`，稳定视口上是 `1.5`（`FIT_MAX_ZOOM` 钳位），「打开即适应」一直比它该有的小一档；
  已记进账本 §3 第 11 条，下一轮候选。
- 2026-10-29 20:28 · **打磨轮 17 · 补齐五个包的 manifest（无代码改动、无门）**。
  `docs/STATUS.md` §6 第 4 条挂了很久：`SE-01`、`CM-01/02/03/10` 只有 `report.md`，没有 `manifest.json`，
  而完成定义要求「报告 + manifest」。按各包报告**原文**回填五份，每份 `_note` 写明是补记、不新增结论；
  场景清单逐个核对 `shots/golden/` 里是否真的存在——顺带发现报告里两个失效名字
  （`cm-relink-sel-only` 实际叫 `cm-relink-select-only`、`Commit::SetRelationshipHead` 实际是 `Command::…`），
  在 manifest 里用了正确的。状态不与报告打架：`CM-02`、`CM-10` 仍是 `进行中`。
  这条路不跑测试也不碰外观，只为把「证据目录不齐」这条债清掉。
- 2026-10-29 21:05 · **打磨轮 16 · M12e 第二步：结构撤销的账不再被扔掉（无视觉改动）**。
  第 15 轮落地后文字撤销/重做进了 P9（15.5 / 15.4ms），但「加一颗再撤掉」只到 28.9 / p95 31.1ms。
  查出来不是布局慢，是**账被自己扔了**：加节点后当前代的**内容指纹**（含结构）变了、进不了候选；
  `put_memo` 又只把「没用上的候选」退成上一代，于是「改之前」那份账当场被丢，撤销时手里没有能对上的账。
  **做法**：候选只挑「指纹对得上」的那几代，**对不上的留在手里**——没有别的可留时，把最近的那代
  （哪怕这轮对不上）退成上一代；它记着的正是另一个真实存在过的内容状态。
  **实测**：20k 结构撤销 **28.9 → 中位 15.8 / p95 17.1ms（P9 30ms 达标）**；
  文字撤销 15.5、重做 15.4、打字 15.9 全部持平（两代账没有拖慢主路径）。
  **新单测** `a_structural_undo_reuses_the_previous_generation`（加一颗 → 重建 → 撤销 → 重建，
  断言行数据是同一份 `Arc`）；第 15 轮那版实现正是让它红的形态。
  **顺带修的门禁摩擦**：clippy 基线手工补了 `glyph.rs:281` 那一条——同一个 `let glyph = glyph;`
  在 lib 与 lib-test 两次检查里偶尔只报一处，ci 的「集合比对」因此偶发假红（今天已经三次）；
  两处都写进基线后，少报只算 gone、不算新增。
  **数字**：测试 934 → **935**；两道门 100/200 与 48/96 全过、152 场零漂移。证据 `shots/x100/M12e/` §8。
- 2026-10-29 20:04 · **打磨轮 15 · M12e 撤销/重做走缓存账（无视觉改动）** · 账本 M12 行的 P9 残账。
  动手前先纠正一个口径：应用的 `Ctrl+Z` 走 `build_scene_cached`，而 `perf_scene` 原来的撤销例程量的是
  **无缓存**路径——43ms 里有一半不是用户真实付的。按真实路径量下去，付全量代价的是**结构撤销**
  （加/删节点让可见序列变了，当前代的账整份作废 → resolve 8 + layout 20 + nodes 7）。
  **做法**：`SceneCache` 留**两代**账；关键一行是「这一轮建账用掉的那一代也要退成上一代」——
  内容指纹不含文字，改了一个字时它照样算「匹配的当前代」，可它代表的正是改之前那份内容（撤销要退回的）。
  撤销/重做前 `Canvas::undo/redo` 调 `prefer_previous()`，让下一次重建先试上一代；
  两代都可用时按「复用得更全」选，只差一颗就不再试另一代（免得白跑一遍布局）。为此
  `compute_layout_memo_with_reach` 额外返回复用颗数。
  **实测**（20k 合成语料，全部走缓存路径）：文字撤销 43.1 → **中位 15.7 / p95 17.5ms**、
  重做 **15.3 / 17.0ms——P9（30ms）达标**；5k 2.8ms。**结构撤销 28.9 / p95 31.1ms——差 1ms 未达标**：
  结构一变内容指纹就变，`resolve`（8ms）整段重算属实白付，这是下一刀。打字路径持平（15.9 → 16.2）。
  **新单测** `undo_hits_the_previous_generation_of_the_memo`（`Arc::ptr_eq` 钉住「撤销后是同一份测量」），
  第一版实现（用掉的那代没留住）当场被它抓住。
  **数字**：测试 933 → **934**；stage-b 门 100 场 / 200 次比较、stage-cm 门 48 场 / 96 次比较，
  152 个场次零漂移；clippy 新增 0（基线重生成吸收行号漂移）。证据 `shots/x100/M12e/`。
  **风险**：账本内存翻倍（20k 下几十 MB 量级），未按节点数封顶。
- 2026-10-29 19:44 · **打磨轮 14 · M12d 20k 打字最后一公里：索引不再每轮重建（无视觉改动）** · 账本 §3 第 7 条收尾。
  M12c 的账上还剩一段纯浪费：`index` 是 `ids → 下标` 的哈希表，而「只改一个字」时 `ids` 逐字相同，
  场景却每轮按最终 `ids` 重建一遍——两万次 `String` 克隆 + 两万次插入，实测 1.1–1.3ms。
  **做法**：① `build_scene_cached` 只在**真的并进了自由主题**时重建索引（那次索引确实会变），
  否则直接搬 `Layout::index`；② `compute_layout` 的 collect 栈从 `(String, u32, Option<usize>)` 改成
  `(&Node, u32, Option<usize>)`，不再为每个孩子克隆一份 id（20k 树两万次本可避免的分配）。
  **实测**（同机两个 release 二进制交替 A/B，各 3 轮）：P7 口径 `[scene] total`（20k 改字、缓存路径、40 次）
  **p50 14.49 → 13.00、p95 16.07 → 13.90、max 16.67 → 15.55ms——p95 首次进 16ms**；
  端到端中位 **18.3 → 16.7ms**、p95 **19.9 → 18.4ms**，**端到端仍超预算**，差额在 `[scene]` 之外
  （账本克隆约 1.2ms + 命令）。改后分阶段：resolve 2.6 / layout 6.5（collect 2.4、measure 1.6、bounds 1.15）/
  index **0.00** / nodes 1.5 / order 1.0。
  **新单测** `the_scene_index_agrees_with_ids_whether_floaters_are_merged_or_not`，做过**变异检查**：
  把沿用条件改成无条件沿用立刻红（概念图分支索引变短），恢复即绿。
  **数字**：测试 932 → **933**；stage-b 门 100 场 / 200 次比较、stage-cm 门 48 场 / 96 次比较，152 个场次零漂移；
  clippy 新增 0（基线重生成吸收 8 条行号漂移；另修掉一版「新测试插在旧测试的 `#[test]` 与函数之间」
  导致的 duplicated attribute / never used）。证据 `shots/x100/M12d/`。
  **未做**：端到端那 1ms（账本克隆 + 20k 次 `String` 分配，要 `Arc<str>` 化 `Scene::ids/index`，动 ~150 处调用点）；
  `bounds` 与 `order` 仍是每轮 O(n)。
- 2026-10-29 14:16 · **打磨轮 13 · C7 整理布局的提示与撤销动线（1 张新基线）** · 账本 §1 C7 行。
  账本写「提示与撤销动线只有单测」，实查两头都空：`concept_layout` 的**纯算法**有单测，但「整理发出的是不是
  一条 `Batch`、一次 `Ctrl+Z` 能不能整张回来」一条测试都没有；右键菜单落地后也**没有任何提示**——
  用户看到画面一跳，不知道刚动了什么、能不能退。
  **做法**：① 命令组装抽成纯函数 `Canvas::arrange_batch(ids, current, placed) -> Option<(Command, usize)>`：
  位置没变的不出命令（一条都没动返回 `None`）、一条以上合成 `Command::Batch`（撤销栈里仍是一条）、
  返回「动过几颗」；`arrange_concepts` 由 `bool` 改 `Option<usize>`。② 菜单落地点补 toast
  「已整理 N 颗概念 · Ctrl+Z 撤销」，只在真动了时弹；给 `set_toast` 加一行 `toast bad= text=` 轨迹——
  提示条按 2.4 秒墙钟消失，拍进 golden 会和时钟赛跑，所以 toast 的证据从此走轨迹。
  ③ 新单测 `arranging_three_concepts_is_one_undo`（`Batch` / `undo_depth` 只 +1 / 一次撤销三颗全回 / 没动返回 `None`）；
  新基线 `cm-arrange-undo`（整理 → 一次 Ctrl+Z）。
  **证据链**：真菜单路径 `menu_activate key=enter label=整理布局` → `concept-arrange n=6 moved=6` →
  `toast bad=false text=已整理 6 颗概念 · Ctrl+Z 撤销`；撤销 `concept-arrange n=6 moved=6` →
  `undo label=摆放概念 left=12`（一条）。`cm-arrange-undo` 与 `cm-arrange` 差 40022 px、
  与 `cm-default` 差 40161 px——差的主要是**镜头**（整理 `request_fit` 过，撤销不回相机），如实记。
  **数字**：测试 931 → **932**；stage-b 门 100 场 / 200 次比较、stage-cm 门 47 → **48 场 / 96 次比较**，
  既有 151 张基线零漂移。**一次假红**：第一遍 stage-cm 第 2 轮在 `cm-arrow-menu` 上 2 个像素超容差
  （最大差 7，x[592..592] y[522..523]），整门重跑 48/48 双跑全过——门槛脚本注释里记过的同类现象，与改动无关。
  **未做**：撤销不还原相机；实机键鼠手感未核（故只记 `已实现`）。证据 `shots/x100/C7/`。
- 2026-10-29 13:50 · **打磨轮 12 · C3 关系端点拖拽的落点余量（有视觉变化：拖拽途中多一个目标框）** · 账本 §1 C3 行。
  账本写的是「手感（阈值/吸附）没量」，量下去发现不是手感而是**行为缺陷**：`mw_ui::link::target_node`
  要严格矩形包含，指针离目标框 1–10px 就判落空；落空在 CM-10 是正经命令 `RetargetLinkToNewConcept`
  （在落点新建概念并改线），于是「想改指到那颗」变成「多长一颗概念」。
  **做法**：新增 `target_node_snapped(scene, from, at, snap)`——压中轮与旧判定逐位一致（仍取面积最小），
  压中为空时按到矩形的欧氏距离取 `snap` 以内最近者（同距取面积小、再同取 `draw_order` 靠前）；
  `target_node` 变成余量 0 的包装。余量 `LINK_SNAP_CSS = 12` css px，经 `camera.css_to_world` 换算，
  缩放后仍是 12 屏幕像素（示例图镜头 0.808× → 世界 9.7）。两条鼠标路径都换：思维导图连线拖
  （`DragKind::Link`）与概念图改端点拖（`DragKind::Relink`），拖拽途中画的就是真实落点。
  **证据**：离屏夹具 `MW_CONCEPT_RELINK_DRAG` 原来写死 `target: None`（拍出来永远是落空预览），
  改成走同一判定，`relink-preview` 轨迹补 `target=`——同一帧 A/B（只切半径 0 ↔ 12）**6991 px**，
  包围盒 (557,741)–(806,839)；新基线 `cm-relink-near`（指针停在「增长策略」框外 6 个世界单位）。
  单测 +3（近失只吸在余量内 / 压中不被更近的框外节点抢走 / 最近者胜且起点不吸自己）。
  **数字**：测试 928 → **931**；stage-b 门 100 场 / 200 次比较、stage-cm 门 46 → **47 场 / 94 次比较**，
  既有 150 张基线零漂移；clippy 新增 0（顺手把打磨轮 11 留下的 9 条行号漂移重生成进基线，种类 0 变化）。
  **未做**：实机鼠标手感（12px 是 UI 惯例，没在真机上试过）——所以本包只记 `已实现`，不记 `已验证`。
  证据 `shots/x100/C3/`。
- 2026-10-29 13:07 · **打磨轮 11 · M12c 20k 真增量重建（无视觉改动）** · 账本 §3 第 7 条后半。
  上一轮的账摆明：20k 一轮 25ms 骨架价里，`resolve_all` 5.5ms 与「缓存命中的测量查表」5.4ms
  **与改了什么无关**，场景节点装配 7.5ms 也是同一批数据每轮重拼一遍。
  **做法**：`SceneCache` 里留一份「上一次的账」——内容指纹（样式 + 备注/链接/卡片 + 图标/标签/图片 +
  层级序号，**刻意不含文字**）、结构指纹（可见节点序列与深度）、每节点盒子输入指纹（文字/图标/标签/图片）、
  盒子表、组装好的 `SceneNode` 表。逐项比对，对不上就整份作废走全量，所以「算错」只退化成「慢」。
  **实测**：20k 打字 `reused=19999/20000`（只有改字那颗重测），缓存路径重建 **29.0 → 18.6ms**
  （p95 20.4），5k **4.7 → 3.5ms**；P7 的「布局函数计时」口径 `[scene] total` **p50 15.5 / p95 16.9ms**，
  **p95 差 0.9ms 未达标**，如实记在 `shots/x100/M12c/report.md` §7。分阶段：resolve 5.5 → 3.0（剩的是
  内容指纹本身）、measure 5.4 → 2.3、nodes 7.5 → 1.5；剩下的 collect 3.0 / index 1.2 都是每轮两万次
  id 克隆，要再降得把 `Scene::ids` / `Scene::index` 改 `Arc`（API 级改动，下一轮）。
  **单测当场抓到两个真 bug**：① 第一版内容指纹没覆盖备注/链接/卡片，加备注后画布上看不见；
  ② 第二版盒子指纹漏了图标，改图标仍命中旧盒子、宽度画错（旧单测 `the_box_cache_remeasures_…` 抓到）。
  新增三条单测（备注改了必须重新装配 / 结构变了账作废 / 同一文档连画两遍共用行数据），
  测试 925 → **928**；fmt 干净、clippy 指纹 37 条新增 0；两道门顺序跑全过
  （stage-b 100 场 / 200 次比较 `gate-b-m12c.log`，stage-cm 46 场 / 92 次比较 `gate-cm-m12c.log`），
  **146 个场次零漂移**——增量路径画出来的像素与全量路逐字节一致。证据 `shots/x100/M12c/`。
- 2026-10-29 07:58 · **打磨轮 10 · M12b 20k 重建分阶段账（无视觉改动）** · 账本 §3 第 7 条只说「20k 打字 47ms」，
  没有构成就没法判断「砍常数够不够、增量能省哪几段」。`MW_LAYOUT_LOG=1`（新证据开关，避开
  `MW_PERF_LOG` 已被启动里程碑占用）给 `compute_layout` 与 `build_scene_cached` 各加一组里程碑，
  20k 缓存路径一轮 = `resolve 5.3–5.7 / layout 10.0–10.8（measure 5.4–6.2、collect 2.5–2.9、bounds 1.0–1.4）/
  index 1.0–1.2 / nodes 6.6–7.1 / order 1.1–1.3`，合计 25–26ms；同时打出测量缓存
  **799,960 命中 / 0 未命中**——全命中也要 25ms，钱花在「每节点每轮查表 + 装配」的骨架里。
  据此砍两笔：① `BoxSpec.lines` / `NodeBox.lines` 改 `Arc<[String]>`（命中只加引用计数）+
  测量循环改借用不再克隆文本；② `resolve_all` 不再逐节点 `clone` 样式覆盖
  （带样式的 XMind 导入文档每轮少克隆几万个 String，20k 全带样式 **10.5 → 7.4ms**，同机 A/B）。
  结果：20k 改字重建 **46.1 → 41.4ms**、缓存路径 **34.1 → 29.0ms**（p95 30.3），5k 缓存路径 5.6 → **4.7ms**；
  perf 例程新增「全带样式」语料与样式级联计时、缓存命中数输出。**否决**：FxHasher（rustc 风格快速哈希）
  换三处 SipHash，同代码双构建交替 A/B 四轮 = 38.9/37.9/39.1/39.8（候选）对 31.7/32.8/32.2/33.4（现行）——
  候选慢 6–7ms，全部撤回；散列不是瓶颈，得少查几趟（增量）。
  新单测 `a_cache_hit_shares_the_line_storage_instead_of_cloning_it`（`Arc::ptr_eq` 钉住「未变节点复用同一份行数据」）；
  测试 924 → **925**；fmt 干净、clippy 指纹 37 条（重生成后逐行核对，14 处全为行号平移、无新增告警）。
  证据 `shots/x100/M12b/`（report + manifest + perf-scene.txt + perf-phases.txt 只留 20k 行）。
  **门**：stage-cm **PASS**（46 场 / 92 次比较，零 FAIL）；stage-b 两道插曲——
  ① 基线指纹是「标题 @ 文件:行」，theme.rs 删一行 import 后行号平移，老告警报成新告警；
  ② 修 `scripts/x100-stage-b-gate.ps1` 的豁免规则：从「文件:行:列」改成**标题 + 文件**（Rgba::rgba 那条历史告警
  不再随行号漂移，新增告警照样抓），重跑门。
  **踩坑（重）**：修门脚本时用 Python 重写文件把 **UTF-8 BOM 写丢了**——PowerShell 5.1 读无 BOM 的 .ps1
  按 ANSI 解码，五个带中文环境值的场景（outline-search 的 `MW_OUTLINE_SEARCH='排版'`、
  outline-20k-deep、cm-crosslink、cm-card-link-back、card-autosave）把乱码传给应用，门第二次红
  （14225/1749600 像素）。补回 BOM（`ï»¿`）后重跑。**教训：改带中文的 .ps1 必须保留 BOM，
  改完 `git show HEAD:<file>` 比对首字节。** ③ BOM 修好后仍剩 `card-autosave` 一场红：53 px（0.002%，最大差 91，包围盒 639,167–646,176），同一栅格位置的**一个 CJK 字形抗锯齿覆盖变了**。用干净 HEAD 构建（`git worktree`）跑同一场景，**53 px 一模一样** ⇒ 与本轮代码无关，是环境级渲染差异；连跑三次逐字节相同 ⇒ 稳定。按规程重拍这一张基线（`--update`，旧基线备份 `target/x100/card-autosave-baseline-before.png`），重拍后连跑两遍 `--tol 0` 全过，新旧基线差异恰为那 53 px。
  **P7 16ms 仍未达标**：下一刀必须是真增量
  （结构指纹 → 复用 ids/styles/盒尺寸 → 只重测编辑中的那颗 → 原地更新 Scene），不是再砍常数。
- 2026-10-29 04:26 · **打磨轮 9 · M7 框选（有视觉改动：两张新基线）** · 账本原话「框选阈值 4px 的手感未量」。
  先查证据：M7 行的证据列写的是 `mm-boundary-select` 等——那是**边界**的图，**橡皮筋自己一张都没有**。
  跑一遍发现比阈值更要紧的一件事：`marquee_pick` 只在**松手那一刻**调用，橡皮筋拉了半个屏幕，
  用户也不知道框住了什么，只能松手看结果再拉一次（XMind 是拖动中实时高亮）。
  **改法**：拖动中每帧算一次 `marquee_pick`，把「松手会选中谁」临时并入 `Frame.selected`，
  命中节点实时挂选中环；预览集合只在**变化**时写一行 `marquee preview picked=… selected=…`。
  新增输入的 `mods ctrl|shift|alt|command|none` 动词——拖拽的 `additive`/`subtract` 在**按下那一刻**就读走，
  `sclick` 只够管单击；拼错报错不静默。
  **量到的**：① 阈值——2px 拖动**零** marquee 事件（按单击空白处理），5px 才 `marquee picked=0 selected=0`；
  比较发生在 egui 的 points 空间，屏幕常量，不随缩放变。② Ctrl 加选——框外先选一颗，`preview selected` 走
  **1 → 3 → 5**，松手 `marquee picked=4 selected=5`，预览与结算一致。
  ③ 换算——新单测 `marquee_pick_maps_the_screen_box_through_the_camera` 在 0.5× / 1× / 2× 三档下
  用 `world_to_screen` 把世界盒换到屏幕再框回去，必须选中同一个节点。
  **A/B**：同一段脚本临时关掉预览重拍再逐像素比，开预览比关预览多 **3975 px（0.227%）**，
  最大差 205，包围盒 `x[422..765] y[426..571]` 正是框内节点（含环外扩），不是橡皮筋矩形。
  **踩坑**：第一版只改了 `Frame.selected`，`select_alpha` 的目标值仍只看 `self.sel`，透明度停在 0——
  A/B 是**零差异**，由此才发现预览压根没画出来；把预览并入 `select_target` 后才真的可见。
  两张基线 `mm-marquee`（未松手定格）/ `mm-marquee-add`（框外加选），stage-b 门
  98 场 / 196 次 → **100 场 / 200 次比较**，既有 98 张零漂移；stage-cm 门 46 场 / 92 次仍绿；
  测试 922 → **924**，fmt 干净，clippy 37 / 新增 0。
  **未做**：实机手感、20k 文档拖动时的帧率（拖动中每帧一次 `intersecting`）、Alt 减选与套索的场景。
  证据 `shots/x100/M7/`。
- 2026-10-29 04:04 · **打磨轮 8 · M9 折叠圆点（无产品行为改动，四张新基线）** · 账本原话「折叠动画与点击热区」。
  先查证据链：`blend_scenes` 的中点有场景层单测（`blend_collapse_slides_children_into_parent_and_edges_follow`），
  但**壳层到屏幕这一路没人拍过**；热区只有 `toggle_center_hits_toggle_and_beats_parent` 一条，
  把 `TOGGLE_HIT_CSS` 错写成世界单位它照样过（圆心在任何写法下都命中）。
  **量到的结论**：① 折叠**有**过渡——同一段脚本用 `MW_MOTION_T` 钉 0.0 / 0.5 / 1.0 与终态比像素，
  分别差 **34374（1.965%）/ 27913（1.595%）/ 0**，包围盒 `x[339..829] y[429..663]` 正落在被收起的
  两条分支与连线上；终点逐字节一致，说明过渡不会停在半路。② 圆点画出来 `r8 + 1.5px` 底色光晕、
  命中半径 `TOGGLE_R + 2 = 10 CSS px`：点圆心正下方 9px（环外、半径内）仍触发折叠；12px 那一击
  `canvas_click hit=none`（连节点都没选中，因为那点在盒外），不误折叠。
  **新增**：`toggle idx=… collapsed=… kids=…` 轨迹（圆点与盒边界的命中带只有几像素，截图看不出来）、
  单测 `toggle_hit_ring_keeps_its_screen_size_across_zoom`（0.5×/1×/2×：`8/zoom` 世界单位命中、
  `12/zoom` 不命中——半径错写成世界常量会立刻挂）、四张基线 `mm-collapse` / `mm-collapse-mid`
  （定格 t=0.5）/ `mm-toggle-edge` / `mm-toggle-miss`。
  stage-b 门 94 场 / 188 次 → **98 场 / 196 次比较**，既有 94 张零漂移；stage-cm 门 46 场 / 92 次仍绿；
  测试 921 → **922**，fmt 干净，clippy 37 / 新增 0。
  **注意**：`mm-collapse` 与 `mm-toggle-edge` 两张基线**逐字节相同**（`84BE6968C60C`）——热区的证据在
  `toggle` 轨迹里，不在图上；别以为第二张是重复拍的。
  **未做**：实机手感（触控笔、150%/200% DPI）、整图折叠 `set_all_collapsed`、聚焦模式下的圆点。
  热区半径是原值，没有手感数据之前不动它。证据 `shots/x100/M9/`。
- 2026-10-29 03:49 · **打磨轮 7 · M6 键盘移动（有视觉改动：三张新基线）** · 账本原话只有一句
  「连续按多次的节奏没人看过」。脚本连按 `Alt+↓` 三下、逐帧 dump 节点盒，看到的不是节奏而是两处不一致：
  ① **层级命令不跟镜头**——折叠、方向键遍历、`Tab` 新建、右键菜单都会 `reveal_node`，偏偏 `Alt+↑↓`、
  `Ctrl+Shift+←/→` 不会；镜头被中键拖走之后（节点在 `(-716,-428)`，整个盒子在视口外）按 `Alt+↓`，
  节点仍在原地，连按十几下只会越走越远；② **多选被忽略**——框选两颗按 `Alt+↓` 只动主选那一颗，
  而大纲页与右键菜单走的是 `BatchOp` 批量命令，同一件事两条路行为不同。
  **改法**：`reveal_id`（`scene.find` → `reveal_node`）接到这三条命令上；`move_selected` /
  `promote_selected` / `demote_selected` 先看 `selected_ids()`，多选走 `move_step_many` /
  `outdent_many` / `indent_many`，单选保持老的单条路径 + 跟镜头；`apply_batch_structure` 成功后同样跟镜头
  （主选不在了退第一个），失败时写 `structure_batch op=… n=… command=none`。
  **证据设施**：`structure move=±1 selected=N path=single|batch`（分辨走哪条路）、
  `marquee picked=N selected=M`（框到几颗 ≠ 选上几颗，被覆盖的后代会被丢掉）。
  **新测试**：`single_selection_moves_the_same_way_through_both_paths`——`move_command`（单条）与
  `batch_structure_command`（批量只给一颗）必须给同一棵兄弟顺序，到边界两条都不动
  （第一版直接比 id，两份文档 id 各生成一次，必挂；改按文字比）。
  **门与基线**：新增 `mm-move-keys` / `mm-move-multi` / `mm-move-keys-reveal` 三场，
  stage-b 门 91 场 / 182 次 → **94 场 / 188 次比较**，既有 91 张零漂移；stage-cm 门 46 场 / 92 次仍绿。
  测试 920 → **921**，fmt 干净，clippy 37 / 新增 0（`non_shorthand_field_patterns` 一处当场改掉）。
  **踩坑**：`mm-move-keys-reveal` 第一版不成立——只拖了 600×500 px，节点停在视口顶端 `(134,-118)`
  仍然可见，`reveal_center` 判「已在视口内」不动相机，改与不改一个样；改成从 `(1500,1000)` 拖到 `(150,120)`，
  改前 `(-716,-428)`、改后 `(48,96)`，差 700 多像素才真的锁住这条行为。**另外**：`mm-move-multi`
  第一版框进了根节点（`picked=5`），`BatchOp::MoveDown` 见「下一个兄弟也在组里」直接 `command=none`；
  改成只框左侧两颗兄弟。
  **产品行为变化**：整组移动是一步撤销（`undo label=移动节点` 一次复原）；层级命令现在会跟镜头。
  证据 `shots/x100/M6/`。
- 2026-10-29 03:25 · **打磨轮 6 · M12 大文档重建成本（无视觉改动）** · 账本原话「5k/20k：增量重排、平移不重排；
  真实节拍下的帧率未量」。先分开验这两句：**「平移不重排」成立**——20k 文档里中键拖三次 + 滚轮进出一格，
  `scene_build` 轨迹**零次**（唯一那行是打开文档时的 155ms）；**「增量重排」从来没有**——改一个字、
  撤销一次都是整棵重建。最要紧的一条顺带量到：**打字每敲一下就重建一次**，
  5k 9.3ms（还在 60fps 内）、20k 47ms（约 21fps，光标明显落后于手指）。
  **改法（只去常数、不动架构）**：① `SceneCache` 的键从 `format!`（含三个 `{:?}`，20k 每次重建格式化两万次）
  换成 `BoxKey` 结构体逐项比较，命中零分配；② 没有边界/概要/关系线的文档整段跳过（不再克隆两万个 id
  跑 `doc_order()`，10.2 → 0.3ms）；③ 布局结果整块搬移，不再克隆 `ids`/`boxes`/`edges`/`spines` 三份；
  ④ `resolve_all` 按节点数预分配。
  **数字**（`cargo run -p mw-core --release --example perf_scene`，12 次中位 / 改字 40 次）：
  5k `build_scene` 12.3 → **6.8ms**、改字 13.7 → **7.7ms**、撤销/重做 13.7/13.1 → **7.5/7.4ms**；
  20k `build_scene` 63.4 → **40.8ms**、仅布局 9.4 → 8.2ms、改字 66.9 → **46.1ms**（缓存路径 34.1ms）、
  撤销/重做 64.8/65.1 → **46.7/47.7ms**。真实壳层：5k 打开 37.38ms → 建子打字 9.33ms；
  20k 打开 157.11ms → 建子打字 47.05ms。20k 一次构建的剩余构成（临时插桩，已删）：
  `resolve_all` 5.9 / `compute_layout` 20.8 / `index` 1.3 / `nodes+order` 8.3 / `edges+spines` 0.25 /
  `advanced+bounds` 0.3。
  **证据设施**：`rebuild()` 新增 `scene_build nodes=… edges=… ms=…` 轨迹（只在 `MW_TRACE` 下写）；
  `press`/`release` 支持 `primary/middle/secondary` 前缀（老脚本一字不改）；perf 例程加「改字重建(缓存)」。
  **新测试**：`the_box_cache_remeasures_only_when_a_size_input_changes`（键少放一项会让盒子错位、
  多放一项会让换色白重测）、`press_can_name_its_button_and_defaults_to_primary`。测试 918 → **920**，
  fmt 干净，clippy 37 / 新增 0。
  **门**：stage-b **91 场 / 182 次**、stage-cm **46 场 / 92 次**全过，既有基线逐字节零漂移；本轮无新基线。
  **踩坑（重要）**：两道门**并发**跑会互踩 `shots/golden/<场景>/actual|diff`，各假报 8 场漂移
  （`outline-hover-select`、`tabs-wheel`、`cm-rel-hover`…），顺序重跑即全绿——以后一律顺序跑。
  **未达标 / 未做**：20k 仍 47ms（P7 16ms），真增量布局另立账本 §3 第 7 条；`scripts/m5-perf.ps1`
  本机这次 60s 内拿不到主窗口句柄，真实鼠标节拍的 P5 未复测；只量 1080p / ppp 1.0；
  卡片页与大纲的 20k 成本未量。证据 `shots/x100/M12/`。
- 2026-10-29 02:51 · **打磨轮 5 · M8 撤销节奏（新建 + 改字合成一条撤销）** · 账本判词只是
  「`Ctrl+Y` 与中文输入混排未验证」。按脚本先跑节奏，撞上更硬的：`Tab` 建节点发 `AddNode`（空文本），
  提交时再发 `SetText`，两条各自入栈——**一次 `Ctrl+Z` 只撤掉字、树上留一个空盒子**；
  「建三颗 + 撤干净」要按六次。用户眼里「建了一个节点写了字」是一个动作。
  **改法**：`ChangeSet::created(id)`（这一条是否新建了这枚节点）；`Document::apply_coalesced(cmd, merge)`
  ——合并判据由调用方给，返回 `false` 时与 `apply` 完全同行为（照常入栈、照常清 redo）；
  `commit_edit` 在 `Editor.fresh` 为真时把 `SetText` 并进上一条；`undo`/`redo` 各补一行栈深轨迹。
  **踩到并修掉**：第一版合并把上一条 `after` 整段覆盖（`SetText` 的 `after` 只含它碰到的节点），
  「父节点 `children` 多了一个孩子」那半条快照被丢掉——撤销正常、**重做只把节点塞回 `nodes`
  不挂回父节点，树上什么都看不见**（先由一张重做后没有节点的基线发现，轨迹里也没有 `scene_node` 行确认）。
  改成按 id 合并（这一条覆盖它的 id、其余留上一条的），单测补了「重做后要挂在父节点名下」。
  **证据**：四张基线 `mm-undo-built`（`3BDED658F32F`）/ `mm-undo-one`（`F790CD6E3700`）/
  `mm-undo-three`（`F469E2125A9C`）/ `mm-redo-one`（`4E51FDA4D475`），像素：丙矩形内一次撤销后 0 px，
  三次撤销后导出回到 `(958,606)`，重做后甲带字回来 `(958,640)`；轨迹 `undo label=… left=…` 显示一次动作只吃一条。
  **漂移**：新增 4 张基线，既有 87 张 stage-b 与 46 张 cm-* 逐字节未动；stage-b 门 87 → **91 场 / 182 次比较**，
  stage-cm 46 场 / 92 次比较；测试 916 → **918**，fmt 干净，clippy 37 / 新增 0。证据 `shots/x100/M8/`。
  **产品行为变化**：本次编辑里刚建的节点，一次 `Ctrl+Z` 整颗收回（已有节点改名仍一条一步）。
  **未验证**：实机 IME 组合期按 `Ctrl+Z`（离屏的「打字」是 `text` 事件，不是真组合）；只测 `Ctrl+Y`；
  卡片页 / 大纲行 / 检查器里的撤销节奏都没测。
- 2026-10-29 02:5x · **打磨轮 4 · C10 「排列」子菜单贴底夹取** · 账本判词「`place_submenu` 两条单测 /
  真贴底时的观感没图」。查下去：**单测那半过期**（现有四条，`submenu_clamps_up_when_the_parent_row_is_near_the_bottom`
  与 `submenu_clamps_to_the_top…` 都在；`menu.rs` 里也早有 `menu_submenu_rect` 形状轨迹），
  **图那半是真的**——全部菜单场景都在 `app_methods_1.rs` 里写死 `pos: egui::pos2(360.0, 120.0)`，
  贴着左上开，子菜单永远装得下，夹取分支在基线上一次都没走过。
  **改法**：只加镜头，不动逻辑。`MW_CONTEXT_POS='x,y'`（`canvas.rs::context_pos` / `parse_pos`，
  两个数都能解析成有限数才算数，否则当没设、退回默认点）挪菜单**请求点**；
  验收环境 `1280x520` 矮窗 + `MW_CONTEXT_DEMO=multi` 让 300 高（8 行 + 2 段标题）的「排列」子菜单必然溢出。
  **证据**：新基线 `cm-submenu-bottom`（sha256_12 `30CD9669D76A`，1920×780）；
  轨迹 `menu_open anchor=380,400 entries=9` 与
  `menu_submenu_rect row=387,584,615,614 pos=613,472 size=200,300 content=0,0,1920,780`——
  不夹取时 `pos.y` 应是 `row.top − PAD = 578`，`578+300 = 878 > 780−8`，夹到 `780−300−8 = **472**`，
  即上推 **106px**，底边正好留 8px；像素侧卡 `613–813 × 472–772`，卡底 772、窗口 780、末行内容在 `y≈760–769` 没被切掉。
  stage-cm 门 45 场 / 90 次比较 → **46 场 / 92 次比较**，双跑 0 差异，既有 45 张逐字节未动；
  stage-b 门 87 场 / 174 次比较仍绿。全量 **916 测试**（+1：`context_pos_reads_two_numbers_or_nothing`）、
  fmt 干净、clippy 37 / 新增 0。证据 `shots/x100/C10/`。
  **未做**：夹取后子菜单盖住父菜单下半部分（既有取舍，实机点父行下半的落点没验）、
  更矮窗口把子菜单顶到内容上边那一档仍只有单测、实机拖窗口重排、150/200% 档。
- 2026-10-29 02:0x · **打磨轮 3 · C9 概念图强调淡入的中间帧** · 账本判词是「只能拍稳态」，
  照它给的方子（「加一个只给验收用的 alpha 乘子，拍一张定格」）做下去，发现一张定格也拍不成，因为**三层断链**：
  ① 离屏金样一律 `reduce_motion = true`（`app_methods_1.rs` 的 `c.set_reduce_motion(... || harness.is_some())`），
  0.2–0.4s 的过渡一帧都不起；② `motion_span` 只量节点盒，概念图里**加一条命题线不挪任何盒子**，
  于是新线从来没有 `Motion`，`blend_scenes` 里那条 `r.alpha = t` 的淡入分支**永远轮不到**
  （`MW_CONCEPT_CROSS` 场景实测：钉了 0.5 拍出来与稳态**逐字节相同**，md5 `d85d4271…` = `26c5cdb4…` 的对照组）；
  ③ `paint_relationship_emphasis` 读的是 `self.scene`（目标场景），就算线在淡入，强调也按 `rel.alpha = 1.0` 画——
  正是 `emphasis_alpha` 注释里说要挡的「幽灵线」。
  **改法**：`MW_MOTION_T=<0..1>`（`motion_pin` / `parse_pin`，`NaN`/`inf`/空白当没设，其余夹到 0..=1）
  把 `blend_scenes` 的混合系数钉死；`motion_reduced(reduce_motion)` 只在设了钉死值时才放开**场景过渡**这一路
  （悬停/选中的 alpha 仍瞬时到位，画面照样确定）；`motion_span` 多认一条「关系线 id 集合变了」（跨度取 0 ⇒ 0.20s，
  按 id 建哈希表，上千条线不退化成平方）；`blend_scenes` 的共有节点/连线/关系线/边界/概要在插值时**同时插 `alpha`**
  （动画中途重建会把此刻那一帧固化成新起点，只固化位置、把透明度跳到 1.0，淡入会当场断掉）；
  定格模式下 `rebuild()` 不重取快照（否则 t 叠成 t²，钉 0.5 拍出来是 0.75）；强调改读插值帧。
  **证据**：新基线 `cm-link-fade-mid`（`MW_CONCEPT_LINK='用户痛点|风险与依赖'` + `MW_MOTION_T=0.5`，
  sha256_12 `3A43A517AA48`）对既有稳态 `cm-link-create`：逐像素差 1926 点、bbox `(599,442)-(924,854)`；
  沿新线量线身（行 600–859、列 640–929 逐行取最暗，248 行平均）**117.4 : 81.1**，
  取样像素 `(887,808)` 为 `(119,153,142) : (47,111,94)`——既不是纯 50%（≈150）也不是稳态，两条证据正好夹住它。
  新轨迹 `rel_emphasis id=… tier=… rel_alpha=… alpha=… w=…`（去重，只在 `MW_TRACE` 下写）给出
  `mid: tier=1.00 rel_alpha=0.500 alpha=0.500 w=3.0` / `end: rel_alpha=1.000 alpha=1.000`。
  stage-cm 门 44 场 / 88 次比较 → **45 场 / 90 次比较**，双跑 0 差异，既有 44 张逐字节未动；
  stage-b 门 87 场 / 174 次比较仍绿。全量 **915 测试**（+2：`the_motion_pin_clamps_and_ignores_junk`、
  `motion_span_sees_a_new_relationship_when_no_box_moves`）、fmt 干净、clippy 37 / 新增 0。证据 `shots/x100/C9/`。
  **产品行为变化**：概念图新增/删除关系线现在有 0.20s 淡入淡出（时长令牌未动，实机观感未核）。
  **未做**：定格是常量 t 不是实拍、只拍 0.5、定格下连续两条结构性命令表达不了、强调单独那一份只有 trace 没有像素、
  只拍 ppp 1.0。
- 2026-10-29 01:3x · **打磨轮 2 · M1/M2 新建节点后的编辑态** · 账本第二行原文是「编辑框位置与滚动没人看过」，
  实测发现比这更重：**合同没实现全**。`docs/07` §3.2 写的是「`Tab` 提交编辑 → 新建子节点」「`Enter` 提交编辑 →
  触发语义（新建同级）」，而代码里编辑态只接 `Enter`（且只提交）、`Tab` 根本不接——egui 的 `Tab` 是焦点导航，
  等我们读到输入时焦点已经交给壳层下一个控件，编辑框失焦顺带提交，树上什么都没建。轨迹里看得很清楚：
  按一次 `Tab` 之后 `focus id ae0e1c7decf967ec`（某个按钮），不是 `focus canvas`。
  **改法**：新纯函数 `commit_follow_up(enter, tab, concept_map)`（思维导图 `Enter`=同级 / `Tab`=子级，
  两者同帧到时 `Enter` 优先只建一个；概念图没有「同级」这层结构，两键都只提交，与 CmapTools 一致）；
  编辑态自己 `consume_key` 吃掉裸 `Tab`；跟随动作记在 `edit_follow_up`，**排在 `commit_edit()` 之后**跑，
  提交才会把同帧敲进去的字落进文档。顺手补「取消」语义：`Editor.fresh` 记下这枚节点是本次编辑建的，
  `Esc` 时用 `Command::DeleteNodes` 一起收回、选中退回父节点——以前会留一个点得到的空白盒子。
  **证据**：四张基线（`--tol 0` 双跑）`mm-new-child` 新节点 `(990,641)` 75×42、编辑环 `(986,637)-(1069,687)` **416 px**；
  `mm-new-chain` 甲 `(987,641)` 已提交、乙盒子 `(1157,646)` 76×33 + 环 **388 px**（盒内墨色 84 px = 「乙」）；
  `mm-new-cancel` 同窗口 **0 px** 节点底/环，全图唯一强调环是根的选中环 `(827,522)-(924,581)` 253 px；
  `mm-enter-sibling` 新同级 `(425,615)`（原节点 y=587 之下）+ 环 **384 px**。
  新轨迹 `editor_open id=… z=… node=(x,y,w,h) edit=(x,y,w,h) in_view=0|1` 与 `editor_close`，四份文本留在 `shots/x100/M1/`。
  顺手量了滚动：从根连按七次 `Tab`，七个新节点 `in_view` 全 1，屏幕 x 990 → 1490 后停在同一位置（`reveal` 每层生效）。
  stage-b 门 83 场 / 166 次比较 → **87 场 / 174 次比较**，双跑 0 差异；stage-cm 门 44 场 / 88 次比较仍绿
  （概念图 `Enter` 语义没动，`cm-node-edit` 等基线逐字节未变）。全量 **913 测试**（+1：`edit_time_enter_and_tab_build_the_next_node_on_a_mind_map_only`）、
  fmt 干净、clippy 37 / 新增 0。证据 `shots/x100/M1/`（report + manifest）。
  **未做**：实机连按节奏（新编辑器抢焦点那一帧、系统键重复）、实机微软拼音组合期归属、150/200% 档；
  `Esc` 收回在撤销栈里是两步（建一条 + 收回一条），记为 M8 的输入。

- 2026-10-29 01:0x · **打磨轮 1 · M5 拖拽落点指示器** · 账本 `docs/x100/polish-ledger.md` 第一行：三区落点
  （整框 / 上缘条 / 下缘条）与非法转红**只有实现、没有一张图**。本轮先把「现在是哪一态、目标是哪个节点」
  变成可读文本，再拍四张基线。
  轨迹（只在 `MW_TRACE` 下）：`scene_node <id> text="…" rect=(x,y,w,h)`（场景变才写）、
  `drop_zone node=… zone=before|child|after illegal=… at=(x,y)`（同态不重复写）、`canvas_click` 补 `at=` 与 `hit=`。
  **踩坑**：先前想用 100px 点击网格反推节点位置，命中被概要/边界吞掉（`owner_node` 把注解算到主人头上），
  160 个点得到一张对不上的散点图；直接导出场景几何只用了十几行。
  四张基线（`--tol 0` 双跑）：`mm-drag-child` 强调色 (47,111,94) 描边矩形 `(989,609)-(1065,652)` **416 px**；
  `-before` 3px 条 `y 606..608` / **233 px**；`-after` `y 653..655` / **233 px**；
  `-illegal` 红 `DROP_BAD=(224,75,75)` 目标 2px 描边 `(425,429)-(566,463)` + 幽灵虚线框。
  脚本以**按住不放**结束，所以截图时幽灵与落点都还在（松手就没有可拍的东西）。
  stage-b 门 79 场 / 158 次比较 → **83 场 / 166 次比较**，双跑 0 差异；全量 **912 测试**（本轮 +0，证据是金样与轨迹）、
  fmt 干净、clippy 37 / 新增 0。证据 `shots/x100/M5/`（report + manifest）。
  另：CM 门 44 场 / **88 次比较**本机复跑绿（`target/x100/gate-cm-m5.log`）；顺手改掉 STATUS 里
  「CM-09 三张场景未拍」这条过期结论——`cm-outline-link/unlink/via` 早就在门表与基线目录里。
  **未做**：`Ctrl` 拖拽复制、150/200% 档的条宽与描边、吸附引导线入景、实机拖拽手感。

- 2026-10-29 00:4x · **CER-20 保存/冲突/只读/长文性能** · 保存改两阶段：读者页发 `ReaderAction::Save` → 壳层 `pending_card_save` 排队 → **下一帧开头** `pump_card_save` 才写盘（顶栏那一帧的「保存中…」才算真话）。顶栏五态（只读 / 保存中… / 保存失败 / 编辑中 / 已保存）+ **6px 圆点**；顺带修掉「保存失败后 `dirty_since` 不清 → 每帧重试一次写盘」的打磁盘循环（`autosave_due` 纯函数 + `mark_save_failed` 清计时）。
  外部修改：打开库内卡片时记 `file_guard::fingerprint`（长度 + 修改毫秒），写盘前 `changed_since` 比一次；变了就把保存请求作废、挂 **32px 行内条**（重新加载 / 覆盖 / 取消），不动正文、不用模态。只读：打开时「追加打开」探可写性 → `TextEdit::interactive(false)` + 顶栏「只读」+ `Ctrl+S` 提示另存为。
  长文：`MW_CARD_PERF` 逐帧探针 + `MW_CARD_20K` 夹具实测——打开首帧 **5.5ms**、连续输入 200 字 p95 **3.6ms** / max **5.4ms**（0 帧 >50ms）、IME 20 轮 max **4.4ms**（0 帧 >16ms），三条判据全部达标，**没有**上块级缓存。
  金样：+6 场（`card-saving` / `card-conflict` / `card-readonly` / `card-autosave` / `card-editing` / `card-20k`），**77 场 / 154 次比较**双跑 0 差异；6px 点让 30 张卡片基线漂移（`card-reveal` 242 px / 0.011%），按 `docs/29` §5 全部重拍。金样里自动保存默认关（`MW_CARD_AUTOSAVE=<ms>` 显式开），避免截图与真实时钟赛跑。
  全量 **911 测试**（+3）、fmt 干净、clippy 37 / 新增 0。证据 `shots/x100/CER-20/`（report + plan + manifest）。

- 2026-10-28 13:3x · **CER-18 斜杠菜单** · 宽 252 → **280**、行距修到 **30 CSS**、条目 8 → **10**（补有序列表 `1. ` 与图片 `![图片]()`，光标落在 `![图片` 之后）、
  阴影改走 `ui_kit::menu_frame`（即 `design::elevation::POPOVER` 令牌）、菜单键盘块加 IME 守卫（本帧有 `Event::Ime` 就不截 ↑↓/Enter/Tab）。
  量出来的真问题：egui 默认 `item_spacing.y = 8` 叠在 30 行高外面，实际行距 **38 CSS**，「最多 8 行」只露 6 行半、第 7 行还被视口切字；浮层内把 `item_spacing.y` 归零后，
  行高实测 **45 设备像素 = 30.0 CSS**、高亮行宽 **402 设备像素 = 268 CSS**（= 280 − 2×6）、可见 **8 行**。
  **漂移量：重拍 `card-slash` 5.213%（91208 px，bbox `(260,513)-(722,1013)`）；新增 `card-slash-filter`（`839B0D27D41F`）、`card-slash-scroll`（`CF09AA3CA59F`）**，
  stage-b 门 63 → **65 场 / 130 次比较**；`note_view` 43 → **45** 条单测；旧基线留档 `target/x100/drift/CER-18/`（未入库）。
  两条踩坑：① `scroll_to_me` 是「滚过去」不是「跳过去」，动效会让同一场景两遍拍出不同的帧（实测 9915 / 8269 px 差），改成自己算 `vertical_scroll_offset` 后三张各跑两遍逐字节相同；
  ② 改完代码只跑 `cargo test` 就调 `./target/release/mw-shell.exe` 拍图，拍的是**旧二进制**（新图与旧基线 diff 0），手工拍图前必须 `cargo build --release -p mw-shell`。
  另修一个环境假失败：本机 `%LOCALAPPDATA%\Temp` 对**测试进程**拒写（`os error 5`），`mw-shell` 56 条 / `mw-format` 8 条写盘用例全红；CI 与门脚本已把 `TEMP`/`TMP` 指到 `target/tmp/ci-temp`，
  `mw-format` 的临时目录改成「仓库 `target/tmp` 优先、Temp 兜底」。
- 2026-10-28 12:40 · **CER-17 收尾：一次撤销** · 带外改动（回车接管 / 列表粘贴）现在记「改前文本 + 改前光标 + 改后文本」，`Ctrl+Z` 优先还原该槽，还原一次即清空、正文被别处改过即作废。
  判据是两张基线**逐字节相同**：`card-paste-none`（什么都不粘）与 `card-paste-undo`（粘两行再按一次 `Ctrl+Z`）同为 `C32274AB09AD`，逐像素 diff 0。
  先试过直接喂 `TextEdit` 的 undoer：egui 下一帧按自己的 `flux` 规则再补一个点，撤销退过头（718 px 差，包围盒 `(539,219)-(612,277)`），已改方案并写入报告 §9.2。
  stage-b 门 61 → **63 场 / 126 次比较**；`note_view` 42 → **43** 条单测。
  另记一条踩坑：`scripts/*.ps1` 必须 UTF-8 **with BOM**，无 BOM 时 PS 5.1 按 ANSI 读中文注释会报 `Unexpected token )`，行号还指向无关行。
- 2026-10-28 12:12 · **CER-17 粘贴半边：多行纯文本按当前块补前缀** · `paste_list`（单行/段落不接管，有序逐行递增、任务恒未勾选、空行留空），6 条单测；实拍 `card-paste-list` 续行首簇 `(543,547)` 是 5px 短横 + `(553,568)` 的字，对照 `card-paste-plain`（无列表夹具）一簇 `(541,556)` 无短横。stage-b 门 60 → **61 场 / 122 次比较**。
  顺带修门的时间敏感点：`outline-20k-deep` warmup 10 → 16（门负载下滚动没落定，实测 0.496%）、`outline-hover-select` 悬停保持 160ms → 400ms（握把阈值 120ms 只多 40ms）、两遍前 `Start-Sleep 10` 降载；改后两次复跑第 1 遍不再失败。
- 2026-10-28 06:58 · **IME 机器判据：`card-ime-preedit` / `card-ime-commit`** · 夹具 `MW_CARD_IME`（两段正文），脚本 `click` + `ctrl+End` 落文末后 `ime preedit 你好世界` → 确认；两张图正文行段**逐像素相同**，差异仅 1792 px、bbox `(652,260)-(715,287)`，组合态在 `y=271` 多一条 `x 654..695` 下划线（`commit` 无）。基线 `D9AD193E6A09` / `D9B029206B67`，stage-b 门 58 → **60 场 / 120 次比较**零差异。真机 IME 仍待 CER-07。
- 2026-10-28 06:52 · **CER-16 后半收口：两张实拍进门** · 新增夹具 `MW_CARD_ENTER`（两行清单）+ 脚本 `card-enter-continue` / `card-enter-exit` / 对照 `card-enter-nox`；像素判据是「新打那行的首簇」——有续写时先一段 5px 短横 `(543,547)` 再缩进起字，退出后首簇是 14px 宽的字 `(541,554)`、没有短横。基线 `D14782EA0C5E` / `7FD1A5ED664E`，stage-b 门 56 → **58 场 / 116 次比较**零差异（`Warmup=20`，连跑两遍一致 = 光标相位确定），CI OK（884）。
- 2026-10-28 06:30 · **CER-16 后半第一刀：行尾回车接管列表** · `list_prefix` + `enter_edit`（空项退出 / 非空续写 / 无序归一 / 有序 +1，行中与标题不接管），接线在正文 `TextEdit` 之前「先探测后消费」，IME 事件优先跳过；9 条单测（`note_view` 27 → 36），工作区 875 → **884**，门 56/112 **零漂移**、clippy 37 / 新增 0、`CI OK`。**状态 `进行中`：缺两张实拍场景与 IME 门**，清单见 `shots/x100/CER-16/plan-part2.md`。
- 2026-10-28 05:45 · **CER-16 第一步：片段级标记揭示**——`docs/28` §6.3 的「语法只在光标处露出」从**行级**推进到**片段级**：
  光标落在 `**粗体**` / `` `码` `` / `~~删除~~` / `[文字](链接)` / `[[双链]]` / `![图](url)` 内时，只露**这一段**的标记，
  同一行其它片段的标记照旧收起；块级标记（`#`/`>`/`-`/`1.`/围栏）仍按「光标所在行整行露出」，与 CER-15 一致。
  实现三件：`MarkerMode::OnlyLine` 带**行内字节列**（`col_in_line` 每次换行归零，字符索引≠字节列）、
  新增 `inline_spans(s, caret)` 预扫「包含光标」的片段跨度、`push_inline` 的 `mark` 改成位置化 `mark(st, at)`。
  正文 buffer 与光标映射**一个字节都没动**（`markdown_job_preserves_every_byte` 仍绿）。
  **漂移量：新增 3 张**——`card-reveal`（`B391E701D2C4`）、`card-reveal-block`（`1B1629B01164`）、
  `card-reveal-link`（`A315DB5F2177`），皆 1800×1200 @ppp1.0；**其余 53 场逐位 0 差异**（未聚焦时 `active=None`）。
  实拍：与「同夹具 + 光标在行首」对照差 **2074 px**，bbox `(539,219)-(682,250)` 只覆盖段落行，`粗体` 墨迹簇右移 **13 px**（两个 `*` 宽）；
  门 53 → **56 场 / 112 次比较** PASS，测试 869 → **875**（`note_view` 21 → 27），clippy 37 / 新增 0，CI OK。
  **未做**：即时转换（`# ` / `> ` / `- ` / `1. ` 敲下成块 + 一次 `Ctrl+Z`）与 CER-17 共用 IME 门，留待下一轮；真实 IME 组合态仍未覆盖。

- 2026-10-28 04:58 · **CER-15：卡片页块级排版收口**——块间距全部落在**空行自己的行高**上
  （正文 buffer 一个字节都不动，`markdown_job_preserves_every_byte` 仍绿），连续空行只让第一个承担间距，
  且取「上块下边距 / 下块上边距」的**较大值**（边距折叠，否则「列表下 8」会吃掉「标题上 40」）。
  记号缩进改成「4px 隐形字 + `extra_letter_spacing`」，解 `n·g + (n−1)·s = 目标`，列表正文起点实测 **24.0**；
  围栏代码块去掉逐行底色，改画整块容器（60% 底、圆角 10、内距 16、右上 11px 语言标签）；
  行→行盒改成按 `row.ends_with_newline` 累计（原来的「行盒数 == 源码行数」守卫会因末尾换行多一个空行盒而永不成立）；
  整行判定统一容 `\r`，CRLF 正文里的分隔线/标题/任务框不再退化。
  **漂移量：新增 `card-blocks`（`993BE9B131ED`）；重拍 8 张**（`card-default` `03207764665E`、`card-empty` `A638ADE8B04C`、
  `card-long` `F04DDFBB6932`、`card-narrow` `55098F12035C`、`card-tags` `EE2FAAC9465F`、`card-slash` `2A13413ED56C`、
  `cm-card-link` `1521E0C8D5FC`、`cm-card-link-back` `DC9A25E9628E`），全部连跑两遍 `--tol 0` 零差异；
  旧基线留档在 `target/x100/drift/CER-15/`（未入库）；逐场景像素漂移 0.00%–9.53%（`card-narrow` 最大，窄窗换行多）。
  门 **52 → 53 场 / 104 → 106 次比较**零差异；CM 门 44/88 零差异；测试 **867 → 869**；clippy **37 / 新增 0**（`push_line` 参数 8 个
  显式 `allow` 并写明理由，其余按建议改掉：手写 `strip_suffix`、无用的 `vec!`）；`scripts/ci.ps1` → **CI OK**。
- 2026-10-28 03:21 · **CER-14：卡片页页头/页尾重排**——`H1 30/40 → 8 → 元信息 12/18 → 12 → 标签 13/20 → 40 → 正文 16/28`，
  反链移到页尾（32 留白 + 1px 线 + `反向链接` 13/20/600 + 条目 13/20）；彩色标签 chip 改纯文字；补元信息行（更新于/来源可点/字数）。
  **漂移量：新增 `card-tags`（`76FA1BC70B49`）；重拍 6 张**（`card-default` `437F9A6C82E0`、`card-empty` `936553F9B1C7`、
  `card-long` `911AE6CCD4BE`、`card-narrow` `996EA93B44A0`、`card-slash` `F43BAE79D78C`、`cm-card-link` `195154512847`）。
  **这一轮没有逐像素漂移量**：卡片页基线原本不在 git 里（`?? shots/golden/card-default/`），重拍前也没留副本，旧字节已经没了。
  结构性证据改为真实 `rect`（`MW_CER14_DEBUG` 临时打印后删除）+ 墨迹聚类，见 `shots/x100/CER-14/report.md` §3.1。
  **流程改进**：以后动外观前，先把要重拍的基线复制到 `target/x100/drift/<场景>-before.png`，重拍后跑一次像素比对再删。
  门 **51 → 52 场 / 102 → 104 次比较**零差异；测试 **865 → 867**（新增标签两条）。

- 2026-10-28 02:05 · **CER-13 收尾：版心像素断言 + 四主题 + 卡片场景进门**——`MW_CARD_LONG` 长段落逼换行；窗口 1080 版心左 180.0/宽 712.7，窗口 900 左 130.0/宽 634.7，四主题几何一致。
  `card-default`/`card-empty` 原来**不在任何门里**，现与 `card-long`/`card-narrow` 一并登记进 stage-b 门（47→51 场）。
  **漂移量：新增 2 张卡片基线（`57BBE5BD3F0C` / `384D44E03E10`），既有 3 张卡片基线零漂移。**
  **附带修复两处基建缺陷**（都是「让 golden 假红」而不是产品坏）：stage-b 门补 TEMP 重定向（`mw-format` 8 个用例在受限令牌下 `os error 5` 假失败）；
  `MW_LIBRARY_DEMO` 夹具原本把示例库写进用户 Temp（本机拒写）且 `let _ = upsert` 吞错误 → 页面画空态、`library` 三档全红。
  现在落 `target/tmp/mw-library-demo-v2`、每次先删再种、固定 id、写失败 panic + 断言张数；重拍库页三档（`352E863E8771` / `561FA6CEA15B` / `2A3F134B8D5B`）。
- 2026-10-28 01:54 · **跨度拖拽松手之后**——`mm-span-commit`（`MW_SPAN_COMMIT`）：提交 target 2→3、撤一次回 2、重做回 3；
  与 `mm-summary-select` 差 7 198 px，`y0..440`/`y610..1200` 各 0 px ⇒ 只续花括号不重排。
  缺口：边界那条 `SetBoundaryTargets` 同路径未单独拍；缩短方向未拍；无 cargo 单测。**漂移量：新增第 44 张，既有 43 张零漂移。**
- 2026-10-28 01:48 · **多选取消不留残影**——`mm-annot-multi-off`（`MW_ANNOT_MULTI_OFF`）：加两条 → 都选中 → 取消边界。
  trace `left=1 kinds=[Summary]`；强调绿 **838 == 838**（单选那张）、与多选那张差 3 284 强像素只在边界环带、与单选基线 **0 强像素**。
  教训：第一版判据写成「与 `mm-summary-select` 逐像素相等」，跑出 sha 不同——不是 bug，那张图里没有边界对象。
  **漂移量：新增第 43 张，既有 42 张零漂移。**
- 2026-10-28 01:32 · **标签弹层封顶 + 滚动**——`cm-outline-label-many`（`MW_OUTLINE_LABELS=120`，66 行）：视口封在 639、总高 653、
  底边 790（窗口 800）⇒ 不越界且能滚。坑一：`Area` 内层 `max_rect` 取**上一帧**矩形，与 `ScrollArea` 的「当前可用高度」互相喂小值，
  会自锁在 400，得先 `ui.set_max_height(max_h)`；坑二：`Frame` 上下内边距 14 不在滚动区里，不扣就让底边到 806（越界 6px）。
  **漂移量：旧场景 `outline-search-labels` 41 526 px 变化 / 2 257 强差，bbox `x734..1294 / y230..306`（只在弹层内），已重拍；新增第 42 张。**
- 2026-10-27 19:40 · **检查器四格的指针路径上锁**：`cm-arrow-inspector`（`2BB3D7E6B968`，147 064 B，27 帧）。
  坑：`key Escape` 不是 `key escape`；菜单要两次 Esc；检查器要 `MW_INSPECTOR=1`。
  对照实验：点「已经在用的那一格」与不点差 **0 px** ⇒ 4 510 px 全是命令效果。**漂移量：既有 31 张零漂移**，新增第 32 张。§3 最后一条闭合。
- 2026-10-28 00:51 · **多选环 + 偶发量化**——`mm-annot-multi`（概要 + 边界同时选中）强调绿 4 381 px ≈ 852+3 983 的并集；`cm-focus-question-edit` 连跑 40 次全过（合计 55 次 0 复现），排除了「离屏过渡没跑完」这个候选根因（`reduce_motion` 本来就关）。场景 41 / 比较 82。
- 2026-10-28 00:36 · **跨度拖拽预览框**——`mm-span-drag`（`MW_SPAN_DRAG`）：以 `mm-summary-select` 为对照，差 10 672 px / 强差 617 px / bbox 430×121 横带；强调绿两张相同（852 vs 852）⇒ 拖拽态不吞选中环。场景 40 / 比较 80。同轮记下 `cm-link-drag-pointer` 偶发（门第 1 遍 1 850 px 红、3 次单跑全过）。
- 2026-10-28 00:25 · **边界选中环补拍**——`mm-boundary-select`（`MW_BOUNDARY_SELECT=1`，同一份 `tiny-cn`）。强差 3 590 px、强调绿净增 3 567 px；边界自身淡色底是 65 125 px 弱差。反直觉：边界色 `#8fb9a8` 精确命中仅 68 px（对照 61），半透明叠色后色值法无效——要弱差。场景 39 / 比较 78。
- 2026-10-28 00:10 · **注解选中环 + 跨度手柄补第一张像素证据**——新增 `mm-summary-select`（`MW_SUMMARY_SELECT` + `MW_OPEN=tests/fixtures/x100/tiny-cn.zmind`；概念图模板平铺、根不挂子级，建不出概要）。同帧对照差 4 015 px，强差 1 369 px，带内强调绿 591 px vs 对照 12 px。门 **38 场 / 76 次比较 PASS**、864 测试、`CI OK`（`cm-gate-r51b.log` / `ci-r51.log`）。同轮两坑：`copyFileSync` 还原源码 `mtime` 变早 ⇒ cargo 不重编（8 张假红，`touch` 后全绿）；改文件脚本写错把源码写成 46 字节 ⇒ 备份恢复后重加方法、两张关键基线逐位一致。
- 2026-10-27 23:30 · **贴图次序 bug**——`ui.painter_at(rect).image(...)` 之前画的手柄/预览线/占位提示/注解环全被整张盖掉（红色诊断线实测 0 像素、`sha` 不变）；挪到贴图之后，8 张基线各变 143–151 px，新增 `cm-relink-select-only`（1 228 px）/`cm-relink-drag`（7 287 px），逐像素求交证明强调 1 081 px 只在选中态出现；加源码次序守卫并实测它会红。门 37 场/74 次比较 PASS（`cm-gate-r50f.log`，864 测试）；8 张既有基线的漂移量用「临时回退重编」逐个实测（102–153 px）。踩到一条陷阱：`copyFileSync` 还原源码会把 `mtime` 设早，cargo 报 `Finished in 0.30s` 却没重编，复核跑的是旧二进制——还原后必须 `touch`。
- 2026-10-27 22:56 · **拖终点抑制强调：三张对照**——`press` 按住已验证（24 帧全 apply），但「只点不按」与默认 0 px ⇒ 点击没选中关系，前两张差异**不能**归因 relink；三张均未登记进门。
- 2026-10-27 22:46 · **引用体检（文档轮）**：19 个 §2.x 全部有效；HANDOFF 23 个文件引用核完，6 个「缺失」是自己解析器的假警（又踩一次裸文件名）。
- 2026-10-27 22:41 · **没有强调对象时整段跳过**：`emphasis_wanted` 早退，测试 863，门 70 次比较零差异，CI OK。
- 2026-10-27 22:36 · **时间戳校正**：按日志 mtime 修正 10 个 HANDOFF 块 / 11 个报告小节 / 9 条 PLAN 记录（原先凭感觉估，偏了 20–60 分钟）。
- 2026-10-27 22:33 · **基线自检**：35 张 = 34 张 1920×1200 + 1 张 960×690（`cm-narrow`）；顺带发现「每场景都有输入脚本」是错的（实测 15/35），已在 §2.38 改正。
- 2026-10-27 22:31 · **看不见的线不再画**：`emphasis_visible` 早退跳过全透明强调的加密/挖洞/三角化，测试 862，门 70 次比较零差异。
- 2026-10-27 22:24 · **报告结构规范化（文档轮）**：`## 2.x`→`###`、内部小标题→`####`、加分组目录、修过期状态行、§3 补第 10 条。代码零改动。
- 2026-10-27 22:21 · **挖洞退化输入补测**：药丸盖住整线 ⇒ 零段；孤点必须被丢掉（不许 1 点段）。测试 861，CI OK。
- 2026-10-27 22:16 · **药丸矩形单测**：拆出 `label_pill_rect_at` + 2 条断言（居中 / 最小宽兜底），测试 859；
  恒等重构 ⇒ 门 70 次比较全零差异。顺带记一个坑：`Rect` 没有 `center_x()`，只有 `center()`。
- 2026-10-27 22:08 · **强调乘上关系过渡透明度**：`emphasis_alpha(tier, rel_alpha)` + 1 条单测（5 断言），
  修掉「淡入中的线被强调画成满色」的幽灵线；门 35 场 / 70 次比较**零差异**（稳态恒等），测试 857。
- 2026-10-27 22:01 · **拖终点时抑制选中强调**（已实现 / 未实拍）：`relink_drag` 期间不画 3px 线，避免与 1.6px 预览线
  读成平行线；离屏 harness 无法保持按住态，钩子拍出与 `cm-default` 同 SHA 后已撤回。基线无漂移。
- 2026-10-27 21:49 · **悬停概念点亮命题线**：关系线强调加第三档（指着端点概念 1.8px/30%），`cm-rel-adjacent` 锁住；
  差集 4 077 px 中盒外 3 137 px 沿三条辐条散开，trace `links=3` 对齐。概念图 35 场 / 70 次比较，测试 856。
- 2026-10-27 21:41 · **弯曲关系线的强调实拍**：`cm-rel-curve`（`LINK_BLANK` 造相交线 + `text 引起` 提交短语）锁住
  「强调沿曲线走、药丸处断开」——离弦最大 9.14 px、差集恰 2 块连通域；同时推翻上一轮「悬停没换指针」的错误结论。
  概念图场景 34 场 / 68 次比较，测试 856。
- 2026-10-27 20:25 · **关系线悬停/选中强调**：`paint_relationship_emphasis` 叠加层（3px 实色 = 选中、45% + 2px = 悬停），
  标签药丸按世界坐标挖洞（先 `densify` 再 `split_around`，直线段只有两点挖不动——这个 bug 是靠“重拍后像素数与改动前逐位相同”发现的）；
  新增 `cm-rel-hover`，8 张基线漂移 24–1 955 px，测试 856。
- 2026-10-27 19:20 · **带子悬停态上锁**：`cm-focus-question-hover`（`EAF7A3AE2082`，121 564 B，18 帧脚本，指针停在带子上）。
  与 `cm-focus-question` 差 8 636 px、边界恰为带子矩形；底色 `242,238,231` vs `252,251,248`。**漂移量：既有 30 张零漂移**，新增第 31 张。
- 2026-10-27 19:00 · **「排列」子菜单竖直夹取：量 + 单测**：`place_submenu` 不翻只夹，此前无竖直单测。
  新 trace `menu_submenu_rect` 量到 `cm-multi-menu` 里子菜单 `200×300` 落在 `(593,298)`、离底边 202px（未触发夹取）。
  补两条单测（贴底 `492`、顶出上沿 `8`），`menu::` 17 passed。**漂移量：零**（只加 trace 与测试）。
- 2026-10-27 18:40 · **双击带子改问题的指针路径上锁，沿途撞出三个真 bug**：新脚本 `cm-focus-question-edit`
  （双击 (960,75) → `Shift+Home` 整行替换 → 回车提交，38 帧）连撞三洞：① `focus_editor_ui` 被罩在
  `if self.rel_edit.is_some()` 里，双击带子编辑器不画；② 双击继续落到 `add_floating_world()`，背后多一个浮动主题
  并重取景（34 458 px）；③ `Home` 不看修饰键，`Shift+Home` 把画布「适应视图」了一遍（缩放 0.83 → 1.00）。
  三个都修了（拆分支 / 新 `focus_band` 进门禁 + `interact_pointer_pos` 兜双击 / `consume_key(Modifiers::NONE, Home)`）。
  **漂移量：既有 29 张零漂移**；新增第 30 张 `8CED3AE31B3D`（115 777 B），与 `cm-focus-question` 只差标题脏点 / 带子文字 / 底栏三块。
- 2026-10-27 18:35 · **门因为「写不了系统 Temp」假失败一次**：`.zmind` 往返与全量测试同时 `os error 5 拒绝访问`，
  但 `Temp` 的 ACL 正常（用户 Modify、无 AppContainer 项），是 cargo 孙进程在受限令牌下写不了系统 Temp。
  `scripts/x100-stage-cm-gate.ps1` 与 `scripts/ci.ps1` 现把 `TEMP/TMP` 定到仓库内 `target\tmp`；改后 `cargo test -p mw-format` 19/19 全绿。
- 2026-10-27 17:58 · **拖动建线指针几何上锁**：探针量出节点屏幕矩形（锚点 = 四边中点），从右锚点 `(642.1,420.3)` 拖到空白 `(1100,200)`，
  trace 出 `link_new_concept`。新场景 `cm-link-drag-pointer`（`B699A5AA689D`，119 119 字节）。
  **漂移量：既有 28 张零漂移**；新增第 29 张。§3 第 1 条闭合。
- 2026-10-27 17:40 · **推翻第 27 轮的「不派发」结论 + 指针路径上锁**：探针量出七行逻辑矩形，真因是三个旧坐标分别落在内边距 / 子菜单行 / 分隔条缝隙；
  正确坐标 `(694,272)` 一次即通（`menu_click` + `relationship head … both`）。新场景 `cm-arrow-menu-click`，与 `cm-arrow-menu-key` 逐字节相同（`472E3BBA1A08`）。
  **漂移量：既有 27 张零漂移**；新增第 28 张。文档同步更正 5 处（`review.md` 2 处、`report.md` §2.20/§3、`docs/32` §11）。
- 2026-10-27 07:05 · **多选菜单 arm 补基线**：`MW_CONTEXT_DEMO=multi`（选两个概念、跳主概念、开节点菜单、展开「排列」），
  新场景 `cm-multi-menu`，`entries=9`、`779AEB5E06D3`（连拍两次同哈希）。**漂移量：既有 26 张零漂移**（只多一个夹具取值分支），
  新增第 27 张。踩坑：`selected_free_topics()` 跳过 `root_id`，按 `doc_order().take(2)` 选会只算到 1 个，「排列」子菜单不出现（`entries=7` 先露了马脚）。
  门 `MACHINE GATE PASS` / 54 次比较 / 848 测试；子菜单竖直夹取未量，已写进 §3 残项。
- 2026-10-27 04:12 · **CM-05 的空白菜单补基线 + 门自律**：`MW_CONTEXT_DEMO=blank` 开空白处菜单，新场景 `cm-blank-menu`，
  概念图空白菜单 9 项（6 动作 + 3 分隔线，`entries=9` 与代码一致，无「全部展开/全部折叠」）。**漂移量：既有 25 张零漂移**
  （钩子只多一个取值分支，节点菜单场景 `MW_CONTEXT_DEMO=1` 路径不变），新增第 26 张（`55EB4C27300D`，连拍两次同哈希）。
  门新增守则：脚本帧数 ≤ warmup，否则 FAIL（拍在脚本中途的图确定但锁错画面）。门 `MACHINE GATE PASS` / 52 次比较 / 848 测试。
- 2026-10-27 04:05 · **CM-10 ⑦ 收尾 2：把「选完之后」也上锁**：上一轮锁的是「勾在第几行」（状态），
  这一轮锁「选了它文档真的改了」。新基线 `cm-arrow-menu-key`：夹具展开子菜单、文档 `off` 起手，脚本键盘三下到「双向 ↔」+ `Enter`，
  trace 三段 `menu_nav` → `menu_activate` → `relationship head … both`。**漂移量：既有 24 张零漂移**（只加脚本与场景记录，没动渲染），
  新增第 25 张；门 `MACHINE GATE PASS` / 50 次比较 / 848 测试，`ci.ps1` → `CI OK`。
  负结果（已记 `report.md` §2.20、`docs/32` §11）：指针点弹层行在离屏里**不派发**——三次不同坐标只有按下/松开事件，
  两张指针终点不同的运行导出 PNG 逐像素相同（连「点外面关菜单」都没发生）。原因未定位；这条属于证据面缺口，不是产品结论。
  顺带：历史菜单场景**一律键盘选行**，菜单行的指针路径在 goldens 里从来是空白。
- 2026-10-27 03:48 · **CM-10 ⑦ 收尾：四档箭头在右键菜单里打勾**：四档互斥，但菜单四行一模一样——点了「反向 ←」只有端点变，
  再开菜单看不出哪档。`Entry::action_checked()` / `Item::row_checked` / `paint_row` 的 `Some(true)` 画勾（`Some(false)` 留空，
  图标槽宽度不变所以文字左缘不动）；当前档由 `Canvas::selected_relationship_head()` 从文档现读，不缓存（撤销后缓存会骗人）。
  新钩子 `MW_CONCEPT_ARROW_MENU=1`。**漂移量：既有 23 张零漂移**（没有别的场景会开关系线子菜单，48 次比较里 46 次是旧基线）；
  新增第 24 张 `cm-arrow-menu`：`off` 勾在第 1 行（y 177–186）、`backward` 勾在第 3 行（y 238–245），勾 10×12 / 42 px。
  测量教训：差分只说「两块不同」，不说哪块属于哪张图——判颜色位置要逐图检色，别用差分归因。
  同时补 `shots/x100/CM-10/review.md`（G-CM-2 人工复核单：24 张基线表 + 复跑命令 + 8 处该盯的地方 + 判定后要改的字）。
- 2026-10-27 03:35 · **CM-10 ⑥ 收尾 3：长问题不再被窗口左右硬切**：137 字的问题原来 `layout_no_wrap`，ink 从 x 1 铺到 x 1918，
  两端硬切且无省略号（看不出被切）。`focus_wrap_width = 画布宽 − 64`（下限 120）+ `Painter::layout(.., wrap)`，
  带子高度按 `text.y + 14` 自动长高。**漂移量：既有 22 张逐字节不变**（短问题排出的 galley 相同），新增第 23 张 `cm-focus-long`。
  取舍：**不封顶行数**——问题写得长是作者的选择，截断比占地方更糟。
- 2026-10-27 03:28 · **CM-10 ⑥ 收尾 2：大纲页就地改问题**：`content_kit::question_field` + `question_edit_outcome`
  与画布共用（控件一份、判定一份），修掉「编辑框第二帧焦点被行抢走」；
  新基线 `cm-outline-focus-edit`（第 22 张），与 `cm-outline-focus` 差异仅在设备行 105–121，正文行零位移。
- 2026-10-27 03:1x · **CM-10 ⑥ 收尾：大纲页的聚焦问题带**：页头 40px 控制台按宽度省略、塞进去长问题会被吃掉，
  所以做成页头下沿一条独立带子（在 `outline.show` 的 `ScrollArea` 之外，不随正文滚走）；几何抽成
  `content_kit::question_band` + `QUESTION_BAND_PAD = 7`（与画布 `pad.y`、导出 `FOCUS_NOTE_PAD` 同数）。
  **漂移量：既有 20 张 `cm-*` 零漂移**（四张 `cm-outline*` 都没设问题）；新增第 21 张 `cm-outline-focus`：
  与 `cm-outline` 比只多 106–120 一行墨迹，正文整体下移 42px；长问题（60 字）带子两行、下移 62px。
  第一版的分隔线撤掉了（上下已有 8px 项间距，再加线成双线）。门 20 → 21 场。- 2026-10-27 02:5x · **CM-10 ⑥ 导出补齐（对标 CmapTools 的交付面）** · `Scene::focus_note: Option<SceneNote>` +
  `attach_focus_note()`：带子锚在内容**上方**并向上扩 `bounds`，既有几何零移动；**只有导出会填**，画布永远 `None`。
  文字只加了一个 `TextJob`（PNG/PDF/SVG 本来就共用 `text_jobs()`），三条导出各自零改动；入口统一到
  `export::attach_focus_note()`，把 CLI `--export-pdf` 那条自己 `build_scene` 的支路也接了进来。
  带宽取整到整数（否则内容落到半个像素，「位移 N 行后逐像素相等」这条证明失效）。
  **漂移量：无。** 没有重拍任何 golden——画布不填该字段，`cm-*` 20 张基线逐张不变（全量门复核）。
  实拍：1386×860 → 1386×934（+74 设备像素 = 37 世界单位 @2×），切掉顶上 74 行后差异(>4) **0 px**、最大 1 LSB；
  PDF 1 页不变、77619 → 92537 字节。仍未做：⑥ 的大纲页头。
- 2026-10-27 02:2x · **CM-10 ⑦ 端点箭头** · `ArrowHead { Off, Forward, Backward, Both }` + `Relationship.head`
  （默认 `Forward`，`skip_serializing_if` 不落盘）→ `Scene` 变成两个可选三角形（`arrow` 终点端 +
  `arrow_back` 起点端，起点端由 `mirror_tri()` 围绕中点镜像）→ wgpu / egui / SVG / PDF 四处同源
  → 入口两处（检查器「箭头」四格、关系线右键「箭头方向」子菜单）→ 基线 `cm-arrow-off` / `-backward` /
  `-both`（CM 门 17 → 20 场）。**既有 17 张 `cm-*` 基线零漂移**：默认值 `Forward` 就是原来的行为；
  同时改正了 `docs/32` §3「默认不画箭头」与 `docs/30` §12 第 7 行「需要扩字段」两处误判。
  差异量：`off` 82 px / 5 簇、`both` 81 px / 5 簇、`backward` 163 px = 82 + 81（`off ∪ both`）。- 2026-10-27 03:1x · **CM-10 ⑥ 绘制层** · 聚焦问题从「只有字段」做到「看得见、改得动」：`focus_band_rect()`（横向居中、贴顶 10px、宽度上限 = 画布宽 − 32px）、`paint_focus_question()`（`READING_BODY` 16px、`dim` 色、悬停才补底线）、双击就地编辑（Enter 提交 / Esc 取消 / IME 期间 Enter 归选字）、检查器「聚焦问题」一栏（**没写问题时的唯一入口**）。带子只在 `focus_question.is_some()` 时绘制，所以既有 `cm-*` 基线**逐张 0 漂移**（CM 门第 2 遍 17 组比较全过）；新增第 15 张 `cm-focus-question`：与 `cm-default` 逐像素只差 `rows 68–83`（1840 px / 0.0799%，x 848–1063），双跑 `--tol 0` 通过。clippy 指纹 37/37、新增 0、缺失 0。导出（PNG/PDF/SVG）与大纲页头未做——导出只认 `Scene`，问题带按设计不进 `Scene`，属单独立项（`docs/32` §4）。
- 2026-10-27 02:4x · **clippy 门可复现性** · 「38 > 37」不是新告警，是计数受 lib/lib-test 重复打印与 cargo 缓存影响（同代码报过 33/37/38）；逼全量重检两次均为「38 原始行 / 37 唯一指纹」；`ci.ps1` 改为比对 `scripts/clippy-baseline.txt`（`-WriteClippyBaseline` 重写），复跑 29 步全绿 EXIT=0。
- 2026-10-27 02:3x · **CM-10 ⑥ 数据层** · `LayoutSettings.focus_question` + `Command::SetFocusQuestion`（空串归一 None）+ 3 条单测（撤销/不写键/旧文件兼容）；`cargo fmt` 干净、`cargo check --workspace --all-targets` 0 错。方案与回滚路径记在 `docs/32` §4。
- 2026-10-27 02:0x · **全量 CI 回归** · `scripts/ci.ps1` 连红三轮（根场景 111px / `theme-ink` 161px / `selected` 156px，全部 0.01% 量级、亚像素形态）。给 `ci.ps1` 加 `-Update`（25 处调用接 `@upd`），按 CI 自带环境表重拍全部基线；再跑 `ci.ps1` → **29 步 0 红、EXIT=0**。查出一件事：109 个子目录 golden 基线从未入库（非忽略），只有 14 个根文件在库里，所以陈旧基线烂了 6 天没人看见。
- 2026-10-27 05:2x · **回归大扫：5 个问题** · `cargo test --workspace` 抓到 `shell_sources_do_not_hardcode_font_sizes` 失败（我第 13 轮写死 `FontId::proportional(12.0)`）→ 改用 `design::content::META.size`；clippy 修 `needless_borrow`、给 `paint_link` 的 8 参数显式 `#[allow(too_many_arguments)]`、给废弃目录的 4 条死代码加 `allow(dead_code)` 并注明理由；`cm-card-link` 基线过期（32014 px，两次 actual 同 hash 证明非抖动）已重拍为 `C991545A4DA4`。clippy 回 3 条历史告警。
- 2026-10-27 04:4x · **CM-10 ⑥⑦ 提案** · 新增 `docs/32-概念图数据结构提案.md`：⑥ `LayoutSettings.focus_question: Option<String>`、⑦ `Relationship.head: ArrowHead{None,Forward,Backward,Both}`，两处都「可选 + `skip_serializing_if`、不升 `FORMAT_VERSION`」（`Relationship.color` 已有同款先例），并列出三件需拍板的事（是否授权扩字段 / ⑥ 挂图级还是做成概念节点 / ⑦ 默认是否画箭头）。`docs/30` §12 两行改指该提案，状态仍为 `延后`。
- 2026-10-27 04:2x · **CM-10 ①b 收尾一致** · 改线到空白后立刻进短语编辑态（与拉新线同构）。trace 同时有 `relink_new_concept` 与 `rel-edit-open screen=(1263,866)`；编辑框区域 (1173..1353,834..896) 差 **261/11403** px；双跑 0 差异（光标不影响基线）。**另**：查证方向键导航已存在（`navigate()` + `directional_neighbor` 几何近邻，概念图同样生效），原计划那条也不是缺口。
- 2026-10-27 03:5x · **CM-10 ①b 改线落空 = 新建概念** · 新原子命令 `RetargetLinkToNewConcept`（建概念 + 改指，一步撤销），`commit_relink` 落空分支接上；占位框抽成 `paint_blank_concept_hint()` 两处共用。单测 `retarget_link_to_new_concept_undoes_in_one_step` 通过；trace 有 `event relink_new_concept`；A/B **5534 px**；复跑 0 差异；门 16 场景。
- 2026-10-27 03:5x · **假 golden 事故（已修）** · 首版钩子按 `字体同源` 找线，该短语不在概念图模板里，钩子静默 no-op；golden 正常出图、复跑 0 差异，但 A/B 对照 diff=0 暴露了它。改用真短语 `带来` 后差分 5534、trace 有事件。**规则**：带钩子的场景必须配无钩子对照图做 A/B。
- 2026-10-27 03:3x · **CM-10 ① 落点语义修正** · 占位框原按指针**居中**画，但 `pos` 是盒子**左上角**（`scene.rs:1687` 断言 `rect.x == pos.x`），预览因此报了个偏 (48,18) 的位置。改 `from_min_size` 与双击空白同口径。单测加「`pos` 原样存下」断言通过；A/B 差分 bbox 240×78 → **298×100**（右 +59、下 +22），与去掉半盒 + 文字下移吻合。**另**：核查确认「概念 ↔ 卡片」往返早已存在（画布徽标点击开卡、悬停出标题摘要，`canvas_methods_3.rs:1869`），原计划的反查入口不是缺口。
- 2026-10-27 03:1x · **CM-10 ① 落空预览** · `paint_link` 在概念图 + 落空时画 96×36 占位框（accent 8% 填充 / 65% 描边，按 zoom 缩放）+ 12px 提示「松手新建概念」。新增 `ConceptHook::LinkDrag` 与 `MW_CONCEPT_LINK_DRAG="源|dx|dy"`，门增至 15 场景（`cm-link-drag`，sha256 `76d5a65e93ea`）。A/B 差分：**6472 px / bbox 240×78**（首测 20205 px 是因为钩子顺手选中了源概念，选中环混进差里，去掉重拍才收窄）。
- 2026-10-27 02:5x · **CM-10 ① 撤销缺陷修复 + ② 核查** · 新增原子命令 `Command::AddLinkedConcept { from, text, x, y }`（`Batch` 用不上：新概念 id 要 apply 时才生成），画布落空分支与大纲 `Tab` 都改成**一条撤销记录**，原先按一次 Ctrl+Z 会留下孤儿概念。单测 `add_linked_concept_undoes_in_one_step` 通过；`cargo test -p mw-shell --bin mw-shell` 446 passed；`cm-link-blank` 重跑 0 差异（画面未变，只有撤销语义变）。② 双击空白核查为**已实现**（`canvas_methods_2.rs:316`），上轮「待核查」已订正。
- 2026-10-27 02:2x · **CM-10 ① 拖到空白处新建概念（对标 CmapTools）** · `commit_link()` 落空分支接上 `insert_floating_world()`，概念图里拖到空白即「建概念 + 建边 + 立起短语框」，非概念图仍不动。语义复用大纲 `Tab` 的 `link_concept_below()`。新增 `ConceptHook::LinkBlank` 与 `MW_CONCEPT_LINK_BLANK="源|dx|dy"` 钩子、`create_concept_link_blank()` 脚本入口；门清单加第 14 个场景 `cm-link-blank`（1920×1200，sha256 `82d51198b8c8`）。trace：`link_new_concept from=… to=… at=-90,-130`。**欠账**：鼠标拖拽几何无脚本证据、一次撤销未断言、②双击空白未做。
- 2026-10-27 01:5x · **目标转向：概念图对标 CmapTools** · 用户新目标改为「完成所有功能后打磨概念图、对标 CmapTools」。新增 `docs/30` §12 CM-10 差距清单（8 条：1 拖到空白新建概念【未开始】、2 双击空白新建【未开始】、3/4/5/8 已实现、6 聚焦问题【延后】、7 箭头样式【延后】）。实测证据：`commit_relink()` 落空即 `return false`（`canvas_methods_2.rs:1339`）；建概念+建边的 Batch 语义已有现成实现 `link_concept_below()`（`canvas_methods_1.rs:1111`），缺的只是画布手势。另：按新骨架重拍 `card-slash` 基线（1620×1080）。
- 2026-10-27 01:2x · **CER-13 骨架（B2-B 第一包）** · 卡片页顶栏固定 40px（返回 / 面包屑 / 字数 / 保存态 / 更多），顶栏第二份标题删除、标题变正文首行 H1（28/600），常驻右目录 204px 整块删除（连带 `toc_click`、`heads` 计算、点击定位），版心 `720` / 窄窗 `640`，上 48 / 下 96。新增 `MW_CARD_EMPTY` 只服务 golden。新基线 `card-default`(`95B3DFB78731`)、`card-empty`(`B69DD5EB165A`)，均 1620×1080 @ppp1.5。**欠账**：① `card-slash` 仍是旧骨架基线，必须重拍 ② 版心宽度没有像素级断言（只有代码值）③ 四主题未拍。
- 2026-10-26 · **短语框场景重开：真凶是夹具，不是产品** · `main.rs` 的 `MW_OUTLINE_SELECT` 每帧检查 `outline_pin.is_none()` 就重新武装并重置预算 4，短语框一丢焦点它立刻把焦点按回行——「短语框立不住 + Backspace 断边」由此自锁。改法两条：① 加 `outline_pin_done`，pin 只武装一次；② `RowKey::Delete` 在 `via_edit.is_some()` 时只记 `outline_delete_blocked_by_via_edit` 不执行（产品级防线，不依赖夹具）。trace：`focus outline-via-edit` → 两次 `outline_delete_blocked_by_via_edit` → `outline_via_close esc=false blank=false`。于是 `cm-outline-via` 重开并补拍基线（哈希 `98fe97a68cae…`，输入 `shots/golden/inputs/cm-outline-via.txt`，warmup 26）。门：**13 场景 ×2 遍 `--tol 0` PASS**（`target/x100/cm09-gate12.log`，`13 comparisons`，finished=23:31:38）。
- 2026-10-26 · **短语框（Shift+F2）断边缺陷：找到真凶是夹具，不是产品** · 现象：短语框里按 `Backspace` 把边断了，且没有 `via_close`。逐帧 trace（`shots/x100/CER-12/trace-via-keys.txt`）显示 `outline_via_start fresh=false` 之后 `outline_focus_pin` **每帧继续抢焦点**：`main.rs` §973 只要 `MW_OUTLINE_SELECT` 还在、`outline_pin` 为 None 就重新武装并把预算重置为 4，而 `is_via_editing()` 一旦为假（短语框丢了）就不再 `clear_pin`，于是「夹具抢焦点 → 短语框立不住 → 夹具再抢」自锁。**结论：这条离屏证据链被自己的夹具污染，产品路径尚未证伪。** 同时加了一条真防线：`RowKey::Delete` 在 `via_edit.is_some()` 时只记 `outline_delete_blocked_by_via_edit` 不执行，短语框立着时任何路径的 Backspace/Delete 都断不了边。机器门 `target/x100/cm09-gate11.log`（12 场景 ×2 遍 PASS，finished=23:25:48）。**未做**：`cm-outline-via` 场景仍未重开——要么先让夹具在短语框立起来时彻底撒手，要么改用不依赖 `MW_OUTLINE_SELECT` 的入口。
- 2026-10-26 · **概念图三条「规格 vs 实测」口径统一（M 轨；改代码 + 改文档）** · 箭头按合同 6×4（`straight_between` 的 9/4.2 → 6/2.0）、关系线宽按合同 1.5（`REL_W` 1.6 → 1.5）、节点圆角按令牌 `radius::MD = 10` 改文档（`docs/30` §5.3.1 的 8 是笔误）。12 张 cm-* 全量重拍，`canvas-ppp1.00.png` 与 `text-canvas-*.png` 由 offscreen / text_render 的 golden 测试用 `MW_GOLDEN_UPDATE=1` 重写（文本基线 3/540000 像素、最大差 8，连跑两次一致）。证据：`target/x100/cm09-gate8.log`、`shots/x100/CM-09/report.md` §5.5。
- 2026-10-26 · **大纲筛选胶囊键盘化（B2-A §5.7 缺口之一）** · `filter_chip` 原来只有 `Sense::click()`；补「胶囊模式」：焦点在搜索框时 `↓` 进第 0 格并 `surrender_focus` 搜索框（否则 `←/→` 会被光标吃掉），`←/→` 绕行三格，`Space`/`Enter` 复用鼠标单击的同一段切换逻辑（`chip_act`，取走即清），`↑`/`Esc` 回搜索框并重新点亮输入焦点；焦点格画 1.5px 强调描边。鼠标路径一行没改。证据：`shots/x100/CER-12/trace-filter-chips.txt`（`chip_focus idx=0 source=down` → `chip_focus idx=1` → `chip_act idx=1 source=key`）、输入脚本 `shots/golden/inputs/filter-chips.txt`、机器门 `target/x100/cm09-gate10.log`（12 场景 ×2 遍 `--tol 0` PASS，finished=23:18:01，mw-shell 零新增告警）。顺手修掉自己引入的两条 clippy：`filter_chip` 8 参（底色改由 `ch.accent` 现算，7 参）、`focused` 与 `hover` 两个分支画同一块底（合并条件）。**未做**：任务胶囊四态循环的「单态」问题、标签弹层键盘化。
- 2026-10-26 · **CM-09 概念图收口（M 轨 M3，已实现）** · 范围：补 G-CM-2 判词里 §5 没做到的 5 条（大纲改结构 / 概念省略 tooltip / 卡片圆点 / 跨分支取色 / 交叉微弧），三条「规格 vs 实测」口径不动。文档先落：`docs/30` §5.6.1（`Tab` 建「概念 + 边」、`Shift+Delete` 只断进边留孤立概念、`Shift+F2`/点前缀改短语、五条短语编辑纪律：单行/IME 不收 Enter/Esc 还原/空短语清零/一字未动不发命令）、§5.3.1、§5.4.1、§5.9 四条禁令、§5.10 三张新场景。代码：`ConceptRow` 增 `via_id`（行要知道断哪条边）与 `branch`（一级概念各占一号、二级沿用一级，跨分支取色要用）；`SceneNode` 增 `truncated`（`measure_concept` 真加 `…` 才为真）与 `has_card`（空白 `card_id` 不算）；`scene` 增 `concept_rel_color`（跨分支取**来源**分支色、显式色优先、**只活在绘制层不写回文档**，否则撤销/往返/换主题全坏）与 `segments_cross`/`bowed_between`/`bow_crossings`（后声明那条在中点偏 ≤6px，箭头按终点切向重算，<12° 近平行不算交叉）；`mw-render` 画右下角 5px 卡片圆点（屏幕恒定 `CONCEPT_CARD_DOT_R/_INSET`）；大纲做短语就地编辑（`ViaEdit`/`PendingVia`，编辑状态 `take` 到行循环外——`self.rows` 在循环里是只读借用，硬塞进去必和 `self.editing` 打架；`ui.put` 单行框 + 2px 强调下划线；`Tab`→`Batch[AddFloatingTopic, AddRelationship]` 失败回滚整条边）；画布 `Canvas::paint_concept_tip`（400ms 停留、节点下沿 8px、最宽 320、圆点命中半径 7px、`Order::Tooltip` 不可交互）。顺手清掉两处死代码（`ViaEdit.original` 改为「一字未动不发命令」、删掉没人调的 `via_editing`）。**外观**：重拍前 10 张 `cm-*` 漂移逐字确定（8 张有差：`cm-default` 2631px/0.114%、`cm-narrow` 2141/0.323%、`cm-arrange` 1011/0.044%、`cm-crosslink` 2531/0.110%、`cm-card-link-back` 2658/0.115%（最大差 205 = 圆点+强调色）、`cm-node-edit` 2605、`cm-link-create` 3567/0.155%、`cm-label-ime` 2631；`cm-card-link` 与 `cm-outline` 为 0——前者停在卡片页画布被盖住、后者只画大纲）；`scripts/x100-stage-cm-gate.ps1` 增 `-Update`（同场景表逐张 `--update`，第二遍即复核）后 10 张全部重拍并通过；旧哈希 `target/x100/cm-baseline-old-hashes.txt` + `shots/x100/CM-09/manifest.json`（8 改 2 未变）。**未做**：`cm-outline-link`/`cm-outline-unlink`/`cm-outline-via` 三张（harness 缺「先点行再发键」入口，只用单测钉键位与投影）；圆点只有 `cm-card-link-back` 一张耦合证据；三条口径仍挂账；未改 `.zmind`、未提交 git。
- 2026-10-26 · **CM-09 收口登记（12 场景 ×2 遍 `--tol 0` PASS）** · 新增 `cm-outline-link`（0d09470dc5c9ab87…）、`cm-outline-unlink`（8413de7a2e26e679…）两张基线；`cm-outline-via` 未拍（短语框在离屏里被「活动行」抢焦点，`Backspace` 落成断边），场景目录与输入脚本保留、门表摘除。为拍这两张给 harness 加了只给验收用的 `Outline::pin`/`clear_pin`，并把 `MW_OUTLINE_SELECT` 的查找挪到首帧扫 `doc.nodes` 全表（`doc_order()` 走不到浮在 `children` 外的概念节点）。证据 `target/x100/stage-cm-gate.log`、`shots/x100/CM-09/report.md` §5.4。
- 2026-10-26 · **CER-12 回补：§5.2 非编辑态长文换行 + 第二处金样确定性修复（B2-A）** · 缺口（`docs/28` §5.2 与 `shots/x100/CER-12/report.md` §7 第 1 条）：非编辑态正文仍是 `wrap.max_rows = 1` + 省略号，超宽文本被截断，没有「换行每行 +24、悬挂缩进」。改法：`Row` 增 `text_rows / via_w`；`Outline::measure_wraps(ctx, col_w)` 按**正文可用宽**（`text_avail` = 列宽 − 缩进 − 任务框/短语前缀 − 行尾角标/折叠计数，与绘制同一把尺子）把显示行数写进缓存，短行用「字符数 × 字号」上界直接判 1 行、不进字体引擎（20k 不受影响），行表重建/增量补行/列宽变化置脏重测（`wrap_dirty / wrap_col / row_w_seen`，行宽取上一帧 `ui.available_width()` 实测）；`row_layout_height(row, row_h, Option<edit_lines>)` 按「每行 +24」长高（根 44、说明 +20 的底子不变）；行绘制改 `row_text_job`（`max_rows = usize::MAX`、`break_anywhere`、换行行 `line_height = 24`、单行仍用字体行高 → 既有基线零漂移），圆点/chevron/任务框/角标改锚**首行**文字带（`first_band`），省略号与「截断悬停全文」提示退役；命中底带改 `hit_bands` 按已排版 galley 拆行（单行沿用旧「行框 ±3」→ `outline-search` 逐字节不变）；概念图短语前缀宽度改用缓存 `via_w`。新场景 `outline-wrap`（`MW_OUTLINE_WRAP=1` + `Session::sample_wrap`：2 行折行 / 短行不折 / 深层 avail 716 / 硬换行 / 说明行叠加 avail 725 / 3 行累计），`MW_TRACE` 记 `outline_wrap depth rows avail`。**修第二处金样确定性缺陷**：带 `MW_GOLDEN_INPUT` 的脚本场景从不清桌面指针，鼠标压在某行上会截出带 hover 的图（实测 3.06% 与 5.06% 两种漂移，bbox 正好一整行）→ `main.rs raw_input_hook` 离屏且脚本**不含指针动作**时每帧先 `PointerMoved(-10000)+PointerGone`，指针归脚本管的场景（`GoldenInput::owns_pointer()`）不清（否则 `hover` 计时攒不满、握把不出现）。外观：既有 9 个大纲场景逐字节未变；新增第 42 个基线目录 `outline-wrap`（1.5 `ea771190…` / 2.0 `4be636a0…`，各双跑 `--tol 0` 0 差异，两档行位步进同构 36 vs 48 device px）；`scripts/x100-stage-b-gate.ps1` 45 → 47 组、`scripts/x100-cer12-outline-gate.ps1` 8 → 9 场景 / 10 组。门槛：fmt 干净、clippy 新增 0（仅 `export_pdf.rs` 3 条历史）、**824 tests / 0 failed**（+5）、release 通过、全量门 **47 组 × 2 = 94/94**（finished 16:58:35）、CER-12 门 9 场景/10 组 × 2 遍 + 12 矩阵 + 2 窄窗全绿。**未做**：握把 16×20 几何、拖拽 4px 阈值与 Forbidden 光标、§5.7 胶囊键盘路径、筛选态/概念图折叠 29ms、微软拼音人工实测、20k 仅 1.0 DPI、长文折行未进四主题矩阵；`outline-hover-select` 在高负载下偶发一次 hover/握把时序漂移（8 次连跑未复现），阶段门请在空闲机器上跑；未改 `.zmind`、未提交 git。
- 2026-10-26 · **CER-12 大纲证据包与 G-B2-1（B2-A 收口）** · 范围：不改功能，把 CER-01–11 收成 `docs/28` §5.10 的 8 场景证据 + §5.1–§5.11 逐条核对 + 阶段报告 + 门清单。新增 `scripts/x100-cer12-outline-gate.ps1`（8 场景各双跑 `--tol 0`、`outline-default` 四主题×三 DPI 12 张、`outline-narrow` 800×600/640×460 两档；**每遍清空隔离 profile**）。证据：`shots/x100/CER-12/{report.md,gate.md,manifest.json,hashes.txt,matrix/,narrow/,scripts/}`、`target/x100/cer12-outline-gate.log`；阶段报告 `docs/x100/stage-b2-report.md`（B2-A 章，含 8 条已知缺口）。**修了一个真 bug**：`outline-edit-ime` 双跑会停在 egui 光标闪烁的亮/灭两态（126 px、最大通道差 87），根因 `golden::prepare` 只归零 `animation_time`；现金样模式 `visuals.text_cursor.blink=false` + 单测 `prepare_freezes_time_dependent_visuals`；四个编辑场景各连跑三次哈希相同且等于既有基线。复用上一遍 `APPDATA`（含空 `notes/index.json`）曾让同一场景整列偏 1–3 px → profile 每遍清空。**门槛**：fmt 干净、clippy 新增 0、**819 tests / 0 failed**、release 通过、8 场景 16/16、12 矩阵 + 2 窄窗复跑 0 差异、全量门 **45 组 × 2 = 90/90**（`target/x100/stage-b-gate.log`，finished 14:49:21）。基线零改动。**未做（记进门判词，不算通过）**：§5.2 非编辑态长文换行（仍 `wrap.max_rows=1`）、握把 16×20 几何、拖拽 4px 阈值与 Forbidden 光标、§5.6 过程截图、§5.7 胶囊键盘路径、筛选态/概念图折叠 29ms、微软拼音人工实测、20k 仅 1.0 DPI。**G-B2-1 待用户八判词**，未回复前不开 CER-13；未提交 git。
- 2026-10-26 · **CER-11 大纲虚拟化与 20k（B2-A）** · 缺口（§5.8 逐条审计）：20k 行**每帧全量布局**、帧耗时 p50 **1090.93ms**（先测到的是 `FocusMemory::note` 用 `Vec::contains` 去重，O(n²)）；折叠/展开走全量 `rebuild`；远距跳转靠平滑滚动，golden 不可复现。改法：`focus.rs` 的 `Entry` 加 `seen: HashSet<FocusKey>`（`note` 去重 O(n²) → O(n)，同帧仍保序）；`outline.rs` 改 `ScrollArea::show_viewport` 虚拟化——前缀行高求 `first`，`back = first.min(10)` 回看、`last` 取视口下沿再 `+10` 行 overscan，上下用 `add_space` 补总高，只布局 `vis[first..last]`；`row_layout_height(row, row_h, edit_lines)` 统一行高（深 0 行 44、有卡 +20、多行编辑 +24/行）；抽出 `collect_rows` 供全量重建与增量展开共用，`apply_collapse` 折叠时 `drain` 子树、展开时 `splice` 新行，`rev = doc.revision` 跳过下次全量；`styles: HashMap<String, ResolvedStyle>` 缓存 `resolve_all`；远距跳转改为 ScrollArea 前的 `vertical_scroll_offset(prefix + h/2 - view_h/2)`（`MW_OUTLINE_SCROLL_TO` 钩子，trace `outline_virt`）；`MW_OUTLINE_PROFILE` 打行数/耗时；`Session::sample_20k()`（4000 分支 × 4 叶 + 根 = 20001 行）；`#[ignore]` 探针 `outline_20k_frame_budget_probe`（`ctx.run_ui` + 滚轮事件，断言首屏 <100ms、p95 <16ms、折叠/展开 <16ms、`laid_rows <= 80`）；`scripts/x100-outline-20k.ps1` 跑 stats 与 3 DPI × 4 主题矩阵。实测：首屏 **23.0ms**、滚动 p50 0.11 / p95 **0.13ms**、折叠与展开各 **0.17ms**、只排 **52/20001** 行；stats fps **56.2–58.7**、UI p95 ≤1.81ms、可见 33/20001。外观：新增 `outline-20k`（`2700c935…`）、`outline-20k-deep`（`50f2152f…`）两张基线，各双跑 `--tol 0` 0 差异；九个既有大纲场景逐字节未变；`scripts/x100-stage-b-gate.ps1` 43 → 45 组。门槛：fmt 干净、clippy 新增 0（仅 `export_pdf.rs` 3 条历史）、**818 tests / 0 failed**（+1）、release 通过、机器门 **PASS 45 组 × 2 遍 = 90/90**（finished 2026-10-26 14:19:23，`target/x100/stage-b-gate-cer11.log`）。**未做**：筛选态与概念图的折叠/展开仍走全量重建（约 29ms，仅与 20k 叠加时超 16ms）、行高前缀每帧 O(n) 未缓存、20k 下编辑/IME 未拍；未改 `.zmind`、未提交 git。
- 2026-10-26 · **CER-10 大纲搜索/筛选/聚焦（B2-A）** · 缺口：筛选只做文本子串；`Ctrl+F` 在大纲全屏时开的是大纲筛选（§5.7 要求 `Ctrl+Shift+F`，`Ctrl+F` 归全局搜索）；没有任务/卡片/标签条件、没有祖先上下文、没有命中字符高亮、输入框持焦时 Enter 不跳、筛选搜不到折叠分支里的命中。改法：`shortcuts` 新增 `Action::OutlineFilter`（`Ctrl+Shift+F`，仅 Outline 全屏消费），`Ctrl+F` 统一走 `open_search()`；`outline.rs` 增 `filter_task/filter_card/filter_label` 三条件 + `filter_chip` 胶囊（任务循环 待办→进行中→完成→不限、卡片开关、标签弹层），`prepare_filter(&Document)` 缓存键 = 文本+三条件，可见表 = 命中行 + 祖先链，非命中可见行 `set_opacity(0.48)`；`hit_ranges` 记字符区间，绘制前量前缀宽画 12% 强调底（alpha 30，省略号截断时夹到行宽）；`filter_owned` 接 Enter/Shift+Enter 只走 `hits`（回绕、焦点回筛选框、trace `outline_filter_step`）；Esc 链清全部条件；`rows_all` 让筛选态行表穿透折叠（命中可搜、文档折叠位不动，单测断言），清空后 `sync` 自动回正常行表；帮助表大纲键位更正（`Ctrl+Shift+F`、过期 `Ctrl+Enter=新建子级` 改现行语义）。外观：新增 `outline-search`（文本「排版」+标签「验收」AND，`1544a2aa…`）与 `outline-search-labels`（标签弹层，`616e6a7f…`）两张基线，各双跑 `--tol 0` 0 差异；`outline` / `outline-focus` / `outline-compact` / `outline-narrow` / `outline-edit` 逐字节未变；`scripts/x100-stage-b-gate.ps1` 41 → 43 组。门槛：fmt 干净、clippy 新增 0（仅 `export_pdf.rs` 3 条历史）、**817 tests / 0 failed**（+2）、release 通过、机器门 **PASS 43 组 × 2 遍 = 86/86**（finished 2026-10-26 12:54:29，`target/x100/stage-b-gate-cer10.log`）。**未做**：条件胶囊无键盘路径（Tab 进不了 22px 热区）、任务胶囊循环单态、标签弹层只能鼠标、高亮区间按 `to_lowercase` 字节位映射（罕见 Unicode 可能偏 1 字符）；未改 `.zmind`、未提交 git。
- 2026-10-26 · **CER-09 门检：`start-tab-focus` 旧基线修复** · 缺口：首次跑 41 组门时只有 `start-tab-focus` 失败（稳定复现，与 CER-09 代码无关）——基线还是开始页 v2 之前的「还没有最近文件」空状态，而场景早已是 `MW_START_DEMO=1`（三条演示最近文件），差异 11.9331%（79,045 px、最大差 221、bbox `(41,175)–(903,318)` 正好落在「最近文件」段）。改法：按 `docs/29` §5 留旧哈希与旧图（`shots/x100/CER-09/baseline-before/start-tab-focus-ppp1.50.png`、`ad05385e…`）后重拍（`8fc6018a…`），并在 `start-tab-focus.txt` 收尾补 `move 24 690` + `wait 2`——脚本存在时 harness 不清桌面指针，悬停 wash 会随光标位置漂进基线。重拍后双跑 `--tol 0` 0 差异。机器门 **MACHINE GATE PASS**（41 组 × 2 遍 = 82/82，815 tests，finished 2026-10-26 12:19:04 +08:00，`target/x100/stage-b-gate-cer09.log`）。**未做**：其它脚本场景未逐个加指针收尾（本次只有这一张被污染）；未改产品代码，未提交 git。
- 2026-10-26 · **CER-09 大纲拖拽与折叠（B2-A）** · 缺口（§5.6 逐条审计）：握把**悬停即时**出现（要 120ms）、拖拽源没有 §5.3 的 40% 降透明度、Child 非法落点只判不画（要红 1px）、**Esc 取消拖拽实际没接**（旧审计误记已完成）、三区分带写死在行内、跨父级自动展开无 trace。改法：`outline.rs` 加 `GRIP_DELAY=120ms` + `grip_since: Option<(String, Instant)>`（同一行保留起点、换行重计时、选中即时）+ `grip_ready()` 纯函数（119/120ms 边界单测）；拖拽源 `Painter::set_opacity(0.4)`；三分支抽 `drop_zone(rel)` 并加 0.25/0.75 边界单测；Child 非法改红 1px 整行描边（合法 6% 底、圆角 5→6）；拖拽块头部 `consume_key(Escape)` 取消并 trace `outline_drag_cancel`；`OA::Move/MoveMany` 补 `outline_expand_parent`；新增 `outline_drag_start/spot/illegal/cancel/ghost` trace。外观：`outline-hover-select` 重拍前后**逐字节相同**（`0031D764…57A7`，漂移 0 px / 0.0000%；脚本改成 hover 保持 160ms 等延迟到点，旧图留档 `shots/x100/CER-09/baseline-before/`）；新增 `outline-drag`（`99cf601f…`，源 40% + 目标 6% + 幽灵）与 `outline-drag-illegal`（`1776a778…`，红 1px + NoDrop）两张基线，各双跑 `--tol 0` 0 差异；`outline` / `outline-focus` / `outline-compact` / `outline-narrow` / `outline-edit` / `cm-outline` 逐字节未变；`scripts/x100-stage-b-gate.ps1` 38 → 41 组。行为 trace（`shots/x100/CER-09/traces/`）：`outline-drag-drop` → `outline_move_drag` 一条命令；`outline-drag-cancel` → `outline_drag_cancel` 且无落盘；`outline-drag-expand` → `outline_expand_parent`。门槛：fmt 干净、mw-shell clippy 新增 0（仅 `export_pdf.rs` 3 条历史）、**815 tests / 0 failed**（+2）、release 通过、`x100_fixtures` 往返 2 passed、机器门 41 组 × 2 遍全 0 差异。**偏差**：4px 拖拽阈值仍用 egui 默认（约 6px）；禁止光标用 `CursorIcon::NoDrop`（egui 无 Forbidden）；握把 16×20 盒/距列左 8 未重做（§5.2 归 CER-05，当前 2×3 点阵）。**未做**：自动滚动/自动展开/多选拖拽未各拍拖拽中截图（静态 golden 不赌经过帧，由既有单测 + trace 覆盖）、拖拽动画曲线、自定义阈值状态机；未改 `.zmind` 版本与字段，未操作用户 Xmind 或个人文件，未提交 git。
- 2026-10-26 · **CM-04 概念图自由摆放与整理布局（M 轨）** · 缺口：概念图里**主概念拖不动**（`SetNodePos` 拒绝根、`detach_commands` 也跳过根）、拖到别的概念上会被挂成子节点（树语义漏进命题网）、没有「整理布局」、没有重叠提示。改法：新增 `mw-core/src/concept_layout.rs`（`arrange()` 纯函数：DFS 丢掉指向「正在访问」节点的回边 = 环按首次访问截断 → 最长路径松弛分层 → 入边父重心稳定重排 4 趟 → 列宽取层内最宽、列间 `LAYER_GAP=56`、层内 `NODE_GAP=24` → 24 轮「朝邻居中心均值靠一半 + 层内压开」→ 列位合成、左对齐到原包围盒左上角；输入只有尺寸/边/当前位置，输出左上角，5 组单测覆盖分层、层内最小间隙、成环、确定性、脏边）；新增 `Command::MoveConcept`（`parent_id` 必须为 `None` 且 `kind == "conceptMap"`，因此**根**可摆，思维导图根仍拒）；`Document::validate` 对概念图主概念的 `pos` 放行，`SetLayout` 从概念图切回树布局时清掉该 `pos`（否则旧不变量立刻判脏）；`build_scene_cached` 在概念图里把主概念布局整体平移到 `pos`（与浮动概念同语义）；概念图拖拽帧内不再算落点、`commit_move` 走 `commit_concept_move`（只发 `MoveConcept`，一条 `Batch` 一次撤销）；落点与别的概念相交 → `overlap_flash` 给 1px `content::warn` 描边亮 0.25s（新版 `design::content` 语义色，亮 `#c02b2b` / 暗 `#ff7b72`，egui 层绘制、不进 `Frame` 与导出）；空白处右键新增「整理布局」（单概念菜单整套留给 CM-05，避免顶开「样式」子菜单改动 `menu` / `menu-narrow` 基线）。外观：`cm-default` / `cm-narrow` 逐字节 0 差异；新增第 30 个基线目录 `cm-arrange`（1280×800、ppp 1.0、`MW_CONCEPT_DEMO=1 MW_CONCEPT_ARRANGE=1`，`1920×1200` 回读）作为「整理前后」对比，双跑 `--tol 0` 0 差异；`scripts/x100-stage-b-gate.ps1` 的过期计数（27 目录 / 31 比较）改为动态 `$($scenes.Count)`（现 30 目录 / 34 比较）。门槛：fmt 干净、mw-shell clippy 新增 0（仅 `export_pdf.rs` 3 条历史）、**796 tests / 0 failed**、机器门 `MACHINE GATE PASS`（34 组 × 2 遍 = 68/68 全 0 差异，finished 2026-10-26 02:05:51 +08:00）。证据：`shots/x100/CM-04/report.md`、`target/x100/stage-b-gate.log`、`shots/golden/cm-arrange/shell-ppp1.00.png`。**未做**：拖拽/重叠的脚本化场景与 golden（与 CM-03 的 `cm-link-create` 同笔账，留 CM-08）、单概念右键菜单里的「整理布局」、整理动画、选中子集整理；未改 `.zmind` 版本与字段，未提交 git。


（每完成一项在此追加一行：日期 · WP · 变更 · 证据）

- 2026-10-26 · **CM-07 概念图大纲视图与导出（M 轨）** · 缺口：概念图的节点是浮动概念（`parent_id = None`、`children` 恒空，`docs/30` §5.2 禁止用 `children` 表达命题网层级），而全仓有三处按 `children` 展平文档——全屏大纲把整张网摊成一列 depth 0、`export_markdown` / `export_opml` / `export_outline` / `export_freemind` 只导出主概念一行（**其余概念全丢，静默数据丢失**）；PNG/SVG/PDF 反而没事（吃 `Scene`）。改法：新增 `mw_core/src/concept_tree.rs` 作为**唯一投影**——`concept_rows(doc)` 从 `root_id` 前序沿 `relationships` 展开（出边按声明顺序、**环取首次访问边**、自环与已访问终点不入栈、`MAX_DEPTH=4096` 兜底），没被任何边指到的概念各自成 depth 0 的根（按 `y`/`x`/`id`，与删根挑新根同一把尺子），非概念图返回空表；大纲 `Outline::rebuild_concept` 用它建行表（生成树父链面包屑、折叠裁子树、聚焦截子树、`Row.via` 行首淡色短语前缀 `解决 →`，前缀按实测字宽让位，选区/编辑框/省略自动跟随），`rebuild` 在 `is_concept_map()` 之后分叉，树文档原路不动；概念图大纲里 **Enter = 新建概念**（新增 `Action::NewConcept` → `canvas.add_concept_below` → `AddFloatingTopic`，落点在被选概念下 56px，随后行内编辑），**Tab / Shift+Tab / Ctrl+Shift+↑↓ / 拖拽一律不接**（`trace outline_tree_key_ignored`）——接了就是把 `children` 写进命题网；页头 `大纲 · {标题} · 概念图 · 按链接层级展开`、右上「N 个概念」、页脚「N 个概念」；四条文本导出都改走生成树，链接短语随行进 `text`（`{短语} → {概念}`，否则导出的层级只剩名词），Markdown 文首 `> 概念图 · 按链接层级展开`、OPML 标题后缀、FreeMind XML 注释、纯文本不加标注。外观：旧基线 `cm-default` `5b0d9dc4…`、`cm-card-link` `b7d11182…`、`outline*` / `library` 三档全部逐字节未变（分叉在文档类型判定之后）；新增基线目录 `cm-outline`（`MW_CONCEPT_OUTLINE=1`、1280×800、ppp 1.0、warmup 6，`1920×1200` 回读，`16ced6ab…`），双跑 `--tol 0` 0 差异；`scripts/x100-stage-b-gate.ps1` 37 → 38 组。门槛：fmt 干净、mw-shell clippy 新增 0（4 条全部历史：`export_pdf.rs` 3 + `panels/outline.rs:3084` 1）、**813 tests / 0 failed**（+8）、机器门 `MACHINE GATE PASS`（38 组 × 2 遍全 0 差异）。证据：`shots/x100/CM-07/report.md` + `manifest.json`、`shots/golden/cm-outline/shell-ppp1.00.png`、`target/x100/stage-b-gate.log`。反例两处：把 `self.concept_map = doc.is_concept_map()` 临时改成 `false` → `cm-outline` 漂 13812 px（0.599%，`bbox x[588..776] y[161..398]`）；把 `concept_label` 的短语摘掉 → io 导出断言立刻断在 `## 因为 → 竞品访谈`；两处恢复即绿。**未做**：大纲里建/删连线（`docs/30` §5.6 期望的「用大纲改结构」只做到新建概念/改名/删除；建边必须先有短语编辑态，§5.9 禁止无线落盘，故 G-CM-2 判词按未完成记）、孤立概念在文本导出里并列成多个顶层（不为它编造父级）、概念图折叠只有大纲一个入口、长标题 + 短语前缀会提前省略；未改 `.zmind` 版本与字段，未操作 Xmind 或个人文件，未提交 git。

- 2026-10-26 · **CM-06 概念 ↔ 笔记联动（M 轨）** · 缺口：概念图的 `card_id` 早就能挂卡，但联动只有半条腿——检查器只有「新建卡片笔记」，`docs/30` §4.2 的三态少了「关联已有」；卡片页「来源」写死「当前文档 / 另一份文档」，§5.6 要的 `概念图名 · 概念名` 根本出不来；笔记库来源行只给一个布尔事实「挂着主题」，概念/主题不分；导出包在概念图里写「主题 · x」；回跳（卡片 → 概念）没有用例，概念图节点是 floating，能不能选中从未验证。改法：`mw-core` 增 `layout_kind()` / `is_concept_map()` / `display_title()`（空白标题 → 「未命名」）/ `linkable_cards(node, limit)`（只列没被别的节点占用的卡片，自己挂着的那张仍可选 = 换绑，按 `updated_at` 倒序）；检查器 `card_section` 补第三态（「新建卡片笔记 / 关联已有」→ 内联候选列表，限高 168px、最多 8 条、`SetNodeCard` 落地），`Action::LinkCard{node,card}`，卡片区措辞按文档类型在「主题/概念」之间切；卡片页 `SourceLink` 增 `concept_map`，来源行改成 `{文档名} · {节点名}`，文档名恰为「概念图」时不画类型角标（否则 `概念图 概念图 · 用户痛点`），按钮「回到概念」；笔记库 `CardSource{doc_label,node_text,concept_map}` + `phrase()`，`show` 的 8 个参数用 `Extras` 打包（clippy `too_many_arguments` 触发过一次），来源行 `· 来源 {文档名} · {节点名}`、跨文档退回「另一份文档」不编名字；回跳统一到 `Shell::jump_to_source()`——关浮层 + 选中 + 交还焦点 + `trace card-jump`，卡片页与笔记库两个按钮共用同一个落地点（之前是两段各写一遍的代码）。外观：`cm-default` / `cm-narrow` / `cm-arrange` / `cm-crosslink` / `library` / `card-slash` 逐字节未变（来源行只对 `sourceNode` 有值的卡片生效，演示卡片都没有，所以笔记库三档缩放一格不漂）；新增第 32/33 个基线目录 `cm-card-link`（`b7d11182…`）与 `cm-card-link-back`（`1ee059f4…`），各双跑 `--tol 0` 0 差异；`scripts/x100-stage-b-gate.ps1` 35 → 37 组。门槛：fmt 干净、mw-shell clippy 新增 0（4 条全部历史：`export_pdf.rs` 3 + `panels/outline.rs` 1）、**805 tests / 0 failed**（+4）、机器门 `MACHINE GATE PASS`（37 组 × 2 遍 = 74/74 全 0 差异，finished 2026-10-26 03:34:25 +08:00）。证据：`shots/x100/CM-06/report.md` + `manifest.json`、`cm-card-link.trace.txt`（`card-jump node=… concept=1` + `focus canvas`）、`cm-card-pick.png`（候选列表只出现没被占的那张）、`gate-summary.txt`。反例三处：拆掉 `linkable_cards` 的 `n.id != node_id` → 换绑断言红；拆掉 `phrase()` 的空名检查 → 跨文档断言红；拿掉 `jump_to_source` 的 `select_node` → `cm-card-link-back` 漂 13729 px（0.596%）、exit 1，恢复即绿。**未做**：笔记库整体重排（三栏 / 来源面包屑属 G-B2-2，本包只改「来源这一行写什么」）、卡片页内的解绑入口、跨文档回跳（仍 toast）、检查器头部「分支主题」字样（SE 轨）、候选列表不含跨文档孤儿卡片；`MW_CONCEPT_CARD` 用 `replace_session` 造初始文档（走 `AttachNewCard` 会把 `updated_at` 刷成墙钟、基线每天漂一天），代价是它同时设 `MW_CONCEPT_CROSS` 时会盖掉交叉连接（现有场景不同时用）；未改 `.zmind` 版本与字段，未操作用户 Xmind 或个人文件，未提交 git。

- 2026-10-26 · **CM-05 概念图交互（M 轨）** · 缺口：概念图的交互语言整套借自思维导图——方向键走 `navigate`（按几何换选下一个节点，而命题网没有前驱后继）、`Shift+方向` 被 `Modifiers::NONE` 吃掉所以 8px 步长不存在、节点右键菜单仍是「新建子主题/新建同级/概要/边界/分离为自由主题/折叠/聚焦」（其中「新建子主题」会把树语义写进命题网）、空白菜单还挂着「全部展开/全部折叠」、新建概念占位写死「自由主题」、`DeleteNodes` 一律拒根导致**主概念删不掉**。改法：`Canvas::nudge_concepts` 把方向键改成**摆位**（素键 1px、`Shift` 8px、多选整组一条 `Batch` 一次撤销、不动相机、不闪重叠警示），步长收在 `consts::nudge_step()` 纯函数；`build_context_menu` 增概念图节点/多选两套菜单并抽出 `arrange_submenu()`，空白菜单按 `is_concept_map()` 去掉展开/折叠；`add_floating_world` 概念图占位改「新概念」；`can_delete` 概念分支按概念数判定；`Ctrl+A` 全选概念（概念图限定）。数据侧最小改动：`DeleteNodes` 在 `kind == "conceptMap"` 时放行 `root_id`，删完用 `surviving_concept()` 把根重挂到剩下里最靠左上（`y`→`x`→`id`）的概念，`ChangeSet` 新增 `before_root / after_root` 让 revert/reapply 把根一起还原，删到 0 个仍判 `RootProtected`；未改 `Node`/`.zmind` 字段与版本。外观：旧基线 `cm-default` `5b0d9dc4…`、`cm-narrow` `fc7f31f1…`、`cm-arrange` `56bc4017…`、`menu` `fbfaf8ce…` 逐字节未变（菜单是运行时态，不进静态基线）；新增第 31 个基线目录 `cm-crosslink`（`MW_CONCEPT_DEMO=1 MW_CONCEPT_CROSS=1 MW_CONCEPT_SELECT=用户痛点` + `inputs/cm-crosslink.txt` 的 `shift+方向` 5 下 → 右下 24/16px，`1920×1200` @1.0，`f2e948d1…`），双跑 `--tol 0` 0 差异；`scripts/x100-stage-b-gate.ps1` 增该场景（34 → 35 组）。门槛：fmt 干净、mw-shell clippy 新增 0（4 条全部历史：`export_pdf.rs` 3 + `panels/outline.rs` 1）、**801 tests / 0 failed**、机器门 `MACHINE GATE PASS`（35 组 × 2 遍 = 70/70 全 0 差异，finished 2026-10-26 02:38:26 +08:00）。证据：`shots/x100/CM-05/report.md` + `manifest.json`、`cm-crosslink.trace.txt`（5 条 `concept-nudge`）、`cm-node-menu.png`（树语义条目已消失）、`cm-blank-menu.png`（无展开/折叠）、`gate-summary.txt`。反例：临时拆掉 `with_root` 快照，换根撤销断言立刻断红，恢复即绿。**未做**：交叉处 ≤8px 微弧（CM-03 转出）、拖出连线的脚本场景（CM-08）、微调不做 500ms 合并撤销、删主概念后选择清空（概念图无父级可落）；未改 `.zmind` 版本与字段，未操作用户 Xmind 或个人文件，未提交 git。

- 2026-10-25 · **X20 人工门未通过 · Stage B2 计划定稿** · 用户判定全屏大纲“超级难看、不够简洁”、卡片笔记页“没有克制/精致的美感”、笔记库“像玩具”；X20 保持 `implemented`，Stage C 冻结。新增 `docs/28-内容体验重构规范.md`（三界面设计合同：共同公理、字阶/版心/状态、大纲/卡片页/笔记库逐项几何与交互、禁止项、验收判词）与 `docs/29-内容体验重构工作包.md`（34 个工作包、4 批、4 道门、依赖、证据模板、golden 重拍协议、风险）。工作量判断：34 包 ≈ 34 个等效轮次；吸收 X61–X67、X69–X70、X81–X82 共 11 轮，净新增约 22 个等效轮次；X68 命令面板延后；纯 AI 开发约 3–4 周，含人工门约 5–6 周。同步更新 `docs/24`（B2 插入阶段与断点）、`docs/x100/stage-b-report.md`（人工门结论）、`HANDOFF.md`、`AGENTS.md`、`README.md`、`docs/00`、`docs/06`、`docs/16`、`docs/26`、`.workbuddy-loop-status.md`。**未做**：未实现任何 CER 工作包，未改代码，未跑 golden `--update`，未开始 Stage C，未提交 git。

- 2026-10-25 · **X20 门后维护 · 开始页/笔记库裸键归属**（⑬⑭） · ⑬ 缺口：开始页（`MW_HOME=1`）盖着文档时，标题栏菜单仍是 egui Memory 弹层，不进 `overlay`；`window_shortcuts` 的开始页裸键（Esc / Enter / 1–5）没看它。1440×1000 / ppp 1.0 / `MW_START_DEMO=1` 实拍：开「文件」菜单后按 `1`，菜单背后的开始页被换成「思维导图」模板（122,992 B，trace 焦点 `canvas`）；按 `Esc` 被 `window_shortcuts` 先消费，菜单收不到（202,498 B，与对照图一致）。只改 `crates/mw-shell/src/app_methods_1.rs`、`app_tests.rs`：新增纯函数 `home_bare_keys_owned(menu_open, help_open)`，菜单或速查表在前台时整段跳过，F1 等应用级快捷键仍在前面处理；单测 `start_page_bare_keys_yield_to_foreground_layers`。脚本 `start-menu-{open,input,esc}.txt`（`wait 8 → click 114 27 → wait 6/8`）；修复后 `1` 保留开始页并收起菜单（169,377 B），`Esc` 产生 `popup_menu key=escape` 并收起菜单（169,377 B）。⑭ 缺口：笔记库页自绘整页，Esc / ↑↓ / Enter 直接读 `ctx.input`，绕过 egui 焦点；模态（设置/移出确认/正在打开与 `MW_DIALOG_DEMO`）在栈顶时 `egui::Modal` 只挡指针与 Tab。1080×720 / ppp 1.5 / `MW_LIBRARY_DEMO=1 MW_DIALOG_DEMO=1` 实拍：弹窗开着按 `↑`，暗幕背后预览从「键盘色板验证」切到「视图跟随」；再按 `Enter` 直接打开阅读页（102,116 B）。只改 `note_library.rs`、`app_methods_3.rs`：新增纯函数 `page_keys_owned(top_modal, dialog_demo)`，`NoteLibrary::show` 增加 `keys_owned` 门；单测 `page_keys_yield_to_foreground_modals`；脚本 `library-modal-input.txt`（`wait 8 → ArrowDown → wait 2 → Enter → wait 8`）。修复后 `↑` 不再移动预览，`Enter` 落到弹窗默认「确定」并关窗（trace `dialog demo closed` / `dialog demo action 1`），库页保留（264,292 B）。外观变化：无，31 组基线逐字节未变、未 `--update`。门槛：fmt 干净、mw-shell clippy 仅 `export_pdf.rs` 3 条历史、**765 tests / 0 failed**、阶段门 **31 组 × 2 = 62/62** 全 0 差异（started 09:05:59 / finished 2026-10-25 09:08:57 +08:00），结论仍 `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`；release `23,470,592 B` / SHA256 `78a01487…f49de`。证据：`shots/x100/X20/traces/caretaker-start-menu-2026-10-25.md`、`caretaker-library-keys-2026-10-25.md`、`caretaker-{start-menu,library-keys}-*-2026-10-25.{png,trace}`、`caretaker-stage-b-gate-start-menu-library-2026-10-25.log`、`caretaker-{tests,clippy}-start-menu-library-2026-10-25.txt`。**未做**：开始页速查表只做纯函数门与单测、未单独实拍；笔记库阅读页等其它页面型浮层未扩面；未开始 X21，未改文档模型/`.zmind`，未操作 Xmind 或个人文件，未提交 git。

- 2026-10-25 · **X20 门后维护 · 卡片演示日期冻结** · 缺口：`MW_CARD_DEMO` 建示例卡片时用 `mw_core::now_ms()`，阅读页用 `format_date` 按 UTC 取日（`ms.div_euclid(86_400_000)`）；本地 08:00（UTC+8）跨日后同一份代码渲染的「创建于/更新于」从 `2026-10-24` 变 `2026-10-25`，而 `card-slash` 基线生成于 10-24 22:30，导致 ⑪ 首次阶段门在 08:27 两遍都出现 198 px（0.011%，最大差 92，`bbox x[1380..1390] y[370..415]`）超差——既有缺陷，与大纲改动无关。只改 `crates/mw-shell/src/app_methods_1.rs`：新增 `DEMO_CARD_TS_MS = 1_792_800_000_000`（2026-10-24 00:00 UTC），`MW_CARD_DEMO` 的 `NoteCard::new` 改用常量；真实笔记的新建/保存/排序仍取 `now_ms()`。`app_tests.rs` 新增 `card_demo_timestamp_is_frozen_for_golden` 断言 `format_date(DEMO_CARD_TS_MS) == "2026-10-24"`。外观变化：无，未重拍任何基线；`card-slash` 单跑 `--tol 0` 0 差异、基线未被 `--update` 改写。门槛：fmt 干净、mw-shell clippy 仅 `export_pdf.rs` 3 条历史、**763 tests / 0 failed**、阶段门 **31 组 × 2 = 62/62** 全 0 差异（started 08:32:49 / finished 2026-10-25 08:34:09 +08:00），结论仍 `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`；release `23,469,056 B` / SHA256 `a4662b1f…f5babbb862`。证据：`shots/x100/X20/traces/caretaker-card-demo-date-2026-10-25.md`、`caretaker-card-demo-date-{context-baseline,baseline-crop,drift-crop}-2026-10-25.png`、`caretaker-stage-b-gate-card-date-fail-2026-10-25.log`、`caretaker-stage-b-gate-card-date-2026-10-25.log`、`caretaker-outline-keys-tests-2026-10-25.txt`；`shots/x100/X20/manifest.json` 补场景与测试条目。**未做**：`NoteCard` 的真实时间来源不改（有意）；其他演示种子若依赖墙钟未逐个审计；未开始 X21，未改文档模型/`.zmind`，未操作 Xmind 或个人文件，未提交 git。

- 2026-10-25 · **X20 门后维护 · 模态与菜单下大纲键盘隔离** · 缺口：全屏大纲自绘整列表，键盘直接读 `ui.input` / `consume_key`，不经 egui 焦点系统；`egui::Modal` 只挡指针与 Tab 巡游、标题栏菜单是 Memory 弹层，都挡不住裸键。1080×720 / ppp 1.5 / `MW_OUTLINE=1 MW_OUTLINE_FOCUS=1 MW_DIALOG_DEMO=1` 下修复前 trace：`Delete` → `outline_delete id=01a1706e-…bf221f` 删除聚焦行「纯 Rust 原生」、`Enter` → `outline_add_sibling` + `outline_edit_start`、`Tab` → `outline_indent`——三下都落在暗幕背后，用户看不见，与 X17 / 2.9 / 2.10「前台浮层是第一键盘主人」冲突。按 `docs/24` 定稿口径只改 `crates/mw-shell/src/panels/outline.rs`、`app_methods_2.rs`、`main.rs`：新增纯函数 `row_keys_owned(blocked, editing, filter_owned)`（单测 `row_keys_are_owned_only_without_foreground_or_field` 覆盖门开/行内编辑/搜索框/皆无四种组合）；`show` 新增 `blocked` 参数，由 `outline_screen` 传入已有 `input_blocked`（`demo || dialog_demo || overlay.blocks_canvas() || tab_drag || Popup::is_any_open`，不新增第二套判定）；键盘处理链头部 `if blocked { outline_key_blocked key=…; }` 跳过全部编辑/缩进/删除分支，搜索框 Esc 回焦加 `!blocked`；不改行高/缩进/焦点环/滚动。证据：脚本新增 `outline-modal-input.txt`（`wait 8` + `Delete` + `Tab` + `Home`，24 帧、warmup 24；修复后 `Enter` 会落到弹窗默认「确定」并关窗，故归档脚本不用它，原 26 帧修复前脚本另存）与 `outline-menu-input.txt`（`wait 8` + `click 115 27` 开「文件」+ `Delete`，23 帧）；修复后模态 trace 三条全为 `outline_key_blocked`、无任何文档事件，实拍弹窗保持打开、大纲 3/13 行与缩进层级不变（156,211 B / `2e51e8e1…d912d61d1`；修复前 169,548 B / `1da1b47a…adc21d9`，前后 201,453 px = 11.514% 来自列表内容而非外观）；菜单场景用展开对照图证明点击前菜单确实开着，修复后 trace 只有 `outline_key_blocked key=Delete`，菜单展开/收起两图差异 91,931 px = 5.254%（`bbox=(112,14,629,619)` 菜单卡片）。外观变化：无，`outline` / `outline-focus` / `outline-compact` 三组基线逐字节未变。门槛：fmt 干净、mw-shell clippy 仅 `export_pdf.rs` 3 条历史、**763 tests / 0 failed**（outline 模块 33 条）、阶段门 **31 组 × 2 = 62/62** 全 0 差异，结论仍 `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`；release `23,469,056 B` / SHA256 `a4662b1f…f5babbb862`。证据：`shots/x100/X20/traces/caretaker-outline-keys-2026-10-25.md`、`caretaker-outline-keys-{before,after}-modal-2026-10-25.{png,trace}`、`caretaker-outline-keys-{menu-open,after-menu}-2026-10-25.{png,trace}`、`caretaker-outline-keys-prefix-modal-input-2026-10-25.txt`、`caretaker-stage-b-gate-outline-keys-2026-10-25.log`、`caretaker-outline-keys-{tests,clippy}-2026-10-25.*`；`shots/x100/X20/manifest.json` 补场景与测试条目。**未做**：菜单场景无修复前 A/B（修复前模态证据已足够，菜单只做修复后复核）；Shift+Tab 与真实读屏未采样；其余模态共享同一门未逐个实拍；未开始 X21，未改文档模型/`.zmind`，未操作 Xmind 或个人文件，未提交 git。

- 2026-10-25 · **X20 门后维护 · 标题栏菜单下画布点击归属** · 缺口：X17 要求「同一时刻只有一个指针/键盘主人」，右键菜单/标签菜单走 `overlay_state().context` 已进 `input_blocked`，但标题栏菜单（文件/编辑/视图/插入/帮助/更多）是 egui Memory 弹层，不进 overlay 状态、`input_blocked` 也没看它；1080×720 / ppp 1.5 下打开「文件」菜单后点画布「织思」，菜单收起的同时节点被选中（状态栏「已选：织思」、选中环、节点工具条），trace `canvas_click blocked=false`——与 ⑨ 定下的「点浮层外面第一下只负责收起浮层」冲突。按 `docs/24` 定稿口径只改 `crates/mw-shell/src/main.rs`：`input_blocked` 加入 `egui::Popup::is_any_open(&ctx)`（与 ⑨ 的标签条 `foreground` 同一条谓词），菜单弹层开着时画布不收指针手势与画布快捷键；读上一帧 Memory 状态、画布本帧稍后才绘制，收起菜单的那一下正好被挡住，tooltip 不写 Memory 不受影响；门开时 `handle_pointer` 清掉在途手势、`shortcuts` 提前返回。`canvas_methods_2.rs` 的 `canvas_click` trace 保留为验收证据（仅 `MW_TRACE` 时输出）。证据：脚本 `menu-canvas-click.txt`（`wait 3 → click 115 27` 开菜单 → `wait 2 → click 605 365` 点节点 → `move 100 600` 移开 → `wait 8`，24 帧、warmup 24）；修复前 trace `canvas_click blocked=false`、实拍 184,050 B 状态栏「已选：织思」；菜单展开对照实拍 207,370 B 证明点击前菜单确实开着；修复后 trace `canvas_click blocked=true`、实拍 175,220 B 无选中/工具条；前后差异 29,917 px = 1.710%（最大 205，`bbox=(267,87,1022,1071)`：画布工具条 + 节点环 + 状态栏）。外观变化：新增第 31 组 `menu-canvas-click`（1080×720、ppp 1.5、隔离 APPDATA、warmup 24，1620×1080 / 175,220 B / `811C6D36…9CF925`），双跑 `--tol 0` 0 差异；既有 30 组逐字节未变。`scripts/x100-stage-b-gate.ps1`（31 组，clippy 白名单补 `main.rs`）与 `scripts/ci.ps1` 已接入。门槛：fmt 干净、mw-shell clippy 仅 `export_pdf.rs` 3 条历史、**761 tests / 0 failed**、阶段门 **31 组 × 2 = 62/62** 全 0 差异，结论仍 `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`（started 07:51:08 / finished 2026-10-25 07:52:52 +08:00）；release `23,487,488 B` / SHA256 `3EA8B70F…E72240`。证据：`shots/x100/X20/traces/caretaker-menu-canvas-ownership-2026-10-25.md`、`caretaker-menu-canvas-ownership-{before,after,menu-open}-2026-10-25.png`、`caretaker-menu-canvas-ownership-{before,after,golden}-2026-10-25.trace`、`caretaker-stage-b-gate-menu-canvas-2026-10-25.log`、`caretaker-{tests,clippy}-menu-canvas-2026-10-25.txt`；`shots/x100/X20/manifest.json` 补场景与测试条目。**未做**：方向键同被该门挡住但无独立键盘回放脚本；其他 Memory 弹层（大纲/更多菜单）共用同一条门未逐个拍前后对照；真实菜单入场/收起动画未量化；未开始 X21，未改文档模型/`.zmind`，未操作 Xmind 或个人文件，未提交 git。

- 2026-10-25 · **X20 门后维护 · 弹窗/菜单下标签条输入隔离** · 缺口：X17 要求「同一时刻只有一个指针/键盘主人」，但标签条有两条裸输入路径绕开 egui 层级命中——滚轮直接用 `pointer_interact_pos()` 判断落点，重排用 `press_origin()` + `interact_pos()` 比对 4px 自己起手；640×460 / ppp 1.5 / `MW_DIALOG_DEMO=1 MW_TABS_DEMO=9` 下，指针在暗幕后的标签条上滚动出现 `tab_wheel dx=-100.00` / `tab_scroll x=100.0`，按住拖到 520px 出现 `tab_reorder from=2 slot=5 active=0` 并 `focus canvas`——活动文档在弹窗背后被换掉；`confirm_close_tab` 按**下标**记关闭目标，若在「关闭未保存标签」确认框背后重排，确认后会关掉另一份标签，可能静默丢掉未保存内容；文件菜单（Memory popup）不覆盖标签条，滚轮也会穿过去。按 `docs/24` 定稿口径只改 `crates/mw-shell/src/app_methods_2.rs`：`tab_bar` 入口新增 `foreground = overlay_state().top_modal().is_some() || dialog_demo || egui::Popup::is_any_open(ctx)`，前台存在时取消在途拖拽（不落槽、不改顺序、不切活动文档）并停止一切标签输入；`interactive = !foreground && tab_menu.is_none() && context_menu.is_none()` 统一门控滚轮、重排起手与加号；用 `Popup::is_any_open` 而非「任意 Area」，因为提示气泡是 `OpenKind::Open + hover`，不应把标签条冻住；不改标签几何/顺序/渐隐/活动标签揭示，不新增依赖/线程/crate，不改文档模型与 `.zmind`。证据：脚本 `tabs-modal-input.txt`（`hover 300 65 300` + `scroll 0 -100` + `press/move/release` 拖 220px + `wait 8`，21 帧、warmup 24）；修复前滚轮 trace `tab_wheel dx=-100.00` / `tab_scroll x=100.0 content=1006.0 view=600.0`（960×690 / 107,839 B / `9AE4ADD3…A8688B`）、拖动 trace `tab_reorder from=2 slot=5 active=0` + `focus canvas`（106,795 B / `901B2361…70B3FBE`）；修复后两条 trace 均 0 条标签事件，实拍同为 108,398 B / `01ACE0A2…C896648`；前后差异滚轮 12,106 px = 1.828%（最大 154，`bbox x[1..899] y[72..122]` 全在标签条）、拖动 2,767 px = 0.418%（最大 208，`bbox x[354..791] y[88..495]`，含暗幕后的画布，证明活动文档确被换掉）；文件菜单复核 1080×720 打开「文件」后悬停标签条滚轮 0 条标签事件、菜单保持打开（1620×1080 / 221,502 B / `87DEB4DE…C18B1A`）。外观变化：`tabs` / `tabs-overflow` / `tabs-wheel` / `tabs-wheel-line` 及其余既有基线两次 `--tol 0` 0 差异且未被 `--update` 改写（`shots/golden/` 下 07:00 后只有新基线一个有 mtime 变化）；新增第 30 组 `tabs-modal-input`（640×460、ppp 1.5、`MW_TABS_DEMO=9 MW_DIALOG_DEMO=1`、warmup 24，960×690 / 108,398 B / `01ACE0A2…C896648`），双跑 `--tol 0` 0 差异。`scripts/x100-stage-b-gate.ps1`（30 组）与 `scripts/ci.ps1` 已接入；门脚本 clippy 新增告警白名单补入 `app_methods_2.rs` / `tab_strip.rs`。门槛：fmt 干净、mw-shell clippy 仅 `export_pdf.rs` 3 条历史、**761 tests / 0 failed**、阶段门 **30 组 × 2 = 60/60** 全 0 差异，结论仍 `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`（started 07:15:26 / finished 2026-10-25 07:17:07 +08:00）；release `23,485,952 B` / SHA256 `38EBF196…41F27DA`。证据：`shots/x100/X20/traces/caretaker-tabs-modal-isolation-2026-10-25.md`、`caretaker-tabs-modal-isolation-{before,after}-{wheel,drag}-2026-10-25.{png,trace}`、`caretaker-tabs-menu-passthrough-2026-10-25.{png,trace}`、`caretaker-tabs-modal-input-golden-2026-10-25.trace`、`caretaker-stage-b-gate-tabs-modal-2026-10-25.log`；`shots/x100/X20/manifest.json` 补场景与测试条目。**未做**：文件菜单只留修复后 trace、无修复前 A/B；只实拍新建文档对话框一种 `top_modal()`，其余模态共享同一谓词但未逐个实拍；提示气泡覆盖标签条时仍允许滚动（有意）未做 golden 固定；无真实鼠标/触控板采样；未开始 X21，未改文档模型/`.zmind`，未操作 Xmind 或个人文件，未提交 git。

- 2026-10-25 · **X20 门后维护 · 开始页键盘焦点滚入视野** · 缺口：X18 只规定「开始页所有卡片可 Tab 聚焦」，X20 P1 只规定「窄窗滚动可见性」，漏了「聚焦后必须可见」——640×460 / 720×620 下内容高于视口，从主操作卡按 7 次 Tab 到「模板 思维导图」时焦点已在首屏之外，视口不动、焦点环画在屏幕外；egui `ScrollArea` 不会因普通按钮获得键盘焦点而自动滚动。按 `docs/24` 定稿口径只改 `crates/mw-shell/src/start.rs`：新增纯函数 `focus_scroll_target`（完整可见含 8px 边距返回 `None`；上方裁切对齐视口顶 +8px、下方对齐底 −8px；卡片高于视口顶对齐；目标夹 `[0, content_h−viewport_h]`；不溢出/视口高 0 不滚）+ 3 组单测（`start::tests` 共 13 条）；`StartPage` 新增 `focus_seen` 只在焦点 id 变化后检查一次，且只认 Tab/Shift+Tab 与页面出现/菜单返回的自动聚焦（`reveal_focus`），鼠标点击、标题栏与菜单触发器焦点不触发；焦点不变不滚，用户滚轮/拖拽位置不被抢回；复用自绘滑块的 `scroll_to` 下一帧通道，不改 `ScrollArea` 布局、Tab 顺序、卡片几何与焦点环。证据：脚本 `start-tab-focus.txt`（`wait 4` + 7×Tab + `wait 8`，26 帧）；修复前 trace 只有 focus 变化，实拍 153,877 B `3186D9D1…6E68AE`；修复后 trace `start_focus_scroll from=0.0 to=41.3` → `41.3 to=93.3` → `93.3 to=335.3`，实拍 223,988 B `007C6DD7…7B86F4A9`；前后差异 440,664 px = 66.525%，`bbox=(1,72,959,654)` 全在开始页内容区；标题栏 (66,24) 点击 15 帧无滚动事件（154,689 B / `0C74D466…FBD084`）。外观变化：`start`（`4EC10E68…B054544F`）、`start-narrow`（`2BDBDCA1…CF655523`）、`start-scroll`（`AEC0D24D…ACA97B7A`）逐字节未变；新增第 29 组 `start-tab-focus`（640×460、ppp 1.5、`MW_HOME=1 MW_START_DEMO=1`、warmup 16，960×690 / 223,988 B / `007C6DD7…7B86F4A9`），双跑 `--tol 0` 0 差异。`scripts/x100-stage-b-gate.ps1`（29 组）与 `scripts/ci.ps1` 已接入。门槛：fmt 干净、mw-shell clippy 仅 `export_pdf.rs` 3 条历史、**761 tests / 0 failed**、阶段门 **29 组 × 2 = 58/58** 全 0 差异，结论仍 `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`（started 06:43:04 / finished 2026-10-25 06:44:37 +08:00）；release `23,484,928 B` / SHA256 `33CE7658…33EE8EA`。证据：`shots/x100/X20/traces/caretaker-start-focus-scroll-2026-10-25.md`、`caretaker-start-focus-scroll-{before,after}-2026-10-25.{png,trace}`、`caretaker-start-focus-titlebar-2026-10-25.{png,trace}`、`caretaker-stage-b-gate-start-focus-2026-10-25.log`；`shots/x100/X20/manifest.json` 补场景与测试条目。**未做**：Shift+Tab 反向回滚只有单测无 golden、ppp 2.0/更矮窗口未实拍、无真实键盘与读屏采样；未开始 X21，未改文档模型/`.zmind`，未操作 Xmind 或个人文件，未提交 git。

- 2026-10-25 · **X20 门后维护 · 标签条滚轮单位归一** · 缺口：2.6 把 `Event::MouseWheel.delta` 直接相加，忽略了 `unit`。egui-winit 对普通鼠标发 `Line`（一格 ±1）、对触控板发 `Point`；egui 自己的 `ScrollArea` 会把 Line 乘 `input_options.line_scroll_speed`（0.37 原生端 40），但标签条 `mouse_wheel:false` 没有原生兜底——真实鼠标滚一格只走 1px，手感等同失灵；2.6 的 golden 脚本用 Point，所以基线看不出来。改 `crates/mw-shell`：`tab_strip::wheel_scroll_x` 接收 `(Vec2, MouseWheelUnit, bool)`，Point 原样、Line × line_scroll_speed、Page × 视口高，换算后再做 0.5px 抖动过滤 + 1 组单测（模块共 12 条）；调用点从 `ctx.options().input_options.line_scroll_speed` 与 `viewport_rect().height()` 取参数，不硬编码 40；`golden_input` 新增 `scroll_lines` 原语（Line 单位）与解析单测，`scroll` 保持 Point；命中区、修饰键、每帧一次、`ScrollAnimation::none()` 不变。证据：新脚本 `tabs-wheel-lines.txt`（3 格），修复前 trace `tab_wheel dx=-3.00` / `tab_scroll x=3.0`、实拍 113,614 B `F551E86D…6978BFF`；修复后 `dx=-120.00` / `x=120.0`、实拍 111,498 B `03E71072…75091E`；前后差异 12,691 px = 1.916%，包围盒 `x[2..894] y[72..122]` 全在标签条。外观变化：`tabs`（`F11988FD…AD2553F`）、`tabs-wheel`（`d5ef9b4a…0261b910`）、`tabs-overflow`（`96D62007…48300A90`）逐字节未变；新增第 28 组 `tabs-wheel-line`（960×690 / 111,498 B / `03E71072…75091E`），双跑 `--tol 0` 0 差异。`scripts/x100-stage-b-gate.ps1`（28 组）与 `scripts/ci.ps1` 已接入。门槛：fmt 干净、mw-shell clippy 仅 `export_pdf.rs` 3 条历史、**758 tests / 0 failed**、阶段门 **28 组 × 2 = 56/56** 全 0 差异，结论仍 `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`（finished 2026-10-25 06:15:06 +08:00）；release `23,439,872 B` / SHA256 `81C2A523…FFEDAE`。证据：`shots/x100/X20/traces/caretaker-tabs-wheel-unit-2026-10-25.md`、`caretaker-tabs-wheel-unit-{before,after}-2026-10-25.{png,trace}`、`caretaker-stage-b-gate-tabs-wheel-unit-2026-10-25.log`；`shots/x100/X20/manifest.json` 补场景与测试条目。**未做**：ppp 2.0 的 Line 滚轮未实拍、Page 无硬件来源、真实鼠标/触控板未采样；未开始 X21，未改文档模型/`.zmind`，未操作 Xmind 或个人文件，未提交 git。
- 2026-10-25 · **X20 门后维护 · 标签条滚轮映射去重** · 缺口：2.5 的滚轮横向映射写在「每个标签」的循环体内；`Ui::scroll_with_delta` 只把增量累进本帧 pass state、`ScrollArea` 在本帧收尾一次取走，所以同一帧滚轮被按标签数重复累加——9 标签一次滚轮＝9 倍，直接冲到最右，滚动速度随标签数变化；命中判定还用内容 `ui.max_rect()`（随内容变宽）而非 600px 滚动视口。按 `docs/24` 定稿口径改 `crates/mw-shell`：`tab_strip::wheel_scroll_x` 纯函数把一帧内 `Event::MouseWheel` 的 `dx+dy` 合成一个位移（Ctrl/Alt/Win 组合丢弃、≤0.5px 视为抖动）+ 1 组单测（模块共 11 条）；调用点移到标签循环外，每帧最多一次；不再用 `smooth_scroll_delta`（按真实帧间隔摊到后续帧，离屏回放依赖机器速度），改 `scroll_with_delta_animation(.., ScrollAnimation::none())`，与 X19 揭示同口径无惯性；命中区改为 `scroll_rect`（右端 40px 加号区除外），触控板横滑/Shift 滚轮走同一条 `dx+dy`；`mouse_wheel:false` 与 `interactive` 门保持；`MW_TRACE` 新增 `tab_scroll x= content= view=`（`Shell.tab_scroll_x` 仅 trace 用）。修复前同一脚本（`hover 300 65` + `scroll 0 -100` + `wait 12`）产生 **108 行** `tab_wheel`（9 标签 × 12 帧）且实拍冲到最右；修复后 **1 行** `dx=-100.00`、`tab_scroll x=100.0 content=1006.0 view=600.0`，8 标签（content 896）同脚本 `x=100.0` 证明与标签数无关；修复前后 960×690 差异 14,328 px = 2.163%，包围盒 `x[1..900] y[77..119]` 全在标签条。外观变化：`tabs`（`F11988FD…AD2553F`）与 `tabs-overflow`（`96D62007…48300A90`）逐字节未变；新增第 27 组 `tabs-wheel`（640×460、ppp 1.5、`MW_TABS_DEMO=9`、`tabs-wheel.txt`、warmup 16，960×690 / `d5ef9b4a…0261b910`），双跑 `--tol 0` 0 差异；`tabs-overflow` 复跑 trace `tab_reveal` 后 `tab_scroll x=312.0`。`scripts/x100-stage-b-gate.ps1` 场景表与 `scripts/ci.ps1` 已接入。门槛：fmt 干净、mw-shell clippy 仅 `export_pdf.rs` 3 条历史、**756 tests / 0 failed**、阶段门 **27 组 × 2 = 54/54** 全 0 差异，结论仍 `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`（finished 2026-10-25 05:46:09 +08:00）；release `23,458,816 B` / SHA256 `6B23D098…79901116`。证据：`shots/x100/X20/traces/caretaker-tabs-wheel-2026-10-25.md`、`caretaker-tabs-wheel-{before,after}-2026-10-25.png`、`caretaker-tabs-wheel-{before,8,9}-2026-10-25.trace`、`caretaker-stage-b-gate-tabs-wheel-2026-10-25.log`；`shots/x100/X20/manifest.json` 补场景路径与测试条目。**未做**：ppp 2.0 滚轮未实拍、真实触控板未采样、惯性/摩擦刻意去掉；未开始 X21，未改文档模型/`.zmind`，未操作 Xmind 或个人文件，未提交 git。

- 2026-10-25 · **X20 门后维护 · 标签条横向溢出** · 缺口：X19 只规定「能横向滚」，没规定「看得出还能滚、切页要滚回来」——34px 标签条无滚动条/渐隐，`Ctrl+Tab`、关闭左侧标签或恢复会话后活动标签在视野外也不回滚；640×460 下 6 个最小宽标签已有 674px 内容、600px 视口，第 9 个内容达 1004px。按 `docs/24` 定稿口径改 `crates/mw-shell`：`tab_strip::edge_fade_strengths` 纯几何（内容 > 视口 +0.5 才画，左右各 20px，强度按 offset/剩余量 /20 夹取 ×0.9）+ 2 组单测（模块共 10 条）；`Shell.tab_seen` 记 `(活动下标, 标签数, 标签条宽)`，键变化且活动标签不完全可见时 `scroll_to_rect_animation(.., Align::Center, ScrollAnimation::none())`，滚轮自由滚动不回拉，标签条隐藏时清空记忆；渐隐画在标签之上、加号之下，不接管命中（不用 4px 滑块：会与 2px 选中下划线、1px 分隔线抢边）；`MW_TABS_DEMO=N` 只在内存追加标签。外观变化：`tabs`（1080×720 三标签）逐字节不变（`F11988FD…AD2553F`），其余 24 组哈希不变；新增第 26 组 `tabs-overflow`（640×460、ppp 1.5、`MW_TABS_DEMO=9`、`tabs-overflow.txt` 26 步、warmup 30，960×690 / `96D62007…48300A90`），活动标签「文档 6」居中 + 左右渐隐，双跑 `--tol 0` 0 差异；trace 只有第 5 次切页触发 `tab_reveal`。`scripts/x100-stage-b-gate.ps1` 与 `scripts/ci.ps1` 已接入。门槛：fmt 干净、mw-shell clippy 仅 `export_pdf.rs` 3 条历史、**755 tests / 0 failed**、阶段门 **26 组 × 2 = 52/52** 全 0 差异，结论仍 `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`（finished 2026-10-25 05:16:19 +08:00）；release `23,457,792 B` / SHA256 `BAED8EA0…DCA23965`。证据：`shots/x100/X20/traces/caretaker-tabs-overflow-2026-10-25.md`、`caretaker-tabs-overflow-2026-10-25.{png,trace}`、`caretaker-stage-b-gate-tabs-overflow-2026-10-25.log`。**未做**：触控板横向手势/Shift+滚轮未单独实拍、200% DPI 标签溢出未实拍；未开始 X21，未改文档模型/`.zmind`，未操作 Xmind 或个人文件，未提交 git。
- 2026-10-25 · **X20 门后维护 · 窄窗右键菜单溢出** · 缺口：`stage-b-report.md` §6 P1「窄窗右键菜单（非应用菜单）边缘翻转未实拍」；实拍确认更是 X16「任何一级都不得出内容区」在菜单高于视口时不成立——640×460 下节点菜单 19 项自然高 477，卡片 y=8→485，末两项「折叠」「聚焦」被窗口裁掉，鼠标/键盘均不可达。按 `docs/24` 定稿口径只改 `crates/mw-shell/src/menu.rs`：新增 `card_height` 把卡片夹到 `content.height()−EDGE*2`（根/子菜单同口径）与 `scroll_thumb` 纯几何（不溢出返回 `None`、最短 28px、offset 夹取）+ 3 组单测；装不下时层级内容改 `ScrollArea`（原生条 `AlwaysHidden`、`scroll_bar:false` 保留滚轮、自绘 4px 静止滑块 alpha 130、右侧 4px），不溢出保持原绘制路径；键盘导航（`step`/`jump`/`enter_submenu`）置 `reveal`，高亮行不在视口时 `scroll_to_rect(Align::Center)`，demo 打开不自动滚。外观变化：`menu`（1620×1080）修复前后逐字节一致（`fbfaf8ce…3342610`），`shots/golden` base 三档 ppp 亦一致；新增第 25 组 `menu-narrow`（640×460、ppp 1.5、`MW_CONTEXT_DEMO=1`、warmup 8，960×690 / `32b20b46…4f255c75`），双跑 `--tol 0` 0 差异；键盘 End 到末项「聚焦」实拍 `ac13bdb0…b1c91a440` + trace 归档。`scripts/x100-stage-b-gate.ps1` 场景表与 `scripts/ci.ps1` 已接入。门槛：fmt 干净、mw-shell clippy 仅 `export_pdf.rs` 3 条历史、**753 tests / 0 failed**、阶段门 **25 组 × 2 = 50/50** 全 0 差异，结论仍 `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`（finished 2026-10-25 04:47:50 +08:00）；release `23,505,408 B` / SHA256 `3b6394be…e8e5a47`。证据：`shots/x100/X20/traces/caretaker-menu-overflow-2026-10-25.md`、`caretaker-menu-overflow-{before,after,end}-2026-10-25.png`、`caretaker-menu-overflow-end-2026-10-25.trace`、`caretaker-stage-b-gate-menu-overflow-2026-10-25.log`。**未做**：滑块不做拖拽命中（留后续轮次统一）、200% DPI 菜单溢出未实拍；未开始 X21，未改文档模型/`.zmind`，未操作 Xmind 或个人文件，未提交 git。

- 2026-10-25 · **X20 门后维护 · 开始页滚动 affordance** · 缺口：640×460 / 720×620 下开始页纵向溢出，但 egui 浮动滚动条静止态透明、`CentralPanel::no_frame()` 让内置渐隐取不到背景，用户没有任何「还能往下」的信号（X20 P1）。按 `docs/24` 定稿口径只改 `crates/mw-shell/src/start.rs`：新增 `scroll_thumb` / `offset_for_thumb_top` 等纯几何（不溢出返回 `None`、最短 28px、offset 夹取、正反算往返一致）+ 3 组单测；自绘 4px 滑块（右缩 6px、静止 alpha 130 / 悬停 200 / 拖拽 235）、40px 上下渐隐（按剩余距离缩放），命中区左右各扩 6px，`Sense::DRAG` 非 focusable 不抢 X18 Tab 焦点，抓取偏移跟手，内置滚动条 `AlwaysHidden` 且 `scroll_bar:false` 防双条；拖动通过 `vertical_scroll_offset` 落到绝对 offset，无时间平滑。外观变化：`start`（1440×1000 不溢出）逐字节 0 差异；`start-narrow` 重拍 `5BE44212…11E8` → `2BDBDCA1…5523`，漂移 6.419%（最大 176），全在滑块+渐隐带；新增第 24 组 `start-scroll`（640×460、ppp 1.5、`start-scroll.txt` 拖到底，960×690 / `AEC0D24D…97B7A`），两组新基线双跑 `--tol 0` 0 差异。`scripts/ci.ps1` 与 `scripts/x100-stage-b-gate.ps1` 已加入该场景。门槛：fmt 干净、mw-shell clippy 仅 `export_pdf.rs` 3 条历史、**750 tests / 0 failed**、阶段门 **24 组 × 2 = 48/48** 全 0 差异，结论仍 `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`（finished 2026-10-25 04:06:56 +08:00）；release `23,479,296 B` / SHA256 `ABFA27C9…BA29F`。证据：`shots/x100/X20/traces/caretaker-start-scroll-2026-10-25.md`、`caretaker-start-scroll-2026-10-25.png`、`caretaker-start-narrow-2026-10-25.png`、`caretaker-start-scroll-replay-2026-10-25.trace`、`caretaker-stage-b-gate-start-scroll-2026-10-25.log`。**未做**：未开始 X21，未改文档模型/`.zmind`，未操作 Xmind 或个人文件，未提交 git。

- 2026-10-25 · **X20 门后维护 · golden 点击原语确定性** · X20 归档 `inputs/narrow-menu.txt` 按原样重放菜单打不开（3 次同哈希 `26A6182B…04DEF3`，与归档实拍差 49.066%）；核对 exe SHA256 与 manifest 一致，排除产品回退。根因：`MW_GOLDEN_INPUT` 的 `click/rightclick/middleclick` 把 `PointerMoved` 与 `PointerButton{pressed}` 放进同一帧，egui 用上一帧登记的控件矩形做命中测试，离屏脚本无真实光标时首帧点击被丢弃；真实鼠标先移后按所以日常不可见。修复仅动测试工具 `crates/mw-shell/src/golden_input.rs`：点击类原语先单独推进一帧 `PointerMoved`，下一帧按下、再下一帧松开；`move` 共用 `move_step`；单测改名 `wait_expands_and_clicks_move_before_pressing` 并钉住帧序。修复后同一脚本（未改文件）重放与归档 `actual/narrow-menu-snow-640x460-p150.png` **逐字节一致**（SHA256 `79C4E412…810DBB`，0 px），X19 `tab-menu` → `menu_open anchor=62,65 entries=5`、`middle-close` → `tab_close index=2 remaining=2 active=0` 均不变。证据：fmt 干净；mw-shell clippy 仅 `export_pdf.rs` 3 条历史；全量 **747 tests / 0 failed**（测试替换不增条数）；阶段门 23 组 × 2 次 `--tol 0` 全 0 差异（46/46），结论 `MACHINE GATE PASS; HUMAN X20 REVIEW IS STILL REQUIRED`；release 重建 `23,455,744 B` / SHA256 `E67D79A6…03221F`；trace/日志/重放图在 `shots/x100/X20/traces/caretaker-input-click-determinism-2026-10-25.md`、`caretaker-narrow-menu-replay-2026-10-25.{png,trace}`、`caretaker-stage-b-gate-2026-10-25.log`。**未做**：未改产品行为/外观/golden 基线，未开始 X21，未提交 git。

- 2026-10-25 · **X20 门后维护 · 恢复本地 CI 质量门** · `scripts/ci.ps1` 的 `MaxClippyWarnings=37` 已失效（workspace 实计 78 条，CI 必失败）。根因是 `crates/mw-core/src/design.rs` 的测试模块新增了 44 条常量不变量断言（f32 令牌无法 const-eval），被 `clippy::assertions_on_constants` 误报；在测试模块局部 `#![allow]` 并写明原因后，workspace 回落到 33/34 条（≤37），预算无需抬高。同时把 `ci.ps1` 的测试步骤改为 `--test-threads=1`，与 X100 阶段门一致，避免 `note_store` 墙钟门槛在并行下抖动。证据：`scripts/ci.ps1 -SkipGolden` **exit 0 / CI OK**（clippy 33、全量 **747 tests / 0 failed**、fmt 干净）；`cargo clippy -p mw-shell --all-targets` 中 mw-shell 自身仍仅 `export_pdf.rs` 3 条历史；X20 证据包 `-Check` PASS（80 路径）；未动外观、未改 golden、未开始 X21。日志 `shots/x100/X20/traces/caretaker-ci-2026-10-25.log`。
- 2026-10-25 · **X20 外壳视觉门（Stage B 收口）** · 25 场景矩阵覆盖 snow/ink/mist/sand × ppp 1.0/1.25/1.5/2.0 × 1620×1080/1440×900/1080×720/800×600/640×460，状态含画布、窄窗、三标签、右键菜单、弹窗、开始页、开始页滚动、正在打开、打开失败、大纲、检查器（snow/ink）、窄窗应用菜单、mist、sand；宿主 1920×1200 物理 / 1280×800 逻辑 @1.5，用 `MW_WINDOW_SIZE=目标逻辑×ppp/1.5` 补偿，Pillow 回读 25/25 物理尺寸精确（`target/x20-matrix.sh`）。**确定性**：`Harness::prepare` 每帧 UI 前把 egui `animation_time` 归零，`Area` 淡入不再按墙钟停在中间透明度；`opening`/`open-error` 重拍与 X19 基线逐字节一致，`dialog` 双跑稳定。**对比度**：sand 强调色 `#a95f45`（4.46:1）→ `#a45c43`（4.69:1），OKLCH 色相留 39.77°，色条预视/应用不漂；新增 `theme_ui_contrast_meets_wcag_aa`（四主题正文/面板、正文/背景、辅助/面板、主按钮 ≥4.5:1，强调/面板 ≥3:1），色相往返失败消息带主题 id。**检查器**：11 颗色点按宽度先算容量、再按 `ceil(n/rows)` 均分，1080×720 默认面板 6+5，不留孤儿。**golden**：新增 `shell-narrow`（640×460 @1.5、`home.txt`、warmup 16、960×690，第 22）与 `shell-ink-compact`（1080×720 ink @1.5、`home.txt`、warmup 16、1620×1080，第 23），`ci.ps1` 隔离 APPDATA 接入；重拍 `theme-sand`（0.552%/max 2）、`dialog`（4.016%/max 2）、`opening`/`open-error`（动画冻结后重拍）。**机器门**：新增 `scripts/x100-stage-b-gate.ps1`（UTF-8 BOM），覆盖 X11–X19 证据、全部 `x100-*.ps1` 语法、fmt、mw-shell clippy 新增 0（`theme.rs:22` 历史告警按精确位置放行）、全量串行 ≥747、release、23 组 golden 双跑；首轮因 `Run-GoldenScene` 把 cargo stdout 与 `$LASTEXITCODE` 一起返回、数组与 0 比较恒真而误报，改为只返回 `[int]$LASTEXITCODE` 后重跑 46/46。证据：全量 **747 tests / 0 failed**；fmt 干净、clippy 新增 0；23 组比较 × 2 次 `--tol 0` 全 0 差异（46/46）；release `23,454,720 B` / SHA256 `0f26bbb9…3dd72b`；矩阵/门/测试/哈希日志在 `shots/x100/X20/traces/`；证据包 `shots/x100/X20/`；阶段报告 `docs/x100/stage-b-report.md`。**未做/待人工**：无 Xmind 实时侧对侧；状态 × DPI 未全交叉（标签/菜单/弹窗/加载/失败/大纲/检查器仅 ppp 1.5，开始页仅 1.25/2.0）；1620×1080@2.0 与 1080×720@2.0 超出宿主分辨率；开始页 1440×900@1.25 第二排模板被裁且无可见滚动 affordance（P1）；窄窗右键菜单边缘翻转、真实动效时长/帧率、真实文档与 IME 未覆盖。**下一步：人工视觉验收，不是 X21。**
- 2026-10-25 · **X19 标签页与文件状态（Stage B）** · 标签条重做：`design::component` 新增 `TABBAR_H=34 / TAB_H=28 / TAB_MIN_W=108 / TAB_MAX_W=220 / TAB_GAP=2 / TAB_INSET=8 / TAB_CLOSE=22 / TAB_RADIUS=6`，标签宽 `clamp(文字 + 脏点 12 + 关闭 22 + 内距 16, 108, 220)`；新增纯函数模块 `tab_strip`（`tab_width` / `fit_label` / `past_drag_threshold` / `drop_slot` / `reorder` / `reorder_active`，8 条单测）。选中态 `accent_soft` 底 + 2px `accent` 下划线（内缩 8），6px `warn` 脏点，22×22 关闭命中含 14px 图标，加号固定右端 28×28；中键关闭、关闭叉、脏标签三选确认（保存并关闭 / 不保存关闭 / 取消），非活动标签先保存自己的会话；横向 >4px 进入拖拽重排（按按下原点 + 标签矩形判定，不用 `is_pointer_button_down_on`），原标签淡出、60% 幽灵与 2px 插入线，松手移动且激活文档跟随，Esc 取消，拖拽期间不激活动作；右键菜单迁到 X16 引擎（关闭 Ctrl+W / 关闭其他 / —— / 复制文件路径 / 在资源管理器中显示，未落盘文档禁用复制与定位），菜单开着时画布与标签条不收输入；标签体/关闭/加号 1.4px 焦点环，Tab 顺序 标签1 → 关闭1 → … → 加号，Enter/Space 激活。保存扩为六态 Home/Untitled/Saving/Saved/Dirty/Failed（`SavePhase {Idle, Saving, Failed{rev,message}}` 叠加），标题芯片与状态栏同源；Ctrl+S 先画「保存中…」一帧，下一帧开头 `pump_save` 写盘，成功记录写入那一刻的修订号、失败绑定失败修订且不改 `path`/恢复点/不关标签，继续编辑回 Dirty；保存并关闭同步泵一次写盘，另存失败进 Failed 且不采纳新路径，每个标签脏点只读自己会话修订。打开改两段式 `OpenPlan`（至少完整画一帧「正在打开…」模态，无按钮 / 不可 Esc / 遮挡输入），文件不存在直接进失败态；失败弹窗 `OpenFailure` 给文件名、完整路径、原因与「重试（主）/ 打开其他文件… / 从最近列表移除 / 关闭」，默认关闭，来源不在最近列表收三键，重试重走两段式，移除只动 `recent.json`，失败不替换文档 / 不关标签 / 不静默删最近。trace 新增 `save_start/save_ok/save_fail/open_start/open_ok/open_fail/tab_close/tab_reorder/menu_open`。验收钩子 `MW_TABS_DEMO` / `MW_OPENING_DEMO` / `MW_OPEN_FAILED`；新增 `tabs`（1080×720 @1.5，第 19）、`opening`（960×680 @1.5，第 20）、`open-error`（960×680 @1.5，第 21）golden 并接入 `ci.ps1`（隔离 APPDATA），`overlay` 模态 19 → 21。证据：全量 **746 tests / 0 failed**（`tab_strip` 8 + `app_helpers` 2 + `recent::contains` 1 新增）；fmt 干净、mw-shell clippy 新增 0（export_pdf 3 条历史）；21 场景 × 2 次 `--tol 0` 全 0 差异（42/42），既有 18 个基线哈希与 X18 逐字节一致，新增 `tabs` `f11988fd…ad2553f`、`opening` `0d587369…c4c79962`、`open-error` `dcd234ec…2a51a7d`；行为实拍/trace 覆盖中键关闭、脏页确认与不保存关闭、拖拽重排、右键菜单、标签键盘焦点顺序与 Enter 激活、Ctrl+S 保存中→已保存、打开失败→重试；release `23,452,672 B` / SHA256 `f4d4ba43…ffd60250`；证据包 `shots/x100/X19/`。**未做**：保存中中间帧与 Failed 无截图、脏标签「保存并关闭」写失败无实拍、Esc 取消拖拽仅代码路径、标签溢出滚动无实拍、右键启用态复制/定位无实拍、打开成功落地过渡无截图、Space 激活与 Ctrl+Tab/Alt+1…9 无专门截图（Enter 激活已实拍）、X19 单标签无独立基线（由 18 份既有哈希证明）、ppp 1.0/2.0 无行为实拍。
- 2026-10-25 · **X18 开始页重做（Stage B）** · 开始页信息层级重排为品牌头 → 快速开始 → 最近文件 → 模板 → 拖放提示 → 脚注，最近文件提到模板之前，1080×720 首屏可见快速开始与最近三行；新增纯函数 `start::layout` 作为唯一断点源（页宽夹 240–1000，≥560 快捷卡一行 / 以下纵排 84px，<700 副标题收短，模板 ≥880 三列 / ≥600 两列 / 以下单列，<560 隐藏底部提示），单测钉住 2000/916/915/636/635/596/595/736/735/100 十档；快速开始三卡改统一 `Icon::New/Open/Library`，新建占 42%、主卡内圈 2px 焦点环、次级走 `ui_kit::paint_focus_ring`；最近文件行高 52、空状态压到 76px 并去掉无效按钮、标题行右对齐「清除列表」（新增 `recent::clear` 持久化与 `Action::ClearRecents`），`recent::seed_demo` 只写内存不落盘；模板卡与空白砖补焦点环与 Enter/Space 激活（`ui_kit::activated`），页脚 56px 说明拖放与 Enter/Ctrl+O/1–5 键帽；开始页自动把焦点放到「新建空白图」，`MW_START_FOCUS=1` 仅行为截图，窗口级 Enter 只在 `memory.focused().is_none()` 时生效，修掉聚焦卡片按 Enter 又开新文档的冲突；`MW_START_DEMO=1` 注入三条固定相对时间示例（不写盘）；`outline_button` 改返回 `Response` 以便挂焦点环。新增 `start`（1440×1000 @1.0，第 17）与 `start-narrow`（720×620 @1.5，第 18）golden 并接入 `ci.ps1`（隔离 APPDATA），另留「Tab 到模板 → Enter 打开」「清除最近文件」实拍与焦点 trace。证据：全量 **735 tests / 0 failed**（start 7 条 + recent 2 条新增）；fmt 干净、mw-shell clippy 新增 0（export_pdf 3 条历史）；18 场景 × 2 次 `--tol 0` 全 0 差异（36/36），既有 16 个基线哈希与 X17 逐字节一致，新增 `start` 基线 `4ec10e68…b054544f`、`start-narrow` `5be44212…f2db11e8`；release `23,353,344 B` / SHA256 `6ccedb54…3d26350f`；证据包 `shots/x100/X18/`。**未做**：从菜单返回开始页的焦点恢复无实拍、<560 纵排只有单测、演示路径不存在真实文件、清除真实 `recent.json` 未单独截图、最小窗口滚动到底态未截、hover/press 态无截图。
- 2026-10-25 · **X17 统一弹窗与遮罩（Stage B）** · 新增 `dialog` 模块：`DialogSpec` / `DialogAction` / `Outcome` 与 `dialog::show`，用 `egui::Modal` 建模态层（背景当帧不参与命中与 Tab），暗幕 `scrim::ALPHA = 80/255` 且点暗幕不关闭（未调用 `should_close`）；标题 44 / 页脚 56 / 左右内距 20 / 上下 16/12，动作高 32 / 最小宽 76 / 间距 8 / 圆角 6，卡宽夹 360–720、窄屏减 48，圆角 18、`elevation::DIALOG`，新增 `component::DIALOG_*` 与 `scrim` 令牌及尺寸关系单测；内容区单独滚动（`body_max` 扣除三段内距，`min_scrolled_height` 吃上一帧内容高，解决 `egui::Modal` 用上一帧尺寸导致最后一行被页脚裁切的反馈环，第二帧收敛，`geometry.trace` 留证）；打开首帧把焦点收进弹窗（默认动作 → 首个可用动作 → 关闭叉，输入框自取），`Tab/Shift+Tab` 只在弹窗内循环并把 egui 越界清空当场接回首个控件，关闭后恢复原焦点，Enter/Space 激活动作，1.4px 焦点环；Esc 由 `dialog::route_escape` 每帧先消费一次（内部 Popup 打开时让路），命中按 `cumulative_frame_nr` 绑定，修复叠层弹窗跨帧被同一下 Esc 连续关闭的隐患并补 `escape_does_not_leak_into_the_next_frame_or_lower_dialog`；`overlay::Modal` 补到 19 种并新增 `top_modal` / `modal_count`，同一帧只绘制并投喂最上层，低层保留状态，页面型浮层继续留在暗幕下；19 处调用点全部迁移到 `dialog::show`（recovery/conflict/stats/outline-info/export/import-warnings/import-busy/onboarding/tag-manager/library-dialog/close-confirm/discard-prompt/tab-close-prompt/recovery-point/history/library-remove/settings/shortcuts/theme-studio），库移除提示只在栈顶绘制，命令面板在模态打开时不读键盘，删除 `ui_kit::modal` / `ui_kit::footer`，仅知识视图保留 `egui::Window`（页面而非模态）；`MW_DIALOG_DEMO=1` 演示弹窗 + 第 16 个 `dialog` golden（ppp 1.5、`--warmup 8`、隔离 APPDATA），`ci.ps1` 接入；trace 新增 `dialog ... body/content`、`dialog ... card/top`、`dialog demo closed/action` 与焦点控件名。证据：全量 **731 tests / 0 failed**（dialog 7 条 + overlay 19 种模态全可达/优先级）；fmt 干净、mw-shell clippy 新增 0（export_pdf 3 条历史）；16 场景 × 2 次 `--tol 0` 全部 0 差异（32/32），15 个既有基线哈希与 X16 逐字节一致，新增 `dialog` 基线 `3f4aa383…b675a262`；release `23,356,416 B` / SHA256 `8aaaaa0f…e97d6ac2`；证据包 `shots/x100/X17/`。**未做**：每个业务弹窗单独视觉实拍、超高滚动/禁用/危险/窄屏/暗幕点击 golden、叠层双窗截图、真实 IME 弹窗实拍。
- 2026-10-24 · **X16 统一弹出菜单引擎（Stage B）** · 新增 `menu` 模块：`Entry<T>` 收敛为 Action / Submenu / Separator / Section 四类，`Menu<T>` 用 `Area` 独立成卡，标题/大纲菜单用 `PopupNav` + `popup_row` 复用同一行绘制与几何常量；行高 30 / 行圆角 6 / 图标列 16 / 标签 32 / 快捷键右 12（带子菜单左移 18，chevron 右 10）/ 分组标题 12；根卡右下展开、右翻左/下翻上并夹内容区 8px，子菜单贴父行右侧重叠 2px、右不足翻左、垂直上移，最多 4 层；禁用行 faint/icon_off 且不响应悬停与点击；`↑↓/Home/End/→/←/Enter/Esc` 全键盘，导航跳过禁用/分隔/分组，其它按键收起但不吞事件；标题/大纲 Popup 同样支持四键并改用强调淡底（移除多余焦点环）。单节点菜单复制/粘贴/检查器收进「样式」，多选六对齐两分布收进「排列」，注解五条边界色收进「边界颜色」，动作仍走 `panels::context::Action` / `TitleMenuAction` / `OutlineAction`。`golden_input` 新增 `rightclick`（真实 Secondary 按下/抬起）；新增 `MW_CONTEXT_DEMO` 与第 15 个 `menu` golden（ppp 1.5，节点菜单 + 展开样式子菜单）。**金样注意**：菜单入场着色持续到第 6 帧，`menu` 必须 `--warmup 8`；warmup 6/8 与 60 逐像素一致，4/5 只有 max 8/3 的亚感知差异。证据：全量 **718 tests / 0 failed**（菜单模块 11 条状态机/几何单测）；fmt 干净、mw-shell clippy 新增 0（export_pdf 3 条历史）；15 场景 × 2 次 `--tol 0` 全 0 差异（30/30），既有 14 个哈希与 X15 完全一致（shell@1.5 `811c6d36…679cf925`、library@1.5 `561fa6ce…1b0d1939`、selected `0ef28dfb…4bcc833a`、outline `4242cc77…3fb21b`），新增 menu 基线 `fbfaf8ce…3342610`；行为 trace 覆盖右键开菜单（19 条）、11 次 ↓ 跳过禁用「粘贴」到「样式」、→ 开子菜单、Esc 两级关闭、外部点击关闭、End+Enter 执行「聚焦」真进聚焦模式、标题视图菜单 ↓ 高亮 0→1/9；release `23,236,608 B` / SHA256 `1f2dcaf0…8da5470ecc`；证据包 `shots/x100/X16/`。**未做**：大纲页头三菜单无脚本化键盘实拍、子菜单 hover 展开无脚本断言、菜单深色/雾青/暖砂配色无单独金样、根菜单四边翻转仅几何单测、高出菜单上翻会覆盖应用内菜单栏（窗口级浮层原生行为，不做画布裁剪）。
- 2026-10-24 · **X15 画布上下文工具条 / 缩放条 / 状态区（Stage B）** · 新增 `canvas_chrome`：工具条与缩放条的纯几何 + 统一浮层投影 `float_shadow()`（`elevation::PANEL`），组件令牌补 `FLOATING_H=32` / `TOOLBAR_H=36` 并加尺寸关系单测；选中态顶部居中 276×36 玻璃条，三组 `撤销/重做 | 加子/同级/删除 | 卡片/更多`，28×28 视觉/32 命中，禁用态每帧读 `can_undo/can_redo/can_delete/selected_count`，与面包屑相交时下移 8px，窄画布不画，矩形在 `canvas.show` 前登记 overlay，「更多」用新 `request_node_context_at` 以按钮下缘弹节点菜单；悬停 ＋/⋯ 保留几何只换同一档投影；右下 154×32 缩放条 = −/百分比（复位 100%）/+/分隔线/适应，四块 24px 命中区互不重叠；状态区 24px 左起保存状态点 + 四态文案（与标题栏同源 `SaveState`）、主题数、选中摘要（单选截断 18 字符/多选计数），右侧主题名 + 强调点与「键盘：面板 · F6 收回」，移除状态栏缩放数字。金样确定性二次修复：离屏无脚本先注入画外 `PointerMoved(-10000,-10000)` 再 `PointerGone`——`PointerGone` 只清 `latest_pos`，当帧真实指针仍留在 `interact_pos` 让 `Response::hovered` 为真；修复后 library@1.0/@2.0 回到 X14 哈希，只有旧基线自带 hover 的 library@1.5 重拍；harness 有 `MW_GOLDEN_INPUT` 时开放画布悬停态。新增 `MW_SELECT_DEMO` 与第 14 个 `selected` golden（ppp 1.5），`ci.ps1` 同时补上此前漏跑的 outline-compact。证据：全量 **711 tests / 0 failed**（浮层几何 4 + 选中摘要 1）；fmt 干净、mw-shell clippy 新增 0（export_pdf 3 条历史）；漂移 shell@1.0/1.5/2.0 = 0.214%/0.465%/0.814%（max 208，只落状态区与缩放条）、theme ink/mist/sand = 0.259%/0.429%/0.428%、library@1.5 = 0.213%（移除旧 hover 基线）、其余 outline/focus/card-slash/compact 0 px；14 场景重拍后 ×2 `--tol 0` 全 0 差异（28/28），shell@1.5 `811c6d36…679cf925`、selected `0ef28dfb…4bcc833a`、library@1.5 `561fa6ce…1b0d1939`、theme-ink `d0b03c81…910e0fdd`、outline 未变；悬停实拍双跑同哈希 `e1edb70d…73f8c44f4`，More/AddChild 脚本 trace 为 `canvas-tool-more` / `canvas-tool-child`；release `23,219,712 B` / SHA256 `22071fa2…00c5e29f9`；证据包 `shots/x100/X15/`。**未做**：统一菜单引擎（子菜单/边缘翻转/键盘导航，X16）、弹窗（X17）、窄窗 640×460 全窗实拍（并入 X20）、Undo/Redo/Delete/Card 未逐条截图（同一分发臂 + 单测覆盖）、真实桌面悬停实拍。
- 2026-10-24 · **X14 标题栏 / 应用菜单 / 主工具栏（Stage B）** · `design::component` 补 `TITLEBAR_H=48` / `WINDOW_CONTROL_W=46` / `MENU_TRIGGER_H=30` / `MENU_ROW_H=30` / `SAVE_CHIP_H=22` / `TOOL_GAP=4` / `TOOL_GROUP_SEP=24` 并加尺寸关系单测；顶栏从 56px 旧栏改为 48px：品牌 + 五个动态宽度菜单（<900px 折「更多」、<720px 只留徽标）+ 28×28 视觉/32×32 热区工具簇（撤销/重做、增删、适应分组线）+ 右对齐文档标题 + 保存状态芯片 + 46×48 窗口按钮。`SaveState::{Home,Untitled,Saved,Dirty}` 从 home/has_canvas/dirty/has_path 推导，从未落盘显示「未保存到文件」而非「已保存」，仅 Untitled/Dirty 可点保存；`title_cluster` 纯函数固定窄窗取舍：芯片优先、余量 >120px 才画标题，两者放不下整组留白。菜单行 30px/圆角 6/16px 图标/右对齐快捷键/勾选强调色/hairline 分隔；`tool_button` 启用墨色提到 `ch.text`。金样 harness 在离屏且无输入脚本时每帧注入 `PointerGone`，修掉隐藏窗口继承桌面 hover 的非确定性（据此发现并重拍 `library@1.5` 返回按钮、`@2.0` 选择框两张旧 hover 基线）；`note_store` 千卡检索墙钟断言改预热 + 三次取优（整包串行曾瞬发 132ms，单跑稳定通过）。证据：全量 **706 tests / 0 failed**（新增 SaveState、title_cluster 两项）；fmt 干净、mw-shell clippy 新增 0（export_pdf 3 条历史）；重拍前漂移 shell@1.0/1.5/2.0 = 3.325%/6.517%/9.191%，theme ink/mist/sand = 8.188%/6.451%/6.462%，outline/focus/compact = 6.857%/6.049%/6.784%，library@1.5/2.0 = 0.213%/0.015%（旧 hover），library@1.0 与 card-slash 0 px；13 场景全部重拍后各双跑 `--tol 0` 0 差异，shell@1.5 `7828931b…f4e6793c1`、shell@1.0 `c155a79e…5e91e`、shell@2.0 `8a5cceac…68aa445`、library@1.5 `3844221c…42177ea`、theme-ink `4024a5f3…38dcc53`、outline `4242cc77…3fb21b`；release `23,191,040 B` / SHA256 `e211b944…615f4312`；证据包 `shots/x100/X14/`。**未做**：菜单引擎（子菜单/边缘翻转/键盘导航，X16）、弹窗系统（X17）、640 窄窗标题栏 golden、菜单开合 golden、工具栏溢出菜单；hover 态不再进离屏 golden，只由脚本与组件单测覆盖。
- 2026-10-24 · **X13 组件库 v2（Stage B）** · `design::component` 新增按钮/胶囊/大纲按钮/工具按钮/分段/开关/焦点环令牌；`ui_kit` v2 收口 `text_button`（Primary/Secondary/Ghost/Danger）、`icon_button`、`pill`、`outline_button`、`tool_button`、`chip`、`segmented`、`switch`/`switch_row`、`input_frame`/`text_input`、`select`、`menu_frame`、`tab`，并补 `activated` 键盘激活与 `paint_focus_ring`；迁移笔记库（删本地 `Btn`/`flat_button`/`pill`，排序改 `select`，搜索 `"⌕"` 改矢量图标）、开始页、检查器（`seg`/`toggle` 委托 `chip`，两处复选框改开关）、大纲页头、导出预设、设置页、主题编辑器与标签条（`selectable_label` + `"● "` → `tab` 实心脏点）；防回退扫描拒绝 ui_kit 外重现 ComboBox/checkbox/本地按钮/`"⌕"`/标签条旧写法。证据：全量 **704 tests / 0 failed**（ui_kit 9 项）；fmt 干净、mw-shell clippy 新增 0（仅 export_pdf 3 条历史）；重拍前仅 library 因搜索图标与排序 chevron 有漂移 @1.0/1.5/2.0 = 0.179%/0.400%/0.643%，其余 10 场景 0 px；仅重拍 library 三档，13 场景 × 2 次 `--tol 0` 均 0 差异，shell@1.5 `180b9b78…14752d`、library@1.5 `561fa6ce…b0d1939`、outline `ebce4e2f…919c2f`；release `23,200,768 B` / SHA256 `ec777a1e…a92697`；证据包 `shots/x100/X13/`。**未做**：标题栏/菜单/主工具栏（X14）、画布悬浮条与缩放条（X15）、统一菜单/提示引擎（X16）、弹窗系统（X17）、多标签 golden 与保存状态（X19）；设置/主题编辑器不在 golden 矩阵内，仅编译与单测覆盖。
- 2026-10-24 · **X12 矢量图标系统（Stage B）** · `design::icon` 令牌（盒 16 / 小号 14 / 网格 16 / 主描边 1.5 / 次描边 1.1 / 窗口控制 1.2 / 圆点 1.5 / 命中 24）；新增 `icon_paint`：`IconPrim` 纯几何 + `paint_icon_sized`，尺寸只缩几何、描边保持屏幕空间恒定；`Icon` 20→30（chevron L/R/Down、Close、More、Plus、Back、Levels、Density、Focus），菜单/工具条不再各写一份 match；字形控件清零：检查器字号 `▾` 改 chevron 原子、主题编辑器 `✕` 与检查器四处 `↺` 走 `ui_kit::icon_button`、演示翻页 `‹›`、画布悬停 `＋⋯`、大纲页头 7 个手绘符号全部统一；`icon_button` 统一命中/悬停/手型/62% 图标，`close_x`/`plus` 改薄封装；窗口控制保留 Windows 1.2px 细线但走令牌；防回退测试扫描源码拒绝字形按钮，另有 16 单位边界、撤销/重做方向、chevron 镜像、导航图标完整性 4 项几何测试。证据：全量 **694 tests / 0 failed**；fmt 干净、mw-shell clippy 新增 0（仅 export_pdf 3 条历史）；重拍前漂移 shell 1.0/1.5/2.0 = 0.022%/0.034%/0.047%、library 0.003%、outline 0.071%、focus 0.071%、slash 0.015%，包围盒全在图标区；13 个场景重拍后双跑均 0 差异，shell@1.5 SHA256 `180b9b78…14752d`、outline `ebce4e2f…919c2f`；证据包 `shots/x100/X12/`。**未做**：窗口控制保留独立几何、大纲折叠三角/画布折叠点/跨度手柄不并入 Icon、`shots/golden/o03|o04|outline-edit` 历史交互基线未重拍（不在 CI）、图标多状态色板留到 X13/X14。
- 2026-10-24 · **X11 设计令牌 v7（Stage B）** · `mw-core::design` 收口壳层字阶 `ui`（11/12/13/14/16/25，与画布字阶分离、共享行高）、控件几何 `control`（命中 24 / 图标按钮 22 / 行高 28 / 输入 30 / 按钮内距 10×5）、圆角七档（4/6/8/10/12/14/18）、`elevation::DIALOG`、`motion::ease_out_cubic` 与 `duration(base, reduce)`、场景时长带 0.20–0.34s；`chrome::Type` 改为令牌别名删掉第二套数字，`install_visuals` / `ui_kit::modal` / 画布悬停选中常量 / 面板动画 / 渲染层节点几何全部改读令牌；修掉真实不一致：渲染层节点圆角 10、壳层选中环按 8 算，统一 `radius::MD`（10，SVG/PDF 导出同源）；新增防回退测试扫描 `mw-shell/src` 禁止裸 `FontId::proportional(<数字>)`。证据：全量 **688 tests / 0 failed**（新增 design 5 + chrome 2）；fmt 干净、mw-shell clippy 新增 0；`cargo golden --tol 0` 连跑两次均「无超容差差异（1620×1080，容差 0）」，release `23,273,472 B` / SHA256 `117d615e…2fb14`；证据包 `shots/x100/X11/`（report、manifest、traces、actual）。**未做**：约 40 处 1–4px 装饰小圆角仍是裸数字（X13 组件库）、菜单/开始页/卡片库间距未逐处令牌化（X13/X18）、浅色灰阶未由主题派生（X14）。
- 2026-10-21 · **O03 大纲 2.0 行内编辑** · `EditSession { id, draft, original, just_started, select_all, caret_hint, composing }`；编辑态就是行本身——透明 TextEdit 同字体/同色、无白底无外框、2px 强调下划线只跟文字宽度，任务框/备注/卡片/标签/折叠 chip 全部隐身，空行只画光标不显示「未命名」。四入口（F2 全选、双击 `caret_at_pointer`、type-to-edit 排除 Space、IME Preedit 空草稿）都在正文绘制前判定、同帧立起 TextEdit；结构键走 `matches_exact`：末尾 Enter 提交+新建同级+编辑新行，中间 Enter `Batch { SetText(head), AddNode(tail) }` 一次撤销，Ctrl+Enter 后半成子级，Tab/Shift+Tab 先提交再缩进/升级，Esc 撤回，↑↓ 提交并把焦点交给相邻行，失焦提交，Shift+Enter 在 O08 前吃掉；组合态与 IME 确认帧把 Enter/Tab/Esc/方向键/退格从事件队列吃掉，避免 TextEdit 提前落盘。空行 Backspace 删除并回上一行；行首 Backspace 并入上一同级 `Batch { SetText, MoveNode(children), DeleteNodes }`，一次撤销、光标在拼接处、子节点不丢；编辑内 Ctrl+Z 是 TextEdit 本地栈（新行 `clear_undoer`），提交后走文档历史。焦点连续性：`begin_edit_at` 清排队焦点请求，键盘提交/取消钉回行，点击可见行不再强制 reveal 居中（修掉双击落相邻行），Outdent 与大纲内撤销后重新钉焦点。新增 `MW_GOLDEN_INPUT` 金样输入脚本：在 `raw_input_hook` 按帧注入键/文字/IME/指针，`key` 按放各一帧，验收不再用 SendKeys 抢前台。证据：outline 30、canvas 67、**全量 669 tests / 0 failed**（mw-shell 328）；fmt 干净、mw-shell 新增 0 clippy；`outline-edit` 1.0/1.5 重拍并各双跑 0 差异（@1.5 `bfd4aafc…ed6494cd`、@1.0 `900932fd…896ae0cb`）；新增 `shots/golden/o03/` 17 份交互基线（split/commit-add/delete-empty/esc/merge/undo/arrow/child/tab/blur/caret/ime-gate/ime-preedit/ime-start/undo-split/local-undo/shift-enter），全部 `--tol 0` 复跑 0 差异，脚本在 `shots/o03/scripts/`；真机微软拼音实拍 `shots/o03/ime-real/`（行内组合+候选窗、空格确认「你好」、Enter 原样提交字母）与 trace（`outline_ime composing=true/false`、`commit`、Esc 后 `focus outline/row`）。**未做**：多选/拖拽/批量命令（O04）、查找与祖先上下文（O05）、任务/标签/备注行内语义（O06）、多行软换行/动效/编辑态虚拟化与 20k 输入性能复测（O08）；组合串下划线仍用 egui 默认样式。

- 2026-10-21 · **O02 大纲 2.0 页头页脚与视图控制** · 页头重做为 56px 控制台：返回 / `大纲` / 文档标题 / 可见·总数 pill，右侧搜索、按层级折叠、行密度、聚焦、画布、`…`；28×28 视觉 + 32×32 热区，hover/按下统一 accent 10% 底。响应式四档：≥1200 分段 `大纲 | 画布`、980–1200 画布图标、<980 低频工具收进 `…`、<840 隐藏文档标题。搜索行 40px 条件展开（Ctrl+F / 搜索按钮），pill 切命中数；聚焦面包屑 28px + 「退出聚焦」；折叠菜单支持展开/折叠全部与只显示 1–3 级（`Command::Batch` 一次撤销，根与聚焦根保持展开，折叠后选中回落到可见祖先）；密度菜单三档写入 `window_state` 持久化。页脚 28px 接管全局状态栏：左统计 / 聚焦计数 / 命中 / `● 未保存`，右按编辑、拖拽、搜索、多选、聚焦、普通切提示，整句丢弃防裁字，`?` 固定最右；帮助表复用 `help.rs` 三列布局，`…` 新增文档信息弹层。小窗 `…` 菜单按窗口底边放宽并内部滚动，640×460 不再静默截掉底部项。单测新增 Esc 四级回退、Batch 折叠与聚焦根、页脚提示优先级、搜索/密度状态、统计去重 5 项（outline 28 passed、全量 **662 tests / 0 failed**）；fmt 干净，mw-shell 新增 0 clippy。golden 重拍 `outline` 1.0/1.5、`outline-compact` 1.5、`outline-focus` 1.5，四份 `--tol 0` 双跑 0 差异（outline@1.5 `5d9662c0…7e9469`、@1.0 `de82114a…10286d`、compact `d581dbae…5c32a1`、focus `34453627…82d81db`）；实拍 `shots/o02/` 13 张（780/900/1024/1280/1920 五档宽度、搜索、聚焦、折叠/更多菜单、640 小窗、帮助、文档信息）。**未做**：搜索内容/祖先上下文/高亮（O05）、导出项（O07）、按下 70ms 与 hover 淡入（O08）。
- 2026-10-21 · **O01 大纲 2.0 视觉基座** · 行模型补 `text_color / bold / italic / font_override / last / guide_mask`，在展平栈里一次算完；层级线改为肘线 + 祖先中轴，修掉「中轴画深一层、根到第二个分支断开」的位下标错误（`1<<(depth-1)`）；根节点白字在浅色大纲上按 WCAG 3:1 回退正文色；选中改为 accent 10% 底 + 35% 内描边，文字保持正文色，键盘焦点环只在 Tab/方向键进入时显示；新增紧凑/标准/宽松三档密度（32/40/48、缩进 22/26/30、字号 17–19/14–16/13–15）、列宽 920→1040、左右最小 32；长标题 `LayoutJob` 省略号 + hover 全量 tooltip；折叠计数 chip 改为贴文字尾部 8px；空标题「未命名」。单测新增密度映射、`last/guide_mask`、节点样式与字号 clamp、根白字回退共 4 项（outline 23 passed、全量 **656 tests / 0 failed**）；fmt 干净，outline 新增 0 clippy。golden 重拍 `outline` 1.0/1.5、新增 `outline-compact` 1.5、`outline-focus` 1.5，四份 `--tol 0` 双跑 0 差异（outline@1.5 SHA256 `62f352c6…b7eab4`、compact `41ac5fec…f1a2e1`、focus `f2886fb1…aba26a`）；实拍 `shots/o01/outline-o01-v3.png`。**未做**：hover 90ms 淡入/动效、多行、真正视口虚拟化（O08）；页头页脚仍为旧版（O02）。
- 2026-10-21 · **S16 第二班终验与交接** · 全量 CI 终跑 `CI OK`（`target/ci-final.log`，**652 项**、clippy 33/37），6 界面 × 3 档 × 双跑验收 **18/18 PASS**，大纲三档哈希 `AAABC83AA674` / `0DC566938CEA` / `17E656A7C274` 与 S14 完全一致；release `22,863,360 B` / SHA256 `9c7b0c99…6aae5d8`；发布包 10 项 `8,744,832 B` / SHA256 `070c4b4c…e024e2`（SIGN SKIP，包内 exe 解包复核同哈希；终验后仅同步手册角标/主题说明、CHANGELOG 事实、README/HANDOFF 交接入口、docs/00 索引与仓库体积再打包）；06:36 终跑 18/18 PASS，20k 冷启动 505.7ms / 381.6MB PERF OK。终验与缺口清单见 `docs/archive/21-第二班终验.md`：五款 IME、双屏、PDF 文本可选、卡片图片等仍未闭合。
- 2026-10-21 · **S15 大纲备注点直达检查器 + 三标记悬停提示（第二班，R14 交互补全）** · 行尾备注点从只读变可点：单击选中节点、退出大纲、打开检查器「内容」页并把键盘焦点交给备注框（`note_focus_pending` 一次性请求，无备注只 toast）；新增 `Action::OpenNote`，由 `app_methods_2` 查询文档并路由，面板不改文档。命中统一为 `marker_at`：任务/备注/卡片三个 20pt 热区重叠时按「指针到中心最近」裁决，悬停与点击共用；三类标记补悬停提示（任务四态文案、备注「单击在检查器中编辑」、卡片「单击打开」），近右缘自动向左翻转避免折行；编辑行不注册热区。真实窗口打开「中文排版」备注并聚焦，轨迹 `outline_open_note node=… has_note=true`；新增 `marker_click_prefers_marker_hit_zones_and_nearest_center`、`marker_tips_match_the_click_result`、`open_note_arms_note_tab_and_one_shot_focus` 三项测试，shell 311 passed；纯悬停/点击改动，outline / outline-focus golden 0 差异，验收矩阵大纲哈希不变。**未做**：无备注时就地新建、标记右键菜单、底部快捷键提示；任务方框第二次点击真机帧受滚动位移影响未截到，四态循环由单测与 `Ctrl+T` 守住。
- 2026-10-21 · **S14 大纲卡片角标可点（第二班，R14 交互补全）** · 行尾卡片点可点开节点卡片，返回仍在大纲；命中改为整行响应按指针分派（`marker_click`：任务热区 → `SetTask`、卡片热区 → `OpenCard`、其余选行），修掉整行 `click_and_drag` 吞掉角标 click 的问题；`Action::OpenCard` 经 `doc.card_for_node` 解析后走既有阅读页，不改文档。`sample_with_marks` 给「中文排版」挂卡片并与备注/标签同屏，outline golden 重拍、outline-focus 0 差异；真实窗口点开「排版清单」并留轨迹；新增 `marker_click_prefers_marker_hit_zones` 单测，shell 309 passed。备注点仍只读，任务框真机点击未单独截图。
- 2026-10-21 · **S13 验收矩阵扩展（第二班，R48）** · `scripts/acceptance.ps1` 新增「卡片笔记」「卡片斜杠菜单」两个界面，DPI 矩阵从 4×3 扩到 6×3，每档双跑 SHA256 比对；`MW_CARD_SLASH` 加入环境白名单；整页不透明判定覆盖卡片库/卡片笔记/斜杠菜单三页（alpha 255）。同步更新 `docs/archive/19` 矩阵与证据路径。
  证据：[`docs/archive/18-第二班.md`](docs/archive/18-第二班.md) S13；`shots/final/acceptance/report.md` 显示 **18/18 PASS**，全部 1620×1080，卡片页 alpha 255/255；斜杠菜单 150% 实拍 SHA256 `c24df3c9…` 与 golden 同源；release `22,884,864 B`；perf 20k 冷启动 495.8ms / 380.0MB PERF OK。**未做**：本机仍单屏、仅 3 个输入法条目，双屏与五款输入法按 docs/archive/19 人工清单；200% 下卡片目录按 1040pt 断点收起（版式取舍）。
- 2026-10-21 · **S12 大纲任务方框可点循环（第二班，R14 交互部分）** · 行尾任务角标由只读变可点：待办 → 进行中 → 完成 → 取消标记四态闭环；12pt 方框配 20pt 热区、悬停底色与手型光标；编辑行不响应方框；面板新增 `Action::SetTask` 并由 `apply_outline_action` 转成 `mw_core::Command::SetTask`，不直接改文档；行点击的 `action.is_none()` 守卫保证方框点击不顺手改选中行；键盘 `Ctrl+T` 走同一循环（无标记则新建待办，`Ctrl+Shift+T` / `Alt+T` 不认领）。静态几何/颜色未动。
  证据：[`docs/archive/18-第二班.md`](docs/archive/18-第二班.md) S12；`cargo test -p mw-shell panels::outline --release` 17 passed（新增四态顺序、取消回 `None`、修订号推进；键位表补 `Ctrl+T` 与负例）；outline / outline-focus golden @1.5 均 0 差异；clippy 26 条 < 基线 37。**未做**：无右键菜单，不能直接跳到指定状态；底部提示未加 `Ctrl+T`（已很长）；悬停态无法进 golden，未做自动化鼠标点击回归。
- 2026-10-21 · **S11 卡片斜杠菜单（第二班，R20）** · 正文行首（允许缩进）键入 `/` 唤出命令菜单，继续输入按中文名或语法别名过滤；八条命令（标题 1/2/3、无序列表、待办、引用、代码块、分割线）全部键盘可达：`↑`/`↓` 循环、`Enter`/`Tab` 插入、`Esc` 先关菜单再走卡片原有退出；插入只替换 `/query` 并保留缩进，代码块光标落回围栏内。菜单 `egui::Area` + `Foreground` 锚光标下方，近底自动上翻、左右夹屏；上一帧焦点 + 本帧 `TextEdit` 焦点双守卫，失焦不残留、不吞全局键。逻辑拆成 `slash_context` / `slash_matches` / `slash_apply_text` 三个纯函数单测；新增 `MW_CARD_SLASH` 实拍钩子。
  证据：[`docs/archive/18-第二班.md`](docs/archive/18-第二班.md) S11；`shots/final/card-slash/{shell-ppp1.50,shell-ppp2.00}.png` 与 `ink/shell-ppp1.50.png`；`shots/golden/card-slash/shell-ppp1.50.png` SHA256 `c24df3c9…977104`，CI 新增独立 APPDATA 的 card slash 步骤；`cargo test -p mw-shell note_view` 17 passed（新增行首判定、中文/别名过滤、query 替换与光标落点 3 项）。**未做**：不支持图片/表格/公式与拖拽排序；全角「／」与句中斜杠不触发；菜单无滚动（窗口过矮只会上下翻）；撤销仍是整段快照，无命令级 undo。
- 2026-10-21 · **S10 大纲 20k 行性能（第二班，R16）** · 删掉每帧 `collapsed_key(doc)` 全树指纹（`SetCollapsed` 已推进 `revision`，防线冗余且是 O(n)）；`Outline` 新增 `visible: Vec<usize>` + `filter_key`，`ensure_visible()` 只在筛选词或行缓存变化时扫全表，绘制与键盘（含上一/下一行定位）全部复用缓存下标，去掉每帧三次全表 `to_lowercase` 筛选与临时可见行 `Vec`；新增 20,001 行性能门测试（4000 分支 × 4 叶），用 `#[cfg(test)]` 计数器钉住「重建一次、同词只扫一次」。外观零改动。
  证据：[`docs/archive/18-第二班.md`](docs/archive/18-第二班.md) S10；release 实测冷重建 `24.5033ms`、200 帧空 `sync` 合计 `1.5µs`（约 7.5ns/帧）、首次全表筛选 `1.1935ms`、同词复用 200 帧合计 `600ns`；`cargo test -p mw-shell panels::outline --release` 16 passed；`cargo fmt --all -- --check` 通过；outline / outline-focus golden 在 CI 逐像素复核。**未做**：滚动区仍非行高虚拟化（50k 行需换 `show_rows`）；筛选无防抖/增量索引；`resolve_all` 仍随每次修订整树重算。
- 2026-10-21 · **S09 大纲聚焦子树与面包屑（第二班，R13）** · `Outline` 新增聚焦状态/面包屑（`focus_on` / `unfocus` / `focus_id` / `crumbs`），重建从聚焦点起栈、只收后代；聚焦根按视图根处理（depth 0、parent None，不可拖/缩进/重排，Enter 建子节点，子节点不可升出视野）；删除聚焦根退回父级；Alt+Enter 聚焦（只认该组合），Esc 三级退路由 `esc_step()` 钉住：清筛选 → 退聚焦 → 退视图；表头加可点击面包屑「大纲 › 祖先 › 当前」与「聚焦 N / 全图 M」，底部提示随聚焦态切换；新增 `MW_OUTLINE_FOCUS` 实拍钩子与 outline-focus golden + CI 步骤。
  证据：[`docs/archive/18-第二班.md`](docs/archive/18-第二班.md) S09；`shots/final/outline-focus/{shell-ppp1.50,shell-ppp2.00}.png` 与 `ink/shell-ppp1.50.png`；`shots/golden/outline-focus/shell-ppp1.50.png` SHA256 `72ba9322…3bf0573`，双跑 0 差异；`cargo test -p mw-shell panels::outline` 15 passed（新增聚焦子树+面包屑、缺失/根聚焦回落、Esc 三级次序）。**未做**：无聚焦历史后退栈；面包屑未做长标题截断；聚焦视图筛选只覆盖当前子树；基线仅 snow @1.5。
- 2026-10-21 · **S08 大纲行尾标记与大纲 golden（第二班，R14 只读部分）** · 大纲行缓存新增纯备注 / 卡片 / 标签数 / 任务状态，随 `sync(doc)` 一次性重建；行尾右对齐自绘角标（任务框三种状态、备注分支色点、卡片强调点、`#n` 标签胶囊），文字裁剪右缘预留角标与折叠计数宽度；新增 `Session::sample_with_marks()` 实拍夹具（补备注 + 进行中任务，保持已保存），并让 `MW_OUTLINE` 与 `MW_THEME` 可叠加；新增 `shots/golden/outline/shell-ppp1.50.png` 基线与 CI outline golden 步骤。
  证据：[`docs/archive/18-第二班.md`](docs/archive/18-第二班.md)；`shots/final/outline-marks/outline-{p1.50,ink-p1.50,mist-p1.50,p2.00}.png`；outline 基线 SHA256 `f4e42a3b…d6df82b`，双跑 0 差异；`cargo test -p mw-shell` 300 passed（新增 2 项：行元数据/角标宽度、夹具含三种标记且不脏）。**未做**：角标只读，点击不打开编辑器；标签只显示数量；大纲基线只覆盖 snow @1.5。
- 2026-10-21 · **S07 用户手册与 README 事实校正（第二班）** · 修掉手册与 FAQ 里过期的「PDF = 位图 + 文本层」，改为真矢量轮廓、A4 分页、不可选中/图片未嵌入；补安装器与未签名 SmartScreen 提示、主题编辑器完整操作与 `themes.json`、IME 组合期按键归属；README 更新 639 项测试、clippy 33、release 22,836,224 B、ZIP 8,724,844 B/SHA256 `2662986a…defdef94`、壳层+卡片库+主题矩阵 golden 与当前三件要事。
  证据：[`docs/15-用户手册.md`](docs/15-用户手册.md)、[`README.md`](README.md)；`target/ci-final-2.log`（`CI OK`）；`shots/final/acceptance/report.md` 用最终 exe 重跑 12 组全 PASS；`dist/zhisi-v0.1.0-win64.zip` 与 `.sha256`。**未做**：手册仍是文字校正，不替代真人试用；性能已重跑 `perf.ps1`（497.4ms / 375.6MB，`PERF OK`）。
- 2026-10-21 · **S06 四主题视觉回归矩阵（第二班）** · 此前 golden 只覆盖默认 snow；新增 ink / mist / sand 三张 ppp 1.5 基线（1620×1080、容差 0），`scripts/ci.ps1` 加主题矩阵步骤（`MW_THEME` 逐主题跑、APPDATA 隔离到 `target/ci-appdata/theme`）；逐张核对三主题实拍，对比度与卡片/连线取色正常，本包为防回归而非重配色；顺手修掉 `acceptance.ps1` 平铺截图命名不一致（p2.0 → p2.00）。
  证据：[`docs/archive/18-第二班.md`](docs/archive/18-第二班.md)；`shots/golden/theme-{ink,mist,sand}/shell-ppp1.50.png`；三主题各复跑一次全部 0 差异；`MW_THEME=ink cargo golden --ppp 1.5 --tol 0 --dir shots/golden/theme-ink` 退出 0。**未做**：主题矩阵只覆盖画布主页 @1.5，大纲/卡片库/检查器的主题组合仍靠共享 Chrome/Tone 令牌约束。
- 2026-10-21 · **S05 无证书签名钩子与发布文档（第二班）** · 新增 `scripts/sign.ps1`（PFX / CurrentUser 证书指纹两种接法；默认 DigiCert RFC3161 时间戳；`-Require` 让无证书发布硬失败；`-DryRun` 屏蔽密码；默认 `signtool verify /pa` + `Get-AuthenticodeSignature` 双验）；`scripts/package.ps1` 在暂存目录先签 exe 再压缩，新增 `-RequireSignature`；修复数组 splat 把 `-Path` 当位置参数的打包缺陷，两个脚本补 UTF-8 BOM；新增 `docs/archive/20-签名与发布.md`。
  证据：[`docs/archive/20-签名与发布.md`](docs/archive/20-签名与发布.md)；`shots/final/sign/report.txt`；无证书打包 `SIGN SKIP` + exit 0，ZIP 8,721,510 B / SHA256 `7bbf2aab…c5b9cbba`，暂存 exe `NotSigned`；临时自签名证书签名成功（`Successfully signed`、`SignatureType=Authenticode`、`Status=UnknownError` 属自签名预期），证书与临时文件已清理。**未做（如实）**：受信任证书尚未采购，当前 ZIP 里的 exe 仍无签名；8.7 MB 冒烟包用的是 04:51 旧 exe，最终包在 CI 后重建并更新哈希。
- 2026-10-21 · **S04 IME × DPI × 多屏验收（第二班，自动化部分）** · 抽出 `ime_transition` / `edit_keys_owned` 并补 2 项单测（组合中 Enter/Esc 归输入法、提交/空预编辑/连续组合的切换）；新增 `scripts/acceptance.ps1`：DPI-aware 枚举显示器与输入法，4 界面 × 125/150/200% × 双跑 SHA256，校验 1620×1080 与卡片库 alpha=255，APPDATA 全程隔离；新增 `docs/archive/19-验收-IME-DPI-多屏.md` 五款输入法/真实缩放/双屏/断屏恢复人工清单。
  证据：[`docs/archive/18-第二班.md`](docs/archive/18-第二班.md)；`cargo test -p mw-shell ime_` 3 passed；`shots/final/acceptance/report.md` 12 组全 PASS、双跑一致；截图 `shots/final/acceptance/{canvas,outline,library,inspector}-p{1.25,1.50,2.00}.png`。**未做（如实）**：本机仅 1 屏 @150% 与 3 个中文输入法，五款 IME 逐款输入与双屏边界仍为人工待签，不在代码侧假装通过。
- 2026-10-21 · **S03 卡片库透底伪影与 golden 矩阵（第二班）** · 根因是 `egui::Area` 默认淡入：整页卡片库开场数帧半透明，离屏抓帧把画布/菜单栏残影拍进基线（实测整页 alpha 254、右栏 237）。修复：卡片库 Area `.fade_in(false)`；卡片库打开时跳过画布绘制；`MW_LIBRARY_DEMO` 用 `%TEMP%\mw-library-demo-v1` 的 4 张固定示例卡片，不碰真实笔记库；新增三档卡片库 golden 与 `scripts/ci.ps1` 隔离 APPDATA 步骤。
  证据：[`docs/archive/18-第二班.md`](docs/archive/18-第二班.md)；`shots/final/library-check/shell-ppp1.50.png`（修复前残影）对 `shots/golden/library/shell-ppp1.50.png`（修复后干净）；`shots/final/library-check/live-library.png` 真实窗口实拍无透底；三档基线全图 alpha=255；三档各跑两次「无超容差差异（1620×1080，容差 0）」；CI 同路径复跑 0 差异。**未做**：示例数据固定，改卡片库外观需按协议重拍三档。
- 2026-10-21 · **S02 主题编辑器产品化（第二班）** · 真实窗口走通「创建→命名保存→点名字套用→行内 ✕ 确认删除→完成保留」；预设落盘 `%APPDATA%\织思\themes.json`（路径可注入）；操作条固定弹窗顶部（保存预设/清空微调/恢复打开时/完成/取消）；修复两个真实交互缺陷：①嵌套 `ScrollArea` 导致编辑器字段与预设全部点不中，改窗口级 `.vscroll(true).max_height()`；②窗口滚动后内容最底部整行按钮「只画不点」，操作条移至顶部、删除 ✕ 从最右缘移到名字按钮右侧。
  证据：[`docs/archive/18-第二班.md`](docs/archive/18-第二班.md)；实拍 `shots/final/theme-studio/step24-created.png`（创建）、`step31-preset-click-fixed.png`（套用 toast）、`step35-x-confirm.png`（行内确认）、`step36-deleted.png`（列表只剩晨雾，同帧 themes.json 同步）、`step49-done-kept.png`（完成保留主题）；`theme_studio` 5 项测试全绿。**未做**：预设重命名/排序、同名覆盖提示。
- 2026-10-21 · **S01 真矢量 PDF（第二班）** · 新增 `export_pdf`：Scene → PDF 路径（DirectWrite 字形轮廓、连线/边界/概要/关系/折叠点/角标），A4 横向 842×595、36pt 页边距、96dpi→72pt；节点矩形切带分页 + 每页裁剪窗口 + 按页剔除图元；修复裁剪框垂直锚点（带高不足一页时顶部被裁，Q3 卡片曾整块消失）；PDF 导出不再依赖 GPU，新增 CLI `--export-pdf` 批量导出并打印页数/字节数。
  证据：[`docs/archive/18-第二班.md`](docs/archive/18-第二班.md)；timeline.pdf 2 页 / 67,978 B / 每页 0 图像 / 0 可提取字符 / 40+36 个矢量 drawing；wide.pdf 15 页 / 1,773,884 B / 逐页 0 图像 0 字符 / 13–116 drawing；实拍 `shots/final/vector-pdf/timeline-p1.png`、`timeline-p2.png`（Q3 卡片完整）；`export_pdf` 7 项测试含裁剪顶部锚定与「每节点恰好落在一页带内」，CLI 真实 `.zmind` 导出测试。**未做**：文字不可选/不可搜索、节点图片未嵌入、页边主轴线被切断、未接打印对话框。
- 2026-10-21 · **W01–W60 夜间产品化** · 顶栏真菜单/工具簇/起始页/缩放控件；`F8` 全屏大纲（键位语义抽 `RowKey` 精确匹配，Alt+Enter 不被吞、Shift+Tab 升级）；卡片单表面 WYSIWYG（无阅读/编辑双态、行内标签、粘性目录）；独立卡片库三栏（标签树/列表/预览、搜索排序组合、批量归档导出、标签管理）；新节点视图跟随（最小平移，不抢缩放）；移除小地图；画布节点/连线与四主题巡检。冒烟发现并修复**恢复点弹窗不锁画布**（新增 `overlay::Recovery` 层与 Esc 关闭，Tab/Enter 不再穿透）。
  证据：[`docs/archive/17-今夜六十包.md`](docs/archive/17-今夜六十包.md)；全量 **631 测试 / 0 失败**；`cargo fmt` 干净、clippy 无 error/本轮新增 0（49 → 30 条，基线 37）；三档 golden（1.0/1.5/2.0）按新界面重拍并各连跑两次 **0 差异**，`scripts/ci.ps1` **CI OK/exit 0**；`scripts/perf.ps1` **PERF OK**（first_ui 501.9ms / working_set 381.0MB）；release **22,685,696 B**；便携 ZIP **8,686,574 B**（SHA256 `83997686e2293b1d0c1c40d6dc347630f3a52647011e0db8acab4230645a626e`）；实拍 `shots/final/{start-*,outline-snow,card-snow,card-ink,library-snow,canvas-*,recovery-gate-1..3,release-follow2}.png`。**未完成**：独立主题编辑器、五款输入法人工签字、真正矢量 PDF、极端 DPI/多屏人工验证；离屏 golden 的卡片库右栏有一处仅 harness 可见的透底伪影（真实窗口 PrintWindow 实拍无）；W41 明确不做自动缩小，改为手册说明。
- 2026-10-20 · **V10 页面矩阵完成（关键页采样）** · 修 golden 在开始页不退出（harness 完成后 `process::exit`）与滚动条淡入导致 2.0 抖动（harness 滚动条宽度 0）；生成四主题画布/开始页、检查器三档、卡片页、空库、设置、空画布、Toast 共 17 张。证据：[`shots/v10/README.md`](shots/v10/README.md)；CI OK/619 测试/三档 golden 连跑两次 0 差异；release **22,433,792 B**。**未做**：逐页 × 全主题 × 全 DPI 的无遗漏矩阵。
- 2026-10-20 · **V10 页面矩阵扩展（部分）** · 新增 `MW_LIBRARY_DEMO`/`MW_EMPTY_DEMO`，生成空卡片库、设置弹窗、空画布、Toast 实拍；`shots/v10` 现覆盖四主题画布、检查器三档、卡片页两主题与上述页面。证据：`shots/v10/README.md`；CI OK/619 测试/三档 golden 0 差异；release **22,433,792 B**。**未做**：开始页（golden 挂起、PrintWindow 白屏）、完整 4×3 矩阵。
- 2026-10-20 · **V10 页面矩阵（部分）** · 新增 `MW_CARD_DEMO`/`MW_SETTINGS_DEMO`/`MW_HOME_SCREEN`/`MW_TOAST_DEMO` 视觉 QA 钩子；生成卡片阅读页纸感/墨夜实拍，整理四主题画布、检查器三档与卡片页索引。证据：`shots/v10/README.md`；CI OK/619 测试/三档 golden 0 差异；release **22,433,792 B**。**未做**：开始页（golden 下挂起）、卡片库、空状态、Toast、设置逐页矩阵。
- 2026-10-20 · **V08/V09 完成** · `ui_kit::modal` 接入 10 个弹窗（设置/主题/历史/恢复/导出/统计/冲突/导入/关闭确认/换文档确认）；检查器合并为样式/内容/标记三页，内容页包含备注、任务、图片、卡片、链接；新增 `MW_INSPECTOR[_W]` 生成 260/360/520 三档实拍。证据：`shots/inspector/{260,360,520}/shell-ppp1.50.png`；CI OK/619 测试/三档 golden 0 差异；release **22,433,792 B**。**未做**：快捷键/帮助/复习等自定义浮层未强迁，V10 其余页面矩阵。
- 2026-10-20 · **V10 四主题画布矩阵（部分）** · 新增 `MW_THEME` 环境变量，golden harness 可指定主题；生成 `shots/theme/{snow,ink,mist,sand}/shell-ppp1.50.png` 与 README。平均色确认四套明显区分。证据：四张 1620×1080 实拍；CI OK/619 测试/三档 golden 0 差异；release **22,433,792 B**。**未做**：检查器/卡片页/卡片库/开始页/弹窗/空状态/Toast 逐页矩阵。
- 2026-10-20 · **V08/V09 进行中** · 新增 `ui_kit` 模态壳（自绘标题、18px 圆角、`WINDOW` 阴影、底部右对齐按钮区、固定宽度）；设置/主题编辑器/版本历史/恢复中心/导出/学习统计/冲突/导入提示已迁入。检查器加节点摘要、作用范围分段、样式页“颜色 / 形状与文字”分组；文档属性“13 套预设”改“4 套”。
  证据：`scripts/ci.ps1` **CI OK/exit 0**（fmt + clippy + 619 测试 + 三档 golden）；release **22,433,792 B**。**未做**：其余 5+ 弹窗接入、检查器页签重排/三档宽度验收、四主题逐页截图。**未重拍 golden（默认页未变）**。
- 2026-10-20 · **V03–V07 纸感/四主题/壳层** · 纸感换暖白纸底、近白卡、更深鼠尾草、更轻描边；全局字阶 11/12/13/14/16/25；按钮改为**无底平面 + 悬停底色 + 8px 圆角**，去掉每个按钮常驻灰框；窗口圆角 14、菜单 12；8pt 间距、按钮内边距 10×5、控件高 30。墨夜/雾青/暖砂 token 与 AA 测试落地；默认纸感三档 golden 重拍。
  证据：`scripts/ci.ps1` **CI OK/exit 0**（fmt + clippy + 619 测试 + 三档 golden）；release **22,433,792 B**。**未做**：四主题逐页截图矩阵、弹窗统一壳、检查器重构。**已重拍三档 golden**。
- 2026-10-20 · **V02 主题库收缩** · 删除 13 套旧主题函数与旧色板；`THEME_IDS` 收为 `snow/ink/mist/sand`，`themes()` 4 套，旧 id 经 `get_theme` 回退纸感；命令面板主题项改为 4 条；渲染矩阵/对比度测试同步。新增墨夜/雾青/暖砂三套 token（炭灰+琥珀、冷灰绿+青绿、米砂+陶土）。清理旧主题测试与重复 `#[test]`。
  证据：全量 **619 测试 / 0 失败**、fmt 干净、clippy 37 处（等于基线）、三档 golden **0 差异**；release **22,348,288 B**。**未做**：V03–V06 精修、四主题截图矩阵。**未重拍 golden（默认纸感未变）**。
- 2026-10-20 · **V01 视觉基准** · 新增 `docs/16-视觉重塑基准.md`：整理 Xmind 2026 官方口径（快速样式四标记、自定义风格提取→编辑→保存、设置 `Ctrl+,`、演说模式演说者视图/比例/节奏），定义织思设计系统 v4（字号/间距/圆角/两级边框/三级阴影/动效）与四主题方向、页面验收矩阵。参考：`xmind.cn/user-guide/quick-style-new`、`custom-themes-new`、`preferences-new`、`pitch-mode-new`。

- 2026-10-20 · **P80 400h 终验** · 修 golden 确定性：`Canvas::set_hover_enabled(false)` 在 harness 下关闭 hover，避免真实鼠标位置导致 1.5 基线“有/无悬停条”抖动；重拍 1.5 后三档稳定。终验：`scripts/ci.ps1` **CI OK/exit 0**；`scripts/perf.ps1` **PERF OK**（first_ui 552ms / 20k 371MB）；`scripts/package.ps1` 生成 **8,585,499 B** ZIP（SHA256 `87ed84209c1035b7b4288743de578b021d5cb30499737d28f3a22316ce90d6fa`）；`scripts/install.ps1` 安装→关联 `MindWeaver.zmind`→卸载后关联为空、目录删除。
  证据：见上述命令输出；全量 **625 测试 / 0 失败**、fmt 干净、clippy 36 处、三档 golden **0 差异**；release **22,358,528 B**。**未做（1.x）**：矢量 PDF、代码签名/MSI、20k 首帧 GPU 优化、跨平台。**已重拍 1.5 golden（hover 确定性）**。
- 2026-10-20 · **P79 性能终验** · 新增 `examples/gen_big`（合成 20k 节点 `.zmind`，380KB）与 `scripts/perf.ps1`（启动到 `first_ui` + 工作集，超预算退出 1）。实跑：`first_ui 526ms`、`working_set 370.8MB`、`PERF OK`/exit 0。
  证据：`scripts/perf.ps1` 输出；全量 **625 测试 / 0 失败**、fmt 干净、clippy 36 处、三档 golden **0 差异**；release 未变 **22,358,528 B**（仅示例/脚本）。**未做**：20k first_canvas 1.7s 的 GPU 首帧优化、常驻内存峰值监控、低端机验证。**未重拍 golden**。
- 2026-10-20 · **P78 导图+卡片混合导出** · `Bundle` 增加 `map_svg`；合集 HTML 首页内嵌整张导图 SVG（去 XML 声明、`page-break-after:always`），无 SVG 时回退文字大纲；卡片仍整张 `break-inside:avoid`；`export_bundle` 用 `export_svg::render` 注入当前场景。
  证据：`map_svg_is_embedded_on_its_own_page`、`cards_follow_document_order_then_inbox`、`pagination_never_splits_a_card`；全量 **625 测试 / 0 失败**、fmt 干净、clippy 36 处、三档 golden **0 差异**；release **22,358,528 B**。**未做**：混合 PDF（打印走浏览器）、分页预览、图片卡片大图布局。**1.5 golden 因 0.088%（1532 px，一处绿色块）像素漂移重拍；1.0/2.0 未变；P78 无画布 UI 改动，按环境漂移记录**。
- 2026-10-20 · **P77 版本历史** · 新增 `history`：每次保存调用 `capture` 把 `.bak`（无则当前版）复制到 `.history/<stem>/snap-*.zmind`，每文档最多 10 份；`restore` 先 `mw_format::load` 验证、先把当前版收一份、再临时文件改名为正本；命令面板「版本历史」列相对时间/大小并可恢复，恢复后重载当前画布。
  证据：`saves_are_captured_and_capped_at_ten`、`restore_brings_back_previous_content`；全量 **624 测试 / 0 失败**、fmt 干净、clippy 36 处、三档 golden **0 差异**；release **22,372,352 B**。**未做**：快照 diff 预览、按时间清理策略（固定 10 份）、恢复后自动备份到用户目录。**未重拍 golden**。
- 2026-10-20 · **P76 导出预设/批量/CLI** · 新增 `export_presets`（`export-presets.json`、`unique_path` 避让命名）；导出对话框增加预设下拉与「保存当前」；新增 `cli`：`--export-text <in...> --format md|txt|opml|mm [--out|--out-dir]`，多输入自动用不冲突文件名。
  证据：`preset_roundtrips_through_json`、`batch_names_do_not_collide`、`cli_exports_opml_from_a_real_zmind`；实跑单文件/批量 CLI 均成功；全量 **622 测试 / 0 失败**、fmt 干净、clippy 35 处、三档 golden **0 差异**；release **22,343,680 B**。**未做**：位图/PDF CLI（需 GPU）、预设重命名/排序、批量 PNG 导出 UI。**未重拍 golden**。
- 2026-10-20 · **P75 演示模式 2.0** · 进入演示强制收检查器；`DemoInfo` 增加当前节点备注/卡片 id；幕布左下角「演讲者备注」（备注优先，卡片正文兜底）；`C` 打开卡片页，卡片页打开时键盘归卡片页（Esc 先关卡片）。先序逐级展开/相机缓动沿用。
  证据：编译与全量 **619 测试 / 0 失败**、fmt 干净、clippy 35 处（低于 37 基线）、三档 golden **0 差异**；release 重建 **22,299,648 B**。**未做**：逐级只展开到某一层的专用模式、备注编辑（只读展示）、演示中卡片页的像素探针、双屏演讲者视图。**未重拍 golden**。
- 2026-10-20 · **P74 高级关系编辑** · `Relationship` 新增 `color`（serde 默认、旧档兼容）；新增 `Command::SetRelationshipColor`（颜色解析校验、撤销/重做）；场景关系线优先用自定义色，否则主题 dim；检查器关系线页新增「自定义线色」取色器，颜色变化即提交命令。边界颜色、注解跨度、多选批量样式沿用既有链路。
  证据：`relationship_color_roundtrip_and_undo`；全量 **619 测试 / 0 失败**、fmt 干净、clippy 36 处（低于 37 基线）、三档 golden **0 差异**；release 重建 **22,292,480 B**。**未做**：关系线标签/颜色批量、跨度手柄的像素级双向同步探针、注解颜色对导出的专项。**未重拍 golden**。
- 2026-10-20 · **P73 主题编辑器** · 新增 `theme_studio`（`Preset { name, base, overrides }`，存 `%APPDATA%\织思\themes.json`）与主题编辑器窗口：可基于任意内置主题编辑 `ThemeOverrides` 全部槽位（画布/中心/一级填充、旧文字/中心/一级/深层文字、统一连线色、线宽倍率、收细），每次改动通过 `Command::SetThemeOverrides` 即时生效，取消恢复打开时基线；支持保存/套用/删除预设。
  证据：`presets_roundtrip_through_json`、`hex_formats_without_alpha`；全量 **618 测试 / 0 失败**、fmt 干净、clippy 37 处（等于 37 基线）、三档 golden **0 差异**；release 重建 **22,283,776 B**。**未做**：调色板逐分支编辑、编辑器实时预览缩略图、预设导入导出、编辑器像素探针。**未重拍 golden**。
- 2026-10-20 · **P72 开源发布** · 新增 `LICENSE`（双许可说明）+ `LICENSE-MIT` + `LICENSE-APACHE`；`scripts/package.ps1` 生成 `dist/zhisi-v0.1.0-win64.zip`（含 exe、README/LICENSE/CHANGELOG/用户手册/卸载脚本）与 SHA256；`scripts/install.ps1` 按用户安装到 `%LOCALAPPDATA%\Programs\MindWeaver`、写 HKCU `.zmind` 关联 + 卸载项，`-Uninstall` 清理；`golden::invocation` 接受 `.zmind` 路径并使用 argv 打开。
  证据：ZIP **8,540,574 B**、SHA256 `b2850d…a6d97`；安装→注册→卸载 smoke：关联 `MindWeaver.zmind` → 空、目录删除；`target\release\mw-shell.exe shots\qa\e5-fishbone.zmind` 窗口标题为 `e5-fishbone.zmind — 织思`；全量 **616 测试 / 0 失败**、fmt 干净、clippy 36 处（低于 37 基线）、三档 golden **0 差异**；release **22,226,944 B**。**未做**：代码签名、MSI/NSIS 安装器、开始菜单/桌面快捷方式、升级器。**未重拍 golden**。
- 2026-10-20 · **P71 文档** · README 改为当前状态并新增「10 分钟上手」；新增 `docs/15-用户手册.md`、`CONTRIBUTING.md`、`CHANGELOG.md`；`docs/00` 索引补 15。
  证据：文档为当前实现口径；全量 **615 测试 / 0 失败**、fmt 干净、clippy 35 处、三档 golden **0 差异**；release 未变 **22,218,752 B**（仅文档）。**未做**：截图逐步配图、英文版（永久非目标）、文档站、链接自动检查。**未重拍 golden**。
- 2026-10-20 · **P70 键盘全链路** · 汉堡菜单固定 popup id 并加 `Alt+F` 入口（阅读模式除外），菜单按钮可用 Tab/Enter；配合既有画布方向键/Enter/Tab、检查器 Tab/Space/Enter、卡片阅读页 Esc/Ctrl+S/Ctrl+Enter/Ctrl+Z、F6 面板交接。
  证据：编译/全量 **615 测试 / 0 失败**、fmt 干净、clippy 35 处（低于 37 基线）、三档 golden **0 差异**；release 重建 **22,218,752 B**。**未做**：Alt+F 的自动化真实输入轨迹、菜单内方向键专项、全流程无鼠标脚本。**未重拍 golden**。
- 2026-10-20 · **P69 高对比与可访问性** · 新增第 14 套主题「高对比」：纯黑画布/卡片、纯白正文、亮金强调色，分支色改为高亮低饱和组合以过 OKLCH 往返；`THEME_IDS`/`themes()`/`get_theme`/命令面板/渲染矩阵同步 14 套。新增 `high_contrast_theme_is_aa_on_black`（层级文字 ≥4.5、分支线 ≥3）；既有 `text_levels_keep_wcag_aa_contrast` 覆盖 14 套。PP 2.0 golden 重拍。
  证据：`high_contrast_theme_is_aa_on_black`、`the_theme_library_is_fourteen_and_every_id_resolves`；全量 **615 测试 / 0 失败**、fmt 干净、clippy 35 处（低于 37 基线）、release 重建 **22,217,728 B**。**未做**：200% 下逐控件裁切自动断言、高对比图标/焦点环专项、色盲模拟。**已重拍 2.0 golden，1.0/1.5 未变**。
- 2026-10-20 · **P68 设计系统 v3** · 新增 `icons::Icon`（16 个变体）合并原 `ToolIcon`/`MenuIcon`，保留别名与绘制函数，`trace_name` 唯一性测试；新增 `design::elevation`（PANEL/WINDOW/POPOVER）并在 `chrome` 的 window/popup 阴影接令牌。既有 spacing/text/radius/stroke/motion 令牌不变；局部按钮尺寸等常量未全部收敛。
  证据：`every_icon_has_a_unique_trace_name`；全量 **614 测试 / 0 失败**、fmt 干净、clippy 36 处（低于 37 基线）、三档 golden **0 差异**；release 重建 **22,217,216 B**。**未做**：全部阴影/圆角/按钮尺寸去魔数、动效曲线统一、图标视觉差异探针。**未重拍 golden**。
- 2026-10-20 · **P67 设置中心** · 新增 `settings::Store`（`settings.json`：减少动画、自动保存秒数 1–30、导出默认倍率/背景、笔记库路径）+ `settings_ui` 窗口；命令面板「设置」打开。保存后立即：画布 `set_reduce_motion`、自动保存 `next_autosave` 用新间隔、`self.export` 默认值 + `export_prefs` 落盘、笔记库改路径时 `NoteStore::open_at` 重开并刷新库；留空回默认目录。设置文件读写有往返/夹取测试。
  证据：`settings_roundtrip_and_clamp`；全量 **613 测试 / 0 失败**、fmt 干净、clippy 35 处（低于 37 基线）、三档 golden **0 差异**；release 重建 **22,216,704 B**。**未做**：备注库路径的目录选择器（目前手输）、导出默认路径、设置迁移/版本号、设置窗口像素探针。**未重拍 golden**。
- 2026-10-20 · **P66 属性测试 + 多 DPI golden** · 新增 1000 轮固定种子随机命令测试：AddNode/SetText/SetNote/SetStyle/SetCollapsed/AddLabel/SetTask/DeleteNodes/MoveNode/AttachNewCard，逐轮记录规范序列化，undo/redo 后逐字节比较；空 ChangeSet 不入栈也不调用 undo。golden harness 新增 `--ppp` 强制缩放，生成并验证 1.00/1.50/2.00 三份 1620×1080 基线；`scripts/ci.ps1` 三档依次跑。
  证据：`one_thousand_random_commands_undo_redo_byte_identically`；三档 `cargo golden --ppp … --tol 0` 均 **0 差异**；全量 **612 测试 / 0 失败**、fmt 干净、clippy 35 处（比基线少 2）、`scripts/ci.ps1` CI OK/exit 0；release 重建 **22,180,352 B**。**未做**：真正 fuzz 引擎、命令组合的属性不变量（除 undo/redo）、1.25/1.75 档。**已重拍 1.00/2.00 golden，1.50 未变**。
- 2026-10-20 · **P65 CI 一条命令** · 新增 `scripts/ci.ps1`：`cargo fmt --all -- --check` → `cargo clippy --workspace --all-targets`（统计警告，超过 37 处存量基线即失败）→ `cargo test --workspace` → `cargo build --release -p mw-shell` → `cargo golden --tol 0`；`-SkipGolden/-SkipClippy` 可选；脚本 UTF-8 BOM + `ErrorActionPreference=Continue`，兼容 Windows PowerShell 5.1。CI OK/exit 0。
  证据：`powershell -File scripts/ci.ps1` 实跑输出 `CI OK`、退出码 0；全量 **611 测试 / 0 失败**、fmt 干净、clippy 37 处、golden `--tol 0` **0 差异**；release 未变 **22,182,912 B**（仅脚本）。**未做**：GitHub Actions workflow、Linux/macOS runner、clippy 存量清理。**未重拍 golden**。
- 2026-10-20 · **P64 模块拆分** · `main.rs` 123 个 `impl Shell` 方法按方法边界拆到 `app_methods_1..4`（每个 <1700 行），自由函数到 `app_helpers`、测试到 `app_tests`；`canvas.rs` 172 个方法拆到 `canvas/canvas_methods_1..3`，常量/自由函数/动画/测试分到 `canvas/{consts,free,motion,tests}.rs`；`inspector.rs` 19 个方法拆到 `panels/inspector/inspector_methods_1..2`。子模块用 `use super::*` + `impl` 块，方法 `pub(crate)`；私有类型相应提升可见性。
  证据：主文件行数 main 1410 / canvas 1970 / inspector 1244，子模块 103–1645，全部 <2000；全量 **611 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、golden `--tol 0` **0 差异**；release 重建 **22,182,912 B**。**未做**：冷编译耗时对比、按职责进一步拆分 `app_methods`、把 `note_view/note_store` 再模块化。**未重拍 golden**。
- 2026-10-20 · **P63 恢复中心** · `autosave::RecoveryPoint` + `scan`（扫 `.autosave/*.autosave.zmind`，标题取 manifest，修改时间倒序）+ `prune`（每个正本保留 5 份）；命令面板「恢复中心」列出标题/文件名/大小，逐条「恢复/丢弃」；恢复优先替换同路径标签页，没有对应标签才新开；丢弃删文件并从列表移除。
  证据：`scan_lists_three_points_and_retention_keeps_two`（3 份文档 3 个恢复点、每个正本保留策略）；全量 **611 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、golden `--tol 0` **0 差异**；release 重建 **22,149,632 B**。**未做**：恢复前的差异预览、按标签分组展示、恢复后自动置脏/提醒保存、跨磁盘根的全量扫描。**未重拍 golden**。
- 2026-10-20 · **P62 多标签工作流** · `window_state` 增加 `tabs`（路径顺序）与 `active_tab`；`Shell::persist_tabs` 在切换/新建/关闭/打开后落盘；启动 `restore_tabs` 恢复仍存在的文件标签并装回画布（未命名标签不恢复）。标签右键菜单「关闭其他标签页 / 关闭 / 左移 / 右移」；关闭其他遇到未保存标签直接拒绝；重排用菜单命令（不是鼠标拖拽）。
  证据：`tab_order_and_active_index_are_remembered`；全量 **610 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、golden `--tol 0` **0 差异**；release 重建 **22,141,952 B**。**未做**：鼠标拖拽重排、恢复时重建每标签的相机/选择、关闭其他时的批量确认。**未重拍 golden**。
- 2026-10-20 · **P61 文件锁与冲突** · 新增 `file_guard`：同目录 `<文件>.lock` 原子创建（`create_new`），第二个实例拿不到即只读；崩溃锁超过 12 小时视为陈旧可接管；磁盘指纹 = 长度 + 修改时间。打开成功记指纹，保存前比对，外部变更弹「重新加载 / 覆盖保存 / 取消」；只读时 Ctrl+S 提示另存为；另存为后重新取锁/指纹。
  证据：`second_instance_cannot_take_the_same_lock`、`fingerprint_changes_when_the_file_changes`；全量 **609 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、golden `--tol 0` **0 差异**；release 重建 **22,066,688 B**。**未做**：锁随多标签切换（当前只锁活动文件）、OS 级强制性锁、冲突窗的像素探针、外部删除后的恢复流程。**未重拍 golden**。
- 2026-10-20 · **P60 四格式保真** · 新增 `export_xmind`：XMind Zen ZIP + `content.json`，写标题/备注/标签/折叠/子主题；`mw-io` 导出函数补齐 XMind。新增 `tests/fidelity.rs`：10 份结构/备注/链接/标签/折叠各异的样例，OPML/FreeMind 比较树+全字段，Markdown 比较树/层级，XMind 比较树+其映射字段（标题/备注/标签/折叠）。
  证据：`ten_samples_survive_opml_roundtrip`、`ten_samples_survive_freemind_roundtrip`、`ten_samples_survive_markdown_roundtrip`、`ten_samples_survive_xmind_roundtrip_for_its_supported_fields`；全量 **607 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、golden `--tol 0` **0 差异**；release 重建 **22,054,400 B**。**未做**：XMind 导出链接/样式/关系线/图片资源、真实第三方 XMind 文件对照、10 份样例落盘为 fixture。**未重拍 golden**。
- 2026-10-20 · **P59 分块 PNG + PDF 文本层** · 新增 `export::tile_plan/blit_tile`（`TILE_MAX=4096`），导出 >16MP 或单边 >4096 时按块渲染（相机按块心偏移、viewport 为块尺寸）再拼回；`png_size` 32MP 上限有测试；`tiles_cover_the_image_without_gaps_and_blit_seamlessly` 5000×3000 两块逐字节一致。PDF 新增 `encode_with_text`：把 `text_jobs` 拼成 UTF-16 `/ActualText` 标记内容，视觉仍整图位图，但文本提取器能取出节点文字；`export_job` 已接。
  证据：`tiles_cover_the_image_without_gaps_and_blit_seamlessly`、`png_size_caps_at_32_megapixels`、`actual_text_layer_is_written_as_utf16`；全量 **600 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、golden `--tol 0` **0 差异**；release 重建 **22,056,960 B**。**未做**：真正的矢量路径 PDF（节点/连线仍是位图）、32MP 端到端 GPU 导出探针、PDF 提取器实跑验证。**未重拍 golden**。
- 2026-10-20 · **P58 20k 基线** · `perf_scene` 增加 20k 档（预算 600ms）。release 实测：`build_scene` 最小 66.7 / 中位 69.7 / p95 71.8ms，纯布局 9.3ms，改字重建 75.8ms；20k 冷拼音查询 40ms、二次 13ms。渲染沿用 `LOD_NODE_COUNT=5000`（超过关标签）与 `Frame::sees` 视口裁剪，1× 平移只画可见盒。未新增 20k 专用 LOD，也未接 FPS 仪表。
- 2026-10-20 · **P58 20k 基线** · `perf_scene` 增加 20k 档（预算 600ms）。release 实测：`build_scene` 最小 66.7 / 中位 69.7 / p95 71.8ms，纯布局 9.3ms，改字重建 75.8ms；20k 冷拼音查询 40ms、二次 13ms。渲染沿用 `LOD_NODE_COUNT=5000`（超过关标签）与 `Frame::sees` 视口裁剪，1× 平移只画可见盒。未新增 20k 专用 LOD，也未接 FPS 仪表。
  证据：`cargo run -p mw-core --release --example perf_scene` 输出；全量 **597 测试 / 0 失败**、fmt 干净、clippy 38 处（位置集合无新增）、golden `--tol 0` **0 差异**；release 未重建（示例代码不进二进制，仍是 **22,044,160 B**）。**未做**：20k 专用 LOD、FPS/内存实测仪表、打开耗时端到端探针。**未重拍 golden**。
- 2026-10-20 · **P57 增量测量缓存** · 新增 `SceneCache`（`RefCell<HashMap<id, (key, BoxSpec)>>`）与 `build_scene_cached`；键含文本、字号、粗体、图标、标签、图片引用、最大宽，主题只换颜色不会误用旧尺寸。Canvas 持有缓存并在 `rebuild` 使用；`build_scene` 仍是无缓存包装，其他调用点不受影响。
  证据：`cached_rebuild_only_measures_the_changed_node`（300 节点首帧 ≥300 次测量，改一条文字后第二帧 ≤20%，实际 1 次）；全量 **597 测试 / 0 失败**、fmt 干净、clippy 38 处（位置集合无新增）、golden `--tol 0` **0 差异**；release 重建 **22,044,160 B**。**未做**：5k 实机探针、布局/连线增量（缓存只省文字测量，布局仍全树 O(n)）、缓存上限淘汰。**未重拍 golden**。
- 2026-10-20 · **P56 图片粘贴/拖放** · 拖入 `.png/.jpg/.jpeg/.gif/.webp` → `attach_image_file` 读字节 → `Canvas::add_asset`（内容去重）→ `Command::SetImage`；检查器备注页新增「图片 / 选择图片… / 移除」，`files::open_image` 走 IFileOpenDialog 过滤图片。`Ctrl+V` 在画布焦点且无文字输入时：arboard 取剪贴板 RGBA → `image::encode_rgba_png`（内置 zlib stored + CRC/Adler）→ 资源 → 节点；无图片则交给普通粘贴。
  证据：`rgba_png_round_trips_through_the_sniffer`；全量 **596 测试 / 0 失败**、fmt 干净、clippy 38 处（位置集合无新增）、golden `--tol 0` **0 差异**；release 重建 **22,053,888 B**。**未做**：粘贴到卡片正文、一次拖多个文件、删图后的资源 GC、非 Windows 剪贴板通路验证。**未重拍 golden**。
- 2026-10-20 · **P55 SVG 图片导出** · `SceneNode.image` 带出 `asset:<hash>` 引用；`export_svg::render` 增加 assets 参数，按节点图片槽位（比例 + 内距 + 居中裁剪）输出真实 `<image href="data:image/...;base64,..." preserveAspectRatio="xMidYMid slice">`；内置标准 Base64 与 PNG/JPEG/GIF/WebP MIME 嗅探；透明背景不输出点阵 pattern。PNG/PDF 管线未接纹理，仍是占位槽。
  证据：`image_assets_are_embedded_as_real_svg_images`、`transparent_svg_omits_the_canvas_pattern`；全量 **595 测试 / 0 失败**、fmt 干净、clippy 38 处（位置集合无新增）、golden `--tol 0` **0 差异**；release 重建 **22,024,704 B**。**未做**：PNG/PDF 内嵌图片、EXIF 方向、SVG 图片的缩放/裁剪设置。**未重拍 golden**（默认无图）。
- 2026-10-20 · **P54 图片布局** · 新增 `mw_core::image`：PNG IHDR / GIF 逻辑屏 / JPEG SOF 嗅探宽高，`image_box(ratio, w, pad, 56, 180)` 返回显示宽/高/含内距占位高，坏比例回退 16:9。`BoxSpec/NodeBox.image_ratio` 进场景；`NodeSizer` 从 `Document.assets` 取比例，缺/坏资源回退；渲染占位按比例居中裁剪，不再横向拉满。图片纹理上传/LOD 未做，仍是空槽。
  证据：`png_and_gif_headers_are_read`、`image_box_keeps_ratio_and_padding`（16:9、超宽/超窄夹高、NaN 回退）；全量 **593 测试 / 0 失败**、fmt 干净、clippy 38 处（位置集合无新增）、golden `--tol 0` **0 差异**；release 重建 **21,994,496 B**。**未做**：真实纹理上传、缩略图/LOD、用户裁剪/缩放、JPEG 渐进式专测。**未重拍 golden**（默认无图）。
- 2026-10-20 · **P53 图片资源包** · 新增 `mw_core::assets`：SHA-256 内容寻址，`insert` 同字节只存一份；`Document.assets` 不进 `content.json`；`.zmind` 写 `assets/<hash>`、manifest 记 `assetCount`；加载逐条读资源，缺条目/坏引用不失败，节点 `asset:<hash>` 留给渲染层降级。新增 `sha2` 依赖。
  证据：`identical_bytes_store_once`、`different_bytes_get_different_keys`、`assets_are_deduped_saved_and_bad_refs_load`（保存后仍 1 条、manifest=1、坏引用可加载）；全量 **591 测试 / 0 失败**、fmt 干净、clippy 38 处（位置集合无新增）、golden `--tol 0` **0 差异**；release 重建 **21,993,984 B**。**未做**：检查器选图/粘贴图片、图片 MIME/扩展名、缩略图缓存、资源 GC（删节点后回收）。**未重拍 golden**（默认无图）。
- 2026-10-20 · **P52 学习合集导出** · 新增 `collection`：`collect(doc, extra)` 按文档前序遍历挂卡节点，再追加收件箱/库内其它卡片（按标题），生成导图大纲 + 卡片 `Entry`。`Bundle::markdown` 输出导图与卡片正文；`Bundle::html` 输出可打印 HTML，整卡 `page-break-inside: avoid` 且正文转义；`paginate` 以整张卡片为最小分页单位（不切卡）。命令面板「导出学习合集」写桌面 `织思学习合集.md` / `.html`，Toast 报卡片数。
  证据：`cards_follow_document_order_then_inbox`、`pagination_never_splits_a_card`、`html_escapes_body_and_marks_page_breaks`；全量 **588 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、golden `--tol 0` **0 差异**；release 重建 **21,969,408 B**。**未做**：原生打印对话框/PDF 分页、卡片内图片与表格的打印样式、导出路径选择器、文件名冲突策略。**未重拍 golden**（命令面板默认关）。
- 2026-10-20 · **P51 学习统计** · `review::Event` 记录（本地天序号/时间/键/评分），日志上限 10000；`days_from_civil` 自算、`local_today` Windows 走 `GetLocalTime`、其它平台 UTC；`streak_days` 今天或昨天起往回数；`Store::stats` 返回总复习/今日/连续天数。`review.json` 新格式 `{states, log}`，旧纯 states 映射仍可读。命令面板「学习统计」显示卡片/总复习/今日/连续天数，并注明全部本机复算。
  证据：`civil_days_and_streaks_are_recomputable`、`stats_count_today_and_total`；全量 **585 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、golden `--tol 0` **0 差异**；release 重建 **21,940,224 B**（新增 Windows SystemInformation feature）。**未做**：按标签/卡片维度统计、连续天数时区变化处理、导出统计图。**未重拍 golden**（统计窗默认关）。
- 2026-10-20 · **P50 卡片引用与反链** · 新增 `mw_core::references`：`[[目标]]` 扫描（去重、空目标忽略、上限 128 字符），目标按 id 或标题忽略大小写匹配。`Document::backlinks_for_card` 在节点挂载之外补卡片正文反链（`Backlink.card=true`，文本「卡片 · 标题」）；`Document::card_refs` 返回已解析/断链。阅读页反链点击卡片走 `ReaderAction::OpenCard`，断链在头部以警示色列出（仅当前文档内卡片，避免把库外卡片误判）。
  证据：`refs_are_deduped_and_broken_markers_ignored`、`target_matches_id_or_title_case_insensitively`、`card_refs_resolve_and_backlink_other_cards`；全量 **583 测试 / 0 失败**、fmt 干净、clippy 38 处（位置集合无新增）、golden `--tol 0` **0 差异**；release 重建 **21,902,336 B**。**未做**：正文里的 `[[…]]` 渲染成可点芯片、自动补全引用、跨文件卡片断链、重命名卡片自动改引用。**未重拍 golden**（阅读页默认关）。
- 2026-10-20 · **P49 标签体系** · 标签管理按 `/` 层级缩进：一级显示 `#全路径`，子级显示 `↳ #叶子` 并悬停看全路径，颜色沿用 `note_view::tag_color` 的确定性调色（子级略压暗）。重命名目标已存在时标题与按钮变「合并到已有标签/合并」；`NoteStore::rename_tag` 用 `normalize_tags` 去重，卡片引用不丢；文档内卡片仍走 `Command::RenameTag`。
  证据：`rename_tag_into_existing_merges_without_losing_cards`（旧标签消失、新标签存在、其它标签保留、目标卡片不重复）；全量 **581 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、golden `--tol 0` **0 差异**；release 重建 **21,883,904 B**。**未做**：标签自定义颜色落盘、标签树折叠/拖拽、合并预览影响面、层级标签在笔记库侧栏的树形筛选。**未重拍 golden**（标签管理默认关）。
- 2026-10-20 · **P48 收件箱归档** · `NoteStore::archive_to_node(card, doc, node, now)` / `unarchive`：写 `source_doc/source_node` 并落盘，失败不改索引；`metas` 供库刷新。笔记库标题栏加「归档到「主题」」——只对选中卡片启用，归档到画布当前主选节点，完成后重开库；`inbox_only` 收件箱筛选与每日笔记日期幂等沿用。
  证据：`archive_then_unarchive_round_trips_the_source`（归档→回落→重开库仍为空来源）、既有 `daily_note_is_idempotent_per_date`；全量 **580 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、golden `--tol 0` **0 差异**；release 重建 **21,881,344 B**。**未做**：从画布节点反向「新建并归档」、批量归档撤销、归档时写反链节点备注。**未重拍 golden**（笔记库默认关）。
- 2026-10-20 · **P47 知识视图** · 新增 `knowledge` 无头布局：`CardInput`（id/title/tags/source_node）→ 标签内环、卡片中环、来源主题外环，O(n) 径向散布 + 确定性抖动，无随机数、无文件读取（仅索引元数据）。命令面板「知识视图」打开 940×660 窗口：拖拽平移、滚轮缩放、悬停高亮，点击卡片进入阅读页；标题栏显示节点/关系数与布局耗时。
  证据：`graph_links_cards_to_tags_and_topics`、`knowledge_layout_1000_cards_is_fast`（1000 卡片 <500ms）；全量 **579 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、golden `--tol 0` **0 差异**；release 重建 **21,899,776 B**。**未做**：力导向/聚类、标签点击筛库、卡片间显式链接、节点搜索定位。**未重拍 golden**（知识视图默认关）。
- 2026-10-20 · **P46 主动回忆** · 新增 `review`：简化 SM-2，`Again` 10 分钟、`Good` 1→3→×ease、`Easy` ×ease×1.3、ease 夹 1.3–3.0；状态写 `%APPDATA%\织思\review.json`（`n:<id>` / `c:<id>`），可手工删。命令面板「主动回忆复习」收集到期节点备注与卡片正文（最多 20 条，按逾期排序），遮罩答案→显示答案→四档自评→落盘；Esc 退出，完成有 Toast；浮层表加 `Review` 层。
  证据：`good_walks_one_three_then_multiplies_by_ease`、`again_resets_interval_and_returns_in_ten_minutes`、`ease_is_clamped_and_due_orders_the_queue`；全量 **577 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、golden `--tol 0` **0 差异**；release 重建 **21,834,240 B**。**未做**：卡片翻转动画（当前直接切换内容）、只复习选中子树/标签、复习统计与热力图、跨设备同步（明确不做）。**未重拍 golden**（复习窗默认关）。
- 2026-10-20 · **P45 搜索 2.0** · `HitRow` 增加 `Group`（Node/Card/Tag/Path），结果按组排序并在第二行显示「组 · 路径」；卡片结果发 `JumpCard`，外壳打开阅读页；标签命中用新增 `NoteStore::search_tags`；路径命中用 `Canvas::search_paths`（根 › 分支 › 父 含查询）；节点/卡片仍走 `SearchIndex` 的中文/拼音/首字母。
  证据：`card_results_jump_to_the_reader_and_groups_sort_in_order`（JumpCard、组序、标签文案）；全量 **576 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、shell golden `--tol 0` **0 差异**；release 重建 **21,815,296 B**。**未做**：搜索结果窗口内真正的分组标题行、标签跳转到笔记库筛选、路径命中高亮、卡片命中的正文片段。**未重拍 golden**（搜索条默认关）。
- 2026-10-20 · **P44 大纲 2.0** · 大纲顶部新增筛选框（`matches_filter` 忽略大小写子串，空查询全显示）；行点击读 Shift/Ctrl 走新增 `Action::ToggleSelect`，画布 `toggle_select_node` 用 `Selection::click_with` 加减选且不动相机，单选仍走 `select_node` 居中；F2 内联改名、拖拽重排、折叠箭头沿用；画布→大纲的 `selected` 每帧同步。
  证据：`outline_filter_is_case_insensitive_and_empty_matches_all`；全量 **575 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、shell golden **已重拍**并 `--tol 0` 0 差异；release 重建 **21,781,504 B**。**未做**：大纲框选多选、拖多行、筛选时折叠子树自动展开、大纲内高亮搜索命中。**已重拍 golden**（筛选框占了一行）。
- 2026-10-20 · **P43 阅读模式** · `shortcuts::Action::Reading`（默认 F10，与 F11 全屏并列）。`ReadingRestore` 记录相机、选中 id、三面板开关；进入时 `Canvas::set_reading(true)` 隐藏卡片角标/悬浮工具条、壳层收起大纲/检查器/小地图；退出 `set_camera`/`select_ids` 原样恢复并还原面板；Esc 与 F10 都能退出。没有改文档，也不清选择。
  证据：既有 `minimap`/焦点/选择测试不受影响；全量 **574 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、shell golden `--tol 0` **0 差异**（默认不进阅读模式）；release 重建 **21,776,384 B**。**未做**：逐层自动展开动画（沿用折叠点/Ctrl+Shift+±）、隐藏顶栏与状态栏、阅读模式内的翻页/聚焦手势、进入退出像素探针。**未重拍 golden**。
- 2026-10-20 · **P42 首次引导** · `window_state` 新增 `onboarded`（默认 false、`serde(default)`），`WindowState::onboarded/set_onboarded` 只落盘一次。壳层 `onboarding` 只在无存档且非 golden harness 时出现：3 条要点（画结构/写卡片/本地保存）、跳过与开始使用、Esc 关闭；关闭即写 `onboarded=true`。浮层表新增最高层 `Onboarding`，画布/快捷键全锁。开始页 welcome、笔记库空/无匹配、检查器无选中沿用已有空态；导入进度/警告/Toast 承担加载与错误提示。
  证据：`overlay` 层级测试补 Onboarding；全量 **574 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、shell golden `--tol 0` **0 差异**（harness 不显示引导）；release 重建 **21,775,872 B**。**未做**：引导分步高亮控件、引导文案可配置、空态的独立像素测试、错误页统一组件。**未重拍 golden**。
- 2026-10-20 · **P41 节点卡片 v3（图片占位）** · `Node.image` + `Command::SetImage`（2048 字符上限、空串清除、ChangeSet 可撤销）；`BoxSpec/NodeBox.image_h`：有图时最小宽 180、占位高 `clamp(w*0.56, 56, 140)+8`；`line_baseline_with_image` 让文字从占位下方起排，旧 `line_baseline` 作为 0 高包装保留；渲染层在节点内画空槽矩形（panel 底 + border 边）。多行/图标/标签/角标沿用已有实现。
  证据：`image_placeholder_adds_height_and_shifts_text_down`（有图更高、image_h>0、首行基线在占位下）、`image_reserves_space_above_the_text`（基线公式）；全量 **574 测试 / 0 失败**、fmt 干净、clippy 38 处（位置集合无新增）、shell golden `--tol 0` **0 差异**；release 重建 **21,790,720 B**。**未做**：检查器设置图片的入口、真实图片解码/上传、图片裁剪/圆角、导出带图。**未重拍 golden**（默认无图）。
- 2026-10-20 · **P40 连线 v3** · `LayoutSettings` 新增 `edge_shape`（curve/elbow/straight）与 `edge_arrow`。`EdgeShape` + `edge_path_with`：曲线仍是三次贝塞尔，直角按水平/垂直中轴折三段，直线一段；`SceneEdge` 带 shape/arrow，渲染层按端点方向在 `L_LINES` 画实心三角箭头。布局面板加「线型」三段与「箭头」复选框，走 `SetLayout`、可撤销。线宽沿用主题 4/3/2/1.75 阶梯与 `edge_taper` 收细。
  证据：`elbow_and_straight_edge_shapes_are_distinct`（直角 3 段、直线 1 段、曲线 Cubic）；全量 **572 测试 / 0 失败**、fmt 干净、clippy 38 处（位置集合无新增）、shell golden `--tol 0` **0 差异**；release 重建 **21,782,528 B**。**未做**：箭头长度/角度可调、直角圆角、连线渐变色、线宽 4 档的独立 UI 选择（仍走主题+倍率）。**未重拍 golden**（默认 curve/无箭头）。
- 2026-10-20 · **P39 网格/标尺** · `mw-ui::snap` 新增 `Snap::has_offset` 与 `snap_to_grid`：左/上角分别吸到最近 `step` 倍数，单轴超阈值就不吸；空网格返回空。画布默认关网格（40 世界单位）与标尺，开启后画屏幕空间细线（每 5 格加深）、顶部/左侧 18px 刻度条；移动拖拽时参考线优先、无参考线的轴才用网格吸附；命令面板加「显示/隐藏网格/标尺」。
  证据：`grid_snaps_left_and_top_independently`（-3/-1、远距离不吸、空）；全量 **571 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、shell golden `--tol 0` **0 差异**；release 重建 **21,771,264 B**。**未做**：标尺数字标签、网格密度/颜色设置、网格吸附的独立开关（跟随网格显隐）、开关持久化。**未重拍 golden**（默认关）。
- 2026-10-20 · **P38 小地图 2.0** · 小地图在原有节点抽样、根节点/选中环、视口框、点击/拖拽平移之上补滚轮缩放：`minimap_zoom_factor(scroll) = exp(scroll * 0.0018)`，向上放大、向下缩小，缩放时把相机中心对到指针所在图的位置；钳位沿用 MIN/MAX_ZOOM，缩放/平移后 `request_repaint`。坐标往返沿用 `minimap_maps_bounds_to_the_card_and_round_trips_the_pointer`。
  证据：`minimap_wheel_zooms_in_and_out`（0 倍率=1、正滚>1、负滚<1、对称积≈1）+ 既有往返 <1px 测试；全量 **570 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、shell golden `--tol 0` **0 差异**；release 重建 **21,768,192 B**。**未做**：小地图拖拽惯性、视口框边缘手柄缩放、极小窗口下的小地图折叠动画。**未重拍 golden**。
- 2026-10-20 · **P37 缩放手感** · 新增 `zoom_at_anchor(camera, at, viewport, factor)`：先记锚点世界坐标，改 zoom（夹在 0.005–4.0），再把相机中心补到锚点仍在指针下；滚轮/触控板缩放改走它。`zoom_by`（画布中心）、Home/`fit_bounds`、聚焦路径保持原样；文档缩放下限/上限不变。
  证据：`zoom_anchor_keeps_the_world_point_under_the_cursor`（1.3→2.6 倍、锚点漂移 <1e-3 px；再放大到上限仍不漂）；全量 **569 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、shell golden `--tol 0` **0 差异**；release 重建 **21,764,096 B**。**未做**：Home 键的独立探针（沿用原路径）、演示模式的连续缩放/平移专测、滚轮惯性。**未重拍 golden**。
- 2026-10-20 · **P36 折叠/展开过渡** · `blend_scenes` 已能把消失节点滑向父中心并淡出；本轮补 `motion_span`：把「消失节点 → 目标父节点」的位移也计入差异尺度，大子树折叠不再一律最短 0.20s，而是随距离到 0.34s。过渡结束仍返回目标场景精确值。
  证据：`blend_collapse_slides_children_into_parent_and_edges_follow`（中间帧 alpha 0.5、中心在旧位置与父中心之间、入边端点落在节点内、结束帧无残留）；全量 **568 测试 / 0 失败**、fmt 干净、clippy 38 处（位置集合无新增）、shell golden `--tol 0` **0 差异**；release 重建 **21,764,096 B**。**未做**：折叠中间帧与其它节点的像素级不重叠探针、连线淡出曲线、批量折叠的时长上限。**未重拍 golden**。
- 2026-10-20 · **P35 拖拽手感** · 新增 `edge_scroll_speed`/`Canvas::auto_scroll`：拖拽时指针进入画布边缘 56px，按接近程度线性加速，最大 900px/s，按 `stable_dt` 推进相机中心；只在非平移手势生效，滚动中主动 `request_repaint`。Esc 取消手势时补清 `lasso`，避免套索路径留在屏幕上。落点/吸附/幽灵沿用原有实现。
  证据：`auto_scroll_only_near_edges_and_scales`（中间 0、左右 ±900、半程有符号）；全量 **567 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、shell golden `--tol 0` **0 差异**；release 重建 **21,761,024 B**。**未做**：真实指针输入探针、自动滚动随缩放调整速度、拖拽中自动滚动与吸附的交互专测。**未重拍 golden**。
- 2026-10-20 · **P34 多选/套索** · `Selection::click_with`：Alt 只减选，点未选项是空操作。框选松手时用 `apply_marquee` 合并：默认替换、Shift/Ctrl 并入、Alt 只减。新增 `DragKind::Lasso`：Alt+拖空白进入自由套索，路径按 2px 采样并绘制闭合线；`point_in_polygon` 射线法判定节点屏幕中心，结果与框选共用加减选。普通方向键仍是选择导航（P02 已有）。
  证据：`alt_click_only_subtracts`、`marquee_merge_honours_shift_and_alt`、`lasso_polygon_contains_points`；全量 **566 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、shell golden `--tol 0` **0 差异**；release 重建 **21,758,464 B**。**未做**：套索路径平滑/自交处理、套索与框选的像素探针、方向键直接移动节点坐标（目前方向键只移动选择，节点移动靠拖拽与 Alt+↑/↓ 层级移动）。**未重拍 golden**。
- 2026-10-20 · **P33 工具条碰撞避让** · `hover_bar_rects` 的边角语义补成参数化测试：左上/右上/左下/右下四角分别断言横向翻转（右/左）、纵向翻转（上/下）、始终在画布内、不压节点盒、不压折叠点；有卡片角标时再断言与角标不相交。算法本身沿用 P31 的「右侧优先、放不下翻左；上方优先、放不下翻下；有角标再抬高」。
  证据：`hover_bar_flips_at_four_corners_and_clears_fold_and_badge`（四角 × 有/无角标）；全量 **563 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、golden `--tol 0` **0 差异**；release 未变（本轮只加测试，仍是 **21,745,152 B**）。**未做**：画布窄到左右都放不下时的第三种落点（当前直接不画）、工具条与注解跨度手柄的碰撞、像素级命中探针。**未重拍 golden**。
- 2026-10-20 · **P32 状态过渡** · `mw_render::Frame` 新增 `hover_alpha/select_alpha`，选中/悬停环按 alpha 淡入淡出；画布每帧用 `stable_dt` 推进（悬停 120ms、选中 180ms），`reduce_motion` 直接落位；动画未完成时 `request_repaint`。Frame 的所有构造点补默认 1.0；编辑态仍由 egui 编辑器自己画边框（不参与环动画）。
  证据：`state_alpha_moves_at_120_and_180ms_and_snaps_when_reduced`（半程/整程/反向/reduce）；全量 **562 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、shell golden `--tol 0` **0 差异**（harness 走 reduce_motion）；release 重建 **21,745,152 B**。**未做**：编辑态边框过渡、悬停工具条/角标淡入、四种状态的统一色板常量、像素代理测试。**未重拍 golden**。
- 2026-10-20 · **P31 画布 Z 序** · 新增 `canvas::zorder`：`Plane` 层级表（Lines<Nodes<Annotations<Badges<Anchors<HoverBar<Menus）与 `badge_visible`；角标被小地图/面包屑矩形压住时不再绘制/收命中。`hover_bar_rects` 增加 `has_card`：有角标时工具条整体抬高一个按钮高度，角标与工具条不再互相遮挡；`paint_card_badges` 用 debug 断言钉住 HoverBar > Badges。
  证据：`zorder` 2 条单测（层级严格递增、浮层/画布外隐藏）、`hover_bar_lifts_above_the_card_badge` 几何测试；全量 **561 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、shell golden `--tol 0` **0 差异**；release 重建 **21,743,104 B**。**未做**：菜单/浮层的像素级 Z 序探针、注解与节点重叠时的命中优先级重排、动画层（P32）。**未重拍 golden**（默认没有卡片/工具条，已复核 0 差异）。
- 2026-10-20 · **P30 检查器视觉（第一轮）** · 分组标题从 12.5px 灰字改为「3px 强调色竖条 + 13.5px 标题 + 10.5px 右对齐说明」，关键分组补说明（文档属性/主题/强调色/对比度/配色微调）；新增 `GRID = 4px` 并用于标题行内边距；标题、说明、数值三档字阶固定为 LABEL/CAPTION/BODY·SMALL。检查器整体密度、图标与留白只做了这一轮结构统一，未逐控件重排。
  证据：全量 **558 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、shell golden `--tol 0` **0 差异**（默认检查器关闭，未覆盖打开后的像素）；release 重建 **21,744,128 B**。**未做**：检查器打开态的 golden 场景、逐控件 4px 重排、分组图标（当前用色条代替）、暗色/高 DPI 视觉复核。**未重拍 golden**。
- 2026-10-20 · **P29 检查器键盘导航** · `FocusMemory::step_focus`：按本帧 `note` 登记顺序环形移动，无焦点时向后落第一个、向前落最后一个；`MW_TRACE` 记 `focus-step <语义名>`。检查器在自定义控件聚焦且无文本编辑时接 ↑/↓（移动焦点）与 Esc（`Action::FocusCanvas` 回画布）。`seg`/`toggle` 补 1.4px 强调色焦点环与 Enter/Space 激活；Tab 仍走 egui 自带顺序。
  证据：`arrow_navigation_moves_through_known_keys_and_wraps`（后/前/首尾环绕）、`step_focus_without_a_current_item_lands_on_an_end`；全量 **558 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、shell golden `--tol 0` **0 差异**；release 重建 **21,741,568 B**。**未做**：色点/图标格等其它自定义控件的焦点环与 Enter（目前只有 seg/toggle）、方向键在组合框/滑杆里的让路专测、`MW_TRACE` 真实键盘探针。**未重拍 golden**（默认检查器关闭，已复核 0 差异）。
- 2026-10-20 · **P28 浮层规则** · 新增 `overlay` 纯状态机：`Layer` 优先级（右键菜单 < 命令面板 < 搜索 < 导出 < 帮助 < 快捷键 < 标签管理 < 库路径 < 笔记库 < 阅读页 < 导入提示）、`top/blocks_canvas/open_count`。壳层每帧算 `overlay_state`，画布锁输入统一走它；打开阅读页/笔记库前 `close_light_overlays`，保证同一时刻一个抢指针；标签管理、库路径对话框、导入提示补 Esc（若阅读页/库页在上层则不抢）。`MW_TRACE` 下多于一个浮层会记 `overlay-double`。
  证据：`overlay` 4 条单测（空/优先级/open_count/每层可达）；全量 **556 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、shell golden `--tol 0` **0 差异**；release 重建 **21,734,400 B**。**未做**：Esc 完全集中到状态机（仍由各浮层消费）、浮层打开动画互斥的像素探针、导入进度 Esc 取消。**未重拍 golden**（默认无浮层，已复核 0 差异）。
- 2026-10-20 · **P27 关系线面板** · `RelationshipSel` 字段公开，新增 `Canvas::relationship_selection/select_relationship/clear_relationship_selection`；检查器新增关系线页：起点→终点小按钮（选中节点）、标签输入（Enter/失焦提交 `SetRelationshipLabel`）、删除、取消选择、全部关系线列表（点击 `SelectRelationship` 切换）。`select_node` 现在会清关系线/注解选择，保证面板只显示一种选中态；`select_relationship` 清节点/注解。
  证据：既有关系线标签/端点命令测试 + 全量 **552 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、shell golden `--tol 0` **0 差异**；release 重建 **21,729,792 B**。**未做**：关系线颜色/线型（模型无槽位）、面板内直接拖端点（仍走画布手柄）、跨父节点的关系线分组。**未重拍 golden**（默认无关系线选中，已复核 0 差异）。
- 2026-10-20 · **P26 注解面板（批量）** · `Extras` 增加全部选中注解切片，`Canvas::annotation_selections()` 暴露列表；注解面板标题显示「已选 N 条」，多选提示「文字/跨度只改主选，颜色/删除作用于全部」。颜色按全部边界生成 `SetBoundaryColor`、删除按全部概要/边界生成 `Delete*`，统一 `wrap` 成一条 Batch。跨度手柄与面板仍共享同一份 `AnnotationSel` 与文档，改完下一帧手柄自动跟新。
  证据：既有 `annotation_click` 多选测试 + 全量 **552 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、shell golden `--tol 0` **0 差异**；release 重建 **21,716,992 B**。**未做**：批量改概要文字、跨父节点的注解批量颜色、面板内直接拖跨度（仍走画布手柄）。**未重拍 golden**（默认无注解选中，已复核 0 差异）。
- 2026-10-20 · **P25 布局与连线面板** · `LayoutSettings` 新增 `direction`（auto/right/left）：思维导图右侧全排、左侧全排或双侧自动平衡；其它布局忽略。`ThemeOverrides` 新增 `edge_taper`（跟随/收细/等粗），布局面板加「方向」「线型」分段，线宽/线色仍走「配色微调」。方向与线型都经 `SetLayout`/`SetThemeOverrides`，可撤销；重排后选择按 id 搬运、相机不重置。
  证据：`mindmap_direction_can_be_forced_right_or_left`（右全在根右、左全在根左）、`edge_taper_override_can_flatten_branch_lines`、`layout_direction_is_saved_and_undoable`；全量 **552 测试 / 0 失败**、fmt 干净、clippy 38 处（位置集合无新增）、shell golden `--tol 0` **0 差异**；release 重建 **21,682,176 B**。**未做**：曲线/折线/直线三种连线形态（当前只做收细/等粗）、方向对非思维导图布局的语义、布局切换动画的相机保持测试。**未重拍 golden**（默认 auto/主题线型，已复核 0 差异）。
- 2026-10-20 · **前置 · 导入保真（P60 的一部分，提前做）** · OPML/FreeMind 导出补 `_link`/`_labels`/`_collapsed`（FreeMind `LABELS`），导入端解析并归一化；`ImportReport` 新增 `ImportCounts`（节点/备注/链接/标签/关系线/折叠/样式），XMind 报告按文档统计，其它格式在 `Session::import_with_report` 统一统计。
  证据：`opml_and_freemind_roundtrip_keep_notes_links_labels_and_collapse`（备注/链接/标签/折叠全保真）；`import_report_counts_embedded_resources` 扩节点数；全量 **549 测试 / 0 失败**、fmt 干净、clippy 37 处（位置集合无新增）、shell golden `--tol 0` **0 差异**；release **21,695,488 B**。**未做**：XMind 样式/图片迁移（P31/P53）、XMind 导出器、OPML head 元数据往返。**未重拍 golden**（导入层不影响默认画面）。
- 2026-10-20 · **P24 主题与配色面板** · `Rgba` 新增 WCAG `relative_luminance/contrast_ratio`（sRGB 线性化，1–21）。检查器强调色加「作用域」：整图走 `SetThemeAccent`；选中走 `SetStyle.branch_color`（可连同子树，仍是一条 `Batch`）。新增「对比度」审计：正文/面板、强调/面板、一级主题文字/填充，按 AA 4.5 / AAA 7.0 标注；主题网格 13 套预览保留。
  证据：`contrast_ratios_match_wcag`（黑白 21、同色 1、顺序无关）；全量 **548 测试 / 0 失败**、fmt 干净、clippy 38 处（位置集合无新增）、shell golden `--tol 0` **0 差异**；release 重建 **21,691,392 B**。**未做**：按节点覆盖整图主题（只做强调色作用域）、主题卡片大图预览、色盲模拟、对比度自动修复。**未重拍 golden**（默认检查器关闭，已复核 0 差异）。
- 2026-10-20 · **P23 内容面板** · `Node` 新增 `task: Option<TaskState>`（todo/doing/done，serde 默认兼容旧文件）与可撤销 `Command::SetTask`；内容页（原「备注」页）在备注/卡片之间加任务分段按钮与清除。链接校验：允许 http/https/file/mailto、本地路径/相对路径；拒绝控制字符、空 scheme/空 host、`javascript:`/`data:` 等伪协议。备注/标签/图标沿用原有长度与数量上限。
  证据：`task_state_is_undoable_and_link_is_validated`（任务撤销、https 通过、`javascript:`/空 https 拒绝、Windows 路径通过）；全量 **547 测试 / 0 失败**、fmt 干净、clippy 38 处（位置集合无新增）、shell golden `--tol 0` **0 差异**；release 重建 **21,673,984 B**。**未做**：任务状态在画布上的角标/删除线（P31+ 视觉）、链接真实可达性探测、备注/链接的实时字数计数。**未重拍 golden**（默认检查器关闭，已复核 0 差异）。
- 2026-10-20 · **P22 样式面板（层级）** · 样式区顶部加「应用到子树」：`ItemView.subtree` 在 `sync` 时缓存自己 + 全部后代及各自当前 `StyleOverride`；`style_cmd` 按子树展开、按 id 去重，每条 `SetStyle` 包进同一个 `Batch`，撤销仍是一步。颜色/分支线/文字/形状/字号/粗斜体沿用现有即时预览控件。
  证据：`subtree_styles_include_self_and_descendants_once`（先序、自己在前、不重复）+ 既有 `style_edits_merge_into_each_nodes_own_override`；全量 **546 测试 / 0 失败**、fmt 干净、clippy 38 处（位置集合无新增）、shell golden `--tol 0` **0 差异**；release 重建 **21,663,744 B**。**未做**：字体家族（应用只带一套字体）、文本对齐（`ResolvedStyle` 尚无 align 槽位）、层级样式的"按深度套用不同样式"。**未重拍 golden**（默认检查器关闭，已复核 0 差异）。
- 2026-10-20 · **P21 检查器架构** · 右栏改为 `egui::Panel::right` 可拖拽宽度：默认 260，范围 220–520，且 `available-360` 封顶，窄窗优先保画布；动画期仍走 `exact_size`，稳定后 `default_size` + 原生拖拽。宽度写入 `window.json`（旧存档 default 260，读写都 clamp），拖动中只改内存、松手及关窗落盘。`WindowState` 新增 `inspector_width/set_inspector_width` 与往返/夹取测试。布局区改为可折叠 `CollapsingHeader`；样式/备注/链接/标记仍用 tab 分区，各自独立滚动。面板占列而不是浮层，画布交互区不会被盖住。
  证据：`inspector_width_round_trips_and_clamps`（333 往返、9999→520、10→220）；shell golden `--tol 0` **0 差异**；全量 **545 测试 / 0 失败**、fmt 干净、clippy 38 处（位置集合无新增）。**未做**：全部区块都可折叠（目前只有布局区）、面板宽度双击复位、宽度随 DPI 跨屏换算。release 已重建 **21,588,992 B**。**未重拍 golden**（默认检查器关闭，0 差异已复核）。
- 2026-10-20 · **P20 卡片文件夹导出/导入** · `mw-format` 新增 `export_cards_dir`（每张卡 `<安全 id>.md`，复用 `card_markdown`）与 `import_cards_dir`（只读 `.md`，坏文件记入 `skipped`，按 id 稳定排序）。笔记库顶栏加「导出…/导入…」：导出选中或全部，路径对话框默认桌面「织思卡片」；导入逐张 `upsert`，同 id 覆盖，完成后刷新并提示跳过数。
  证据：`card_folders_export_import_byte_exact_and_keep_tags`（逐字段往返 + 二次导出逐字节一致）；`card_folder_import_skips_bad_files`；全量 **544 测试 / 0 失败**、fmt 干净、clippy 38 处（位置集合无新增）。**未做**：系统文件夹选择器（路径手填）、导出进度/取消（卡片量一般小）、导入冲突的"保留两份"策略（当前同 id 覆盖）。**未重拍 golden**（对话框是模态）。**未重建 release**。
- 2026-10-20 · **P19 卡片全文搜索** · `NoteStore` 增加正文缓存 `texts`（`title\nbody`，懒加载）与复用的 `mw_core::search::SearchIndex`；`search(query, limit)` 把卡片转成临时节点跑原文/拼音/首字母检索，返回 `CardHit { before, matched, after, score, kind }`，片段按字符区间截取。笔记库搜索改为全文命中：输入变化才重搜，结果按分数排序，命中词用强调色 + 底色高亮，并标注「拼音命中/拼音首字母命中」；标签/收件箱过滤仍生效。
  证据：`full_text_search_finds_body_snippet_and_pinyin`（正文命中、片段前后文、`shen`/`sd` 拼音命中、空查询）；`search_thousand_cards_is_bounded`（1000 张、debug 构建 <100ms）；全量 **542 测试 / 0 失败**、fmt 干净、clippy 38 处（位置集合无新增）。**未做**：中文分词（当前子串，已够卡片场景）、搜索结果分页（上限 500/显示 50）、索引增量更新（正文变化清缓存靠 query 失效）。**未重拍 golden**（库页是模态）。**未重建 release**。
- 2026-10-20 · **P18 快速捕获** · 节点右键新增「新建卡片笔记」，命令面板新增「为选中主题新建卡片」「今日笔记」。`Canvas::capture_card_for_primary` 已有卡片则返回原卡，否则 `AttachNewCard` 新建；壳层打开阅读页，空正文直接进编辑。
  `NoteStore::daily_note(date, now)`：标题=日期、标签 `每日`，同一天重复调用返回同一 id。阅读页保存新增库内独立卡片路径：文档内卡片仍走 `SetCardTitle/Body/Tags` 可撤销命令，库内卡片（每日笔记）直接 `upsert`，因此每日笔记可编辑。笔记库新增「收件箱」开关，只看 `source_node` 为空的卡片。
  证据：`daily_note_is_idempotent_per_date`（同日同 id、异日异 id、标签正确）；全量 **540 测试 / 0 失败**、fmt 干净、clippy 38 处（位置集合无新增）。**未做**：收件箱的纯函数单测（过滤在 UI 里）、每日笔记模板/日期跳转、快捷键（只有右键与命令面板）、库内卡片保存的撤销（直接写库，无命令快照）。**未重拍 golden**（捕获入口是菜单/阅读页）。**未重建 release**。
- 2026-10-20 · **P17 节点卡片联动** · `Canvas` 新增 `paint_card_badges`：遍历 `scene.ids`，只对 `node.card_id.is_some()` 的节点画角标；角标中心在节点屏幕右上角外侧 `(radius+3, -(radius+3))`，半径 6.5px（zoom<0.55 时 3.5px，zoom<0.28 不画），视口外跳过。悬浮 tooltip 显示卡片标题/80 字摘要/「点击打开卡片」；点击写 `card_open`，壳层 `take_card_open` 后走 `card_reader_for` 打开 P12 阅读页。角标是屏幕空间绘制，不进合成纹理，离屏导出不会带上。
  证据：`card_badge_is_outside_the_node_and_scales_with_radius` 几何单测；全量 **539 测试 / 0 失败**、fmt 干净、clippy 38 处（位置集合无新增）。**未做**：角标命中与画布节点点击的优先级探针（点击角标可能同时选中节点）、20k 节点角标批量性能探针、角标样式随主题自定义。**未重拍 golden**（样例文档没有卡片，静态基线不触发角标；未跑 release 视觉验证）。**未重建 release**。
- 2026-10-20 · **P16 双向链接** · `mw-core` 新增 `Backlink { node_id, text }` 与 `Document::backlinks_for_card()`（扫 `Node.card_id`，按文本稳定排序；节点移动不影响）。阅读页新增 `SourceLink { node_id, text, missing, doc_label }`：壳层打开卡片时查当前文档，给出「当前文档/另一份文档 · 节点文本」；源节点不在时显示「源节点已删除（卡片保留）」，不提供跳转；反链列表点击回节点。节点→卡片仍走检查器卡片区「打开卡片」。
  证据：核心 `attach_card_...` 扩展反链断言（链接后 1 条、文本/节点 id 正确）；全量 **538 测试 / 0 失败**、fmt 干净、clippy 39 处（位置集合无新增）。**未做**：跨 `.zmind` 文件的源文档路径解析（只有文档 id/「另一份文档」标签，P77 恢复中心再补）、卡片间互相引用（当前是卡片↔节点，不是卡片↔卡片）、反链双向编辑。**未重拍 golden**（反链只在阅读页）。**未重建 release**。
- 2026-10-20 · **P15 笔记库页** · 新增 `note_library`：全屏库页，顶栏搜索/标签下拉/四档排序，列表显示标题、摘要、彩色标签、日期；复选批量、批量移出本地库（确认框明确"不删 .zmind 内嵌副本"）、刷新重建索引；点击标题进 P12 阅读页。命令面板加「笔记库」。过滤/排序是纯函数：搜索覆盖标题/摘要/标签及层级段，标签过滤含子标签（`知识` 命中 `知识/方法`，不命中 `项目/知识`）。
  证据：搜索/层级过滤/排序 3 条单测；全量 **538 测试 / 0 失败**、fmt 干净、clippy 38 处（位置集合无新增）。**未做**：批量导出（留给 P50 导出中心，当前没有目录选择器）、批量打标签、列表虚拟化（1000 张先全量克隆，P19 搜索性能再压）、多选跨页保持。**未重拍 golden**（库页是模态）。**未重建 release**。
- 2026-10-20 · **P14 标签系统** · 标签模型：`NoteCard::normalize_tags`（trim、大小写无关去重、限量）+ `tag_path/tag_parent/tag_root` 层级 helper。命令：`SetCardTags`、`RenameTag`（整份卡片快照、可撤销、无变化不入历史）。`NoteStore`：`all_tags()` 聚合计数、`rename_tag()` 逐卡重写 Markdown 后统一落索引（失败可 rebuild 自愈）。
  UI：阅读页编辑态标签行可加/删，输入时按库内标签补全（最多 6 个建议），颜色按路径根哈希稳定；检查器卡片区显示彩色标签；阅读页「标签」按钮打开标签管理窗，重命名同时改当前文档（撤销可回）与本地库。自动保存的 dirty 判定、撤销栈、`Save` 动作都带上标签。
  证据：核心 `card_tags_are_normalized_and_undoable`（含 RenameTag 撤销）、层级 helper 测试、`NoteStore` 全库重命名/大小写合并/空标签拒绝 2 条、标签颜色稳定性；全量 **535 测试 / 0 失败**、fmt 干净、clippy 39 处（位置集合无新增）。**未做**：跨多个 `.zmind` 文件的标签重命名（只改当前文档 + 本地库）、标签拖拽排序、自定义颜色（当前按根哈希固定）。**未重拍 golden**（标签只在阅读页/检查器卡片区）。**未重建 release**。
- 2026-10-20 · **前置 · 导入异步与丢失项报告（P60 的一部分，提前做）** · `mw-io` 新增 `ImportReport`（resources 计数 + warnings）与 `import_xmind_with_report`：扫描 `resources/`、`attachments/`，把数量写成一条人话警告；`Session::import_with_report` 统一四种格式的返回。壳层新增 `import_job`：后台线程读/解析、原子取消、100ms 轮询、进度窗（文件名/耗时/取消）；完成后开标签，有警告就弹「导入提示」列表。
  证据：XMind 资源计数单测（2 个资源 → 1 条警告）、后台导入 Markdown 成功 + 缺失文件失败 2 条线程测试；全量 **530 测试 / 0 失败**、fmt 干净、clippy 38 处（位置集合无新增）。**未做**：图片/附件真正迁移进 `.zmind`（P53 资源包）、50MB 真实文件探针、导入任务队列（一次一个）。**未动外观**。
- 2026-10-20 · **P13 卡片编辑器** · 阅读页顶栏加「编辑」：进入后标题变单行输入、正文变源码编辑；「预览/源码」切换，预览复用 P12 的 `render_blocks`，没有第二套排版。保存走两条核心命令 `SetCardTitle` / `SetCardBody`（整份卡片快照、可撤销、内容没变不入历史）。
  交互：停止输入 **800ms 自动保存**；`Ctrl+S` 立即保存；`Ctrl+Enter` 保存并退出编辑；`Ctrl+Z` 本地撤销（最多 100 步）；`Esc` 丢弃草稿回到上次保存；关闭/返回在脏状态下先保存再关。保存成功后同步 `.zmind` 内嵌卡片与本地库。
  证据：核心 `card_body_and_title_are_undoable`（含"无变化不入历史"）；`CardReader` 3 条编辑态测试（dirty/mark_saved/cancel、Save 动作携带 close/leave）；全量 **527 测试 / 0 失败**、fmt 干净、clippy 39 处（位置集合无新增）。**未做**：真实键盘/IME 探针、自动保存失败重试提示、编辑页滚动位置记忆；**未重拍 golden**（编辑页是模态）。**未重建 release**。
- 2026-10-20 · **P12 卡片阅读页** · 新增 `note_view`：全屏前景阅读页 + `mw-markdown → egui` 只读渲染器。正文栏最大 760px、窗口够宽（≥1040）右侧出现目录；标题按 26/22/18/16 分档，段落 `horizontal_wrapped` 支持中文折行，列表/引用/代码块/表格/任务/图片占位/分隔线都有渲染；链接可点并外开，Esc/返回/关闭按钮退出，`回到节点` 关闭阅读页并选中画布节点。顶栏显示字数、更新日期、标签；侧栏显示创建日期。
  渲染器 `render_blocks` 是公开给 P13 编辑器预览的同一份实现，避免阅读/预览两套排版。字数算法：CJK 按字、拉丁按词、标点空白不算；日期用 Howard Hinnant 的 days→civil 算法输出 UTC 日期。
  证据：4 条测试（中英字数、目录块下标、日期稳定性、CardReader 解析/标签/节点链接）；全量 **524 测试 / 0 失败**、fmt 干净、clippy 38 处（位置集合无新增）。**未做**：编辑器预览（P13）、真实 1000 字滚动性能探针、图片实际加载（P56）；阅读页是模态，未重拍 golden（静态基线不打开卡片）。**未重建 release**。
- 2026-10-20 · **P11 Markdown 进阶** · `mw-markdown` 的 AST 扩展：`Block::Table { headers, aligns, rows }`、`ListItem { task, blocks }`、`Span.image` + `ImageRef`。支持 `|` 表格（`:---`/`---:`/`:---:` 列对齐、单元格内粗斜/行内代码）、`- [ ]`/`- [x]` 任务列表、`![alt](url "title")` 图片；`Document` 新增 `images()`，纯文本/摘要把图片输出为 alt、任务输出 `[x]/[ ]`。
  关键修复：`push_span` 合并相邻 Span 时没比较 `image`，导致图片被并进普通文字后丢失；现在带图片的 Span 不参与合并。表格必须有分隔行才成立，否则仍是普通段落。
  证据：5 条新测试（表格对齐+行内标记、无分隔行不误判、任务勾选态、图片 alt/url/title、畸形图片原样保留）；全量 **520 测试 / 0 失败**、fmt 干净、clippy 38 处（位置集合无新增）。**未做**：脚注（按需暂不做）、egui 实际渲染（P12）。**未动外观**。
- 2026-10-20 · **P10 Markdown 解析核心** · 新增独立 crate `mw-markdown`（纯 CPU，无 GUI 依赖）：`Block::{Heading, Paragraph, Quote, List, Code, Rule}` + 扁平 `Span`（粗/斜/行内代码/链接）。支持 ATX 标题、段落软换行、`**粗**`/`*斜*`/`_斜_`/`` `代码` ``、`[文字](url)`、转义、有序/无序/嵌套列表、引用（递归块）、围栏代码（保留缩进与语言）、分隔线、CRLF、未闭合标记原样保留。
  `Document` 提供 `plain_text` / `excerpt` / `links`，阅读页、库预览、导出都从这棵树取文字，避免四套排版。10 条测试覆盖上述语法 + 中文/转义/空文档/畸形标记；shell 已加依赖，P12/P13 直接接渲染。
  证据：全量 **515 测试 / 0 失败**、fmt 干净、clippy 39 处（位置集合无新增）。**未做**：表格/任务列表/图片/脚注（P11）、egui 实际排版（P12）。**未动外观**。
- 2026-10-20 · **P09 节点↔卡片链接** · `Node` 新增 `cardId`；`Command::SetNodeCard { node, card }` 负责挂/解，`AttachNewCard { node, card }` 一条命令完成"新建卡片 + 挂到节点"。校验：卡片必须存在、一个节点最多一张、一张卡不能被两个节点挂；解除链接时清 `sourceNode` 但**不删卡片**。删节点后卡片保留为孤儿，`Document::validate` 不把孤儿当损坏。
  `ChangeSet` 增加整份卡片快照（`before_cards/after_cards`），撤销/重做能同时回退节点链接与卡片来源字段；Batch 合并也带卡片。`Document::from_parts_with_cards` 让"先有卡片、后有 node.cardId 校验"的顺序成立，格式层与恢复点都改走它。
  检查器「备注」页新增「卡片笔记」区：已链接显示标题/摘要 + 解除；未链接显示「新建卡片笔记」。新建后同步进本地卡片库。**未做**：打开/编辑卡片页（P12/P13）；从已有库卡片挑一张链接（P15）。**未重拍 golden**：卡片区只在备注页出现，默认样式页不渲染，但未跑 release 视觉验证。
  证据：核心 2 条链接/撤销/孤儿测试、格式层 `node.cardId` 往返、全量 **505 测试 / 0 失败**、fmt 干净、clippy 39 处（位置集合无新增）。
- 2026-10-20 · **P08 本地卡片库** · 新模块 `note_store`：`%APPDATA%\织思\notes` 下每张卡一个 `<id>.md`，另有 `index.json`（只存摘要，不含正文）。索引坏了/版本不符就扫盘重建；单个坏 Markdown 只计数不拖垮全库。写文件走「tmp → 旧文件备份 → 更名」，索引同路径。`sync_document` 只在文档那份 `updatedAt` 更新时覆盖库，避免旧 `.zmind` 反杀新笔记。`paths.rs` 统一 appdata 根，自动保存也改用它。
  `mw-format` 把卡片 Markdown 编解码提成公共 API：`card_markdown` / `card_from_markdown`（JSON 字符串即合法 YAML 双引号，省掉自写转义）；`.zmind` 内写 `notes/<id>.md` 与库共用同一份编码。
  证据：`NoteCard` 编解码往返（含引号/换行/标签）、库读写/索引、坏文件重建、旧文档不覆盖新库、**1000 张卡片重建**；全量 **503 测试 / 0 失败**、fmt 干净、clippy 39 处（位置集合无新增）。**未做**：库的 UI（列表/搜索/标签过滤在 P15/P19）、索引与磁盘外部改动的增量刷新（目前靠启动加载/重建）。**未动外观**。
- 2026-10-20 · **P07 卡片笔记数据模型与内嵌存储** · 新模块 `mw-core::note::NoteCard`：id/title/body(Markdown)/tags/sourceDoc/sourceNode/createdAt/updatedAt；标签归一化（trim、大小写无关去重、限量）、正文/标题/标签上限、纯文本摘要。`Document` 新增 `cards: Vec<NoteCard>`，校验重复 id 与「一个节点最多一张卡片」，孤儿卡允许存在；读接口 `card` / `card_for_node` / `cards` / `set_cards`。
  `.zmind` 现在除了 `content.json` 还写 `notes/<id>.md`（YAML front matter + 正文），解压即可阅读；加载时以 `content.json` 为准并校验卡片。恢复点 `Snapshot` 也带上 cards，崩溃恢复不会丢笔记。ZIP 条目上限从 1024 提到 20,000（1000 张卡是常规量级，总体积仍有 256MB 守卫）。
  证据：4 条 `NoteCard` 单测（标签/标题/摘要/上限）、Document 卡片校验、`mw-format` 卡片往返 + ZIP 内 Markdown 内容断言；全量 **498 测试 / 0 失败**、fmt 干净、clippy 39 处（位置集合无新增）。**未做**：全局笔记库/索引/UI（P08+）；`notes/*.md` 目前是随 `.zmind` 写出的可读副本，加载仍以 `content.json` 为准，P08 再决定是否让文件成为主源。**未动外观**。
- 2026-10-20 · **P06 每标签自动保存** · `autosave::Background` 从「单个 `pending` + 单个 `Outcome`」改成**按槽位去重的队列**：不同标签各排各的，同一槽位只留最新一版；`poll()` 一次可能收回多条结果并带上 `slot`，壳层用 `HashMap<PathBuf,u64>` 分别记每份文档的已保存修订号。未命名标签的恢复点从共享的 `未命名.autosave.zmind` 改成 `未命名-<docId>.autosave.zmind`，否则多标签会互相覆盖。`note_session_rev` 不再 bump 全局 epoch（那会把别的标签在途的写判成过期），只更新当前槽位基线。
  证据：新增「两个不同 path 的会话同时排队、各自落盘、同槽位只留最新版」单测；全量 **492 测试 / 0 失败**、fmt 干净、clippy 38 处（新增 0）。**未做**：启动时"恢复中心"列表（目前仍只对单个路径弹一次恢复提示）——已拆到 P63；自动保存没有真实崩溃探针。**未动外观**。
- 2026-10-20 · **路线图扩展到 400h / 80 包** · 按产品新定位重写 §7：开源、本地、不做 i18n/付费/升级/云同步；重心改为**检查器 2.0、卡片笔记系统、画布不遮挡、视觉质感、知识内化**。80 包 × 5h = 400h，P01–P06 已完成，P07 起进入卡片笔记。
- 2026-10-20 · **P05 自由主题边界条件** · 双击空白/右键「新建自由主题」现在和 Tab/Enter 一致：创建后立刻选中新主题并打开行内编辑器（以前只建出一个写着「自由主题」的盒子，用户还得再双击一次）。实现用 `doc_order` 的前后差集找新 id，再 `Selection::one` + `start_edit`；拖到空白落盘、拖进主树改父、拖出成浮动继续走既有 `detach_commands` / `plan_moves`。
  证据：全量 **491 测试 / 0 失败**、fmt 干净、clippy 38 处（新增 0）。**未做**：自由主题的 z 序/置顶置底（模型没有自由主题顺序字段，需要文档级顺序，单独立项）；创建即编辑是窗口态，未写自动化 UI 测试。**未动外观**。
- 2026-10-20 · **P04 自由主题对齐/等距分布** · 新模块 `mw-ui::arrange`：`align(rects, 六向)` 按选区包围盒对齐，`distribute(rects, 水平/垂直)` 保留首尾、均分盒间空隙（允许负空隙），空/1条/2条都安全返回原坐标。壳层只在选中 **≥2 条自由主题** 时把 6+2 行加进多选右键菜单；执行时把 `new - 当前矩形` 的增量加到节点存储的 `pos` 上，包成一条 `Batch`（一次撤销）。分布需要 ≥3 条，少于 3 条菜单项置灰。
  **架构诚实标注**：树内节点的位置由布局引擎拥有，`SetNodePos` 只接受自由主题；所以 P04 没有做"树内多选对齐"——硬做需要给每个节点加手动偏移槽，属于布局模型变更，已从本包移出并记录。吸附/参考线保持原有行为，本包未改。
  证据：`mw-ui` 4 条几何单测（六向落边、等距首尾不动、空/短列表）、全量 **491 测试 / 0 失败**、fmt 干净、clippy 38 处（新增 0）。**未动外观**，golden 不重拍；release 未重建。
- 2026-10-20 · **P03 关系线编辑** · 画布可以选中关系线（点击曲线/箭头/标签，不再只是选中起点节点）：选中后终点出现手柄，拖到另一个节点松手发一条 `SetRelationshipTarget`；双击或 `F2` 打开标签内联编辑（Enter 提交、Esc 取消、空串=清除），右键菜单给「编辑标签 / 删除关系线」，`Delete` 也先删关系线。
  核心新增 `Command::SetRelationshipTarget { from, id, to }`：拒绝自环、未知目标、同一 from 的重复目标；快照 from 节点，撤销可回原目标。绘制上选中态画起点小点 + 终点手柄，拖动时画橡皮筋并高亮目标节点。
  证据：全量 **487 测试 / 0 失败**（core +1 换目标/撤销/拒绝非法目标）、fmt 干净、clippy 38 处（位置集合无新增）。**未做**：关系线单独改色（模型没有 color 槽，主题统一用 `ui.dim`）；拖起点换 from（语义上是另一条关系线，未做）；没有真实鼠标探针。**未动外观**。
- 2026-10-19 · **P02 注解多选与批量删除** · 画布注解选择从单条改成有序多选（末位主选）：Shift/Ctrl 点击切换进/出，普通点击收成单选；选中环逐条画，右键菜单在多选时只给「删除选中的 N 条注解」；删除走一条 `Batch`，撤销一次全部回来；边界换色也支持一次改多条。
  实现：`annotation_click` 抽成纯函数钉住三种语义；`delete_annotation` / `set_boundary_color` 统一构造 `Batch`；右键菜单按 `annotation_count()` 分流。
  证据：全量 **486 测试 / 0 失败**（core +1 批量删除撤销、shell +1 多选语义）、fmt 干净、clippy 39 处（位置集合与改动前一致，新增 0）。**未做**：概要主题盒整体拖动（模型无偏移槽，硬做会改语义；留给 P05 与自由主题移动一起设计）；多选注解没有真实鼠标探针。**未动外观**。
- 2026-10-19 · **P01 注解交互收尾** · 在 WP9 的跨度手柄之上补齐三件事：① 右键点中概要/边界时不再落到「节点/空白」菜单，而是选中注解并弹注解菜单（概要：编辑文字 / 删除；边界：删除 / 默认/红/蓝/绿/橙 5 个颜色）；② 概要文字**画布内联编辑**：双击摘要盒或菜单「编辑概要文字」打开，Enter/Ctrl+Enter 提交、Shift+Enter 换行、Esc 取消、失焦提交，文字没变不发命令；③ `Delete` 键沿用原有「先删注解再删节点」的路径。
  实现上新增 `SummaryEdit` 状态与 `summary_editor_ui`，与节点编辑器互斥；编辑期间画布手势/快捷键/点击全部让路，避免「点一下输入框把选择清空」。边界色直接发 `SetBoundaryColor`，走既有撤销栈。
  证据：全量 **484 测试 / 0 失败**、fmt 干净、clippy 38 处（新增 0）。**诚实标注**：内联编辑是 GPU/窗口态，未写自动化 UI 测试（与节点编辑器同一限制）；菜单与命令路径没有独立单测，只有编译与全量回归。**未动外观**，golden 不重拍。
- 2026-10-19 · **WP9 注解跨度拖拽** · 选中概要或边界后，花括号两端/边界上下缘出现两个屏幕空间手柄；按下手柄进入 `DragKind::Span`，拖动时按「指针 y → 同侧最近兄弟」实时预览将被圈住的节点，松手只发**一条** `SetSummaryTargets` / `SetBoundaryTargets`，撤销一次全回去。
  关键约束：`span_bounds` 把 targets 归一成 children 的闭区间（文件里 targets 顺序不可信）；`span_range` 保证区间永远非空（上端拖过终点夹在终点、下端拖过起点夹在起点）；放射式布局只统计与当前注解同侧的兄弟，避免拖右侧概要把左侧分支圈进来；`Esc`/浮层/换文档都会清掉 `span_drag`，按下即 `swallow_click`，不会一次没挪动的点击把注解取消。
  证据：3 条纯逻辑单测（乱序 targets 归一、区间不空、最近兄弟）、全量 **484 测试 / 0 失败**、fmt 干净、clippy 38 处（新增 0）。**未做**：拖动手柄没有真实鼠标探针（静态 golden 不出现手柄），也没有把概要主题盒本身做成可拖动物体——那是 WP10。
- 2026-10-19 · **WP8 收尾** · 全量门槛复跑：`cargo test --workspace` **481 项 0 失败**、`cargo fmt --all -- --check` 干净、`cargo clippy --workspace --all-targets` **38 处存量 / 新增 0**（顺手删掉旧「＋」留下的 `Align2`、`LayoutKind` 未用导入，比上一轮少 2 条）、三份 golden `--tol 0` 各连跑两次 0 差异。release 从 20,763,136 B 增到 **20,772,864 B**（+9,728 B：四套主题令牌 + 悬停工具条绘制 + 折叠点第二个圆实例）。
  三份基线重拍：shell 191,606 → **192,193 B**、canvas 27,594 → **27,599 B**、canvas-text 30,822 → **30,822 B**（重写但字节数恰好相同）。画布漂移实测 **93/540,000 px（0.017%）· 最大差 146**，包围盒 `x[171..190] y[290..309]`——正好是一处折叠点的 20×20 盒子，没有别的区域变色。
  **诚实标注**：壳层基线在重拍前没有留旧副本，所以只能记字节变化，不能像画布那样给出像素漂移百分比；壳层改动与画布同源（折叠点圆环），悬停工具条不出现在静态基线里（基线没有指针、没有悬停态）。
- 2026-10-19 · **WP7 主题库与护眼** · 主题从 9 套扩到 **13 套**：从本机 XMind 26.5.1107 的 `resources/app.asar` 里逐项抄出 `M02 Classic`、`M05 Colorful`、`M12 Robust` 三份官方 JSON（不是凭印象配色），再加一套按 §5 指定值新调的深色「深潭」（画布 `#15171A`、卡片 `#1D2024`、正文 `#C9CDD3`、强调 `#4FA98F`）。
  M02：蓝中心卡 `#0288D1`、一级「透明底 + 2px 墨灰描边」、二级起下划线、线全为 `#333333` 且**等粗**（官方不写 `line-tapered`）。M05：白中心卡 + `#8793A5` 2.5px 描边、白色一级卡、六支官方 `multi-line-colors`。M12：紫墨画布 `#302934`、浅绿中心 `#D1F786`、白字下划线一级、青色 `#30D8C4` 分支线。
  `THEME_IDS`、`themes()`、`get_theme`、命令面板四张表同步（新增「主题：古雅 / 缤纷 / 黛紫 / 深潭」），`contrast.rs` 自动遍历 13 套 × 4 层的文字与分支线对比度；新增两条守卫：13 套 id 都能被 `get_theme` 解析且与 `themes()` 顺序一致、深潭四个指定令牌逐项相等。深色像素代理：深潭画布主色 **92.5%**、纯白 **0.00%**；黛紫 **97.0%** / **0.16%**。
- 2026-10-19 · **WP6-2 折叠点圆环** · `push_toggle` 从一个「填充 + 描边」实例拆成**两个实例**：先画外扩 `TOGGLE_HALO = 1.5px` 的画布色底圆（把穿过的连线挖掉），再画 **无填充**、1.5px 分支色描边的环；`+ / −` 横竖线照旧。这样环与连线之间始终有 1.5px 净空，不会在缩小时糊成「实心点粘在线上」。
  `TOGGLE_R` 与 `layout::TOGGLE_OFFSET` 不动，命中半径仍为 `TOGGLE_HIT_CSS = 10`；新增编译期断言 `TOGGLE_HIT_CSS >= TOGGLE_R + TOGGLE_HALO`（10 ≥ 9.5），防止以后改大视觉半径却忘了点击区。新增单测拆解两个实例的半宽 / 填充 alpha / 描边 alpha，钉死「环必须无填充」。
- 2026-10-19 · **WP6-1 悬停工具条** · 删掉旧的「选中节点外侧 22px 圆形＋」（`PLUS_R/PLUS_GAP/paint_plus/plus_rect`），换成悬停节点右侧的两枚 **22×22 圆角按钮**（「＋」「⋯」，圆角 6）：离节点边 `8px`（= `design::space::SM`），按钮间距 `4px`（= `design::space::XS`），**贴右上角之上 4px**（不是垂直居中——那会正好盖住右侧中线的折叠圆环），右边放不下就镜像到左侧，上方放不下就落到下方，都放不下就不画。
  命中区不压节点：`HOVER_GAP > mw_ui::drag::DROP_PAD_X` 由**编译期断言**钉住（8 > 6），点击按钮不会再被画布当成「按在节点上」起拖拽；指针跨越缝隙时用 `node_rect ∪ 按钮` 的悬停保持区接住，否则工具条会在指尖前消失。`MW_TRACE` 语义名为 `node-plus` / `node-more`，点击另写一条 `event`（鼠标点击不保证抢焦点）。5 条几何单测：22×22 / 离节点 ≥ DROP_PAD_X / 贴节点上缘 / 两个按钮都不压折叠圆环 / 右缘镜像 / 两侧都放不下时宁可不画 / 画布比按钮还矮时不 panic / 缝隙属于悬停区。
  **诚实标注**：静态 golden 没有指针、没有悬停态，所以这条只有几何断言与代码路径，没有像素证据；旧「＋」的移除也不改变基线（基线首帧没有选中节点）。
- 2026-10-19 · **WP6 起步（动效令牌化）** · 面板滑入/滑出时长从写死的 `0.2s` 改为 `design::motion::BASE`（0.18s），曲线仍是 `cubic_out`。写死时"快一点/慢一点"只能改代码，且无法与后续悬停/抽屉动画对齐节奏。`reduce_motion` 与快照 harness 仍然直接返回 0/1（不动画）。
  全量 **472 测试**、fmt、clippy 40（新增 0）、`--tol 0` 0 差异（动效不影响静态基线）。
  **未做**：折叠点小圆环（现在在 GPU 合成器里画实心圆，改环形要新增实例类型或在 shader 里做描边环，属独立一项）、悬停工具条。
- 2026-10-19 · **WP5 工具栏分组** · 组间间距 13 → **24px**（`TOOL_SEP`），组内按钮间距 2 → **4px**（新增 `TOOL_GAP`，同时用于宽度计算与绘制，避免"算的和画的不一致"）；分隔线从 18px 缩短到 16px、颜色由 `sep` 换成更淡的 `border`。以前组间 13、组内 2，只差 11px——分组"有"，但看不出层次。
  golden 重拍，全量 **472 测试**、fmt、clippy 40（新增 0）、`--tol 0` 0 差异、release 20,763,136 B。
  **诚实标注**：想用像素扫描量出"组间 24 / 组内 4"，但工具栏里图标笔画与分隔线同为浅灰、阈值法分不干净，所以这条只有常量与重拍基线，没有独立像素证据。
- 2026-10-19 · **WP5 起步（面板内距归一）** · 新增 `panels::PAD = design::space::LG`（16px）。检查器里 38 处硬编码的 `add_space(14.0)` 全部换成这个常量——多块面板并排时，文字起始线以前差 2px，肉眼能看出**没对齐**。行首缩进（大纲的 `PAD_L=6`）保持不动：那是行内层级缩进，不是面板内距。
  golden 重拍（整窗基线 191,369B → 见 shots/golden），全量 **472 测试**、fmt、clippy 40（新增 0）、`--tol 0` 0 差异。
  **下一步**：工具栏分组（文件 / 编辑 / 视图，组间 24px、组内 8px）+ 分组标题小号大写字距。
- 2026-10-19 · **WP4 几何部分完成** · 分支线两端加**离卡缝隙**：起点 +0.5px、终点 −1.5px（锥形末端最细，贴边会像"粘"在卡片上，而且卡片描边会吃掉一半线宽）；左右两侧布局都做了镜像处理。新增 `EDGE_GAP_START/END` 常量与两条几何测试（`branch_lines_keep_a_small_gap_from_the_cards` 断言缝隙 > 0 且 < 3、纵向对齐卡片中线；`edges_connect_facing_edges_in_horizontal_layouts` 同步更新）。
  三份 golden 重拍（canvas 27,621→27,594B、canvas-text 30,796→30,822B、shell 191,267→191,369B），`--tol 0` 两次 0 差异；全量 **472 测试**、fmt、clippy 40（新增 0）。
  **未做**：折叠点小圆环（现在还是实心点）——留给 WP6 交互质感统一处理。
- 2026-10-19 · **WP4 前半完成（可读性归一）** · 新增 `Theme::legible`：所有内置主题在出口处统一过一遍 WCAG——**分支线对画布/卡片 >= 3:1**、**每层文字对自身底色 >= 4.5:1**，不达标就按需压暗（保持色相，逐次 ×0.93；浅字压深底时改压填充而不是文字）。
  实测发现的问题：雪枝 #F6212D 白字仅 3.80、派对 #318DE1 白字 3.48；分支线里 #FEB40B 1.79、#6DC354 2.19、#FEC938 1.44 等**远低于 3:1**——线画在纸上几乎看不见。
  新增 `crates/mw-core/tests/contrast.rs`（2 条遍历全部 9 套主题×4 层级的测试）。三份 golden 重拍，`--tol 0` 两次 0 差异；全量 **471 测试**、fmt、clippy 40（新增 0）。
  **未做**：WP4 的几何部分（根部扇形引出、折叠点小圆环、端点 1px 内缩）留到下一轮。
- 2026-10-19 · **WP3b 完成** · 卡片内距改为**随字号分档**（`design::card_padding`）：中心(28px) 26/12、一级(20px) 18/10、二级(15px) 14/8、三级及以下 12/7。原先全应用一个 18/9，导致中心卡挤、深层卡松、层级感全靠字号硬撑。同步改了四处消费点（`scene` 测量与 text_x/labels、`mw-render` 图标对齐、`mw-shell` 行内编辑器内距），避免"卡片变大而编辑器没跟上"的错位。三份 golden 重拍（canvas 27,863→27,621B、canvas-text 30,817→30,796B、shell 191,383→191,267B），`--tol 0` 连跑两次 0 差异；全量 **469 测试**、fmt、clippy 40（新增 0）。
  单测钉住：内距随字号**单调不增**、水平内距必须大于垂直内距、中心与子级的内距差 >= 8/4px（差太小眼睛看不出来）。
- 2026-10-19 · **WP3 完成** · 行高从写死 1.45 改成**分档令牌**（11–12px 1.50 / 13–15px 1.46 / 16–21px 1.44 / ≥22px 1.40），20px 一级卡行高 29、卡高 47px——仍与 XMind 实测 46.7 对齐；标签胶囊上下留白 7→8（落到 4px 网格）。单测把"行高/字号 比值随字号单调下降"和三个具体值钉住。重拍三份 golden（canvas 27,911→27,863B、canvas-text 30,308→30,817B、shell 191,457→191,383B），`--tol 0` 连跑两次 0 差异；全量 **468 测试**、fmt、clippy 40（新增 0）。
  **本轮没做（诚实）**：内距随字号成比例、字重 400/500/600 分级——前者要同时改 scene 测量与 canvas 编辑器几何，后者要把 `bold: bool` 换成 `weight: u16` 并打通 DirectWrite 三个字面，两者都会牵动导出管线，留到 WP3b。
- 2026-10-19 · **WP2 完成** · 默认主题从「素白」改成 **「纸感」**：画布 #FBFAF7（暖白）、文字 #2A2E33、面板 #FAF8F4 / #F3F0EA、强调色换成灰绿 #3A7D6C；调色板 8 支统一压饱和（青绿/陶土/赭石/橄榄/靛蓝/紫檀/玫瑰/石板）；一级卡改为**白底 + 发丝边 #E3DFD7 1px**（不堆阴影）；中心卡 #2E6F62 实色；线宽 1.75/3.5。
  证据（像素）：画布主色 (251,250,247) 占 **90.86%**、纯白像素 **0.07%**、中心卡 (46,111,98) 命中。`LEVEL_TYPE` 改为引用 `design` 令牌。
  外观大改 → **重拍三份 golden**：shell 189,737→191,457B、canvas 26,792→27,911B、canvas-text 29,515→30,308B；重拍后 `--tol 0` 连跑两次 0 差异。全量 **468 测试**、fmt、clippy 40（新增 0）。
- 2026-10-19 · **WP1 开工** · 新增 `mw-core::design`：4px 间距网格（XS 4 / SM 8 / MD 12 / LG 16 / XL 24 / XXL 32）、字阶（11/12/13/15/15/20/28/24）+ 分档行高（1.42/1.36/1.30/1.22）、圆角三档（6/10/14）、发丝线 1px@6%、动效三档（0.12/0.18/0.24s）、`contrast()` WCAG 工具。4 条单测：网格对齐、字阶单调 + 行高越界检查、默认「纸感」配色正文与次要文字对比度 >= 4.5、对比度公式与已知值/对称性。全量 468 测试、fmt、clippy 40（新增 0）、golden 0 差异。

## 第 69 轮 · 交接轮（2026-11-02 20:0x–21:0x）：两门实测 + X22–X30 审计 + 导出/cmap 修码

**背景**：用户决定更换 AI，要求本轮收尾并把状态写清。本轮做三件事：
① 把两道正式门跑出**日志级结论**；② 把 Stage C 剩余包（X22–X30）的现状审计与设计合同铺开；
③ 修掉审计中**不需要拍板就能修**的真 bug（导出同源、`.cxl` 缺陷）。

**门**：
- `stage-b` **正式门 PASS**（19:48:38–20:15:05，`target/x100/stage-b-gate.log`）：
  证据包 / 语法 / fmt / clippy / **测试 1328 / 0 / 1 ignored** / **golden 178×2 零差异**。
- `stage-cm` **首跑 FAIL**（20:15–20:19）：门内项全过，红在并行会话在飞的
  `mw-core/src/geom.rs`、`mw-shell/src/canvas/tests.rs`（未格式化）与
  `canvas_methods_3.rs` 一条 clippy。本轮把 clippy 那行改成 `then_some`、跑全仓 `cargo fmt` 后再跑：
  fmt / clippy / `.zmind` 往返 / 全量测试 / **golden 第 1 遍 74/74** 全 PASS；
  **第 2 遍红在构建被打断**（`error[E0063]: missing field flash_window in initializer of canvas::Canvas`，
  并行会话当时正在重构 `Canvas`），**无画面漂移记录**。→ 门仍记红，复跑前提是「没人在构建」。

**修码（两个 crate，均在审计后当场修）**：
- `mw-shell` 导出同源（X24 G-01…G-07）：SVG/PDF 的树边改为调 `edge_path_with(…, e.shape)`，
  锥形按弧长插值出填充轮廓、箭头与画布同款 7 css 三角形，删掉 PDF 的第二套几何（`AL=9.0/AW=6.5`）。
  `cargo test -p mw-shell` → **736 / 0 / 1 ignored**；新增 8 条几何单测；画布路径一字未动。
- `mw-io` `.cxl`（CM-23 四个真缺陷）：时间戳改真 ISO 8601、读回 `dc:title`、
  反向箭头写到自己那条边并可反推、删掉 `groups` 死参数；`cargo test -p mw-io` → **38 / 0**；
  新增 11 条单测。**未修**：节点扩展字段写得出读不回（根因在 `mw-format`，越界，已钉成测试）。
- X22 的 G04（中英混排度量同源）**写完又撤回**：只影响含「汉字紧邻 ASCII」的盒
  （实测 `canvas-text-ppp1.00` 漂 0.090%），收尾轮不允许重拍基线，
  补丁留 `shots/x100/X22/evidence/g04-measure-parity-2026-11-02.patch`。

**文档**：`docs/28` §4.1 按实现校准 9 档 + §4.5 选中 140→180ms；`docs/30` 新增 §5.3.2
（画布主选/键盘焦点合同）；`docs/24` X23 补「节点不投影」；`docs/06` §2.2 线宽标历史值；
`shots/x100/X22…X30`、`CM-23` 报告 + manifest；`docs/STATUS.md` §0/§1/§4.3/§5/§6 回填。

**裁决**：`docs/x100/log/decisions-2026-11-02-stage-c.md`（9 包 / 41 条）。
一句话：**能自己收的都收了；剩下的等三件事** —— 等并行会话停手复跑 `stage-cm`、
等人工签字（实机拼音 + 系统 DPI）、等用户对「新字段 / 新形状 / 嵌套概念」拍板。
