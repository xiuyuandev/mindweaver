# M1 / M2 · 新建节点后的编辑态（打磨轮 2）

> 依据：`docs/x100/polish-ledger.md` §1 M1/M2 与 §3 第 2 条；
> 合同侧 `docs/07` §3.2 事件行为表（「`Enter` 提交编辑 → 触发语义」「`Tab` 提交编辑 → 新建子节点」）。
> 状态：**建树的键盘链路补全并有四张基线**；实机手感与实机 IME 仍 `未验证`。

## 1. 这一轮要解决什么

账本对 M1/M2 的判词是「`Tab` / `Enter` 新建节点后的编辑框位置与滚动没人看过」。
一查下去，问题比「没人看过」更重：**合同要求的行为根本没实现全**。

| 键 | `docs/07` §3.2 要求 | 实测（改之前） |
| --- | --- | --- |
| `Tab`（编辑·未组合） | 提交编辑 → 新建子节点 | **只把焦点转走**：egui 把焦点交给壳层下一个控件，编辑框失焦顺带提交，树上什么都没建，焦点还漏在一个按钮上 |
| `Enter`（编辑·未组合） | 提交编辑 → 触发语义（新建同级） | 只提交，不建同级 |
| `Esc`（编辑·未组合） | 取消编辑还原 | 取消编辑，但**刚建出来的空节点留在树上** |

「`Tab`、打字、`Tab`、打字」是思维导图最常用的建树节奏，改之前从第二下就断了。
本轮按合同补齐，并把四种状态拍成基线。

## 2. 非目标

- 不改 `Shift+Enter` 换行、不改 IME 组合期归属（`edit_keys_owned` 一行未动，`docs/07` R2 仍然成立）。
- 不改概念图的编辑语义：概念图里 `Enter` / `Tab` **都只提交**（没有「同级」这层结构，
  与 CmapTools 的就地改名一致）；`cm-node-edit` 等概念图基线逐字节未动。
- 不改层级/布局算法，不改 `.zmind` / `Node` / `NoteCard` 字段。
- 不拍大文档下的 reveal 节拍（那是 M12）。

## 3. 改了什么（代码）

| 位置 | 改动 |
| --- | --- |
| `canvas.rs::commit_follow_up` | 新纯函数：`(enter, tab, concept_map) → Option<AddKind>`。思维导图里 `Enter`=同级、`Tab`=子级（两者同时到时 `Enter` 优先、只建一个）；概念图两个键都返回 `None` |
| `canvas_methods_2.rs`（`show` 里吃键那一段） | 编辑态多接一个裸 `Tab`；跟随动作记在 `edit_follow_up`，**排在 `commit_edit()` 之后**执行——提交才会把同帧敲进去的字落进文档 |
| `Editor.fresh`（`canvas.rs`） | 记下「这枚节点是这次编辑建出来的」；`add_relative` 建完立刻置位 |
| `canvas_methods_3.rs::cancel_edit` | `fresh` 且被取消时，用 `Command::DeleteNodes` 把它一起收回，选中退回父节点 |
| `canvas_methods_2.rs`（编辑器分支） | 新增 `editor_open` / `editor_close` 轨迹（只在 `MW_TRACE` 下） |

`Tab` 必须在编辑态**自己吃掉**：egui 的 `Tab` 是焦点导航，等到我们读输入时焦点已经动了；
现在这一帧先 `consume_key`，再在提交后自己建节点、把选中挪过去、在新节点上开编辑。

## 4. 验收场景（四张基线）

同一份示例文档（「织思」树，`Session::fresh`），ppp 1.0，1620×1080，`--warmup 20`。

| 场景 | 脚本 | 应看到什么 | 实测 |
| --- | --- | --- | --- |
| `mm-new-child` | 点根 → `Tab` | 根右侧一枚空盒子 + 盒外一圈强调色编辑环 + 末尾光标 | 节点 (990,641) 75×42；环 (986,637)-(1069,687) **416 px** 强调色 |
| `mm-new-chain` | 点根 → `Tab` → 甲 → `Tab` → 乙 | 甲已提交、乙在编辑环里（文字要看得见） | 甲 (987,641) 76×42；乙盒子 (1157,646) 76×33，环 (1152,641)-(1237,682) **388 px**，盒内墨色 84 px = 「乙」 |
| `mm-new-cancel` | 点根 → `Tab` → 草稿 → `Esc` | 同位置什么都不剩，选中退回根 | (985,635)-(1075,690) 里 0 px 节点底/环；全图唯一强调环是根的选中环 (827,522)-(924,581) **253 px** |
| `mm-enter-sibling` | 点「平移缩放」→ `Enter` | 新盒子落在同级序列里（原节点下方），进编辑 | 新节点 (425,615) 76×33，环 (421,610)-(505,651) **384 px** |

