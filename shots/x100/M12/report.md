# M12 · 大文档：重建成本与「平移不重排」（打磨轮 6）

> 依据：`docs/x100/polish-ledger.md` §1 M12 行（「5k/20k 大文档：增量重排、平移不重排」，
> 已知粗糙「真实节拍下的帧率未量」）与 §3 第 6 条。
> 状态：**5k 达到逐键 60fps（9.3ms / 次重建）**；**20k 仍是全量重排，一次重建 47ms（约 21fps），
> `docs/08` P7 的 16ms 在 20k 不达标**。没有新建基线：本轮不做视觉改动。

## 1. 这一轮要解决什么

账本这一行写了两件事，先分清楚哪件是真的：

| 账本的话 | 实测（本轮） | 结论 |
| --- | --- | --- |
| 平移不重排 | 20k 文档里只做中键拖 + 滚轮缩放，`scene_build` 轨迹**一次都没有** | 成立，以前没人证过 |
| 增量重排 | 改一个字 / 撤销一次，**整棵重建**（20k 一次 ~47ms） | 不成立：从来没有增量布局 |

顺带量到的最要紧的事：**打字每敲一下就是一次全量重建**。5k 时 9.3ms（还在 60fps 预算里），
20k 时 47ms——用户在 20k 文档里打字，屏幕一秒只能跟 20 帧，光标会明显落后于手指。

## 2. 非目标

- **不实现真增量布局**（脏子树 + 版本号缓存）。那是架构级改动，不是一轮打磨能收口的，
  本轮只去掉「白花的钱」，把全量重排的常数压下来，并把 20k 的真实数字摆出来。
- 不改任何视觉、不改 `Command` 集合、不改 `.zmind`、不加依赖。
- 不覆盖卡片页 / 大纲的 20k 表现（它们的渲染路径与画布不同，要另量）。

## 3. 量法（三份证据，全部可复跑）

### 3.1 进程内：`perf_scene` 例程

```bash
cargo run -p mw-core --release --example perf_scene     # 输出存 shots/x100/M12/perf-scene.txt
```

新增一段「改字重建(缓存)」：`SceneCache` 在循环外预热，之后每轮 `SetText` + 重建——
这才是应用里「打字 → 重建」的真实路径（命中的节点不再测量）。

### 3.2 真实壳层：`scene_build` 轨迹

`rebuild()` 新增一行轨迹（只在 `MW_TRACE` 下写）：

```
event scene_build nodes=5000 edges=4999 ms=9.33
```

5k / 20k 语料由 `node scripts/make-sample.mjs target/perf-5000.zmind --nodes=5000` 生成，
`MW_OPEN` 直接打开（golden 模式也认这个变量），输入脚本用现成的 `mm-new-child`（`Tab` 建子 + 写名字）。

### 3.3 平移/缩放：新增 `press middle` 输入原语

原来的 `press`/`release` 只会按主键，中键拖拽只能真手搓。加了可选按钮前缀
（`press middle 900 600`，不写就是主键，老脚本一个字不改），
脚本 `shots/golden/inputs/mm-pan-camera.txt`：中键按下 → 三次移动 → 松开 → 滚轮进出一格。

## 4. 改了什么（全部零语义变化）

| 位置 | 改动 | 为什么 |
| --- | --- | --- |
| `mw-core/src/scene.rs::BoxKey` | 测量缓存的键从 `format!` 字符串（含三个 `{:?}`）换成结构体逐项比较，命中时零分配 | 20k 每次重建要格式化两万次，实测是「仅布局 9.4 → 20.8ms」里的大部分 |
| `scene.rs` 高级元素段 | 没有边界/概要/关系线的文档整段跳过（先 `any()` 扫一遍），`doc_order()`（克隆两万个 id）不再白跑 | 20k 这一段原来 10.2ms，现在 0.3ms |
| `scene.rs` 合并自由主题 | 布局结果**整块搬移**，不再克隆 `ids`/`boxes`/`edges`/`spines` 三份 | 20k 省掉几万次分配 |
| `theme.rs::resolve_all` | 按节点数 `with_capacity` | 两万条结果不再反复扩容重哈希 |
| `canvas_methods_3.rs::rebuild` | 新增 `scene_build nodes=… edges=… ms=…` 轨迹 | 「平移不重排」与「一次操作几次重建」都要能数出来 |
| `golden_input.rs::press/release` | 可选按钮前缀 `primary/middle/secondary` | 中键拖拽是画布平移的真实手势 |
| `mw-core/src/scene.rs` 测试 | 新增 `the_box_cache_remeasures_only_when_a_size_input_changes` | 键改结构体后，靠这条守住「该失效才失效、只改颜色不失效」 |
| `mw-shell/src/golden_input.rs` 测试 | 新增 `press_can_name_its_button_and_defaults_to_primary` | 前缀解析不能悄悄把老脚本改成别的键 |

