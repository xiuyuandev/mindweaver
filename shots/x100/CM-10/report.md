# CM-10 · 对标 CmapTools：拖到空白处新建概念（①）

- 批次：概念图打磨（用户 2026-10-27 新目标）
- 状态：**已实现**（①②③④⑤⑥⑦⑧ 全部 `已实现`；⑥ 的四个落点齐：画布带子 / 检查器 / 三处导出 / 大纲页就地改，见 §2.13–§2.17；⑦ 四档箭头 + 双箭头几何 + 三处导出 + 两处入口；§3 自定对标打磨 10 项全部收口，证据齐备，机器门绿；人工门未复判，不记已验证）
- 设计依据：`docs/30` §12（本轮新增的 CM-10 差距清单）

## 目录

- [§1 本次做了什么](#1-本次做了什么)
- §2 证据（40 个小节，按夜里做的次序排；每节都以「判断 → 证据 → 未做」为骨架）
  - **前半夜（02:5x–07:0x，轮次 2–26）**：2.1 – 2.10 把 ① 落空新建、② 双击空白、⑦ 箭头四档、
    门的指纹化改造逐条做完；2.11 – 2.26 是 ⑥ 聚焦问题从「字段」做到「看得见、改得动、导得出、大纲页也能改」，
    以及 ⑦ 的两条激活路径（键盘 / 指针）。
  - **晚 18:00–20:20**：2.26 – 2.29 分别锁住带子双击改问题、排列子菜单夹取、带子悬停态、检查器四格指针路径。
  - **夜 20:20–23:00（对标 CmapTools 的收尾打磨）**：2.30 – 2.40 —— 关系线的悬停/选中强调、弯线的药丸缺口、
    指概念点亮命题线、拖终点时抑制强调、强调跟随淡入淡出、药丸矩形的两条单测、挖洞的退化输入、
    看不见的线不再画、没有强调对象时整段跳过、基线尺寸自检、时间戳校正、引用体检。
- [§3 未做 / 风险](#3-未做--风险)

> **关于本文的时间戳**：各小节标题里的 `HH:MM` 是那一轮收尾的时刻。早先几轮是我凭感觉写的，
> 偏了 20–40 分钟；现已统一按 `target/x100/*.log` 的 mtime 校正（例：第 40 轮 = `ci-r40.log` 22:08、
> 第 44 轮 = `ci-r44.log` 22:31）。要复核就查日志时间，别信记忆。

## 1. 本次做了什么

CmapTools 的核心手势之一：从概念边缘拖一条线到**空白处**松手，落点直接长出一个新概念并连上。
织思原来只有「拖到已有概念才连线」——`commit_link()` 在 `l.to == None` 时 `return false`。

改动（不动数据结构、不加新 crate）：

| 文件 | 改动 |
| --- | --- |
| `canvas/canvas_methods_3.rs` | `commit_link()` 落空分支：概念图里调 `insert_floating_world()` 在落点建概念，再建边；非概念图仍返回 false |
| 同上 | 新增 `create_concept_link_blank(from, dx, dy)`：按源概念 + 相对偏移造一次落空拖拽，供 golden 复跑 |
| `canvas/canvas_methods_1.rs` | `insert_floating_world()` 提为 `pub(crate)`（同一 crate 内的兄弟方法调用） |
| `canvas.rs` | `ConceptHook::LinkBlank(String, i32, i32)`（用整数偏移是因为 `ConceptHook` 派生 `Eq`，`f32` 不满足） |
| `app_methods_1.rs` / `main.rs` | `MW_CONCEPT_LINK_BLANK="源|dx|dy"` 解析与分发 |

语义与 `link_concept_below()`（大纲 `Tab`）一致：**概念 + 边一次成型**，短语留空并立刻立起短语编辑框，留空则连边带概念一起撤。

## 2. 证据

| 项 | 值 |
| --- | --- |
| 场景 | `cm-link-blank`（`MW_CONCEPT_DEMO=1`、`MW_CONCEPT_LINK_BLANK=用户痛点|240|40`、1280×800、ppp 1.0、warmup 10） |
| 基线 | `shots/golden/cm-link-blank/shell-ppp1.00.png`，1920×1200，sha256 `82d51198b8c8` |
| trace | `target/x100/t-cm-link-blank.txt`：`link_new_concept from=… to=… at=-90,-130`、`concept-hook ran=true` |
| 门 | `scripts/x100-stage-cm-gate.ps1` 已加入该场景（14 场景） |

### 2.1 第二轮补强（2026-10-27 02:5x）

#### ② 双击空白其实是现成的

核查结论：画布双击落空**已经**走 `add_floating_world()`（`canvas_methods_2.rs:316`）——选中并直接进行内编辑。
所以 CM-10 第 2 条不是「没做」，是我上一轮写「待核查」时没查完。文档已改成 `已实现`。

#### ① 的撤销缺陷：从两步改成一步

原实现是 `AddFloatingTopic` + `AddRelationship` **两次 apply**，也就是两条撤销记录：
用户按一次 Ctrl+Z 只撤掉边，留下一个**孤儿概念**。这和 CmapTools 的「一步回去」不一致。

`Command::Batch` 用不上——它要求子命令自带 id，而新概念的 id 是 apply 时才生成的。
所以新增原子命令 `Command::AddLinkedConcept { from
 text
 x
 y }`：

- `apply`：校验 → 建节点（落点是盒子左上角）→ 往 `from` 推一条边 → `ChangeSet` 同时快照 `from` 与新节点；
- `undo`/`redo`：沿用既有的快照机制，一条记录进出；
- 复用点：画布落空分支 + 大纲 `Tab`（`link_concept_below()`），两处都从两步变一步。

单测：`mw-core` `add_linked_concept_undoes_in_one_step` → **1 passed**（撤销后节点 2→1、边 1→0；重做一步回来）。

回归：`cargo test -p mw-shell --bin mw-shell` → **446 passed / 0 failed**；`cm-link-blank` 重跑 `--tol 0` → **无超容差差异**（换命令后画面逐像素不变，只有撤销步数变了）。

### 2.2 第三轮：落空预览（2026-10-27 03:1x）

问题：拖到空白处时，曲线到指针就断了，用户**看不出松手会发生什么**——是连线？还是新建？原代码两边都无声。
（本轮之前落空分支直接 `return false`，连「会发生什么」都没有；现在会新建概念，预览就更必须存在。）

实现（`canvas.rs` `paint_link`）：概念图且 `l.to.is_none()` 时，在指针处画

- 96×36 的圆角占位框（accent 8% 填充 + 65% 描边，尺寸按 `camera.zoom` 缩放）；
- 框下方 14px 一句 `松手新建概念`（12px）；
- 目标概念上方已有的强调环保持不动（`l.to.is_some()` 分支）。

#### 证据：A/B 逐像素差分

同一场景两张图——`MW_CONCEPT_LINK_DRAG=用户痛点|240|40`（停在拖拽中）与不拖（其余完全相同）：

| 项 | 值 |
| --- | --- |
| 差异像素 | **6472** |
| 差异包围盒 | x 641..880、y 419..497（240×78，正好框住占位框 + 提示文字 + 曲线终点） |
| 场景基线 | `shots/golden/cm-link-drag/shell-ppp1.00.png`，1920×1200，sha256 `76d5a65e93ea` |
| 门 | `scripts/x100-stage-cm-gate.ps1` 增至 **15 个场景** |

> 第一次 A/B 差 20205 px、包围盒铺满全图——原因是钩子里顺手把源概念选中了，选中环也进了差。
> 去掉选中后重拍，差才收窄到 6472 px / 240×78。这一步是必须的：不修的话「占位框可见」这句话就没有可信的测量支撑。

### 2.3 第四轮：落点语义自证（2026-10-27 03:3x）

上一轮我按「指针为中心」画占位框。查实后这是**假的**：

- `mw-core` `floating_topic_honours_its_stored_position` 断言 `rect.x == pos.x`（`scene.rs:1687`），
  即 `pos` 是盒子**左上角**；
- 而 `commit_link` 落空分支把 `l.at`（指针）原样当 `pos` 传下去。

两者一合：真正长出来的概念，左上角在指针处——占位框却以指针为**中心**画，
等于向用户承诺了一个偏移了半个盒子（48×18）的位置。

修法：占位框改 `Rect::from_min_size(p3
 …)`，与双击空白（`add_floating_world`）同一口径。

#### 证据

| 项 | 值 |
| --- | --- |
| 单测 | 在 `add_linked_concept_undoes_in_one_step` 里加上「`pos` 原样存下」断言 → **1 passed** |
| A/B 差分（修正前） | 6472 px，bbox 240×78（x641..880
 y419..497） |
| A/B 差分（修正后） | 6665 px，bbox **298×100**（x641..939
 y419..519） |
| 位移方向 | 右边 +59、下边 +22 —— 与「去掉半个盒子（48×18）+ 提示文字移到框下」吻合 |
| 基线 | `shots/golden/cm-link-drag/shell-ppp1.00.png`，sha256 `450731e36eff` |

> 这条修的**不是像素好不好看**，是「预览有没有说实话」。同样的错在别处也会有：
> 凡是「先把结果画出来」的地方，都得跟落盘语义对一遍。

### 2.5 第五轮：把「落空 = 新建」推广到**改线**（2026-10-27 03:5x）

发现的不一致：`commit_relink` 拖到空白是 `return false`——**静默取消**，预览也只画到指针。
同一个手势族里，「从概念拖到空白」会新建概念，「把线改到空白」却什么都不发生，用户没有线索区分。

实现：

- 新原子命令 `Command::RetargetLinkToNewConcept { from
 id
 text
 x
 y }`：建概念 + 把关系线改指到它，
  `ChangeSet` 同时快照两端 → **一步撤销**；
- `commit_relink` 落空分支改走它（仅概念图），并选中改指后的那条线；
- 占位框抽成 `paint_blank_concept_hint()`，由「拉新线」与「改线」**共用**——两份画法各自演化迟早会不一致；
  改线时的提示文案是「松手新建概念并改线」。

#### 证据

| 项 | 值 |
| --- | --- |
| 单测 | `retarget_link_to_new_concept_undoes_in_one_step` → **1 passed**（3 节点 → 撤销回 2 节点且线还原到原目标） |
| trace | `event relink_new_concept 01a17933… 01a17933… -> 01a17933…` |
| A/B 差分 | **5534 px**（跨整图：整条折线改道 + 新概念） |
| 复跑 | `--tol 0` → 无超容差差异（1920×1200） |
| 基线 | `shots/golden/cm-relink-blank/shell-ppp1.00.png`，sha256 `1473142552bf` |
| 门 | 15 → **16 场景** |

#### 差点交出一张假 golden

第一版钩子我按短语 `字体同源` 找关系线——那是我从 `session.rs` 里抄的，**不在**概念图模板里。
结果：`cargo golden` 正常出图、复跑也 0 差异，但 A/B 对照（不带钩子）**diff_pixels=0**——
说明钩子什么也没做，那张图只是普通演示图。

真实短语是 `解决/支撑/参照/转化/带来`（`start.rs:383`）。换成 `带来` 后差分 5534、trace 出现事件。

> 教训：**golden 拍成功 ≠ 被测行为发生过**。带钩子的场景必须配一张不带钩子的对照图做 A/B，
> 否则「0 差异」既可以是被测行为正确，也可以是钩子压根没跑。

### 2.6 第六轮：补上上一轮自己标出的不一致（2026-10-27 04:2x）

上一轮 §4 债务第 1 条：改线到空白后短语框不立起，而拉新线会立起。
同一个手势族里，一条能直接打短语、另一条要先双击，说不通。

改法：`commit_relink` 落空分支收尾调 `start_rel_edit(sel
 viewport
 true)`，与 `finish_new_link` 同构。

#### 证据

| 项 | 值 |
| --- | --- |
| trace | `event relink_new_concept … -> 01a17936…` 与 `event rel-edit-open screen=(1263.1
865.6) zoom=1.237` 同帧都在 |
| 编辑框区域差分 | (1173..1353
 834..896) 内 **261 / 11403** 像素不同（框线与光标画在那里） |
| 双跑 | `--tol 0` 无超容差差异 → 光标闪烁**不会**让基线飘 |
| 基线 | `shots/golden/cm-relink-blank/shell-ppp1.00.png`，sha256 `90d1febf6076` |

> 全图差分从 5534 掉到 5300，一开始像是「编辑框没画」。其实相反：
> 编辑框的底色盖住了那块本来在差异里的连线，差异数变小反而不能当证据——
> 所以我按 trace 给的坐标单独量了编辑框那一小块。**总差分数不等于某处画了东西**。

### 2.7 第七轮：回归面大扫，一次抓到 5 个问题（2026-10-27 05:2x）

前四轮都在「加东西」，这轮换成只看不动：跑全量工作区测试 + 16 个 `cm-*` 场景两遍 `--tol 0` + clippy。
结果是**红的**——而且红得有价值。

| # | 问题 | 性质 | 处置 |
| --- | --- | --- | --- |
| 1 | `chrome::tests::shell_sources_do_not_hardcode_font_sizes` **FAILED** | 我第 13 轮引入的回归：占位提示写死 `FontId::proportional(12.0)` | 换 `design::content::META.size`（同为 12.0，像素不变） |
| 2 | `clippy::needless_borrow` @ `canvas_methods_2.rs:1651` | 我多写了一个 `&` | 去掉 |
| 3 | `clippy::too_many_arguments` @ `canvas.rs` `paint_link` | 我第 13 轮加的 `concept: bool` 把参数顶到 8/7 | 显式 `#[allow]` + 说明为什么拆结构体只是把 `>` 换成 `.` |
| 4 | 4 条死代码告警（`Heading` / `headings` ×2 / `created_at`） | 不是我这轮写的：常驻右目录已按 `docs/29` CER-13 删除，解析器留在原地没人调 | 保留并 `#[allow(dead_code)]` + 写明「解析有测试覆盖，删了会丢回归网」 |
| 5 | `cm-card-link` 漂移 **32014 px（1.389%）** | 基线过期 | 重拍，新 hash `C991545A4DA4` |

#### 两个必须说清楚的判断

**① 漂移不是抖动。** 连跑两次取 hash：`a1=C991545A4DA4 a2=C991545A4DA4`（相同），而 `base=B7D11182B516`（不同）。
如果是暖机不足的动画抖动，两次 actual 之间就会不同；相同就说明渲染真的变了，只能重拍，不能靠多跑几遍碰运气。

**② 漂移原因只是推断，没有证实。** `cm-card-link` 会直接打开卡片页（`MW_CONCEPT_CARD=1`，`app_methods_1.rs:393`），
而差异分布正是卡片页的形状：y0..39 整幅（40px 顶栏）、y120..159 整幅（标题带）、y160..479 集中在 x473..1346（720 版心正文）。
这与 B2 卡片页排版改动吻合，`cm-card-link-back`（更晚拍的基线）不漂也支持这个解释。
但我**没有**逐行比对旧基线与旧卡片页实现，所以这条记为推断。

#### 数字

| 项 | 值 |
| --- | --- |
| 全量工作区测试（修前） | 445 passed / **1 failed** / 1 ignored（失败项即 #1） |
| clippy `mw-shell` bin | 4 → **3**（回到 `export_pdf` 的 3 条历史告警基线） |
| 定向测试 | `retarget_link_to_new_concept_undoes_in_one_step`、`shell_sources_do_not_hardcode_font_sizes` 各 1 passed |
| `cm-card-link` 基线 | `C991545A4DA4`，连跑两遍 0 差异 |
| 修后门禁 | `scripts/x100-stage-cm-gate.ps1` → **MACHINE GATE PASS**（16 组比较 ×2 遍、全量测试、clippy 全绿；`finished=2026-10-27 01:33:11`） |

> 这轮没加任何功能。但 5 条里有 3 条是我前几轮自己写进去的——
> 只跑 build + 定向测试是看不见它们的，必须跑门禁。

### 2.8 第八轮：换成外壳回归，CI 在根场景红（2026-10-27 01:5x）

上一轮只扫了 `cm-*`。这轮跑 `scripts/ci.ps1`（fmt → clippy → 全量测试 → **全部** golden），
在 golden 步红了，红在根目录 `shots/golden` 的 `shell-ppp1.00`：

| 指标 | 值 |
| --- | --- |
| 差异像素 | 111 / 1749600（**0.006%**） |
| 最大通道差 | **13** |
| 包围盒 | x746..830 y495..532（84×37，画面正中） |

#### 判「不是抖动」

连跑两次取 hash：`a1=64EA1C60C621 a2=64EA1C60C621`（相同），`base=4BA64BDEBE2C`（不同）。
两次 actual 相同就排除了「暖机不足/动画未收敛」，只剩「渲染真的变了」或「基线过期」两种可能。

#### 判「是基线过期」而非回归

两条互相独立的证据：

1. **差异形状**：111 个差像素里 71 个落在包围盒两侧 14px 内，跨 24/38 行——沿**左右竖边**分布，是盒子边沿的亚像素位移；不是整块换色，也不是几何移动。
   颜色配对全是近灰微移（`8D9490→959B98`、`C5C7C5→CDCFCB`、`69726E→707975`，每对只出现 1–2 次），即抗锯齿边缘，无色相变化。
2. **基线年龄**：`git log -1 -- shots/golden/shell-ppp1.00.png` → **2026-10-21 00:37**，
   比整场 B2 还早 6 天；CM-09 明确改过节点圆角令牌与关系线宽，`cm-*` 12 张当时重拍了，根场景漏了。

#### 处置

按 `docs/29` §5 量化后重拍根目录三档 ppp，并各复跑两遍 `--tol 0`：

| 文件 | 新基线 hash |
| --- | --- |
| `shell-ppp1.00.png` | `64EA1C60C621` |
| `shell-ppp1.50.png` | `7823AFEC0F6F` |
| `shell-ppp2.00.png` | `E75C13BB604D` |
| `canvas-ppp1.00.png` | `4B3A1F5109AA` |
| `canvas-text-ppp1.00.png` | `ED37BDE4CE9C` |

> **诚实边界**：原因我只做到「与 CM-09 的令牌改动最吻合」，**没有**对 6 天的改动做逐提交二分。
> 支撑它的是形状（竖边亚像素）+ 年龄（6 天前）+ 当时确实有意改过圆角/线宽三件事，不是直接证据。

#### 这条教训

`scripts/x100-stage-cm-gate.ps1` 只覆盖 `cm-*` 16 个场景，**根目录与外壳场景不在里面**。
所以「cm 门 PASS」不能当作「外观没回归」的证据——后者只有 `ci.ps1` 能给出。

### 2.9 第九轮：全量 golden 基线已经烂了很久（2026-10-27 02:0x）

修完根目录后，`ci.ps1` 又红在 `theme-ink`；修完主题矩阵，又红在 `selected`。**这不是偶发，是一串**。
一次次等 CI 跑到下一处太慢（每轮约 5 分钟只暴露一处），改成两步走。

#### 第一步：给 `ci.ps1` 加 `-Update`

25 处 `cargo golden` 调用统一接上 `@upd` 参数数组（`@()` / `@('--update')`），
这样重拍走的是 **CI 自己的环境表**（每个场景的隔离 `APPDATA`、`MW_THEME`、`MW_OUTLINE`、窗口尺寸都照原样），
而不是手写命令——手拍最容易被漏掉的就是这些环境量。

    powershell -File scripts/ci.ps1 -Update -SkipClippy   # 按 CI 口径重拍全部基线
    powershell -File scripts/ci.ps1                      # 再验证一遍

#### 第二步：逐个量化漂移——结果发现基线根本不在 git 里

我本想用 `git show HEAD:<基线>` 取出旧图，逐像素比出新旧差异表。写完脚本跑出来**全部是 `NEW`**，
一查：`git cat-file -s HEAD:shots/golden/selected/shell-ppp1.50.png` 报不到路径、
`git ls-files shots/golden/selected/` 是空的——**这些基线从来没进过版本库**。
精确口径（第二轮核实后改写）：`git check-ignore` 退出码 1 = **没有被忽略**，`git ls-files shots/golden` 只有 **14 个文件**——都是根目录那几张（`shell-ppp1.00/1.50/2.00`、`canvas-ppp1.00`、`canvas-text-ppp1.00`）和几个 `actual/`，**109 个子目录场景基线一个都没入库**（不是被忽略，是从没 `git add`）。

这解释了为什么陈旧基线能一直烂着：**它们不出现在 diff 里，评审看不见，只有跑 `ci.ps1` 才会撞上**。

#### 实测到的漂移（只有这三处能测）

| 场景 | 像素 | 占比 | 最大差 | 包围盒 |
| --- | --- | --- | --- | --- |
| `shots/golden` shell @1.0 | 111 | 0.006% | 13 | 84×37 @ 画面正中 |
| `theme-ink` shell @1.5 | 161 | 0.009% | 19 | 126×56 @ 画面正中 |
| `selected` shell @1.5 | 156 | 0.009% | 20 | 126×57 @ 画面正中 |

三处形态一致：都在画面正中一个节点框上，都在 0.01% 量级，颜色配对都是近灰微移或整列左右互换（亚像素），无色相变化。

#### 诚实的边界

**其余场景的漂移量我测不到**：基线不在 git 里，`--update` 又把旧图覆盖了，而 CI 每次只报第一处失败。
所以除上表三处之外，**重拍是「盲拍」的**——依据只有：① 三处样本形态一致且都在 0.01% 量级；
② 这些基线全部早于整场 B2/CM 设计改动；③ 重拍后 `ci.ps1` 的验证是逐场景 0 差异。
另外，对那 14 个入库文件我用 `git show HEAD:<path>` 取旧图比了一次，数字**不能当本轮漂移读**：`shell-ppp1.00` 差 364115 px（20.8%，max 214）、`canvas-ppp1.00` 差 2076 px（max 90）——那是**自 2026-10-21 起**的累计（工作区基线在后续轮次重拍过但从未提交，HEAD 还是老那张），而 CI 报的 111 px 是「上一次重拍到现在」。两个量纲不同，别混。如果要更硬的证据，应该在**动手修之前**先跑一遍「全目录对比 + 记录」的脚本，而不是让 CI 一处一处地报。
这条写进下一轮的第一件事。

#### 一条建议（留给用户拍板）

基线不在版本库 → 谁改坏了外观都没有历史可比、评审也看不见。
要么把 `shots/golden/**/*.png` 纳入 git（体积会涨），
要么在 `ci.ps1` 里加一步「把当前基线打包成 `target/golden-baseline-<时间>.zip`」留档。我没有擅自改仓库策略。

### 2.10 第十轮：⑥ 数据层落地 + clippy 门本身是坏的（2026-10-27 02:4x）

#### ⑥ 聚焦问题：只动 `mw-core`，先不碰绘制

`docs/32` §4 已把三条待拍板事项落成决定（用户给了全权判断的授权），本轮按决定执行第一步：

- `model.rs`：`LayoutSettings.focus_question: Option<String>` + `#[serde(skip_serializing_if = "Option::is_none")]`；
- `command.rs`：`Command::SetFocusQuestion { text: Option<String> }`，撤销文案「编辑聚焦问题」。
  没有复用现成的 `SetLayout`——它整块替换 `LayoutSettings`，撤销会说「更换布局」，
  用户按 Ctrl+Z 时以为整张图的排版被动了；
- `document.rs`：apply 分支把空串/纯空白归一成 `None`，磁盘上不允许出现「有键但没内容」；
- 三条测试（全绿）：
  `focus_question_is_normalised_and_undoable`（去空白 / undo / redo / 空白=清空）、
  `the_focus_question_is_written_only_when_set`（没设置不写键，`mw-format`），
  `an_older_file_without_the_focus_question_still_opens`（缺键读成 `None`，`mw-format`）。

兼容性不是口头保证：`an_older_file_...` 那条走的就是老文件的反序列化路径
（`{"kind":"conceptMap","direction":"auto"}` 没有 `focusQuestion` → `None`），所以不升 `FORMAT_VERSION`、不需要迁移。

#### clippy 门报「38 > 37」，但新增的不是我的代码

CI 在 clippy 步骤红了一次。逐条查下来：这轮改动的四个文件里没有任何告警
（`document.rs` 只有 1268 行的历史 `unneeded late initialization`）；那两条最像新告警的
（`this operation has no effect`、`casting to the same type is unnecessary`）
分别在 `mw-golden/src/lib.rs:174` 与 `mw-probe/src/bin/probe_egui.rs:300`（当时行号），我都没碰过。

真正的结论是**门本身不可复现**。同一份工作树，历史日志里报过 33、37、38 三个数字
（`caretaker-ci.log` / `ci-r18*.log` / `ci-r19.log`）。两个原因：

1. 同一条告警会在 `lib` 与 `lib test` 两个 target 各打印一次 → 计数被放大；
2. cargo 对没有重新检查的 crate **不重印**告警 → 缓存越热，报得越少。

验证办法是逼它全量重检：把所有 `.rs` 时间戳刷新后连跑两次
（`target/x100/r19-clippy-det.log`），两次都是 **38 条原始行 / 37 条唯一指纹**——
即 38 是真实值，而「37」这个数字恰好等于去重后的条数，之前一直被当成上限。

#### 处置：把数字上限换成指纹集合

`ci.ps1` 的 clippy 步骤改成比对 `scripts/clippy-baseline.txt`（一条告警一行「标题 @ 文件:行」，37 行）。
缓存只会少报、不会凭空多报，所以「出现基线里没有的指纹」才是可信的新增信号；
少报的那些打一行提示、不失败。`-MaxClippyWarnings` 参数删除，
新增合理告警的正确动作变成 `-WriteClippyBaseline` 重写基线文件（把改动留在 diff 里，而不是改一个魔数）。

#### 本轮数字（都有日志可查）

| 项 | 结果 | 出处 |
| --- | --- | --- |
| `cargo test --workspace -- --test-threads=1` | 835 passed / 0 failed（23 个 target） | `target/x100/r19-test.log` |
| `cargo check --workspace --all-targets` | 0 error | `target/x100/r19-check.log` |
| `cargo fmt --all -- --check` | 干净 | `target/x100/r19-fmt.log` |
| `ci.ps1`（含新 clippy 门） | 29 步、0 ✗、0 FAILED、EXIT=0 | `target/x100/ci-r19b.log` |

#### 同源的第二处：注解选中环也没画出来

顺手把同一个文件里所有 `ui.painter()` / `ui.painter_at()` 调用按「在贴图行之前还是之后」过了一遍，
发现还有一处：注释选中环（`AnnotationKind::Boundary` / `Summary` 各画一圈 2px 强调色）也画在贴图之前。
同样改成「先收集 `Vec<egui::Rect>`、贴图后统一画」——这条环的作用是「多选时能看出删的是哪几条」，
看不见等于多选没法确认。

现在这个文件里贴图之前只剩 `ui.painter_at(rect)` 那一行本身（就是贴图），其它绘制全部在它之后。
#### 本轮证据

| 项 | 结果 | 日志 |
| --- | --- | --- |
| 阶段门（37 场 / 74 次比较） | **MACHINE GATE PASS**，`tests passed=864 failed=0` | `target/x100/cm-gate-r50f.log`（修复后首轮为 `cm-gate-r50d.log`，那时守卫测试还没加，是 863） |
| 全量 CI | **CI OK**，864 passed / 0 failed，clippy 当前 37 条 / 基线 37 条 / 新增 **0** | `target/x100/ci-r50.log` |
| `cargo fmt --check` | 干净 | 同 CI |
| 次序守卫 | 先证明会红（`self.paint_relink_preview(ui, viewport) 画在贴图之前…`），恢复后转绿 | `cargo test -p mw-shell --bin mw-shell overlays_are_painted_after` |
| 三张对照哈希 | `cm-default` `954F0F34E5ED`（不变）、`cm-relink-select-only` `2FFFA4F97767`、`cm-relink-drag` `0F259E48AD50` | 目录内 PNG |
#### 一个差点误判的假警报：还原源码后 cargo 认为「无需重编」

取那 5 个漂移数字的做法是「临时把四个调用挪回贴图之前 → 重编 → 只跑这 8 场」。量完我用 `copyFileSync`
把备份的修复版拷回去，`cargo build` 打印 `Finished in 0.30s`——**它没重编**。原因是拷贝把源文件 `mtime`
设成了**早于上次构建**的时刻，cargo 按时间戳判定「源比产物旧 ⇒ 新鲜」。于是那一次复核跑的仍是回退版二进制，
8 场全部报 ✗，数字与刚才量的**逐字相同**。

是一条假警报，也是一条真教训：**还原源码后要 `touch` 再编，或者对「Finished in 0.x s」保持怀疑**。
`touch` 之后重编（3m35），同一批 8 场全部 ✓。

留档：`target/x100/run8.sh`（8 场的跑法，env 从门脚本解析出来）、`drift8.log`（回退版实测）、
`verify8.log`（陈旧二进制那次，8 个 ✗）、`verify8b.log`（真重编之后，8 个 ✓）、`baselines-r50/`（8 张基线备份）。
#### 未做

画布顶部的聚焦问题带、检查器里的输入框、PNG/SVG/PDF 导出取同一字段、`cm-focus-question` golden——
一条都没做，所以 ⑥ 状态是 `进行中`，不是 `已实现`。⑦ 箭头样式按 §4 的决定继续延后。

> 这段记的是**第 20 轮当时**的账。⑥ 已在第 22–24 轮补齐（§2.13–§2.17），⑦ 已在第 21 轮落地（§2.12），本文首部的状态行已全部改成 `已实现`。

### 2.11 第 20 轮：⑥ 从「只有字段」做到「看得见、改得动」（2026-10-27 02:0x–03:1x）

#### 问题

第 19 轮把 ⑥ 的数据层做完了，但用户打开一张概念图，**什么都不会变**——`focus_question` 写进去了，
画布上没有它，检查器里也没有它。一个只有字段的功能等于没做。

#### 决定：带子只在「设了问题」时出现

整条路都在 `focus_question.is_some()` 后面。代价是「没设问题的人看不到入口」，
收益是 **14 张既有 `cm-*` 基线一个像素都不用重拍**——否则每张基线都要重审一遍，
而重审 14 张图的收益是零（它们本来就对）。入口的问题用检查器解决（见下）。

#### 改了什么

| 位置 | 内容 |
| --- | --- |
| `mw-core`（第 19 轮） | `LayoutSettings.focus_question` + `Command::SetFocusQuestion` + 归一化/撤销/兼容 3 条测试 |
| `canvas.rs` | `focus_band_rect(origin, text_size)` 纯几何；`FocusEdit { text, opening, focus, rect, ime }` 编辑态 |
| `canvas/canvas_methods_3.rs` | `paint_focus_question()` / `start_focus_edit()` / `focus_editor_ui()` / `set_focus_question_for_demo()` |
| `canvas/canvas_methods_2.rs` | 在 `paint_card_badges` 前画带子、在 `rel_editor_ui` 前跑编辑框 |
| `panels/inspector/inspector_methods_2.rs` | `doc_body` 增「聚焦问题」一栏（`doc.is_concept_map()` 才出现），失焦即落盘 |
| `app_methods_1.rs` | `MW_CONCEPT_FOCUS=1` 场景钩子 |
| `scripts/x100-stage-cm-gate.ps1` | 第 15 张场景 `cm-focus-question`（`ppp 1.0`、`warmup 6`） |

交互细节：`dim` 色常驻、悬停才转 `text` 色并补一条 `accent` 底线（避免顶上一行字看起来像标题或说明文字）；
双击进编辑；Enter 提交、Esc 取消、点走也提交；**IME 组合期间 Enter 归选字**（`ed.ime` 守卫）；
文本没变就不发命令（不发空撤销记录）。

#### 证据

| 项 | 结果 | 出处 |
| --- | --- | --- |
| 新基线 `cm-focus-question` | 与 `cm-default` 逐像素只差 `rows 68–83`、1840 px / 0.0799%、x 848–1063 | `target/x100/r20-focus4.log` |
| 双跑 `--tol 0` | 第二遍 0 差异（`VERIFY_EXIT=0`） | 同上 |
| CM 门（15 场景 × 2 遍） | `PASS concept golden pass 2 (17 comparisons)`、0 FAIL、`MACHINE GATE PASS` | `target/x100/cm-gate-r20.log` |
| 单测 | `focus_band_sits_centred_at_the_top_of_the_canvas`、`focus_band_never_spills_out_of_a_narrow_canvas` 通过 | `cargo test -p mw-shell focus_band` |
| clippy | 指纹 37/37、新增 0、缺失 0 | `target/x100/r20-clippy-raw.log` |

#### 这轮被单测抓到的两个真错

1. **带子整体高了 7px**：中心点算成 `top + 10 + 行高/2`，忘了上下还各有 7px 内边距，
   实际 `top` 落在 `画布顶 + 3`。单测 `贴顶 10px` 直接失败——如果没有这条断言，
   基线会把「错的位置」锁成「对的位置」，以后谁都发现不了。修完重拍。
2. **夹具把文档弄脏了**：夹具原本走 `apply_command`，于是标题栏多出一个未保存标记、状态栏也跟着变，
   `cm-focus-question` 与 `cm-default` 的差异从「一条带子」变成「三块」。改成直接写 `meta` 后
   差异收敛到 `rows 68–83`——命令路径本身由单测覆盖，截图不该顺带锁住脏标记。

#### 未做（明确的，不是「大致完成」）

- **导出 PNG/PDF/SVG 不带聚焦问题**。三种导出的唯一真相源是 `Scene`（`AGENTS.md` §6 不变量），
  而问题带按 `docs/32` §2 的决定不进 `Scene`。要让导出带上，就得在导出管线里另排一行文字：
  那是第二处排版，不是「同一字段读两遍」。要么接受「导出没有聚焦问题」（CmapTools 是带上的），
  要么给导出加页头排版路径——单独立项，本轮不做。
- **大纲页头不显示聚焦问题**（CM-07 页头现在只有标题）。
- **没有跑真实鼠标脚本**：`cm-focus-question` 是纯静态场景，双击进编辑、Esc 取消、
  IME 期间 Enter 归选字这三条只有代码路径和 `rel_editor_ui` 的同款实现作为依据，没有脚本证据。

### 2.12 第 21 轮：⑦ 关系线方向从「延后」变成「四档可选」

#### 问题：提案里有一句假话

`docs/32` §3 写的是「默认 `None` = 现状（不画箭头）」，`docs/30` §12 第 7 行的差距栏写的是
「需要扩字段」，两处都把「今天的关系线没有箭头」当成了事实。**实测不是**：

- `mw-core/src/scene.rs` 的 `straight_between()` / `curve_between()` 一直在算一个 6×4 的
  实心三角形，存在 `SceneRelationship.arrow` 里；
- `mw-render/src/instances.rs` 无条件把它推进 `TriInst`，`export_svg.rs` / `export_pdf.rs`
  也无条件输出；
- 也就是说概念图和思维导图的关系线**今天就有前向箭头**。

更难看的是：这个事实我们自己的文档里一直写着——`docs/31` 第 123 行描述 CM-03 就是「直线 + 6×4 箭头」，
第 183 行还专门写「复用既有箭头几何（`Design` 里已有 `edge_arrow`），不新建第二套」。
提案当时没有回读这两行。

这个假话的代价很具体：如果按提案把默认值设成「不画箭头」，17 张既有 `cm-*` 基线会一起掉箭头，
而那不是「改了个默认值」，是「把已有的视觉回退了」。

#### 决定

| 事项 | 决定 | 理由 |
| --- | --- | --- |
| 默认值 | `Forward` | 它等于今天的样子，于是零基线漂移；这正是「没回归」的抓手 |
| 四档命名 | `Off` / `Forward` / `Backward` / `Both` | 不叫 `None`：`Off` 是「用户明确选了不画」，没设置是另一回事 |
| 落盘 | 默认值不写（`skip_serializing_if = "ArrowHead::is_forward"`） | 否则每个文件的 diff 都多一行没意义的键 |
| 几何 | 只在 `Scene` 算一次，两个三角形 | wgpu / egui / SVG / PDF 四处若各算一遍，必然不一致 |
| 起点端箭头 | 由 `mirror_tri()` 围绕连线中点镜像 | 重算会因两端切线不同画出两个不一样大的箭头 |

#### 改了什么

| 层 | 文件 | 改动 |
| --- | --- | --- |
| 模型 | `mw-core/src/model.rs` | `ArrowHead`（`Default = Forward`）+ `Relationship.head` |
| 命令 | `mw-core/src/command.rs` | `SetRelationshipHead { from, id, head }`，撤销文案「修改关系线方向」 |
| 应用 | `mw-core/src/document.rs` | apply 分支（照 `SetRelationshipColor` 的形），只在该变时才写 `modified_at` |
| 几何 | `mw-core/src/scene.rs` | `arrow` 改 `Option`、新增 `arrow_back`；`mirror_tri()`、`lerp_tri()`；`bow_crossings` 重钉两端 |
| 渲染 | `mw-render/src/instances.rs` | 两个三角形都推；`relationship_bounds` 两个都算 |
| 命中 | `mw-render/src/hit.rs` | 两个三角形分别入命中表 |
| 导出 | `export.rs` / `export_svg.rs` / `export_pdf.rs` | 逐三角形输出（包围盒也含起点端） |
| 入口 | `inspector_methods_1.rs` | 「关系线」块新增「箭头」四格 |
| 入口 | `context.rs` + `app_methods_2.rs` | 关系线右键菜单「箭头方向」子菜单（提案里写的就是这个入口） |
| 夹具 | `canvas_methods_2/3.rs`、`app_methods_1.rs` | `set_selected_relationship_head()`、`set_arrow_head_for_demo()`、`MW_CONCEPT_ARROW=off|backward|both` |
| 门 | `scripts/x100-stage-cm-gate.ps1` | 增 `cm-arrow-off` / `cm-arrow-backward` / `cm-arrow-both`（17 → 20 场） |

#### 证据

| 证据 | 结果 |
| --- | --- |
| 单测 `the_arrow_head_can_be_changed_and_undone` | 默认 `Forward`、改 `Both`、撤销回 `Forward`、重做回 `Both` |
| 单测 `the_arrow_head_is_written_only_when_it_is_not_the_default` | 默认不出 `head` 键；改成 `both` 后写出 `"head":"both"` |
| 单测 `an_older_relationship_without_the_arrow_head_still_opens` | 缺键读成 `Forward`；写了 `off` 读得回来 |
| 基线 `cm-arrow-off` / `-backward` / `-both` | `--tol 0` 各跑两遍，1920×1200，差异 0 处 |
| `cm-arrow-off` vs `cm-default` | 82 px（0.0036%）全落在 5 个连通域，4×7 … 8×6 px——5 个终点箭头消失 |
| `cm-arrow-both` vs `cm-default` | 81 px（0.0035%）5 个连通域——5 个起点箭头出现 |
| `cm-arrow-backward` vs `cm-default` | 163 px（0.0071%）12 个连通域 = `off` 的 5 个 + `both` 的 5 个 + 2 个 1px 抗锯齿碎点 |
| 17 张既有 `cm-*` 基线 | 全量门重跑，0 差异（默认 `Forward` 没改动任何像素）；20 场 × 2 遍 = 40 次比较全 PASS |

`backward = off ∪ both` 这个等式是这轮最有用的一条自检：三张图来自三个不同的环境变量，
却精确满足 82 + 81 = 163，说明「换方向」= 「拿掉终点箭头」+「加上起点箭头」，没有第三种改动。

| 全量 `scripts/ci.ps1` | 29 步全绿（`clippy` 指纹 37/37、新增 0；全量 golden 阶梯 0 差异；`cargo test` 全过） |
**口径更正**：第 20 轮记的是「14 张既有 `cm-*` 基线 / 新增第 15 张」，但脚本当时实际已有 17 行场景。
数字以 `scripts/x100-stage-cm-gate.ps1` 的行数与门日志的比较次数为准：本轮前 **17 场**，本轮后 **20 场**，
pass 2 报 `20 comparisons`，40 次比较（20 场 × 2 遍）全 PASS。

#### 这轮被测试抓到的一个真错

`the_arrow_head_is_written_only_when_it_is_not_the_default` 第一遍挂在
`json.contains("\"head\": \"both\"")`——我按 pretty-print 的格式写了断言，实际 `file_json()`
是紧凑的（`"head":"both"`）。错的是断言不是代码，但它提醒一件事：**断言序列化字符串时，
断言的是那一行字节，不是我以为的那一行字节**。

另外有一处差点写错的：`SceneRelationship` 不是 `Serialize` 类型（`Scene` 从不落盘），
我在起草时给它加了 `#[serde(...)]`，那会直接编译失败——编译器替我拦住了「把几何当成数据」的念头。

#### 未做

- **画布上没加浮条**：改方向只能从检查器和右键菜单进（两处都试过，够用）；
  没有做「选中连线时在线上浮一排按钮」。
- **没有脚本证据**：`cm-arrow-*` 三张是纯静态场景，右键子菜单和检查器四格的点击路径只有代码与单测依据。
- **思维导图的关系线也吃这个设置**：`Backward` 在弧线上的起点箭头是镜像近似（弧两端切线不同），
  概念图的直线是精确的。这条差异没有单独做测试断言，只在 `mirror_tri()` 的注释里写了。

### 2.13 第 22 轮：⑥ 的导出补齐——PNG / PDF / SVG 都带上聚焦问题

第 20 轮把问题带画在 egui 层，`Scene` 里没有它，于是三种导出**都看不见聚焦问题**；CmapTools 的导出是带的。

#### 先否掉一个"显然"的解法

「把 egui 那条带子搬进 `Scene`，再让画布也从 `Scene` 画」——**错**。画布上的带子贴视口顶（`focus_band_rect`），
滚动画布时它不走，这正是 CmapTools 的手感；导出没有视口，只锚得住内容包围盒的顶。两边几何本来就不一样，
共用的只是**排版**。所以正解是：给 `Scene` 加一个**只有导出才填**的字段。

#### 三点落法

1. `Scene::focus_note: Option<SceneNote>`（`rect` / `text` / `font_size` / `color`）+ `Scene::attach_focus_note()`：
   带子放在 `bounds` **上方**并把 `bounds` 向上扩，既有几何一格不动，`content_bounds()` 那本账也自然带上。
2. 文字走 `mw_render::text::collect_jobs()`——PNG / PDF / SVG 三种导出**本来就共用**这一份 `text_jobs()`，
   所以只加了一个 `TextJob`，三条导出各自一个字都不用改（PDF 的矢量字形走同一份排版）。
3. `export::attach_focus_note(scene, doc, theme)` 是唯一入口：画布的 `export_scene()` 和 CLI 的 `--export-pdf` 都过它。
   CLI 那条原本自己 `build_scene`，不接上就会留一个「画布导出有、命令行导出没有」的缺口。

带子高 = `round(字号 × design 行高倍数 + 2 × 7)`：7 跟画布那条带子的 `pad.y` 对齐；**取整**是因为带小数会让整幅内容
落到半个像素上，导出图与「没带子」那张就不再逐像素同源，也就没法用「位移 N 行后完全相等」来证明只多了顶上一条。

#### 证据

| 项 | 数字 |
| --- | --- |
| 单测 | `cargo test -p mw-shell --bin mw-shell the_focus -- --test-threads=1` → 3 passed / 449 filtered |
| 单测锁的判词 | bounds 只向上长、既有节点矩形**逐格不动**、导出 SVG 里**能搜到**问题文字 |
| 单测锁的判词（PDF） | 页数不变、字节变多（`+500` 门槛，实测 +14918） |
| 实拍 PNG（2×） | 同一份概念图 1386×860 → **1386×934**，高 +74 设备像素 = 37 世界单位 |
| 纯位移验证 | 切掉顶上 74 行后与「不带问题」那张比：**差异(>4/255) 0 px，最大差 1 LSB** |
| 带子里的墨 | 5273 px，横向 x∈[469,899]；图中轴 693、墨迹中轴 684 |
| 实拍 PDF | 1 页 → 1 页，77619 → **92537** 字节 |

PNG 的实拍路径：`MW_OPEN=<file.zmind> MW_EXPORT_BENCH=<out.png>`（走的是与 `Ctrl+E` 完全相同的导出管线）。

#### 诚实边界

- 样本是 `tests/fixtures/x100/tiny-cn.zmind` 把 `layout.kind` 改成 `conceptMap` 再写入 `layout.focusQuestion`（内容没换，只换类型）——
  仓里没有一份现成的概念图 `.zmind`，而 `MW_CONCEPT_DEMO` 那条模板钩子不落盘。所以「导出带子」的像素证据是在这份改造样本上拍的。
- PDF 里没有可搜文字（全是字形轮廓），所以 PDF 侧只锁「页数不变 + 内容变多」；「画的是不是那几个字」由**同一条流水线**的 SVG 文字断言保证。
- 带宽 37 世界单位 vs 画布那条实测 ≈36（galley 高 + 14）：不是同一个量（DirectWrite 行高 vs egui galley），只保证观感一致，没做逐像素对齐。
- 没新增 golden 基线：画布不填 `focus_note`，所有 `cm-*` 基线应零漂移——本轮跑过全量门复核（见 §2.14）。

#### 顺手修的两处旧账

- `canvas_methods_1.rs::export_scene` 的文档注释**重复了两遍**（第 20 轮留下的复制粘贴），删掉一份。
- `canvas.rs` 里 `FocusEdit` 的注释写着「不参与几何与导出」，导出补上后这句已经不成立，改成说明两条路的分工。

### 2.14 第 22 轮的门：全量 CI + 概念图门复核

| 门 | 结果 |
| --- | --- |
| `scripts/ci.ps1`（29 步） | 全绿，末行 `CI OK` |
| `cargo test --workspace -- --test-threads=1` | 451 passed（bin）+ 38 passed（其它 target），0 failed |
| clippy（指纹基线） | 当前 37 条 / 基线 37 条 / **新增 0** |
| CM 门 `scripts/x100-stage-cm-gate.ps1` | 20 场 × 2 遍 = **40 次比较全 PASS** |

clippy 这次先报过「新增 2」，逐条核对是**行号位移**：`scene.rs:1492 → 1533`、`scene.rs:841 → 881`
（新增的 `SceneNote` 与 `attach_focus_note` 把它们往后推了 41/40 行），总数仍是 37。
指纹里带行号，这类位移每次加代码都会出现——`-WriteClippyBaseline` 重写后再跑全量，新增归 0。

**关键结论**：20 张 `cm-*` 基线**逐张零漂移**。这正是「画布永远不填 `focus_note`」这个前提的可验证形式——
如果哪天有人把带子搬进画布，这一行数字会立刻变红。

### 2.15 第 23 轮：⑥ 的最后一块——大纲页也显示聚焦问题

起点：⑥ 只剩「大纲页头未做」。这一轮把它补完，⑥ 的三个落点（画布 / 导出 / 大纲页）全部齐了。

**取舍**：大纲页页头是一行 40px 控制台（返回 / 面包屑 / 计数 / 搜索 / 更多），
已经挂着「· 概念图 · 按链接层级展开」并且**按可用宽度省略**。问题塞进同一行，长问题会被省略号吃掉——
而恰恰是长问题才最需要被看见。所以改成**页头下沿一条独立带子**：

| 方案 | 结果 |
| --- | --- |
| 塞进页头那一行 | 与副标题抢宽度，长问题被 elide |
| **页头下沿一条带子**（选这个） | 完整显示；在 `outline.show` 的 `ScrollArea` **之外**，读的时候一直在，不随正文滚走 |

**实现**：`content_kit::question_band(page, margin, line_h)` 是纯几何（带高 = 行高 + 上下各 `QUESTION_BAND_PAD`=7，
宽 = 页宽 − 2×`column_margin`，与正文列同一口径）；`app_methods_2.rs::outline_screen` 按列宽**换行**排版，不用省略号。
7 这个数与画布那条的 `pad.y`、导出那条的 `FOCUS_NOTE_PAD` 是同一个数。

**实测**（`MW_TRACE` 临时打印，量完已删）：页头底 `88` → 带子 `96→130`（高 34 = 字高 20 + 7×2）→ 正文行起点 `138`。
96 与 130 外面各有 8px，**不是我加的**，是 egui 的 `item_spacing`：带内 7/7、带外各 8，视觉 15/15。

第一版在带子底下画了 1px 分隔线，**撤掉**了：上下已有 8px 间距，再加一条线就成了「双线」，反而像第二个页头。

**证据**（`shots/golden/cm-outline-focus`，1920×1200，`--tol 0` 连跑两遍 0 差异）：

- 与 `cm-outline` 逐像素比：新增墨迹只落在 **106–120** 一行（x 849–1062，中心 955.5，图宽中心 960，差 4.5px 是汉字侧边距），
  正文整体下移 **42px**，之后各段一一对应（152/204/243/283/324 ↔ 110/162/201/241/282）；
- 长问题（60 字，超列宽 800）：带子变两行（106–120、126–139），正文下移 **62px**；
  带子横向 **561–1358 = 797 ≈ 列宽 800**，中心 959.5 ⇒ 换行与居中都对；
- 单测 `content_kit::tests::question_band_sits_in_the_column_and_keeps_seven_above_and_below`（含窄窗收零宽）。

**钩子**：`MW_CONCEPT_FOCUS_TEXT=<文字>` 可临时换问题（只为验证换行，不进任何基线）。
另有一处顺序陷阱：`MW_CONCEPT_OUTLINE` 与 `MW_CONCEPT_FOCUS` 同时给时，大纲钩子会**再换一次 session**，
把先写进去的问题冲掉，所以问题要在它里面补写一次——`cm-outline-focus` 就是靠这条才拍出来的。

**漂移**：既有 20 张 `cm-*` **零漂移**（四张 `cm-outline*` 都没设问题）；新增第 **21** 张。

**结论**：CM-10 表的第 1、1b、2、3、4、5、6、7、8 行全部 `已实现`，⑥ 不再有尾巴。

### 2.16 第 23 轮的门：全量 CI + 概念图门（21 场）

| 门 | 结果 |
| --- | --- |
| `scripts/ci.ps1`（29 步） | **`CI OK`** |
| `cargo fmt --all -- --check` | 干净（第一次跑门时它先挂了——见下方「格式化」那条） |
| clippy 指纹比对 | 当前 37 / 基线 37 / **新增 0** |
| `cargo test --workspace`（串行） | **844 passed / 0 failed** |
| 31 组 shell golden（`--tol 0`） | 全部 `无超容差差异` |
| `scripts/x100-stage-cm-gate.ps1` | **`MACHINE GATE PASS`**，21 场 × 2 遍 = **42 次比较**，`03:12:33 → 03:15:30` |

**格式化**：第一次跑门时 `cargo fmt --check` 先挂了——写代码时手写的换行不合 rustfmt 口径。
`cargo fmt --all` 之后再跑，`fmt` 步 PASS 且无残留 diff。教训很短：**门的第一步就是 fmt，先本地跑一次再排队等十分钟**。

**为什么 21 场而不是 20**：新增的 `cm-outline-focus` 是唯一锁「大纲页问题带」的基线；
其余 20 张逐张零漂移，四张 `cm-outline*` 更是完全没动（它们都不设 `MW_CONCEPT_FOCUS`）。

### 2.17 第 24 轮：大纲页那条问题带可以就地改

上一轮把大纲页的问题带做成只读，并在 §3 第 5 条记了欠账。这一轮还账。

| 做法 | 结果 |
| --- | --- |
| 只读（第 23 轮） | 少一个入口，但改问题要 Esc 回画布、再找一眼带子 |
| **双击就地改**（选这个） | 手势与画布同一条；键盘、提交、取消、撤销全部复用 |

**共用的是两样东西，不是三样**：控件 `content_kit::question_field()`（字号 / 内边距 16,7 /
占位提示 / 单行无边框）与判定 `question_edit_outcome(text, opening, esc, done)`
（Esc 优先、trim 后没改就不发命令、改了提交 trim 版本）。
几何**不**共用：画布那条带子贴视口顶、宽 240 由 `focus_band_rect` 算，大纲页这条贴页头下沿、宽 = 正文列 800。
这和 §7 里「能共用的只有文字排版，不是坐标」是同一条判断。

#### 实测到的坑：焦点在第二帧被抢走

第一版立起编辑框，截图能看见框，但 `MW_TRACE` 打出来的 `has_focus` 只有第一帧是 `true`：

```
event r24-focus has=true  txt="优质睡眠由哪些习惯共同决定？" lost=false
event r24-focus has=false txt="优质睡眠由哪些习惯共同决定？" lost=false
```

抢的人不在问题带里，而是 `outline.show` 返回之后那条「首次进大纲，把焦点给第一行」的请求
（`take_focus_request` → `request_focus(row_id)`，它有意排在 `ScrollArea` 收尾之后，
否则会反过来被滚动条抢走——同一段注释里写着这个理由）。
修法：编辑框立着时 `keys_blocked` 已经为真，这一帧的行焦点请求**直接让开**，并留一条
`outline_focus_request_skipped` trace。修完再打：`has=true` 一路到底，截图里文字末尾出现光标
（x 799–800、20px 高，紧跟文字右端 790）。

> 这不是离屏特有的：真实用户双击那行字，也一样会在下一帧失去光标，之后必须再点一次才能打字。

#### 数字

| 量 | 静态（`cm-outline-focus`） | 编辑态（`cm-outline-focus-edit`） |
| --- | --- | --- |
| 带子文字 x 范围（设备 px） | 849–1062（居中） | 576–790（列左沿 560 + 16 内距） |
| 光标 | 无 | 799–800，高 20 |
| 正文行起点（三段） | 152 / 204 / 243 | **152 / 204 / 243** |
| 两张图的差异 | — | 仅设备行 **105–121**（3788 px） |

`ui.put` 是绝对放置、不推进光标，所以编辑框立起来时机身一格不动——这是这轮唯一担心的回归点，量过了。

#### 门

| 门 | 结果 |
| --- | --- |
| 单测 `question_edit_outcome_says_cancel_continue_or_commit` | PASS（Esc 优先 / 未改不发命令 / trim 提交 / 清空提交空串） |
| `scripts/x100-stage-cm-gate.ps1` | **`MACHINE GATE PASS`**，22 场 × 2 遍 = **44 次比较**，`03:27:54 → 03:29:21` |
| 全量测试 | **845 passed / 0 failed**（844 + 本轮新单测） |
| clippy 指纹 | 新增 **0** |
| `cm-focus-question`（画布那条编辑框） | 零漂移 —— 共用改造没动像素 |

### 2.18 第 25 轮：画布那条带子遇到长问题会换行

第 24 轮给大纲页那条做了换行，这一轮补上画布这条——它一直是 `layout_no_wrap`。

**缺陷是量出来的，不是猜的**：把 `MW_CONCEPT_FOCUS_TEXT` 换成 137 字的问题，
画布带子在 1280×800 的画面里排成一行，ink 从 **x=1 铺到 x=1918**，正好贴住窗口两沿。
也就是说首尾都被硬切，而且**看不出被切**（没有省略号）——用户只会以为句子就这么长。

| | 改前 | 改后 |
| --- | --- | --- |
| 行数 | 1 | **2** |
| ink 横向范围 | x **1..1918**（贴两沿） | x **33..1885** |
| 第一行 / 第二行宽 | 1917 / — | 1853 / 326 |
| 带子高度 | 34（一行） | 两行自然长高 |

改法三小步（`docs/32` §9）：`focus_wrap_width(origin) = (origin.width() - 64).max(120.0)`，
`layout_no_wrap` → `layout(.., wrap)`，其余不动——`focus_band_rect` 本来就按 `text.y + 14` 取高，
行数一多自己就长高，不需要第二处修改。

**零漂移**：18 字的 `cm-focus-question` 在两种写法下 galley 宽度相同，第 22 张基线逐张不变（门里 46 次比较为证）。

#### 门

| 门 | 结果 |
| --- | --- |
| 单测 `focus_wrap_width_leaves_sixteen_of_padding_on_both_sides` | PASS（1280 → 1216；120 宽窗 → 120） |
| 单测 `focus_band_grows_with_wrapped_lines_and_stays_inside_the_canvas` | PASS（两行 40 高 → 54；宽 768；居中且在画布内） |
| `scripts/x100-stage-cm-gate.ps1` | **`MACHINE GATE PASS`**，23 场 × 2 遍 = **46 次比较**，`03:35:04 → 03:36:19` |
| 全量测试 | **847 passed / 0 failed**（845 + 本轮 2 个新单测） |

### 2.19 第 26 轮：⑦ 箭头四档，菜单里当前那档打勾

⑦ 的四档是**互斥**的，但右键菜单里那四行长得一模一样。点了「反向 ←」之后只有画布上的箭头端点变了，
再打开菜单看不出现在是哪一档——互斥选项组不给勾，用户每点一次都要重新确认一遍。

三层改动都很薄（`docs/32` §10）：

| 层 | 改动 |
| --- | --- |
| 菜单模型 | `Entry::action_checked(.., checked)` 带 `check: Some(bool)`；`Item::row_checked` 转发 |
| 绘制 | `paint_row` 里 `Some(true)` 画勾（强调色）、`Some(false)` 留空，**图标槽宽度不变** |
| 取值 | `Canvas::selected_relationship_head()` 从文档现读，不缓存（撤销之后缓存会骗人） |

新钩子 `MW_CONCEPT_ARROW_MENU=1`：选中示例图第一条关系线并展开「箭头方向」子菜单。

**实测**（设备 px，1920×1200；子菜单四行文字在 y = 177 / 207 / 237 / 267）：

| 当前档 | 勾的位置 | 勾的尺寸 |
| --- | --- | --- |
| 无箭头（`off`） | 第 1 行，y **177–186** | 10×12，42 px |
| 反向 ←（`backward`，新基线 `cm-arrow-menu`） | 第 3 行，y **238–245** | 10×12，42 px |

勾落在行左的图标槽（x 610–621）。四行文字左缘不因为勾而错位——图标槽宽度是恒定的。

**一个测量教训（写下来给下一轮）**：我先用两张不同档位的图做**差分**，看到两簇 42 px 的差异，
一度以为「勾没跟着动」。差分只说「这两块不同」，**不说哪块属于哪张图**；改成逐图直接检饱和色才对上，
勾一直在对的那一行。测颜色/位置别用差分归因。

#### 门

| 门 | 结果 |
| --- | --- |
| 单测 `menu::tests::checked_rows_carry_their_flag_and_plain_rows_stay_none` | PASS（`Some(true)` / `Some(false)` / `None` 三态） |
| 首次 `scripts/x100-stage-cm-gate.ps1` | **FAIL**：只有 `cargo fmt --check` 没过（手写的测试没走格式化），48 次 golden 比较全过 |
| `cargo fmt --all` 后重跑 | **`MACHINE GATE PASS`**，24 场 × 2 遍 = **48 次比较**，`03:45→03:48`，848 passed / 0 failed，clippy 新增 0 |

### 2.20 ⑦ 的激活路径：键盘与指针都能改文档（2026-10-27 04:45，17:30 更正）

§3 第 7 条一直记着「点这一行没有指针证据」。本轮想用 `MW_GOLDEN_INPUT` 把这一下点掉。

> **⚠ 本节原来的「负结果」是错的，已于 2026-10-27 17:30 更正。**
> 原判断：*指针点弹层行不派发，原因未定位*。真因是**三个坐标没一个落在可点的行上**——
> harness 一直是好的。错判保留了整整一天，教训写在下面。

**更正后的结论：指针点得动，而且和键盘落到同一个终态。**

用一次性探针（`MW_PROBE_ROW`，验完即删）把每一行的逻辑矩形打出来后，几何就摆在那了：

| 行 | 逻辑矩形 | 备注 |
| --- | --- | --- |
| 编辑关系线标签 | `x 367..595, y 127..157` | 根菜单第 1 行 |
| 箭头方向 | `x 367..595, y 166..196` | 根菜单第 2 行（子菜单入口） |
| 删除关系线 | `x 367..595, y 205..235` | 根菜单第 3 行 |
| 无箭头 | `x 600..788, y 167..197` | 子菜单 |
| 单向 → | `x 600..788, y 197..227` | 子菜单 |
| 反向 ← | `x 600..788, y 227..257` | 子菜单 |
| 双向 ↔ | `x 600..788, y 257..287` | 子菜单 |

再对照当年那三个点，全部落空，而且各自落空的方式不同：

| 旧坐标 | 实际落点 | 为什么没有 `menu_click` |
| --- | --- | --- |
| `(410,122)` | 卡片顶部 8px 内边距（第一行从 y=127 起） | 没压到任何行；探针显示 `any_click=true` 但七行 `hovered` 全 false |
| `(450,182)` | 「箭头方向」那一行 | 子菜单行点击**只负责展开**，本身不是动作（`menu.rs`:887 只置 `clicked`，不发命令） |
| `(500,160)` | y157..166 的分隔条缝隙 | 分隔条不响应 |

三张「指针终点不同却逐像素相同」的图也就说得通了：这三下**确实什么都没改**，图当然一样。
当时把「图没变」读成「事件没到」，是**用一张图去证明一个事件级结论**——图只能证明没变化，不能证明没派发。

**正：**把点挪到子菜单「双向 ↔」的中心 `(694,272)`，一次就通：

```text
golden_input 11 move [694.0 272.0]
golden_input 12 pointer [694.0 272.0] Primary pressed=true
golden_input 13 pointer [694.0 272.0] Primary pressed=false
menu_click label=双向 ↔
relationship head 01a17aae-…f3d6 01a17aae-…c51606 both
```

从 `MW_CONCEPT_ARROW=off` 起手，点完 document 变 `both`，渲染跟上。

**顺带结掉一个悬了一轮的几何悬案**：`place_submenu` 预测子菜单逻辑 x≈592，本轮实测 **600**；
之前记的「观测 ≈388」是误读（388 那个数是另一处的量法）。预测是对的，模型没坏。

**仍然成立的教训**：历史所有菜单场景都是「键盘开菜单 + 快捷键选行」，
从来没有一张基线点过菜单行——这条路径在 goldens 里一直是空白（`start-menu-input` 用 `key 1`、
`outline-menu-input` 用 `key Delete`）；现在已经补上，见 §2.24。

**正：键盘路径全链可验。** 同一夹具改用键盘三下 + `Enter`，trace 把三段都写了：

```text
menu_nav key=down label=单向 →
menu_nav key=down label=反向 ←
menu_nav key=down label=双向 ↔
menu_activate key=enter label=双向 ↔
relationship head 01a179ca-…d0ad 01a179ca-…7449 both
```

新基线 `cm-arrow-menu-key`（第 25 张）：从 `MW_CONCEPT_ARROW=off` 起手（画面本来一个箭头都没有），
键盘选「双向 ↔」并 `Enter` 之后，画布两端各长出一个箭头，菜单收起。

**这条基线锁什么**：不是锁像素，是锁「选一行 → 发命令 → 文档改 → 渲染跟上」这一整段。
**它不证明什么**：光看这张图分不清是键盘还是指针改的——所以指针路径另有 §2.24 一场，两场终态**逐像素相同**，正好互为交叉验证。

顺带一个可复用的教训：菜单的四种选法（加速键 / 指针 / 键盘导航 / 快捷键）不该只锁住其中一种；
本轮之前只锁了「开菜单」这一步，看起来像锁了交互，其实只锁了外观。

### 2.24 指针点菜单行上锁：`cm-arrow-menu-click`（2026-10-27 17:35）

§2.20 更正之后，「指针能派发」只是 trace 级结论。要让它进门口，得有一条**基线**：
脚本从 `MW_CONCEPT_ARROW=off` 起手（画面一个箭头都没有），展开「箭头方向」子菜单，
指针点「双向 ↔」，等 8 帧收尾。

| 项 | 值 |
| --- | --- |
| 平台 | `--ppp 1.0`、`1280×800`、`--warmup 24`、`--tol 0`；回读 1920×1200 |
| 脚本 | `shots/golden/inputs/cm-arrow-menu-click.txt`（`wait 10` / `click 694 272` / `wait 8`，共 21 帧） |
| 哈希 | `472E3BBA1A08` |
| 大小 | 112 348 字节 |
| 与 `cm-arrow-menu-key` 比 | **逐字节相同**（同哈希、同大小）——键盘选行与指针点行落到同一个文档状态 |
| 与 `cm-arrow-both` 比 | 1960 px 不同（相机/取景不同，不是箭头不同） |

**这一场锁的是「派发链」不是「外观」**：它画面里没有菜单（点完菜单就收起了），所以人眼看的图和
`cm-arrow-both` 差不多；但一旦指针派发坏掉，箭头会退回 `off`——整块箭头消失，门立刻红。
这类「外观平凡、行为不平凡」的基线，价值全在**它坏掉时的样子**。

顺带记一条门自己的数字：`PASS script frames: 21 <= warmup 24`——第 28 轮加的那条守则
第一次就在新场景上生效了（脚本帧数必须被 warmup 吃满，否则等于拍到脚本中途）。

### 2.25 拖动建线的指针几何：`cm-link-drag-pointer`（2026-10-27 17:55）

§3 第 1 条一直写着「鼠标路径未单独跑脚本」：既有的 `cm-link-blank` 走的是 `MW_CONCEPT_LINK_BLANK` 钩子，
它和鼠标松手调的是**同一个 `commit_link`**，但「按在锚点上 → 拖 → 松在空白处」这段指针几何从来没被脚本走过。

§2.24 证明指针能派发之后，这条就只差**坐标**。锚点落在节点矩形四条边的中点，命中半径 `ANCHOR_HIT = 7px`（CSS px，
在世界空间里随缩放换算），所以先要节点的屏幕矩形。又一次用一次性探针（`MW_PROBE_NODES`，验完即删）量出来：

```text
probe_node text=用户痛点 box=[523.4,398.0..642.1,442.5] anchors=L(523.4,420.3) R(642.1,420.3) T(582.7,398.0) B(582.7,442.5)
```

这些是**窗口逻辑点**（`1280×800` 窗口下直接就是脚本坐标）。取右锚点 `(642.1,420.3)` 按下，
分四帧往右上挪到空白 `(1100,200)` 松手，trace 第一帧就给了结果：

```text
golden_input 12 pointer [642.1 420.3] Primary pressed=true
golden_input 13..16 move [660,400] → [720,340] → [850,250] → [1100,200]
golden_input 17 pointer [1100.0 200.0] Primary pressed=false
link_new_concept from=01a17cc3-…7d33 to=01a17cc3-…a2e3 at=136,-330
```

一次就通，没有调参：锚点命中 → 拖 → 松手 → 新概念 + 新关系线。

| 项 | 值 |
| --- | --- |
| 平台 | `--ppp 1.0`、`1280×800`、`--warmup 30`、`--tol 0` |
| 脚本 | `shots/golden/inputs/cm-link-drag-pointer.txt`（27 帧） |
| 哈希 | `B699A5AA689D` |
| 大小 | 119 119 字节 |
| 确定性 | 第二遍 `--tol 0` → `无超容差差异` |
| 与 `cm-link-drag` 比 | 11 009 px（钩子版落在别处，相机也不同） |
| 与 `cm-default` 比 | 4 377 px（新增的一个概念框 + 一条线） |

**这一场锁什么**：锚点命中半径、拖动阈值、松手落点、以及「新概念建在哪」这一整套几何。
钩子版 `cm-link-drag` 只锁了 `commit_link` 之后的样子，指针几何它一点都不覆盖。

**它不证明什么**：不证明真实鼠标的抖动/多帧微动下也稳——脚本的移动是跳变的四帧，不是 60Hz 的真实轨迹。

### 2.21 CM-05 的空白菜单终于有基线了（2026-10-27 04:06）

`docs/30` 给概念图定过一条规矩：**菜单里不许出现按不动的条目**——概念图没有子树，
「全部展开 / 全部折叠」这种树语义不该出现在空白处菜单里；也别把「新建概念」叫成「新建自由主题」。
这条规矩从写下那天起只有代码依据（`app_methods_2.rs` 的 `if !canvas.is_concept_map()`），
没有任何一张基线能证明它真的成立。

本轮把夹具扩成 `MW_CONTEXT_DEMO=blank`（原来只开节点菜单并展开「样式」子菜单），
新基线 `cm-blank-menu`（第 26 张）：概念图 + 空白处菜单。

`MW_TRACE` 里菜单引擎自己报的数是 `event menu_open anchor=360,120 entries=9`，
和代码逐条对得上（分隔线也算一项）：

| # | 条目 | 说明 |
| --- | --- | --- |
| 1 | 新建概念 | 概念图叫「概念」，不叫「自由主题」 |
| 2 | 粘贴 | 剪贴板空时置灰 |
| 3 | —（分隔） | |
| 4 | 整理布局 | 概念图专有（CM-04） |
| 5 | —（分隔） | |
| 6 | 适应画布 | `Ctrl+1` |
| 7 | 100% | 已经在 100% 时置灰 |
| 8 | —（分隔） | |
| 9 | 打开文档所在文件夹 | 未保存时置灰 |

**没有**第 10、11 项——`全部展开` / `全部折叠` 那一整段在概念图下不进入 `items`。

尺寸与确定性：

- 平台：`--ppp 1.0`、`1280×800`、`--warmup 8`、`--tol 0`；回读 1920×1200（逻辑 × 1.5）。
- 与 `cm-default` 逐像素比：**62 605 px 变化**，bbox device `x 349..612 / y 119..361`；
  卡片比节点菜单（`y 119..314`）高，因为空白菜单行更多。
- 连拍两次哈希相同（`55EB4C27300D`），不是「看起来一样」。

诚实边界：这张锁的是**条目集合与外观**，不是每个条目点下去的效果（指针路径见 §2.20 的负结果）。

### 2.22 门自己长了一条守则：脚本帧数必须被 warmup 吃满（2026-10-27 04:02）

`cargo golden` 只报「脚本 N 帧」，不管 warmup 够不够。拍在脚本中途的图是**确定**的
（每次跑都是同一帧），所以它不会飘——它只会悄悄地锁错画面：菜单开到一半、编辑器打字打到一半。
这种错比红更难发现，因为门是绿的。

门脚本的 `Run-GoldenScene` 现在把两行日志都抓下来比对，不够就 `FAIL` 并让整道门失败：

```text
PASS script frames: 25 <= warmup 32
FAIL script frames: warmup 12 < 脚本 25 帧（拍到脚本中途）
```

现状：7 个带脚本的场景全部富余（17≤20、25≤32、15≤20、13≤18、38≤44、24≤30、17≤26），
两遍各报一次，共 14 条 `PASS script frames`。失败分支单独验过：把 warmup 换成 12 喂同一段逻辑，
输出正是上面那行 `FAIL`。

### 2.23 多选那条菜单 arm 也上锁了（2026-10-27 07:05）

概念图节点菜单有两条 arm：单选（`selected_count() == 1`）和多选（`> 1`）。
27 张基线里，**多选那条一条都没有**——它是独立的一段代码（`app_methods_2.rs` 的
`ContextTarget::Node if canvas.is_concept_map() && canvas.selected_count() > 1`），
而且比单选多一块：「排列」子菜单（对齐 6 项 + 分布 2 项 + 两个分组标题，共 10 项）。
那条子菜单够高，会压到 `place_submenu` 的竖直夹取；本轮只锁住「它长什么样 + 有几项」，
**夹取本身没有单独量**（见下面的诚实边界）。

夹具扩成 `MW_CONTEXT_DEMO=multi`：选两个概念（跳过主概念）、开节点菜单、展开「排列」。

**撞到一处真规矩**（不是 bug，是漏读）：`selected_free_topics()` 明确跳过 `root_id`——
概念图里「第一个概念」就是 root，第一版夹具照 `doc_order().take(2)` 选，结果
`free_topic_count() == 1 < 2`，「排列」子菜单根本没出现，`entries=7`。
改成跳过 root 选两个之后才 `entries=9`（7 条根项 + 分隔线 + 「排列」子菜单）。
这条值得留着：**夹具选得对不对，菜单自己会说实话**（`entries` 数就是旁证）。

测量：

| 项 | 值 |
| --- | --- |
| 平台 | `--ppp 1.0`、`1280×800`、`--warmup 8`、`--tol 0`；回读 1920×1200 |
| 哈希 | `779AEB5E06D3`（连拍两次相同） |
| 大小 | 136 504 字节 |
| 菜单引擎自报 | `event menu_open anchor=360,120 entries=9` |
| 与 `cm-default` 比 | 134 172 px 变化，bbox device `x 172..1363 / y 17..1194`——**不是只有菜单**：多选换了选区、相机也重取景，所以整图移动过 |

诚实边界两条：

1. 这张锁的是「多选菜单 + 排列子菜单长什么样」，**不是**点其中一项之后的效果（指针路径见 §2.20）。
2. 因为选区和相机都变了，这张图**不能**用差分来单独定位菜单卡；要看菜单本身，
   得和 §2.21 的 `cm-blank-menu`（只差菜单）对照读。
3. 「排列」子菜单在 800 逻辑高的窗口里会不会翻到上方、翻完之后哪一行贴边——**未量**。

### 2.26 双击那条带子改问题：三个真 bug 和一张新基线（2026-10-27 18:35）

§3 第 5 条①和第 1 条是同一个洞：带子的**指针路径**（双击进编辑）从来没有脚本走过。这次把它挂上
`MW_GOLDEN_INPUT`，结果一跑就撞出三个用户能感知的 bug——它们能活到今天，正是因为这条路径既没有基线
也没有脚本。

#### bug 1：双击带子，编辑器根本没被调用

先把脚本写成「`doubleclick 960 75` + `text ...`」，trace 里死活没有 `focus-question` 事件；
用一次性探针确认 `clicked=true / double_clicked=true` 都到了，说明事件没丢、是**没人接**。
读代码：`canvas_methods_3.rs` 里两段 UI 被同一个 `if` 罩着——

```text
if self.rel_edit.is_some() {
    dirty |= self.rel_editor_ui(ui);
    dirty |= self.focus_editor_ui(ui);   // 缩进在关系编辑器分支里
}
```

带子的编辑器于是只在「关系短语编辑器也开着」时才画。撕成两段后 trace 立刻出
`event focus-question text=...`。**这是本轮之前就存在的生产 bug**，不是脚本写错。

#### bug 2：双击带子会顺手在背后新建一个浮动主题

修好 bug 1 之后，同一张图上 `cm-focus-question-edit` 与 `cm-focus-question` 差 36 275 px，其中
26 268 px 落在画布主体的 304–919 行。平移搜索（dy −12..12）最优点只比 dy=0 好 5 px，说明不是位移、
是**内容真的变了**。原因：双击处理排在带子的 `ui.interact` 之前，
`hit == None` 那一支照旧走 `add_floating_world()` ——于是在带子背后多出一个概念框，顺带 `request_fit`
把相机也重取景了。

修法不是加一个特例判断，而是把带子接进画布**已有的指针门禁**：新增 `canvas.focus_band: Option<Rect>`，
每帧在 `paint_focus_question` 里写/清，并进 `pointer_blocked_at()`（和缩放控件、悬停工具条同一条路）。
双击那支再加一道 `response.interact_pointer_pos()` 落在带子里就跳过——用这一击自己的位置，
别用 `hover_pos()`（离屏下它可能为空，第一次修就是这么漏的）。

#### bug 3：`Shift+Home` 顺手把画布「适应视图」了一遍

`key ctrl+a` 在带子编辑器里会被画布快捷键抢走（全选变选中全部节点），所以脚本改用 TextEdit 原生的
`Shift+Home`（选到行首）来实现「整句替换」。结果图又动了：y=600 那行的节点边缘从 581 挪到 502、
873–991 的框变成 853–997——按 x=960 反算，缩放从 0.83 跳到 1.00（1.21 倍）。

根因在 `canvas_methods_2.rs`：

```text
if ui.input(|i| i.key_pressed(egui::Key::Home)) { self.fit(viewport); }
```

`key_pressed` 不看修饰键、也不看**带子编辑器有没有焦点**，于是 `Shift+Home` 一样触发重取景。
改成 `consume_key(Modifiers::NONE, Key::Home)` 并把 `self.focus_edit.is_none()` 加进外条件。
这一条同样是生产 bug：真实用户双击带子改问题时按 Shift+Home 选字，整张图会突然跳一下。

#### 新基线

| 项 | 值 |
| --- | --- |
| 场景 | `cm-focus-question-edit` |
| 脚本 | `shots/golden/inputs/cm-focus-question-edit.txt`（38 帧） |
| 平台 | `--ppp 1.0`、`1280×800`、`--warmup 40`、`--tol 0` |
| 哈希 | `8CED3AE31B3D` |
| 大小 | 115 777 字节 |
| 确定性 | 第二遍 `--tol 0` → `无超容差差异` |
| 与 `cm-focus-question` 比 | 4 041 px，三块：标题栏脏标记 19–30 行（1 034）、带子那句字 68–83 行（2 206）、底栏状态 1 183–1 194 行（798）；**画布主体零漂移** |

「画布主体零漂移」是本轮最要紧的一个数字：它同时证明了 bug 2 和 bug 3 真的修掉了——只要还有一个
浮动主题或者相机动过，304–919 行不可能是 0。

trace 关键三行（`MW_TRACE`）：

```text
event focus-question text="我们要回答什么"
golden_input_done steps=35 applied=35
```

脚本最后一步是 `move 200 700`：指针若停在带子上，hover 的底色 + 强调线会被烤进基线（实测 8 620 px，
正好是带子矩形 832–1087 × 58–91）。这类「指针停在控件上」的假差异，下次写脚本要一并躲开。

#### 顺带修正的旧记录

- `MW_CONCEPT_FOCUS_EDIT` 这个环境变量在 `app_methods_1.rs` 里是给**大纲页**用的（`cm-outline-focus-edit`），
  画布带子这条以前没有任何定态钩子——现在也不需要，脚本自己能走完整条路。
- 之前把「双击带子没反应」记成「未做」，实际是 bug 1；记在 §3 第 5 条①里已改。

### 2.27 「排列」子菜单的竖直夹取：量了一遍，补两条单测（2026-10-27 19:00）

§3 第 8 条写着「子菜单在 800 逻辑高的窗口里是往上翻还是贴底、最后一行是否被切，**未量**」。
这次先读代码再量：`place_submenu()`（`crates/mw-shell/src/menu.rs:239`）**不翻**，而是把 y 夹进内容区
（`min_y = content.top() + EDGE`、`max_y = content.bottom() - size.y - EDGE`），横向才翻到父行左侧。
而这条夹取分支此前**一条单测都没有**——只有「翻左」和「贴右」两条横向用例。

#### 实量

`cm-multi-menu` 那一刻的 `menu_submenu_rect`（本轮新加的 trace，只在 `MW_TRACE` 打开时格式化）：

```text
menu_submenu_rect row=367,304,595,334 pos=593,298 size=200,300 content=0,0,1280,800
menu_submenu_rect row=367,304,595,334 pos=593,298 size=200,300 content=0,0,1920,1200
```

父行（「排列」）在 `y 304..334`，子菜单 `200×300` 落在 `(593,298)`，占 `298..598`：
离底边（离屏那张 1200 高）还有 202px，**既没翻也没夹**。第一帧 `content` 是 `1280×800`、随后变 `1920×1200`，
是离屏 harness 首帧还没换到渲染目标尺寸，第二帧起稳定。

也就是说：**这一场根本没有触发夹取**，所以「拿一张图证明夹取对不对」不成立。要真的贴底，得把菜单开到屏幕底部，
而 `MW_CONTEXT_DEMO` 的菜单落点是固定的（`(380,300)` 附近），脚本又还没有右键语法——这条只能落到单测上。

#### 补的两条单测

`submenu_clamps_up_when_the_parent_row_is_near_the_bottom`：内容区 `1200×800`、父行 `y 760..790`、子菜单 `200×300`，
断言 `pos.y == 492`（= 800 − 300 − 8）、`pos.x == 526`（贴父行右侧）、`pos.y < row.top()`。
顺带把**取舍**写进注释：夹取会让子菜单盖住父菜单下半部分（父行 760..790 落在 492..792 里），
但横向只重叠 2px、指针不在父行中心之外，所以父行照样可点、不会来回抖。
`submenu_clamps_to_the_top_when_the_row_is_above_the_content`：父行被顶出内容区（`y −40`）时夹到 `8.0`。

`cargo test -p mw-shell --bin mw-shell menu::` → **17 passed**（原 15 + 新 2）。

#### 结论

第 8 条从「未量」降级为「**已量 + 已单测**」，但**不是**「已验证」：真实窗口里贴到底部时的观感（子菜单盖住父菜单下半截）
没有截图证据，只有数字。要补这个，得先给 `MW_GOLDEN_INPUT` 加右键语法，或者给 `MW_CONTEXT_DEMO` 加一个「贴底落点」变体。

### 2.28 给带子的悬停态单独拍一张（2026-10-27 18:46）

§3 第 5 条②写着「带子的悬停态没进基线——离屏没有指针，要锁就得写 `MW_GOLDEN_INPUT` 脚本」。
这轮补上了：`cm-focus-question-hover`，脚本只有三步（`wait 10` / `hover 960 75 600` / `wait 6`，18 帧），门里给 `--warmup 24`（多留 6 帧让悬停底色收敛），
**故意让指针停在带子上**——其它脚本最后都要把指针挪开，否则 hover 会被当成假差异（§2.26 末尾就踩过一次）。

| 项 | 值 |
| --- | --- |
| 场景 | `cm-focus-question-hover` |
| 平台 | `--ppp 1.0`、`1280×800`、`--warmup 24`、`--tol 0` |
| 哈希 | `EAF7A3AE2082` |
| 大小 | 121 564 字节 |
| 确定性 | 第二遍 `--tol 0` → `无超容差差异` |
| 与 `cm-focus-question` 比 | **8 636 px，且全部落在 `x 832..1087 × y 58..91`**——正好是带子矩形（256×34） |

差分只有一条带、而且边界精确等于 `focus_band_rect` 算出来的矩形，说明：
① 悬停没有溢出到带子外面（没有多加阴影/描边）；② 带子矩形本身没动（没有因为 hover 而改布局）。

像素取样（同一坐标，悬停版 vs 静态版）：

```text
(840,60)  (242,238,231)  vs  (252,251,248)
(960,60)  (242,238,231)  vs  (252,251,248)
(960,90)  (242,238,231)  vs  (252,251,248)
(1080,90) (242,238,231)  vs  (252,251,248)
```

底色比静态版暗一档（`242,238,231` < `252,251,248`），四角取样一致，说明整块带子统一换底色而不是渐变。
强调线在带子下沿那一行（`y=91`），和 `hovered` 分支里 `paint` 的位置对得上。

这一张同时把「带子可点」这件事**从代码事实变成像素事实**：任何人把悬停反馈删掉、或者让它溢出带子，
这张图都会立刻变红。

### 2.29 检查器那四格的指针路径：一次点击只发一条命令（2026-10-27 18:58）

§3 最后一条说「检查器四格（箭头四档）没有任何脚本走过」。这轮补了 `cm-arrow-inspector`：
`MW_CONCEPT_ARROW_MENU=1` 先选中第一条关系线（顺带把「箭头方向」子菜单开起来），
脚本两次 `Esc`（第一次收子菜单、第二次收菜单本身），再点第四格「双向」，最后把指针挪开。

| 项 | 值 |
| --- | --- |
| 场景 | `cm-arrow-inspector`（`--ppp 1.0`、`1280×800`、`--warmup 30`、`--tol 0`） |
| 脚本 | `shots/golden/inputs/cm-arrow-inspector.txt`，27 帧 |
| 哈希 / 大小 | `2BB3D7E6B968` / 147 064 字节 |
| 确定性 | 第二遍 → `无超容差差异` |
| 四格矩形 | 无 `1707..1739`、单向 `1747..1791`、反向 `1799..1843`、双向 `1851..1895`，y 均 `340..370` |

#### 三个坑（都不是代码 bug，是脚本写法）

1. `key escape` → **认不出**。键名走 `egui::Key::from_name`，必须写 `key Escape`。
2. **只按一次 `Esc` 收不住菜单**：第一次只收子菜单（`menu_submenu_close key=escape level=1`），
   菜单还在，下一击被当成「点外面」直接关菜单（`menu_close reason=outside`）——点击被吃掉了。
   两次 `Esc` 才对（第二次 `menu_close reason=escape`）。
3. **检查器默认不显示**：`MW_CONCEPT_DEMO=1` 下 `show_inspector` 是关的，四格的 trace 一条都不出。
   加 `MW_INSPECTOR=1` 才画出来（这个钩子本来就有，只是之前的场景没用到）。

#### 对照实验：点击本身零痕迹

为了把「点一下造成的像素变化」和「命令造成的像素变化」分开，跑了两个控制场景：

- **A（不点）**：同样的 `Esc`×2 + 把指针挪开 —— 5 万像素里没有任何一格高亮变化；
- **B（点已经在用的那一格「单向」）**：与本场景只差一个坐标（`1769` vs `1873`）。

B 与 A **相差 0 px**。也就是说：点击不留 hover、不留焦点环、不留半帧动画——
下面那 4 510 px 全部是那条命令的效果。

#### 4 510 px 拆开看

| 行带 | 像素 | 列范围 | 是什么 |
| --- | --- | --- | --- |
| 19–30 | 1 088 | 1627–1761 | 标签条上的**脏标记**（文档从干净变脏） |
| 340–369 | 2 560 | 1747–1894 | 四格这一行：「单向」失去高亮、「双向」得到高亮 |
| 581–585 | 15 | 753–759 | 画布上那条关系线**新增的起点箭头** |
| 1182–1194 | 847 | 13–161 | 状态栏左侧提示文字变了 |

第 3 行那个 15 px 一开始看着可疑，拿参照场景一验就通了：`cm-arrow-both` 与 `cm-default` 整张图只差 **81 px**
（四个箭头加起来），单个箭头就是 15–34 px 这个量级——箭头是细线，不是色块。

#### 结论

检查器四格的指针路径**第一次有脚本证据**：点哪一格 → 哪一格高亮、→ 画布上真的多一个起点箭头、
→ 文档变脏。§3 第 8 条之后的最后一条缺口闭合，概念图场景到 **32 场 / 64 次比较**。

### 2.30 关系线终于“能被看见”：悬停与选中各有一套强调（2026-10-27 19:25）

读代码时撞见一件说不通的事：**点中一条关系线，画布上一点变化都没有**——只有右侧检查器换了内容。
于是“我刚点的是哪条”只能靠猜；悬停更是零暗示，一条 1.5px 的细线没人知道它能点。
CmapTools 里连接线一选中就亮起来，这里补的是同一件事。

| 状态 | 画法 | 为什么 |
| --- | --- | --- |
| 选中 | 强调色实色、**3px** | 与节点选中环同色系；线本来就细，不加到 3px 压不住 |
| 悬停（且没选中它） | 同色 **45% + 2px** | 比选中弱一档，避免“划过就以为选中了” |

不碰 `Scene`、不碰格式：`SceneRelationship.path` 已经算好世界坐标路径，按当前相机搬到屏幕、
曲线采样成折线后一条 `Shape::line` 画完，代价只与被强调的那一两条线有关。

#### 三个必须记下来的坑

1. **药丸会被画成删除线**。标签药丸在合成纹理里压在线上（`instances.rs` 先画线再画药丸），
   强调线若照直画过去，就等于给标签画了一道删除线。做法：在世界坐标里把药丸矩形挖掉再画。
2. **直线段只有两个采样点，挖不动中间**。首次实现按“曲线 12 采样”加密，可直线段原样只有首末两点，
   `split_around` 若丢掉一个端点就只剩一个点、整段被丢弃——于是**什么都没发生**：
   重拍后 8 张基线像素数与改动前**逐位相同**（`1217 / 1954 / 24 / 1252 / 1120`），
   这个“没有任何变化”本身就是发现 bug 的证据。修法：先 `densify`（步长 6 世界单位）再挖洞。
3. **assert 的“中点”要选对控制点**。第一次写曲线单测时两个控制点一上一下，在 t=0.5 正好抵消，
   采样正确与否都看不出差别（实测中点恰好落在直线上）；改成两个控制点同向拉才有效。

#### 悬停实拍

| 项 | 值 |
| --- | --- |
| 场景 | `cm-rel-hover`（`--ppp 1.0`、`1280×800`、`--warmup 16`、`--tol 0`） |
| 脚本 | `hover 1134 644 120`，指针停在那条线中点上（故意不挪开） |
| 坐标来源 | 一次性 `rel_screen` 探针（纯 demo 第一条线 `a=991,619 → b=1278,670`），用完已删 |
| 基线 | SHA `FED7CE1055B1`、**114 922 B** |
| 差分 | 与 `cm-default` 差 **706 px**，bbox `990..1278 × 617..670` |
| 药丸缺口 | 差分被切成两段：`617..640`（352 px）与 `648..670`（354 px），中间 `1106..1162` 一条**零差分**的缝 |
| 命中证据 | `event rel_hover <from> <id>`（截图只能证明“亮了”，trace 才证明“亮的是这条”） |

最后那行“零差分的缝”是最硬的一条：药丸中心在 `1134,644`，差分缺口正好落在 `1106..1162`，
说明强调线在药丸处真的断开，而不是被药丸挡住（挡住的话缺口会随药丸高度变化、且边缘会有半透明过渡）。

#### 既有基线漂移（8 张，全部因为“被选中的关系线现在会亮”）

| 场景 | 漂移 px | 变化范围 | 新 SHA | 新体积 |
| --- | --- | --- | --- | --- |
| `cm-arrow-inspector` | 954 | `494..761 × 442..587` | `2DBBDD42CA1E` | 148 008 B |
| `cm-arrow-menu` | 961 | `622..891 × 441..587` | `13CC3FB922F1` | 133 234 B |
| `cm-arrow-menu-click` | 961 | `622..891 × 441..587` | `6A06DF4BB811` | 113 261 B |
| `cm-arrow-menu-key` | 961 | `622..891 × 441..587` | `6A06DF4BB811` | 113 261 B |
| `cm-link-create` | 1 955 | `598..926 × 441..856` | `B6E44601D82D` | 124 803 B |
| `cm-link-blank` | 24 | `641..820 × 428..461` | `7B0C207A85CE` | 116 995 B |
| `cm-relink-blank` | 1 215 | `1196..1329 × 701..1029` | `ACB20B92EED0` | 115 500 B |
| `cm-label-ime` | 1 116 | `985..1294 × 700..857` | `203BBC116C29` | 114 324 B |

> 上表每个场景在门里的 `--warmup`：`cm-relink-blank` 10、`cm-rel-hover` / `cm-rel-adjacent` 16、
> `cm-rel-curve` 22、`cm-focus-question-hover` 24。规矩仍是「脚本帧数必须被 warmup 吃满」（§2.22）。
`cm-link-blank` 只有 24 px 是合理的：那张图里被选中的关系线本来就短，且大部分落在药丸与新节点盒之间。
两两对照还发现一件事：`cm-arrow-menu-click` 与 `cm-arrow-menu-key` 的 SHA **完全相同**（`6A06DF4BB811`）——
用鼠标点四格与用键盘选四格，落到画布上是同一张图，这正是第 35 轮想要的“两条路径同一条命令”的像素级佐证。

### 2.31 关系线的“弯”也被覆盖了：造一条真的弯线，并让药丸在弯线上断开（2026-10-27 21:41）

上一轮收尾时我给自己留了三条残余，其中两条在写的时候是“凭印象”，这一节把它们逐条查清。

#### ① 悬停没换指针形状 —— 这条是**我写错了**

我的原话：`PointerIcon` 只在节点/锚点上换过，关系线这一路没接。查代码：
`canvas_methods_2.rs:570` 那行是 `hot.is_some() || pointer_over_bar` → `CursorIcon::PointingHand`，
而 `hot` 来自 `hit_at()` → `Target::Relationship`（点它能选中就是关系线在命中集里的证据）。
所以**悬停关系线本来就是手型**，这条残余不存在。文档里的错误比代码里的更难发现，记在这里。

#### ② 曲线关系线的药丸缺口只有单测 —— 这条是真的，补上了

难点是夹具：demo 图里 6 条命题线**全是直线段**（第一次掏探针查 `PathSeg` 全是 `Line`，0 条弯）。
弯线只在**两条关系线几何相交**时出现（`scene.rs:693` 的 `segments_cross` → `bowed_between`），
所以得造一条“斜穿既有辐条”的线。试了 6 组落点，只有 3 组真的相交（`MW_CONCEPT_LINK_BLANK=增长策略|600|-500` 等）。

第二次尝试还踩了一个坑：`LINK_BLANK` 建完线会自动打开短语编辑器，
于是“选中版”里有一个带光标的输入框、而“点空白取消版”里它被关掉了——
差集里混进了编辑器的 314 px（一个 39×13 的致密小块 + 一条 2×17 的光标竖线），
差点被当成“药丸被划穿”的像素。修法：脚本里 `text 引起` + `key Enter` 把短语提交，
两边都关着编辑器，差集**只剩强调线**。

#### 证据

| 项 | 值 |
| --- | --- |
| 场景 | `cm-rel-curve`（`--ppp 1.0`、`1280×800`、`--warmup 22`、`--tol 0`） |
| 夹具 | `MW_CONCEPT_LINK_BLANK=增长策略|600|-500` + 脚本 `text 引起` → `key Enter` |
| 基线 | SHA `58C6E15F53A7`、**131 282 B** |
| 对照 | 同一场景末尾 `click 120 740` 取消选中，两者差集 = 强调线 |
| 差集 | **3 392 px**，bbox `645..1336 × 168..745` |
| 连通域 | 恰好 **2 块**：`x1005..1336 y168..435`（1 698 px）与 `x645..966 y467..745`（1 694 px） |
| 药丸缝 | `x 967..1013`（斜线的 x 投影宽度 ≈ 药丸宽 38 + 高 22 / 斜率 0.836 ≈ 64 的实测落地值） |
| 弯曲量 | 离弦最大 **9.14 px**、均值 4.63 px、超过 3 px 的像素 **2 467** 个 |

最后一行是这轮最硬的数字：若强调线被画成“两端相连的直线”，离弦距离最大只会到半个描边宽（≈1.5 px）；
实测 9.14 px 恰好等于 `bowed_between` 的弯弧上限 6 世界单位 × 1.5 缩放，说明**强调线跟着曲线走**。
而“恰好 2 块连通域”说明它在药丸处真的断开——不是被药丸盖住（盖住会是 1 块、且边缘有半透明过渡）。

#### 顺便清掉的临时探针

为了查“demo 里有没有弯线”，我往 `paint_relationship_emphasis` 里塞过一段每帧遍历所有关系的 `rel_curved` trace。
它已完成使命（结论：0 条弯线），已删除；删的时候切错边界留了半截 `for` 和一个错位的文档注释，
`hover_bar` 的注释被我误删过一次，都已恢复。现在文件里没有任何一次性探针。

### 2.32 指着概念就能读出它的命题：悬停点亮相关关系线（2026-10-27 21:49）

上一轮补的是「指着**线**」。可概念图里更常问的是反过来的问题：**这个概念连着哪些命题？**
CmapTools 里把指针放到一个概念上，与它相连的连接线会一起亮起来——这是读图的主路径，不是装饰。

#### 做法：强调线加第三档

`paint_relationship_emphasis` 原来是两档（选中 / 指着线），现在三档，强弱分明：

| 档 | 触发 | 描边 | 不透明度 |
| --- | --- | --- | --- |
| 重 | 选中的那条线 | 3.0 px | 100% |
| 中 | 指针正指着这条线 | 2.0 px | 45% |
| 轻 | 指针指着它的**端点概念** | 1.8 px | 30% |

第三档刻意比第二档更淡更细：否则「指着概念」和「指着线」看起来一样，用户会以为自己在指着线。
药丸断开的逻辑对三档一视同仁（同一段路径、同一个洞）。

#### 证据

| 项 | 值 |
| --- | --- |
| 场景 | `cm-rel-adjacent`（悬停「用户痛点」中心 `873,630`，`--warmup 16`、`--tol 0`） |
| 对照 | 同场景把指针停在空白 `120,740`（脚本 `shots/golden/inputs/r38-empty-hover.txt`，不登记为门场景，只作对照），差集 = 悬停效果总和 |
| 基线 | SHA `719D262776A7`、**118 754 B** |
| trace | `event rel_adjacent node=<uuid> links=3`（这个概念有 3 条命题线） |
| 差集 | **4 077 px**，bbox `623..1278 × 367..670` |
| 其中盒内 | **940 px**（节点自身的悬停反馈） |
| 其中盒外 | **3 137 px**（≥20 px 远离概念盒 → 只可能是连线被点亮） |

盒外那 3 137 px 按「从概念中心出发的角度」分箱，落在几条从该概念射出的细长角段上
（`0–4°` 492 px、`216–222°` 333 px、`228–270°` 276 px、`326–358°` 2 036 px），
另有一处 `91×48` 的块状聚类在概念正上方——那是悬停工具条，不是连线。
换句话说：**亮的是从概念射出去的线，不是概念周围的晕**。

#### 一处我没做漂亮的地方

我本来想用「盒外连通块数量」直接对上 `links=3`，但抗锯齿把每条线断成 2–3 段（实测 8 块），
数字对不上就会被误读成「多亮了线」。所以改成角度分箱 + 明确的「块状 vs 细长」区分，
并老实写清楚「工具条也算盒外像素」这件事。

### 2.33 拖终点时不该再画那条 3px 强调（已实现，未实拍）（2026-10-27 22:01）

§2.30 留的第三条残余是「选中强调与正在拖终点同时出现时谁压谁」。这轮把它做掉了，但**没有实拍**，
原因写在下面，别把它读成「已验证」。

#### 判断

拖终点的手势（`relink_drag`）会从固定端到指针画一条 1.6px 的强调色预览线；
而选中强调会在**原来那条线**上再画一条 3px 实线。两者同时出现时，画面读成「拖出了一条平行线」，
用户会以为自己在复制一条关系，而不是在改它的终点。所以拖拽期间不画强调，让预览线独占焦点。

```rust
// 正在拖终点时不画强调：这一刻的焦点是预览线（1.6px 强调色），
// 原来那条 3px 实线会让画面读成「拖出了一条平行线」。
if self.relink_drag.is_some() {
    return;
}
```

#### 为什么没实拍

我先加了 `MW_CONCEPT_RELINK_DRAG` 钩子（对齐已有的 `MW_CONCEPT_LINK_DRAG`），跑出来 `concept-hook ran=true`，
但截图的 SHA 是 `954F0F34E5ED`——**和 `cm-default` 逐位相同**，也就是说那一帧根本没有拖拽态。
根因：离屏 harness 里没有按住的主键，`handle_pointer` 的松手分支（`canvas_methods_2.rs:1234` 的
`relink_drag.take()` → `commit_relink`）会在同一帧就把它提交掉。`LinkDrag` 之所以能拍，是因为它只写 `self.link`，
而 `paint_link` 不做松手判定。

钩子拍不出预期画面就是个死钩子，所以我把那四处改动**全部撤回**（枚举/分派/环境变量/方法），
没有留在代码里充数。要真拍到这一帧，需要二选一：
① 让 harness 的输入脚本支持「按住不放」（`press` + 多帧 `move`，不动画布代码）；
② 加一个**只被绘制读取**的 demo 拖拽态字段（不进交互路径）。
两条都不在今晚的时间窗里，所以这条状态是**已实现 / 未实拍**。

#### 副作用检查

`cm-relink-blank`（提交后的那一帧）重拍后 SHA 仍是 `ACB20B92EED0`，与旧基线逐位相同——
说明这段抑制只影响「拖拽进行中」的帧，不影响任何已登记基线。

### 2.34 强调线要跟着关系的淡入淡出走（2026-10-27 22:08）

概念图里新建/删除关系有过渡：`SceneRelationship.alpha` 会在几百毫秒里从 0 走到 1（或反过来）。
强调线原来是「档位不透明度」直接乘到强调色上，**完全不管 `rel.alpha`**。
后果很具体：一条刚建好、还在 30% 透明的命题线，如果它恰好是选中项，
强调会把它画成 100% 实色——于是画面上先出现一条满色线，再慢慢「变回」细线，看着像幽灵线。

#### 改法

```rust
/// 强调线的最终不透明度 = 档位 × 关系自身的过渡透明度。
pub(crate) fn emphasis_alpha(tier: f32, rel_alpha: f32) -> f32 {
    (tier * rel_alpha.clamp(0.0, 1.0)).clamp(0.0, 1.0)
}
```

两头都夹取：档位写错（比如复制粘贴时多打了个数）或 `alpha` 越界，都不该画出「比实色更实」的线。

#### 证据

| 项 | 值 |
| --- | --- |
| 单测 | `emphasis_alpha_fades_with_the_relationship`：`1.0×1.0=1.0`、`0.30×0.5=0.15`、`0.45×0=0`、`3.0×2.0=1.0`（夹取）、`-1.0×1.0=0`（夹取） |
| 回归 | 门 35 场 / 70 次比较**零差异**——稳态下所有关系的 `alpha` 都是 1，乘法是恒等变换，所以这条改动**不改任何已登记画面** |
| 测试总数 | 856 → **857** |

「零差异」在这里不是「没生效」，恰恰相反：它证明这条改动只在过渡态起作用，
没有顺手把稳态画面也改了（那才是最危险的那种「优化」）。

#### 没做的

「淡入到一半时被强调」的那一帧**没有实拍**：demo 的过渡动画没有对应的定格钩子，
截图永远抓到稳态。要拍得给 harness 加「在第 N 帧冻结过渡」的能力。数学部分由单测保证。

### 2.35 给药丸的挖洞矩形补上单测（2026-10-27 22:16）

§2.29 修的是一类很脏的 bug：关系线的标签药丸被线划穿。挖洞靠 `split_around` 把路径切掉一段，
而「切哪儿」由 `label_pill_rect(rel)` 决定。这个函数一直是**没有单测**的——它要一个 `SceneRelationship`，
而那东西在测试里造起来很啰嗦，于是就一直没人补。

#### 做法：把纯几何拆出来

```rust
fn label_pill_rect(rel: &SceneRelationship) -> Rect {
    label_pill_rect_at(rel.label_at, rel.label_w)
}

pub(crate) fn label_pill_rect_at(at: Point, label_w: f32) -> Rect {
    let w = label_w.max(REL_LABEL_MIN_W);
    let h = REL_LABEL_H;
    Rect::new(at.x - w * 0.5 - 1.0, at.y - h * 0.5 - 1.0, w + 2.0, h + 2.0)
}
```

外扩那 1px 是给抗锯齿留的余量（正好贴边会漏线头），这个意图现在也写进了测试，防止以后有人「顺手」删掉。

#### 两条断言

| 测试 | 钉住什么 |
| --- | --- |
| `label_pill_rect_is_centered_on_the_label` | 矩形中心**正对** `label_at`（偏一点就会在线的一侧留下没挖掉的线头）；宽 = `label_w + 2` |
| `label_pill_rect_keeps_a_minimum_width` | 两个字（「引起」）不会把药丸压成竖线：有 `REL_LABEL_MIN_W` 兜底，且兜底后仍居中 |

#### 状态

- 单测 857 → **859**（+2）。
- 纯几何重构，数学是恒等变换 ⇒ 门 35 场 / 70 次比较应当**零差异**；
  这条「零差异」就是「重构没有偷改画面」的证据。

#### 一处小坑

第一版我把断言写成 `r.center_x()`——`Rect` 没有这个方法，只有 `center()` 返回 `Point`，编译直接报错。
改成 `r.center().x` 后通过。记在这里是因为这类「凭印象写 API」的错误，
在纯几何代码里最容易蒙混过关（编译器抓不到语义，只抓得到类型）。

### 2.36 挖洞的退化输入：1 点段是崩溃的入口（2026-10-27 22:21）

`split_around` 现在是「强调线避开标签药丸」的唯一实现（直线和曲线都走它）。它原有的两条单测覆盖了
「中间切一刀」和「没命中就别动」，但**都没有覆盖退化输入**。读代码时发现两个真实风险：

1. 药丸盖住整条线（长标签压在短线上）时，剩下的不是「两段」而是**零段**——如果哪天有人在这里加一句
   `assert!(!runs.is_empty())`，或者绘制器假设「至少一段」，就会在长标签场景炸掉。
2. 洞前只剩**一个孤点**时，代码走 `cur.len() < 2` 分支把它 `clear()` 掉。这个分支是故意写的，
   但没有测试守着——一旦被改成 `runs.push(...)`，绘制器就会拿到 1 点折线，`Shape::line` 对退化折线
   的行为是「什么都不画」，看起来像随机丢线。

#### 补的两条

| 测试 | 钉住什么 |
| --- | --- |
| `split_around_drops_everything_when_the_pill_swallows_the_line` | 全被盖住 ⇒ `runs.is_empty()` |
| `split_around_never_emits_a_one_point_run` | 孤点被丢、尾巴保留 ⇒ 恰好 1 段且首点是洞后第一个采样点 |

#### 状态

- `split_around` 相关单测 2 → **4**，工作区测试 859 → **861**。
- 测试专用改动，不碰渲染/几何路径 ⇒ 只跑 CI（含 fmt / clippy 指纹 / 全量串行测试 / 31 组 golden 子集），
  不重跑 35 场概念图门；整门最后一次 PASS 是第 41 轮（70 次比较全零差异）。

### 2.37 看不见的线不该被画（2026-10-27 22:30）

第 40 轮让强调乘上了关系自己的 `alpha`，于是有了一种新情况：**强调算出来是全透明的**。
原来的绘制循环不管这个——它照旧把路径采样成折线、按 6 世界单位加密、给药丸挖洞、再建一遍 mesh，
最后画出一批 alpha = 0 的三角形。图形库会照单收下，GPU 也照画不误。

这不是「看得到的问题」，是**白花钱**：一条正在淡出的关系，每帧都在做一次没必要的采样 + 切段 + 三角化。
概念图里同时淡出的往往是刚删掉的那几条（一次撤销会有好几条一起走），开销叠在一起。

#### 改法

```rust
let emphasis = emphasis_alpha(alpha, rel.alpha);
if !emphasis_visible(emphasis) {
    continue;
}
```

阈值取多少不是拍脑袋：8 位色深下一档是 `1/255 ≈ 0.004`，把阈值放在它的**一半**（0.001），
保证「被跳过」的那些在任何主题、任何缩放下都不可能被看成一条线。判定写成独立函数，
这样这个阈值有名字、有注释、也有测试守着。

#### 证据

| 项 | 值 |
| --- | --- |
| 单测 | `emphasis_below_the_threshold_is_not_painted`：`0.0` 跳过、`0.001` 跳过（用的是 `>` 不是 `>=`）、`0.002` 画、`0.30×0=0` 跳过、`0.30×1.0` 画 |
| 回归 | 门 35 场 / 70 次比较**零差异**——被跳过的线本来就是全透明，跳过它不改变任何一个像素 |
| 测试总数 | 861 → **862** |

「零差异」在这里同时验证了两件事：早退没有误杀可见的线；也没有偷偷改线宽或颜色。

#### 没做的

没有量化省下多少毫秒。要量化得给单帧上计时器（`mw-probe` 已有帧预算探针，但那是整帧口径，
分不出这一项）。按采样点估：一条 300 世界单位的关系约 50 个采样点，省下的是一帧里一次 `densify` + 一次
`split_around` + 一次 mesh 构建——小，但这是「本来就不该做」的那一类小。

### 2.38 基线自检：35 张图的尺寸口径（2026-10-27 22:33）

文档里每一节都写着「1920×1200」，写多了就没人核对了。这轮把它**从磁盘上量一遍**：读每张 PNG 的 IHDR，
按场景统计。

| 尺寸 | 张数 | 说明 |
| --- | --- | --- |
| 1920×1200 | 34 | `--ppp 1.0` + `MW_WINDOW_SIZE=1280x800`（逻辑 × 1.5） |
| 960×690 | 1 | `cm-narrow`，`--ppp 1.50` 的窄窗变体（640×460 逻辑） |

35 张全对上，**没有第三档尺寸**。这条自检值得留下的理由：尺寸一错，像素差分的 bbox 会整片错位，
而错位看起来很像「功能坏了」——第 37 轮为了量一条弯线，光是对齐坐标系就花了半小时。

#### 顺带记下的三条不变量（都可随时复跑）

1. 概念图基线**只有 35 张**，与门脚本的 35 条记录一一对应（`review.md` 也是 35 行）。
2. `cm-narrow` 之外的 34 张都必须 `1920×1200`；出现别的尺寸就是 `--ppp` 或窗口尺寸写漏了。
3. **只有 15 个场景用输入脚本**，另外 20 个靠环境钩子或干脆是静态态——这条是量出来的，不是设计的：
   有脚本（`shots/golden/inputs/<场景>.txt`）：`cm-focus-question-edit`、`cm-focus-question-hover`、`cm-rel-adjacent`、`cm-rel-curve`、`cm-rel-hover`、`cm-arrow-inspector`、`cm-crosslink`、`cm-arrow-menu-key`、`cm-arrow-menu-click`、`cm-link-drag-pointer`、`cm-node-edit`、`cm-label-ime`、`cm-outline-link`、`cm-outline-unlink`、`cm-outline-via`；
   没脚本（钩子或静态）：`cm-default`、`cm-focus-question`、`cm-arrow-off`、`cm-arrow-backward`、`cm-arrow-both`、`cm-narrow`、`cm-arrange`、`cm-card-link`、`cm-card-link-back`、`cm-outline`、`cm-focus-long`、`cm-arrow-menu`、`cm-outline-focus`、`cm-outline-focus-edit`、`cm-blank-menu`、`cm-multi-menu`、`cm-link-create`、`cm-link-blank`、`cm-link-drag`、`cm-relink-blank`。
   有脚本的场景，帧数必须被 `--warmup` 吃满（§2.22）。

### 2.39 没有任何强调对象时，整段跳过（2026-10-27 22:41）

上一轮让「全透明的线」不再画。这一轮往上再退一步：**连循环都不该进**。

三档强调各自的门槛是「选中了一条关系」「指着一条关系」「指着一个概念」——三者都没有时，
循环体里每一圈都会走到那句 `else { continue; }`。概念图的瓶颈不在这儿，但这是**可以证明为空的事情**，
空跑一遍就没有任何理由：关系上千条的大图（读入别人的 `.zmind` 就会遇到），每帧白遍历一次数组。

```rust
if !emphasis_wanted(self.relationship.is_some(), self.hovered_rel.is_some(), self.hovered.is_some()) {
    return;
}
```

判定依然抽成纯函数、带名字、带测试：等价性靠「三档的触发条件穷举」这条推理，而不是靠眼力。

| 项 | 值 |
| --- | --- |
| 单测 | `emphasis_is_skipped_only_when_nothing_can_trigger_it`：全 `false` ⇒ 跳过；任一 `true` ⇒ 照画 |
| 回归 | 门 35 场 / 70 次比较**零差异**（没有触发条件时本来就是一帧空绘制） |
| 测试总数 | 862 → **863** |

#### 与第 39 轮那个早退的关系

函数开头现在有**两道**早退，顺序是有讲究的：先看 `relink_drag`（正在拖终点 ⇒ 一律不画），
再看「有没有强调对象」。反过来写也不影响结果，但读起来会像「拖终点只是顺便跳过」——
拖终点那条是**语义**（这一刻焦点是预览线），不是优化。

### 2.41 第 50 轮：一个真 bug——手柄、预览线、占位提示全都画在合成纹理**下面**

#### 怎么发现的

第 49 轮的三张对照证明「点击没选中关系」，于是我改用夹具钩子（`MW_CONCEPT_RELINK_DRAG`）直接把
`relationship` + `relink_drag` 置上，绕开命中测试。画面却与 `cm-default` **逐位相同**（`954F0F34E5ED`）。
加 trace 后看到 `relink-preview` 帧帧在跑、`from=(292.1, 76.0) at=(244.9, 340.0)` 也确实算出来了——
那就是「画了，但没进图」。

再把预览线临时改成不透明纯红（`Color32::from_rgb(255,0,0)`）拍一张：**红色像素 0 个，`sha` 仍是 `954F0F34E5ED`**。
画了、颜色也是对的、就是不可见。

#### 根因

`canvas_methods_2.rs:697` 把合成器的 wgpu 纹理当**一张不透明位图**贴上画布：

```rust
if let Some(id) = self.texture {
    ui.painter_at(rect).image(id, rect, uv, egui::Color32::WHITE);
}
```

egui 的形状按加入次序绘制，所以**任何在这一行之前画的形状都会被整张盖掉**。而当时这四个覆盖层
（跨度手柄、跨度预览、关系端点手柄、拖终点预览）画在这一行**之前**：

| 覆盖层 | 之前 | 后果 |
| --- | --- | --- |
| `paint_span_handles` | 贴图前 | 注解跨度手柄永远看不见 |
| `paint_span_preview` | 贴图前 | 跨度拖拽预览永远看不见 |
| `paint_relationship_handles` | 贴图前 | 关系端点手柄永远看不见（拖终点根本无从下手） |
| `paint_relink_preview` | 贴图前 | 预览线、落空「松手新建概念并改线」提示永远看不见 |

`cm-arrow-inspector`、`cm-relink-blank`、`cm-link-create` 等 8 张基线里，「选中关系」的状态一直存在，
但手柄那几十个像素从来没进过图——**那些场景锁住的其实是「没有手柄的样子」**。

#### 修法与漂移

把这四个调用挪到贴图**之后**（紧跟 `paint_relationship_emphasis`，那里本来就有「贴在合成纹理上」的注释，
只是这段代码没遵守）。全部基线重拍：**8 张变化**，102–153 px、最大差 205–206，包围盒都落在被选中关系的端点附近。

这 8 个数是这样取的：把四个调用**临时挪回**贴图之前重编一份，只跑这 8 场对现基线做差（`target/x100/run8.sh`、
`target/x100/drift8.log`），其中 `cm-rel-curve` 151、`cm-arrow-inspector` 147、`cm-arrow-menu-key` 143 三个数与
修复前那次门的原始日志**逐字相同**——说明这套复现没有引入偏差，另外 5 个数才有资格写进表里。

| 场景 | 变化像素 | 包围盒 | 说明 |
| --- | --- | --- | --- |
| `cm-link-create` | 153 | x597..931 y439..861 | 选中新连线的手柄 |
| `cm-rel-curve` | 151 | x643..1341 y164..747 | 同上 |
| `cm-relink-blank` | 148 | x1191..1331 y699..1035 | 同上 |
| `cm-arrow-inspector` | 147 | x489..763 y437..588 | 同上 |
| `cm-label-ime` | 146 | x980..1295 y699..861 | 同上 |
| `cm-arrow-menu-key` | 143 | x617..892 y436..588 | 同上 |
| `cm-arrow-menu-click` | 143 | x617..892 y436..588 | 同上 |
| `cm-link-blank` | 102 | x639..826 y427..466 | 同上（只有一个端点落在图内） |

`cm-default` **逐位不变**（`954F0F34E5ED`）——它没有选中任何关系，所以手柄不出现。这是「修的是覆盖层、
不是底图」的对照。

#### 「拖终点期间不画强调」终于有了像素证据

两个新场景，只差一个状态位：

| 场景 | 夹具 | 与 `cm-default` 差 |
| --- | --- | --- |
| `cm-relink-select-only` | `MW_CONCEPT_SELECT_REL=带来`（只选中） | **1 228 px** |
| `cm-relink-drag` | `MW_CONCEPT_RELINK_DRAG=带来\|200\|140`（选中 + 停在拖终点途中） | **7 287 px** |

逐像素求交：两态**共有 147 px**（端点手柄，两态都画），**只在选中态出现 1 081 px**（那正是 3px 强调线），
**只在拖拽态出现 7 140 px**（预览线 + 落空提示）。也就是说 `paint_relationship_emphasis` 开头那句
`if self.relink_drag.is_some() { return; }` 确实把强调整段摘掉了，同帧换成了预览线。

#### 未做

- 只拍了「拖到空白」一种落点；拖到**另一个节点**上（`target = Some(id)` 的描边分支）**未实拍**。
- 四个覆盖层里，跨度手柄与跨度预览现在可见了，但**没有专门的基线**去锁它们（注解跨度场景仍是旧的）。
- 这次修的是绘制次序；**取样几何**（`relink_handle_at` 的 `SPAN_HANDLE_HIT` 半径）没有改动，也没重测。
### 2.40 引用体检：19 个 §2.x 全部有效，但我的解析器先撒了一次谎（2026-10-27 22:45）

报告长到 1 500 行以后，最怕的不是写错，是**指错地方**：`§2.31` 指向一个不存在的节，或者 `HANDOFF` 里
`canvas_methods_2.rs:570` 那个文件早就改了名。这轮做一次机械体检。

#### 两条检查

| 检查 | 方法 | 结果 |
| --- | --- | --- |
| 小节交叉引用 | 抽出正文所有 `§2.x`，与真实的小节标题集合求差 | 38 个小节、19 个不同引用、**缺失 0** |
| 文件引用 | 抽出 `HANDOFF` 里 23 个 `xxx.rs` / `.ps1` / `.md` / `.txt` 引用，逐个探存在性 | 首轮报 6 个「找不到」，**全是误报** |
| 设计文档对齐 | `docs/30` §12 的八行状态与本文对照 | 8 行全是「已补 / 已有 / 已实现」，**没有脱节** |

#### 误报怎么来的（这次记下来，下次别再犯）

我那 23 个引用里有 6 个是**裸文件名**（`canvas_methods_2.rs`、`report.md`、`tokens.txt`），而解析器只试了
「原样 / `crates/mw-shell/src/` / `crates/mw-core/src/` / `scripts/` / `docs/`」这几种拼法——
`canvas_methods_2.rs` 其实在 `crates/mw-shell/src/canvas/` 下，`report.md` 在 `shots/x100/CM-10/` 下，
`tokens.txt` 在 `shots/x100/CER-01/` 下。换个按 basename 的 glob 一搜，三个全在。

**这就是第二次踩同一个坑**：第 41 轮做引用审计时也把 `scene.rs` 拼成了 `crates/mw-shell/src/canvas/scene.rs`，
同样报了假警。结论不是「再加几条候选路径」，而是：**审计脚本先建索引**（全仓扫一遍 basename → 真实路径），
再解析引用；否则它只是在检验我的想象力。

#### 顺手修的第三个问题：`HANDOFF` 的「当前现场」标题漂到了中间

体检文件引用时发现：第 32–35 轮那几块（18:35–19:40）被我早先插到了 `## 当前现场` **标题之上**，
而第 36–48 轮的块在标题之下——标题把时间线切成了两段。已把标题移到叙述区最前面，并加一句
「块按写入次序排列，找最新看 `> **第 N 轮` 里 N 最大的那块」。这不是排版洁癖：接手的人第一眼要找的就是最新现场，
标题漂在中间会让他先读到一段旧账再读到结论。

#### 体检顺带纠正了一条自己的判断：「按住不放」其实一直支持

第 39 轮把「拖终点抑制强调拍不出来」记成「离屏 harness 没有按住的主键」，并打算给语法加新指令。这轮离场前核对：
`golden_input.rs:259` 的 `"press" | "release"` 是**两条独立指令**，脚本按帧逐条把事件追加进 `RawInput`；
`press X Y` 之后不写 `release`，主键就一直按着（egui 只在收到 release 事件时才清）。

所以第 39 轮失败的真正原因是那个**钩子自己在一帧里发完了 press + release**，拖拽当场提交，于是与 `cm-default` 逐位相同。
**语法不缺东西，缺的是一个以 `press` 结尾的脚本。** 顺带把 `relink_handle_at`（`canvas_methods_2.rs:1403`）的几何读了：
关系必须**先选中**才能拿句柄，按下点要落在 `sr.path.end()`（目标端）的 `SPAN_HANDLE_HIT` 半径内，
命中后 `start_relink` 会置 `relink_drag` 并 `swallow_click`——从那一帧起强调整段不画。下一轮的脚本因此是确定的：
`click` 线上一点 → `press` 目标端 → `move` 两三帧 → **不写 `release`**。已把这条写进
`HANDOFF` 待办第 1 条。`MW_PROBE_NODES` 只吐节点矩形、不吐关系句柄，坐标得自己从锚点算。

这也是这一轮最值得记的一课：**「工具不支持」这个结论，必须读了代码才能下**。我当时凭失败结果就下了判断，
和「裸文件名拼错路径」是同一类错误——都是拿猜测替代查证。

#### 真的去拍了「拖终点抑制强调」：三张对照，结论是我又猜错了一次

有了「`press` 之后不写 `release` 就能按住」这条，我直接拍了三张（脚本 24 帧，release 版二进制不用重编）：

| 场景 | 脚本要点 | 与 `cm-default` 差 | 结论 |
| --- | --- | --- | --- |
| `cm-relink-drag` | `click 756 429` → `press 852 447` → 两帧 `move` → 不 release | **55 102 px**（bbox `522..1397 × 220..900`） | 画面确实变了 |
| `cm-relink-drag-control` | 同上，但 `press 700 500`（偏离句柄 150px） | 与上者差 **70 767 px** | 不同起点效果不同 |
| `cm-relink-select-only` | 只 `click 756 429`，不按不放 | **0 px** | **点击什么也没选中** |

第三张把前两张的解读**全部推翻**：只点一下与默认逐位相同 ⇒ `756 429` 落在空白处，根本没选中关系。
而 `relink_handle_at` 的第一句是 `let rel = self.relationship.clone()?;`——没选中就直接 `None`，
relink 不可能开始。所以那 55 102 px 与 70 767 px 都不是 relink 的证据，只是「按在某个点拖到另一个点」的效果
（很可能是框选或平移）。

**这一轮真正的产出是三个负结果 + 一个确定的技术事实**：语法与按住可用（trace 里 `pressed=true` 之后只有 `move`，
24 帧全部 apply 完）；缺的是**选中关系那一步**。下一轮别猜坐标，给夹具加「预选中某条关系」的钩子更可靠。

三张图都留在盘上（`shots/golden/cm-relink-drag`、`-control`、`-select-only`），**一张都没有登记进门口**——
没搞清机制就登记基线，等于把错误锁进回归里。

#### 这轮的性质

代码零改动。门与 CI 的结论取自第 47 轮（`cm-gate-r47.log`：35 场 / 70 次比较零差异；`ci-r47.log`：863 测试、
clippy 37 / 新增 0、`CI OK`），本轮的改动只有这份记录本身。
### 2.42 第 51 轮：注解选中环与跨度手柄的第一张像素证据

#### 为什么补

第 50 轮修的五处覆盖层里，**注解选中环**和**跨度手柄**只有代码改动、没有画面证据：38 场概念图基线里
没有任何一场选中过注解。修好了没人拍，等于下一轮谁都不敢说它们真的出现了。

#### 踩到的一个结构约束

第一版夹具 `MW_CONCEPT_SUMMARY` 想在概念图示例上加概要，结果 trace 只有一行 `summary_try root=0 kids=0`。
去翻模板才发现：概念图模板**根本是平铺的**——`start.rs` 里 `Shape::Concept { .. } => assert!(root.children.is_empty())`
那条断言就在守「根节点不挂子级」。概念靠关系线相连，没有父子层级；而 `Command::AddSummary` / `AddBoundary`
要求「目标是该父节点的直接子节点」，在平铺图上建不出来。

所以改用 `tests/fixtures/x100/tiny-cn.zmind`（根节点挂 4 个子级）配 `MW_OPEN`，夹具改名 `MW_SUMMARY_SELECT`
（不再限概念图，注解本来在思维导图里也成立）。

#### 证据

| 项 | 数 |
| --- | --- |
| 夹具执行 | trace `summary_try root=0 kids=4` → `summary_after count=2`（夹具原有 1 条 + 新增 1 条）→ `summary_added …` → `concept-hook ran=true` |
| 对照 | 同文档同一帧、只少 `MW_SUMMARY_SELECT`（`target/x100/mm-ctl/`），SHA `340F5B96BD12`；场景 SHA `90C6AF8BA7B8` |
| 差异总量 | 4 015 px；强度 ≥64/765 的 **1 369 px**；其中 1 652 px 落在 x 350–700 / y 350–700 这一带 |
| 环的颜色 | 该带内场景图有 **591 px** 主题强调绿 `rgb(47,111,94)`，对照图只有 **12 px**；对照图同处是纸白 `rgb(252,251,248)` 1 509 px |
| 稳定性 | 连跑两次 `--tol 0` 均 `✓ 无超容差差异` |

外围角上另有 56 px / 35 px 的差异（左下、右上），是选中态引起状态栏文字变化，不是环。

#### 事故：一次写文件把源码清空了（已恢复）

改掉 clippy 那条多余转换时，我的改文件脚本写错了两行——`writeFileSync(路径, 'utf8')` 把
`canvas_methods_2.rs` 的内容替换成了字面量 `utf8`... 实际落地的是文件名那一行（46 字节）。
发现得快：紧接着的 clippy 复查报「0 命中」，我去看文件大小才发现源码只剩 46 字节。

恢复路径：第 50 轮做漂移量实验时留的 `target/x100/canvas_methods_2.fixed.bak`（75 425 字符，
含第 50 轮的贴图次序修复）→ 拷回 → 把第 51 轮的方法按同一定义重新插入 → `cargo fmt` → clippy → 重编。

复核：`cm-default` 与 `mm-summary-select` 两张基线重跑 `--tol 0` 均**逐位一致**，
随后整门 38 场 / 76 次比较重跑通过（`target/x100/cm-gate-r51b.log`）。
教训与第 50 轮那条 mtime 陷阱同类：**工具脚本自己的写操作也要当成会出错的代码来验**，
写完至少回读一次长度/关键串再往下走。
#### 登记

`mm-summary-select` 已进 `scripts/x100-stage-cm-gate.ps1`，场景数 **38**、比较 **76**。

#### 未做

1. **边界（boundary）**的选中环**未实拍**：与概要共用同一段绘制代码，但边界有自己的几何与 `SetBoundaryTargets` 路径，没拍就没有证据。
2. **多选（additive）**下的环**未实拍**：`select_annotation(..., true)` 那条分支只有单测。
3. 跨度手柄的**拖拽中**状态**未实拍**：`begin_span_drag` 之后要按住不动，harness 缺「按住」语法（与第 39 轮的拖终点同一条限制）。

### 2.43 第 52 轮：边界（boundary）的选中环

第 51 轮把**概要**的选中环拍下来了，但边界没拍——两者共用同一段绘制代码（`ann_rings` 里那个 2px 强调色矩形），
几何却不同：边界圈住的是一片区域、还带一层淡色底。共用代码不等于共用证据，所以补拍。

夹具 `MW_BOUNDARY_SELECT=1`：仍是 `tiny-cn`（根挂 4 个子级），在根的前两个子级上加一条边界并选中。
命令是既有的 `Command::AddBoundary { parent, targets, color }`，颜色 `#8fb9a8`。

#### 证据

| 项 | 数 |
| --- | --- |
| 夹具执行 | trace `annotation_try root=0 kids=4` → `boundary_added …` → `concept-hook ran=true` |
| 对照 | 同文档同帧、无夹具（`target/x100/mm-ctl/`，SHA `340F5B96BD12` 同第 51 轮） |
| 变化总量 | 65 125 px——**边界自身的淡色底**铺开一片，强度大多低于阈值 |
| 强差 | 3 590 px（≥192/765）；其中主题强调绿 `rgb(47,111,94)` **3 274 px** |
| 环的净增 | 整图强调绿：场景 3 983 px / 对照 416 px ⇒ **净增 3 567 px**（与强差里的 3 274 同量级） |
| 稳定性 | 连跑两次 `--tol 0` 均 `✓ 无超容差差异` |

边界底色 `#8fb9a8` **没有**大面积命中精确色值（场景 68 px / 对照 61 px）——它是半透明叠在纸白上，实测得到的是混色。
这一条写下来是为了避免下一轮有人拿「精确色值命中数」去证明边界没画出来：**该看的是 65 125 px 的弱差**。

#### 登记

`mm-boundary-select` 已进门，场景 **39**、比较 **78**。

#### 未做

1. 边界的**多选**（additive）环、**拖拽中**状态仍未实拍（与 §2.42 未做 2–3 同源）。
2. 边界的**颜色编辑**（`SetBoundaryColor`）路径没有画面证据。

### 2.44 门槛上的一次偶发漂移：`cm-link-drag-pointer`

第 52 轮第一次跑门，第 1 遍 39 次比较里**只错 1 张**：

```
[golden] ✗ 1850/2304000 像素超容差（0.080%），最大差 156，包围盒 x[1095..1282] y[170..249]
```

第 2 遍 39 次全过；把这一场单独连跑 **3 次**也都过（`--tol 0`、`--warmup 30`、同一份输入脚本）。
所以它和 `cm-focus-question-edit`（§2.41 记过，≤1/8）是同一类：**偶发**，不是稳定漂移。

这一场是 27 帧的指针脚本：从「用户痛点」右锚点 `642.1 420.3` 按下，拖经 `1100 200` 松手新建概念。
漂移区（图像坐标 `x1095..1282 / y170..249`，按 1.5 倍缩放折算窗口 `x730..855 / y113..166`）在拖拽路径中段偏上，
按代码读**推断**是拖拽预览线/落点提示一类的逐帧几何，实际是哪一层没有单独定位——
要定位得把这一场的帧数逐帧导出对比，本轮没做。

口径：**这不是「修好了」**。它出现在门里意味着门本身有一次假红；下一次跑门若又红在这一张，先单独重跑再判断，别急着改代码。

**量化（第 54 轮补）**：按门里同一份环境变量（`MW_CONCEPT_DEMO=1 MW_CONCEPT_FOCUS=1`、`--warmup 40`、同一份输入脚本）连跑 **40 次**，全过；
加上前一轮的 15 次，共 **55 次连续通过、0 次复现**。也就是说这个偶发的上界大致是「55 次里至多 1 次」（95% 区间上限约 5%），
但**根因仍未定位**。

### 2.45 第 53 轮：跨度拖拽中的预览框

跨度手柄拖起来会画一排强调色预览框（`paint_span_preview` 遍历 `sd.preview`）。这段同样在第 50 轮之前被合成纹理盖掉，
同样没有画面。这轮补上，并且**用第 51 轮那张当对照**——同文档、同一条概要、同一次选中，只差「正在拖」这个状态位。

夹具 `MW_SPAN_DRAG=<概要文字>`：先走 `add_summary_and_select`，再 `span_drag_start(ann, SpanEnd::Bottom)`，
往 `preview` 里多塞一个兄弟（模拟拖到第三个子级），把 `span_drag` 留在画面上。trace：`span_drag_demo preview=3`。

#### 证据

| 项 | 数 |
| --- | --- |
| 对照 | `mm-summary-select`（同文档同选中、不拖） |
| 变化总量 | 10 672 px |
| 强差 | 617 px（≥192/765） |
| 包围盒 | `x723..1153 / y467..588`——一条 430×121 的横带，正是「多出来那个子级」被预览框圈住的位置 |
| 预览框色 | 场景里出现 `rgb(144,172,162)` 370 px / `rgb(144,177,167)` 188 px，即强调色低透明度叠在纸白上的混色 |
| 选中环 | **两张图的强调绿像素数完全相同：852 / 852**——拖拽态没有吞掉选中环 |
| 稳定性 | 连跑两次 `--tol 0` 均 `✓ 无超容差差异` |

最后一行是这轮最值钱的一条：它同时排除了「预览一画、选中环就没了」这个怀疑，
并且说明这两张图只差预览那 10 672 px，其余逐位相同。

#### 未做

1. 预览框的**圆角半径/描边**没有逐值断言，只有像素证据。
2. 松手提交（`commit_span_drag`）后的终态没有单独拍——`SetSummaryTargets` 的效果可以另开一场。

### 2.46 第 54 轮：概要 + 边界同时选中（additive）

第 51/52 轮各拍了一圈单选环，第 53 轮拍了跨度预览；`additive`（按住 Ctrl 再加点第二条）这条路径一直是空白。
夹具 `MW_ANNOT_MULTI=<概要文字>`：先加概要并选中 → 再加边界（这一步会把选择换成边界）→ 把概要按 additive 补回来，
于是两条注解同时在选。trace：`annot_multi selected=2`。

#### 证据

| 项 | 数 |
| --- | --- |
| 对照 | `mm-summary-select`（只选概要那圈） |
| 强调绿 | 多选 **4 381 px**；概要单选 852 px、边界单选 3 983 px ⇒ 约等于两圈环的并集，**两圈都在** |
| 变化总量 | 64 275 px（边界那层淡色底铺开一片） |
| 强差 | 3 284 px，包围盒 `x527..1337 / y438..555` |
| 稳定性 | 连跑两次 `--tol 0` 均一致 |

#### 未做

1. `additive` 的**取消**（再点一次把某条从选中集合里去掉）没有画面证据。
2. 多选时检查器显示哪一条，没有断言。

### 2.47 第 55 轮：标签弹层的「封顶 + 滚动」与一次自锁

起因：查 `panels/outline.rs` 的标签弹层时发现它是一列 `selectable_label`，**没有高度上限、没有滚动**，
锚点是固定偏移（`Pos2::new(r.left() - 20.0, r.bottom() + 6.0)`）。示例文档只有 6 个标签，所以基线里一直看不出来；
但文档里每个节点最多 5 个标签（`MAX_LABELS`），真实文档随便几十个标签就会把弹层顶出窗口。

改法（docs/30 §5.11 先写规则再动代码）：

1. `popup_max_height(screen_bottom, anchor_y, margin, floor)` —— 纯函数，返回「锚点下方到窗口底边」那一段；
2. 弹层内容包进 `ScrollArea::vertical().max_height(max_h)`，位置交给 `Area::constrain_to(窗口内缩 8px)`；
3. `Frame::popup` 的上下内边距（实测 14）不在滚动区里，`max_h` 里减掉 `POPUP_FRAME_PAD = 16`。

#### 证据（场景 `cm-outline-label-many`，夹具 `MW_OUTLINE_LABELS=120`）

| 量 | 值 |
| --- | --- |
| 标签总数 / 弹层行数 | 121 / 66（trace `outline_labels_fixture n=120 ok=120 total=121`、`rows=66`） |
| 窗口 / 锚点 | 1280×800 / y=137.3 |
| `max_h` | 639 |
| 弹层总高 `h` | 653（= 639 + 14 内边距） |
| 底边 | 790.3 ≤ 792（窗口 800 − 8 边距）→ **不越界** |
| 滚动区内容 / 视口 | 2 500 / 639 → **封顶并滚动** |
| 稳定性 | `--tol 0` 连跑两次一致 |

#### 过程中的一次自锁（记下来，下次别再踩）

第一版只加 `ScrollArea::max_height(max_h)`，实测却稳定 `h=400`，而 `max_h` 明明是 655。逐层打点后确认：
`Area` 内层 ui 的 `max_rect` 取自**上一帧的矩形**（egui `containers/area.rs`：`let max_rect = self.state.rect();`），
`ScrollArea` 又按「当前可用高度」封顶，两者互相喂小值 → 自锁。修法：`Frame` 里先 `ui.set_max_height(max_h)`。
修完 `h` 才从 400 变成 669（加上内边距），再扣 `POPUP_FRAME_PAD` 才收进 790。

#### 连带改动

- 旧场景 `outline-search-labels` 用的就是这个弹层，几何变了必须重拍：**41 526 px 变化、2 257 强差**，
  包围盒 `x734..1294 / y230..306`（只落在弹层那一块，其余未动）；重拍后连跑两次 `--tol 0` 一致。
- 新增单测 `popup_max_height_caps_to_window_bottom`（3 个断言）。

#### 未做

1. `constrain_to` 的「太靠下就整体上移」只用库行为保证，没有专门场景（大纲的标签胶囊固定在顶部，拍不出贴底的情形）。
2. 滚动条本身在离屏基线里不可见（`shell_new` 里给 harness 把 `bar_width` 收成 0），所以「有滚动条」这点只有数字（内容 2 500 > 视口 639），没有像素。

### 2.48 第 56 轮：多选「取消」不留残影（用已有基线当断言）

additive 选择（Shift/Ctrl 点第二条）第 54 轮拍了「两条都在选」；**取消**那一步从来没拍过——
而 toggle 语义最容易留下的 bug 正是「取消了但环没撤干净」或「检查器目标没跟着换」。

这一轮不去新造一张「好看的图」，而是让它与**已有基线**比：

夹具 `MW_ANNOT_MULTI_OFF=两条主线先合并`：加概要 + 边界 → 两条都选中 → 再按 additive 把**边界**取消一次。
trace 断言状态机：`annot_multi selected=2` → `annot_multi_off left=1 kinds=[Summary]`。

#### 三条数字（同一份 `tiny-cn.zmind`，ppp 1.0，窗口 1280×800）

| 比较 | 结果 |
| --- | --- |
| 强调绿 `rgb(47,111,94)`（±6）| **838**，与 `mm-summary-select` 的 **838** 完全相同（多选那张是 4 367）|
| 新图 vs `mm-annot-multi` | 差 **3 284 强像素**，包围盒 `x527..1337 / y438..555` ⇒ 只少掉边界那一圈环 |
| 新图 vs `mm-summary-select` | **0 强像素**；60 654 px 弱差是边界对象自身的淡色底（与 §2.27 同源）|

也就是说：取消之后**画面上与「只选概要」只差那条边界本身**，选择状态一点残影都没留。

#### 为什么不用「逐像素等于 `mm-summary-select`」当判据

第一版我确实这么写，跑出来 sha 不同（`da9450158e87` vs `90c6af8ba7b8`）。原因不是 bug：那张基线里**根本没有边界对象**，
而这张有（只是没选中），边界自己是要画的。于是判据换成上面三条——把「有对象」与「被选中」两件事分开量。

#### 未做

- 「检查器在多选下的目标」仍未断言（检查器要 `MW_INSPECTOR=1`，与这张图的夹具不是同一套环境）；
- 取消**概要**（留下边界）同理没拍，判据应镜像：绿 = 边界单选那张的数。

### 2.49 第 57 轮：跨度拖拽**松手之后**到底发没发命令

第 53 轮拍的是「拖拽中」的预览框（`mm-span-drag`），松手那一半从来没验过：
`commit_span_drag` 只在 `preview != origin` 时发一条 `SetSummaryTargets`/`SetBoundaryTargets`，
发不发、发几次（一次撤销还是撒一地）、撤销能不能回来，三件事都没有证据。

夹具 `MW_SPAN_COMMIT=两条主线先合并` 走完整条路，并在应用内自检：

1. 加概要（覆盖 2 个子级）并选中；
2. `span_drag_start` 拿到跨度，预览扩到 3 个；
3. 提交 → trace `span_commit before=2 after=3`（读文档，不读缓存）；
4. `undo()` → trace `span_commit_undo back=2`（**撤销一次就回到 2**，说明只发了一条命令）；
5. 再提交一次，让画面停在「跨度真的变宽了」的状态，供像素比对。

#### 证据

| 断言 | 值 |
| --- | --- |
| 提交后文档目标数 | `span_commit before=2 after=3 changed=true preview=3` |
| 撤销一次 | `span_commit_undo back=2 undone=true` ⇒ **只发了一条命令** |
| 重做 | `span_commit_redo targets=3 redone=true` |
| 画面 vs `mm-summary-select` | 差 **7 198 px**、强差 **2 771 px**、包围盒 `x419..1460 / y448..600` |
| 局部性（按 y 带） | `y0..440` **0 px**、`y440..610` 7 198 px、`y610..1200` **0 px** |
| 强调绿 | 923（对照 838，+85 = 花括号变长那一段）|

局部性那一行是关键：如果提交走的是「整页重排」而不是「只续一段括号」，差就不会这样干净地夹在一条带子里。

#### 未做

- 只证明了**概要**的提交；边界的 `SetBoundaryTargets` 同一条代码路径，但没有单独的场景；
- 反向拖动（把跨度**缩短**）没拍，只有「变宽」；
- 提交后撤销栈里正好一条，是靠 `undo()` 一次就回到 2 推出来的，没有直接读撤销栈长度。
- **没有 `cargo test` 单测**：`mw-shell` 的测试不构造完整 `Canvas`（要 GPU/主题/编辑器版式），这条只有场景内自检 + 像素证据；要补单测得先把 `commit_span_drag` 的「发不发、发哪条」抽成纯函数。

## 3. 未做 / 风险

1. ~~鼠标路径未单独跑脚本~~ → **已补**（第 31 轮）：`cm-link-drag-pointer` 真的按在锚点上拖到空白，trace 出 `link_new_concept`（§2.25）。仍不覆盖真实鼠标的多帧微动。
2. ~~一次撤销未断言~~ → 已修：改成原子命令并有单测（见 §2.1）。
3. ~~②双击空白新建概念仍未开始~~ → 核查为已实现（§2.1）。
4. **⑥ 聚焦问题**四个落点全齐：画布带子 + 检查器 + 三处导出（§2.13）+ **大纲页带子**（§2.15）
   + **大纲页就地改**（§2.17）；
   **⑦ 箭头样式**已于同日落地（四档 + 双箭头几何 + 三处导出 + 两处入口 + 三张基线，见 §2.12），
   `docs/32` §3 里「默认不画箭头」那半句已改正。
5. ~~大纲页那条问题带只读~~ → **已修**（第 24 轮，§2.17）：双击就地改，控件与判定都和画布共用。
   ① ~~双击 → 编辑框这条指针路径没有脚本证据~~ → **已补**（第 32 轮，§2.26）：`cm-focus-question-edit`
   走完整条路（双击进编辑 → Shift+Home 选中整行 → 整句替换 → 回车提交），并在补的过程中撞出**三个生产 bug**
   （编辑器被错 `if` 罩住根本不画 / 双击连带新建浮动主题 / Shift+Home 顺手重取景），三个都已修。
   ② ~~带子的悬停态没有基线~~ → **已补**（第 34 轮，§2.28）：`cm-focus-question-hover` 让指针停在带子上拍，
   与静态版差 8 636 px、边界精确等于带子矩形 `832..1087 × 58..91`，底色 `242,238,231` vs `252,251,248`。
   ③ 光标闪烁相位本身进了基线（`cm-outline-focus-edit` 里 x 799–800 那两列、20px 高），换 `--warmup` 就会漂。
   `cm-focus-question-edit` 定格在**提交之后**，编辑框与光标都已消失，所以不吃这个亏。
6. **带子不封顶行数**（第 25 轮引入的取舍）：问题写得极长（几百字）时，画布顶部那条带子会压掉一大块画面。
   没加行数上限，理由是「问题写得长」本身是作者的选择，截断比占地方更糟；等真实用例说话再决定要不要封顶。
   大纲页那条不受影响：它在 `ScrollArea` 里，跟着页面滚。
7. **⑦ 的「当前档」有基线，「点这一行」只有键盘证据**（第 26/27 轮）：`cm-arrow-menu` 锁住勾在第几行
   （`off` → 第 1 行，`backward` → 第 3 行）；`cm-arrow-menu-key` 锁住「键盘选行 → 文档变 `both` → 渲染跟上」
   这一整段（§2.20，`menu_nav` / `menu_activate` / `relationship head` 三段 trace 齐全）。
   而**指针点弹层行是能派发的**（§2.20 已于 17:30 更正：原「不派发」是坐标算错）：
   第 30 轮补了 `cm-arrow-menu-click` 把这条路径锁进门口（§2.24），所以「鼠标点得动」现在有基线证据，
   同第 1 条与第 5 条①。~~检查器四格那条入口同样没有脚本证据~~ → **已补**（第 35 轮，§2.29）：
   `cm-arrow-inspector` 两次 `Esc` 关掉菜单后点第四格「双向」，四格矩形由 trace 量出；
   对照实验（点「已经在用的那一格」）与不点相比 **0 px**，证明那 4 510 px 全是命令的效果。
8. ~~「排列」子菜单的竖直夹取未量~~ → **已量 + 已单测**（第 33 轮，§2.27）：`place_submenu` 不翻、只夹；
   实测 `cm-multi-menu` 里子菜单 `200×300` 落在 `(593,298)`，离底边 202px，**这一场没触发夹取**；
   夹取分支补了两条单测（父行贴底 `→ pos.y=492`、父行被顶出上沿 `→ pos.y=8`），`menu::` 17 passed。
   ~~**仍未验证**：真实窗口里贴到底部时的观感（子菜单会盖住父菜单下半截）没有截图证据，
   要补得先给 `MW_GOLDEN_INPUT` 加右键语法或给 `MW_CONTEXT_DEMO` 加「贴底落点」变体。~~
   **已补**（打磨轮 4 / C10）：新增 `MW_CONTEXT_POS='x,y'` 只挪请求点，在 `1280×520` 矮窗里让
   300 高的「排列」子菜单必然溢出；`cm-submenu-bottom` 基线 + `menu_submenu_rect` 轨迹锁住夹取
   （`pos.y = 780−300−8 = 472`），夹取后确实盖住父菜单下半截。实机窗口贴底的观感仍无人工核对。
9. ~~关系线没有悬停/选中反馈~~ → **已补**（第 36 轮，§2.30）：选中 3px 实色、悬停 45% + 2px，
   药丸处断开；`cm-rel-hover` 锁住悬停态（SHA `FED7CE1055B1`），8 张既有基线按新语义重拍。
   残余（写下来的目的不是免责，是下一轮接着做）：
   ① ~~悬停没换指针形状~~ → **查代码后否定**：`canvas_methods_2.rs:570` 的 `hot.is_some() || pointer_over_bar`
   已经覆盖关系线命中（`hit_at` → `Target::Relationship`，点它能选中就是证据），所以悬停时指针**本来就是手型**。
   这条残余是我上一轮凭印象写下的，属于文档错误，已改。
   ② ~~曲线关系线的药丸缺口只有单测覆盖~~ → **已补实拍**（第 37 轮，§2.31）：`cm-rel-curve` 造出一条真的弯的、
   带标签的关系线，强调线沿曲线走（离弦最大 9.14 px = 弯弧上限 6 世界单位 × 1.5 缩放），并在药丸处断成两段。
   ③ ~~选中强调与「正在拖终点」`RelinkDrag` 同时出现时谁压谁~~ → **已实现**（第 39 轮，§2.33）：拖终点期间不画强调。
   ~~但**未实拍**——离屏 harness 没有按住的主键，松手分支会立刻提交拖拽，加过的 `MW_CONCEPT_RELINK_DRAG` 钩子
   拍出来与 `cm-default` 逐位相同（`954F0F34E5ED`），已撤回；要拍需输入脚本支持「按住不放」。~~
   **已补实拍**（打磨轮 12 / C3）：`cm-relink-drag` 与 `cm-relink-select-only` 只差「是否在拖终点」一个状态位，
   逐像素求交证明选中态独有的 1 081 px 强调在拖拽态完全消失（拖拽态多出的 7 140 px 是预览线）；
   `cm-relink-near` 另锁 12 css px 吸附余量。早先「钩子拍出来逐位相同」是因为钩子没有真的进入拖拽态。

10. ~~**强调在「淡入到一半」时没实拍**（第 40 轮，§2.34）~~ **已补实拍**（打磨轮 3 / C9）：`MW_MOTION_T=0.5`
    把 `blend_scenes` 的混合系数钉死在 0.5，`cm-link-fade-mid` 与稳态 `cm-link-create` 是同一文档、同一布局，
    差别只有线身与强调的 0.5 透明度；「淡入中那一帧」现在是像素证据，不再只是 `emphasis_alpha` 的数学推断。

> 第 1 条（拖动建线，第 31 轮）、第 5① 条（双击带子改问题，第 32 轮）、第 7 条末句（检查器四格，第 35 轮）
> 都已补进门口，而且**前两次补脚本都撞出真 bug**——这就是「指针路径没有脚本证据」这个洞的实际代价。
> 第 8 条（第 33 轮）补的是单测：打磨轮 4 用 `MW_CONTEXT_POS` 造出贴底触发条件并拍进 `cm-submenu-bottom`。
>
> 顺带纠正本文早先的一句错话：先前记「检查器四格要等右键/点击语法补齐」——**点击语法一直有**（`click X Y`）。
> 真正的三块绊脚石是：键名必须写 `key Escape` 而不是 `key escape`；菜单要按**两次** `Esc` 才关得掉，
> 否则下一击会被当成「点外面」吃掉；以及检查器在 `MW_CONCEPT_DEMO` 下默认不显示，得加 `MW_INSPECTOR=1`。
> 这三条都写进 §2.29 了，下次别再猜。
