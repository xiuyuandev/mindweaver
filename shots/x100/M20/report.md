# M20 · 搜索跳进折叠子树（打磨轮 43）

- 日期：2026-10-31
- 依据：`docs/02` §4「查找」——结果要能定位；`docs/06` 折叠主题的语义；
  M19 / C17 已把检索做全，但「跳过去」这一步在折叠子树上静默失败。
- 状态：**已实现**（机器门绿；人工门未开，不记 `已验证`）

## 1. 现状（代码复核，先于改动）

`Document::search` 遍历 `doc_order()`，**折叠子树里的节点照样进结果**。
但画布的场景只排可见节点：`layout.rs` 在 `n.collapsed` 时跳过子树，
`Scene` 里根本没有这些节点（`scene.rs` 的 `collapsed_children_leave_the_scene_but_keep_the_badge`）。

搜索结果跳转走 `Canvas::select_node_keep_focus`（`canvas_methods_1.rs` 的
「搜索结果跳转：选节点、对相机」那条注释下面）：

```text
select_node_keep_focus(id) → select_node(id) → scene.find(id)?  ← 找不到就直接 return
```

于是：搜索命中「中文排版」下的「禁则换行」，按 `Enter` 时
`scene.find` 返回 `None`，函数**一声不响地退出**——结果行高亮了、相机不动、
什么都没选中，也没有 toast 或轨迹。用户会以为「搜索坏了」。

改前实测（`target/x100/M20/before/trace.txt`，夹具 `MW_COLLAPSE_DEMO=中文排版`）：

```text
event search_hits q=禁则换行 rows=1 note=0 link=0 rel=0
（之后 8 帧之内没有任何跳转事件）
```

## 2. 范围

1. **模型层查折叠祖先**（`mw-core::document`）：
   `Document::collapsed_ancestors(id) -> Vec<String>`，从直接父级往上，
   只收 `collapsed == true` 的祖先；节点不存在返回空。
2. **纯函数生成展开命令**（`mw-shell::canvas`）：
   `reveal_commands(doc, id) -> Vec<Command>`，把祖先按**根 → 近**顺序展开成
   `SetCollapsed { collapsed: false }`，交给一条 `Command::Batch`（一次撤销全收回）。
3. **跳转先展开再选中**：
   `select_node_keep_focus` 改为返回 `bool`；`scene.find` 失败时先调用
   `reveal_commands` → `apply_command(Batch)` → 重新 `find`，成功才选中 + glow；
   轨迹写 `search_reveal id=… expanded=N`。展开后仍找不到（例如聚焦模式把节点挡在
   `layout_root` 之外）返回 `false`，由壳层给 toast，**不自动退出聚焦**。
4. **跳转轨迹**：`SA::Jump` 写 `search_jump id=… ok=1/0`，让「点/回车有没有真的跳过去」
   有据可查；失败时 toast「这个主题在当前视图之外」。
5. **夹具**：`MW_COLLAPSE_DEMO=<节点文字>` 启动时用 `Command::SetCollapsed`
   折叠指定节点（默认样例里选「中文排版」），给金样一个确定的折叠子树。
6. **金样**：
   - `search-collapsed`：`MW_COLLAPSE_DEMO=中文排版`，`Ctrl+F` → `text 禁则换行` →
     `Enter`；画布展开「中文排版」并选中「禁则换行」，搜索卡片仍开着。
   - 改前同脚本对照：结果行在、画布没有任何变化（存 `target/x100/M20/before/`）。

落地位置：`crates/mw-core/src/document.rs:600`（`collapsed_ancestors`）、
`crates/mw-shell/src/canvas.rs:1470`（`reveal_commands` / `in_layout_subtree`）、
`crates/mw-shell/src/canvas/canvas_methods_1.rs:566`（`select_node_keep_focus -> bool`）、
`crates/mw-shell/src/app_methods_2.rs:351`（`search_jump` 轨迹 + 失败提示）、
`crates/mw-shell/src/app_methods_1.rs:843`（夹具）、
`scripts/x100-stage-b-gate.ps1:338`（新场景）。

## 3. 非目标

- 不自动退出聚焦模式：聚焦是用户显式进入的视图状态，搜索不能替用户做主。
- 不改搜索结果的命中集合、排序、分组与角标；不把折叠隐藏的节点从结果里去掉。
- 不改 `Node.collapsed` 语义与布局；展开只走既有 `SetCollapsed` 命令与撤销栈。
- 不做「搜索结果自动滚动到可见」——当前结果窗口已经跟着 `current` 移动（M19 既有）。
- 不改 `.zmind` / `Node` 字段，不做迁移。