## 5. 数字（同机、release；中位数；改字取 40 次、其余 12 次）

| 指标 | 改之前 | 改之后 | 预算 |
| --- | --- | --- | --- |
| 5k `build_scene`（冷缓存） | 12.3ms | **6.8ms** | P8 120ms |
| 5k 改字重建（冷缓存） | 13.7ms | **7.7ms** | P7 16ms |
| 5k 改字重建（缓存路径） | —（新量） | **5.6ms**（p95 7.9） | P7 16ms |
| 5k 撤销 / 重做重建 | 13.7 / 13.1ms | **7.5 / 7.4ms** | P9 30ms |
| 20k `build_scene`（冷缓存） | 63.4ms | **40.8ms** | —（无预算） |
| 20k 仅布局（不含测量/场景装配） | 9.4ms | **8.2ms** | — |
| 20k 改字重建（冷缓存） | 66.9ms | **46.1ms** | P7 16ms ❌ |
| 20k 改字重建（缓存路径） | —（新量） | **34.1ms**（p95 37.3） | P7 16ms ❌ |
| 20k 撤销 / 重做重建 | 64.8 / 65.1ms | **46.7 / 47.7ms** | P9 30ms ❌ |

真实壳层（`shots/x100/M12/trace-*.txt`）：

| 场景 | 轨迹 | 读法 |
| --- | --- | --- |
| 5k 打开 + `Tab` 建子 + 写名字 | `scene_build nodes=5000 … ms=37.38` → `nodes=5001 … ms=9.33` | 打开时冷缓存 37ms；打字那次重建 9.3ms |
| 20k 同上 | `nodes=20000 … ms=157.11` → `nodes=20001 … ms=47.05` | 打开 157ms（P3/P4 预算内）；打字 47ms |
| 20k 中键拖 + 两格滚轮 | 全文件只有一行 `scene_build ms=155.32` | **平移/缩放零重建** |

20k 一次构建的剩余构成（临时插桩实测，插桩已删）：`resolve_all` 5.9、`compute_layout` 20.8、
`index` 1.3、`nodes+order` 8.3、`edges+spines` 0.25、`advanced+bounds` 0.3。

## 6. 门槛

- `cargo fmt --all -- --check` 干净。
- `cargo clippy --workspace --all-targets`：37 条基线 / **新增 0**。
- `cargo test --workspace -- --test-threads=1`（先 `TEMP=target/tmp`）：**920 passed / 0 failed**（+2）。
- stage-b 门 **91 场 / 182 次比较**全过（`target/x100/gate-b-m12-solo.log`），**既有 91 张逐字节零漂移**。
- stage-cm 门 **46 场 / 92 次比较**全过（`target/x100/gate-cm-m12-solo.log`）。
- 本轮**没有新增基线**：所有改动都不该改像素，门的结果也确实是一像素没动。

**踩到的坑（写给下一任）**：两道门**不能同时跑**。`gate_golden.py` 与 `gate_cm.py`
共用 `shots/golden/<场景>/actual|diff`，并发跑时两边互相覆盖，会各报 8 场「漂移」
（实测为 `outline-hover-select`、`tabs-wheel`、`cm-rel-hover` 等，重跑单道门即全绿）。
顺序跑：先 stage-b，再 stage-cm。

## 7. 未做 / 风险

1. **20k 仍不达标**（P7 16ms）。真要达标得做增量布局：只重排脏子树、其余复用上次几何。
   本轮把常数压下来 30–45%，但没改「每次重建都排整棵树」这件事。
2. **真实鼠标节拍下的 P5 没量到**：`scripts/m5-perf.ps1` 本机这次没能拉起窗口
   （60s 内拿不到主窗口句柄），所以 5k 帧率仍只有 2026-10-18 的 60fps 记录。
   本轮改用「离屏 harness + 真实输入脚本 + `scene_build` 轨迹」量同一条链路。
3. **只在 1080p / ppp 1.0 下量**，没量 150%/200%。
4. **卡片页 / 大纲 / 检查器**在 20k 下的重建成本没量（走的是不同渲染路径）。
5. 语料形状是合成的（每层 6/8/8/5 轮换），真实文档的分支宽度、长文比例都不同。
6. 撤销/重做在 20k 下也还是 47ms（P9 预算 30ms），同样要吃增量布局才可能达标。
