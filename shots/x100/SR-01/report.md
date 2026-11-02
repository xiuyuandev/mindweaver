# SR-01 · 保存格式核：fsync、`.bak` 回滚、原子导出、未知字段保真

> 任务来源：Team Lead 派发的 `task-1`（保存可靠性核 SR-01）。
> 写到哪：`crates/mw-format/src/lib.rs`、`crates/mw-shell/src/export.rs` —— **只这两个文件**；
> 本报告是任务显式要求的交付物（`shots/x100/SR-01/report.md`），不在代码写范围内。
> 状态：`已实现`（**不记 `已验证`**：掉电/磁盘满等真故障不可在本机复现，见 §7）。

## 1. 范围

`.zmind` 的**保存路径**与文本导出的**落盘路径**：现在这两条路都在「写一半就断」时留下坏文件，
而且读侧比写侧更严——一份带未来字段的文件会被旧版本读成「打不开」，或者打开了再存就把新字段吃掉。

结合 `lib.rs` 文件头那句承诺——

> 保存采用「临时文件 → 旧文件转 .bak → 更名」的崩溃安全流程：任一时点磁盘上至少存在一份完整文件（05 §6）

——这一包就是把这句承诺真的兑现。

## 2. 非目标

- **不动 `FORMAT_VERSION`、不做数据迁移**（硬约束）。
- **不改 `mw-core`**：`FileDoc` 在 `mw-format` 里，但 `Node` / `TaskState` / `ArrowHead` 在 `mw-core`。
  所以第 4/5 条（未知键保真、未知枚举值退化）**不能加 `#[serde(flatten)]` 到 `Node`、也不能加
  `#[serde(other)]` 变体**，全部在 `mw-format` 的读侧做（见 §3.4 / §3.5）。
- 不改 ZIP 结构、条目名、`manifest.json` 字段。
- 不改自动保存策略、不改恢复点（`autosave.rs` 走的是同一个 `save`，自动受益）。
- 不动 `cargo golden` / 门脚本 / 其它 crate。
- 不新增依赖（`Cargo.toml` 不动）。

## 3. 逐条修法

### 3.1 保存无 fsync（`write_atomic`）

现状：`std::fs::write(&tmp, bytes)?` 之后直接 `rename`。掉电时**rename 可能先落盘、内容后落盘**，
结果正本指向一个内容为空的 inode —— 恰好破坏了「任一时点至少一份完整文件」。

改成：

1. `File::create(&tmp)` → `write_all` → **`sync_all()`**（内容没落盘就返回错误，不再往下走）；
2. rename 流程结束后对**父目录**也 `sync_all()`；
3. 目录同步走新函数 `sync_dir()`：`File::open(dir)` 在 Windows 上对目录会失败（拿不到句柄），
   失败**一律忽略**——不能因为「同步目录做不到」就让一次成功的保存报错。

### 3.2 `.bak` 转存失败被吞且不回滚

现状：

```rust
let _ = std::fs::remove_file(&bak);
let _ = std::fs::rename(path, &bak);   // ← 吞掉了错误
```

正本 rename 失败时 `.bak` 可能是上一次的旧备份，代码照样往下 rename tmp → 正本，
一旦这一步也失败，用户看到的是「文件不见了」。

改成照抄同仓 `crates/mw-shell/src/note_store.rs` 里 `write_atomic` 的正确写法（那份实现是对的）：

- 正本 → `.bak` 的 rename **不吞错误**（`?` 直接返回；此时正本还在原地，没有需要回滚的东西）；
- tmp → 正本失败时，**先把 `.bak` 改回原名**再返回错误，并顺手删掉 tmp。
- 与 `note_store` 唯一的差别：**成功后保留 `.bak`**（本 crate 的滚动备份语义，
  `second_save_creates_backup_and_preserves_created_at` 这条既有测试盯着它）。

### 3.3 tmp 文件名固定 + 陈旧 tmp 不清理

现状：`.{file_name}.tmp` 固定名字，两个写者（自动保存 + 手动保存，或两个进程）会互撞，
撞坏的 tmp 还可能被对方 rename 成正本。

改成：

- 新函数 `tmp_path()`：`.{file_name}.{pid}.{seq}.{nanos}.tmp`，
  `seq` 是进程内 `AtomicU64` 自增，`nanos` 是纳秒尾数——同进程不同次、不同进程都不会重名。
