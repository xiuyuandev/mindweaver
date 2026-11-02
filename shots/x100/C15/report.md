# C15 · 平行命题的短语避让与独立点选（打磨轮 39）

- 日期：2026-10-31
- 依据：`HANDOFF.md` §7.5⑤「平行命题在画布上的独立点选（现在靠检查器「命题」组进）」；
  `docs/30` §5.4「同一对概念之间允许多条命题」；打磨轮 34（C14）的车道实现。
- 状态：**已实现**（机器门绿；实机鼠标手感未核，人工门未开，不记 `已验证`）

## 1. 范围

C14 让同一对概念之间的多条命题各走一条车道（法向扇开，单条零偏移）。
这一轮处理它留下的两个可见问题：

1. **短语胶囊叠字**：车道间距上限 24 世界单位，而短语胶囊高 20、宽 ≥ 36。
   边**竖直**时法向是水平的——两条短语的中心只差 24，横向却各占三十几个单位，
   于是第二条命题的短语压住第一条。水平边、斜边不重叠，所以既有基线一张都没拍到。
2. **独立点选没有端到端证据**：`HitIndex` 给每条关系线单独登记了路径，
   理论上两条车道各点各的；但没有任何单测/脚本证明「点第一条选第一条、点第二条选第二条」。
   HANDOFF 把这笔记成「未做」，需要证据来判断是行为缺口还是文档过期。

## 2. 非目标

- 不改数据结构、不改 `.zmind`、不升 `FORMAT_VERSION`。
- 不新增依赖、不动 `Frame` ABI。
- 不改车道弯曲算法本身（`CONCEPT_LANE_GAP` / `bowed_straight` 的拱形）。
- 不动单条命题的任何几何（既有基线必须逐字节不变）。
- 不做「关系线端点箭头错开」——两条车道在终点汇合是设计使然。

## 3. 验收条款

### 3.1 短语不叠（单测）

同一对概念竖直摆放（`(0,-220)` 与 `(0,220)`），两条命题短语「导致」「支持」：

- 两条关系线都进场景；
- 两个短语胶囊（`label_at` ± `label_w`/2、高 20）的矩形**不相交**；
- 单条命题仍走原来的中点，`label_at` 不动。

### 3.2 独立点选（单测 + 端到端）

- `mw-render` 单测：两条平行命题各自路径中点处 `HitIndex::hit` 返回自己的下标。
- 演示钩子 `MW_CONCEPT_PARALLEL=1`：在示例概念图的「用户痛点 → 增长策略」（近竖直边）
  上补第二条命题「推动」，形成车道。
- 输入脚本分别点两条车道的短语/线段中点，轨迹 `select relationship <from> <id>` 必须
  给出**两个不同的 id**；两张最终态基线各锁一条被选中的强调线。

### 3.3 既有基线零漂移

不设 `MW_CONCEPT_PARALLEL` 时车道数仍是 1，`label_at` 不变；既有 `cm-*` 基线不重拍。

## 4. 预计证据

| 类型 | 内容 |
| --- | --- |
| 单测 | 竖边平行短语不重叠；两条车道中点各自命中；既有 C14 车道测试仍绿 |
| 轨迹 | `scene_rel` 每条关系线的端点/标签屏幕坐标；两次 `select relationship` |
| 金样 | `cm-parallel-select-a`、`cm-parallel-select-b`（两张各选一条车道） |
| 像素 | 两基线之差应只落在两条短语/强调线附近；与只打开演示的对照也要有可解释的差 |

## 5. 实现

| 文件 | 改动 |
| --- | --- |
| `crates/mw-core/src/scene.rs` | 车道分组改为**跨源节点**一次收齐：按无向对 `(min,max)` 把 `doc_order()` 里所有源节点的 `relationships` 收进同一张表（反向命题 B→A 挂在 B 身上，各分各的组会让 A→B 与 B→A 都成「组内一条」、画成重合直线）。`owner_ids` 只取一次，车道收集与元素绘制共用。 |
| 同上 | 新增 `CONCEPT_LABEL_STAGGER = 24`：多条车道时短语中心沿**规范方向**的切线错开 `(k-(n-1)/2)*24`；单条车道的 `label_at` 仍是原来的路径中点，既有基线不动。 |
| 同上 | `bow_crossings` 跳过已经是二次贝塞尔的车道线：再按端点弦线重弯会把车道偏移抹掉，两条命题又叠回一条。 |
| `crates/mw-render/src/hit.rs` | 新增 `parallel_propositions_are_individually_hittable`：竖放两颗概念、两条反向命题，各自路径中点必须命中自己的下标。 |
| `crates/mw-shell/src/canvas/canvas_methods_2.rs` | `scene_rel` 轨迹：每条关系线的 `start/end/mid/label/w` 屏幕坐标 + id，脚本点车道不用再靠猜。 |
| `crates/mw-shell/src/canvas/canvas_methods_3.rs` | `add_concept_parallel_link()`：示例图「用户痛点 → 增长策略」补第二条命题「推动」（仅验收钩子调用）。 |
| `crates/mw-shell/src/app_methods_1.rs` | `MW_CONCEPT_PARALLEL=1` 分发。 |
| `shots/golden/inputs/cm-parallel-select-{a,b}.txt` | 分别点两条车道的短语中心（坐标取自 `scene_rel` 的 `label=`）。 |
| `scripts/x100-stage-cm-gate.ps1` | 新增 `cm-parallel-select-a` / `-b` 两场。 |

