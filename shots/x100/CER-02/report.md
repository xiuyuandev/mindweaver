# CER-02 · 内容表面通用组件

> 状态：已实现 · 开始 2026-10-25 · 批次 B2-A · 依赖：CER-01（已完成）
> 设计合同：[`docs/28`](../../../docs/28-内容体验重构规范.md) §4（§4.1–§4.6）
> 执行合同：[`docs/29`](../../../docs/29-内容体验重构工作包.md) §6

## 1. 范围

新建 `crates/mw-shell/src/content_kit.rs`（`mw-shell` 内新模块，**不新建 crate**），
九个内容表面通用组件。全部消费 `mw_core::design::content` 令牌
（`content_metrics(theme, dpi)` 派生的 `ContentColors` / `ContentMetrics`），
复用 `icons` / `icon_paint` / `ui_kit` 的既有绘制习惯，禁止第二份像素值：

1. **ContentPage** 页面骨架：页头（默认 40px，高度可配——CER-23 的 48px 库页头
   用同一组件）、内容滚动区、页脚（默认 0，大纲 24px 用同一组件）；
   页头/页脚与内容之间画 1px `divider`（线宽取 `metrics.hairline`，1 物理像素）。
   内容区由调用方在闭包里填（`ScrollArea` 由组件内建，`id_salt` 防碰撞）。
2. **PageTitle**：按 `TypeStyle` 渲染单行标题（`PAGE_TITLE` 30/40/600、
   `OUTLINE_TITLE` 28/38/600 等）；600 字重用 `fonts::BOLD` 族（偏差 ①，见下）。
3. **TextAction**：13/20 文字动作（NAV 档），`link` 色，hover 下划线，无底无框；
   禁用态 `text.faint`、不可点。
4. **InlineBanner**：32px 行内提示条：13/20 `text.secondary` 文案 + 右侧若干
   TextAction；上下 1px `divider`；无填充、无圆角、无图标（克制原则；
   CER-20/27/29 接线时人工复核可见性）。
5. **EmptyState**：左对齐空状态：16/26/600 标题（`text.primary`）+ 13/20 说明
   （`text.secondary`）+ 可选一个 TextAction；无图标、不居中。
6. **SearchField**：32px 输入框：圆角 6、1px `hairline` 边（键盘焦点换焦点环）、
   左内距 10 + 14px 放大镜图标 + 13/20 文本；placeholder `text.faint`。
   基于 egui `TextEdit::singleline`，IME 组合态由 egui 既有路径承接。
7. **FocusRing**：纯绘制助手：1.5px 强调色 80%、外偏移 1px、圆角 6
   （`content::focus` 令牌）；只在调用方声明"键盘焦点"时画。
8. **TagText**：13/20 纯文字标签（`text.secondary`），可选 6px 圆点前缀
   （`tag_dots`）；hover：6% 中性底（圆角 6）+ 下划线；返回 `Response`
   由调用方接跳转/删除；无彩色 chip。
9. **MoreButton**：28×28 命中、16px `More` 图标（`icons::Icon::More`）；
   hover `state.hover` 底 + `text.primary` 图标，默认 `text.secondary`。

**接线（仅演示）**：`MW_CONTENT_KIT=1` 启动一个组件陈列页（gallery），
作为 `content-kit` golden 的被摄体；正常启动不可见，不进任何交互路径。
陈列页各节用固定行高，保证输入脚本坐标稳定。

### 字重偏差（开工前钉死，G-B2-1 人工复核）

egui 侧只注册了两档雅黑：常规 400（`msyh.ttc` 索引 1）与
`zhisi-bold`（`msyhbd.ttc`，700 面）。`docs/28` §4.1 要求 400/500/600：

- **600**（PAGE_TITLE / OUTLINE_TITLE / H2–H4）：用 `zhisi-bold` 700 面呈现。
  层级意图（标题读作标题）优先，视觉重量比 600 略重；**偏差 ①**。
  不合成粗体、不新增字体文件。
- **500**（LIST_TITLE / TAG）：用 400 面呈现，层级由字号与灰度补足；**偏差 ②**。
- 若门判词否决，回退方案：全部 400 + 字号差，或届时评估 DirectWrite 文本路径。

