# CER-06 · 大纲选择 / 焦点 / 多选

> 状态：已实现（焦点环四主题截图为已知缺口，见 §7）· 开始 2026-10-25 · 批次 B2-A · 依赖：CER-05、CER-03（均已完成）
> 设计合同：[`docs/28`](../../../docs/28-内容体验重构规范.md) §5.3（状态表）、§4.3（状态透明度）、§4.6（焦点与键盘）
> 执行合同：[`docs/29`](../../../docs/29-内容体验重构工作包.md) §6（X63/X69 范围并入）

## 1. 范围

把行状态从 chrome 混色迁到 `design::content` 令牌，并落实 §5.3 状态表：
只动**状态着色与焦点归属**；选择/多选交互逻辑沿用 X-series 既有实现
（`ToggleSelect` / `SetSelection` / 区间选择已存在并有单测）。

1. **状态着色改令牌**（`content_metrics(theme, dpi)` 派生，亮/暗自动）：
   - 悬停 = `state.hover`（主色 4%）；
   - 选中 = `state.selected`（强调色 7%）；
   - 多选非主选 = 强调色 5%（§5.3 多选行；令牌表未单列，按 §4.3 口径取
     强调色 5%——alpha 13，代码注释钉住出处）；
   - 编辑行 = `state.editing`（强调色 3%）；
   - 圆角 8 → `content::radius::ROW`（6）；
   - **删除选中描边**（§5.3：无边框；X20 的 `sel_stroke` 35% 描边框是人工
     判词点名项）。
2. **选中与键盘焦点分离**（修正 CER-04/05 记录的偏差）：
   `request_row_focus`（命令路径 pin：剪贴板/结构动作/演示种子）不再置
   `keyboard_focus`；焦点环只在真实键盘导航（方向键/Tab/Esc，`nav_key` 与
   行内键处理）后出现——鼠标点击与程序化选中只显示选中底。
3. **选中摘要**：页脚已有「选中 N」（CER-04 保留）；本包核对多选下文案。
4. **Shift/Ctrl 选择**：既有键盘/鼠标路径回归核对（不改逻辑）。

## 2. 非目标

- 不做拖拽源/目标状态重绘（CER-09；现有拖拽视觉保留）。
- 不做搜索命中高亮 12%（CER-10）。
- 不做行内编辑重做（CER-07；编辑行底色 3% 本包一并落地，属 §5.3 状态表）。
- 不改选择数据结构与命令层。

## 3. 验收场景

| 场景 | 输入 | 期望 |
| --- | --- | --- |
| `outline`（重拍） | 13 节点样例 @1.5 | 默认画面无选中底、无描边；演示种子 pin 不再画焦点环 |
| `outline-focus`（重拍） | 聚焦子树 @1.5 | 同上（pin 行无环） |
| `outline-compact` / `outline-narrow`（重拍） | no-op / 800×600 | 同上 |
| `outline-hover-select`（新增） | 脚本 hover 一行 + click 另一行 | hover 4% 与 selected 7% 同帧共存、无边框、圆角 6 |
| `outline-ring-*`（新增 ×4 主题） | 脚本 ArrowDown | 键盘焦点环 1.5px 出现在键盘到达的行上（四主题） |
| 单测 | 纯函数 | `request_row_focus` 不置 `keyboard_focus`（pin/键盘分离钉子）；既有选择逻辑测试全绿 |

## 4. 旧基线（开工前留旧账）

CER-05 后的当前基线（1200×720 默认窗 / ppp 1.5 / snow）：

| 场景 | SHA-256（前 16） | 大小 |
| --- | --- | --- |
| `outline` | `bd39b43d7087de13` | 153,733 B |
| `outline-focus` | `74741253205d04c8` | 106,589 B |
| `outline-compact` | `bd39b43d7087de13` | 153,733 B（与 outline 同帧，密度已移除） |
| `outline-narrow` | `a82230285f16c76f` | 123,787 B |

预期漂移：默认画面去掉根行焦点环（pin 分离）+ 选中底 10%→7% + 描边删除 +
圆角 8→6（当前画面无选中行，主要是环消失）。

## 5. 变更

