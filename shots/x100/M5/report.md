# M5 · 拖拽落点指示器（打磨轮 1）

> 依据：`docs/x100/polish-ledger.md` §1 M5 与 §3 第 1 条；实现侧 `canvas.rs::paint_drop_target`、
> `mw-ui::drag::drop_target`（`BEFORE_BELOW=0.28` / `AFTER_ABOVE=0.72` / `DROP_PAD_X=6` / `DROP_PAD_Y=4`）。
> 状态：**三形态与非法态首次有实拍基线**；手感（阈值、吸附、实机拖拽）仍 `未验证`。

## 1. 这一轮要解决什么

账本对 M5 的判词是「落点指示器三形态没有一张实拍」。原文的实现是有的：
整框 2px 描边 + 15% 底 = 成为子级，上缘 3px 条 = 插到它之前，下缘 3px 条 = 插到它之后，
非法目标整块转红。但**四张图一张都没有**，所以任何一次改动都能悄悄改坏它而不被机器发现。

本轮把它变成门里的一等公民：四张基线 + 一份「哪一态、哪个节点、在哪」的文本轨迹。

## 2. 非目标

- 不动拖拽手感：`DRAG_THRESHOLD`（4px）、吸附 60ms 延迟、`SNAP_PX` 都不改。
- 不拍 `Ctrl` 拖拽复制（`drag.copy`）——那是同一条手势的另一个分支，留给下一轮单独立景。
- 不拍概念图：概念图**故意不产生落点**（`m.drop = None`），没有形态可拍。
- 不改 `.zmind` / `Node` 字段，不改拖拽提交逻辑（`plan_moves` 一条命令都没动）。

## 3. 新增的证据工具（`MW_TRACE` 下才工作，关闭时零开销）

像素只能回答「这里变了没有」，回答不了「现在是哪一态、目标是哪个节点」。补三行轨迹：

| 轨迹 | 位置 | 内容 |
| --- | --- | --- |
| `scene_node <id> text="…" rect=(x,y,w,h)` | `canvas_methods_2.rs` show 末尾 | 整张场景的节点几何（屏幕坐标），场景变了才重写一次 |
| `drop_zone node=<id> zone=before\|child\|after illegal=<bool> at=(x,y)` | `DragKind::Move` 分支 | 落点判定，同态不重复写 |
| `canvas_click at=(x,y) … hit=<id\|none>` | 点击处既有那行 | 补上坐标与命中节点 |

为什么要 `scene_node`：先前试过用 100px 点击网格反推节点位置，命中会被概要/边界吞掉
（`owner_node` 把注解算到主人头上），160 个点的结果是一张对不上的散点图；
直接导出几何是十几行代码的事，且 M1/M2（新建后的编辑框位置）与 M7（框选阈值）都要用。

## 4. 四张基线

同一份示例文档（`Session::fresh` 的「产品路线图」示例），同一个源节点「平移缩放」，
脚本以「按住不放」结束，所以截图时幽灵与落点都还在画面上。

| 场景 | 脚本 | 落点 | 机器判据（`--tol 0`） |
| --- | --- | --- | --- |
| `mm-drag-child` | `press 463 605` → `move 483 605` → `move 1027 631` | 「导出」整框 | 强调色 (47,111,94) 描边矩形 `(989,609)-(1065,652)` 共 **416 px**；框内 10–15% 底色；trace `drop_zone … zone=child illegal=false` |
| `mm-drag-before` | 同上，落点 `1027 613` | 「导出」上缘 | 3px 条 `x 988..1066 · y 606..608` 共 **233 px**；trace `zone=before` |
| `mm-drag-after` | 同上，落点 `1027 649` | 「导出」下缘 | 3px 条 `x 988..1066 · y 653..655` 共 **233 px**；trace `zone=after` |
| `mm-drag-illegal` | `press 698 473`（纯 Rust 原生）→ `move 496 446`（自己的子级 winit） | 自己子树 | 红色 `DROP_BAD=(224,75,75)`：目标 2px 描边 `(425,429)-(566,463)` + 幽灵虚线框；trace `zone=child illegal=true` |

三张合法图**逐像素只差那条指示**：目标节点位置、幽灵位置、其余节点全部相同
（`before` 与 `after` 的差别只有 y 606..608 与 653..655 这两条 3px 带）。

## 5. 命令

```bash
cargo build --release -p mw-shell
MW_GOLDEN_INPUT=shots/golden/inputs/mm-drag-child.txt ./target/release/mw-shell.exe \
  --golden --update --warmup 20 --ppp 1.0 --dir shots/golden/mm-drag-child
# 其余三张同上；随后双跑 --tol 0 全部零差异
python -I target/x100/gate_golden.py target/x100/app-m5.exe   # 门全量（83 场 / 166 次比较）
```

## 6. 门登记

`scripts/x100-stage-b-gate.ps1` 末尾新增四场（`Warmup = 20`，`Ppp = 1.0`，`Env` 只有 `MW_GOLDEN_INPUT`）。
stage-b 门 79 场 / 158 次比较 → **83 场 / 166 次比较**。

基线 `sha256_12`：

| 场景 | sha256_12 |
| --- | --- |
| `mm-drag-child` | `511DCB5A9226` |
| `mm-drag-before` | `A036F69B055F` |
| `mm-drag-after` | `B11768F986D4` |
| `mm-drag-illegal` | `05257DA1BA26` |

## 7. 没做 / 风险

1. **手感仍无证据**：4px 阈值、60ms 吸附延迟、贴边自动滚动都只有单测，真人拖拽没人量过
   （要实机 + 鼠标轨迹，不是金样能回答的）。
2. **只拍 ppp 1.0**：条宽 3px、描边 2px 是屏幕空间常量，150% / 200% 下应该是
   「CSS 不变、设备像素变粗」，但没拍；这是最容易在换 DPI 时回归的一处。
3. **吸附引导线（`snap.guide_x/y`）没入景**：本轮的落点都是非吸附位置；引导线与落点同帧
   出现时的层级关系尚未有图。
4. **`Ctrl` 拖拽复制、框选、套索**仍未立景（账本 M7 与 M5 的 copy 分支）。
5. 轨迹里的 `drop_zone` 只在 `MW_TRACE` 下设了；门只跑金样，所以门的通过**不依赖**它，
   它只作为人工复核时的读数。

## 8. 变更记录

- 2026-10-29：建立。新增 4 张基线、4 个输入脚本、3 行 `MW_TRACE` 轨迹；门 83 场 / 166 次比较。
