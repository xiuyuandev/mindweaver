# C9 · 强调淡入的中间帧（打磨轮 3）

> 依据：`docs/x100/polish-ledger.md` §1 C9 行与 §3 第 3 条
> （「给渲染路径加一个只给验收用的 alpha 乘子，拍一张定格」，对应 CM-10 §3 第 10 条）。
> 状态：**中途那一帧有基线、有数值证据**；实机窗口里的过渡手感仍 `未验证`。

## 1. 这一轮要解决什么

账本对 C9 的判词很短：`emphasis_alpha` 只有 5 条断言、**只有稳态基线**，
「淡入到一半」的那一帧没人拍过。查下去发现不是一个「没人拍」的问题，而是**三层断链**，
任何一层没修，定格都拍不出一张有意义的图：

| 层 | 原状 | 后果 |
| --- | --- | --- |
| 金样 | 离屏一律 `reduce_motion = true`（`app_methods_1.rs` 里 `c.set_reduce_motion(... \|\| harness.is_some())`） | 截图的墙钟永远追不上 0.2–0.4s 的过渡，**中间帧根本不存在** |
| 过渡 | `motion_span` 只量节点盒；概念图里**加一条命题线不挪任何盒子** | 新线从来没有 `Motion`，`blend_scenes` 里那条 `r.alpha = t` 的淡入分支**永远轮不到** |
| 强调 | `paint_relationship_emphasis` 读的是 `self.scene`（目标场景），不是这一帧的插值场景 | 就算线在淡入，强调也按目标场景的 `rel.alpha = 1.0` 画——正是 `emphasis_alpha` 注释里说要挡的「幽灵线」 |

改之前实测（同一份环境拍两张）：`mid` 与 `end` 两张基线 **md5 完全相同**
（`d85d4271…` / `26c5cdb4…`），也就是「钉了 0.5 也什么都没变」。三条都补齐之后，
同一组环境的 `mid` 有 1926 个像素与 `end` 不同（bbox `(599,442)-(924,854)`，即那条新线）。

## 2. 非目标

- 不改任何过渡时长令牌（还是 0.20–0.34s），不改缓动曲线，不改线条宽度与三档强调的档位（3 / 2 / 1.8px，1.0 / 0.45 / 0.30）。
- 不为概念图新增数据结构，不动 `.zmind` / `Node` / `NoteCard` / `Relationship` 字段。
- 不改思维导图的过渡语义：`motion_span` 只多认「关系线 id 集合变了」这一件事。
- 不拍 0.25 / 0.75 的中间帧，也不拍 150% / 200% ppp。

## 3. 改了什么（代码）

| 位置 | 改动 |
| --- | --- |
| `mw-shell/src/canvas.rs::motion_pin` / `parse_pin` | 新证据开关 `MW_MOTION_T=<0..1>`：把 `blend_scenes` 的混合系数钉死，定格停在那一帧。空白 / 非数 / `NaN` 一律当「没设」，其余夹到 `0..=1` |
| `canvas.rs::Motion::{active, scene}` | 有钉死值时用常量 `t`，不再看墙钟 |
| `canvas/motion.rs::motion_reduced` + `Canvas::motion_reduced` | `reduce_motion && 没有钉死值`。金样照旧一帧过渡都不跑；只有明确要拍定格时才放开**场景过渡**这一路（悬停/选中的 alpha 仍瞬时到位，画面依然确定） |
| `canvas/motion.rs::motion_span` | 关系线 id 集合变了（增/删）也算「结构变了」，跨度取 0（⇒ 用最短时长 0.20s）。按 id 建哈希表比较，上千条线不退化成平方 |
| `mw-core/src/scene.rs::blend_scenes` | 共有节点 / 连线 / 关系线 / 边界 / 概要在插值时**同时插 `alpha`**。动画中途再来一条命令时 `rebuild()` 会把此刻那一帧固化成新起点；只固化位置、把透明度直接跳到 1.0，淡入会当场断掉 |
| `canvas/canvas_methods_3.rs::rebuild` | 定格模式下不重取快照（否则 t 会叠成 t²：钉 0.5 拍出来是 0.75）。**只在设了 `MW_MOTION_T` 时走这条路** |
| `canvas/canvas_methods_3.rs::paint_relationship_emphasis` | 新增 `scene: &Scene` 参数，改读**这一帧的插值场景**（`animated`），不再是目标场景。概念提示：不是 `scene_for_frame`——那是演示副本，下标与 `self.hovered` / `self.relationship` 不同源 |
| `canvas/canvas_methods_3.rs::trace_rel_emphasis` | 新轨迹：每条强调线一行 `rel_emphasis id=… tier=… rel_alpha=… alpha=… w=…`，值没变不重复写；只在 `MW_TRACE` 下进 |
| `trace.rs::enabled` | 由私有放宽为 `pub(crate)`，供上面那条轨迹做零开销短路 |

**顺带的产品行为变化**（有意为之，记在这里供复核）：概念图里**新增 / 删除一条关系线**现在也会有一段
0.20s 的淡入淡出，不再硬弹出来。这是 `blend_scenes` 里 `alpha = t` 分支存在的理由，
也是 `emphasis_alpha` 乘 `rel.alpha` 的前提；不动时长令牌。

## 4. 验收：一张新基线 + 一张对照

两张图是**同一份文档、同一帧布局**，差别只有关系线的透明度：