## 2. 非目标

- 不接线大纲/卡片页/笔记库（CER-04 起）；陈列页不是产品页面，不含真实数据。
- 不做行内编辑、斜杠菜单、目录浮层（CER-07/16/18）。
- 不改 `ui_kit` 现有组件、chrome 令牌与画布；不新增依赖/crate/字体。
- 状态切换**即时落位**，不做 120ms 过渡：hover/选中渐变属于页面层动效
  （§4.5），与 golden 稳定性耦合，CER-04 起随页面包实现；本包组件只输出状态。
- 滚动条组件本包只消费令牌（CER-01 已留），不自绘 6px 滚动条——内容页滚动条
  在 CER-04 骨架里做。
- 不改 `.zmind`、`Document`、`NoteCard`、命令层。

## 3. 验收场景

| 场景 | 输入 | 期望 |
| --- | --- | --- |
| `content-kit` | 陈列页：九组件按节平铺（标题两档、搜索空/填充、标签 6 点、动作默认/禁用、横幅两条、空状态、焦点环样张） | 四主题（snow/ink/mist/sand）× 1.0/1.5/2.0 DPI 共 **12 基线**；字阶/圆角/透明度与 `docs/28` §4 一致；固定行高 |
| `content-kit-hover` | `MW_GOLDEN_INPUT` 脚本先悬停 TextAction 行、收尾悬停在 #知识 标签行 | hover 底（6% 中性）+ 下划线入画；1 基线 @1.5 snow |
| 组件单测 | 纯函数 | 骨架拆分（页头/内容/页脚矩形求和 + divider 位置）；SearchField 几何（高 32、图标 14、文本起点）；InlineBanner 高 32；FocusRing 矩形外扩 1px / 线宽 1.5；TagText 长文本截断；TextAction 禁用色 |

## 4. 旧基线

不改既有绘制路径：新增模块 + `main.rs` 一处路由分支（`content_kit_demo` 时
提前返回，位于 home/demo/outline 判断之前）+ 启动钩子一处。

- 开工基线（CER-01 后）：**770 tests / 0 failed**；31 组 golden 双跑 `--tol 0`
  全 0 差异；fmt 干净；mw-shell clippy 仅 3 条历史告警。
- 本包要求：既有 31 组**逐字节不变**（新场景全部走新目录）；
  测试数增加、失败 0；clippy 新增 0。

## 5. 变更

| 文件 | 变更 | 动外观/数据/格式 |
| --- | --- | --- |
| `crates/mw-shell/src/content_kit.rs` | **新增**：九组件（`content_page`/`page_title`/`text_action`/`inline_banner`/`empty_state`/`search_field`/`focus_ring`/`tag_text`/`page_icon_button`+`more_button`）、`font_for` 字重映射（偏差 ①/② 的唯一落点）、纯函数 `split_page`、陈列页 `show_gallery` 与 hover 锚点 `gallery_anchors`、7 个单测 | 否 |
| `crates/mw-shell/src/main.rs` | `mod content_kit;`；`App.content_kit_demo` 字段；CentralPanel 路由首分支（`content_kit_demo` 时渲染陈列页并提前返回） | 否（环境变量门控，正常启动不可见） |
| `crates/mw-shell/src/app_methods_1.rs` | `content_kit_demo: false` 初始化；`MW_CONTENT_KIT=1` 启动钩子（置位并关开始页） | 否 |
| `shots/golden/inputs/content-kit-hover.txt` | hover 脚本（先 TextAction 行、收尾标签行；坐标按基线实测校准） | 否 |
| `shots/golden/content-kit-{snow,ink,mist,sand}/` | 各 3 档 ppp 基线（共 12） | 新增场景 |
| `shots/golden/content-kit-hover/` | hover 基线 1 张 @1.5 snow | 新增场景 |

组件对环境间距免疫：`content_page` 三个子区显式清零 egui `item_spacing`，
垂直节奏全部由组件显式间距管理（开发中发现环境间距会把内容推出视口）。
开发中修复一个会毒化全部基线的缺陷：`Rgba`→`Color32` 转换最初丢弃 alpha，
状态填充（selected 7% 等）被渲染成实心强调色；已改为保留源 alpha，
不透明令牌结果不变。

