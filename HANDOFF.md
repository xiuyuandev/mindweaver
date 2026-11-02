# HANDOFF · 织思 X100

> 更新：2026-11-02 19:26（B2 收口；四道 B2 人工门与 G-B3-1 / G-B3-2 / G-CM-1 / G-CM-2
> 均按授权代判通过（原话：「继续开发，而且呢，你帮助我验收，不需要我决定。」）；
> **Stage C 已解锁，X21 / X21b 落地中**）。**测试数、门场次与实跑读数只看
> [`docs/STATUS.md`](docs/STATUS.md) §1**——本文不写结论，也不复制会过期的数字。
> 口径更新（2026-11-02 21:0x，**交接轮**——本项目换 AI 接手，先读
> [`docs/STATUS.md`](docs/STATUS.md) **§0 的「交接说明」块**，那里是本轮的全部结论与欠账）：
> `stage-b` **正式门 PASS**（1328 tests / golden 178×2 零差异）；`stage-cm` 正式门
> **fmt/clippy/测试/往返/golden 第 1 遍（74/74）全过，第 2 遍因并行会话重构
> `Canvas`（`flash_window`）编译失败而红**，复跑前提是「没人在构建」。
> Stage C 的 X22–X30 已铺开为**审计/设计报告**（`shots/x100/X2x/`），实现未动；
> X24 导出同源与 CM-23 `.cxl` 缺陷已修码并有单测。裁决全账见
> [`docs/x100/log/decisions-2026-11-02-stage-c.md`](docs/x100/log/decisions-2026-11-02-stage-c.md)。
> `已验证` 仍欠实机拼音与系统 DPI，别只写绿。
> · **本文是接手的第一入口，但它不记录进度。**
> 进度、测试数、门状态只看 **[`docs/STATUS.md`](docs/STATUS.md)**；
> 逐轮现场与漂移量在 [`docs/x100/log/`](docs/x100/log/)。
> **本轮刚收工的现场、遗留红灯与下一刀在哪：直接跳到本文 §7「本轮交接」。**

---

## 1. 五分钟认识这个项目

- 产品：**织思 / MindWeaver**，仅 Windows 的纯 Rust 思维导图 + 卡片笔记工具。
- 栈：Rust + egui/eframe + wgpu + DirectWrite。**禁止** WebView / HTML / CSS / JS / Electron / Tauri。
- 目标：以本机 Xmind 为视觉与交互参考，用 **X100（100 轮 / 10 阶段）** 重建核心体验。
- 当前：**Stage B2（CER-01–34）已收口**；四道 B2 人工门（G-B2-1/2/3/4）与平行轨的
  G-B3-1、G-B3-2、G-CM-1、G-CM-2 均按授权代判通过（2026-11-02）；
  **Stage C（X21+）已解锁，X21 / X21b 落地中**。
  进度、测试数、门场次与红灯**只查 [`docs/STATUS.md`](docs/STATUS.md) §1**；
  `已验证` 仍欠实机微软拼音与系统 DPI。

**读顺序（约 30 分钟）**：

1. 本文件 + [`docs/STATUS.md`](docs/STATUS.md)（10 分钟）——现在在哪、下一步是什么。
2. [`docs/24-X100升级路线.md`](docs/24-X100升级路线.md) §1–§4（10 分钟）——阶段划分与执行协议。
3. 当前批次的合同：
   - B2 设计 → [`docs/28`](docs/28-内容体验重构规范.md)；执行 → [`docs/29`](docs/29-内容体验重构工作包.md)
   - P/M 轨设计 → [`docs/30`](docs/30-外壳体验与概念图设计规范.md)；执行 → [`docs/31`](docs/31-外壳体验与概念图工作包.md)
4. 需要时：`docs/05`（数据模型）、`docs/07`（中文）、`docs/27`（数据/命令审计）。

---

## 2. 不可违反

1. Windows、纯 Rust、egui/eframe/wgpu/DirectWrite；禁止 WebView/HTML/CSS/JS/Electron/Tauri。
2. UI 只发 Action；文档变更走 `mw_core::Command` 和撤销栈，不许直接改 `Document`。
3. 测试/截图隔离 `APPDATA`、`LOCALAPPDATA` 和临时文档；离屏 golden 不得赌桌面指针。
4. 不操作用户已有的 Xmind 窗口或个人文件；隔离采样先过 `scripts/x100-xmind-capture.ps1 -Mode check`。
5. 不复制 Xmind 的专有字体、图标、文案和素材。
6. **不提交 git**，除非用户明确要求；也不清理/重置/stash 现有未提交工作。
7. 每道人工门未通过，不开始下一批、不开始下一阶段。
8. 数字只有一个真相：**`docs/STATUS.md`**。别在别的文件里再写一份会过期的数字。

---

## 3. 构建与测试

```powershell
cargo build --release -p mw-shell
powershell -File scripts/test.ps1        # 串行全量测试，已内置 TEMP 重定向
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
```

### 3.1 本机必踩的坑：TEMP（先看这条）

裸跑 `cargo test` 在本机会出 **56 条 `mw-shell` 写盘用例假失败**，
全是 `Os { code: 5, PermissionDenied }`——受限令牌下 cargo 孙进程写不了系统 `%TEMP%`，
**不是代码坏了**。要么用 `scripts/test.ps1`，要么手工重定向：

```bash
TEMP="$(pwd)/target/tmp" TMP="$(pwd)/target/tmp" cargo test --workspace -- --test-threads=1
```

