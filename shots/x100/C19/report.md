# 打磨轮 54 · C19 概念图检查器属性（概念元信息、钉住开关与局部整理入口）

> 建立：2026-10-31 · 适用：织思 X100 Stage CM 打磨轮 54
> 目标：对齐 `docs/30` §4.2 / §5.5，补齐概念图在检查器面板中的属性完备性，消除树语义残留，提供钉住开关与排布操作入口。

---

## 1. 范围与非目标

### 在范围内
1. **对象行副行与元信息识别**：
   - 当文档为概念图（`doc.is_concept_map()`）时，单选副行识别为「概念」，不再显示「中心主题 / 一级主题 / 分支主题」；
   - 单选副行如已钉住显示「已钉住」；统计进出命题总数显示「N 条命题」；挂载卡片显示「已挂卡片」；标签显示「N 个标签」；屏蔽无树层级意义的「N 个子主题」；
   - 多选概念副行识别为「N 个概念」，并展示「M 个已钉住」或「全部已钉住」。
2. **隐藏概念图无意义的「含子树」作用域**：
   - 概念图不存在父子树层级，「颜色」与「形状与文字」分组标题右侧隐藏「本节点 / 含子树」切换，正常显示摘要文字（如「填充 · 分支 · 文字」）；
   - 隐藏「文字样式应用到子树」树结构开关。
3. **概念排布分组与钉住位置开关**：
   - 概念图样式页新增「概念排布」分组（键 `concept/layout`，默认展开，带活摘要「已钉住 / 自动重排」）；
   - 提供 28px 高度标准的 `switch_row`「钉住位置」轨道开关与辅助说明；
   - 单选点击开关下发 `Command::SetConceptPinned`；多选点击批量下发 `Command::Batch`；
   - 组内提供「整理选中概念」/「整理全部布局」快捷操作按钮，直通画布整理引擎。
4. **单元测试与金样基线**：
   - 覆盖对象行元信息生成、钉住状态展示、开关下发命令与分组开合；
   - 新增两场概念图检查器基线 `cm-inspector` 与 `cm-inspector-pinned` 并纳入 `stage-cm` 门。

### 非目标
- 不动 `.zmind` 格式版本与持久化模型（钉住状态仍为内存交互态）；
- 不改思维导图模式下的检查器原有行为与基线；
- 不改 `mw_render::Frame` ABI。

---

## 2. 验收场景与证据规划

| # | 场景 | 验收要点 | 状态 | 证据 |
|---|---|---|---|---|
| 1 | 单选概念元信息 | 副行显示「概念 · N 条命题」，钉住时前置「已钉住」 | 达标 | 单测 `object_meta_for_concept_map_shows_concept_and_propositions_and_pinned` |
| 2 | 多选概念元信息 | 副行显示「N 个概念」，全部钉住显示「全部已钉住」 | 达标 | 单测 `object_meta_for_concept_map_multiselect_shows_concept_count_and_pinned` |
| 3 | 钉住开关交互 | 点击「钉住位置」开关立即更新状态并落单条命令，支持一次 Ctrl+Z | 达标 | 单测 `concept_layout_pinned_command_and_undo` + trace |
| 4 | 多选批量钉住 | 批量切换所有选中概念钉住状态，合成一条 Batch 命令 | 达标 | 单测 + 交互代码验证 |
| 5 | 金样锁画 | `cm-inspector`（未钉住）与 `cm-inspector-pinned`（已钉住）进 `stage-cm` 门 | 达标 | golden 0 容差双跑（两基线差 4 659 px） |

---

## 3. 实现细节

| 文件 | 改动 |
|---|---|
| `crates/mw-core/src/document.rs` | 新增 `is_concept_pinned(&self, id: &str) -> bool` 与 `concept_proposition_count(&self, id: &str) -> usize`（进出命题总计，自环不双计）。修复 line 1362 clippy `unneeded late initialization`。新增 1 条单测。 |
| `crates/mw-shell/src/panels/inspector.rs` | `Action` 枚举新增 `ArrangeConcepts` 与 `ArrangeSelectedConcepts`。改造 `object_meta`：概念图判定单选为「概念」，前置「已钉住」，追加「N 条命题」、「已挂卡片」、「N 个标签」，屏蔽子主题计数。新增 4 条单测覆盖元信息、多选元信息、子树隐藏与命令撤销。 |
| `crates/mw-shell/src/panels/inspector/inspector_methods_1.rs` | 多选对象行在概念图下展示「N 个概念」与钉住统计。`style_body` 接收 `doc: &Document`。概念图隐藏分组标题右侧的「本节点 / 含子树」切换并屏蔽「文字样式应用到子树」。新增「概念排布」分组（键 `concept/layout`，默认开，活摘要），承载 28px `switch_row`「钉住位置」与说明，以及「整理选中概念」/「整理全部布局」按钮。 |
| `crates/mw-shell/src/main.rs` | 接入 `Action::ArrangeConcepts` 与 `Action::ArrangeSelectedConcepts` 到 `canvas.arrange_concepts()` / `canvas.arrange_selected_concepts()`，并显示相应 toast 提示。 |
| `scripts/clippy-baseline.txt` | 36 → 35 条（修复 1 处既有 late init，新增 0）。 |
| `scripts/x100-stage-cm-gate.ps1` | 新增 `cm-inspector` 与 `cm-inspector-pinned` 两场金样，场景数 60 → 62。保持 UTF-8 BOM。 |
| `shots/golden/cm-inspector` | 新增基线（194,030 字节，sha256_12: `DDEDFEFA1445`）。 |
| `shots/golden/cm-inspector-pinned` | 新增基线（192,943 字节，sha256_12: `B19304716E8F`）。 |

---

## 4. 证据矩阵

1. **单元测试**：
   - `cargo test --workspace -- --test-threads=1`（`$env:TEMP = "$PWD\target\tmp"`）：**1114 passed; 0 failed; 1 ignored**（单测净增 5 条：core 1 条 + shell 4 条）。
2. **代码规范与 Clippy**：
   - `cargo fmt --all -- --check`：PASS（0 差异）。
   - `cargo clippy --workspace --all-targets`：PASS（35 条基线，新增 0 条）。
3. **金样与机器门**：
   - `scripts/x100-stage-cm-gate.ps1`：**PASS**（62 场 × 2 遍 = 124 次比较，0 容差 0 漂移）。
   - `scripts/x100-stage-b-gate.ps1`：**PASS**（140 场 × 2 遍 = 280 次比较，0 容差 0 漂移）。
   - 金样目录数：203 → **205**。
   - `cm-inspector` ↔ `cm-inspector-pinned` 差量：精确相差 4,659 像素（画布钉住徽标 1,632 px；检查器对象行元信息、分组活摘要、轨道开关高亮 3,027 px）。

---

## 5. 未做与已知风险

1. **钉住状态未持久化**：
   - 钉住状态目前仅保存在内存 `Document.pinned_concepts` 集合中，不写入 `.zmind`，重开文件后重置为未钉住（保持格式兼容）。
2. **实机键鼠触控未核**：
   - 依赖无头金样与脚本模拟验证，实机高 DPI 与触控板手感尚未人工核对。

---

## 6. 状态结论

**已实现**（机器门全绿；人工门未开，不记已验证）。
