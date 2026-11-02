# CER-05 · 大纲行绘制 v2

> 状态：已实现（换行 +24 留 CER-11，见 §7）· 开始 2026-10-25 · 批次 B2-A · 依赖：CER-04（已完成）
> 设计合同：[`docs/28`](../../../docs/28-内容体验重构规范.md) §5.2（行解剖）、§5.9（禁止项）
> 执行合同：[`docs/29`](../../../docs/29-内容体验重构工作包.md) §6（X61 范围并入）

## 1. 范围

按 `docs/28` §5.2 重写行绘制（只动绘制层；选择/焦点状态表属 CER-06）：

1. **行高 32 固定**；换行每行 +24（沿用既有换行测量，节距按 24 补足）；
   **说明行 +20**（13/20 `text.secondary`，与文本同 x，取备注首行）。
2. **缩进每级 24**。
3. **项目符号 4px 圆点**，距文本 12；折叠时换成 12px chevron + 子项数
   （11.5 辅助色）；展开时 chevron 只在悬停出现。
4. **文本 16/26/400**，`text.primary`；节点显式字号覆盖保留（夹 12–28）。
5. **右侧指示简化为单个 5px 圆点**（有卡片/备注才显示）；删除标签计数
   `#1` 胶囊、双色点、旧任务框角标；任务框改为**行内 14×14、1.5px 边**
   （完成后文字 `text.faint`，不划删除线）。
6. **删除全部树形竖线/连接线/引导线**（§5.9）与 `guide_mask` 绘制路径。
7. **密度三档移除**：§5.2 固定 32/24/16-26，`OutlineDensity` 收敛为单一规格；
   `MW_OUTLINE_DENSITY` 环境变量保留为接受但不产生差异的 no-op
   （旧门脚本的 `outline-compact` 场景在 CER-12 新门脚本接管前保持可跑）。
8. 握把/更多维持"悬停/选中出现"（现状已满足，几何按 §5.2 微调）。

## 2. 非目标

- 不做选择/焦点/多选状态表（CER-06）；演示种子 pin 焦点环的偏差仍留待 CER-06。
- 不做行内编辑改造（CER-07）、结构命令（CER-08）、拖拽重做（CER-09）、
  搜索筛选（CER-10）、虚拟化与 20k（CER-11）。
- 不改命令层、文档模型、`.zmind`。
- 不做折叠 chevron 的展开态悬停新交互（现有 chevron 点击保留）。

## 3. 验收场景

| 场景 | 输入 | 期望 |
| --- | --- | --- |
| `outline`（重拍） | 13 节点样例 @1.5 | 32px 行、24px 缩进、4px 圆点、无树线、单个 5px 右点、任务框行内 |
| `outline-focus`（重拍） | 聚焦子树 @1.5 | 同上 |
| `outline-compact`（重拍） | 密度 no-op @1.5 | 与 `outline` 同帧（密度已移除） |
| `outline-narrow`（重拍） | 800×600 | 行绘制 v2 + 不横向滚动 |
| 行度量单测 | 纯函数 | 行高 32/换行 +24/说明行 +20/缩进 24 的度量函数断言 |

## 4. 旧基线（开工前留旧账）

CER-04 重拍后的当前基线（1200×720 默认窗 / ppp 1.5 / snow）：

| 场景 | SHA-256（前 16） | 大小 |
| --- | --- | --- |
| `outline` | `ad4fbc4a839288f6` | 140,929 B |
| `outline-focus` | `3dcb44968c7b12fc` | 106,463 B |
| `outline-compact` | `8cc162256c62b755` | 138,663 B |
| `outline-narrow` | `77d31cb0ad5a077b` | 108,333 B |

## 5. 变更

