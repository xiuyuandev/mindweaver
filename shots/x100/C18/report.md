# C18 · 概念图「钉住」概念与「只整理选中」布局（打磨轮 53）

- 日期：2026-10-31
- 依据：`HANDOFF.md` §7.5(5) 未做项「钉住 / 只整理选中」、`docs/30` §5.5「手工位置受保护」。
- 代码：
  - `crates/mw-core/src/concept_layout.rs`（`arrange_scoped`、`settle_against_fixed`、单测）；
  - `crates/mw-core/src/command.rs`（`Command::SetConceptPinned` 已有声明与标签）；
  - `crates/mw-core/src/model.rs`（`Node.pinned` 已有模型字段）；
  - `crates/mw-core/src/document.rs`（`SetConceptPinned` 命令落地与单测）；
  - `crates/mw-shell/src/canvas/canvas_methods_3.rs`（`is_concept_pinned`、`toggle_concept_pinned`、`toggle_concept_pinned_selected`、`selected_concept_ids`、`arrange_concepts` 接 `fixed`、`arrange_selected_concepts`、`paint_pinned_badges`）；
  - `crates/mw-shell/src/canvas/canvas_methods_2.rs`（绘制层接入 `paint_pinned_badges`）；
  - `crates/mw-shell/src/panels/context.rs`（`ToggleConceptPinned`、`ArrangeSelectedConcepts`）；
  - `crates/mw-shell/src/app_methods_2.rs`（右键菜单接入与动作落地、toast 提示）；
  - `crates/mw-shell/src/app_methods_1.rs`（`MW_CONCEPT_PIN` 验收钩子）。
- 状态：**已实现**（机器门绿；人工门未开，不记已验证）

## 1. 现状与问题

1. 概念图允许用户手工自由摆放概念节点（自由拖拽）。用户花时间排好了某些关键概念的位置，点击「整理布局」时，所有概念都会被统一重排，手工位置全部丢失。
2. `docs/30` §5.5 明确提出「手工位置受保护」——用户应能「钉住」特定概念，整理布局时钉住的概念保持原位，其余概念依其拓扑关系环绕排布并避让。
3. 当图中有多颗概念时，用户常常只想局部整理选中的几颗概念（「只整理选中」），而不动其他无关概念。
4. 模型层此前已备好 `Node.pinned: Option<bool>` 与 `Command::SetConceptPinned`，`concept_layout` 也写好了 `arrange_scoped` 纯几何算法，但缺少：
   - 核心层对 `arrange_scoped` 的完备单测；
   - `Document::apply_inner` 对 `SetConceptPinned` 的撤销/重做/拒绝边界单测；
   - 画布层 `arrange_concepts` 未传 `fixed` 数组（始终当全部可动）；
   - 画布层未提供 `arrange_selected_concepts` 接口；
   - 右键菜单缺少「钉住位置 / 取消钉住」与「只整理选中概念」选项；
   - 画布上钉住的概念没有任何视觉提示（不知道谁被钉住了）。

## 2. 范围

1. **核心算法验证**：
   - 为 `concept_layout::arrange_scoped` 补齐单测：
     - `fixed` 为空时与 `arrange` 结果逐字节一致；
     - `fixed` 全为 true 时所有概念位置原样保留；
     - 部分 `fixed` 时，钉住概念留在 `current` 原位，其余概念正常分层摆放并避让钉住盒子；
     - 溢出与不足长度容错。
2. **文档模型与撤销**：
   - 为 `Command::SetConceptPinned` 补齐单测：
     - 概念图上正常切换 `pinned: true/None`，一次撤销复原；
     - 树节点与非概念图文档严格拒绝（`Invalid`），不改文档，不入撤销栈；
     - 序列化 roundtrip 验证（`skip_serializing_if = "Option::is_none"`，未钉住时不输出冗余键）。
3. **壳层交互与菜单**：
   - `Canvas::is_concept_pinned(&self, id: &str) -> bool`；
   - `Canvas::toggle_concept_pinned(&mut self, id: &str) -> bool`，发出 `Command::SetConceptPinned` 并记录轨迹 `concept-pin id=... pinned=...`；
   - `Canvas::toggle_concept_pinned_selected(&mut self) -> Option<bool>`（多选批量切换）；
   - `Canvas::arrange_concepts(&mut self) -> Option<usize>`：按文档中的 `pinned` 属性构建 `fixed` 标记，传给 `arrange_scoped`；
   - `Canvas::arrange_selected_concepts(&mut self) -> Option<usize>`：
     - 只将选中的概念标记为可动（未选中的概念和已钉住的概念均标记为 `fixed`）；
     - 至少有 2 颗可动概念才执行，否则 toast 提示；
     - 记录轨迹 `concept-arrange-scoped n=... selected=... moved=...`。
   - 右键菜单（`app_methods_2.rs`）：
     - 单选概念：新增「钉住位置」（未钉时）或「取消钉住」（已钉时）；
     - 多选概念（>1）：新增「整理选中概念」、「整理全部布局」与「钉住位置 / 取消钉住位置」。
4. **视觉反馈**：
   - 在壳层覆盖层（不改 `mw_render::Frame` ABI），为处于 `pinned == Some(true)` 的概念节点右上角绘制一个小巧精致的固定圆点标记（5.5px 外圈圆，内嵌 2.5px 强调绿实心点）；
   - 具备 zorder 遮罩感知（`zorder::badge_visible`），不穿透浮层控件。
