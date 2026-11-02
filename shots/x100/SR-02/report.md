# SR-02 · 壳层生命周期：多标签守卫 / 关窗 / 恢复点

> 工作包：SR-02（壳层生命周期收口）
> 写入范围：`crates/mw-shell/src/{main.rs, session.rs, autosave.rs, file_guard.rs, history.rs,
> app_methods_1.rs, app_methods_2.rs, app_methods_3.rs}`
> 状态：**已实现**（机器门绿：`cargo build` / `cargo test -p mw-shell --bins` 588 通过 0 失败 /
> clippy 零新增；人工门未开）

---

## 1. 范围

七个已审计的生命周期缺陷，全部落在「文件守卫 / 保存 / 关窗 / 恢复点」这条链上：

| 编号 | 缺陷 | 严重度 |
| --- | --- | --- |
| 1 | `file_lock` / `disk_print` / `read_only` 是**窗口级**字段，但文档是**每标签**的 | 高（会丢稿） |
| 2 | 关窗只检查**活动标签**脏不脏 | 高（后台标签静默丢稿） |
| 3 | `on_exit` 既不补恢复点，也不冲卡片去抖队列 | 中（最后 800ms 的草稿丢） |
| 4 | 启动恢复走 `Session::open`，从不看 `pending_for`，崩溃恢复在主路径上不生效 | 高（崩溃保护形同虚设） |
| 5 | `after_saved()` 少两个调用点：「覆盖保存」「历史版本恢复」 | 中（被覆盖的那版永不进 `.history`） |
| 6 | 自动保存失败完全静默 | 中（用户以为有崩溃保护） |
| 7 | 提交即记账：写盘结果没回来就先记「已保存」 | 中（该修订号此后再不重投） |

## 2. 非目标

- 不动 `canvas*.rs` / `canvas/*`（另一位同事）、`inspector*.rs` / `outline.rs`（第三位）、
  `crates/mw-format/*` / `crates/mw-core/*`。
- 不改 `.zmind` 格式、`Node` / `NoteCard` 字段、`FORMAT_VERSION`。
- 不加任何新依赖；不引入新的提示机制（复用既有 `Toast` / `set_toast` 与状态栏）。
- 不重做保存的三阶段状态机（`SavePhase`），只在它的入口/出口补遗漏。
- 不做真实多进程 `.lock` 争抢的人工实测（无法在本机自动化，记入风险）。

## 3. 逐条修法与落点

### 3.1 问题 1：守卫字段搬进 `Tab`

**现状**：`Shell` 持有 `file_lock` / `disk_print` / `read_only`（`main.rs` 结构体），
而 `struct Tab` 只有 `session` / `camera` / `sel_ids`。`load_active`（`app_methods_1.rs`）
只重置 `save_phase`，**不刷新**这三项。

**后果链**（已复核代码路径）：打开 A → 打开 B（三项被 B 覆盖）→ 切回 A →
`Ctrl+S` 拿 B 的指纹比 A 的文件 → 弹假「文件已被外部修改」→ 在「重新加载」上点击 →
`conflict_ui` 走 `start_open` → `load_open`，**绕开唯一那道脏检查**（它只在 `request()` 里）→
A 的未保存改动被静默丢弃。

**修法**：
1. 新增 `TabGuard { path, lock, print, read_only }`，作为 `Tab` 的第四个字段。
   `TabGuard::arm(path)` 负责「路径换了就放掉旧锁 → 没锁就取锁 → 重取指纹 → 判只读」。
2. `load_active` 对**目标标签**调 `arm`：切回来时指纹/只读都是这一份文档的。
3. `Shell` 的三个窗口级字段退役，改为 `guard()` / `guard_mut()` 访问活动标签的守卫；
   所有读写点（`action_save`、`pump_save`、`action_save_as`、`load_open`、`conflict_ui`）改口。
4. 「重新加载」补脏检查：新增 `PendingAction::Reload(PathBuf)`，脏就走既有的
   `discard_prompt`（先保存 / 直接继续 / 取消），干净才真的重载。

**验收场景**：
- A、B 两份文档各自脏/各自只读，来回切标签后 `Ctrl+S` 不再弹假冲突。
- 在有未保存改动时点「重新加载」，必须先问，不允许直接换内容。
- 未命名标签不留陈旧锁；另存为换路径后旧 `.lock` 立即消失。

### 3.2 问题 2：关窗检查所有标签

**现状**：`guard_close` 只看 `canvas.session().dirty()`；`close_prompt` 的「保存并关闭」
只 `action_save()` 一次（活动标签）。

