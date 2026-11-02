# X22 · G04 中英混排度量同源 —— 改码证据（2026-11-02，**已回退，补丁在案**）

- **包**：X22（Stage C 第 2 轮）。Lead 裁决见 `docs/x100/log/decisions-2026-11-02-stage-c.md` §1
  「G04 → 批准修码（X22 动码期第一刀）」。
- **本轮最终状态**：`进行中`，**代码改动已全部撤回，工作树对 `crates/` 零净改动**。
  原因见 §5：修完后 `mw-render` 一张集成基线
  （`shots/golden/canvas-text-ppp1.00.png`）按预期漂移 0.090%，收尾轮硬要求
  「`cargo test -p mw-render` 通过」且禁止跑 golden 重拍 → 按 Lead 的「宁可少做一件，
  不能留半截」把改动整块撤回，**完整补丁留在**
  [`g04-measure-parity-2026-11-02.patch`](g04-measure-parity-2026-11-02.patch)
  （`git apply --check` 已过，可直接重放）。
- **写作用域**：只碰过 `crates/mw-render/src/text.rs`、`crates/mw-render/src/dwrite.rs`
  （现已还原）；`crates/mw-core/**`、`mw-shell/**`、`hit.rs`、`docs/`、`scripts/`、
  其他 `shots/x100/*` 未碰；未 commit。

---

## 1. 补丁里是什么（3 处改动，一个出宽出口）

| # | 文件:位置 | 改动 |
| --- | --- | --- |
| 1 | `crates/mw-render/src/text.rs`（`CJK_LATIN_GAP_EM` 之后） | 新增 `cjk_latin_gap_width(text, font_size)`（= `边界数 × 0.1em × 字号`）与 `line_advance(dwrite_advance, text, font_size)`（= 净行进 + 上述间距）。后者是**唯一出宽函数**。 |
| 2 | `text.rs` `TextEngine::shape_uncached` | 循环里不再自己定宽（原式 `width.max(raw.x + raw.advance + gap_sum)`），只记 `raw_advance`，宽度统一 `let width = line_advance(raw_advance, text, font_size);`；逐字形 `gap_before` 仍管落点。 |
| 3 | `crates/mw-render/src/dwrite.rs` `DWriteMeasurer::width_of` | 签名加 `font_size`，每行 `width.max(crate::text::line_advance(metrics.width, line, font_size))`。于是 `Measured.width`（→ 节点盒宽 / 折行依据）与 `ShapedLine.width`（→ 画面对齐）同源。 |

**没有**动 `mw-core` 的 `Measurer` 接口、折行策略（断行点仍由 DWrite 定）、字形图集、数据模型。

## 2. 补丁里的单测（5 条，名与判据）

| 单测 | 位置 | 钉住什么 |
| --- | --- | --- |
| `text::tests::line_advance_counts_every_cjk_latin_boundary` | `text.rs` | 0 边界原样返回；1/2/3 边界 = `n×0.1em×字号`；字号线性（16→32 间距翻倍） |
| `text::tests::only_mixed_script_lines_gain_width` | `text.rs` | **像素影响面的纯函数论证**：纯中文/纯英文/中文+全角标点/数字/破折号 零加宽；混排行加宽 = 边界数 × 1.6px（16px）且 ≤ 字数 × 0.1em |
| `text::tests::measured_width_equals_shaped_advance` | `text.rs` | **G04 等式**：6 样本 × 4 档字号（15/16/20/28），`measure()` 宽 == `shape()` 宽（≤0.5 css px），且两者都 == `DWrite 净宽 + 混排间距`（≤0.5） |
| `dwrite::tests::measure_adds_cjk_latin_gaps_to_the_dwrite_width` | `dwrite.rs` | 度量契约：`Measured.width` == 净宽 + `0.1em × 边界数`（±0.01）；**零边界盒宽与修复前逐位相同**（`assert_eq!`） |
| `dwrite::tests::weight_faces_are_read_back_from_dwrite` | `dwrite.rs` | G06/G07 一次性探针（打印 `GetFirstMatchingFont` + `GetWeight`，读数见 §4） |

带补丁那一趟的实测（20:2x）：
`cargo test -p mw-render --lib` → **41 passed / 0 failed**（含上面 5 条）。

## 3. 像素影响面（实测，**不因回退而失效**）

规则：只有「汉字 ↔ ASCII 字母数字**紧邻**」才加 `0.1em × 字号`；空格/全角标点分隔的不算
（`crosses_script` 只认 `is_ascii_alphanumeric`，`"Rust 中文"` 边界数 0）。

### 3.1 带补丁时的实测漂移

`crates/mw-render/tests/text_render.rs::text_canvas_matches_golden`（基线
`shots/golden/canvas-text-ppp1.00.png`）：

```
[golden] ✗ 文本画布漂移：484/540000 像素超容差（0.090%），最大差 90，包围盒 x[757..771] y[261..338]
```

