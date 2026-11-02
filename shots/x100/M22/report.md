# M22 · 聚焦视图外的搜索跳转（打磨轮 45）

- 日期：2026-10-31
- 依据：`docs/02` §4「查找」——结果要能定位；§3「聚焦」——聚焦只影响视图，不改数据；
  M20 已让搜索跳进折叠子树，但**聚焦视图之外**的命中仍然是一条死路（M20 报告 §6 记的未做）。
- 状态：**已实现**（机器门绿；人工门未开，不记 `已验证`）

## 1. 现状（代码复核，先于改动）

`Canvas::select_node_keep_focus`（`canvas_methods_1.rs` 的「搜索结果跳转」注释下）在
`scene.find(id)` 失败时先判定 `in_layout_subtree(doc, layout_root, id)`：

- 折叠子树里的节点：在布局根之下 → `reveal_commands` 展开祖先（M20）；
- **聚焦模式**：`layout_root()` 是聚焦根，目标在它之外 → 直接 `return false`，
  一条命令都不发；壳层 toast「这个主题在当前视图之外」。

于是用户在聚焦视图里 `Ctrl+F` 搜到全图任意主题，结果行列得出来、按 `Enter` 却
什么也不发生（只多一条 toast）。`docs/02` §4 的口径是「结果要能定位到命中处」，
而聚焦只是**用户自己进入的视图状态**，不是数据边界——在结果上按回车就是
「我要去这个主题」，这时继续留在聚焦视图里反而无法满足请求。

改前实测（聚焦夹具 + `Ctrl+F → wgpu → Enter`）：

```text
event search_hits q=wgpu rows=1 note=0 link=0 rel=0
event search_jump id=<wgpu> ok=0
toast 这个主题在当前视图之外
（相机与画布一动不动）
```

## 2. 范围

1. **跳转前退出全部聚焦**（`mw-shell::canvas`）：
   `select_node_keep_focus` 在目标不属于当前 `layout_root` 的布局子树、
   且 `focus` 非空、且目标确实是文档里的节点时，先 `focus_to(0)` 退出全部聚焦层级
   （等价于连续 Esc / 点面包屑回到主根），再按既有逻辑展开折叠祖先、选中、居中、glow。
   退出层级数写轨迹 `search_focus_exit id=… levels=N`。
2. **壳层说明这次视图变化**：`SA::Jump` 在跳转成功且聚焦层级被清空时
   toast「已退出聚焦并跳转」，不再留给用户「画布怎么突然全了」的疑问。
3. **夹具**：`MW_FOCUS_DEMO=<节点文字>` 启动时选中并 `focus_selected()` 该节点
   （默认样例里用「中文排版」），给金样一个确定的聚焦视图。
4. **金样**：`search-focus-jump`——聚焦「中文排版」→ `Ctrl+F` → `text wgpu` → `Enter`；
   画布退出聚焦、回到全图、选中「wgpu 四层合成器」并居中；搜索卡片仍开着。
   改前同脚本对照存 `target/x100/M22/before/`。

落地位置：`crates/mw-shell/src/canvas/canvas_methods_1.rs`（`select_node_keep_focus`）、
`crates/mw-shell/src/app_methods_2.rs`（`SA::Jump` 的成功 toast）、
`crates/mw-shell/src/app_methods_1.rs`（夹具）、`scripts/x100-stage-b-gate.ps1`（新场景）。

## 3. 非目标

- 不把聚焦根改成目标的祖先：聚焦是用户显式进入的视图，跳转只退出、不重新取景到别的子树。
- 不改搜索结果的命中集合、排序、分组与角标；聚焦视图外的命中本来就在结果里。
- 不撤销「点结果不抢键盘」的纪律：跳转后搜索框仍握着键盘，列表明暗与当前行不变。
- 不改 `Node.collapsed` / 聚焦语义 / `.zmind` 字段。
- 不做跨标签页搜索：跳转只作用于当前文档。

## 4. 验收条款

### 4.1 单元（`mw-shell` canvas）

| # | 用例 | 期望 |
| --- | --- | --- |
| 1 | 聚焦子树内命中 | 不触发 `search_focus_exit`；选中且 `focus` 层级不变 |
| 2 | 聚焦子树外的命中 | 退出全部聚焦（`focus_path()` 为空）、目标选中、返回 `true` |
| 3 | 无聚焦且目标不在场景 | 返回 `false`，不 panic、不发命令 |
| 4 | 目标在聚焦子树之外但折叠 | 先退出聚焦、再展开挡路祖先（一条 `Batch`）再选中 |
| 5 | 聚焦层级 ≥2 | 一次退到主根，`levels` 等于原层级数 |

### 4.2 端到端（轨迹 + 金样）

- 改前：`search_jump ok=0` + toast「这个主题在当前视图之外」；画布不动。
- 改后：`search_focus_exit … levels=1` → `search_jump … ok=1`；
  画布回到全图、`wgpu 四层合成器` 选中环 + 居中；toast「已退出聚焦并跳转」。
- 新基线 `search-focus-jump` 进 `stage-b` 门，`--tol 0` 双跑。

### 4.3 既有基线零漂移

- 只动「聚焦视图外命中」这一条分支；既有搜索场景都没有聚焦。
  `stage-b` / `stage-cm` 应零漂移。

## 5. 证据（实测）

| 类型 | 内容 |
| --- | --- |
| 单测 | `mw-shell` `focus_exit_levels_only_fires_for_targets_outside_the_focused_subtree`（6 组断言：无聚焦 / 视图外 / 主根 / 聚焦子树内 / 节点不存在 / 多层聚焦）；测试 1065 → **1066** |
| 轨迹 | 改前 `shots/x100/M22/before-trace.txt`：`search_jump … ok=0` + `toast bad=true 这个主题在当前视图之外`；改后 `shots/x100/M22/trace.txt`：`search_focus_exit … levels=1` → `search_jump … ok=1` → `toast bad=false 已退出聚焦并跳转` |
| 金样 | `shots/golden/search-focus-jump`（sha256-12 `D271AB2E4183`），`--tol 0` 双跑无差异；进 `stage-b` 门 |
| 像素 | `shots/x100/M22/diff.txt`：改前 vs 改后 **189 421 / 2 304 000 px（8.2213%）**；包围盒 x8..1918 y56..1194。搜索卡片 `x600..1320 y45..225` **0 px**；差异分三块——标题栏左侧面包屑（5 156 px）、画布聚焦子树 → 全图（约 172 000 px）、左下角提示条（11 390 px） |
| 门 | stage-b **127 场 / 254 次比较** 零差异（`09:14:32–09:21:18`）；stage-cm **58 场 / 116 次比较** 零差异（`09:09:47–09:14:25`）；fmt 干净、clippy 无新增；全量测试 `tests passed=1066 failed=0` |
| 目录 | 基线 187 → **188** 个；输入脚本 124 → **125** 个 |

## 6. 未做 / 风险

- `Ctrl+Z` 不把聚焦状态还回来：聚焦是视图态、本来就不入撤销栈；
  这条只保证「跳转能到」，不承诺「撤销回到聚焦视图」。
- 跳转展开仍会入撤销栈（M20 遗留）：`Ctrl+Z` 会把祖先收回。
- 目标在文档里但既不在主根子树也不在任何聚焦子树（例如概念图浮动概念且
  `scene.find` 失败）时仍返回 `false`，保持既有 toast。
- 实机 IME / 键盘手感未核（与 M4 同缺口）。