5. **验收与金样**：
   - 新增金样 `cm-pin`：展示钉住标记外观；
   - 新增金样 `cm-arrange-pinned`：展示整理布局时，钉住概念不动、其余概念环绕重排；
   - 更新 `cm-multi-menu` 与 `cm-submenu-bottom` 多选菜单基线；
   - 验证既有概念图金样零意外漂移。

## 3. 非目标

- 不支持思维导图树节点的钉住（思维导图是严格树布局，坐标由树结构计算推导，不保存任意手工绝对坐标）。
- 不动 `.zmind` 容器版本号（`FORMAT_VERSION` 保持不变）。
- 不动渲染层 `Frame` 结构体（钉住标记作为交互层装饰绘制在 egui 覆盖层）。

## 4. 验收条款

### 4.1 单元测试

1. `arrange_scoped` 空 fixed 逐字节等于 `arrange`；
2. `arrange_scoped` 全 fixed 零位移；
3. `arrange_scoped` 钉住概念坐标不变，且作为重心/邻居锚点参与排布；
4. `SetConceptPinned` 概念图上一步撤销/重做；
5. `SetConceptPinned` 在思维导图或非浮动概念上返回 `Err(Invalid)`。

### 4.2 端到端与金样

1. 单选概念点右键 → 出现「钉住位置」；点击后节点右上角显示钉住徽标，toast 提示「已钉住概念位置 · 整理时保持原位」；
2. 再次右键该概念 → 菜单显示「取消钉住位置」；点击后徽标消失，toast 提示「已取消钉住概念位置」；
3. 钉住某概念后点击「整理布局」→ 钉住的概念绝对坐标保持不变，其余概念重排并避让；
4. 多选概念右键 → 出现「整理选中概念」；点击后仅选中概念重排，其余概念不动。

## 5. 实现细节

| 文件 | 改动 |
| --- | --- |
| `crates/mw-core/src/concept_layout.rs` | 补充 `arrange_scoped` 4 组单元测试，覆盖空固定、全固定、单点钉住与切片越界容错。 |
| `crates/mw-core/src/document.rs` | 补充 `SetConceptPinned` 命令落地测试（撤销/重做）与对树节点及非概念图的拒绝判定测试。 |
| `crates/mw-shell/src/canvas/canvas_methods_3.rs` | 实现 `is_concept_pinned`、`toggle_concept_pinned`、`toggle_concept_pinned_selected`、`selected_concept_ids`；改造 `arrange_concepts` 注入 `fixed` 标记；实现 `arrange_selected_concepts`；实现 `paint_pinned_badges` 覆盖层绘制。 |
| `crates/mw-shell/src/canvas/canvas_methods_2.rs` | 在合成纹理之后接入 `self.paint_pinned_badges(ui, rect, viewport)`。 |
| `crates/mw-shell/src/panels/context.rs` | 枚举扩充 `ToggleConceptPinned` 与 `ArrangeSelectedConcepts`。 |
| `crates/mw-shell/src/app_methods_2.rs` | 右键菜单接入单选概念「钉住位置/取消钉住位置」、多选概念「整理选中概念」、「整理全部布局」及「钉住/取消钉住」，支持 toast 提示反馈与焦点恢复。 |
| `crates/mw-shell/src/app_methods_1.rs` | 新增 `MW_CONCEPT_PIN=<文字>` 演示钩子，支持多值逗号分隔。 |
| `scripts/x100-stage-cm-gate.ps1` | 新增 `cm-pin` 与 `cm-arrange-pinned` 2 场，更新 `cm-multi-menu` 与 `cm-submenu-bottom` 基线；修复 C15 脚本 Warmup 8 错位为 24；添加 UTF-8 BOM 确保 Windows PowerShell 原生解析。 |

## 6. 证据与验证

### 6.1 单元测试

全工作区串行全量通过：
- `mw-core`：301 passed（新增 6 条：4 条 concept_layout，2 条 document）；
- `mw-shell`：616 passed / 1 ignored；
- `mw-ui`：50 passed；
- `mw-format`：46 passed；
- `mw-io`：54 passed；
- `mw-markdown`：42 passed；
- `mw-render`：1 passed；
- `mw-golden`：1 passed；
- 工作区总计：**1109 passed / 0 failed / 1 ignored**。

### 6.2 机器门禁

1. **Stage CM 门禁**（`scripts/x100-stage-cm-gate.ps1`）：
   - `cargo fmt --check`：PASS；
   - `mw-shell clippy`：PASS（新增 0 告警）；
   - `.zmind` 往返断言：PASS；
   - 工作区串行全量测试：PASS（1109 passed）；
   - Release 构建：PASS；
   - 金样双跑：**60 场 / 120 次逐像素比较 PASS，0 意外漂移**。

2. **金样场景哈希**：
   - `cm-pin`：1920×1200，sha256 `F5BB4AA8DBC8`，133 274 B，钉住徽标 117 像素；
   - `cm-arrange-pinned`：1920×1200，sha256 `8C2D3960524A`，119 036 B，与 `cm-arrange` 差异 42 569 像素；
   - `cm-multi-menu`：1920×1200，sha256 `F47BEFEAAC5F`，161 809 B；
   - `cm-submenu-bottom`：1920×780，sha256 `DF00C44F0969`，136 393 B。

## 7. 未做与风险

- 快捷键支持：目前仅右键菜单提供钉住/只整理选中，后续可根据用户偏好增加快捷键绑定。
- 人工核验：机器门全绿，人工交互门等待人工确认（不记已验证）。