三个门脚本（`scripts/ci.ps1`、`scripts/x100-stage-b-gate.ps1`、`scripts/x100-stage-cm-gate.ps1`）
都已内置这层重定向，所以「门绿但手工裸跑红」是正常的。

### 3.2 阶段门

```powershell
powershell -File scripts/x100-stage-b-gate.ps1   # Stage B / B2 全量回归
powershell -File scripts/x100-stage-cm-gate.ps1  # 概念图
powershell -File scripts/ci.ps1                  # fmt + clippy 基线 + test + golden
```

- 阶段门一律 `--test-threads=1`；`note_store` 的墙钟门槛已改三次取优，但别在低电量/高负载时跑。
- **手工拍图前必须 `cargo build --release -p mw-shell`**，否则拍的是旧二进制（踩过）。
- golden 双跑是刻意的：第 2 遍抓「自己和自己不一致」。依赖真实时间的场景（滚动落定、悬停计时）
  在机器刚跑完测试时会多抓半帧，门里已留 10 秒降载。

### 3.3 golden 的两条老规矩

- 离屏会先把指针移到画外再 `PointerGone`（`-10000,-10000`）；只发 `PointerGone` 不够。
  带脚本的场景里，脚本含 `move/hover/click/press/release/scroll` 时指针归脚本管，否则每帧清。
- 要验证 hover 态必须写 `MW_GOLDEN_INPUT` 脚本注入，不要指望桌面光标。

---

## 4. X100 证据命令

```powershell
powershell -File scripts/x100-xmind-capture.ps1 -Mode check
powershell -File scripts/x100-fixtures.ps1
powershell -File scripts/x100-evidence.ps1 -Round X20 -Check
powershell -File scripts/x100-stage-b-gate.ps1
cargo test -p mw-format --test x100_fixtures -- --test-threads=1
```

---

## 5. 文件地图

| 路径 | 内容 |
| --- | --- |
| `docs/STATUS.md` | **进度与数字的唯一真相源** |
| `docs/24-X100升级路线.md` | 100 轮规范、阶段划分、执行协议 |
| `docs/26` / `27` | 完成定义与四类证据 / 数据与命令审计（`25` 差距矩阵已归档：`docs/archive/25-Xmind参考与差距矩阵.md`） |
| `docs/28` / `29` | B2 设计合同 / B2 执行合同（CER-01–34） |
| `docs/30` / `31` / `32` | 外壳与概念图设计合同 / 执行合同（SE/CM）/ 概念图数据结构决定 |
| `docs/x100/stage-a-report.md` | Stage A 人工门（已批准） |
| `docs/x100/stage-b-report.md` | Stage B 阶段报告（当时结论；当前门状态见 `docs/STATUS.md` §1） |
| `docs/x100/stage-b2-report.md` | B2 阶段报告（B2-A–D 各章与门判词；当前状态见 `docs/STATUS.md` §1） |
| `docs/x100/stage-cm-report.md` | M 轨阶段报告（G-CM-1 / G-CM-2 已于 2026-11-02 判「通过 4/4」） |
| `docs/x100/log/` | 2026-10 的逐轮现场与执行记录（**历史，不是当前状态**） |
| `docs/archive/` | 已被取代的文档（含旧路线图 `PLAN-历史路线与执行记录.md`） |
| `shots/x100/<ID>/report.md` | 每个工作包的证据 |
| `shots/golden/<场景>/` | golden 基线；脚本在 `shots/golden/inputs/` |
| `crates/` | `mw-core` / `mw-render` / `mw-shell` / `mw-ui` / `mw-markdown` / `mw-format` / `mw-io` / `mw-golden` / `mw-probe` |
| `tests/fixtures/x100/` | 固定语料与生成规则 |

crate 依赖方向：

```
mw-shell → mw-render / mw-core / mw-format / mw-io / mw-markdown
mw-render → mw-core
mw-format / mw-io → mw-core
mw-markdown 零内部依赖（连 mw-core 也不依赖）
mw-core 零 UI 依赖
```

关键不变量：`Scene` 是几何与视觉的唯一真相源；`Frame.viewport = Some(...)` 裁剪到视口（交互），
`None` 即全图（离屏/导出）；视图层不含领域语义。

---

## 6. 接手第一步

1. 读 **本文 §7「本轮交接」**（最新现场、遗留红灯、下一刀）→ 再读 [`docs/STATUS.md`](docs/STATUS.md) §1、§5、§6
   （机器健康、下一步顺序、已知红灯）。两者数字不一致时以 STATUS 为准，但先把 STATUS 对齐到实测。
2. 跑一次全量测试（**带 TEMP 重定向**）和 `get-date`，确认数字与 STATUS 一致；不一致就改 STATUS。
3. 确认没有别的会话在同一片代码上工作（`git status`、`.workbuddy-loop-status.md`）。
4. 从 STATUS §5 的第一优先项开始。B2 已收口、Stage C 已解锁（X21 / X21b 已落地）；
   不要跳批、抢门，也不要跳过阶段内的每批验收。
5. 每包收工按 `docs/29` §3（B2）或 `docs/31` §3（SE/CM）的完成定义自查：
   报告 + manifest + 四类证据 + 阶段报告回填 + STATUS 更新。

---

## 7. 本轮交接（2026-10-31 更新）

