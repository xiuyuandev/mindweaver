# CER-03 · 内容表面键盘 / IME / 焦点底座

> 状态：已实现 · 开始 2026-10-25 · 批次 B2-A · 依赖：CER-01（已完成）
> 设计合同：[`docs/28`](../../../docs/28-内容体验重构规范.md) §4.6（焦点与键盘）、§5.4/§5.5、§6.8、§7.8
> 执行合同：[`docs/29`](../../../docs/29-内容体验重构工作包.md) §6

## 1. 范围

新建 `crates/mw-shell/src/content_keys.rs`：内容表面的**唯一**裸键判定点。
CER-04 起的大纲页、CER-13 起的卡片页、CER-22 起的笔记库都从这里取"此刻键归谁"，
不允许各自再写一套 if 链。

1. **`Foreground` 快照**：模态（`top_modal().is_some()` + 演示弹窗）、菜单
   （标题栏 Memory 弹层 / 右键菜单 / 速查表）、斜杠菜单、拖拽态、IME 组合态
   五个布尔位，由调用方每帧收集一次。
2. **`page_owns_bare_keys(fg)`**：模态/菜单/斜杠/拖拽任一占用 → 内容页不收裸键
   （Esc/方向键/Enter/单字母）。docs/28 §4.6「弹窗、菜单、斜杠菜单打开时，
   内容表面不收裸键」。
3. **`editor_owns_text_keys(fg)`**：IME 组合态（`Preedit` 非空）时
   Enter/Tab/Esc 先归输入法——编辑器不得拆行、不得缩进、不得提交；
   docs/28 §4.6「组合期间不切换行、不提交、不触发全局快捷键」。
4. **`ime_composing(events)`** 纯函数：从当帧输入事件识别组合态；
   与现有三处（`canvas.rs:1208`、`panels/search.rs:152`、`panels/outline.rs:774`）
   的事件式判定同口径，本包不改动那三处。
5. **`tab_cycle(slots, current, backwards)`**：Tab 顺序助手——在固定槽位序列里
   循环前进/后退，供页面包定义 Tab/F6 顺序（如库页 侧栏→列表→预览）。
6. **焦点保存/恢复助手**：`save_focus(ctx)` / `restore_focus(ctx, id)` 薄封装
   （egui Memory），开浮层前存、关浮层后还；语义与 `panels::focus::FocusMemory`
   互补（FocusMemory 管面板级 Tab 记忆，本助手管"浮层关闭后焦点回原控件"）。
7. **同源改造**：`home_bare_keys_owned(menu_open, help_open)` 与
   `page_keys_owned(modal_top, dialog_demo)` 改为委托 `page_owns_bare_keys`，
   签名与既有单测不动，行为逐位等价：
   - 开始页：`menu_open: menu_open || help_open`；
   - 笔记库：`modal_open: modal_top || dialog_demo`。

## 2. 非目标

- 不改大纲/卡片页/笔记库的任何绘制与交互路径（页面接线在 CER-04+）。
- 不改 `canvas.rs` / `panels/search.rs` / `panels/outline.rs` 现有三处
  IME 事件判定（大纲行内编辑在 CER-07 重做时换用本门）。
- 不动 `FocusMemory` / `overlay::Modal` / 快捷键表结构。
- 不做全局快捷键重新分配；`window_shortcuts` 的优先级链保持原样。
- 不改外观；预期 31+13 组 golden 全部逐字节不变。
- 不新增依赖、crate；不改 `.zmind` / 命令层。

## 3. 验收场景

| 场景 | 输入 | 期望 |
| --- | --- | --- |
| 门矩阵单测 | `Foreground` 32 种组合 × 2 个判定 | 空前台才拥有裸键；IME 只影响编辑键判定；两个旧函数委托后与原真值表逐位一致 |
| `tab_cycle` 单测 | 空序列 / 越界 / 首尾循环 / 反向 | 首尾相接、None 起点 → 首、backwards 从首 → 尾 |
| `ime_composing` 单测 | Preedit 空/非空/Commit/Delete 事件 | 非空 Preedit 才算组合态，与既有三处口径一致 |
| `MW_TRACE` 模态脚本 | 复跑 `library-modal-input.txt`（`MW_DIALOG_DEMO=1`） | 弹窗开着时 `ArrowDown/Enter` 不落库页：轨迹无库页焦点变化 |
| `MW_TRACE` 菜单脚本 | 复跑 `start-menu-input.txt` + `start-menu-esc.txt`（标题栏菜单开） | 菜单开着时 `1/Esc` 归菜单；Esc 收菜单后焦点回开始页 |
| 旧 golden 双跑 | 31+13 组 | 全部逐字节不变（本包零视觉改动） |

## 4. 旧基线

- 开工基线（CER-02 后）：**777 tests / 0 failed**；31 组旧 golden + 13 组
  content-kit 双跑 `--tol 0` 全 0 差异；fmt 干净；clippy 新增 0。