**修法**：
1. 抽出纯函数 `dirty_tab_names(&[(String, bool)]) -> Vec<String>`（保序），
   `guard_close` 用它判断「任一标签脏」。
2. 抽出 `save_background_dirty_tabs(&mut [Tab], active) -> SaveBackground`
   逐个把后台脏标签写回自己的路径，并刷新它的守卫指纹。
3. `close_prompt`：先存后台标签，再存活动标签；任一失败就**不关窗**并说明失败者；
   未命名脏标签无法非交互落盘，改为「已写恢复点，Ctrl+S 可写回」的明示。

**验收场景**：A 脏（后台）+ B 干净（前台），关窗必须提示；「保存并关闭」后 A 的正本在盘上。

### 3.3 问题 3：`on_exit` 补恢复点 + 冲卡片去抖

**修法**：
1. `flush_recovery_points_on_exit()`：对每个脏标签补提交一次恢复点（判据抽成纯函数
   `needs_recovery_submit(dirty, rev, last_confirmed)`），随后 `settle`。
2. `flush_card_draft_on_exit()`：卡片页去抖是 800ms，退出前把还压着的草稿落盘。
   脏判定不复制视图层逻辑：拿草稿与「库里/文档里那张卡」的已存内容比
   （`NoteStore::get` / `doc.cards()`，都是公开 API）。

**验收场景**：改完立刻关窗（<800ms）再启动 → 恢复提示里有最后的改动。

### 3.4 问题 4：启动恢复查恢复点

**修法**：抽出 `restore_tab_session(path) -> Option<(Session, Option<PathBuf>)>`：
有比正本新的恢复点时用它 `Session::open_recovered`（内容进内存、`dirty()` 为真），
并回传槽位；`restore_tabs` 用它装每个标签，活动标签把槽位交给既有恢复提示，
后台标签用一条提示说明「已按恢复点打开，Ctrl+S 写回正本」。

**验收场景**：正本 + 更新恢复点并存时，启动不会用旧正本盖掉恢复点内容。

### 3.5 问题 5：`after_saved()` 补齐调用点

**修法**：`conflict_ui` 的「覆盖保存」与版本历史恢复在写盘成功后都走 `after_saved()`
（清恢复点 + 记基线 + 刷最近 + 收 `.history` + 提示）。

### 3.6 问题 6：自动保存失败要说出来

**修法**：
1. `Outcome::Failed` 带上 `reason`（`autosave.rs` 内改，属本包范围）。
2. 失败弹一次 `bad` 提示条（同一槽位只在第一次失败时提示，不刷屏）。
3. 退避重试：`autosave_backoff(attempts)`（15s → 30s → 60s → 120s → 240s，封顶 300s），
   连败 5 次后**停止自动重试**，等下一次真实编辑（修订号变化）再恢复。

**验收场景**：把恢复点目录设为不可写 → 3 秒后出现「自动保存失败」提示，5 次后不再打磁盘。

### 3.7 问题 7：写成功后才记账

**修法**：把「已提交」与「已确认」分成两张表：
- `last_autosave_rev`：**只在** `Outcome::Wrote` 与正本保存后写入（确认基线）。
- `autosave_submitted`：已提交待收尾；收尾（成功/失败）即删。
- 每帧用 `Background::is_pending(slot)` 核对，**被 `invalidate`/作废掉的提交一律清账**，
  下一轮可重投。

判据抽成纯函数 `autosave_wants(dirty, rev, confirmed, submitted, stalled)`。

## 4. 新增单元测试（15 条，全部实测通过）