> **2026-11-02 补记**：B2（CER-01–34）已全部收口、G-B2-4 授权代判通过，Stage C 已解锁；
> 本节及以下逐轮记录是 2026-10-31/11-01 的现场，**当前断点只看 [`docs/STATUS.md`](docs/STATUS.md) §1/§5/§6**。
> Stage C 的 **X21 / X21b** 亦已落地，下一步只看 STATUS §5。
>
> 上一段交接（2026-10-30）覆盖打磨轮 **30–33** 与六个工作包（CM-12 / SE-11 / CM-13 / M15 / C14 /
> SR-01 / SR-02），正文留在 §7.2/§7.3；此后又做了**第 34 轮 C14 平行命题分车道**与
> **第 35 轮 M16 金样按帧确定性**。以下是**你可以直接接着干**的状态，
> 不是历史记录；历史在 `docs/x100/log/execution-log-2026-10.md` 与 `docs/x100/polish-ledger.md`。

### 7.1 状态与数字（**只查 `docs/STATUS.md` §1**，本节不留快照）

本节以前贴过「本机实测」的测试数与分项、两道门的场次、clippy 指纹 —— **写一次就过期一次**，已全部删掉：

| 项 | 去哪看 | 怎么复跑 |
| --- | --- | --- |
| 全量测试（含分项） | `docs/STATUS.md` §1 | `powershell -File scripts/test.ps1`（TEMP 已内置重定向） |
| `stage-b` / `stage-cm` 场次与最近一次实跑 | `docs/STATUS.md` §1 | `scripts/x100-stage-b-gate.ps1` / `scripts/x100-stage-cm-gate.ps1` |
| 最近一次 golden 矩阵实跑与红灯 | `docs/STATUS.md` §1 | `scripts/golden-matrix.ps1` |
| `cargo fmt` / clippy 指纹 | `docs/STATUS.md` §1 | `scripts/ci.ps1` |

### 7.2 本轮落地的能力（改了什么、在哪）

1. **概念图节点改椭圆**（CM-12，`dbg` 无）：`Shape::Ellipse` 进 `theme.rs`，`scene.rs::measure_concept`
   按比例外扩（`ELLIPSE_EXTRA_X 0.22` / `ELLIPSE_EXTRA_Y 0.16`，封顶 24/8），内接文本框一字未动
   ——换行/省略/提示仍逐字节相同。渲染走 `ELLIPSE_CODE = -1_000.0` 哨兵（`Params.x` 为负即椭圆），
   `RectInst` **仍是 68 字节**，WGSL 用 `sd_ellipse`；SVG 出原生 `<ellipse>`，PDF 出四段三次贝塞尔
   （`k = 0.5522848`）；命中测试用归一化 `dx²+dy² ≤ 1`。
2. **关系线箭头屏幕空间恒定**（CM-13）：`instances.rs::screen_arrow`，`ARROW_CSS_LEN = 7.0`，
   只放大不缩小（`k = max(css_to_world(7)/len, 1)`），尖端锚定终点；30% 缩放下 1.2 → 7 css px。
   `Scene` 里仍存 6×4 世界单位基准，导出路径同样受益。
3. **平行命题分车道**（本轮新增，`scene.rs::bowed_straight`）：同一对概念之间的多条命题
   原先画成**同一条直线**（重合 ⇒ 看不出有几条、点不中第二条）。现在按无向对分组、
   法向偏移 `gap = min(24, len/6)`；单条命题仍走 `straight_between`（`--tol 0` 基线不动），
   有两条会红的单测钉住。
4. **对比度与设计令牌归一**（SE-11，`theme.rs` / `design.rs`）：三个浅主题 `dim` 调深到
   ≥4.5（snow 4.34→**4.53**、mist 4.36→**4.56**、sand 4.31→**4.51**），`text_faint` 0.48→**0.56**、
   暗色 0.50→**0.58**；新增 `design::text::CANVAS_LEAF = 14.0`，`component::FOCUS_RING` 改 1.5/1.0，
   `card_padding(font_size)` 四级，删掉 `layout.rs` 里已死的 `PAD_X/PAD_Y`。
   门是新增的 `crates/mw-core/tests/contrast.rs`（7 条，含「faint 必须严格淡于 dim」）。
5. **画布交互补齐 10 项**（M15）：删除确认（含框选多选的 `delete_confirm_plan` / `delete_ids`）、
   `Ctrl+A` 只选可见、`Ctrl+C/X/V` 与 `Ctrl+Shift+V`、`Esc` 逐层退（`CanvasEsc`/`esc_plan`）、
   缩放钳在 `0.1–5.0` 且 `zoom_at_anchor` 保住光标点、打字即进编辑（`take_type_start`/`type_to_edit`）、
   粘贴成兄弟/子节点的分支、拖拽批量 `mw_ui::drag::move_commands`（一条撤销）、`Shift+Enter` 软换行
   （`take_exact_key` 先精确取键，否则被 egui 的 `matches_logically` 吃掉）。
6. **概念图语义护栏 + 命题可见**（C14）：`MoveNode` / `AddNode` / `InsertSubtree` /
   `AddLinkedConcept` 在概念图里显式拒绝（不再静默把概念变成树节点）；**同一对概念之间允许多条命题**
   （只在概念图放开，思维导图仍禁——树场景下重复边会完全重合）；连接短语在概念图里必填（但允许
   创建时暂时留空）；检查器新增「命题」组（`propositions_section`，出/入摘要、行点击选线、× 删除），
   关系线页四个箭头档补**无修饰数字键 1–4**。
7. **保存与多标签的可靠性**（SR-01 / SR-02）：`mw-format::write_atomic` 走
   `create → write_all → sync_all → .bak 改名 → rename → sync_dir`，失败回滚；`export.rs` 的文本导出
   也改原子写。壳层把「协作锁 / 指纹 / 只读」从窗口级下沉到**每个标签**（`TabGuard`），
   「重新加载」不再绕过脏检查（`PendingAction::Reload` + 一次性放行票），关窗检查**所有**标签，
   退出补恢复点，`after_saved` 加幂等闸门，自动保存失败带原因 + 退避重试。
