# SE-04 · 起始页证据包与 G-B3-1（P 轨 E1 第四包）

> 设计合同：`docs/30` §3（起始页 v2 全节）；执行合同：`docs/31` §4 与 §3「完成定义」。
> 依赖 SE-01、SE-02、SE-03（都已实现）。

## 1. 开工前审计（对着 `docs/30` §3 逐条查代码）

审的是「合同写了什么」对上「代码做了没有」，不是「看起来像不像」。

| 合同条目 | 现状 | 结论 |
| --- | --- | --- |
| §3.1 五段顺序：品牌头 → 快速开始 → 最近文件 → 模板 → 提示行 | `start::show` 的 body 顺序一致；没有大提示卡、没有居中脚注 | ✅ |
| §3.1 快速开始四卡内容（新建思维导图 / 新建概念图 / 打开文件… / 卡片笔记库） | `quick_card` 四张，`Quick` 四个变体 | ✅ |
| §3.1 模板六张（思维导图 / 逻辑图 / 组织架构 / 鱼骨图 / 时间轴 / 概念图） | `TEMPLATES` 六项 + 六个结构图标 | ✅ |
| §3.2 版心 880 / 页边距 32 / 顶底 40 | `MAX_W = 880`、`PAGE_PAD = 32`、`PAGE_PAD_TOP/BOTTOM = 40` | ✅ |
| §3.2 品牌头 44 / 图标 32 / 产品名 20 | `BRAND_H = 44`、`BRAND_ICON = 32`、`BRAND_NAME_SIZE = 20` | ✅ |
| §3.2 快速开始卡 72 / 间距 12 / 标题 14 / 副标题 12 | `QUICK_H = 72`、`GAP = 12`、`size::QUICK_TITLE = 14`、`QUICK_HINT = 12` | ✅ |
| §3.2 最近文件行 44 / 标题 13.5 / 路径与时间 11.5 secondary | `ROW_H = 44`、`ROW_TITLE = 13.5`、`ROW_META = 11.5`、两处都用 `tone.muted` | ✅ |
| §3.2 模板卡 88 / 图标 28 / 名称 13.5 / 说明 11.5 / 键帽 11.5 | `TPL_H = 88`、`paint_icon_sized(..., 28.0, ...)`、`TPL_NAME/TPL_HINT/KEYCAP` 对得上 | ✅ |
| §3.2 提示行 32 / 12 secondary + 键帽 | `FOOTER_H = 32`、`size::FOOTER = 12`、`size::KEYCAP = 11.5` | ✅ |
| §3.2 断点 900 / 640 三档（4/2×2/1 列，模板 3/2/1，最近 8/5/3） | `start::layout` 单点判定；单测 `layout_is_the_single_source_of_breakpoints` + SE-03 新增 `cards_stay_between_180_and_the_page_width` | ✅ |
| §3.2 不横向溢出、640×460 不文字压边 | `start-narrow` / `start-scroll` 基线 + 上条单测 | ✅ |
| §3.3 页面底/卡片底/无阴影/1px hairline | `tone.bg` / `tone.card` + `design::stroke::HAIRLINE`，无 shadow 调用 | ✅ |
| §3.3 主卡 8% 强调底 + 强调色图标，禁止实底 hero | `mix(tone.card, accent, 0.08)` + `icon_ink = accent` | ✅ |
| §3.3 六个结构图标 28px 单色 1.5px 描边 | `icons.rs` 六个 + `every_icon_stays_inside_the_sixteen_unit_grid` | ✅ |
| §3.3/§3.4 焦点环：卡外 2px、主卡卡内 2px | SE-03 落地；`ring-probe.txt` 实测卡外 0 强调色像素 | ✅ |
| §3.4 hover 4% 底 + 手型 + `trace::name` | `quick_card` / `template_card` / 最近行都走同一套 | ✅ |
| §3.4 激活无按下缩放、按下 90% 底 | 卡片没有缩放；`ui_kit::activated` 统一 | ✅ |
| §3.4 最近文件为空：左对齐两行、不放打开按钮 | `empty_recents` 两行，没有按钮 | ✅ |
| §3.4 加载中：3 行骨架、不转圈 | `recents_skeleton`（SE-02） | ✅ |
| §3.4 **最近文件缺失：保留行，标题 secondary，路径标「文件已移动」，点击走打开失败态** | `Recent::prune_missing()` 在启动时**直接把这行删掉**（`recent.rs` 的 `retain(|e| e.path.exists())`），全仓没有「文件已移动」这四个字 | ❌ **与合同相反** |
| §3.5 键盘（Tab 序、1–6、Ctrl+N/O、Ctrl+Shift+N、Enter 守卫、自动聚焦） | SE-03 收口；轨迹 `shots/x100/SE-03/tab-order.txt` | ✅ |
| §3.6 概念图四个入口 | SE-02（卡片/模板）+ SE-03（快捷键/菜单） | ✅ |
| §3.7 七条禁止项（实底 hero、5 张手绘预览、彩色示意图、148/216 提示卡、居中脚注、装饰留白、faint 承载时间/路径、搜索/预览、刷新/设置按钮） | 逐条对照：一条都没有 | ✅ |