- 新函数 `sweep_stale_tmp()`：save 前扫同目录里形如 `.{file_name}.*.tmp` 的文件，
  **只删 mtime 早于 1 小时的**（`STALE_TMP_AGE`）。带时限是刻意的：无脑删会把**另一个正在写的
  写者的 tmp** 删掉，那等于制造它刚刚要消灭的那种故障。尽力而为，任何错误都忽略。

### 3.4 降级即删字段（`FileDoc` 丢未知键）

`FileDoc` 在 `save()` 里逐字段重建，`content.json` 里本版本不认识的键**静默丢失**。
于是「新版本写了 `futureKey` → 用户用旧版本打开 → 改一个字 → 旧版本保存」= 新字段蒸发。

修法分两半：

1. **`FileDoc` 加 `#[serde(flatten)] extra: serde_json::Map<String, Value>`**
   （`#[serde(default)]` + 容器已有 `default`）：读的时候不认识的顶层键原样收进来，
   **写的时候 serde 会把它们原样摊回去**（摊在已知字段之后，不改变已知字段的顺序）。
2. **`extra` 得有来源**：`save(doc, path)` 只拿得到 `&Document`，而 `Document` 在 `mw-core`（不改），
   所以「加载时捡到的未知键」需要一个落地处。新增私有结构 `DocExtras`（顶层 + 逐节点两级）
   与**进程内账本** `EXTRAS_CACHE`（`Mutex<Vec<ExtrasEntry>>`，上限 `EXTRAS_CACHE_CAP = 32`）。
3. **账本的键是 `(docId, 源文件路径)`，不是 `docId`**（Lead 复核后追加的要求，也是这一版最大的修正）：
   用户把一份 `.zmind` 复制成两份、两个都打开，两份 `docId` 相同；只按 id 记账，
   A 的未知键会被写到 B 上——**把别人的字段搬进另一份文档，比丢字段严重得多**。
   落法：
   - `load(path)` 走内部 `load_from_bytes_with_source(bytes, Some(path))`，账本记下这次读的**真实路径**；
   - `load_from_bytes(bytes)`（公开 API 不变）记 `None`——没有源文件；
   - **保存时校验**：`extras_for_save(docId, 目标路径)` 只认「源路径与目标路径是同一份文件」
     （`same_path` 先 `canonicalize` 再比，规避 `C:\a\b` / `c:/a/./b` 两种写法；目标还不存在时退化成字面比较）。
     对不上就**一个未知键都不写**（`DocExtras::default()`）。
   - **删掉的节点不会复活**：合并 `merge_node_extras` 只按现存节点 id 走 Value，
     账本里残留的已删节点 id 找不到落点就被跳过（`a_deleted_node_does_not_resurrect_its_unknown_keys` 钉住）。
4. 节点级未知键没有 `flatten` 可用（`Node` 在 `mw-core`），所以在 `save` 里做一次
   `Value` 合并（`merge_node_extras`）。**只在真有节点级未知键时才走 Value 这条路**，
   普通文档仍然是 `serde_json::to_vec(&FileDoc)`——既有文件的字节形状零变化
   （`a_plain_document_writes_the_same_bytes_as_before` 逐字节钉住）。

> **放弃的方案（写下原因，便于复核）**：最初版本还打算在 `save` 时读一遍目标文件、
> 若其 `manifest.docId` 与本文档一致就「补捡」未知键，用来覆盖「重启后再存」。
> 这条路被我去掉了：① 同一份文档的两份副本 `docId` 也相同，「按 docId 补捡」正是上面那条串味的来源；
> ② 「打开 → 编辑 → 保存」在同一个进程内完成，账本已经覆盖；
> ③ 每次保存多读一遍几 MB 的旧容器是白付的延迟。
> 代价写在 §7 第 8 条：`另存为` / 从字节读来再保存 / 自动保存槽位回灌再另存，这几种情况**会丢未知键**。

### 3.5 读侧过严：未知枚举值让整份文件打不开

现状：`TaskState`（`todo/doing/done`）与 `ArrowHead`（`off/forward/backward/both`）
都是无兜底的枚举，一个未知取值 = 整份文档 `Json` 错误。而这恰恰是「新版本写、旧版本读」的日常。

`mw-core` 不能改（不在写范围），所以修法在 `load_from_bytes` 里加一步
`normalize_unknown_enums()`：先把 `content.json` 解成 `serde_json::Value`，
把不认识的枚举取值改写成**语义上最安全的默认**，再反序列化成 `FileDoc`：

