# CER-10 · 大纲搜索 / 筛选 / 聚焦

> 状态：`已实现`（证据齐；非门包，待 G-B2-1 人工复核）· 批次 B2-A · 依赖：CER-08（已完成）
> 设计合同：[`docs/28`](../../../docs/28-内容体验重构规范.md) §5.7（搜索/筛选/聚焦）、§5.3（搜索命中状态）
> 执行合同：[`docs/29`](../../../docs/29-内容体验重构工作包.md) §6（X65/X66/X67 范围并入）

## 1. 范围

把大纲筛选从「文本子串」扩成完整筛选条（条件 AND、祖先上下文、命中高亮、前后跳），
把筛选条挂到 `Ctrl+Shift+F`，复核聚焦模式；新增 `outline-search`、`outline-search-labels`
场景，`outline-focus` 逐字节复核。

## 2. §5.7 契约落地（2026-10-26 代码实测）

| §5.7 项 | 开工前 | 本包处置 |
| --- | --- | --- |
| `Ctrl+Shift+F` 打开筛选条 | ❌ `Ctrl+F`（大纲全屏时）直接开关搜索行 | **改**：新增 `shortcuts::Action::OutlineFilter`（`Ctrl+Shift+F`），仅大纲全屏时消费；`Ctrl+F` 归还全局搜索 |
| 筛选条 32px，页头下 | ✅ 40px 槽位内 32px 框 | 不动 |
| 条件：文本 / 任务 / 卡片 / 标签 AND | ❌ 只有文本子串 | **补**：筛选条右侧三枚条件胶囊——任务（循环 待办→进行中→完成→不限）、卡片（开关）、标签（弹层选标签）；与文本 AND |
| 祖先上下文 48% 灰 | ❌ `visible` 只含命中行 | **补**：`prepare_filter` 把命中行 + 祖先链放进 `visible`，非命中的可见行整行 `set_opacity(0.48)` |
| 命中字符 12% 强调底 | ❌ | **补**：`hit_ranges` 记每行命中区间（字符下标），绘制前按同一字体量前缀/命中宽，画 12% 强调底（alpha 30） |
| 计数 `3/17` | ✅ | 含义不变（命中 / 视图总行）；有条件时才显示 |
| `Enter` 下一个，`Shift+Enter` 上一个 | ❌ 输入框持焦时未接 | **补**：`filter_owned` 分支接 Enter/Shift+Enter → `step_match`（只走 `hits`，到头回绕），焦点回筛选框 |
| `Esc` 清空 | ✅ 逐级链 | Esc 现在清**全部条件**（文本/任务/卡片/标签）再逐级 |
| 折叠状态筛选期间保持不变 | ⚠️ 行表不含折叠子树，折叠分支里的命中永远搜不到 | **补**：筛选态重建行表时忽略折叠（`rows_all`），命中穿透折叠显示；文档折叠位不动（单测断言）。清空后下一次 `sync` 自动回到正常行表 |
| 清空筛选恢复 | ✅ | 不动 |
| 聚焦模式 | ✅ 面包屑/逐级 Esc/聚焦根守卫已有 | `outline-focus` 复核逐字节不变 |

## 3. 实现清单

1. `crates/mw-shell/src/panels/outline.rs`
   - 状态：`filter_task: Option<TaskState>`、`filter_card: bool`、`filter_label: Option<String>`、
     `hits: Vec<usize>`、`hit_set: HashSet<usize>`、`hit_ranges: HashMap<usize,(usize,usize)>`、
     `rows_all: bool`、`label_options: Vec<String>`、`label_popup: bool`。
   - `filter_active()` / `clear_filter()` / `prepare_filter(&Document)`（可见表缓存键 =
     文本 + 三个条件的组合串）；`sync` 在「筛选态 ↔ 正常态」切换时强制重建行表。
   - `rebuild`：`rows_all` 时穿透折叠（树与概念图两条分支都接）。
   - 绘制：祖先 48%、命中 12% 底（省略号截断时夹到行宽内）；`filter_chip` 纯函数画三枚胶囊。
   - 键盘：Enter/Shift+Enter 前后跳（trace `outline_filter_step`）；Esc 链改用 `filter_active`。
2. `crates/mw-shell/src/shortcuts.rs`：`Action::OutlineFilter`（`Ctrl+Shift+F`）。
3. `crates/mw-shell/src/app_methods_1.rs`：快捷键接线；`MW_OUTLINE_FILTER_TASK/_CARD/_LABEL`、
   `MW_OUTLINE_LABEL_POPUP` 实拍钩子。