8. **导出不再能写出空文件**：`app_methods_2.rs` 里 `fmt.text(doc).unwrap_or_default()` 改成
   失败就报「导出失败：这个格式无法表示当前文档」，不再用 0 字节覆盖用户文件。
9. **撤销栈上限可配**（M28，`document.rs` / `settings.rs` / `settings_ui.rs`）：`Document.history_limit`
   是内存字段（默认 500、clamp 100–2000、不写 `.zmind`），`set_history_limit()` 立即丢最旧；
   `apply` / `apply_coalesced` / **`redo`**（以前不裁，降上限后会越界）三条路径统一按实例上限裁剪。
   `settings.undoLimit` + 设置中心「编辑」组滑轨（100–2000、步进 100）；保存即作用于画布与
   所有标签页，`Shell::ui()` 每帧 `sync_history_limit()`（`usize` 相等即返回）兜住新开的文档。
   `MW_UNDO_LIMIT=100` 只改内存，轨迹 `history limit=<n> depth=<u>/<r>` / `settings save undo_limit=<n>`。

### 7.3 数据与兼容（重要，别踩）

- **`.zmind` 格式没变**：`FORMAT_VERSION` 仍是 1，无迁移、无新字段。椭圆是渲染层编码，不落盘。
- 概念图在**存储层本来就是图**（`Relationship.from/to` + `LayoutKind::conceptMap`），
  「不能沿用思维导图结构」是**交互与视觉**要求，不是数据迁移。`concept_tree.rs` 只是大纲/导出投影。
- 未知字段（`FileDoc.extra`，`#[serde(flatten)]`）用按路径键控的 `EXTRAS_CACHE` 保住；
  **已知降级**：另存为 / `load_from_bytes` 后保存 / 从自动保存槽恢复这**三条路径**会丢未知键
  ——故意选的（绝不把一份文档的未知字段搬到另一份）。见 `shots/x100/SR-01/report.md` §7。
- 撤销栈上限**已可配**（打磨轮 51 / M28）：`Document.history_limit` 是内存字段
  （默认 500、clamp 100–2000、不写 `.zmind`），设置中心「编辑」组滑轨对应 `settings.undoLimit`。

### 7.4 证据与门怎么复跑

```powershell
# 一次跑完全部（fmt → clippy 基线 → 串行全量测试 → golden）
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/ci.ps1
# 两道阶段门（串行，别并行）
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/x100-stage-cm-gate.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/x100-stage-b-gate.ps1
# 单场景重拍/复核
cargo golden --ppp 1.5 --tol 0 --warmup 8 --dir shots/golden/<场景>   # 加 --update 重拍基线
```

- 必须带 `-ExecutionPolicy Bypass`：本机执行策略默认禁止运行脚本。
- 本机**没有 `pwsh`（PowerShell 7）**，只有 `powershell.exe`（5.1）；带中文的 `.ps1` 必须存成 **UTF-8 BOM**。
- 本轮包证据：`shots/x100/CM-12/`、`SE-11/`、`CM-13/`、`M15/`、`C14/`、`SR-01/`、`SR-02/`
  （各含 `report.md` 与 `manifest.json`；七份清单于 2026-10-31 打磨轮 36 按报告回填）。

### 7.5 未做 / 已知红灯（诚实清单，按优先级）

1. ~~**`stage-b` 第 1 遍的 `outline-*` 抖动（唯一还在红的东西）**~~ **已修（2026-10-31 打磨轮 35 / M16）**：
   两次实测各撞到一个不同场景（`outline-hover-select` 2.751% max 92；`outline-drag-illegal` 2.827% max 225），
   第 2 遍都全绿。根因不是「机器没凉」，而是两件事叠在一起：① 金样按**固定帧数**截帧，
   而大纲握把 120ms / 拖拽展开 500ms 读 `Instant` 墙钟、egui 滚动落定读 `stable_dt`，
   帧耗时随负载变 ⇒ 第 N 帧对应的真实毫秒数不同；② 离屏进程仍可能收到宿主 `PointerGone` / 真实键入，
   足以抹掉悬停态或往标题里敲字。修法：`GoldenInput::advance_clock()` 帧计数虚拟时钟
   （`sleep` 冻结、到期补名义时长）、壳层写 `RawInput::time` / `predicted_dt`、
   harness 下 `retain(!is_host_input)` 剥离全部宿主输入事件、`outline.rs` 两处计时改读 egui 时间。
   验证：三场风险场景逐字节不变；`stage-b` 连跑两次、四遍矩阵 **472 次比较零差异**。
   证据 `shots/x100/M16/`。**别再靠加长 `Start-Sleep` 掩盖**——那只是让窗口更窄。
2. ~~**悬停反馈只做了一半**~~ **已补（2026-10-31 打磨轮 37 / M17）**：折叠圆点 / 概要括号 / 摘要主题 /
   边框现在都有悬停反馈（圆点 1.0→1.3 倍、120ms；概要/边框 2px 强调描边、55% 强度），
   **没有改 `mw_render::Frame` 的 ABI**——`Frame.hovered` 只带节点 id，指针位置在壳层本来就有，
   所以反馈画在壳层覆盖层（与 span 握把/框选同一通道），`hover_detail` 只认 hit 目标身份。
   新增三场金样 `mm-toggle-hover` / `mm-summary-hover` / `mm-boundary-hover`；证据 `shots/x100/M17/`。
   还发现 `Frame.hovered` 仍是**节点级**：悬停折叠圆点时节点本身也会亮（既有行为，未改）。
