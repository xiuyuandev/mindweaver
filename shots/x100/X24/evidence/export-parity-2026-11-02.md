# X24 · 导出同源证据（G-01–G-07，2026-11-02）

> 范围：Lead 授权「允许改代码，写作用域严格限定导出相关文件」。**只改了两个文件**：
> `crates/mw-shell/src/export_svg.rs`、`crates/mw-shell/src/export_pdf.rs`（含这两个文件里的单测）。
> **没有碰**：`crates/mw-core/**`、`crates/mw-render/src/hit.rs`、`crates/mw-shell/src/canvas/**`、
> `docs/`、`scripts/`、`docs/STATUS.md`、其它 `shots/x100/*`。**没有 commit**、没有 release 构建、没有跑 golden。

## 1. 改的文件与改法

| 文件 | 改了什么 |
| --- | --- |
| `export_svg.rs` | 新增导出侧**唯一一份树边几何**：`tree_edge_path`（调 `mw_core::layout::edge_path_with(…, e.shape)`）、`edge_outline` / `edge_outline_sides` / `unit_normal` / `cap_points`（锥形两侧轮廓 + 端点圆头）、`tree_edge_arrow`（与画布同款 7 css 三角形）、`polygon_path_elt`（闭合多边形 `fill`）。树边循环从「`edge_path` + 单条 `stroke-width`」改成「填充轮廓 + 三角形箭头」 |
| `export_pdf.rs` | 树边循环改调 `crate::export_svg::{tree_edge_path, tree_edge_outline, tree_edge_arrow}` + 新的 `fill_polygon`（`m/l/h f`）；**删掉** `arrow_head()` 与 `const AL = 9.0 / AW = 6.5`（第二套箭头几何）；`polygon` 改为转调 `fill_polygon` |

## 2. 哪几条 G 修好了

| G | 状态 | 判据（测试名） |
| --- | --- | --- |
| G-01 SVG 丢线型 | **已实现** | `tree_edge_path_keeps_the_shape_the_canvas_draws`、`svg_fills_a_tapered_band_and_draws_the_arrow` |
| G-02 SVG 丢锥形 | **已实现** | `tapered_outline_has_the_documented_widths_at_both_ends`、`equal_widths_give_a_straight_band_not_a_cone`、`elbow_outline_turns_at_the_corner` |
| G-03 PDF 丢线型 | **已实现** | `pdf_fills_the_same_tapered_band_and_arrow`（`edge_shape = elbow`） |
| G-04 PDF 丢锥形 | **已实现** | 同上（轮廓点逐点出现在内容流里） |
| G-05 SVG 无树边箭头 | **已实现** | `svg_fills_a_tapered_band_and_draws_the_arrow`（`<polygon points=` 数 == 边数） |
| G-06 箭头两套几何 | **已实现** | `tree_edge_arrow_matches_the_canvas_triangle`（长度 7、底半宽 2.94、尖端锚终点）、`pdf_fills_the_same_tapered_band_and_arrow`；`AL/AW` 已删 |
| G-07 几何不进 `Scene` | **已实现（选项 B）** | 不新增字段；导出侧只经 `tree_edge_path/outline/arrow` 三个纯函数，`pdf_applies_the_world_to_page_scale_exactly_once` 钉住「世界坐标只写一次」 |

## 3. 哪几条**没**修（如实记账）

| 项 | 原因 |
| --- | --- |
| G-08 线宽档位 3.5/2.625/1.75/1.3125 vs 合同 4/3/2/1.5 | Lead 明确：**拍板项，不动** |
| G-11 逐分支线型/箭头/锥形覆盖（新字段） | 同上，需用户拍板（提案在 `report.md` §3.2） |
| G-12 四主题色板 vs `docs/06` 八色表 | 同上，只登记 |
| golden / 基线 / 逐像素（AC-12、AC-13） | Lead 明确：不跑 golden、不 release 构建；`--tol 0` 与 PNG↔SVG 逐像素比**未跑** |
| 新场景 `mm-edge-elbow` / `mm-edge-straight` / `mm-edge-arrow` / `mm-export-parity` | 未开始（要动 `shots/golden` 与门脚本，不在写作用域） |
| `cargo fmt --all` 全仓 | 只对这两个文件跑了 `rustfmt`，全仓 fmt 由 Lead 统一做 |

## 4. 新增单测（8 条）

SVG（6）：`tree_edge_path_keeps_the_shape_the_canvas_draws`、`tapered_outline_has_the_documented_widths_at_both_ends`、
`equal_widths_give_a_straight_band_not_a_cone`、`elbow_outline_turns_at_the_corner`、
`tree_edge_arrow_matches_the_canvas_triangle`、`svg_fills_a_tapered_band_and_draws_the_arrow`。

PDF（2）：`pdf_fills_the_same_tapered_band_and_arrow`、`pdf_applies_the_world_to_page_scale_exactly_once`。

判据全是**关键几何数值**（控制点、两端宽度、三角形长度/底宽、轮廓顶点坐标），不做字节比对。

另：**改了一条既有测试** `labels_become_chips_before_the_text_pass`——它原来在整个 SVG 里搜
`fill="#3a7d6c"`，而树边锥形填充现在也用同一个 RGB，会先命中连线。改成只在 `<g id="nodes">`
里搜。这是测试选择器问题，不是产品行为变化。

## 5. 命令与最后一行

```
cargo check -p mw-shell --all-targets
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.25s

TEMP/TMP=target/tmp  cargo test -p mw-shell -- --test-threads=1
→ test result: ok. 736 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 14.41s
```

## 6. 「画布像素零变化」的论证

1. 画布像素只由 `mw-render`（`instances.rs` / `compositor` / `lattice.wgsl`）+ `mw_core::Scene` 决定；
   本轮两个改动文件都在 `mw-shell` 的**导出路径**里，没有任何一处被画布渲染或 `build_scene` 调用。
2. `SceneEdge` 的字段一个没动（`shape` / `width_end` / `arrow` 早就在），`build_scene` 零改动
   ⇒ 同一文档生成的 `Scene` 逐字段不变 ⇒ golden 截图不变，**不需要重拍任何基线**。
3. 唯一被两条路径共用的新代码是 `export_svg` 里的纯几何函数，它们只被 SVG/PDF 导出调用（编译期可证：
   `tree_edge_*` 的调用点只有 `export_svg.rs` 的树边循环、`export_pdf.rs` 的树边循环与单测）。
4. 反向证据：本轮 `cargo test -p mw-shell` 全绿，其中 `canvas/**`、`golden`、`mw-render` 的画布用例
   一条没改、一条没红。

## 7. 未做的验证（留账）

- `cargo golden --tol 0` 双跑、`stage-b` / `stage-cm` 整门：按要求未跑（避免与并行会话抢锁）。
- PNG↔SVG/PDF 逐像素比（AC-12）：需要离屏 GPU 与栅格化工具，未做。
- 30 分钟前 `mw-core/src/design.rs`、`mw-io/src/cmap.rs` 被并行会话改到不可编译，导致本轮前两次
  `cargo test` 无法判定；收尾时它们已恢复，上面的绿是**在恢复后的工作区**上跑的。