## 6. 验证

**行为证据**

- `cargo test --workspace -- --test-threads=1` → **777 passed / 0 failed**
  （CER-01 后 770 + 本包 7：`split_page_sums_to_total_and_clamps_overflow`、
  `search_field_geometry_matches_contract`、`banner_and_tag_rows_match_contract`、
  `focus_ring_matches_content_tokens`、`font_for_only_uses_bold_family_for_semibold`、
  `empty_state_and_gallery_type_matches_docs28`、
  `page_icon_button_and_more_geometry_match_outline_header`）。
- hover 态由 `MW_GOLDEN_INPUT` 脚本注入（`content-kit-hover.txt`），非手点。

**视觉证据**

- 13 张基线：四主题 × 1.0/1.5/2.0 + hover @1.5 snow。四主题 1.5 档均经
  人工核验（snow/ink/mist/sand PNG 逐张过目）：字阶三层灰度、选中底/焦点环/
  链接色/标签点各归其位，无框、无彩色 chip、无阴影。
- hover 基线显示标签行 6% 中性底 + 下划线；其余与静态基线一致。
- 陈列页按 1200×860 设计（场景表指定 `MW_WINDOW_SIZE`），全部七节完整入画。

**回归证据**

- 13 个新场景连跑两次 `--tol 0` 全 0 差异；修 clippy 后重建 release 又第三次复核，仍 0。
- 旧 31 组 golden 矩阵双跑 `--tol 0` PASS（`target/x100/cer-02-golden.log`）：
  本包不改任何既有绘制路径，`content_kit` 仅在 `MW_CONTENT_KIT=1` 分支可达。
- 未跑旧文件往返/撤销重做：本包不触碰文档模型、命令层与磁盘。

**工程证据**

- `cargo fmt --all -- --check` → 干净。
- `cargo clippy -p mw-shell --all-targets` → `content_kit` 相关新增 0。
- release 构建通过（golden 13 场景与 hover 脚本均在 release 二进制上验证）。

## 7. 未做 / 已知偏差

- **字重偏差 ①**：600 用 `zhisi-bold`（700 面）呈现；**偏差 ②**：500 用 400 面呈现。
  由 `font_for_only_uses_bold_family_for_semibold` 单测钉住；G-B2-1 人工复核，
  若否决按报告 §1 的回退方案处理。
- **egui 行盒高度 ≠ 规格行高**：`layout_no_wrap` 的 galley 高度由字体
  ascent/descent 决定（30px 雅黑约 44px，规格行高 40）。组件按 galley 实际高度
  占位；CER-04+ 页面做垂直节奏时以显式间距补足，不能拿 galley 高当行高。
- 状态切换即时落位，无 120ms 透明度过渡（§2 非目标声明）；动效随页面包实现。
- 陈列页为 1200×860 设计，更窄窗口会滚动；hover 锚点只对基线窗口有效，
  改布局必须重新校准 `gallery_anchors` 与脚本坐标。
- 滚动条自绘未做（CER-04 骨架）；`InlineBanner` 无填充的可见性与
  `SearchField` 深色边对比度留人工门复核。
- 组件 API 尚无产品页面消费者（CER-04 起接线）；`MoreButton` 点击在陈列页
  无动作。

## 8. 风险

- **字重偏差 ①/②** 是全批共享的决定：CER-04+ 的页面标题、列表标题都沿用
  `content_kit::font_for(weight)`；若 G-B2-1 否决，回退会波及 A 批已接线页面。
- egui 的 CJK 换行由既有文本布局承担；`docs/28` 的禁则要求在 CER-05（大纲行
  绘制）与 CER-15（块排版）实测，本包组件不引入新的换行逻辑。
- 陈列页坐标稳定性依赖固定行高；后续若加节必须同步更新 hover 脚本坐标。
- `InlineBanner` 无填充的可见性、`SearchField` 在深色主题的边对比度，均属
  审美判断，留人工门复核。

## 9. 人工判词

非门包，不填。
