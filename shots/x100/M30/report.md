# M30 · 画布缩略卡（小地图）接回画布

> 打磨轮 55 · 2026-11-01
> 账本：`docs/x100/polish-ledger.md`（新增 M30 行）
> 合同：`docs/02` §4「小地图」（P1）、`docs/06` §4「右下 190×130；视口框可拖；
> >5k 节点自动降采样；可折叠为 34×26 角标」、`docs/04` 核心纪律 3（Scene 单一来源）

## 1. 为什么是这一轮

`docs/02` §4 把「小地图」列为 P1：右下角缩略导航图、显示全图与视口框、拖动视口框定位、
可折叠、大文档降采样。纯几何层 `crates/mw-ui/src/overview.rs`（608 行，含 20 余条单测）
与壳层布局 `canvas_chrome::minimap_layout` / `minimap_wheel_factor` 都在，`Shell` 也已经有
`show_minimap` / `minimap_collapsed` 两个字段与 `Canvas::scene()` / `pan_to` /
`zoom_around_world` 三个只读/取景入口——**但没有任何一处绘制或交互**：

- 全仓库除定义处外没有 `minimap_layout` 的调用点；
- `show_minimap` 出厂 `false` 且没有开关入口；
- 构造 `Shell` 时两个字段根本没有初始化（工作树当时编译失败，`E0063 missing field`）。

结论：这是 P1 规格在视觉 v2 重写时丢掉的一整块功能，不是「换肤」能盖住的细节。

## 2. 范围

- 把缩略卡接回中央画布：右下角、缩放条上方 8px、190×130、可折叠为 34×26 角标。
- 画：卡片底板/边框/阴影、节点代理块（按分支色）、关系线代理、当前视口框。
- 交互：卡内点击/拖动 → 相机中心；悬停滚轮 → 一档缩放；`–` 折叠/角标点击展开。
- 入口：命令面板「显示/隐藏小地图」、顶栏菜单同名项；状态落 `window.json`（出厂关）。
- 验收旋钮：`MW_MINIMAP=1` 强制显示（金样用）、`MW_MINIMAP_COLLAPSED=1` 折叠态。
- 命中区：折叠按钮 18 → **24 CSS px**（`docs/13` §4.19 记着的「17×17 偏小」旧账），
  视觉图标仍画 12px，不让按钮变大后压住地图内容。

## 3. 非目标

- 不做鹰眼之外的导航功能（双击定位、拖拽节点、缩略图生成 `thumbnail.png`）。
- 不改 `Scene` / `mw-render::Frame` ABI：缩略卡只读 `Scene`，绘制在 egui 覆盖层。
- 不改 `.zmind`、`Node`、`NoteCard` 字段。
- 不恢复旧版「分支色块 + 根描边 + 选中环」的全部细节；本轮只做视口框 + 代理块 + 关系线。

## 4. 验收场景

1. **展开态**：`mm-minimap` 基线（`MW_MINIMAP=1`）。右下角 190×130 卡片、缩放条上方 8px，
   卡内能看到节点代理块与视口框；卡片不压工具条/面包屑。
2. **折叠态**：`mm-minimap-collapsed` 基线（`MW_MINIMAP=1` + `MW_MINIMAP_COLLAPSED=1`）。
   34×26 角标与展开态同右对齐、同底边。
3. **输入门禁**：点卡片不会同时触发画布框选/清空（卡片矩形进 `Canvas::set_overlays`）。
4. **纯函数**：`minimap_layout` / `minimap_wheel_factor` 既有单测保持；
   新增折叠按钮 ≥24px 的断言。
5. **默认零漂移**：出厂关 ⇒ 既有 140 场基线逐像素不变（除新增两场）。

## 5. 预计证据

- `shots/golden/mm-minimap/`、`shots/golden/mm-minimap-collapsed/` 两张基线，`--tol 0` 连跑两遍。
- `shots/x100/M30/report.md`（本文）+ `manifest.json`。
- `stage-b` 门 140 场 / 280 次 → 142 场 / 284 次。
- `MW_TRACE` 轨迹：`minimap toggle on=1` / `minimap pan center=…` / `minimap zoom factor=…`。

## 6. 实测结果（2026-11-01）

