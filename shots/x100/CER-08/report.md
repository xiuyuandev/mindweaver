# CER-08 · 大纲结构命令与键盘

> 状态：已实现 · 开始 2026-10-25 · 批次 B2-A · 依赖：CER-07（已完成）
> 设计合同：[`docs/28`](../../../docs/28-内容体验重构规范.md) §5.5（结构命令与键盘）
> 执行合同：[`docs/29`](../../../docs/29-内容体验重构工作包.md) §6（X63/X69 范围并入）

## 1. 范围

对照 §5.5 结构命令表逐项审计 X20 键盘映射（`row_key` 纯函数），补齐缺口；
全部结构操作走既有 `Command`/`Action`；键盘 trace + 撤销/重做回归。

## 2. §5.5 契约审计（2026-10-25，`row_key` 代码实测）

| §5.5 键 | 现状 | 处置 |
| --- | --- | --- |
| Enter 新建同级 / 拆分 | ✅ `AddSibling` / 编辑态拆分 | 不动 |
| Tab 缩进（含子树） | ✅ `Indent` | 不动 |
| Shift+Tab 提升（根停止） | ✅ `Outdent` | 不动 |
| **Ctrl+Enter 切换任务** | ❌ 现为 `AddChild`（docs/06 旧语义，§5.5 明文作废） | **改**：`(true,false,Enter) => Task`；原 `Ctrl+T` 任务键保留为额外路径 |
| **Alt+. 折叠/展开当前** | ❌ 无（Alt 只认领 Alt+Enter=聚焦） | **补**：`Alt+Period => Toggle` |
| **Alt+Shift+. 全部折叠/展开** | ❌ 无 | **补**：`Alt+Shift+Period => 全部切换`（按当前根状态取反） |
| **Ctrl+] 进入当前节点（聚焦）** | ❌（聚焦只有 Alt+Enter） | **补**：`Ctrl+] => Focus` |
| **Ctrl+[ 返回上一级** | ❌ | **补**：`Ctrl+[ => Unfocus`（非聚焦态不认领） |
| Ctrl+Shift+↑/↓ 移动（含子树） | ✅ `MoveUp/MoveDown` | 不动 |
| F2 编辑 | ✅ `Rename` | 不动 |
| Delete 删除行/子树 | ✅ `Delete` | 不动 |
| ↑/↓ 移动选中行 | ✅（非编辑态） | 不动 |
| **↑/↓ 编辑态移动光标** | ❌ 现为"提交并走行"（X20 语义；多行编辑落地后与 §5.4 软换行冲突：草稿含 `\n` 时提交会整段写入） | **改**：编辑态不认领 ↑↓，交给 multiline TextEdit 移动光标 |
| **Shift+↑/↓ 扩展多选** | ❌ 无 | **补**：以 `selection_anchor` 为锚做可见区间选择（`SetSelection`，primary=移动端） |
| **Ctrl+A 全选当前可见行** | ❌（大纲未认领 Ctrl+A；不跨折叠） | **补**：可见行全选（跳过根标题行），primary=末行 |
| Esc 逐级退出 | ✅ `esc_step`（清词→关搜索→退聚焦→退出） | 不动 |
| F8 进出大纲 | ✅ 全局 | 不动 |

补充说明：`Ctrl+T`（X20 任务键）保留为 §5.5 之外的额外路径；`←/→` 折叠/展开、
`Space` 展开为 X20 增强保留。

## 3. 实现清单

1. `row_key`：新增 `Alt+Period` / `Alt+Shift+Period` / `Ctrl+]` / `Ctrl+[` /
   `Shift+↑` / `Shift+↓` / `Ctrl+A` 七条映射；`Ctrl+Enter` 从 `AddChild` 改为
   `Task`；新增 `RowKey::ExpandAllToggle` / `FocusHere` / `UnfocusHere` /
   `SelectNext` / `SelectPrev` / `SelectAllVisible` 变体。
2. 编辑态：↑↓ 不再消费/提交（删除 `up || down` 提交分支）。
3. 处理器：折叠/展开与聚焦已有 Action（`Toggle`/`Focus`/`Unfocus`/
   `SetCollapsedAll`）；多选区间用 `range_between` + `SetSelection`；
   `Ctrl+A` 用 `visible_ids` 全选。
4. 单测：`row_key` 全映射表（每条 §5.5 键一条断言）。
5. 证据：`MW_TRACE` 键盘脚本（`outline-keys.txt`：Tab/Shift+Tab/Ctrl+Enter/
   Alt+./Ctrl+]/[ 走一遍）；`outline-focus` 重拍核对；撤销/重做回归由
   机器门全量测试覆盖。

## 4. 非目标

- 不做拖拽（CER-09）、搜索筛选（CER-10）、虚拟化（CER-11）。
- 不改 `Command` 语义；不新增持久化字段。
- Delete 有子树的二次确认保持 P1 不做（§5.5 允许）。

## 5. 变更 / 6. 验证 / 7. 未做

| 文件 | 变更 | 动外观 |
| --- | --- | --- |
| `crates/mw-shell/src/panels/outline.rs` | §5.5 键盘映射补齐：`Alt+.`（折叠/展开）、`Alt+Shift+.`（全部折叠/展开取反）、`Ctrl+]`（聚焦）、`Ctrl+[`（返回上一级）、`Shift+↑/↓`（多选扩展——面板级链既有逻辑同一语义）、`Ctrl+A`（全选可见行）；**`Ctrl+Enter` 语义改为切换任务完成**（旧加子级作废，§5.5 明文）；**编辑态 ↑/↓ 让位给 TextEdit 移动光标**（删除 X20 的"提交并走行"分支，`edit_next` 清理）；死代码清理（`RowKey::AddChild` 变体与处理臂）；**修复编辑态键链结构损伤**（CER-07 多轮脚本拼接把软换行光标推进块错放进 Enter 拆分分支、行内粘贴残块留下野生括号——整体重写 `empty→enter→esc` 链恢复正确归属）；+1 全映射单测、2 个旧断言更新为 §5.5 语义 | **是（键盘行为；静态帧不变）** |

## 6. 验证

- **行为**：784 tests / 0 failed。`row_key_maps_the_docs28_5_5_table` 25 条断言逐键钉住
  §5.5 全表（含"未定义组合不认领"）；ajax 旧测试
  `outline_keys_are_exact_and_shift_tab_outdents` 的 Ctrl+Enter/Shift+↑ 断言更新为新语义。
- **trace**（`outline-keys.txt`，`MW_TRACE`）：ArrowDown → Ctrl+Enter
  `outline_set_task task=Some(Todo)` → Tab/Shift+Tab（Outdent/Indent）→ Alt+.
  → **Ctrl+] `outline_focus id=`** → **Ctrl+[ `outline_unfocus old=Some(…)`**
  （探针期证实根行聚焦被 sync 清掉、改子行后切换成功）→ Alt+Shift+.
  `outline_collapse_all collapsed=true`。全链按 §5.5 表打点。
- **回归**：9 个 outline 场景（默认/聚焦/紧凑/窄窗/编辑/ime/软换行/粘贴/hover-select）
  双跑 `--tol 0` 全 0 差异（键链修复后 3 个脚本场景重拍并双跑）；
  机器门矩阵双跑（`target/x100/cer-08-golden.log`，其余 26 组逐字节不变）。
- **工程**：fmt 干净；clippy 触碰文件新增 0；release 通过。

## 7. 未做 / 已知偏差

- **Delete 有子树二次确认**未做（§5.5 允许 P1）。
- **编辑态方向键的"提交并走行"已退役**：退出路径 = Esc 还原/提交 或
  失焦；§5.5 的"↑↓ 移动选中行"只作用于非编辑态。
- 微软拼音人工实测待用户配合（`ime` 原语注入已覆盖脚本层）。
- ring 场景四主题待补拍（`wait 24` 模板）。

## 8. 风险

- `Ctrl+Enter` 语义变更影响 X20 用户习惯——§5.5 明文"旧组合作废"，且
  加子级路径（Tab 后输入 / Ctrl+T）仍在。
- 编辑态 ↑↓ 改为移动光标后，键盘走行只剩 Esc→退出编辑→↑↓ 一条路径；
  `outline-edit-softbreak` 场景需重拍（End+Shift+Enter 后光标行为不变）。

## 9. 人工判词

非门包，不填。