| 测试 | 钉住什么 |
| --- | --- |
| `tab_guards_do_not_cross_wire` | 两个 `Tab` 各自的指纹/只读互不串台（问题 1 的反例） |
| `tab_guard_takes_over_when_other_holder_leaves` | 另一个实例放锁后切回来能重新拿到 |
| `tab_guard_releases_lock_when_path_changes` | 另存为换路径后旧 `.lock` 立即消失、未命名不留锁 |
| `dirty_tab_names_covers_background_tabs` | 关窗脏检查覆盖全部标签（老代码只看活动标签） |
| `save_background_dirty_tabs_writes_each_path` | 「保存并关闭」把后台标签逐个写盘；未命名如实报 `untitled` |
| `restore_tab_session_prefers_newer_recovery_point` | 启动恢复优先用更新的恢复点、且落成脏会话 |
| `restore_tab_session_uses_plain_file_without_recovery_point` | 无恢复点时行为不变；读不到返回 `None` |
| `every_save_success_path_calls_after_saved` | 四个写盘成功点都调 `after_saved()`（源码次序 + 窗口扫描） |
| `after_saved_is_idempotent_for_same_revision` | 同文件同修订号重复调用不重写快照（幂等闸门） |
| `autosave_backoff_grows_and_caps` | 退避阶梯 15/30/60/120/240 → 封顶 300s，单调不减 |
| `autosave_may_retry_resets_on_new_revision` | 退避到点/修订号变化/连败放弃三种判定 |
| `autosave_wants_skips_confirmed_and_in_flight` | 提交记账与确认记账分离（问题 7 判据） |
| `needs_recovery_submit_only_for_dirty_unconfirmed` | 退出补恢复点的判据 |
| `draft_differs_ignores_tag_order_but_catches_content` | 卡片草稿脏判定：标签按集合比、全空白不算内容 |
| `autosave::tests::is_pending_tracks_queued_and_in_flight_writes` | `Background::is_pending` 在提交后为真、写完后为假 |

## 5. 实测证据（2026-10-30 收工）

三条命令严格串行执行（不与其他 cargo 并发），工作目录 `C:\Users\douha\Pictures\思维导图rust`：

| 命令 | 结果 |
| --- | --- |
| `cargo build -p mw-shell` | `Finished dev profile` —— 无错误、无告警 |
| `TEMP/TMP=$(pwd)/target/tmp cargo test -p mw-shell --bins` | **`588 passed; 0 failed; 1 ignored`**（6.17s） |
| `cargo clippy -p mw-shell --all-targets` | 我改动的 9 个文件（`main.rs`/`app_methods_1..3`/`autosave.rs`/`history_ui.rs`/`session.rs`/`file_guard.rs`/`history.rs`）**零告警**；仓库其余告警都在 `mw-core`/`mw-render`/`export_pdf.rs`/`canvas/tests.rs`，属既有基线或他人范围 |

新增 15 条单测（`sr02_tests` 14 条 + `autosave::tests` 1 条），全部在 `--bins` 里跑通：

```
sr02_tests::tab_guards_do_not_cross_wire ... ok
sr02_tests::tab_guard_takes_over_when_other_holder_leaves ... ok
sr02_tests::tab_guard_releases_lock_when_path_changes ... ok
sr02_tests::dirty_tab_names_covers_background_tabs ... ok
sr02_tests::save_background_dirty_tabs_writes_each_path ... ok
sr02_tests::restore_tab_session_prefers_newer_recovery_point ... ok
sr02_tests::restore_tab_session_uses_plain_file_without_recovery_point ... ok
sr02_tests::every_save_success_path_calls_after_saved ... ok
sr02_tests::after_saved_is_idempotent_for_same_revision ... ok
sr02_tests::autosave_backoff_grows_and_caps ... ok
sr02_tests::autosave_may_retry_resets_on_new_revision ... ok
sr02_tests::autosave_wants_skips_confirmed_and_in_flight ... ok
sr02_tests::needs_recovery_submit_only_for_dirty_unconfirmed ... ok
sr02_tests::draft_differs_ignores_tag_order_but_catches_content ... ok
autosave::tests::is_pending_tracks_queued_and_in_flight_writes ... ok
```

`test result: ok. 14 passed; 0 failed` / `test result: ok. 10 passed; 0 failed`（autosave 模块全量）。

**命令口径更正**：`crates/mw-shell` 是纯 bin crate，没有 lib target，
任务书里的 `cargo test -p mw-shell --lib` 会直接报 `error: no library targets found in package mw-shell`。
实际使用 `--bins`。

## 6. 落点与实测（收工时回填）

> 行号是 2026-10-30 收工时的快照，同时写出那一行长什么样；行号漂了按内容找。

### 6.1 逐条落点

**问题 1（守卫每标签化）**