4. `crates/mw-shell/src/app_methods_2.rs`：页头搜索按钮 tooltip 改 `Ctrl+Shift+F`；
   `outline_screen` 每帧调 `prepare_filter`。
5. `crates/mw-shell/src/panels/help.rs`：大纲键位表改 `Ctrl+Shift+F`，并把过期的
   `Ctrl+Enter=新建子级` 换成现行语义（`Ctrl+]/[`、`Alt+.`）。

## 4. 非目标

- 不改全局搜索与命令面板；不做筛选历史/正则/跨块高亮。
- 不改侧栏 232px 大纲面板；筛选是纯显示态，不落盘、不发 Command。
- 条件胶囊暂无键盘路径（Tab 进不了胶囊），属键盘可达性缺口（见 §7）。

## 5. 验证

### 5.1 单测（`cargo test --workspace -- --test-threads=1` → **817 passed / 0 failed**，+2）

- `filter_conditions_and_ancestor_context_with_hit_ranges`：文本 AND 任务/标签、
  祖先上下文（命中 1 + 祖先 2）、命中区间 `(0,2)`、组合归零、**折叠穿透且文档折叠位不变**。
- `filter_steps_only_through_hits_and_wraps`：Enter/Shift+Enter 只走命中行、回绕、
  从祖先出发落在第一个命中。
- `twenty_thousand_rows_rebuild_once_and_filter_once`：改判口径——命中 100、
  可见 101（含根祖先）；换词重扫、200 帧复用不变。

### 5.2 视觉（ppp 1.5，`--tol 0`）

| 场景 | 结果 |
| --- | --- |
| `outline-search`（新增） | `1544a2aac7e49a1fab55b80d030466d0afa506b1a60e64cab8e2ff2513639dbc`——文本「排版」+ 标签「验收」AND：命中「中文排版」（`排版` 12% 底）、祖先根 48%、标签胶囊激活、计数 `1 / 13` |
| `outline-search-labels`（新增） | `616e6a7f53a7444e5e42bd4e651594ddba97f6e4a2f15b9bb9b624f27bd3c71f`——标签弹层展开（全部标签 / 验收） |
| 逐字节复核 | `outline`、`outline-focus`、`outline-compact`、`outline-narrow`、`outline-edit` 全部 0 差异 |

两个新场景各连跑两次 `--tol 0` 均 0 差异；已加入 `scripts/x100-stage-b-gate.ps1`
（41 → 43 组比较）。

### 5.3 工程

- `cargo fmt --all -- --check` 干净；`cargo clippy -p mw-shell --all-targets` 仅
  `export_pdf.rs` 3 条历史，新增 0；`cargo build --release -p mw-shell` 通过。
- 机器门：`powershell -File scripts/x100-stage-b-gate.ps1` → `MACHINE GATE PASS`，
  **43 组 × 2 遍 = 86/86 全 0 差异**，817 tests，finished 2026-10-26 12:54:29 +08:00
  （`target/x100/stage-b-gate-cer10.log`）。

## 6. 变更

- 代码：`panels/outline.rs`（筛选模型/绘制/键盘）、`shortcuts.rs`（新动作）、
  `app_methods_1.rs`（快捷键 + 实拍钩子）、`app_methods_2.rs`（tooltip + prepare）、
  `panels/help.rs`（键位表）。
- golden：新增 `shots/golden/outline-search/`、`shots/golden/outline-search-labels/`；
  `scripts/x100-stage-b-gate.ps1` +2 场景。

## 7. 未做 / 遗留

- 条件胶囊没有键盘路径（Tab 巡游到不了 22px 热区）；标签弹层只能鼠标操作。
- 任务胶囊是「循环到具体状态」，不是三态同时多选。
- 命中计数 `1 / 13` 的分母是「视图总行数」，聚焦模式下等于聚焦子树行数。
- 筛选态行表包含折叠分支（穿透），意味着筛选期间行数翻倍上限；20k 测试里
  筛选扫描与无筛选同量级，未见额外回归。

## 8. 风险

- 高亮区间按 `to_lowercase` 的字节位置映射字符下标；对常见 CJK/拉丁正确，
  个别大小写映射改变字符数的 Unicode（如 `İ`）可能偏移 1 字符，不影响正确性只影响底色起点。
- `Ctrl+F` 行为变化：大纲全屏时不再开大纲筛选，改开全局搜索；速查表与页头
  tooltip 已同步。

## 9. 人工判词

非门包，不填；G-B2-1 一并复核。