结论：**唯一没做到的是 §3.4 的「最近文件缺失」**——现在的行为是启动时把这行删掉（`prune_missing`），
而合同要的是**留着那一行、把它标灰、点它走打开失败态**。这不是外观问题：
文档被移动/改名之后，用户看到的是「最近文件里少了一条」，而不是「这一条去哪儿了」。

## 2. 非目标

- 不加「重新定位」「在文件夹中显示」这类补救动作（合同没写，属于新功能）。
- 不改 `recent.json` 的结构与语义（`docs/31` §4.1）：缺失标记只进内存，不落盘。
- 不动 §3 已通过的几何与断点。

## 3. 验收场景

1. **缺失行保留并标灰**：种三条最近文件、把第二条的路径指到不存在的文件 → 三条都还在；
   第二条标题用 secondary 色、副行写「文件已移动」（替换原来的目录提示），时间列照常。
2. **点击走打开失败态**：点第二条 → 弹既有的打开失败面板（`OpenFailure { from_recent: true }`），
   不是静默失败、也不是把这行删掉。
3. **新基线** `start-missing`：与 `start` 同一页，只差第二条的样子；差异必须只落在第二条行内。
4. **不回归**：`start*` 系列基线不变（demo 播种的三条是虚构路径，必须仍然显示成正常行）。

## 4. 预计证据

- 轨迹：`start-missing` 的 `recents row … missing=1`、点击后的 `open failed from_recent=1`。
- 基线：`start-missing`（`--tol 0` 双跑），与 `start` 的逐像素差异只落在第二条行内。
- 单测：`missing_entries_are_kept_and_marked`（纯 `Recent` 逻辑：`mark_missing` 之后条数不变、
  缺失集合正确、落盘内容仍是原三条）。
- `docs/30` §3.4 那行补「落地」注；`manifest.json`；STATUS 回填。

## 5. 风险

- 「每帧 stat 磁盘」会让长列表卡顿：只在载入/触摸时算一次，`missing` 集合存内存。
- demo 播种的三条是虚构路径（`C:/示例/…`），若不显式清空缺失集合，`start*` 九张基线会一起变红。
- 打开失败的重试路径：失败面板关掉之后那行仍然在（这是对的——文件可能只是暂时不在盘上）。

## 6. 结果（第 22 轮，2026-10-30）

| 项 | 结果 |
| --- | --- |
| 缺失行保留 | `Recent::Entry.missing`（`#[serde(skip)]`）+ `Recent::mark_missing()`：载入与 `touch` 时算一次，**不删条目**；`prune_missing` 已删。`recent.json` 结构不变，缺键即老行为 |
| 缺失行外观 | `recents_card` 缺失行：标题 `tone.secondary`、副行「文件已移动」、时间列照常；轨迹 `recents row missing title=读书笔记 rect=526,433 868x44`（`missing-trace.txt`） |
| 点击 | `start-missing-click.txt`（行中心 960,455，坐标取自稳定后的行矩形）→ 轨迹 `open_start` → `open_fail from_recent=1`，走既有打开失败面板，不静默、不删行（`missing-click.txt`） |
| 新基线 | `start-missing`（1440×1000，`MW_RECENTS_MISSING=1`）：`--tol 0` 双跑零差异。与 `start` 逐像素比 **1082 px，bbox `x562..627 y441..470`** = 第二条行的标题与副行，**行外 0 差异**（`missing-row-diff.txt`） |
| 不回归 | `start*` 其余 10 张基线零漂移（demo 播种的三条虚构路径仍显示为正常行——`mark_demo_missing` 只标被点的那一条） |
| 单测 | `missing_files_are_pruned` → `missing_files_stay_in_the_list_and_get_marked`（语义反转）；全量 **939 / 0 失败** |
| 门 | stage-b **107 场 / 214 次比较**双跑全过（2026-10-30 00:40，日志 `target/x100/stage-b-gate.log`）；stage-cm 48/96 复跑见 `docs/STATUS.md` §1 |
| 人工门 | 清单 `gate.md`：五判词逐条留 ☐ —— **本轮不代写结论**，SE-01–04 仍只记 `已实现` |

**一句话读法**：SE-04 没换皮，改的是「文件没了」这件事的说法——以前是那一行悄悄消失，现在是那一行留下来、
说清「文件已移动」、点它会老老实实报打开失败。