| 落点 | 那一行长什么样 |
| --- | --- |
| `main.rs:739` `pub(crate) struct TabGuard` | `pub(crate) struct TabGuard {` —— `path` / `lock` / `print` / `read_only` 四个字段 + `arm` / `refresh_print` / `release` |
| `main.rs:512` | `autosave_submitted: std::collections::HashMap<std::path::PathBuf, u64>,` —— 问题 7 的「已提交」账 |
| `main.rs:521` / `main.rs:523` | `allow_replace: bool,` / `last_finish: Option<(std::path::PathBuf, u64)>,` |
| `app_methods_1.rs:1675` `load_active` | 换完会话后 `self.arm_active_guard();` —— 切页时守卫跟着换 |
| `app_methods_1.rs:1699` `arm_active_guard` | 读 `canvas.session().path` 再 `tab.guard.arm(...)` |
| `app_methods_1.rs:1713` `guard_mut` / `:1718` `active_read_only` / `:1723` `active_print` | 全部读写点改口的三个口子 |
| `app_methods_2.rs:2469` `action_save` | 原来的 `if self.read_only` → `if self.active_read_only()` |
| `app_methods_2.rs:2503` `pump_save` | 原来的 `self.disk_print = fingerprint(&path)` → `g.refresh_print()` |
| `app_methods_2.rs:2536` `action_save_as` | 原来的取锁/取指纹/判只读三行 → `g.arm(Some(&path))` |
| `app_methods_2.rs:3222` `load_open` | 读盘成功后 `g.arm(Some(path))` |
| `app_methods_2.rs:3135` `request_reload`（新） | 「重新加载」的脏检查入口 |
| `app_methods_2.rs:3147` `run` | `self.allow_replace = true;` + `PendingAction::Reload` 分支（先 `guard.release()`） |

**问题 1 的丢稿通道（重点）**：脏检查**下沉到 `load_open`**（`app_methods_2.rs:3222` 首行
`let allowed = std::mem::replace(&mut self.allow_replace, false);`）。任何换掉当前文档的路径都要经过它；
`allow_replace` 是一次性放行票，由 `run()`（= 用户已在 `discard_prompt` 里确认）签发、`load_open` 消费，
所以接管道之后不会自锁成死循环。

**问题 2（关窗看全部标签）**

| 落点 | 那一行长什么样 |
| --- | --- |
| `main.rs:812` `dirty_tab_names` | `fn dirty_tab_names(tabs: &[(String, bool)]) -> Vec<String>` |
| `main.rs:835` `save_background_dirty_tabs` | `fn save_background_dirty_tabs(tabs: &mut [Tab], active: usize) -> SaveBackground` |
| `app_methods_3.rs:1085` `guard_close` | 原来的 `canvas...session().dirty()` → `dirty_tab_names(&self.tab_dirty_snapshot())` |
| `app_methods_3.rs:1098` `tab_dirty_snapshot`（新） | 活动标签取画布、其余取各自 `Tab` |
| `app_methods_3.rs:1113` `close_prompt` | 「保存并关闭」先 `save_background_dirty_tabs(...)`、再存活动标签；任一失败就**不关窗** |

**问题 3（退出收尾）**

| 落点 | 那一行长什么样 |
| --- | --- |
| `app_methods_3.rs:1180` `flush_recovery_points_on_exit`（新） | 按 `needs_recovery_submit` 逐标签补提交 |
| `app_methods_3.rs:1224` `flush_card_draft_on_exit`（新） | 读 `draft_*` → 比 `NoteStore::get` / `doc.cards()` → `save_card_from_reader` |
| `main.rs:926` `draft_differs` | 纯判定；标签按集合比（顺序不同不算改动） |
| `app_methods_1.rs` `on_exit` | 先 `flush_card_draft_on_exit()`、再 `flush_recovery_points_on_exit()`，然后 `autosave.settle(1500ms)` |

**问题 4（启动查恢复点）**

| 落点 | 那一行长什么样 |
| --- | --- |
| `main.rs:950` `restore_tab_session` | `fn restore_tab_session(path: &Path) -> Option<(Session, Option<PathBuf>)>` |
| `app_methods_1.rs:1780` `restore_tabs` | 去重路径 → `restore_tab_session` → 活动标签把槽位交给既有恢复提示 |

**问题 5（`after_saved` 补点 + 幂等）**

| 落点 | 那一行长什么样 |
| --- | --- |
| `app_methods_2.rs:2443` `after_saved` | 开头 `let last = self.last_finish.as_ref().map(...)` + `save_finish_is_new` 闸门 |
| `main.rs:913` `save_finish_is_new` | `fn save_finish_is_new(last: Option<(&Path, u64)>, path: &Path, rev: u64) -> bool` |
| `app_methods_1.rs:2444` `conflict_ui` | 覆盖保存成功后 `self.after_saved();` |
| `history_ui.rs:70` / `:74` | `g.refresh_print();` 与 `self.after_saved();` —— **写入范围经 lead 追加授权**，只动这两行（连同其注释） |

**问题 6（失败可见 + 退避）**