| 字段 | 未知取值 → | 为什么 |
| --- | --- | --- |
| `relationships[].head` | `"forward"` | 明确要求：`Forward` = 今天画的样子，老文件缺这个键也是它 |
| `nodes[].task` | **删掉这个键**（⇒ `None`） | 「有某个状态」是旧版本编不出来的信息；`None` 与老文件缺键走同一条路，不替用户编一个状态出来 |

## 4. 第 6 条：`export.rs::write_text` 非原子

现状：`std::fs::write(path, ...)`。导出到一半崩溃/磁盘满 → 用户得到一个**半截 SVG / 半截 Markdown**，
而且它看起来是「成功的导出」。

改成同目录 `.{file_name}.part` → `File::create` → `write_all` → `sync_all` → `rename`；
任何一步失败都删掉 `.part`（`cleanup_part`）并返回原来的中文错误文案
（`写入失败：{e}`，调用方 `app_methods_2.rs` 直接把它当 toast 显示，文案不能变）。

## 5. 验收场景（每条都有会红的单测）

| # | 场景 | 断言 |
| --- | --- | --- |
| A | 老文件：缺 `themeAccent` / `themeOverrides` / `layout.focusQuestion` / 所有 `relationships[].head` / `cards` | `load_from_bytes` 成功、`validate()` 通过、各字段读成默认值 |
| B | 未来未知键：`{"futureKey":1}` 同时出现在 doc 级与 node 级 | 能打开、`validate()` 通过、未知键不影响已知字段 |
| C | 保真：读 → 存 → 再读 | 存出去的 `content.json` 里两级 `futureKey` 都还在 |
| D | 未知枚举值：`"head":"sideways"`、`"task":"blocked"`（外加一个彻底未知的 `"state":"blocked"`） | 不报错；`head == Forward`、`task == None`；再存时 `head` 不落盘（默认值不写） |
| E | `.bak` 回滚：正本 rename 失败 | 返回 `Err` 且**正本还在原地、内容不变**（两种触发方式：`.bak` 位置被占成目录 / 注入失败点） |
| F | tmp 命名与清理 | save 后不留本进程的 tmp；陈旧（1 小时前）同前缀 tmp 被清掉，新鲜的不被误删 |
| G | 文本导出原子性 | 正常导出不残留 `.part`；失败导出（目标是已存在目录）报 `写入失败` 且不残留 `.part` |
| H | 路径对不上：读 A → 存 B（另存为）；以及从字节读来（无源路径）→ 保存 | 两种都**不写**任何未知键；存回 A 时仍然保真 |
| I | 删掉带未知键的节点再保存 | 文件里既没有该节点、也没有它的未知键（不复活） |

## 6. 证据

**实跑（本人跑过，输出为实）**

- `cargo build -p mw-format` ✅
- `TEMP/TMP=target/tmp cargo test -p mw-format` ✅ **29 passed / 0 failed**（既有 18 + 新增 11），
  集成测试 `tests/x100_fixtures.rs` 2 passed
- `cargo clippy -p mw-format --all-targets` ✅ `mw-format` 自身**零告警**（输出里只有 `mw-core` 既有的 11 条）
- `rustfmt --edition 2021 --check` 我改的两个文件 ✅

新增 11 条（全在 `crates/mw-format/src/lib.rs` 的 `#[cfg(test)]`）：`a_legacy_file_without_any_newer_optional_keys_still_opens`、
`a_file_with_future_unknown_keys_still_opens`、`unknown_keys_survive_a_read_save_read_roundtrip`、
`extra_keys_are_not_written_when_the_source_path_differs`、`a_deleted_node_does_not_resurrect_its_unknown_keys`、
`unknown_enum_values_degrade_instead_of_failing_the_load`、`a_plain_document_writes_the_same_bytes_as_before`、
`temp_file_names_do_not_collide`、`saving_sweeps_only_stale_temp_files`、
`a_failed_backup_move_keeps_the_original_in_place`、`a_failed_commit_restores_the_original_from_the_backup`。

**`mw-shell` 侧：未跑成（原因与本人改动无关）**

- 派单里写的 `cargo test -p mw-shell --lib export` **跑不了**：`mw-shell` 没有 lib target（只有 bin），
  cargo 直接报 `error: no library targets found in package mw-shell`；等价命令是 `cargo test -p mw-shell export`。
