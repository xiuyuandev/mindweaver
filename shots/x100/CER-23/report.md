# CER-23 · 自适应三/两/单栏骨架 + 内容令牌接主题

日期：2026-11-02　状态：**已实现**（机器证据齐；G-B2-3 人工门未开）
合同：`docs/29` §8 CER-23；设计：`docs/28` §7.2（分档）、§7.1–§7.5（尺寸）、§3（令牌）。

## 1. 范围

- 分档纯函数 `Pane::of(width)`：≥1200 三栏（侧栏 232 + 列表 360 + 预览余量 ≥480）、
  840–1199 两栏（侧栏 + 列表，**预览变整页阅读面**，不是被挤窄的第三栏）、
  <840 单栏（侧栏与列表都进抽屉，页头出抽屉开关）。拖窗跨档不重建、不丢状态：
  选择集、光标、滚动、筛选都在 `NoteLibrary` 上，分档每帧现算。
- 宽度可拖并记忆：侧栏 200–280、列表 320–440（`WindowState` 落盘）；夹取不回绕
  （`sidebar_width_is_clamped_not_wrapped` 钉住「拖到边界外记边界值」）。
- 页头 48：返回 + 标题 + 计数在左，「更多」在右；极窄窗口退化不叠字
  （`split_header_gives_48_and_the_rest` / `split_header_degenerates_on_a_tiny_window`）。
- 三栏空间不够时的收缩顺序：先列表后侧栏，预览永远 ≥480
  （`three_pane_keeps_preview_at_least_480`、`three_pane_shrinks_the_list_before_the_sidebar`、
  `three_pane_shrinks_sidebar_only_as_last_resort`）。
- 内容色**只**从 `design::content` 取（`ContentColors::for_theme`）：文字三档、hover 4%、
  selected 7%、nav_selected 4%、hairline/divider 半透明令牌，四套主题各一份 alpha 表。
  壳层 Chrome 的颜色一律不进笔记库——亮色分支把面板写死成纯白会让四套主题塌成一张灰白纸。
- `c32()` 必须带 alpha（`from_rgba_unmultiplied`）：hover/selected/hairline 全是 4–12% 的
  半透明令牌，丢成不透明就会画出实心色块与近黑分隔线（2026-11-02 实拍修掉）。

## 2. 非目标

- 不动卡片页与画布的 Chrome 令牌（SE-11 的对比度门已在 P 轨钉过）。
- 不做窗口级布局记忆以外的「每档独立状态」（跨档共享一份状态是合同要的）。

## 3. 验收场景与证据

| 合同要求 | 证据 |
| --- | --- |
| 三/两/单栏各一张 | `library-three-pane`（css 1280×800）、`library-two-pane`（1080×720）、`library-one-pane`（800×720） |
| 宽度标尺（DPI 矩阵） | `shots/golden/library/` ppp 1.0/1.5/2.0：离屏帧缓冲固定物理 1620×1080，css = 1620/1080/810 → 三/两/单栏各一张，正是自适应骨架在 DPI 矩阵里的正确样子 |
| 四主题 | `library-three-pane`(snow) + `library-ink` / `library-mist` / `library-sand`：同一场只换 `MW_THEME`，背景/文字/圆点/选中底全随主题走 |
| 分档与收缩顺序单测 | `note_library/tests.rs`：`pane_breakpoints_follow_7_2`、`three_pane_*` 三条、`two_and_one_pane_have_no_preview_column` |
| 拖宽夹取与记忆 | `sidebar_width_is_clamped_not_wrapped` + `WindowState` 落盘字段 |
| 无刷新跨档 | 状态在 `NoteLibrary`；跨档只改 `Pane`，轨迹 `library_resize`、`library_zone` |

基线 sha 见 `manifest.json`。17 张基线双跑 `--tol 0` 零差异（2026-11-02）。

## 4. 未做 / 风险

- 离屏帧缓冲被 `with_clamp_size_to_monitor_size(true)` 夹到本机显示器物理 1920×1200
  （`main.rs` 的 `ViewportBuilder`），所以 ppp2.0 档的 css 视口是 810×540 而不是 1080×720；
  这是「固定物理尺寸变 DPI」的正确语义，但意味着**门脚本跑在更小的屏幕上时 css 会变**。
  门只在拍基线这台机器上跑，已记账；换机器跑门需重拍（与既有 ppp 基线同一约束）。
- 单栏抽屉的键盘路径在 CER-29 收口（Tab 环跳过关着的抽屉已钉：
  `tab_ring_one_pane_skips_a_closed_drawer`）。