3. ~~**`Ctrl+X` 多节点不弹删除确认**~~ **已证伪并钉住（2026-10-31 打磨轮 38 / M18）**：
   读码看 `cut_action()` 与 `apply_clipboard(Cut)` 都走 `delete_selected()` → `delete_confirm_plan`，
   **本来就会弹**。新增轨迹（`delete-confirm pending=3 selected=1` + `dialog id=…`）与两场金样
   `mm-cut-confirm` / `mm-cut-cancel`；取消后与「只选中」逐像素 0 px，证明剪切没有提前删节点。
   产品行为零改动，只加了三条 `MW_TRACE` 事件。证据 `shots/x100/M18/`。
4. **IME 只做了离屏等效**：`TypeStart::Compose` 的真实微软拼音未在本机核对过（历史欠账，CER-08 起就有）。
5. **未做**：缩略图 / 力导向布局（~~钉住~~、~~「只整理选中」~~ 已由打磨轮 53 / C18 补上）；
   ~~节点与关系线选中互斥~~ **已补（2026-10-31 打磨轮 49 / M26）**：三条选中道
   （节点 / 关系线 / 注解）统一由 `SelectionLane` + `foreign_lanes` + `clear_foreign_selection`
   守成三选一，方向键遍历、框选/套索松手、检查器选注解/关系线都会清掉另外两道；
   `Delete` 按 `delete_lane` 一次只删一道（关系线 → 注解 → 节点）。两场金样
   `mm-rel-select-arrow` / `mm-rel-select-delete`，测试 1073，证据 `shots/x100/M26/`；
   ~~平行命题在画布上的独立点选（现在靠检查器「命题」组进）~~ **已补（2026-10-31 打磨轮 39 / C15）**：
   跨源节点收车道 + 短语沿切线错开；`cm-parallel-select-a/-b` 两场金样，轨迹给出两个不同的关系 id；
   `app_methods_2.rs` 里 `MW_*bench`
   那条基准路径仍是 `unwrap_or_default()`（只是基准，不写用户文件，故未改）。
6. ~~**`manifest.json` 缺口**~~ **已补（2026-10-31 打磨轮 36）**：`CM-12`/`SE-11`/`CM-13`/`M15`/
   `C14`/`SR-01`/`SR-02` 七份清单按各自 `report.md` 回填，结构照 `shots/x100/C9/manifest.json`；
   `_note` 标明补记、不新增结论；M15/C14/SR-01/SR-02 不动外观，`scenes` 为空并写明原因。
   同时补了 `M16`（第 35 轮）的清单。
7. ~~**`docs/STATUS.md` 尚未回填本轮数字与状态**~~ **已对齐（2026-10-31）**：§0/§1 改为
   1022 测试、`stage-b` 连跑两次全绿、无机器红灯；§5 补第 30–35 轮；§9 补第 31–35 轮变更记录。
   **以后每轮收工先回填 STATUS，再开下一轮。**
8. ~~**命题短语的空白提交静默失败**~~ **已修（2026-10-31 打磨轮 40 / C16）**：概念图的既有命题
   被清空时模型早已拒绝，但画布编辑器与大纲都会「关掉框、什么都不说」；新建边按 `Enter` 留空
   更会真的落一条 `w=0` 的线团。现在两种空提交都变成「保留/撤线 + 提示」，判定抽成
   `phrase_commit` 纯函数；顺带修掉短语框开着时 `Ctrl+A` 漏进大纲全选 6 行的键盘归属。
   证据 `shots/x100/C16/`。
9. ~~**搜索只索引标题**~~ **已补（2026-10-31 打磨轮 41 / M19）**：`docs/02` §4 的 P0 是「标题 +
   备注 + 链接」，代码只读了标题。现在三字段都支持原文/全拼/首字母，备注/链接命中显示 ±24 字
   摘录与字段角标；240 字以上只做原文（`FIELD_PINYIN_CAP`）。`Ctrl+H` 替换仍只改标题。
   证据 `shots/x100/M19/`。
10. ~~**关系线短语不进检索**~~ **已补（2026-10-31 打磨轮 42 / C17）**：命题短语支持原文/全拼/首字母，
    结果新增「命题」组（节点 < 命题 < 卡片 < 标签 < 路径），点击/Enter 选中该边并居中两端中点、
    搜索框不丢焦点。**未做**：命题不参与 `Ctrl+H`、不索引颜色/箭头/概要/注解、
    每帧仍扫全部边（只缓存拼音键）。证据 `shots/x100/C17/`。
11. ~~**搜索跳进折叠子树静默失败**~~ **已修（2026-10-31 打磨轮 43 / M20）**：检索遍历文档前序，
    折叠子树里的节点照样命中，但画布只排可见节点，`select_node_keep_focus` 在 `scene.find`
    失败时直接 return。现在先按根 → 近展开挡路的折叠祖先（一条 `Command::Batch`，一次撤销全收回），
    再选中 + 相机居中 + glow；轨迹 `search_reveal` / `search_jump`，展开后仍不可达（聚焦模式）
    给 toast「这个主题在当前视图之外」而不替用户退出聚焦。新夹具 `MW_COLLAPSE_DEMO`、
    新基线 `search-collapsed`。~~**未做**：聚焦模式仍不可达~~ **已补（2026-10-31 打磨轮 45 / M22）**：
   聚焦是用户显式进入的视图态，但点搜索结果同样是显式动作——目标在聚焦子树外时先
   `focus_to(0)` 退出全部聚焦再展开/选中，toast「已退出聚焦并跳转」；轨迹 `search_focus_exit`。
    仍未做：跳转展开会入撤销栈。证据 `shots/x100/M20/`、`shots/x100/M22/`。
