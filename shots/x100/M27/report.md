# M27 · 快捷键禁用与配置持久化收口（打磨轮 50）

- 日期：2026-10-31
- 依据：`docs/02` §10 的两行口径（「快捷键可在设置中查看全部并禁用单项冲突项
  （如被输入法占用）；禁用状态持久化于配置」＋脚注⑥「快捷键配置与冲突禁用未做」）；
  `crates/mw-shell/src/shortcuts.rs` 的 `Bindings` / `Dialog`；
  `docs/13` §4.39–§4.41 的「快捷键配置仍是 M2c 唯一未做项」。
- 状态：**已实现**（机器门绿；人工门未开）

## 1. 现状（代码复核，先于改动）

`shortcuts.rs` 已经做到的（比 `docs/02` 脚注⑥ 写的多）：

- `Bindings::load()` 从 `%APPDATA%\织思\shortcuts.json` 读覆盖表，缺项/坏项/未知项
  都退回出厂值（`load_from`，含 UTF-8 BOM 兼容）；
- `Bindings::set()` 在绑定时检查冲突，同一和弦被别的动作占用时返回占用者、不落盘；
- 设置面板 `Dialog`（`Ctrl+,`）列出 28 个窗口级动作，点「改」后捕获下一个组合键，
  `Esc` 取消；「恢复默认」清空全部覆盖；
- 菜单 / 命令面板的提示走 `hint()`，实际按键走 `consume()`，两者都只认当前绑定。

**缺的是「禁用」**：

1. `Bindings.entries` 是 `Vec<(Action, Chord)>`，没有「未绑定」状态；
2. 面板每行只有「改」，被输入法占用的键只能改成另一个键，不能彻底关掉；
3. `shortcuts.json` 无法表达未绑定——手写 `"save": "none"` 会被当成坏条目，
   静默退回 `Ctrl+S`，等于「禁用」在配置层也不存在；
4. `docs/02` §10 脚注⑥ 把「查看 / 改绑 / 冲突检测 / 持久化」也算成未做，
   与代码不符；真正未做的只有「禁用单项」这一条。

这不是一个假想缺口：`docs/02` 的规格行从 10-18 起就写着「禁用单项冲突项」，
面板却只有「改」。本轮把最后一条补上，并把文档口径改回与代码一致。

## 2. 范围

1. **绑定表支持未绑定**：`Bindings.entries: Vec<(Action, Option<Chord>)>`。
   `chord()` / `hint()` / `consume()` 对 `None` 分别返回 `None` / 空串 / `false`；
   `entries()` 仍按 `DEFAULTS` 顺序返回全部 28 项（含禁用项），面板据此画行。
2. **禁用 / 启用**：
   - `disable(action) -> bool`：已禁用返回 `false`，否则置 `None`；
   - `enable(action) -> Result<(), Action>`：恢复该动作的**出厂和弦**；若该和弦
     已被别的**启用中**动作占用，返回占用者、保持禁用，由面板提示先改占用者。
3. **冲突口径**：只在启用中的和弦之间判重。禁用项不再占位，
   它的旧和弦可以被别的动作绑走；`set(action, chord)` 会顺带启用该动作。
4. **持久化**：`save_to` 把 `None` 写成 `"none"`；`load_from` 把 `"none"`
   与空串读成 `None`，未知值/坏和弦仍退回出厂值；重复的启用和弦仍「先到先得」。
   `reset()` 恢复全部出厂绑定（含重新启用全部）。
5. **面板**：
   - 启用行：和弦 + 「改」+「禁用」；
   - 禁用行：`未绑定`（弱化色）+ 「改」+「启用」；
   - 禁用 / 启用 / 改绑成功 / 冲突都在面板下方给一句话；
   - 面板文字补一句「被输入法占用时可禁用，不必改到别的键」。
6. **轨迹**：`shortcut set action=… chord=…`、`shortcut reject action=… owner=…`、
   `shortcut disable action=…`、`shortcut enable action=…`、
   `shortcut enable_reject action=… owner=…`、`shortcut reset`。
7. **验收旋钮**：`MW_SHORTCUTS_DEMO=1` 启动即打开面板（只写内存）；
   `MW_SHORTCUTS_DEMO=save,undo,redo` 同时禁用列出的动作 id，给金样与行为脚本
   一个确定性靶子。不落盘、不影响正常启动。
8. **单测**（`shortcuts.rs`）：禁用释放和弦并可往返持久化；启用遇到占用要报占用者、
   解除后可恢复出厂键；`"none"` / 空串读成禁用、坏值仍退回默认。
9. **金样**：`shortcuts-default`（面板原样）与 `shortcuts-disabled`
   （`save,undo,redo` 三行显示未绑定/启用），进 `stage-b` 门。
10. **文档**：改掉 `docs/02` §10 脚注⑥ 的过期结论；新增账本 M27 行；
    `docs/STATUS.md`、执行日志、`AGENTS.md` 回填。

落地位置：`crates/mw-shell/src/shortcuts.rs`（绑定表 / 面板 / 单测）、
`crates/mw-shell/src/app_methods_1.rs`（验收旋钮）、
`shots/golden/inputs/` 与 `scripts/x100-stage-b-gate.ps1`（两场金样）。

## 3. 非目标

- **不改画布编辑键**：`Tab` / `Enter` / `Del` / 方向键等仍由画布手势语义固定，
  不进入这张可重绑表（`shortcuts.rs` 文件头的既有取舍）；F1 速查仍可查看。
