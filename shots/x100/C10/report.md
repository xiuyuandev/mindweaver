# C10 · 「排列」子菜单贴底夹取（打磨轮 4）

> 依据：`docs/x100/polish-ledger.md` §1 C10 行与 §3 第 4 条；CM-10 §3 第 8 条。
> 状态：**贴底夹取第一次有图**（一张基线 + 一行形状轨迹）；实机拖窗口的观感仍 `未验证`。

## 1. 这一轮要解决什么

账本对 C10 的判词是「`place_submenu` 两条单测 / 真贴底时的观感没图」。查下去两半都对了一半：

- **单测那半过期了**：`menu.rs` 现在四条（`submenu_flips_to_the_left…`、`submenu_stays_on_the_right…`、
  `submenu_clamps_up_when_the_parent_row_is_near_the_bottom`、`submenu_clamps_to_the_top…`），
  竖向夹取早有断言；`menu.rs` 里也已经有一条形状轨迹 `menu_submenu_rect row=… pos=… size=… content=…`。
- **图那半确实没有**：全部菜单场景都在 `app_methods_1.rs` 里写死的 `(360,120)` 开菜单，
  贴着左上角，**子菜单永远装得下**，夹取分支在基线上一次都没走过。

所以本轮要补的不是逻辑，是**一次真的溢出**：把菜单请求点挪到矮窗底部，让 300 高的「排列」子菜单
必然装不下，从而把 `place_submenu` 的夹取拍进基线。

## 2. 非目标

- 不改 `place_submenu` / `place_root` 的任何几何常量（`EDGE` = 8、`SUBMENU_OVERLAP` = 2、`PAD` = 6）。
- 不改菜单项与「排列」子菜单的内容（8 行 + 2 个分组标题 = 300 高）。
- 不拍横向翻转（`cm-*` 里已有能翻左的场景，且那条一直有单测）。
- 不改窗口最小尺寸之类的产品设定：矮窗只是验收环境。

## 3. 改了什么（代码）

| 位置 | 改动 |
| --- | --- |
| `canvas.rs::context_pos` / `parse_pos` | 新证据开关 `MW_CONTEXT_POS='x,y'`：只挪右键菜单的**请求点**。`'x,y'` 两个都能解析成有限数才算数，否则当「没设」退回默认点 |
| `app_methods_1.rs`（`MW_CONTEXT_DEMO` 那段） | `pos: canvas::context_pos().unwrap_or(egui::pos2(360.0, 120.0))`；不设变量时**与原来完全一致** |
| `canvas/tests.rs::context_pos_reads_two_numbers_or_nothing` | 单测：`"380, 690"` / `" 12.5 ,7 "` 能读；`"380"`、`"380,690,1"`、`"a,b"`、`"NaN,10"`、`""` 一律 `None` |

夹取逻辑一行没动——这一轮**只加了一把对准底边的镜头**。

## 4. 验收：一张新基线 + 一行形状轨迹

| 项 | 值 |
| --- | --- |
| 场景 | `cm-submenu-bottom` |
| 环境 | `MW_CONCEPT_DEMO=1`、`MW_CONTEXT_DEMO=multi`、`MW_CONTEXT_POS='380,400'`、`MW_WINDOW_SIZE=1280x520`、`--ppp 1.0`、`--warmup 8` |
| 基线 | `shots/golden/cm-submenu-bottom/shell-ppp1.00.png`（1920×780），sha256_12 = **`30CD9669D76A`** |

轨迹（`shots/x100/C10/trace-submenu-bottom.txt`，原行照抄）：

```
event menu_open anchor=380,400 entries=9
event menu_submenu_rect row=387,584,615,614 pos=613,472 size=200,300 content=0,0,1920,780
```

读法：父行 `387,584–615,614`，子菜单 200×300，内容区 `0,0,1920,780`。
不夹取时 `pos.y` 会是 `row.top − PAD = 578`（`PAD` = 6，见单测 `submenu_flips_to_the_left…` 的 94），
`578 + 300 = 878 > 780 − 8`，装不下；夹取后 `pos.y = 780 − 300 − 8 = **472**`，
即**被往上推了 106px**，底边正好留 8px。
横向不受影响：`pos.x = row.right − 2 = 613`。

像素侧：子菜单卡 `613–813 × 472–772`，卡底边在 `y=772`，窗口内边距 8（下一行 773 起是窗口边框）；
末行「垂直等距」整行落在卡内（内容带 `y≈760–769`），没有被切掉。

## 5. 复跑

```bash
cargo build --release -p mw-shell
MW_CONCEPT_DEMO=1 MW_CONTEXT_DEMO=multi MW_CONTEXT_POS='380,400' MW_WINDOW_SIZE=1280x520 \
  MW_TRACE=target/x100/c10.txt \
  ./target/release/mw-shell.exe --golden --dir target/x100/c10 --ppp 1.0 --warmup 8
# 轨迹里找最后一条 menu_submenu_rect：pos.y 必须是 content.h - 300 - 8

# 双门（与本目录数字同源）
python target/x100/gate_cm.py target/release/mw-shell.exe      # 46 场 / 92 次
python target/x100/gate_golden.py target/release/mw-shell.exe  # 87 场 / 174 次
```

本轮实测：测试 **916** passed / 0 failed / 1 ignored（+1：`context_pos_reads_two_numbers_or_nothing`）；
fmt 干净；clippy 37 条基线与当前指纹一致、**新增 0**；
stage-cm **46 场 / 92 次比较**全过（`target/x100/gate-cm-c10.log`），既有 45 张零漂移；
stage-b **87 场 / 174 次比较**全过（`target/x100/gate-b-c10.log`）。

## 6. 未做 / 风险

1. **夹取后的遮挡是既有取舍**：子菜单往上推之后会盖住父菜单下半部分，父行仍能点
   （横向只重叠 2px、指针不在父行中心上方），但「点了父菜单下半部分会先点到子菜单」这件事没有实机验证。
   代码注释与 `menu.rs` 的单测都写着这个取舍，本轮只是第一次有图。
2. **只拍一张**：窗口再矮（例如 520 以下到菜单自身都装不下）时子菜单会被夹到内容顶边，
   这一档仍只有单测 `submenu_clamps_to_the_top_when_the_row_is_above_the_content`。
3. **实机手感未核**：拖动窗口改变高度时菜单会不会跟着重排/闪一下，离屏拍不到。
4. 只拍 ppp 1.0（`content` 的 1280→1920 换算说明该环境下逻辑与物理像素不是 1:1，
   150% / 200% 下夹取后的视觉边距没复核）。