12. ~~**替换只有「全部替换」、没有预览**~~ **已补（2026-10-31 打磨轮 44 / M21）**：
    `Ctrl+H` 展开即聚焦替换框，`Ctrl+Enter` 替换当前、`Ctrl+Shift+Enter` 全部；
    当前行显示 `→ 新标题`；被替换行仍命中就前进一行、到底绕回，不再命中则按原下标夹取；
    只接受节点标题的字面命中（备注/链接/拼音不给入口）；替换走一条 `SetText`、不抢键盘。
    新基线 `search-replace` / `search-replace-one`。**未做**：替换后相机居中不随 `Ctrl+Z` 回退；
    替换后文本仍含查询词会绕圈；实机 IME / `Ctrl+Shift+Enter` 归属未核。证据 `shots/x100/M21/`。

### 7.6 本轮踩到的七个坑（省你几小时）

1. **`mw-shell` 是纯 bin crate**：`cargo test -p mw-shell --lib` 直接报错，要用 **`--bins`**。
2. **`cargo build` 绿 ≠ 测试能编译**：`{:?}` / `panic!` 之类只在 test target 才暴露缺 derive，
   改完记得 `cargo test --workspace --no-run`。
3. **改了颜色/形状一定要重拍基线并连跑两遍 `--tol 0`**，否则 `stage-b` 会在
   `canvas-ppp1.00` / `text_render` 上红（本轮 `dim` 三个十六进制值就让两张基线红过）。
4. **删代码会让 clippy 指纹漂移**：基线文件是「标题 @ 文件:行」，`layout.rs` 删掉四个常量后
   8 条既有告警行号全变 ⇒ `ci.ps1` 报「新增」。正确做法是确认无新增后
   `scripts/ci.ps1 -WriteClippyBaseline`，并用 `git diff scripts/clippy-baseline.txt` **逐条审计**。
5. **`edit` 前必须先 `read`**：只用 `sed`/`grep` 看过会报「file has not been read」。
   热点文件（`command.rs` / `document.rs` / `canvas/tests.rs` / `inspector.rs` …）改前重读。
6. **本模型没有图像输入**：一切「看起来对不对」都要转成数字（对比度、令牌成员、像素 diff、包围盒），
   `read_image` 会直接报 `does not declare image input`。
7. **金样里的「时间」有两个来源，必须都掐死**：应用计时（`Instant` / `SystemTime`）与 egui 的
   `InputState.time` / `stable_dt` 是两套；前者要改读 `ui.input(|i| i.time)`，后者要由
   `raw_input_hook` 覆盖 `RawInput::time`。只做一半，抖动会从另一个门缝钻回来
   （第 35 轮 M16 的完整病历）。另：离屏进程照样收得到宿主事件——harness 下要主动剥掉。

### 7.7 下一刀建议顺序

1. ~~把 `docs/STATUS.md` §1/§5 对齐到实测值~~ ✅ 2026-10-31 已对齐（1022 测试、门全绿）。
2. ~~修 7.5 ① 的 `outline-*` 抖动，让 `stage-b` 两遍都绿~~ ✅ 2026-10-31 第 35 轮已修（M16）。
3. ~~补 7.5 ⑥ 的 7 份 `manifest.json`~~ ✅ 2026-10-31 已补（CM-12/SE-11/CM-13/M15/C14/SR-01/SR-02 + M16）。
4. 回填 `docs/x100/polish-ledger.md` 与 `docs/x100/log/execution-log-2026-10.md` 的漂移账
   （第 31–35 轮；第 35 轮已补，31–34 已补骨架，缺的逐张数字可用 `git diff --stat -- shots/golden`
   与 `git show HEAD:<png>` 复算）。
5. 再挑 7.5 ④ 里的体验缺口：~~**② 悬停反馈**~~ ✅ 2026-10-31 第 37 轮已补（M17，壳层覆盖层，
   未动渲染层 ABI）、~~**③ `Ctrl+X` 多节点删除确认**~~ ✅ 2026-10-31 第 38 轮已证伪并加金样（M18，
   产品行为本来就正确）、④ 实机微软拼音（阻塞，需人工）。
6. ~~或按 `docs/STATUS.md` §5 回到人工门（G-B2-2 / G-B3-1 / G-B3-2 / G-CM-2）走验收~~ ✅ 2026-11-02
   全部按授权代判通过（G-B3-2 复判 5/5）；**Stage C 已解锁**，顺序只看 `docs/STATUS.md` §5。
7. ~~第 42 轮计划 **C17 命题短语进搜索**~~ ✅ 2026-10-31 第 42 轮已完成：
   `RelationshipHit` + `SearchIndex::relationship_hits` + 结果行「命题」组 + 跳转选中关系线并居中；
   金样 `search-rel` / `search-rel-jump` 已进 `stage-cm`（56/112 → **58/116**）。
   证据 `shots/x100/C17/`。
8. ~~第 43 轮计划 **M20 搜索跳进折叠子树**~~ ✅ 2026-10-31 第 43 轮已完成：
   `collapsed_ancestors` + `reveal_commands` + `select_node_keep_focus -> bool`，
   跳转先展开再选中，轨迹 `search_reveal` / `search_jump`；金样 `search-collapsed`
   已进 `stage-b`（123/246 → **124/248**）。证据 `shots/x100/M20/`。
