# SE-02 · 起始页视觉与状态（P 轨 E1）

> 设计合同：`docs/30` §3.3（视觉与颜色）、§3.4（状态）；执行合同：`docs/31` §4。
> 依赖 SE-01（已实现）。本轮先做**审计**，只补真缺的，不重做已经对的。

## 1. 开工前审计（对着合同逐条查代码）

| 合同条目 | 现状 | 结论 |
| --- | --- | --- |
| 卡底 `surface.panel` + 1px hairline、**无阴影** | `start::quick_card` / `template_card` 都用 `tone.card` + `design::stroke::HAIRLINE`，没有任何 shadow 调用 | ✅ 已在 |
| 主作用卡 8% 强调底 + 强调色图标（禁止实底 hero） | `quick_card`：`primary` 时 `mix(tone.card, accent, 0.08)`，图标用 `accent` 上色；深色主题 chip 另加一档 | ✅ 已在 |
| `design::icon` 新增 `ConceptMap` + 六个结构图标 | `icons.rs` 里 `ConceptMap / Mindmap / Logic / Org / Fishbone / Timeline` 都在，`Icon::ALL` 与 `trace_name` 穷尽匹配 | ✅ 已在 |
| 结构图标统一 28px、单色、1.5px 主描边，**不再手绘示意图** | `template_card` 用 `paint_icon_sized(..., 28.0, tpl.icon, tone.ink)`；无彩色示意图 | ✅ 已在 |
| 最近文件右时间用 secondary（不用 faint） | `recents_card` 时间列 `tone.muted`（= secondary） | ✅ 已在 |
| 空状态左对齐两行 | `empty_recents`：图标 + 「还没有最近文件」+「打开或新建后，会在这里快速回来」，两行左对齐 | ✅ 已在 |
| 图标几何测试（16 单位边界） | `icon_paint::tests::every_icon_stays_inside_the_sixteen_unit_grid` 遍历 `Icon::ALL`（37 个，含 6 个结构图标） | ✅ 已在 |
| **3 行加载骨架** | 没有。最近文件在 `App::new` 里同步读 `recent.json`，没有可挂骨架的「加载中」态 | ❌ **缺** |
| 证据：`start` 四主题 | 只有默认主题的 `start`；`theme-ink/mist/sand` 目录里只有 `shell-ppp1.50`，没有起始页 | ❌ **缺** |
| 证据：空状态 / 骨架场景 | `start-empty` / `start-skeleton` 都没拍 | ❌ **缺** |
| 证据：`start-concept`（hover 态） | 没拍 | ❌ **缺** |

结论：SE-02 的**视觉条款基本已落地**（这是 SE-01 那轮顺带做的），真缺的是
**加载骨架**与**四主题 / 空状态 / 悬停三组证据**。因此本轮做：

1. 最近文件改**异步加载**（`recent::Recent` 起一个后台线程读盘），加载中画 3 行骨架；
2. 新增场景：`start-empty`、`start-skeleton`、`start-concept`、`start-ink` / `start-mist` / `start-sand`；
3. 回填 `docs/STATUS.md` §4.1 与 SE 包表。

## 2. 非目标

- 不改版心/断点/字号（§3.2 是 SE-03 的事：键盘、响应式）。
- 不改快速开始卡的内容与顺序（四张卡是 SE-01 定下的）。
- 不引入新依赖：后台读盘用 `std::thread` + `std::sync::mpsc`。

## 3. 验收场景

1. **骨架**：起始页首帧到列表就绪之间，最近文件区画 3 行骨架（灰条），不是空白也不是「还没有最近文件」；
   列表一到就换成真行。轨迹记 `recents loaded n=…`（有几条）与 `recents pending`。
2. **空状态**：列表为空且已就绪 → 两行空状态（已实现，本轮补一张基线 `start-empty`）。
3. **四主题**：`start-ink` / `start-mist` / `start-sand` 各一张 1.5 基线，锁住主卡强调底与图标色在四套主题下的表现。
4. **悬停**：`start-concept` 用 `MW_GOLDEN_INPUT` 把指针移到「新建概念图」卡上，锁 4% hover 底与强调描边。

## 4. 预计证据

- 轨迹：`recents pending` → `recents loaded n=1`（异步路径真的走过一次）；
- 基线：`start-empty`、`start-skeleton`、`start-concept`、`start-ink/-mist/-sand`（`--tol 0` 双跑）；
- 单测：骨架行数/高度断言（纯函数），异步加载的「首帧 pending、次帧 loaded」用 `Recent::load_async` 的
  单元测试（临时目录写一个 recent.json，等通道）；
- 两道门绿、测试数 +N。

## 5. 风险

- 异步读盘会让「打开在起始页 → 立即点最近文件」出现一个空窗（列表还没到）。处理：加载中列表区不可点，
  骨架本身不吃点击；`Recent` 保留同步入口给测试用。
- 金样是逐帧确定的：骨架只存在于「第一次拿到结果之前」，`MW_GOLDEN_INPUT` 的 `wait` 帧数决定拍到哪一帧——
  `start-skeleton` 必须在第 1 帧拍（warmup 0）才不会拍成已加载；这一点要在场景注释里写死。

## 6. 结果（打磨轮 20，2026-10-29）

| 项 | 结果 |
| --- | --- |
| 加载骨架 | `Recent::load_async` + `start::recents_skeleton`：3 行、与三行真列表同高（`recents_height(3)` = 144）、不吃点击 |
| 骨架证据 | 骨架 vs 三行真列表：px>8 = **12386**，bbox (536,345)-(1387,514) 只在最近文件卡内（`skeleton-diff.txt`） |
| 悬停证据 | 指针压「新建概念图」卡：px>8 = **12766**，bbox (743,199)-(953,270) 正好是卡面，卡外 **0** 差异（`hover-diff.txt`） |
| 四主题 | `start-ink/-mist/-sand` @1.5 各一张；整页均色 ink(32,35,40) / mist(239,244,241) / sand(242,237,228) / 默认(244,241,236) |
| 轨迹 | `quick rect concept 743,199 211x72`（`rect-trace.txt`）——悬停脚本的坐标就从这行来，不是猜的 |
| 单测 | +2：`the_pending_list_is_empty_but_still_knows_where_to_write`、`async_load_hands_back_what_the_sync_one_would`；全量 **937** |
| 门 | stage-b **106 场 / 212 次比较** 全过（新增 6 场）；stage-cm 48/96 全过、152 张零漂移 |

**一句话读法**：SE-02 的视觉条款本来就在，这一轮补的是「数据还没到时页面长什么样」——
3 行骨架、与真列表同高、不可点，以及把它在四主题与悬停态下钉进基线。