- 本包要求：44 组 golden 逐字节不变（零视觉改动）；测试数增加、失败 0；
  `home_bare_keys_owned` / `page_keys_owned` 既有单测不改一行仍通过。

## 5. 变更

| 文件 | 变更 | 动外观/数据/格式 |
| --- | --- | --- |
| `crates/mw-shell/src/content_keys.rs` | **新增**：`Foreground` 快照（模态/菜单/斜杠/拖拽/IME 五位）、`page_owns_bare_keys`、`editor_owns_text_keys`、`ime_composing(events)` 纯函数与 `ime_composing_now(ctx)`、`save_focus`/`restore_focus` 薄封装、`tab_cycle` 助手；4 个单测（含 32 组合门矩阵） | 否 |
| `crates/mw-shell/src/app_methods_1.rs` | `home_bare_keys_owned` 改为委托 `page_owns_bare_keys`（`menu \|\| help` 归菜单位）；签名不变 | 否（行为逐位等价） |
| `crates/mw-shell/src/note_library.rs` | `page_keys_owned` 改为委托 `page_owns_bare_keys`（`modal \|\| demo` 归模态位）；签名不变 | 否（行为逐位等价） |
| `crates/mw-shell/src/main.rs` | `mod content_keys;` | 否 |
| `shots/x100/CER-03/traces/` | 三份 MW_TRACE 证据 + 终帧 PNG + 判据 README | 否 |
| `shots/x100/CER-03/scripts/` | 三个既有行为脚本的同名副本（docs/29 §10） | 否 |

未改任何绘制路径；既有三处 IME 事件判定（canvas/search/outline）未动；
`FocusMemory`、`overlay::Modal`、`window_shortcuts` 未动。

## 6. 验证

**行为证据**

- `cargo test --workspace -- --test-threads=1` → **781 passed / 0 failed**
  （CER-02 后 777 + 本包 4）。既有 `start_page_bare_keys_yield_to_foreground_layers`、
  `page_keys_yield_to_foreground_modals` 单测**一行未改**仍通过——同源改造的护栏。
- 门矩阵：`gate_matrix_covers_every_foreground_combination` 遍历 32 种前台组合；
  `legacy_page_gates_delegate_bit_for_bit` 遍历两个旧函数的全部入参组合，
  证明委托后真值表逐位一致。
- `MW_TRACE` 模态/菜单脚本（详见 `traces/README.md`，跑在 release 二进制上）：
  - `library-modal.trace`：`Down` 未移动库页预览、`Enter` 落弹窗默认按钮
    （`dialog demo closed` / `dialog demo action 1`），末帧库页保留；
  - `start-menu-input.trace`：菜单开着时 `key 1` 之后无模板创建事件，焦点留开始页；
  - `start-menu-esc.trace`：`popup_menu key=escape`，Esc 归菜单。

**视觉证据**

- 本包零视觉改动：不渲染任何新像素。44 组 golden（31 旧 + 13 content-kit）
  全部逐字节不变（回归证据）。

**回归证据**

- 31 组旧 golden 矩阵双跑 `--tol 0` PASS + 13 组 content-kit 双跑 0 差异
  （`target/x100/cer-03-golden.log`）。
- 旧行为脚本复跑即旧文件兼容证据（本包不触碰文档模型与磁盘）。

**工程证据**

- `cargo fmt --all -- --check` 干净；`cargo clippy -p mw-shell --all-targets`
  本包新增 0（前瞻 API 以 `#[allow(dead_code)]` 注记，CER-04+ 接线后移除）；
  release 构建通过；行为脚本与 trace 均在 release 二进制上采集。

## 7. 未做 / 已知偏差

- `editor_owns_text_keys` / `ime_composing*` / `save_focus` / `restore_focus` /
  `tab_cycle` 目前**没有产品页面消费者**（CER-04/07/29 接线）——docs/29 CER-03
  要求底座先行，故以前瞻 API 交付并用单测钉住语义。
- `Foreground.slash_open` 与 `drag_active` 两个位暂无采集方（斜杠菜单 CER-18、
  大纲拖拽 CER-09 才有真实来源）；本包只在门矩阵里保证它们占用时让键。
- IME 判定是事件式（当帧 Preedit），跨帧组合态记忆留给 CER-07 的编辑器本地草稿。
- 开始页速查表（F1）仍只有纯函数门 + 单测，未单独实拍（沿 X20 未做项）。

## 8. 风险

- 同源改造若改变任何一个真值表位，开始页/笔记库的既有行为脚本会立刻露馅；
  委托必须逐位等价，单测矩阵是护栏。
- `ime_composing` 是事件式判定（当帧 Preedit 事件），不维护跨帧状态；
  CER-07 行内编辑若需要"跨帧组合态"，由编辑器自己保存，门保持无状态。
- Tab 顺序助手的槽位语义要等 CER-04/29 定义页面槽位清单后才有真实消费者；
  本包只交付纯函数与测试，防过早设计。

## 9. 人工判词

非门包，不填。