9. ~~第 44 轮计划 **M21 Ctrl+H 逐个替换与预览**~~ ✅ 2026-10-31 第 44 轮已完成：
   `replace_char_range` + `ReplaceOne` + 替换/全部替换按钮 + 当前行预览 + 当前行前进（绕回）；
   金样 `search-replace` / `search-replace-one` 已进 `stage-b`（124/248 → **126/252**）。
   同轮修掉 `card-autosave` 的墙钟按 UTC 跨日腐烂（`MW_CARD_NOW_MS`）。证据 `shots/x100/M21/`。
10. ~~第 45 轮计划 **M22 聚焦视图外的搜索跳转**~~ ✅ 2026-10-31 第 45 轮已完成：
    `focus_exit_levels` 判定 + `select_node_keep_focus` 先 `focus_to(0)` 再展开/选中，
    轨迹 `search_focus_exit id=… levels=…`，toast「已退出聚焦并跳转」；夹具 `MW_FOCUS_DEMO`、
    金样 `search-focus-jump` 已进 `stage-b`（126/252 → **127/254**）。证据 `shots/x100/M22/`。
11. ~~第 46 轮计划 **M23 思维导图插入关系线**~~ ✅ 2026-10-31 第 46 轮已完成：
    `Ctrl+L`（仅思维导图、单节点选中）两步式插入 —— 源节点强调环 + 虚线预览，点另一节点落地并
    选中新关系线；`Esc` 两级取消（插入态 → 关系线 → 聚焦 → 选择），重复对 toast 拒绝；
    右键菜单补「插入关系线 Ctrl+L」。金样 `mm-rel-link` / `mm-rel-link-created` 已进 `stage-b`
    （127/254 → **129/258**）；`menu` / `menu-narrow` 因菜单多一行有意重拍（后者出现滚动条）。
    证据 `shots/x100/M23/`。
12. ~~第 47 轮计划 **M24 边界/概要键盘路径**~~ ✅ 2026-10-31 第 47 轮已完成：
    `Ctrl+B` 添加边界、`Ctrl+]` 添加概要（仅思维导图，一次撤销）；`take_exact_key` 精确匹配；
    没选/选根两种失败提示 + `annotation_add_failed` 轨迹，概念图不响应；菜单补快捷键列。
    金样 `mm-boundary-key` / `mm-summary-key` 已进 `stage-b`（129/258 → **131/262**）；
    `menu` / `menu-narrow` 因快捷键列有意重拍（581 / 579 px）。证据 `shots/x100/M24/`。
13. ~~第 48 轮计划 **M25 大纲行「更多」与行右键菜单**~~ ✅ 2026-10-31 第 48 轮已完成：
    行尾固定 24px 动作道 + 悬停/选中出现的 16px `⋯`（角标左移让位），`⋯` 与行右键共用一张菜单；
    单行 12 项、多行 5 项，画布专属项不收；菜单键盘先于画布与行键；概念图大纲不出 `⋯`
    （`outline_row_menu_skipped reason=concept_map`）。修正两处与大纲键位不符的菜单标签
    （`Tab` 留空、`/` → `Alt+.`）。金样 `outline-more` / `outline-row-menu` /
    `outline-row-menu-add` 已进 `stage-b`（131/262 → **134/268**）；`outline*` 九场有意重拍，
    角标左移 128–272 px、`outline-wrap` 换行差 37 166 / 47 089 px。证据 `shots/x100/M25/`。
14. ~~第 49 轮计划 **M26 选中道互斥**~~ ✅ 2026-10-31 第 49 轮已完成：
    节点 / 关系线 / 注解三条选中道互斥，`Delete` 只删当前道；
    金样 `mm-rel-select-arrow` / `mm-rel-select-delete` 已进 `stage-b`（134/268 → **136/272**）。证据 `shots/x100/M26/`。
15. ~~第 50 轮计划 **M27 快捷键禁用与持久化**~~ ✅ 2026-10-31 第 50 轮已完成：
    快捷键面板单项禁用与恢复出厂、`"none"` 持久化；
    金样 `shortcuts-default` / `shortcuts-disabled` 已进 `stage-b`（136/272 → **138/276**）。证据 `shots/x100/M27/`。
16. ~~第 51 轮计划 **M28 撤销栈上限可配**~~ ✅ 2026-10-31 第 51 轮已完成：
    `Document` 内存字段 `history_limit` + 设置中心「编辑」组滑轨（100–2000）；
    金样 `settings` 已进 `stage-b`（138/276 → **139/278**）。证据 `shots/x100/M28/`。
17. ~~第 52 轮计划 **M29 搜索匹配方式可切换**~~ ✅ 2026-10-31 第 52 轮已完成：
    `MatchMode::Pinyin` / `Text` + 搜索条底部开关 + `Ctrl+Shift+M`；
    金样 `search-text` 已进 `stage-b`（139/278 → **140/280**），stage-cm 58/116 两遍全过。证据 `shots/x100/M29/`。
18. ~~第 53 轮计划 **C18 概念图「钉住」概念与「只整理选中」布局**~~ ✅ 2026-10-31 第 53 轮已完成：
    `Command::SetConceptPinned` + `concept_layout::arrange_scoped` + 右键菜单单选/多选「钉住位置」「整理选中概念」「整理全部布局」+ 覆盖层钉住徽标；
    金样 `cm-pin` / `cm-arrange-pinned` 已进 `stage-cm`（58/116 → **60/120**），`stage-b` 140/280 两遍全过。证据 `shots/x100/C18/`。
