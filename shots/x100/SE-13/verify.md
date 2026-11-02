# SE-13 · 独立像素与导出验证（2026-11-02）

> 谁做的：Lead 亲自跑的独立验证（不采信实现者的自述，只用基线与数字）。
> 目的：回答「四项新能力到底有没有画到画布上、有没有进导出」。
> 口径：全部基线都是 `1920×1080 @ ppp 1.5`、`--tol 0`。
> **2026-11-02 19:05 复核**：§1 的像素表已按重拍后的实测重写。第一轮那张「场景 vs
> `inspector-shape`」的横向表与现在的数字**口径不同**，冲突原因见 §1.1，**以 §1 为准**。

## 1. 像素层：重拍前后的纵向漂移（2026-11-02 19:05 实测，唯一有效口径）

检查器「组内行距 8 → 6」是 `docs/30` §4.1 的合同值（「组内 | 上下 8、左右 12；控件行高 28；
**行间距 6**」），实现从 8 改到 6 属**有意外观改动**，14 张检查器基线全部重拍。下表是
**各自旧基线 → 重拍后**的箱内差异（同一场景纵向对照，不是横向对照另一张场景）：

| 场景 | 差异像素 | 占比 | 包围盒读法 |
| --- | --- | --- | --- |
| `inspector` | 43 820 | 2.113% | 只在面板（x ≥ 1506） |
| `inspector-narrow` | 43 847 | 2.115% | 只在面板 |
| `inspector-shape` | 35 150 | 1.695% | 只在面板 |
| `inspector-ink` | 78 488 | 3.785% | 从左边缘起 |
| `inspector-mist` | 77 480 | 3.736% | 从左边缘起 |
| `inspector-sand` | 77 448 | 3.735% | 从左边缘起 |
| `inspector-content` | 78 158 | 3.769% | 从左边缘起 |
| `inspector-multiselect` | 43 316 | 2.089% | 只在面板 |
| `inspector-error` | 37 594 | 1.813% | 只在面板 |
| `inspector-stepper-edit` | 35 241 | 1.700% | 只在面板 |
| `inspector-weight` | 63 770 | 3.075% | 从左边缘起 |
| `inspector-align` | 63 463 | 3.061% | 从左边缘起 |
| `inspector-radius` | 63 591 | 3.067% | 从左边缘起 |
| `inspector-ellipse` | 67 092 | 3.236% | 从左边缘起 |

**读法**：

- **面板-only 的 6 张**（`inspector` / `-narrow` / `-shape` / `-multiselect` / `-error` /
  `-stepper-edit`）：bbox 全在 x ≥ 1506，画布一个像素没动，漂移全部来自行距 8 → 6。
- **带主题/样式覆盖的 8 张**（`-ink` / `-mist` / `-sand` / `-content` / `-weight` /
  `-align` / `-radius` / `-ellipse`）：bbox 从左边缘（x=19）起，因为 SE-13 改的是
  **画布节点本身**（字重改度量 → 整树重排；对齐改锚点；圆角/椭圆改轮廓）——
  这是这批能力**第一次上屏**，不是漂移异常。
- 重拍后逐个复核：**同一二进制、`--tol 0`，14 张全部零差异**。

### 1.1 与第一轮像素表的冲突（如实）

第一轮（2026-11-02 17:2x）这里贴过一张**横向**对照表（每个场景 vs `inspector-shape`）：
`inspector` 47 897 / 2.310%、`inspector-weight` 68 251 / 3.291%、
`inspector-align` 35 019 / 1.689%、`inspector-radius` 37 098 / 1.789%、
`inspector-ellipse` 53 990 / 2.604%。它与 §1 对不上，原因两条：

1. **参照物不同**：旧表是「场景 A vs 场景 B」的**横向**差（用来证明覆盖画到了画布）；
   §1 是「重拍后 vs 各自旧基线」的**纵向**差（用来量化行距改动的漂移）。两者不是一个量，
   数字本来就不该相等。
2. **拍图时间不同**：旧表拍在「组内行距 8 → 6」与复审 5 处修复**之前**，基线此后已重拍，
   照片和基线都换了一代。