- **落地改动**：
  - `crates/mw-shell/src/main.rs`：`minimap_panel()`（绘制 + 折叠按钮 + 卡内点击/拖动/滚轮），
    中央面板把 `card` 提前登记进 `Canvas::set_overlays` 指针闸门；`zoom_control` 顶部
    「它替代了旧的小地图」的过时注释删除。
  - `crates/mw-shell/src/app_methods_1.rs`：`Shell` 构造补上缺失的字段初始化
    （`E0063`——此前工作树编译不过），`MW_MINIMAP` / `MW_MINIMAP_COLLAPSED` 旋钮。
  - `crates/mw-shell/src/app_methods_2.rs`：`toggle_minimap()`（开合落盘 `window.json`、
    展开态复位折叠、不抢画布焦点）；命令面板 `Action::ToggleMinimap` 落地。
  - `crates/mw-shell/src/panels/command.rs`：命令面板新增「显示/隐藏小地图」。
  - `crates/mw-shell/src/app_helpers.rs` / `app_methods_3.rs`：顶栏「视图」菜单补同名项
    （勾选态跟随 `show_minimap`，与大纲/检查器同一张菜单）。
  - `crates/mw-shell/src/canvas_chrome.rs`：`MAP_BTN` 18 → **24**（`docs/13` §4.19 记的
    「17×17 太小」旧账），`MAP_BTN_INSET` 3 → 4；单测补「命中区 ≥24px」断言。
  - `crates/mw-shell/src/canvas/consts.rs`：**修掉上一会话留下的锚点缩放 bug**——
    `zoom_around_world` 把 `center' = at − (at − center)/k` 写成了乘 `k`
    （锚点该钉住的世界点反而跟着相机反向漂），门内单测
    `zooming_around_a_world_point_pins_it_on_screen` 咬住；
    顺带把该测试里「0.005 再缩一档必须动」的错误期望改回
    「下限之下原地不动（返回 false）」，与 `zoom_at_anchor` 的既有测试同一口径。
    该函数只有本包的缩略卡滚轮一条调用链，不影响任何既有基线。
  - `scripts/x100-stage-b-gate.ps1`：门清单 +2 场（142 场 / 284 次比较）。
- **像素账**（1920×1200 @ ppp 1.0，对照帧 = 同窗口不开缩略卡）：
  - 展开态 ↔ 对照：**27 080 px**，bbox `(1710,988)–(1907,1125)` —— 198×138 ≈ 190×130 卡片
    加 4px 阴影/边框，**只在右下角缩略卡区域**；卡外 0 px。
  - 折叠态 ↔ 对照：**1 184 px**，bbox `(1866,1092)–(1907,1125)` —— 42×34 ≈ 34×26 角标加边框。
  - 展开 ↔ 折叠：**26 482 px**，与展开态 bbox 相同（角标之外的部分逐像素一致）。
- **确定性**：两张基线各连跑两遍 `--tol 0` 全过（`[golden] ✓ 无超容差差异` ×4）。
- **轨迹**：`minimap toggle collapsed=0/1`、`minimap toggle on=…`（面板开关）已埋；
  `minimap pan center=…` / `minimap zoom factor=…` 在卡内交互时打出（离屏基线不触发 hover，
  故金样只锁静态两态，交互判据走轨迹 + `overview` 纯函数单测）。

## 7. 门内抓到的环境红灯（与缩略卡无关，一并修掉）

第一遍 stage-b 门在 `mm-cut-confirm` / `mm-cut-cancel` 上红（3.012% / 0.924%），
重跑逐像素定位到「启动即弹出**恢复点**提示条、点选动作被浮层挡掉」：夹具目录
`tests/fixtures/x100/.autosave/` 里残留一份**比正本新**的自动保存（01:07–01:10 的
上一会话遗留，gitignore 内的运行产物），`autosave::pending_for` 按文档路径找，
金样一开这份夹具就先问恢复，确认框没拍上。

修法两处：
- 删除残留（`.autosave/`、`tiny-cn.zmind.lock`），`.gitignore` 补 `*.zmind.lock`；
- **金样不再读文档旁的恢复点**：`App` 初始化里 `recovery` 在 `harness` 下恒为 `None`
  ——夹具旁的运行产物从此不再影响门（与 `WindowState::disabled()`、
  「金样不读 recent.json」同一层隔离纪律）。
  两个场景用新二进制复跑 `--tol 0` 全过。

## 8. 未做 / 风险

- 双击缩略卡定位某节点（`docs/13` §4.19 提过的 `reveal_center` 路径）——延后。
- 选中环 / 根描边 / 卡片角标在缩略卡上的投影——延后（本轮只做视口框 + 代理块 + 关系线）。
- 实机 150%/200% DPI 的观感与拖动手感没量（离屏只有 ppp 1.0）；触控板连续滚动
  被钳成一档一档（`minimap_wheel_factor` 的既有语义），触控笔未测。
- 概念图与思维导图共用同一张缩略卡：`Scene::bounds` 含聚焦问题带（`focus_note`）时
  上边会被一起装进卡内——概念图金样未单独拍，G-CM 门打开时一并复核。
