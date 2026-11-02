# C21 · 三车道以上平行命题的等距避让（打磨轮 63）

> 打磨轮 63 · 2026-11-01
> 账本：`docs/x100/polish-ledger.md` C15 行的「已知粗糙」
> 合同：`docs/30` §5.4「同一对概念之间允许多条命题」；C15 的既有实现与证据 `shots/x100/C15/`

## 1. 为什么是这一轮（先量化，不凭空设计）

C15 把同一对概念之间的多条命题拆成车道：跨源节点收齐、法向按常量
`min(len/6, 24)` 扇开、短语再沿规范切线错开常量 `24`。它解决了
「两条命题完全重合」与「竖边短语叠字」，但留下两条粗糙：

1. **三条以上车道仍用常量步长**。短语沿切线的相邻间距恒为 24，与短语宽度无关。
2. **短边上的相邻短语会真的叠在一起**：车道法向间距 `len/6` 在短边上小于 20
   （短语胶囊高 20），切线又只错开 24，两个轴同时不足以分开。
3. **靠近端点时两条线汇合**是曲线本身的几何事实；短语锚点在中点 + 车道偏移，
   不受汇合影响（读 `scene.rs` 确认：`label_at = 曲线中点 + lane_off + u*stagger`，
   不是端点）。本轮不改曲线，也不改这条几何事实。

> 实现位置说明：任务书写的是 `concept_layout.rs`，但 C15 的车道偏移实际落在
> `scene.rs`（`concept_lanes` / `CONCEPT_LABEL_STAGGER`）。本轮把偏移算法抽成
> `concept_layout::proposition_label_offsets` 纯函数，`scene.rs` 只保留
> 「收车道 → 量宽 → 查偏移 → 加在曲线中点」的透传。

### 1.1 量出来的现状（先写会红的单测，实跑抓到的数）

新增两条场景级单测（三条同向命题；竖边长边 / 水平短边），在**未改生产代码**
的情况下先跑：

```text
three_lane_scene_keeps_24_world_unit_label_clearance
  相邻短语净空 4.00 < 24：
  [(64.56,-13.12,36,20), (40.56,10.88,36,20), (16.56,34.88,36,20)]

short_edge_three_lanes_do_not_overlap
  短边相邻短语净空 0.00 < 24（盒重叠）：
  [(16.56,3.73,36,20), (40.56,10.88,36,20), (64.56,18.03,36,20)]
```

| 场景 | 相邻短语盒的净空（改前） | 结论 |
| --- | --- | --- |
| 3 条平行命题、竖边长边、2 字短语（宽 36） | **4.0** 世界单位（切线 24 − 盒高 20） | 不重叠，但只有目标的 1/6 |
| 3 条平行命题、竖边、8 字短语（宽 108） | **4.0** | 短语变宽**不会**换来更多切线间距 |
| 3 条平行命题、水平短边（裁后 42.88，`gap=7.15`） | **0.0（真重叠）**：x 叠 12.00、y 叠 12.85 | 法向 7.15 < 盒高 20，切线 24 也不够 |
| 2 条平行命题（任何边） | **4.0**（既有行为） | 本轮**必须逐位不变**，见 §3.2 |
| 5 条平行命题、宽窄混排 | 每对都是 4.0（或短边为负） | 常量步长不感知短语宽度 |

> 注：2 车道的净空 4 也不达标，但 `cm-parallel-select-a/b` 与 `cm-crosslink`
> 已经把 ±12 的旧位置锁进像素，**本轮不动 2 车道**；24 净空只对 n ≥ 3 生效。

## 2. 范围

- 新增纯函数 `concept_layout::proposition_label_offsets(label_widths, tangent)`：
  按每条短语沿规范切线的**实测盒半宽**（`(w/2)|ux| + (h/2)|uy|`）累加所需间距，
  保证相邻短语盒在切线方向的投影净空 ≥ 24；整条带以中车道为 0 对称展开。
- `crates/mw-core/src/scene.rs` 的车道组装处改为调用该纯函数：
  车道组收集时顺带度量每条短语宽度；按无向对算一次偏移表；
  主循环按 `(组内序号 k, 组大小 n)` 取偏移。n ≤ 2 走旧公式（逐位一致）。
- 单测放在 `concept_layout.rs` 底部 `tests`：3 车道净空/对称、2 车道旧公式位级锚、
  5 车道长短语混排、斜边、退化输入，以及两条场景级集成测试。