所以旧表的数字**只作历史留档、不再引用**；现行唯一有效口径是 §1。
旧表的复跑脚本一并留在这里，但基线已重拍——今天跑它得到的是「新基线之间」的横向差，
同样核不了旧表的那几个数：

```bash
python - <<'PY'
from PIL import Image, ImageChops
base = Image.open('shots/golden/inspector-shape/shell-ppp1.50.png').convert('RGB')
for sc in ['inspector','inspector-weight','inspector-align','inspector-radius','inspector-ellipse']:
    im = Image.open(f'shots/golden/{sc}/shell-ppp1.50.png').convert('RGB')
    d = ImageChops.difference(base, im).convert('L')
    print(sc, sum(1 for v in d.get_flattened_data() if v > 0), d.getbbox())
PY
```

## 2. 导出层：四个键进了 SVG

`mw-shell/src/export_svg.rs` 的 `text_elt()` 按 `TextJob.weight` 写 `font-weight`（≠400 才写）、
按 `TextAlign::Right` 写 `text-anchor="end"`；`rect_elt()` 在半径 ≤ `ELLIPSE_CODE+0.5` 时发 `<ellipse>`、
否则发 `<rect rx=…>`。这条链路由单测 `export_svg::tests::the_new_style_keys_reach_the_svg` 钉住，
它断言同一份场景导出的 SVG 文本里同时出现：

```
font-weight="600"      ← weight 键
text-anchor="end"      ← align=right
rx="14"                ← radius 键（默认 12 + 覆盖 14 的那一档）
<ellipse               ← shape=ellipse
```

**说明（如实）**：本轮没有跑「真实导出到桌面文件」的整链路（那要开文件对话框/走命令面板），
所以导出侧的证据是**单测级**，不是端到端文件级；这与 SE-01–12 的「离屏等效」同一口径。
PDF 侧文字走字形轮廓（`OutlinedText`），圆角/椭圆走 `mw_render::corner_style` 同一出口，
伪粗描边判据本轮已从 `bold` 改成 `weight >= 600`（复审修，见 `report.md` §6）。

## 3. 兼容性不变性

- `cargo test -p mw-format`：`the_new_style_keys_are_optional_and_default_to_the_old_look` 与
  `x100_committed_fixtures_load_and_roundtrip` 均绿——三个新键没设置时不写键，旧 `.zmind` 逐字节不变。
- 新键解析：缺键 ⇒ `None` ⇒ 老行为（字重 400、左对齐、形状默认圆角），不升 `FORMAT_VERSION`。

## 4. 结论

**四项能力在画布端有像素级独立证据、在导出端有单测级同源证据；没有发现「假账」。**
欠的一端是「真实导出文件」的端到端核对（与实机字体/DPI 同一笔欠账），已记进 `report.md` §7。

机器门层面另记一笔（详见 `report.md` §9）：矩阵层在 **2026-11-02 19:35–19:47** 用**同一个二进制**
（mtime 19:32:30）跑了两遍，`stage-b` **178 场 × 2 遍 = 356 次比较全部零差异**、
`stage-cm` **73 场 × 2 遍 = 146 次比较全部零差异**；19:05 那批红（`mm-rel-link-created`、
`mm-summary-hover`、`card-autosave`、`search-text`、`mm-zoom-wheel` 等）是并行会话 `rustc` 构建 +
跑 `mw-shell` 抢占 CPU/GPU 导致的**并发假红**，单场景复跑逐个通过，真漂移只有 `cm-inspector` /
`cm-inspector-pinned`（行距 8→6，已重拍并复核零差异）。

但**这不是「正式门绿」，也不是 `已验证`**：① 这两遍跑的是**已构建二进制**，正式门脚本本身要等
并行会话落笔后补跑（源码一动触发约 4 分钟 LTO 重建）；② 全量测试 1313 / 0 / 1 ignored、`fmt`、
门内 `clippy` 是分别实测，不是一次整跑；③ `已验证` 仍欠实机微软拼音 + 系统 DPI。
功能判词 5/5 通过替代不了门绿，门绿也替代不了实机验收。