该夹具唯一混排文本是节点标题 `"中文Rust混排"`（`text_render.rs:36`）。它是根的子节点 →
`CANVAS_MAIN` 20px，边界 2 个（`文|R`、`t|混`）→ 盒宽 **+2×0.1×20 = +4.0 DIP**；
漂移包围盒 14px 宽（4px 盒边 + 抗锯齿）、77px 高与该节点两行文字相符。
其余 8 条 `text_render` 集成测试（含四档 DPI 的
`text_is_sharp_at_100_125_150_200_percent`）**全绿** → 纯中文场景零漂移。

### 3.2 仓库级扫描：现有场景几乎没有混排节点标题

| 扫描 | 范围 | 结果 |
| --- | --- | --- |
| 节点构造代码 | `crates/**/*.rs` 里 `add_child` / `Document::new` / `SetText` / `AddNode` / `AddConcept` 之行（362 行） | 含混排边界的字面量 **0 条** |
| 默认演示文档 | `crates/mw-shell/src/session.rs:403` `sample_document()`（无文件启动时多数 golden 场景用它） | 中英之间全有空格/标点分隔（`纯 Rust 原生`、`wgpu 四层合成器`、`DirectWrite 度量`、`uniform grid 命中`）→ 边界 **0** |
| `.zmind` 夹具 | 仓库 5 个 `.zmind`（解 ZIP 读 `document.json` 的 `"text"`） | 混排节点文字 **0 条** |

**判断**：像素影响面确实只限「含中英混排节点标题」的盒（+`n×0.1em×字号`），
现有 golden 场景里几乎不存在这种节点；卡片页/库/大纲/检查器走 egui 绘制，不吃
`Measured.width` → **零影响**。用户自写文档里若有「中文Rust」这类无空格混排标题，
盒会变宽——这正是修复目的（以前文字顶出盒、居中偏「间距和/2」）。

## 4. G06/G07 探针读数（一次性，不改映射）

命令（已构建的 lib 测试二进制）：`./target/debug/deps/mw_render-<hash>.exe weight_faces --nocapture`

```
[probe] Segoe UI 请求 400 → 实际面 400
[probe] Segoe UI 请求 500 → 实际面 600
[probe] Segoe UI 请求 600 → 实际面 600
[probe] Segoe UI 请求 700 → 实际面 700
[probe] Microsoft YaHei UI 请求 400 → 实际面 400
[probe] Microsoft YaHei UI 请求 500 → 实际面 400
[probe] Microsoft YaHei UI 请求 600 → 实际面 700
[probe] Microsoft YaHei UI 请求 700 → 实际面 700
[probe] Microsoft YaHei 请求 400 → 实际面 400
[probe] Microsoft YaHei 请求 500 → 实际面 400
[probe] Microsoft YaHei 请求 600 → 实际面 700
[probe] Microsoft YaHei 请求 700 → 实际面 700
```

结论（已写进 `report.md` §7.2）：
1. **画布与内容面在 400/500/600 上落面完全一致**（两边都没有真 500，都靠最近面：
   拉丁 Semibold 600 / 中文 Regular 400，或拉丁 Semibold 600 / 中文 Bold 700）→
   原 §3 G06 推断的「两侧落面不同」**不成立，已删**；真问题只剩「500 不是独立档」。
2. **CJK 600 实测落雅黑 700 面**（`docs/28` §4.1「不用 700」与实现冲突；但它是雅黑族里
   唯一的加粗真面，不是合成粗体）。

## 5. 收尾状态（回退后实测，工作树干净）

```
cargo check -p mw-render --all-targets          → Finished（0 warning / 0 error）
cargo test  -p mw-render -- --test-threads=1    → test result: ok. 9 passed; 0 failed   （最后一个二进制）
                                                  合计 36 + 11 + 4 + 12 + 9 = 72 passed / 0 failed
```
（`TEMP`/`TMP` 均指到 `target/tmp`；`text_canvas_matches_golden` 随回退恢复绿。）

**未做 / 阻塞（诚实记账）**
1. **`cargo clippy -p mw-render --all-targets` 一次都没跑成**：并行会话当时在改 `mw-core`
   （`crates/mw-core/src/design.rs:891` 报 `cannot find type Rgba in this scope`），
   clippy 与中途两次 `cargo test` 都卡在 `mw-core` 编译。→ 本包 **clippy 记 `未开始`**，不是「通过」。
2. **未跑 golden、未重拍任何基线**（Lead 明令，且 `shots/golden/` 不在写作用域）。
3. **`mw-core` 的 `MonoMeasurer` 兜底路径仍不加混排间距**（禁改区）；它是否参与盒宽需另开一刀。
4. `hit.rs` 不需要改（命中盒来自 `Scene` 几何，会随 `Measured.width` 一起变宽）。

## 6. 重放清单（下一棒，3 步）

1. `git apply shots/x100/X22/evidence/g04-measure-parity-2026-11-02.patch`
2. `MW_GOLDEN_UPDATE=1 cargo test -p mw-render -- text_canvas_matches_golden` 重拍
   `shots/golden/canvas-text-ppp1.00.png`，按 `docs/30` §7.2 记旧/新哈希（旧哈希在 git 里，
   本次回退未改该文件）。
3. `cargo test -p mw-render -- --test-threads=1`（应 77 条全绿）+
   `cargo clippy -p mw-render --all-targets`（新增必须 0），结果回填本文件 §5。
