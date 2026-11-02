# M8 · 撤销/重做节奏（打磨轮 5）

> 依据：`docs/x100/polish-ledger.md` §1 M8 行与 §3 第 5 条
> （「连续『Tab 建三个 + 改字 + 撤销』的端到端」）。
> 状态：**撤销粒度补成「一颗节点一步」，四张基线与轨迹都在**；实机 IME 组合期的撤销仍 `未验证`。

## 1. 这一轮要解决什么

账本的原判是「`Ctrl+Y` 与中文输入混排未端到端验证」。按脚本先把节奏跑了一遍，看到的是更要紧的事：

| 动作 | 改之前 | 应有的样子 |
| --- | --- | --- |
| 建一颗、写名字、按一次 `Ctrl+Z` | 只撤掉**文字**，树上留一个**空盒子** | 这一颗整颗消失 |
| 连建三颗 | 要按**六次** `Ctrl+Z` 才撤干净 | 三次 |

原因是建节点和写名字是两条命令：`Tab` 先 `AddNode`（空文本），提交时再 `SetText`。
按「每次提交一条」的规矩这没错，但用户眼里「建了一个节点写了字」是**一个动作**。

## 2. 非目标

- 不改撤销栈容量（`HISTORY_LIMIT`）、不改 `Command` 集合、不改 `.zmind`。
- 不让「改已有节点的字」也变成一步以外的东西——合并只在**这次编辑刚建出来的那颗**上发生。
- 不测卡片页 / 大纲页 / 表格的撤销节奏，也不测 `Ctrl+Shift+Z`（只测 `Ctrl+Y`）。

## 3. 改了什么（代码）

| 位置 | 改动 |
| --- | --- |
| `mw-core/src/document.rs::ChangeSet::created` | 「这一条是不是新建了这枚节点」= `after` 里有、`before` 里没有 |
| `mw-core/src/document.rs::Document::apply_coalesced` | 新 API：执行命令后按调用方给的判据决定**并进上一条撤销**还是照常入栈。判据返回 `false` 时与 `apply` 完全一致 |
| `mw-core/src/document.rs::Document::{undo_depth, redo_depth}` | 栈深，供轨迹与测试读 |
| `mw-shell/src/canvas/canvas_methods_3.rs::commit_edit` | 节点是 `Editor.fresh`（这次编辑建出来的）时，`SetText` 走 `apply_coalesced(... |prev| fresh && prev.created(&id))` |
| `canvas_methods_3.rs::{undo, redo}` | 新轨迹 `undo label=… left=…` / `redo label=… left=…`（只在 `MW_TRACE` 下写） |

**踩到并修掉的坑（写在这里，免得下一任重犯）**：第一版合并把上一条的 `after`
**整段换成**这一条的 `after`。`SetText` 的 `after` 只含它碰到的那个节点，
于是「父节点的 `children` 里多了一个孩子」这半条快照被丢掉——**撤销正常，重做却只把节点
塞回 `nodes`、不挂进父节点，树上什么都看不见**。改成按 id 合并（这一条覆盖它的 id，其余留上一条的），
单测也补了「重做后要挂在父节点名下」这一条断言。

## 4. 验收：四张基线 + 轨迹

同一份脚本的四个分叉（`shots/golden/inputs/mm-undo-*.txt`、`mm-redo-one.txt`）：
点「导出」→ `Tab` 建甲并写「甲」→ `Tab` 建乙并写「乙」→ `Tab` 建丙并写「丙」→ 点空白提交 → 指针挪开。

| 场景 | 脚本尾部 | 基线 sha256_12 | 树上应看到 |
| --- | --- | --- | --- |
| `mm-undo-built` | —（对照组） | **`3BDED658F32F`** | 甲 `(958,640)` / 乙 `(1140,645)` / 丙 `(1348,647)` 三颗都在 |
| `mm-undo-one` | `Ctrl+Z` ×1 | **`F790CD6E3700`** | 丙**整颗消失**（该矩形内节点底/字 0 px），甲、乙原地不动 |
| `mm-undo-three` | `Ctrl+Z` ×3 | **`F469E2125A9C`** | 三颗都没了，导出回到无子节点时的位置 `(958,606)` |
| `mm-redo-one` | `Ctrl+Z` ×3 → `Ctrl+Y` ×1 | **`4E51FDA4D475`** | 甲带着「甲」回来 `(958,640)`，乙/丙仍不在 |

`--tol 0` 双跑逐字节相同（`target/x100/gate-b-m8.log`：91 场 / 182 次比较全过）。

## 5. 轨迹证据（`shots/x100/M8/trace-*.txt`）

新轨迹一行一件事：

```
event undo label=<被撤销那条命令的名字> left=<撤销后栈里还剩几条>
event redo label=<…> left=<重做后栈里还剩几条>
```

| 轨迹 | 关键行 |
| --- | --- |
| `trace-undo-one.txt` | `undo label=新建节点 left=19` —— 一次撤销，撤的就是「新建节点」 |
| `trace-undo-three.txt` | 三条 `undo label=新建节点`（`left` 19/18/17），三次正好撤干净 |
| `trace-redo-one.txt` | 三条 `undo` 之后 `redo label=新建节点 left=2`，随后 `scene_node … text="甲" rect=(958,640,82,46)` |

`left` 是**整个会话**的栈深（示例文档本身有十几条建树命令），不是用户动作数——
它只用来证明「一次操作只吃掉一条」。

## 6. 复跑

```bash
cargo build --release -p mw-shell
MW_TRACE=target/x100/m8.txt MW_GOLDEN_INPUT=shots/golden/inputs/mm-undo-one.txt \
  ./target/release/mw-shell.exe --golden --dir target/x100/m8 --ppp 1.0 --warmup 30
# 本目录下的四份 trace-*.txt 也是这样生成的（写到 target/ 再拷进来，别让复跑覆盖证据）

python target/x100/gate_golden.py target/release/mw-shell.exe  # 91 场 / 182 次
python target/x100/gate_cm.py target/release/mw-shell.exe      # 46 场 / 92 次
TEMP="$(pwd)/target/tmp" TMP="$(pwd)/target/tmp" cargo test --workspace -- --test-threads=1
```

本轮实测：测试 **918** passed / 0 failed / 1 ignored（+2：`coalesced_create_and_text_is_one_undo`、
`coalescing_is_opt_in_per_call`）；fmt 干净；clippy 37 条基线与当前指纹一致、**新增 0**；
stage-b **91 场 / 182 次比较**全过（`target/x100/gate-b-m8.log`），既有 87 张零漂移；
stage-cm **46 场 / 92 次比较**全过（`target/x100/gate-cm-m8.log`）。

## 7. 未做 / 风险

1. **实机 IME 未核**（全员共用缺口）：离屏脚本的「打字」是 `text` 事件，不是真组合。
   踩着拼音候选时按 `Ctrl+Z` 会不会先把组合撤掉，没有证据。
2. **只测 `Ctrl+Y`**：`Ctrl+Shift+Z` 是同一动作的备用和弦，没单独拍。
3. **只覆盖画布上的节点编辑**：卡片页正文、大纲行改名、检查器里的字段都没测撤销节奏。
4. **合并在「刚建的那颗」上生效**：已有的节点改名仍是「一次提交一条」，这是有意的；
   但如果将来有人让 `Editor.fresh` 在别的路径上长期为真，会把不相干的命令也合进去。
5. **`left` 数字依赖示例文档**：换测试文档就变，写进基线没有意义，故只留在轨迹里。