- 该命令在我收尾时仍编不过，原因是**别的成员正在改 `mw-shell` 的中间态**，报错首行：
  - `error[E0609]: no field 'disk_print' on type '&mut Shell'`（`crates/mw-shell/src/history_ui.rs:68`）
  - `error[E0599]: no method named 'flush_card_draft_on_exit'` / `'flush_recovery_points_on_exit'`
    （`crates/mw-shell/src/main.rs:1274` / `:1277`）
  - 最后一次尝试（`cargo build -p mw-shell`）剩 5 个错误，均在 `canvas/canvas_methods_3.rs` 等**非我改动**的文件里
- **替代证据（不是最终证据）**：把 `write_text` 一字不改抄成隔离探针
  （`target/tmp/sr01_export_probe.rs`，只用 std）在本机 Windows 上实跑 5 个用例全过：
  新建导出 / 覆盖导出（`rename` 覆盖现存文件可行）/ 目标被目录占住 → `写入失败：拒绝访问。(os error 5)`
  且 `.part` 已清、目标没动 / 父目录不存在 → `写入失败：系统找不到指定的路径。(os error 3)`、不造目录不留 `.part` / 裸文件名落当前目录。
- `export.rs` 里已写好 2 条会跑的单测（`text_export_writes_atomically_without_leaving_a_part_file`、
  `a_failed_text_export_reports_and_cleans_up_the_part_file`），等 `mw-shell` 能编过就能跑；
  **Lead 会在集成后统一补跑 `cargo test --workspace`**。

## 7. 未做 / 风险（诚实清单）

1. **没有真掉电验证**。fsync 只能证明「调了 `sync_all`」，不能证明「掉电后一定完整」。
   `write_text` 的 `.part` 同理。要真验证得上断电/故障注入工具，不在本机做。
2. **进程内未知键账本是全局可变状态**（static `Mutex<Vec<ExtrasEntry>>`，上限 32 份）。
   它是「`mw-core` 不给 `Node`/`Document` 加字段」这个约束下的取舍：`mw-core` 若能加
   `#[serde(flatten)] extra`，账本应该直接住在 `Document` 里。
   **它现在键到 `(docId, 源路径)`**：同一份文档的两份副本各记各的，保存时还要求路径一致，
   已堵掉「A 的未知键写到 B 上」这条路（`extra_keys_are_not_written_when_the_source_path_differs` 钉住）。
3. **未知键保真只到 doc 级与 node 级**。`relationships[]` / `summaries[]` / `boundaries[]` /
   `style` 里的未知键仍然会丢（`Node` 内嵌结构同样在 `mw-core` 且无 `flatten`）。
   要彻底解决得改 `mw-core`（不在写范围）。
4. **未知枚举值被改写后，原值不再保真**：`"head":"sideways"` 存回去会变成不写（= `forward`）。
   这是「能打开」换「能保真」的自觉取舍；要两者兼得同样得改 `mw-core` 加 `Other(String)` 变体。
5. **陈旧 tmp 清理用 mtime 判据**（> 1 小时），极端情况（系统时间被改、长跑写者）可能误删/漏删；
   误删的后果是那个写者的 `rename` 失败并回滚——正是 §3.2 保住的那条路。
6. **目录 fsync 在 Windows 上大概率是空操作**（`File::open` 目录失败即忽略），
   所以「rename 本身落盘」在 Windows 上实际**没有被保证**。这是平台限制，注释里写明了，不假装做到。
7. 仅在 `target/tmp` 口径下验证（`TEMP`/`TMP` 已重定向），未跑 `cargo test --workspace` /
   golden / 门脚本（按派单要求留给 Lead）。
8. **路径一致才写未知键 ⇒ 三种情形会丢未知键**（有意的降级：宁可少写一个不认识的字段，
   也不要把别的文件的字段搬过来）：
   - `另存为` 到新路径（源路径 ≠ 目标路径）；
   - `load_from_bytes` 读来（无源路径，例如从字节流/网络拿到的内容）再保存；
   - 自动保存槽位（`autosave.rs` 的 `write_snapshot`：`Document::from_parts_with_cards` 重建后
     `save(&doc, slot)`，槽位路径 ≠ 原文件路径）——**槽位文件里不保真未知键**；
     因此「崩溃 → 重启 → 从槽位恢复 → 存回原文件」这条路上，未知键**会丢**（重启后账本是空的，
     槽位里也没有存过它们）。正常「打开 → 编辑 → 保存」（同一进程、同一路径）不受影响。
9. **`mw-shell` 侧的两条单测尚未实跑**（原因见 §6，与本人改动无关），
   `export.rs` 的原子性目前只有隔离探针证据。