没动数据模型、`.zmind`、`Frame` ABI 与 `CONCEPT_LANE_GAP` 的拱形；单条命题的几何逐字节不变。

## 6. 证据

### 6.1 单测

| 测试 | 修复前 | 现在 |
| --- | --- | --- |
| `mw-core` `opposite_propositions_between_the_same_pair_also_take_lanes` | 两条反向命题的 `label_at` 距离 **0.00**（按源节点分组导致零偏移重合） | 两条车道分离，短语不重叠 |
| `mw-core` `parallel_proposition_labels_do_not_overlap_on_a_vertical_edge` | 竖边上两个 36 宽的短语中心只差 24，**横向重叠 12 世界单位** | 沿切线错开 24，两短语盒不相交 |
| `mw-core` `crossing_bow_leaves_a_lane_curve_alone` | 无护栏时 `bow_crossings` 会把车道曲线按弦线重弯 | 车道 Quad 路径保持原样 |
| `mw-render` `parallel_propositions_are_individually_hittable` | 无此证据 | 两条反向命题各自路径中点命中自己的下标 |

计数：`mw-core` 253 passed、`mw-render` 27 passed、`mw-shell` 592 passed / 1 ignored，
全工作区 **1029 passed / 0 failed**（串行，TEMP/TMP 指向 `target/tmp`）。

### 6.2 轨迹（落定相机，1280×800 / ppp 1.0）

`MW_CONCEPT_DEMO=1 MW_CONCEPT_PARALLEL=1` 下两条车道的 `scene_rel`：

```text
idx=3 id=…6906ffc8b875 from=用户痛点 to=增长策略 start=(506,411) end=(544,768) mid=(543,588) label=(541,570) w=36
idx=4 id=…690870df5df7bc from=用户痛点 to=增长策略 start=(506,411) end=(544,768) mid=(507,591) label=(509,609) w=36
```

点 **A** 脚本（541,570）→ `select relationship …6906ffc8b875`（idx=3）；
点 **B** 脚本（509,609）→ `select relationship …690870df5df7bc`（idx=4）。
同一对 `from/to`、两个不同的关系 id——独立点选不是「碰巧都选中第一条」。

### 6.3 金样与门

| 项 | 值 |
| --- | --- |
| `cm-parallel-select-a` | 1920×1200，sha256 `30eca0171375…`，139 438 B |
| `cm-parallel-select-b` | 1920×1200，sha256 `d36f6fbaa531…`，139 611 B |
| A/B 差 | **2 528 px**，bbox `x500..549 y409..765`——只落在两条车道与短语所在的竖带上 |
| 门 | `stage-cm` 51 → **53 场 / 102 → 106 次比较**；连跑两遍全绿，既有 51 张基线零漂移 |
| 日志 | `target/x100/stage-cm-gate.log`（2026-10-31 02:2x 收口） |

`stage-b` 未加场景、未重拍。`doc_order()` 只取一次属于常数优化：输出逐像素不变，两遍门为证。

## 7. 风险 / 未做

- 三条以上车道时短语沿切线错开的步长是常量，边极短时短语可能越出线段端点（可接受，
  标签本来就允许压在线外）。
- 实机手感（点车道是否容易点中）未核；离屏只证明判定正确。
- 关系线在终点汇合，靠近端点的点击仍会选中后一条；独立点选只在车道分离区成立。
- 反向命题的车道顺序取决于 `doc_order()`（源节点在文中的先后），不是稳定的「上/下」语义；
  用户若期待固定左右，还需要一条产品规则。