- `scripts/clippy-baseline.txt` 只改两处**行号**（既有 `type_complexity` 因新增代码平移：
  `concept_layout.rs:717→795`、`scene.rs:1051→1045`），没有新增告警。

## 3. 非目标

- 不改 `bowed_straight` 的拱形、不改 `CONCEPT_LANE_GAP` 的法向扇开、不改曲线端点汇合。
- 不改 `mw-render` 的绘制/命中 ABI，不改 `mw-shell`。
- 不改数据结构、`.zmind`、`Node`/`NoteCard`，不引入依赖。
- 不拍基线、不跑 `--update`、不改任何 `scripts/*.ps1`；新场景只给建议（§7）。
- 不动 1 条/2 条命题的任何几何（金样逐字节不变）。

## 4. 验收条款

### 4.1 三车道净空与对称

同一对概念、竖直摆放、三条 2 字短语（宽 36）：

- 切线方向相邻短语盒净空 ≥ 24；
- 偏移序列首尾互为相反数；中车道偏移严格 0；
- 车道曲线保持三条不同的 `Quad`（曲线未动）。

### 4.2 两车道零变化（回归锚）

`proposition_label_offsets(&[w0, w1], u)` 必须**逐位**等于旧公式
`[(0-0.5)*24, (1-0.5)*24]`（用 `f32::to_bits()` 比较）；单条返回 `[0]`。
金样 `cm-parallel-select-a/b`、`cm-crosslink` 不重拍、不漂移。

### 4.3 五车道长短语混排

宽度 `[24, 120, 36, 200, 24]`（含最小宽、长短语）：

- 每两条短语的轴对齐盒净空 ≥ 24（用区间距离，不是中心距）；
- 整条带首尾对称；中车道 0；
- 偏移随车道序号单调递增。

### 4.4 反向命题（B→A）

既有 `opposite_propositions_between_the_same_pair_also_take_lanes` 保持绿：
反向命题仍与正向同组、按规范方向（小索引 → 大索引）取偏移，不被方向翻号。

## 5. 算法与取舍

```text
he_k   = (max(label_w_k, 24) / 2) * |u.x| + (20 / 2) * |u.y|   // 盒在切线上的投影半宽
need_k = he_k + he_{k+1} + 24                                  // 相邻所需切线间距
need'_k = max(need_k, need_{n-2-k})                            // 镜像取大 → 带对称
pos_0 = 0;  pos_{k+1} = pos_k + need'_k
pos_k -= (pos_0 + pos_{n-1}) / 2                               // 整条带以曲线中点为 0
```

- **为什么是切线投影**：短语盒是轴对齐矩形，它在单位向量 `u` 上的投影半宽恰好是
  `(w/2)|ux| + (h/2)|uy|`。两盒投影间距 ≥ 24 ⇒ 盒上任意两点的距离 ≥ 24，
  比直接算轴对齐距离更保守、也更好测；法向的车道偏移垂直于 `u`，不影响投影。
- **为什么不改法向车道间距**：法向决定曲线拱形；动它要重绘所有 3+ 车道曲线。
  切线只挪短语，不挪线，`bowed_straight` 的几何完全不变。
- **为什么镜像取大**：奇数车道的中车道要严格为 0，且整条带对称；宽度不等时
  「对称 + 每对都够宽」只能取两侧需求的较大值。
- **取舍**：水平长短语会得到较大的切线跨度（5 车道例里最外两条相距 556 世界单位），
  短语可能明显偏离中点/伸向端点；换取的是一条曲线都不动、2 车道零漂移。
- **n ≤ 2 特判**：完全走旧算式，连浮点运算顺序都不改，保证金样逐字节。

## 6. 实测结果（2026-11-01）

### 6.1 单测

- **改前（先写会红的测试）**：两条场景级测试分别量到 `净空 4.00` 与
  `净空 0.00（真重叠 x 12.00 / y 12.85）`，与 §1.1 的推导一致。
- **改后**：`mw-core` lib **316 passed / 0 failed**（本包新增 **10** 条：
  `one_lane_label_offset_is_always_zero`、
  `two_lane_offsets_match_the_legacy_constant_step_bit_for_bit`、
  `three_equal_lanes_get_real_clearance_and_stay_symmetric`、
  `three_lanes_on_a_horizontal_edge_pay_for_their_width`、
  `diagonal_tangent_uses_the_projected_half_extent`、
  `five_lanes_with_mixed_widths_never_crowd`、
  `degenerate_inputs_stay_finite_and_spaced`、
  `three_lane_scene_keeps_24_world_unit_label_clearance`、
  `short_edge_three_lanes_do_not_overlap`、
  `mixed_direction_three_lanes_share_one_band`）。