| 落点 | 那一行长什么样 |
| --- | --- |
| `autosave.rs:242` | `Failed { slot, revision, reason }`（原来只有 `slot`） |
| `autosave.rs:458` `write_snapshot` | 返回 `Result<(), String>`（原因要能说给用户听） |
| `main.rs:866` `AutosaveStall` / `:876` `AUTOSAVE_GIVE_UP` / `:882` `autosave_backoff` / `:892` `autosave_may_retry` | 退避阶梯与"放弃"判据 |
| `app_methods_3.rs:963` `autosave_tick` | `Outcome::Failed { .. }` 分支：首次失败弹一次 `bad` 提示条 |

**问题 7（确认后才记账）**

| 落点 | 那一行长什么样 |
| --- | --- |
| `main.rs:904` `autosave_wants` | `fn autosave_wants(dirty, rev, confirmed, submitted) -> bool` |
| `autosave.rs:313` `is_pending` | 队列/在途是否还有这个槽位的写 |
| `app_methods_3.rs:963` `autosave_tick` | 提交只写 `autosave_submitted`；`Outcome::Wrote` 才写 `last_autosave_rev`；每轮按 `is_pending` 清孤儿账 |

### 6.2 `after_saved()` 幂等性证据（lead 提醒 ①）

`after_saved` 现在有四个入口，重复调用必须无害。逐项说明：

1. **闸门本身**：`last_finish: Option<(PathBuf, u64)>` + 纯函数 `save_finish_is_new`。
   同一文件同一修订号第二次进来直接 `return`，并留痕 `trace::event("after_saved_skip dup=1")`。
   单测 `after_saved_is_idempotent_for_same_revision` 钉住四种情形（首次 / 同修订号 / 修订号涨 / 换文件）。
2. **`history::capture`**：这是唯一"不幂等"的副作用（它每次都新建一份快照）。幂等由闸门保证 ——
   同一修订号不会走到它。补充一点实测语义：历史恢复那条路上 `history::restore` 内部先
   `capture` 了一次**恢复前**的内容，随后 `after_saved` 的 `capture` 拿到的是 `.bak`（= 恢复时被顶掉的
   那一版），**两份内容不同**，不是重复快照，而是"恢复前 / 被顶掉"两个版本，对用户有价值，故不去重。
3. **`recent::touch`**：`retain(same_file)` 之后 `insert(0)`，按路径天然只有一条，重复调用不产生重复项。
4. **`autosave.invalidate`**：`remove_file` + 队列剔除 + 记 `stale`，重复调用是幂等的空操作。
5. **`note_session_rev`**：纯 map 写入。
6. **`set_toast`**：同一句话覆盖同一个提示条，不叠加。

结论：`after_saved` 重复调用 = 一次免费的 trace + 覆盖同一条提示，不产生第二份 `.history` 快照、
不产生重复的最近文件、不改可见状态。

### 6.3 一个刻意的取舍（后台标签的恢复点）

`close_prompt` 的「保存并关闭」里，**未命名**的脏标签无法非交互落盘（会弹「另存为」对话框），
所以对它只写恢复点并明示「已写入恢复点」，而不是偷偷丢内容、也不是在关窗流程里弹文件对话框。
代价：恢复点在 `<AppData>` 下、下次启动出现在恢复提示里，而不是变成用户指定的文件。这是有意的：
关窗流程里弹系统文件对话框是更糟的体验（而且它与 `hwnd` 的模态关系在 eframe 里不稳）。

### 6.4 未做 / 风险

- **没做真实多进程 `.lock` 争抢的人工实测**：单测里用同进程两次 `Guard::try_acquire` 覆盖了
  `create_new` 语义，但"两个真实例"只做了代码审读，未开第二个窗口实测（本机自动化成本高）。
- `allow_replace` 是**一次性票**：如果将来有人新增"换文档"的入口却忘了走 `run()`，用户确认后仍会被
  `load_open` 再问一次（不会丢数据，只是多问一次）。这是有意的失败方向（宁可多问）。
- 卡片草稿的脏判定用公开字段比对，**与视图层 `is_dirty()` 不完全等价**：视图层可能还考虑
  光标位置/IME 组合态。极端情形下退出时可能多写一次卡片（`updated_at` 变），不会少写。
- 未命名标签的恢复点写在 `appdata_dir()` 下，`prune` 的保留策略沿用既有实现（未改）。
- `crates/mw-shell` 是**纯 bin crate**，没有 lib target，任务书里的 `cargo test -p mw-shell --lib`
  会直接 `error: no library targets found`；实际用 `cargo test -p mw-shell --bins`。