| 文件 | 变更 | 动外观 |
| --- | --- | --- |
| `crates/mw-shell/src/panels/outline.rs` | 状态着色迁 `design::content` 令牌：悬停 4%（主色）/ 主选 7%（强调色）/ 多选 5%（强调色 alpha 13，§5.3）/ 编辑 3%，圆角 8→6，**删除 `sel_stroke` 35% 描边框**；状态优先级 编辑 > 主选 > 多选 > 悬停；**行悬停双源修复**：`is_hot = 画布同步 \|\| resp.hovered()`——X20 的 `outline_screen` 对全屏大纲恒传 `None`，行悬停此前永远画不出来；`request_row_focus` 不再置 `keyboard_focus`（pin 与键盘焦点分离，CER-04/05 偏差修正）；删除孤儿 `mix32`；新增 `c32a`（保 alpha 转换）与 `pin_keeps_keyboard_focus_separate` 单测 | **是** |

选择/多选交互逻辑（Shift/Ctrl/区间/Ctrl+A/Esc 收敛）沿用 X-series 既有实现，本包未改。

## 6. 验证

**行为证据**

- `cargo test --workspace -- --test-threads=1` → **781 passed / 0 failed**
  （CER-05 后 780 + 1：`pin_keeps_keyboard_focus_separate`——程序化 pin 只落
  `focus_request`，不点亮键盘焦点环）。
- `outline-hover-select` 脚本实拍（`MW_GOLDEN_INPUT`）：点击 wgpu 行（选中 7%
  持久）→ 悬停 winit 行（hover 4%）——两种状态同帧共存、无边框、圆角 6、
  两行各带握把；连跑三次全 0 差异。

**视觉证据**

- 5 个 outline 场景全部人工过目：`outline`（默认画面干净——演示种子 pin 不再
  画环，CER-04/05 偏差消除）、`outline-focus`、`outline-compact`、
  `outline-narrow`、`outline-hover-select`。
- 悬停/选中底色由令牌派生（snow 实测：hover ≈ (246,246,247)、selected ≈
  (240,245,244)，与 §4.3 的 4%/7% 吻合）；暗色主题走同一令牌路径（`ink` 的
  hover 6%/selected 12% 由 `ContentAlphas` 保证，未单独实拍——陈列页
  content-kit-ink 已覆盖令牌的暗色值）。

**回归证据**

- 5 个 outline 场景双跑 `--tol 0` 全 0 差异；机器门 golden 矩阵双跑 PASS
  （`target/x100/cer-06-golden.log`，其余 26 组基线逐字节不变）。
- 选择逻辑既有单测（区间/收敛/主选）全绿，交互零改动。

**工程证据**

- `cargo fmt --all -- --check` 干净；`cargo clippy -p mw-shell --all-targets`
  触碰文件新增 0（`mix32` 随 chrome 混色退役直接删除）；release 构建通过。

## 7. 未做 / 已知偏差

- **键盘焦点环四主题截图未完成**：`outline-ring-*` 试拍发现**键盘焦点起点
  非确定性**——同一脚本两次运行，ArrowDown 后的选中行不同（差分图钉在
  DirectWrite 行）。根因方向：boot 时 pin 的 egui 焦点与 ArrowDown 的
  `next` 起点存在竞态（`focus_target` 在两帧序里解析到不同当前行）。
  试拍基线已删除（不留不稳定基线）；修复属于焦点模型收束，列入 CER-06
  续轮或 CER-07（行内编辑同样依赖焦点起点）。环本身可用（CER-04 时代
  pin+keyboard_focus 路径实拍过）。
- 拖拽源/目标状态、搜索命中 12% 高亮：CER-09/CER-10。
- 悬停探测依赖指针注入：离屏无脚本时无 hover 底，属预期。

## 8. 风险

- `request_row_focus` 去掉 `keyboard_focus` 后，键盘 Delete/移动后的环依赖
  此前导航留下的 `keyboard_focus=true`——需核对键盘路径不会中途被鼠标复位
  （点击行为置 false，属正确语义）。
- hover 探测在离屏 golden 无指针时不出现；hover 态必须走脚本注入。

## 9. 人工判词

非门包，不填。