- 不改任何出厂和弦、不调整动作判别顺序（`Ctrl+Shift+N` 仍先于 `Ctrl+N`）。
- 不做多套快捷键方案、导入导出、每文档独立绑定。
- 不改 `Node` / `.zmind` 字段，不加依赖，不动画布与检查器。
- 不顺手重构 `shortcuts.json` 的目录 / 文件名 / 格式版本。

## 4. 验收条款

### 4.1 单元（`mw-shell`，`shortcuts.rs`）

| # | 用例 | 期望 |
| --- | --- | --- |
| 1 | `disable(Save)` 两次 | 第一次 `true`、第二次 `false`；`chord(Save) == None`；`hint(Save) == ""` |
| 2 | 禁用后把 `Ctrl+S` 绑给 `Open` | `set` 成功（禁用项不占位） |
| 3 | 禁用 + 改绑后 `save_to` → `load_from` | `Save` 仍禁用、`Open` 仍是 `Ctrl+S`；条目数仍是 28 |
| 4 | 禁用 `Save`，`Open` 占用 `Ctrl+S` 后 `enable(Save)` | 返回 `Err(Open)`，`Save` 仍禁用 |
| 5 | 把 `Open` 改成 `Ctrl+Alt+O` 后再 `enable(Save)` | 返回 `Ok(())`，`Save` 回到 `Ctrl+S` |
| 6 | `"save":"none"` / `"undo":""` / `"open":"Ctrl+"` | 前两个禁用，`Open` 退回默认 `Ctrl+O` |
| 7 | `reset()` 之后 | 全部 28 项启用，出厂和弦无重复 |

### 4.2 端到端（轨迹 + 金样）

- `MW_SHORTCUTS_DEMO=1` 打开面板：金样 `shortcuts-default`，28 行全部是
  和弦 + 「改」+「禁用」。
- `MW_SHORTCUTS_DEMO=save,undo,redo`：金样 `shortcuts-disabled`，
  三行显示 `未绑定` + 「启用」，其余行不变；两张基线的差异只落在
  这三行的和弦列与按钮列（实测 3 557 px，包围盒 `(815,379)–(1075,698)`）。
- 行为脚本（ppp 1.5，隔离 APPDATA）：点第三行「打开」的「禁用」→
  `shortcut disable action=open`，`shortcuts.json` 写入 `"open": "none"`；
  **重启进程**后同一面板显示「未绑定/启用」，再点「启用」→
  `shortcut enable action=open`，配置回到 `"open": "Ctrl+O"`。
- 冲突路径由单测覆盖（`enable` 返回占用者、保持禁用），不另拍图。
- 既有基线零漂移：面板不在任何现有场景里。

### 4.3 证据

`shots/x100/M27/`：`before.png`（改前面板，只有「改」）、
`after-default.png`（改后默认态）、`after-disabled.png`（三行禁用态）、
`trace-after-disable.txt` / `trace-after-enable.txt`、报告与 `manifest.json`；
金样在 `shots/golden/shortcuts-default/` 与 `shots/golden/shortcuts-disabled/`。

### 4.4 实测（2026-10-31）

- 单测：新增 `a_disabled_shortcut_frees_its_chord_and_survives_a_round_trip`、
  `enabling_restores_the_factory_key_unless_it_is_taken`、
  `none_and_blank_values_disable_while_broken_values_fall_back` 3 条；
  全量 `cargo test --workspace -- --test-threads=1` **1076 passed / 1 ignored / 0 failed**。
- 端到端：`shortcut disable action=open` → `"open": "none"` 落盘；
  重启后加载为禁用，点「启用」→ `shortcut enable action=open` → `"open": "Ctrl+O"`。
- 像素账（1620×1080，逐像素零容差）：改前→改后默认态差 **45 504 px**，
  包围盒 `(445,123)–(1150,937)`（说明文案两行 + 新增「禁用」列 + 行位置随之微移）；
  默认态↔禁用态差 **3 557 px**，包围盒 `(815,379)–(1075,698)`，
  只在「保存 / 撤销 / 重做」三行的和弦与按钮列。
- 两场新基线 `--tol 0` 各连跑两遍零差异；sha256-12：
  `shortcuts-default` = `F76BCA820852`、`shortcuts-disabled` = `E32C45210BE9`。
- 机器门：`stage-b` **138 场 / 276 次比较**（2026-10-31 `12:41:58–12:51:06`，两遍 `--tol 0`，
  含 fmt / mw-shell clippy 子集 / 全量串行测试 1076），既有 136 场零净漂移；`stage-cm`
  **58 场 / 116 次比较**（`12:51:57–12:55:03`）随后顺序复跑，两遍全过（见 `docs/STATUS.md` §1）。

## 5. 风险 / 未做（收口实测）

- `shortcuts.json` 的 `"none"` 是一个新的取值约定；老版本读到会当坏条目退回默认，
  新版本读老配置不受影响（缺键 = 出厂值）。不做迁移，也没有格式版本号。
- 禁用后旧和弦不保留：重新启用恢复的是**出厂键**，不是禁用前的自定义键；
  面板开头第一句已写明「重新启用恢复出厂键」，避免用户误以为只是临时关掉。
- 画布编辑键仍不可配置（文件头的既有取舍），设置面板里也不列；
  想查这些键看 `F1` 速查浮层。
- 实机输入法占用（微软拼音）仍无法在本机自动核对；本轮只把「有路可走」补齐。
- 两场金样是静态面板；按钮点击路径靠行为脚本与轨迹，不靠像素猜测。
- 行为脚本点的是第三行「打开」，不是报告初稿写的「保存」；两条走同一段
  禁用/启用代码，禁用态金样另外覆盖了「保存 / 撤销 / 重做」三行。