19. ~~第 54 轮计划 **C19 概念图检查器属性（概念元信息、钉住开关与局部整理入口）**~~ ✅ 2026-11-01 第 54 轮已完成：
    改造对象行识别为「概念」与命题/钉住统计、隐藏「含子树」切换、新增「概念排布」分组（键 `concept/layout`）+ 28px `switch_row`「钉住位置」轨道开关与快捷整理按钮；
    金样 `cm-inspector` / `cm-inspector-pinned` 已进 `stage-cm`（60/120 → **62/124**），两基线差 4 659 px，`stage-b` 140/280 两遍全过。证据 `shots/x100/C19/`。
20. ~~打磨轮 55 · **M30 画布缩略卡接回画布**~~ ✅ 2026-11-01 已完成：
    `minimap_panel` 把 `mw_ui::overview` 接进中央画布覆盖层（190×130、视口框、
    点击/拖动定位、滚轮一档缩放、可折叠 34×26），命令面板与顶栏「视图」菜单可开关，
    状态落 `window.json`（出厂关，既有基线零漂移）；折叠命中区 18 → 24；
    顺带修掉 `zoom_around_world` 把 `/k` 写成 `*k` 的锚点 bug，以及
    **夹具旁残留 `.autosave` 让 `mm-cut-*` 启动弹恢复框** 的环境红灯（金样现在不读
    文档旁恢复点）。新基线 `mm-minimap` / `mm-minimap-collapsed` 已进 stage-b。
    证据 `shots/x100/M30/`。
21. 下一刀候选（打磨轮 56）：
    - 概念图命题多选或批量操作体验打磨；
    - 20k 端到端取景/重建剩余 ~1ms（`memo_nodes` 克隆与 `Scene::ids/index` 的 `String` 分配，见账本 M12）；
    - 搜索窄窗「文本 / 拼音」短标签补实拍（账本 M29 的未做项）。
22. ~~第 56 轮 · C20 概念图大纲行菜单~~ ✅ 2026-11-01 已完成：
    概念图大纲补 `⋯` 与行右键菜单（7 条命题网动作，多选只留复制/删除），全部复用既有
    `OA` 动作；新基线 `cm-outline-menu` / `-cancel` 已进 stage-cm（64/128），
    `cm-outline-via` / `-via-clear` / `-unlink` 各 36 px 有意重拍。证据 `shots/x100/C20/`。
23. ~~第 57 轮 · M29b 搜索脚注窄窗~~ ✅ 2026-11-01 已完成：`mode_toggle` 纯函数 + 6 个常量 +
    `search_footer` 轨迹；新基线 `search-narrow`（640×700 + 检查器，ppp 1.5；
    轨迹 `width=359 narrow=1`）。证据 `shots/x100/M29b/`。
24. 下一刀候选（打磨轮 58）：
    - 20k 端到端取景/重建剩余 ~1ms（账本 M12）；
    - 关系线/注解的批量选择与删除体验（画布侧，需先补 `docs/30` 设计）；
    - 缩略卡双击定位节点（`overview` 文档里已写明走 `reveal_center`，账本 M30 的未做项）。

### 7.8 支线交接：开放格式与库管理（2026-11-02 21:0x · 设置 UI 未开工）

> 这条支线**不属于 X100 Stage C**、不占阶段门；改动面在 `crates/mw-shell/src/settings*.rs`、
> `files.rs`、`note_view.rs`、`main.rs` 与 `scripts/x100-stage-b-gate.ps1` 的场景表。
> 与本轮 Stage C 的并行改动不冲突，但同样改 `mw-shell` 与 `stage-b` 场景表，**串行做**。

- **已完成（上一轮机器门绿；当前工作树含并行会话改动，跑门前以复跑为准）**：`docs/34` §11「已实现」表——`.xmind` / `.cxl` / `.md` 三格式双写 + 保真槽 +
  按扩展名分发；`settings::{theme, cardFontSize, vaults, activeVaultId}` 数据模型 + `notes_root()` 优先级 +
  `needs_vault()` + 库 API（都有单测）。
- **一行代码都还没写**：设置面板分区（主题 / 字号 / 库）、启动库门、`files::pick_folder`、
  卡片字号缩放、对应金样。
- **下一棒照做**：`docs/34` **§11.1**（任务 A/B/C/D + 决策点 E + 完成定义 F）——集成点已核对到函数级
  （`ui_kit::segmented`、`apply_theme` / `sync_chrome`、`note_view::LineStyle`、`card_reader_for`、
  门插入点 `main.rs:2132` 之前），不必重新调研。
- **三个先看**：
  1. 设置草稿是**每帧** `self.settings.clone()`；egui 是 immediate mode，滑轨/开关/输入框的值只活一帧。
     加新控件前先改成 `self.settings_draft`（照 `theme_studio` 的 `self.theme_studio` 范式），
     否则主题 4 选、库行点击都不跨帧生效。
  2. 全局主题走 `Command::SetTheme` 会 **revision+1、入撤销栈**（label「更换主题」），
     与「设置是壳层的，不写进文档」有张力；§8.1 又明写「apply_theme（文档层）+ chrome_of（壳层）」。
     写码前先定语义（§11.1 E.1），无论选哪种都要修 `sync_chrome` 无画布回落（现在硬编码 `snow`）。
  3. 库门金样用假路径播种演示库，`needs_vault()` 仍为 `true`（它查 `is_dir()`），
     必须用显式 `vault_demo_seeded` 标志跳过门，否则所有场景都会撞门。
- **金样**：`settings` / `settings-undo-warn` 必重拍，新增 `vault-gate`；
  `card-*`（`k=1.0`）、`start-*`、`theme-*` 必须零漂移；整门 `stage-b` 两遍零差异。