| 文件 | 变更 | 动外观 |
| --- | --- | --- |
| `crates/mw-shell/src/panels/outline.rs` | `OutlineDensity` 三档收敛为固定规格（row 32 / indent 24 / font 16；枚举与 `MW_OUTLINE_DENSITY` 保留为 no-op）；**删除全部树形竖线/肘线绘制**；圆点 4px 中性色（`ch.dim`，分支色随颜色测试移除）；折叠/悬停/选中换 12px chevron，热区 20px；文本 16/26（depth 0 标题 28/600 不变）；**任务框行内 14×14、1.5px 边**（先定位后画字），完成任务文字 `text.faint` 不划删除线；**行尾简化为单个 5px 圆点**（卡片/备注），删除 `#n` 标签胶囊与双色点；折叠子项数改 11.5 纯文字（去胶囊底）；`marker_reserve` 按新规则重写；`Row.branch/guide_mask/labels` 字段与 `Icon::Levels/Density`、`ui_kit::segmented` 等孤儿加注保留 | **是** |
| `crates/mw-shell/src/panels/outline.rs` 测试 | `density_changes_row_height_indent_and_type_scale` → `density_is_a_noop_and_geometry_is_fixed`（32/24/16 断言）；`marker_reserve` 断言改为单圆点口径；`rows_carry_note_card_label_and_task_marks` 增加 `note_preview` 首个非空行断言（夹具备注改多行） | 否 |

数据/命令层零改动；`Row` 数据字段保留（CER-06/11 复用）。

## 6. 验证

**行为证据**

- `cargo test --workspace -- --test-threads=1` → **780 passed / 0 failed**
  （781 − 1：密度测试改写为固定规格断言，数量不变）。
- 行度量单测：32/24/16 固定值、`marker_reserve` 单圆点口径。

**视觉证据**（四场景均人工过目）

- `outline`（重拍 @1.5 snow）：行即文字——无树线、中性 4px 圆点、32px 节距、
  24px 缩进、任务框行内（"交互与性能"）、单个 5px 右点（"中文排版"）、
  **说明行 13/20**（"标点悬挂与避头尾规则要逐条对照。"），`#1` 标签胶囊与
  双色点消失。
- `outline-focus` / `outline-compact` / `outline-narrow`（重拍）：同一行语言；
  compact 与 default 同帧（密度已移除，no-op 验证）。

**回归证据**

- 4 个 outline 场景双跑 `--tol 0` 全 0 差异。
- 其余 27 组旧 golden + 13 组 content-kit：改动仅在 `Outline::show` 绘制路径，
  机器门 golden 矩阵双跑复核（`target/x100/cer-05-golden.log`）。

**工程证据**

- `cargo fmt --all -- --check` 干净；`cargo clippy -p mw-shell --all-targets`
  触碰文件新增 0（孤儿项加注放行，注明 CER-06+ 复用）；release 构建通过。

## 7. 未做 / 已知偏差

- **换行 +24 未做**：当前长文本仍为单行省略 + 悬停全文（X20 行为）；
  换行行高依赖版心宽度测量，与 CER-11 的动态行高虚拟化一并实现。
- **说明行已实现**（本报告 §5/§6）：备注首行 13/20 `text.secondary`、与正文同 x、
  行高 +20、正文锚上带；多行备注取首个非空行并去首尾空白（单测钉住）。
- **标题行焦点环**：CER-04 已记录的演示种子 pin 焦点偏差仍在（CER-06 修）。
- 折叠 chevron 的"展开时悬停出现"已实现，但悬停探测用 `pointer_hover_pos`
  全局指针——离屏 golden 无指针时不出现，属预期（脚本注入 hover 才可见）。
- 密度移除属功能删除：`outline-compact` 场景语义变为"与 default 同帧"，
  CER-12 新门脚本应把它从场景表移除。

## 8. 风险

- 换行 + 说明行会改变行高计算，虚拟化（CER-11）未就位前依赖现有
  `visible` 切片；20k 预算在 CER-11 验证。
- 演示样例里 `#1` 标签计数与双色点删除后，标签信息只剩检查器/右键菜单
  一条路径——符合 §5.2「行内不显示标签」，但用户可见性下降，留 G-B2-1 判词。
- 密度移除属功能删除；X19 的密度 golden（本包 `outline-compact`）语义变化
  已在报告与 manifest 记录。

## 9. 人工判词

非门包，不填。
