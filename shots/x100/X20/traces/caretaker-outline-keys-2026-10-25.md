# X20 门后维护 ⑪ · 模态与菜单下大纲键盘隔离（2026-10-25）

范围：`crates/mw-shell/src/panels/outline.rs`、`app_methods_2.rs`、`main.rs`。
不改文档模型、不改外观、不新增依赖/线程/crate，不进入 Stage C。X20 人工门
仍未确认，本项随 X20 一并验收。

## 缺口

全屏大纲自绘整列表，键盘直接读 `ui.input` / `consume_key`，不经过 egui
焦点系统。`egui::Modal` 只挡指针与 Tab 巡游，标题栏菜单是 Memory 弹层，
两者都挡不住这些裸键。

修复前实拍（1080×720 逻辑 / ppp 1.5 / 1620×1080；`MW_OUTLINE=1
MW_OUTLINE_FOCUS=1 MW_DIALOG_DEMO=1`，脚本
`caretaker-outline-keys-prefix-modal-input-2026-10-25.txt`，26 帧）：

| 输入 | trace | 文档后果 |
| --- | --- | --- |
| `Delete` | `outline_delete id=01a1706e-…bf221f` | 删除聚焦行「纯 Rust 原生」 |
| `Enter` | `outline_add_sibling anchor=…bf221f new=Some(…4afc35dd8099)` → `outline_edit_start` | 新增空同级并进入行内编辑 |
| `Tab` | `outline_focus_target …4afc35dd8099` → `outline_indent id=…4afc35dd8099` | 新行缩进 |

三下都发生在暗幕背后，用户看不到选择变化；删除/缩进落到不可见的目标上。
这与 X17 / 2.9 / 2.10 定下的「前台浮层是第一键盘主人」冲突。

## 修复

- 新增纯函数 `panels::outline::row_keys_owned(blocked, editing, filter_owned)`：
  任一为真时行表不收键。单测
  `row_keys_are_owned_only_without_foreground_or_field` 覆盖门开、行内编辑、
  搜索框、三者皆无四种组合。
- `outline::show` 新增 `blocked: bool`；`outline_screen` 由 `main.rs` 传入
  已有的 `input_blocked`（`demo || dialog_demo || overlay.blocks_canvas()
  || tab_drag || Popup::is_any_open(&ctx)`，与标签条 `foreground`、画布手势
  同一条谓词，不新增第二套判定）。
- 键盘处理链头部插入 `if blocked { … }`，整条 `if/else if` 全部跳过；门开时
  只发 `outline_key_blocked key=<K>` trace，不改选择、不进入编辑、不缩进/
  删除。搜索框 Esc 回焦也加 `!blocked`，避免从弹窗里抢焦点。
- 门开时不抢行焦点、不移动焦点；门关后原行焦点保持，下一下按键照常落回。

## 修复后证据

### 模态（`MW_DIALOG_DEMO=1`，脚本 `shots/golden/inputs/outline-modal-input.txt`，24 帧）

`wait 8` + `Delete` + `Tab` + `Home`（修复后 `Enter` 会落到弹窗默认「确定」并
关闭弹窗，证据里不再用它；原 26 帧脚本保留在 `caretaker-outline-keys-prefix-modal-input-2026-10-25.txt`）。

trace（`caretaker-outline-keys-after-modal-2026-10-25.trace`）：

```
event golden_input 9 key Delete
event outline_key_blocked key=Delete
event golden_input 13 key Tab
event outline_key_blocked key=Tab
focus id d8ad0c7df674cb33          <- Tab 由弹窗自己巡游
focus dialog close                 <- 仍是弹窗内部焦点
event golden_input 17 key Home
event outline_key_blocked key=Home
```

- 无 `outline_delete` / `outline_add_sibling` / `outline_indent` / `outline_edit_start`。
- 实拍 `caretaker-outline-keys-after-modal-2026-10-25.png`（1620×1080 /
  156,211 B / SHA256 `2e51e8e1…d912d61d1`）：弹窗保持打开且默认按钮仍在，
  大纲完整显示「纯 Rust 原生 / winit + windows-rs / wgpu 四层合成器」，
  页脚 `聚焦 3 / 全图 13`，缩进层级未变。
- 修复前同场景 `caretaker-outline-keys-before-modal-2026-10-25.png`
  （169,548 B / `1da1b47a…adc21d9`）是删除 + 新增 + 缩进后的文档状态。
  前后差异 201,453 px = 11.514%（最大通道差 165，
  `bbox=(47,27,1572,1068)`），差异来自列表内容与行数，不是外观漂移；
  「外观不变」由 `outline` / `outline-focus` / `outline-compact` 三组
  golden 逐字节一致证明（见阶段门）。

### 标题栏菜单

- `caretaker-outline-keys-menu-open-2026-10-25.png`（1620×1080 /
  146,684 B / `ada89176…dad2f0c`）：1080×720 下点击 (115,27) 后「文件」
  菜单确实展开（新建/新建标签页/打开…/导入/保存/另存为/导出/开始页/退出）。
- 同场景按 `Delete`（脚本 `shots/golden/inputs/outline-menu-input.txt`，
  23 帧）→ `caretaker-outline-keys-after-menu-2026-10-25.trace` 仅有
  `outline_key_blocked key=Delete`，无 `outline_delete`；菜单由自己的
  `PopupNav` 消费该键并收起，大纲 3/13 行保持不变
  （`caretaker-outline-keys-after-menu-2026-10-25.png`，119,369 B /
  `96487082…504e26a9`）。菜单展开与收起两图差异 91,931 px = 5.254%
  （`bbox=(112,14,629,619)`，即菜单卡片区域）。
- 该场景未开任何弹窗/编辑/搜索，门只可能来自 `Popup::is_any_open`；
  修复后 `blocked=true` 与菜单展开图互为对照。

## 门槛

- `cargo fmt --all -- --check` → 干净。
- `cargo clippy -p mw-shell --all-targets` → 仅 `export_pdf.rs` 3 条历史告警，
  `outline.rs` / `app_methods_2.rs` / `main.rs` 新增 0
  （`caretaker-outline-keys-clippy-2026-10-25.log`）。
- `cargo test --workspace -- --test-threads=1` → **763 passed / 0 failed**
  （`caretaker-outline-keys-tests-2026-10-25.txt`；含 ⑫ 新增的日期冻结单测；
  single test: `cargo test -p mw-shell panels::outline` → 33 passed）。
- 阶段门（31 组 golden × 2 次 `--tol 0`）→ **62/62 全 0 差异**，
  `outline` / `outline-focus` / `outline-compact` 基线未被 `--update` 改写；
  证据 `caretaker-stage-b-gate-outline-keys-2026-10-25.log`。
- ⑪ 的首次阶段门在 `card-slash` 上暴露了既有的日期漂移（与大纲无关，
  本地 08:00 UTC 跨日）；按 ⑫ 冻结演示时间戳后复跑通过，见
  `caretaker-card-demo-date-2026-10-25.md`。
- release `target/release/mw-shell.exe` 23,469,056 B /
  SHA256 `a4662b1f…f5babbb862`。

## 文件

- 脚本：`shots/golden/inputs/outline-modal-input.txt`（新增）、
  `shots/golden/inputs/outline-menu-input.txt`（新增）。
- trace/图：本目录 `caretaker-outline-keys-*`。
- 代码：`panels/outline.rs`（`row_keys_owned`、`blocked` 门、trace）、
  `app_methods_2.rs`（`outline_screen(…, blocked)`）、`main.rs`（传
  `input_blocked`）。
