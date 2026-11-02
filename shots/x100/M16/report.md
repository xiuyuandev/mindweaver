# M16 · 金样按帧确定性（outline-* 抖动）

- 日期：2026-10-31（打磨轮 35）
- 合同：[`HANDOFF.md`](../../../HANDOFF.md) §7.5① / §7.7 第 2 步；[`docs/STATUS.md`](../../../docs/STATUS.md) §1「唯一未修的机器红灯」
- 范围：**harness 与大纲的计时/输入底座**，不改产品视觉、不改文档模型、不动 `.zmind`
- 状态：已实现（机器门两遍全绿后回填；人工门不涉及）

---

## 1. 问题

`stage-b` 门第 1 遍（紧跟 fmt/clippy/全量测试之后，机器还在降载）会在两场大纲场景上抓到
与基线不同的画面，第 2 遍又全绿：

| 场景 | 2026-10-30 失败时的差异 | 现场（`failure-2026-10-30/`） |
| --- | --- | --- |
| `outline-drag-illegal` | 2.827%，49465 px，max 225，bbox `(210,155)-(1409,954)` | 页面标题变成了**编辑态**、只剩一个 `4|`，整张列表因此下移；其余拖拽幽灵/落点框与基线一致 |
| `outline-hover-select` | 2.751% | 点击选中的行正常，「纯 Rust 原生」行的**悬停底与握把整块消失** |

两场都是「同一份构建、同一份脚本、同一帧数」，说明差异不在脚本，而在**帧里读到的真实时间与真实输入**。

## 2. 诊断

复现尝试：两场各跑 5 次空载 + 6 次带 8 个自终止 CPU 负载，**10 张图逐字节相同**，且都等于基线；
用 `--update` 抓到临时目录对比哈希也一致。所以不是「CPU 一忙就必现」，只在本机门第一遍那种
「GPU 刚起、帧耗时抖动」的窗口里出现。

对着两条线索查代码，root cause 有两层：

1. **应用层计时读墙钟**。大纲的握把延迟 `GRIP_DELAY = 120ms` 与折叠行拖拽展开 `500ms`
   都用 `std::time::Instant::now()`/`elapsed()`（`outline.rs` 的 `grip_since` / `drag_hover`）。
   金样按**固定帧数**截帧（脚本帧数 + 4），帧耗时随机器负载变化 ⇒ 第 N 帧对应的真实毫秒数不同，
   悬停态可能还没攒满阈值。egui 自己的滚动落定动画读 `stable_dt`，同样随真实帧耗时变化。
2. **宿主输入漏进离屏进程**。`raw_input_hook` 只处理「纯键盘脚本要清指针」这一种情况；
   带指针的脚本完全信任宿主事件。离屏窗口虽然不可见，仍可能收到桌面的
   `PointerGone` / 真实键入（前者正好把 `hover` 抹掉，后者能解释标题里多出的字符）。
   两条都只在**窗口不可见但进程仍收事件**的机器状态下发生，和「第一遍偶发」吻合。

## 3. 改法

### 3.1 金样时间改为帧计数（`golden_input.rs` / `main.rs`）

- `GoldenInput::advance_clock()`：普通帧推进 `1/60s`；`sleep` 期间冻结（本来也不截帧），
  到期那一帧把脚本写的名义时长一次补上。总时长与真实帧率无关。
- `Shell::golden_clock` 每帧把虚拟秒数写进 `RawInput::time`，并把 `predicted_dt` 固定为 `1/60`。
  eframe 在 `raw_input_hook` **之前**写好真实时间（`epi_integration.rs:415`），hook 里的覆盖生效。
- 效果：`InputState::time` / `stable_dt` / `unstable_dt` 全部由帧数决定；egui 的悬停计时、
  滚动落定、`animate_bool` 类动画都不再随机器负载漂。

### 3.2 宿主输入与金样隔离（`main.rs`）

`raw_input_hook` 在 harness 下先 `retain(!is_host_input)`：指针、滚轮、键盘、文本、IME、
剪贴板、窗口焦点、AccessKit 事件一律丢掉，只保留脚本后注入的那一份。
纯键盘脚本仍把桌面指针挪到画外；带指针脚本的脚本事件照旧入场。

### 3.3 大纲计时改读 egui 时间（`outline.rs`）