## 4. 验收条款

### 4.1 纯函数（`mw-core` / `mw-shell`）

| # | 用例 | 期望 |
| --- | --- | --- |
| 1 | 父级折叠 | `collapsed_ancestors(child)` 返回 `[父]` |
| 2 | 祖父 + 父都折叠 | 返回近→远两条；`reveal_commands` 反成根→近 |
| 3 | 链上无人折叠 | 返回空，`reveal_commands` 为空 |
| 4 | 节点不存在 | 返回空，不 panic |
| 5 | 展开命令走 `Batch` | 应用后祖先全展开、子树节点回到 `Scene`；一次 undo 全收回 |
| 6 | 可达性只认布局子树 | 折叠不影响「在 `layout_root` 之下」的判定；聚焦到祖先时根节点算在外 |

### 4.2 端到端（轨迹 + 金样）

- 改前：`search_hits q=禁则换行 rows=1` 但无选中；`search_jump ok=0`（改前无轨迹，
  由改后代码在旧逻辑上跑一次对照记录）。
- 改后：`search_hits … rows=1` → `search_reveal … expanded=1` → `search_jump … ok=1`；
  画布出现「中文排版」子树与「禁则换行」选中环，搜索框仍是焦点。
- 新基线 `search-collapsed` 进 `stage-b` 门，`--tol 0` 双跑。
- 改前 / 改后同脚本像素差：搜索卡片区域必须 **0 px**；标题栏只有撤销按钮会从灰变亮；
  其余大块差异来自相机居中（跳转本来就要把目标带到眼前），不是样式漂移。

### 4.3 既有基线零漂移

- 只新增一条跳转前的展开路径与两条轨迹；所有既有场景都不搜折叠子树。
  `stage-b` 123 场 / `stage-cm` 58 场应零漂移。

## 5. 证据（实测）

| 类型 | 内容 |
| --- | --- |
| 单测 | 6 条：`mw-core` `collapsed_ancestors_are_listed_near_to_far` / `…_skip_expanded_and_self` / `…_is_empty_for_missing_node`；`mw-shell` `reveal_commands_open_collapsed_ancestors_root_to_near_in_one_batch` / `reveal_commands_skip_visible_and_missing_nodes` / `layout_subtree_membership_ignores_collapse_but_not_focus` |
| 轨迹 | `shots/x100/M20/trace.txt`：改前只有 `search_hits … rows=1`；改后多出 `search_reveal … expanded=1` 与 `search_jump … ok=1`，焦点仍是 `search-input` |
| 金样 | `shots/golden/search-collapsed`（sha256-12 `37AF90C2C865`），`--tol 0` 双跑进 `stage-b` |
| 像素 | `shots/x100/M20/diff.txt`：改前 vs 改后 252 082 px（画布整体平移 + 子树展开 + 选中环）；搜索卡片区域 0 px，标题栏 132 px（撤销按钮亮起） |
| 门 | stage-b **124 场 / 248 次比较** 零差异（`07:14:21–07:20:56`）；stage-cm **58 场 / 116 次比较** 零差异（`07:07:48–07:12:31`）；fmt 干净、clippy 指纹 36/36 新增 0 |
| 全量测试 | **1058 passed / 0 failed / 1 ignored**（23 个套件；mw-core 273+3=276 + contrast 7；mw-shell --bins 598） |

## 6. 未做 / 风险

- 聚焦模式下命中的节点仍不可达：`in_layout_subtree` 先判定目标在布局根之下（**不看折叠**），
  不在就直接返回 `false`、一条命令都不发；在布局根下但展开后仍找不到时也返回 `false`，
  壳层 toast「这个主题在当前视图之外」；**不自动退出聚焦**（聚焦是用户显式进入的状态）。
- 跳转展开会入撤销栈：用户按 `Ctrl+Z` 会把这次跳转的展开收回（一次撤销收回全部祖先，
  选中会按 `restore_selection` 的回退规则落回根或已消失节点之外）。
- 搜索跳转带来的相机居中是既有行为（`select_node`），本次没有改；因此改前/改后像素差
  的大头是画布整体平移，不是新样式。
- 实机 IME / 键盘手感未核（与 M4 同缺口）。
- 失败提示只在 `SA::Jump` 一处；结果行的鼠标悬停/键盘上下选中不触发跳转，不涉及。