- 改后关键数：3 车道竖边偏移 `[-44, 0, 44]`（相邻净空 24.00）；
  短水平边偏移 `[-60, 0, 60]`（相邻净空 24.00）；5 车道混排相邻净空 24.00–64.00。
- 计数口径：本轮开工时 `mw-core` lib 已是 **305**（门日志 10:31 的 302 是更早快照，
  期间并行代理 +3）；C21 自身新增 **10**，理论应为 315，实测快照 **316**——
  多出的 1 条是并行代理在本轮期间又加的，不记在 C21 名下。
- 既有 `scene.rs` 车道测试（含反向命题、竖边不重叠、单条命题仍走直线）全部保持绿。

### 6.2 格式与静态检查

- `cargo fmt -p mw-core`：干净。
- `cargo clippy -p mw-core --all-targets`：按 `scripts/ci.ps1` 的指纹口径
  **current=14 / baseline=35 / added=0**（mw-core 范围内无新增告警）。
  基线只改了两处行号（见 §2），无新增指纹。

### 6.3 金样

未拍基线、未跑 `--update`。**受影响的既有 concept 图基线：无**。逐张核对：

- 概念图模板（`start.rs` 的 `Shape::Concept`）只有 5 条链接、**没有任何一对概念
  有两条以上命题**；`cm-*` 基线用的就是这张模板，全部走 `ln == 1`（几何不变）。
- `MW_CONCEPT_CROSS=1`（`cm-crosslink`）补的两条是 `用户痛点→竞品对比`、
  `增长策略→技术方案`，都是模板里没有的新对，`ln == 1`。
- 唯一的多命题夹具是 `MW_CONCEPT_PARALLEL=1` 给 `用户痛点→增长策略` 补的第二条，
  即 `cm-parallel-select-a/b` 的 `ln == 2`——走纯函数里**逐位保留的旧公式**。
- `cm-crosslink` 不与 `MW_CONCEPT_PARALLEL` 同场景（门脚本里两者分开）。

因此本机未跑 stage-cm 门；理论漂移为 0，仍请用户复跑确认。3+ 车道没有既有基线。

## 7. 新基线 `cm-parallel-3`（打磨轮负责人已落地）

**旋钮**：`MW_CONCEPT_PARALLEL` 从布尔改成数量：值是**再补几条**，
`=1` 与 C15 行为逐位一致（补「推动」，共 2 条），`=2` 再补「阻碍」`=3` 再补「依赖」
（壳层 `add_concept_parallel_links(extra)`，clamp 1–3）。

**基线**：`shots/golden/cm-parallel-3`（`--tol 0` 双跑零差异，sha256-12 `6BA2BF3A32FE`），
无输入脚本（静态图）。**门行**（已加进 `scripts/x100-stage-cm-gate.ps1`）：

```powershell
@{ Name = 'cm-parallel-3'; Dir = 'shots/golden/cm-parallel-3'; Ppp = '1.0'; Warmup = 24; Env = @{ MW_CONCEPT_DEMO = '1'; MW_CONCEPT_PARALLEL = '2'; MW_WINDOW_SIZE = '1280x800' } },
```

既有 `cm-parallel-select-a/b` 用 `=1`，行为未变。

## 8. 风险 / 未做

- 只保证**切线投影**净空；短语盒的轴对齐净空只会更大（保守），不会更小。
- 短边（裁后长度 < 120）上法向间距仍可能 < 20；切线自适应把标签沿边挪开，
  但整条带可能伸到端点外（C15 已记的既有取舍）。
- 端点汇合区的点选歧义是曲线几何，未改。
- 长短语在水平边上会让标签带变宽（5 车道最外两条相距 556），这是「不重叠」的必要代价；
  若嫌太散，下一轮可考虑「法向也参与分配」，但那会改车道曲线，需单独批准。
- 实机鼠标手感 / 高 DPI 未核。
- 本机 stage-b 门第 2 遍红在 `start` / `start-scroll` / `start-tab-focus`
  （16.8% 像素差，来自并行代理对 `crates/mw-shell/src/start.rs` 的改动），
  与本包（只动 `mw-core`）无关；本包未跑完整两道门。