四张基线 `--tol 0` 双跑逐字节相同（`target/x100/gate-m1.log`）。

## 5. 轨迹证据（`shots/x100/M1/trace-*.txt`）

新轨迹一行一件事，场景变化才写：

```
event editor_open id=<节点 id> z=<缩放> node=(x,y,w,h) edit=(x,y,w,h) in_view=<0|1>
event editor_close
```

`node` 是节点盒（屏幕坐标），`edit` 是编辑器这一帧真正占住的矩形，`in_view` 说它有没有整个落在画布视口里。
四份轨迹里能读到的（节选）：

| 轨迹 | 关键行 | 说明 |
| --- | --- | --- |
| `trace-mm-new-child.txt` | `editor_open … node=(990,641,75,42) edit=(991,642,75,42) in_view=1` | 编辑框压在节点盒上，只偏 1px（`text_x*zoom − ⌊margin⌋` 的整像素取整） |
| `trace-mm-new-chain.txt` | 两条 `editor_open`（(990,641) → (1157,646)） | 第二个 `Tab` 提交了甲（甲落进 `scene_node` 的 `text="甲"`）并新建了乙 |
| `trace-mm-new-cancel.txt` | `editor_open … node=(990,641,75,42)` → `editor_close`，之后没有任何新的 `editor_open` | `Esc` 之后树上不再有那枚节点 |
| `trace-mm-enter-sibling.txt` | `editor_open … node=(425,615,76,33) in_view=1` | `Enter` 建的是**同级**：y 从 587 下移到 615，x 不变 |

滚动也顺手量了一遍（不进门，只留结论）：从根连按七次 `Tab`，七个新节点的 `in_view` 全是 1，
屏幕 x 从 990 一路走到 1490 后**停在同一位置**（相机跟着往右走，`reveal` 每层都在生效），
不存在「建到屏幕外还看不见」的情况。

## 6. 复跑

```bash
cargo build --release -p mw-shell
MW_TRACE=target/x100/m1-trace.txt MW_GOLDEN_INPUT=shots/golden/inputs/mm-new-child.txt \
  ./target/release/mw-shell.exe --golden --dir target/x100/m1-mm-new-child --ppp 1.0 --warmup 20
# 本目录下的四份 trace-*.txt 就是这样生成的（写到 target/ 再拷进来，别让复跑覆盖证据）

# 双跑 tol 0（与门同源）
python target/x100/gate_golden.py target/release/mw-shell.exe > target/x100/gate-m1.log 2>&1

# 单测
TEMP="$(pwd)/target/tmp" TMP="$(pwd)/target/tmp" cargo test --workspace -- --test-threads=1
```

本轮实测：测试 **913** passed / 0 failed / 1 ignored（+1：`edit_time_enter_and_tab_build_the_next_node_on_a_mind_map_only`）；
fmt 干净；clippy 37 条基线与当前指纹一致、**新增 0**；stage-b 门 **87 场 / 174 次比较** 全过；
stage-cm 门 44 场 / 88 次比较 全过（`target/x100/gate-cm-m1.log`）。

## 7. 未做 / 风险

1. **实机手感没人量**：连按 `Tab` 时新编辑器抢焦点的那一帧、以及 `Tab` 按住不放（系统自动重复）时的节奏，
   仍然只有离屏证据。系统重复键会不会一次建两个节点，需要在真实窗口里按一次确认。
2. **实机微软拼音仍未核对**（全员共用缺口）：组合期 `Tab` / `Enter` 归 IME 这一点只有离屏脚本证据。
3. **`Esc` 收回节点会留两步撤销**：建节点一条 `Command`、收回一条 `Command`，
   `Esc` 之后按 `Ctrl+Z` 会先「取消收回」（把空节点变回来），再按一次才撤销建节点。
   这符合「一切走 Command」的硬约束，但撤销栈里的步数比用户直觉多一步，记为 M8（撤销节奏）的输入。
4. **只拍 ppp 1.0**：150% / 200% 下编辑环与盒子是否仍然对齐没拍。
5. `in_view` 只看编辑器矩形，不看候选窗（IME 候选窗位置由 `set_ime_cursor_area` 决定，离屏拍不到）。