`grip_since` / `drag_hover` 从 `(String, Instant)` 改为 `(String, f64 秒)`，
记录与比较都用 `ui.input(|i| i.time)`；`grip_ready` 仍收 `Duration`。
真实运行语义不变（egui 时间本来就是单调墙钟），金样下则与帧数绑定。

## 4. 验证

### 4.1 单测

新增 `golden_input::tests::clock_is_frame_based_and_sleep_adds_nominal_time`：
普通帧 = 1/60；`sleep` 期间返回 0；到期那帧返回 0.040（脚本写 40ms）。

全量：**1022 passed / 0 failed / 1 ignored**（23 个套件；`scripts/test.ps1`，TEMP 已重定向）。

### 4.2 三场风险场景仍与基线逐字节一致

| 场景 | 命令要点 | 结果 | 基线 sha256 |
| --- | --- | --- | --- |
| `outline-drag-illegal` | ppp 1.5 / warmup 3 / 脚本 | `✓ 无超容差差异` | `1776a77897f6…` |
| `outline-hover-select` | ppp 1.5 / warmup 3 / 1200×720 / 脚本 | `✓ 无超容差差异` | `0031d764b789…` |
| `outline-20k-deep` | ppp 1.0 / warmup 16 / `MW_OUTLINE_SCROLL_TO=分支 0150` | `✓ 无超容差差异` | `50f2152f9c11…` |

后两场是原门里的另外两个历史抖动点（`20k-deep` 0.496%、`hover-select` 2.751%），
本轮不改基线、不重拍——虚拟时钟把「真实帧耗时」从截帧条件里拿掉后，画面没有漂。

### 4.3 机器门

- [x] `stage-b` 两遍全绿：`started=00:24:06` → `PASS pass 1 (118)` / `PASS pass 2 (118)` /
      `MACHINE GATE PASS`，`finished=00:30:48`（日志 `target/tmp/gate-b-run1.log`）
- [x] 冷机复跑一遍 `stage-b`：`started=00:33:57` → `PASS pass 1 (118)` / `PASS pass 2 (118)` /
      `MACHINE GATE PASS`，`finished=00:40:10`（日志 `target/tmp/gate-b-run2.log`）
- [x] `stage-cm`（上一轮已绿，本轮未动概念图代码）：49 场 / 98 次比较，`PASS`

两次连跑共四遍矩阵、472 次比较，**零差异**。第 1 遍正是过去偶发红的那个位置
（紧跟 fmt/clippy/全量测试、机器还在降载），本轮不再复现。

据此，[`docs/STATUS.md`](../../../docs/STATUS.md) §1 的「唯一未修的机器红灯」关闭；
`HANDOFF.md` §7.5① 结项。人工门（X20 审核）仍需人来判，本轮不代判。

## 5. 未做 / 风险

- **未在旧代码上复现失败**：10 次空载/负载复跑都逐字节等于基线，因此无法做「旧码红 → 新码绿」
  的 A/B；结论来自失败现场（两张实际图）+ 代码路径。这一点必须如实记着。
- `outline-drag-illegal` 失败图里的标题「4|」无法用当前代码路径单独解释（没有任何脚本会键入 `4`），
  最合理的解释是宿主真实键入漏进离屏进程；本轮做了输入隔离，但没有在真机上重放那条事件序列。
- 虚拟时钟只覆盖 harness；真实交互仍用墙钟，这是刻意的（用户的时间和机器时间应一致）。
- 金样 `sleep` 期间虚拟时间冻结：如果某场依赖「sleep 期间动画继续走」，该场会改为
  「到期后一次跳到名义时长」的状态。三场风险场景与既有基线逐字节一致，但整表 170 张
  仍需过一道门确认；若其它场景出现漂移，按 `docs/29` §5 单独记账重拍，不混进本轮。
- 门跑绿后已清掉 `shots/golden/outline-*/actual|diff` 六场共 12 张旧失败图（gitignore 的运行时产物）。
  本包重点两场的现场原件留 `failure-2026-10-30/`；其余四场（`20k-deep` / `drag` / `search` /
  `search-labels`，2026-10-28 至 10-29 的旧失败）收进 `target/tmp/stale-golden-artifacts-2026-10-31/`，
  不占仓库，也不误导后来的读者。