| 场景 | 环境 | 基线 |
| --- | --- | --- |
| `cm-link-fade-mid`（新，进门） | `MW_CONCEPT_DEMO=1`、`MW_CONCEPT_LINK='用户痛点|风险与依赖'`、**`MW_MOTION_T=0.5`**、`--ppp 1.0`、`--warmup 20` | `shots/golden/cm-link-fade-mid/shell-ppp1.00.png`，sha256_12 = **`3A43A517AA48`** |
| `cm-link-create`（既有，未重拍） | 同上但**不设** `MW_MOTION_T` | `shots/golden/cm-link-create/shell-ppp1.00.png`（稳态对照） |

两张图逐像素差 1926 点、bbox `(599,442)-(924,854)`（新线整条）。沿新线量线身
（行 600–859、列 640–929 内逐行取最暗像素，248 行取平均）：

| 帧 | 线身平均亮度（0–255） | 取样像素 `(887,808)` |
| --- | --- | --- |
| `cm-link-fade-mid`（t=0.5） | **117.4** | `(119,153,142)` |
| `cm-link-create`（t=1.0） | **81.1** | `(47,111,94)` |

`mid` 的线身对得上「强调色 50% 叠在底色上」：`0.5×47 + 0.5×252 ≈ 150`，
再被 3px 强调线按 0.5 叠一次，落到 119——**既不是纯 50%（150），也不是稳态（47）**，
两条证据正好夹住它。

`--tol 0` 双跑逐字节相同（`target/x100/gate-cm-c9.log`：45 场 / 90 次比较全过，
44 张既有 `cm-*` 基线一张没动）。

## 5. 轨迹证据（`shots/x100/C9/trace-fade-*.txt`）

```
event rel_emphasis id=<关系 id> tier=<档位> rel_alpha=<这一帧的过渡透明度> alpha=<tier×rel_alpha> w=<线宽>
```

两帧各只写一行（这一帧只有那条被选中的新线在强调）：

| 轨迹 | 关键行 |
| --- | --- |
| `trace-fade-mid.txt` | `event rel_emphasis id=01a183ac-9ab9-… tier=1.00 rel_alpha=0.500 alpha=0.500 w=3.0` |
| `trace-fade-end.txt` | `event rel_emphasis id=01a183ac-9e69-… tier=1.00 rel_alpha=1.000 alpha=1.000 w=3.0` |

像素只能证明「这条线亮了」，这一行才能证明**它按 0.5 亮而不是按 1.0 亮**——
强调确实乘了 `rel.alpha`，而且乘的是**这一帧**的值。

## 6. 复跑

```bash
cargo build --release -p mw-shell
# 定格（新基线就是这样拍的；写到 target/ 免得覆盖证据）
MW_CONCEPT_DEMO=1 MW_CONCEPT_LINK='用户痛点|风险与依赖' MW_MOTION_T=0.5 MW_WINDOW_SIZE=1280x800 \
  ./target/release/mw-shell.exe --golden --dir target/x100/c9-mid --ppp 1.0 --warmup 20
# 稳态对照
MW_CONCEPT_DEMO=1 MW_CONCEPT_LINK='用户痛点|风险与依赖' MW_WINDOW_SIZE=1280x800 \
  ./target/release/mw-shell.exe --golden --dir target/x100/c9-end --ppp 1.0 --warmup 20
# 轨迹（两份 trace-fade-*.txt 都这样生成，再拷进本目录）
MW_TRACE=target/x100/c9-mid.txt ... # 上面两条命令各加一次，设了才写

# 双门（与本目录数字同源）
python target/x100/gate_cm.py target/release/mw-shell.exe      # 45 场 / 90 次
python target/x100/gate_golden.py target/release/mw-shell.exe  # 87 场 / 174 次

# 单测
TEMP="$(pwd)/target/tmp" TMP="$(pwd)/target/tmp" cargo test --workspace -- --test-threads=1
```

本轮实测：测试 **915** passed / 0 failed / 1 ignored（+2：`the_motion_pin_clamps_and_ignores_junk`、
`motion_span_sees_a_new_relationship_when_no_box_moves`）；fmt 干净；clippy 37 条基线与当前指纹一致、
**新增 0**；stage-b **87 场 / 174 次比较**全过（`target/x100/gate-b-c9.log`）；
stage-cm **45 场 / 90 次比较**全过（`target/x100/gate-cm-c9.log`）。

## 7. 未做 / 风险

1. **定格不是实拍**：`MW_MOTION_T` 让混合系数每帧都取 0.5，画面确定但**不是**在真实过渡里抓的一帧。
   真实窗口里 0.2s 的淡入读起来顺不顺、掉不掉帧，没有证据（同一类缺口：M1/M2 的实机手感）。
2. **只拍 t=0.5**：0.25 / 0.75 没拍；`MW_MOTION_T` 的解析有单测（空白 / `NaN` / `inf` / 越界），但只有 0.5 有图。
3. **定格下不重取快照**：连续两条结构性命令（例如删节点后立刻撤销）在定格里只能看到「从最初起点插到当前」，
   不能表达「第二段过渡」。这是定格工具本身的边界，写在这里免得下一任把它当 bug。
4. **强调线的像素与线身重叠**：强调色叠在同色线身上，`mid` 里只能量出「比纯 50% 更深」，
   拆不出强调单独那一份；这一份由 `rel_emphasis` 轨迹背书，不是像素。
5. **新线淡入是行为变化**：概念图新增/删除关系线现在有 0.20s 过渡，实机观感未核；若嫌拖沓，
   改 `motion_duration(0.0)` 的下限即可（`design::motion::SCENE_MIN`）。
6. **只拍 ppp 1.0**：150% / 200% 下线宽 1.8 / 3.0 与透明度的组合没拍。
