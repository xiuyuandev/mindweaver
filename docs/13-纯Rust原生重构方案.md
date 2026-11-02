# 13 · 纯 Rust 原生重构方案（取代路线 A）

> 状态：**历史冻结（2026-10-30 文档整顿起不再追加）** · 最后更新：2026-10-19（v7 后、仓库清理轮） · 上游：03、04、07、08 · 下游：04（现行架构）
>
> **先读哪里**：想接手开发 → 根目录 `HANDOFF.md`；想知道当前做了什么/没做什么 → `docs/STATUS.md`；本文档是「为什么这样做」的决策与逐轮实现记录，§4.x 按时间追加，**早期小节的数字（测试数、体积、性能）已被后续轮次覆盖，以 `docs/STATUS.md` 与 `HANDOFF.md` 为准**。
>
> **冻结原因**：本文记的是 v3–v7 时代（2026-10-16 – 10-19）的 ADR 与逐轮实现。X100 之后逐轮现场统一写在 `docs/x100/log/`，架构现状看 `docs/04`。本文仍被十余处代码注释按「§N」引用，故不删除、不搬家、只就地冻结。
>
> **关于 docs/12**：本文多处引用的「docs/12」指已删除的 WebView 版《画布与编辑体验重构方案》；其诊断（D1–D10）与规格仍作为历史需求来源保留在引用中，文件本身已由 `docs/archive/12-历史注记-WebView路线.md` 替代（墓碑 + 废弃原因），**不要按旧 docs/12 实现 WebView**。
>
> 一句话：**「原生」是硬约束而非体验目标——视图层彻底脱离 Web 技术，用纯 Rust 重写；`mw-core` / `mw-format` / `mw-io` 全部保留并继续作为唯一数据权威。**
>
> 本文档记 **ADR-010：纯 Rust 原生 UI 重写**，取代旧 docs/12 §2 的「路线 A：壳内换 Canvas 2D 引擎」。旧 docs/12 的诊断（D1–D10）与规格（§3.1 渲染分层、§3.2 交互状态机、§3.3 壳层令牌、§3.4 编辑缺口 E1–E12）**全部继续有效**，仅「在 WebView 内实现」这一前提作废。
>
> 编号说明：ADR-001…009 已在 docs/04 §ADR 登记表占用（其中 007=UUID v7、008=快照式自动保存、009=zh-CN 源语言），本文档初版误用 007/008 指代「路线 A/B」，已于 2026-10-16 更正。

---

## 1. 为什么改路线

docs/12 的结论是「WebView2 不是原罪，Xmind 本身就是 Electron」——技术上成立。但它的前提是**「原生」是体验目标**。经与产品负责人确认，前提不成立：

| 判据 | 结论 |
| --- | --- |
| 产品定位 | README 第一卖点即「Rust 原生」；开源叙事的核心是「纯 Rust 思维导图」，壳里塞 Chromium 与该叙事冲突 |
| 硬约束 | **不允许任何 Web 技术**（含 WebView2/Svelte/TS/CSS），非「效果达标即可」 |
| 质感对标 | 画布手感对标 Sketch（原生 GPU 合成），不是「像 XMind 那样能用就行」 |
| 投入意愿 | 单人全职，可投 3–6 个月，接受到现有功能 parity 的完整重写工期 |

**同时确认：中文四痛点全部真实存在**——IME 候选窗错位/丢字、CJK 换行禁则错误、文字发虚/字体回退不对、拼音搜索不准。这四项在本文档 §5 逐项归属并列为验收项。

**放弃路线 A 的代价**：`ui/`（7.4k 行 Svelte/TS）与基于 CDP 的冒烟脚本作废；docs/12 P2/P3 已落地的部分（`snap.ts` 吸附、`motion.ts` 动效、`ctxmenu.ts` 原生菜单、`invScale` 屏幕恒定铬件）**逻辑作废、规格保留**，在 Rust 侧重新实现。

---

## 2. 保留 / 废弃清单

| 资产 | 处置 | 理由 |
| --- | --- | --- |
| `crates/mw-core`（模型/26 命令/ChangeSet/撤销/校验/检索） | **原样保留** | 与 UI 无关，是产品最硬的资产 |
| `crates/mw-format`（`.zmind` 容器/原子保存/备份/防穿越） | **原样保留** | 同上 |
| `crates/mw-io`（MD/OPML/FreeMind/XMind 导入，多格式导出） | **原样保留** | 同上 |
| `ui/src/lib/scene.ts` + `layout.ts`（纯函数场景图/三种布局） | **移植为 Rust，并入 core** | 布局必须成为渲染与导出的唯一排版权威（ADR-003 精神不变，实现语言变） |
| `ui/src/lib/measure.ts`（折行/禁则） | **规格保留，实现换 DirectWrite** | §5 |
| `ui/` 其余（Svelte 组件、CSS、store、IPC 客户端） | **废弃** | WebView 专属 |
| Tauri 壳（`mw-app` 的 IPC/窗口/对话框） | **重写为 Win32 壳**；其会话/标签/最近文件/恢复槽逻辑**移植** | 逻辑正确且已测，载体作废 |
| `scripts/{ui,feature}-smoke.mjs`（CDP 驱动） | **作废**，用例清单转为 §7 的新验收体系 | 无 WebView 即无 CDP |
| docs/01–10 产品与交互规格 | **保留**（03 技术选型、04 架构分工需按本文档修订） | 与实现语言无关 |
| docs/12 诊断 D1–D10、缺口 E1–E12、§3.1–3.3 规格 | **保留为需求来源** | 诊断仍然成立 |

---

## 3. 目标架构

```
┌──────────────────────────────────────────────────────────┐
│ 壳层（Win32 原生）                                          │
│  合并式标题栏+标签（Mica/DWM backdrop）· 原生菜单 ·          │
│  IFileDialog · 文件关联 · 单实例 · 跳转列表 · PMv2 DPI      │
├──────────────────────────────────────────────────────────┤
│ UI 框架层（待 M0 定案：egui / gpui-kit / 自绘）              │
│  面板：大纲 · 检查器 · 命令面板 · 搜索 · 小地图 · 主题 · 导出  │
├──────────────────────────────────────────────────────────┤
│ 交互引擎（状态机 + 手势）                                    │
│  Idle→Pressed→(Drag|Marquee|Pan|Connect)→Drop             │
│  拖拽幽灵跟手 · 对齐吸附 · 拖出连线 · 框选 · 动效插值          │
├──────────────────────────────────────────────────────────┤
│ 渲染引擎（wgpu 自绘合成器，DPI 感知）                         │
│  4 层：背景点阵 / 边界+连线 / 节点 / 覆盖层                  │
│  屏幕空间恒定铬件 · uniform grid 命中 · LOD · 帧预算监控      │
│  文本：DirectWrite 排版 → 字形图集 → wgpu                    │
├──────────────────────────────────────────────────────────┤
│ 领域核心（Rust，已存在，不动）                                │
│  mw-core（模型/命令/撤销/检索）· mw-format · mw-io           │
└──────────────────────────────────────────────────────────┘
```

**进程内直连**：不再有 IPC 序列化往返，UI 直接 `&mut Document`。撤销栈、脏标记、自动保存调度改为同栈函数调用；`ApplyResult` 那套前后端镜像协议（docs/11 §2.1 B2 的根因）**整体消失**——这是换路线顺带修掉的一类 bug。

---

## 4. 技术选型

| 层 | 选型 | 依据 |
| --- | --- | --- |
| 窗口/事件 | `winit` + `windows` (windows-rs) | winit 提供跨后端事件循环与 **`WindowEvent::Ime` + `set_ime_position`**（§5 关键）；windows-rs 补 Mica、原生菜单、`IFileDialog`、跳转列表、DPI 感知 |
| 渲染 | `wgpu` | GPU 合成，5k 节点平移缩放有余量；分层与 LOD 按 docs/12 §3.1 规格实现 |
| 文本排版 | **DirectWrite**（`IDWriteTextLayout` + `SetLocaleName("zh-Hans")`） | 禁则、CJK 换行、中英混排字体回退、ClearType 级清晰度**全部由系统提供**；Zed 在 Windows 上即此路线，可行性有先例 |
| 文本光栅 | DirectWrite glyph run → 字形图集 → wgpu | 与渲染器解耦，支持增量重排 |
| IME | winit `Ime::Preedit` / `Ime::Commit` + 组合期语义键守卫 | 候选窗定位到节点内光标；组合期间 `Tab/Enter/Del` 不穿透（docs/07 §3 规则原样移植） |
| 导出 | `resvg`（SVG/PNG）+ `printpdf`（PDF） | 从 Rust 侧 Scene 出，与画布同源，保证 WYSIWYG；顺带补 E4 PDF 缺口 |
| 壳层令牌 | 复用 docs/12 §3.3 设计令牌表（圆角 4/8/12、1px 边框、2px accent 焦点环、WinUI elevation 三档） | 质感靠纪律不靠框架 |

### 4.1 唯一未定项：UI 框架

> **2026-10-16 更正**：本表初版写「egui 无禁则、无 ClearType，痛点 2/3 需自建」——**前半句是错的**。
> 读 0.37 源码 + 实测截图后确认：禁则与系统字体回退 egui 0.37 已自带（见下表与 §4.2）。
> 真正剩下的差异只有 **ClearType 子像素** 与 **API 稳定性**。

| 候选 | 优势 | 风险 |
| --- | --- | --- |
| **egui 0.37**（+`egui-wgpu`） | 上手最快；**CJK 禁则已在排版引擎内实现**（`epaint::text::text_layout` 调 `is_cjk_break_allowed`）；**系统字体回退已接 DirectWrite**（`fontique` + `harfrust` shaping + `egui_system_font_provider`）；**IME 候选窗定位已接**（`egui-winit` 调 `set_ime_cursor_area`，并处理 `should_interrupt_composition`）；即时模式与自绘画布天然契合 | 字形为 alpha 覆盖，**无 ClearType 子像素**（C3 唯一残留项）；0.37 布局系统大改（`App::update`→`App::ui`、`TopBottomPanel/SidePanel`→`Panel`、`RectTransform` 移除）——**小版本即破坏 API**，长期依赖需锁版本 |
| **gpui-kit**（Zed 的 GPUI 之上，75+ 组件） | Windows 文本即 DirectWrite，C3 白拿；保留模式 + 组件齐全 | 以 `gpui-pre-*` 快照发布，API 未稳定，社区小，许可为 Apache-2.0 但含 Zed 声明 |
| 自绘（wgpu + 自研控件） | 上限最高、无第三方风险；C3 可自己做 ClearType/灰度抗锯齿策略 | 面板/列表/输入框全部自己造，3–6 个月预算内做不完 |

**决策方式**：不靠博客横评，用 **M0 探针实测**（§6）后定案。先验倾向已由「egui 文本质量是否过关」收窄为「**egui 无 ClearType 在 150%/200% DPI 下是否肉眼可辨**」——若不可辨 → 选 egui（速度优先）；若可辨且不可接受 → 在 gpui-kit 与「egui + 自绘文本层」之间再评。

### 4.2 M0 egui 侧实测数据（2026-10-16）

探针：`crates/mw-probe/src/bin/probe_egui.rs`，`cargo run -p mw-probe --bin probe_egui --release`。
截图脚本：`scripts/m0-capture.ps1`（窗口位图抓取 + 区域均值采样）；性能矩阵：`scripts/m0-perf.sh`。
环境：Win11 26200，主屏 **pixels_per_point = 1.50（150% DPI）**。

| 项 | 结论 | 证据 |
| --- | --- | --- |
| **Mica 材质** | ✅ **可作用于 eframe/wgpu 窗口** | `ViewportBuilder::with_transparent(true)` + `App::clear_color` 返回 `[0,0,0,0]` + `DwmSetWindowAttribute(DWMWA_SYSTEMBACKDROP_TYPE, DWMSBT_MAINWINDOW)`。HWND 直接由 `eframe::Frame`（实现 `HasWindowHandle`）取得，无需前台窗口兜底。同区域采样对照：不透明清除 = `RGB(237,240,245)`（与请求值逐位一致），透明清除 = `RGB(243,244,235)`（暖色壁纸染色）→ 背景确从交换链 alpha 透出 |
| **CJK 禁则（C2）** | ✅ **egui 自带，无需自建** | 240px 宽容器内折行 6 行，行首字为「织/源/它/等/确/会」，「，。、：；？！」无一落行首 |
| **字体回退（C3 之一）** | ✅ 无豆腐块，中英混排基线一致 | 14px 逻辑字号在 150% DPI 截图下字形与系统一致，「——」「→」正常 |
| **ClearType（C3 之二）** | ✅ **不构成差距** | 像素法判定：取字形暗像素统计 `mean|R−B|`，egui 文本区 = **0.00**（纯灰度抗锯齿）。对照组：**Win11 自己的 DWM 非客户区标题文字 = 2.0**，同样是灰度抗锯齿而非子像素。即本机原生壳层本身不用 ClearType，egui 不落后于原生基线。C3 的「发虚」根因是 WebView 里 SVG 文字在分数缩放下的位图重采样，与子像素无关 |
| **IME 候选窗（C1）** | ✅ 管线存在，⚠️ 待 5 款输入法人工签字 | `egui-winit` 0.37 由 `IMEOutput{purpose, rect, cursor_rect, should_interrupt_composition}` 驱动 `set_ime_cursor_area`；探针提供「输入框左偏移」滑杆以人工验证跟随 |

**性能矩阵**（连续正弦平移 8 s，`scripts/m0-perf.sh`，`ui` = `App::ui` 的 CPU 时间，`build` = 场景构建，`fps` 为墙钟；150% DPI、vsync 开）：

| 配置 | 可见节点 | `ui` | `build` | fps |
| --- | --- | --- | --- | --- |
| 5k 总 · 文字 · 剔除 · s1.0 | ≈900 | 0.33 ms | 0.21 ms | **60** |
| 20k 总 · 文字 · 剔除 · s1.0 | ≈3 660 | 1.01 ms | 0.87 ms | **60** |
| 20k 总 · 纯框 · 剔除 · s1.0 | ≈3 660 | 0.33 ms | 0.20 ms | **60** |
| 20k 总 · 文字 · **无**剔除 · s1.0 | 20 000（多在屏外被裁剪） | 4.50 ms | 4.37 ms | **60** |
| 20k 总 · 文字 · 剔除 · s0.15 | ≈12 800–16 200 | 3.5–4.1 ms | 3.4–3.9 ms | **16–18** |
| 20k 总 · 文字不随缩放 · 剔除 · s0.15 | ≈12 800–16 000 | 3.9–4.0 ms | 3.7–3.9 ms | **15–18** |

**读法（重要，别把探针当目标架构）**：

1. 探针走的是 egui 最朴素的路径——每节点 2 个 `Shape` + 1 次 `painter.text`，**不是** §3 计划的实例化合成。所以这张表是**下界**。
2. 最后一行说明：帧率崩塌与字号无关（缩放字号 vs 恒定字号同为 ~16 fps），**瓶颈是「同屏图元数量」，不是文本光栅**。
3. s0.15 那两行 `ui` 只有 4 ms 却掉到 16 fps → 超出部分全在 `App::ui` 之外（tessellate + GPU 提交 + 海量重叠小矩形的 overdraw）。**只优化 CPU 侧布局救不了它**，必须做 §3 的分层实例化 + LOD。
4. 结论：**真实文档规模（≤5k 节点、同屏 ≤4k）在 egui 朴素路径下已稳 60 fps**；要撑到「缩到全局仍不卡」，靠的是 M1 的 LOD（低于阈值只画块不画字）与实例化，而非换框架。

### 4.3 ADR-011：UI 框架 = egui 0.37（提案，待 IME 矩阵签字）

四项 M0 待定项的实测结果全部倒向 egui：Mica 可用、禁则自带、字体回退走 DirectWrite、IME 定位管线已在、真实规模帧率有余、且「无 ClearType」这条原本最大的扣分项经对照证明并不落后于 Win11 自身壳层。

**定案**：壳层与控件用 **egui 0.37**（锁版本，`Cargo.lock` 固化）；画布不用 egui 的逐节点 `Shape`，而按 §3 自绘 wgpu 分层实例化，egui 仅承担面板/菜单/对话框/覆盖层。gpui-kit 降级为「若 IME 矩阵不通过」的备胎，不再单独做探针——因为分叉理由（DirectWrite 文本）已被上表证伪其必要性。

**残留风险与对策**：

| 风险 | 对策 |
| --- | --- |
| 0.37 布局系统大改（`App::update`→`App::ui`、`TopBottomPanel/SidePanel`→`Panel`、`RectTransform` 移除）说明**小版本即破坏 API** | 锁 `=0.37.0` 精确版本；升级须走一次独立 PR 并跑 §7 快照 |
| C1 的 5 款输入法人工矩阵尚未签字（本环境无法自动化 IME） | M0 剩余工作；不通过则回看 gpui-kit 或自绘文本层 |
| 200%/深色底下的文本观感未采 | M0 内补拍；判定阈值同 §4.2 像素法 |

### 4.4 M0 壳层骨架实测（2026-10-16）

`crates/mw-shell` = 无框 eframe 窗口 + Mica + 合并标题栏 + 自绘窗口按钮 + 8 条缩发热区。1 台机器、150% DPI（1920×1200 物理 / 1280×800 逻辑）、release 构建，用 `scripts/m0-interact.ps1` **注入真实鼠标输入**逐项验证（不是单元调用，是走完整的 OS 模态循环）：

| 项 | 手法 | 结果 |
| --- | --- | --- |
| Mica 透出 | 标题栏区域取色 | `242,246,229`（带桌面色偏），非不透明底 ✅ |
| 圆角 | `DWMWA_WINDOW_CORNER_PREFERENCE=DWMWCP_ROUND` | 返回 `Ok(())`，截图四角为圆 ✅ |
| 右缘缩放 | 东热区按下横拖 120 逻辑像素 | `dw=+123`，`dl=0`（左缘纹丝不动）✅ |
| 角缩放 | 东南热区斜拖 | `dw=-92 dh=-72` ✅ |
| 标题栏拖动 | 栏内下 20px 处按下拖 140×90 | `dx=129 dy=83`，尺寸不变 ✅ |
| 双击最大化/还原 | 标题栏空白处双击 | `zoomed=True` → 再双击 → `zoomed=False` 且精确回到 900×600 ✅ |
| 关闭键悬停 | 指针移到关闭按钮取整块均值 | `196,53,39`（Win11 红 `C4,2B,1C` + 白字形）✅ |
| 最大/最小键悬停 | 同上 | `229,228,227` 中性灰 ✅ |
| 最小化 | 点击最小化键 | `IsIconic=True` ✅ |

**四条踩过的坑，M3 做完整壳层时会再遇到，先记在这**：

1. **`Sense::drag()` 下 `double_clicked()` 恒为 false**——`Response::double_clicked` 要求 `Flags::CLICKED`，而该位只在 `Sense::CLICK` 时置位。标题栏必须用 `Sense::click_and_drag()`。
2. **拖拽区不能盖住按钮条**：整条标题栏注册命中区时，双击窗口按钮会连带触发「双击最大化」。把拖拽区右界收 `3 × BTN_W`。
3. **DWM 圆角遮罩会吃掉最外圈约 5 物理像素的输入**——角热区若贴着 (w−3, h−3) 采样，点击落到桌面上，窗口毫无反应。热区内缩到 8 逻辑像素后稳定命中。
4. **无框窗口最大化后，窗口矩形比客户区每边大出约 7 逻辑像素**（Win11 的隐形边框仍在），所以 `GetWindowRect` 与 egui 的点坐标系对不上。任何自动化都必须用 `GetClientRect + ClientToScreen`，且驱动进程要显式 `SetProcessDPIAware`，否则 `CopyFromScreen` 用物理坐标而 `ClientToScreen` 用虚拟化坐标，取色会整体偏移一个 DPI 倍数的距离。

另外 `ViewportCommand::StartDrag` 在 egui-winit 里被 `window.has_focus()` 挡着（`lib.rs:1961`），而 `BeginResize` 没有这道闸——所以「未获焦时拖不动标题栏」是框架行为，不是我们的 bug。

### 4.5 M0 验收 harness：离屏渲染 + golden image（2026-10-16）

`mw-shell --golden` 把壳层渲染成一张图，与 `shots/golden/shell-ppp{缩放}.png` 逐像素比对。这就是 ADR-010 冻结 `ui-smoke.mjs` / `feature-smoke.mjs` 之后欠下的那块视觉验收基建，§7 的第三行由它落地。

机制上有三件事决定这套东西能不能信：

| 项 | 结论 | 依据 |
| --- | --- | --- |
| 像素来源 | `egui::Context::request_screenshot` → `ViewportCommand::Screenshot` → egui-wgpu 的 `CaptureState`：帧先画到一张自有纹理，再 copy 进 CPU 可读缓冲 | `egui-wgpu-0.37.0/src/capture.rs` |
| **真离屏** | 隐藏窗口拿不到交换链纹理也照样能截：`capture` 为真时即使 `output_frame = None` 仍走完整渲染路径；eframe 的 `is_visible` 把 `Screenshot` 计入 `wants_paint` | `winit.rs:603-634`、`wgpu_integration.rs:686` |
| 与桌面无关 | Mica 由 DWM 在窗口背后合成，回读的是我们自己那层 → 该区域恒为 alpha=0。壁纸、窗口遮挡、任务栏位置全部不进变量 | 实测 |

基线文件名带 `pixels_per_point`（`shell-ppp1.50.png`）：缩放是显示器属性，跨档次的像素不可能相同，150% / 200% 各存各的——§4.3 残留风险里的「200% 补拍」因此只是换档重跑一遍。默认容差 0（单通道）：驱动或 wgpu 版本变动本该被看见，而不是被容差吃掉。

实测（本机 150%、debug 构建）：

| 项 | 结果 |
| --- | --- |
| 首条基线 | `--golden --update` → 回读 **1620×1080 @ ppp 1.50**，隐藏窗口一次成功，无需 `--shown` |
| 稳定性 | 连跑 3 次 `--golden`，**容差 0 下逐像素一致**，退出码 0 |
| 灵敏度（负向测试） | 副标题 `from_gray(110)` 改成 `111` → `2976/1749600 像素超容差（0.170%），最大差 1，包围盒 x[554..1065] y[195..216]`，退出码 1；包围盒正是副标题那一行 |
| 差异图 | `shots/golden/diff/*.png`：超容差像素涂纯红，其余预乘→白底合成。肉眼看即知是「字形变了」还是「整块位移」 |
| 回退 | 改回 110 → 再次容差 0 一致（证明上面那条不是抖动） |
| 其它退出路径 | `--tol 1` 吞掉 1 级噪声 → 0；基线缺失 → 2 并提示用 `--update`；未知参数 → 打印用法 + 2 |
| harness 自身 | `cargo test -p mw-shell` 5 项纯逻辑单测：比对、容差、包围盒、差异图合成 |
| 交互壳层未回归 | §4.4 那九项重跑，数字逐条一致 |

**一个不明显的坑**：`fn main() -> eframe::Result<i32>` 编不过——`i32` 没实现 `Termination`（只有 `ExitCode` 实现了）。退出码得经 `Arc<AtomicI32>` 从被 `Box` 吃掉、再也取不回来的 App 里捎回 `main`。

**边界（M1b-1 已兑现）**：egui 的 capture 只覆盖 egui 自己那一层。§3 的四层 wgpu 合成器接进来后，画布层不在 `request_screenshot` 的视野里——所以画布基线**不走** egui 截图，而是直接渲染自有纹理再回读（§4.7）。比对、容差、差异图这三件事如预告般原样留下，只是从 `mw-shell::golden` 拆成了 `crates/mw-golden`，两个捕获源共用同一口径。

### 4.6 M1a：布局 / 场景 / 主题 / 排版移植进 `mw-core`（2026-10-16）

ADR-010 那句「核心逻辑不必陪 WebView 一起作废」的第一次兑现：`ui/src/lib/{layout,scene,themes,measure,icons}.ts` 共 **1140 行**算法逐常数搬进 Rust，`ui/` 保持冻结、降级为对照参考，不再是运行依赖。§9 第一行由它落地。

| 新模块 | 行 | 职责 | 单测 | TS 出处 |
| --- | --- | --- | --- | --- |
| `geom.rs` | 237 | `Point`/`Rect`/`Enclosing`/`clip_toward` + 几何路径 `Path{start, segs}`（`Line`/`Quad`/`Cubic`） | 5 | 散在 layout/scene 里 |
| `text.rs` | 208 | `TextMeasurer` trait、行高 `round(fs×1.45)`、CJK 逐字 / 拉丁整词的贪心折行、`MonoMeasurer` 占位度量 | 7 | `measure.ts` |
| `theme.rs` | 669 | 4 套主题令牌 + `Rgba` 解析（`#rgb/#rrggbb/#rrggbbaa`）+ `resolve_all` 样式级联（覆盖、分支色循环、文字下渗） | 11 | `themes.ts` |
| `icons.rs` | 117 | 12 个图标标记，字形全部是印刷符号（①…⑥ ○ ◐ ✓ ★ ? !），**不用 emoji**（docs/12 D6） | 2 | `icons.ts` |
| `layout.rs` | 824 | 放射 / 向右脑图 / 组织结构图三布局、CSR 子表、`band_h` 反向扫描、`edge_path`、折叠按钮位 | 17 | `layout.ts` |
| `scene.rs` | 1070 | `build_scene`：级联 + 布局 + 自由主题合并 + 边界 / 概要花括号 / 关系线弧线箭头 + 全局包围盒 | 28 | `scene.ts` |

**六条移植约束**（M1b 的合成器就按这些接口对接）：

1. **`mw-core` 零 UI 依赖**：布局只认 `NodeMeasure` trait，排版只认 `TextMeasurer` trait。字形图集（M1b）实现这两个 trait 就接得上，core 不引 egui / wgpu。
2. **稠密 `u32` 索引**：`Scene{ids, index, nodes, edges…}` 里节点用 `0..n` 的下标互相引用，不用字符串键——实例缓冲和均匀网格命中结构可以直接建，不必每帧哈希 UUID。
3. **路径是几何段，不是 SVG 字符串**：`PathSeg` 三型即可描述花括号、连线、关系线。渲染层 tessellate、导出层喂 resvg / printpdf，同一份数据出两条产物（§9「导出与画布同源」）。
4. **颜色在 core 里解析一次**：`resolve_all` 把主题级联 + 覆盖全部摊平成 `ResolvedStyle{fill,text,bold,italic,font_size,shape,stroke,stroke_width,branch_color}`，画布和 PDF 不会再各自解析出不同结果。
5. **确定性是补出来的，不是照抄的**：Rust `HashMap` 迭代序每轮进程都不同，旧 TS 版没这条约束。所以自由主题先按 id 排序再摆位、边界 / 概要 / 关系线一律按 `doc.doc_order()` 前序遍历、`draw_order` 按（深度, y, x）排且用 `total_cmp`（负坐标下 `to_bits` 排序是反的）。有单测锁住（`scene_is_deterministic_across_runs`、`layout_is_stable_across_runs`）。
6. **不用递归**：深度大的导图会爆栈，`resolve_all` / 子树并集 / 布局摆位全部显式栈；`band_h` 与列宽用「反向前序」累加，子表用计数排序建 CSR（前序保证兄弟连续）。

实测（`cargo run -p mw-core --release --example perf_scene`，本机 debug→release，度量器是占位实现，量的是规模不是排版精度）：

| 规模 | `build_scene` 中位 | 仅 `compute_layout` | 改一个字后全量重建 p95 | 预算 |
| --- | --- | --- | --- | --- |
| 500 节点 | 0.8 ms | 0.2 ms | 1.4 ms | P7 16 ms |
| 2000 节点 | 3.1 ms | 0.6 ms | 3.9 ms | P7 16 ms |
| 5000 节点（4999 连线） | **8.3 ms** | 1.8 ms | **11.1 ms** | P8 120 ms / P7 16 ms |

三个可执行的结论：① P8 余量 14 倍，布局本身不是风险；② `build_scene − compute_layout ≈ 6.5 ms`，**真正的大头是样式级联与场景装配**，M1c 做增量时应连带缓存 `ResolvedStyle`，别只优化布局；③ 还没写任何增量重排，全量重建的 p95 已经压进 P7 的 16 ms——增量的优先级可以往后放，先保渲染帧预算（P5 ≥55fps）。

两个不明显的坑：
- `match pos { Some(p) => … }` 之后又 `pos.is_none()` → `Position` 不 `Copy`，部分移动后借用报错；改成 `match pos.as_ref()`。
- 场景层原本照抄了 TS 的 `labels.iter().filter(|l| !l.trim().is_empty())`，但 `AddLabel` 入口已 trim 并拒绝空白、`validate()` 再兜一道——**这条防御针对的情况根本不可能出现，删**（连带删掉为它编的假文档单测）。概要文字相反：`check_annotation_text` 只限长度，`"   "` 会存成 `""`，所以「空概要显示『概要』」是真会走到的分支，保留。

**验证**：`cargo test --workspace` **127 项全绿**（`mw-core` 100，M1a 净增 70），`cargo check --workspace --all-targets` 0 警告。常数逐条对齐 TS：`PAD_X 14 / PAD_Y 9 / TOGGLE ±11 / 间隙下限 4·20 / 兄弟 24 / 层间 80 / maxNodeWidth 300 / 文字高 = lines×lh+18 下限 32 / 节点宽 = 文字宽+28+图标宽 下限 60 / 胶囊字号 clamp(10,12,round(fs×0.78))、高 = 字号+8、宽 = 文字宽+12、间隔 6、标签区 = 胶囊高+7 / 图标宽 = 字号+6 / 边界外扩 14 / 概要 13px·上限 220·主题 max(w+20,44)·max(h+12,26)、括号外移 10、尖 13、间距 6 / 关系线弓形 k=min(56, len×0.22)、箭头长 9 半宽 4.2、标签 12px / 画布外扩 16 / 连线宽 2.4·1.6`。

---

### 4.7 M1b-1：`mw-render` 四层合成器骨架（2026-10-17）

§3 那张图里的画布渲染器第一次落地：`Scene` 不再是 SVG 字符串的产地，而是 wgpu 实例缓冲的产地。新增两个 crate，**`ui/` 从此除了「对照参考」之外一行代码都不剩**。

| 模块 | 行 | 职责 |
| --- | --- | --- |
| `mw-render/src/instances.rs` | 398 | `Scene` → 四层实例（`RectInst` 64 B / `SegInst` 48 B / `TriInst` 48 B，全部 POD）。**纯函数，不看 GPU**，所以没有显卡也能单测 |
| `mw-render/src/camera.rs` | 84 | `Camera{center,zoom,ppp}` ↔ `CameraUni`（64 B，字段序与 WGSL `Uni` 逐字节一致）；`css_to_world` / `chrome` 两个换算口子 |
| `mw-render/src/flatten.rs` | 139 | 贝塞尔 → 折线（de Casteljau 递归，`MAX_DEPTH 12`，quad 先升三次）；容差按**物理像素**换算 |
| `mw-render/src/compositor.rs` | 542 | 适配器/设备/4 条管线/12 块实例缓冲/离屏目标 + `copy_texture_to_buffer` 回读 |
| `mw-render/src/shaders/*.wgsl` | 243 | `common`（`Uni` + `world_to_clip` + `cov`）/ `lattice` / `rect` / `seg` / `tri` |
| `mw-golden/src/lib.rs` | 179 | 从 `mw-shell::golden` 拆出：`Drift`（逐像素比对 + 包围盒 + 白底热力图）、`read_png`、`write_png` |

**着色器接口约定**（M1b-2 加文本族时必须照抄）：`@group(0) @binding(0)` 相机 uniform（VERTEX|FRAGMENT 可见），`@group(1) @binding(0)` storage 实例数组；四个族共用 `common.wgsl`，编译期 `concat!(include_str!…)` 拼进每个模块——WGSL 没有 `import`。四边形是 `TriangleStrip`，所以顶点序固定为左下·右下·左上·右上。

**五条不显然的决定**：

1. **在编码 sRGB（gamma）空间混合，输出预乘，目标 `Rgba8Unorm`（不是 `…Srgb`）**，`BlendState{One, OneMinusSrcAlpha}`。与 egui-wgpu、Direct2D、旧 SVG 三层一致——换成线性空间会让 `#f7f8fb` 这类浅灰底整体压暗一档，观感上就是「变脏了」。
2. **不开 MSAA，全部解析抗锯齿**：矩形 `sd_round`、胶囊段"到线段距离减半宽"、三角形重心坐标 × 高。带宽 `px = 1/(scale·ppp)` **从 uniform 推导，不用 `fwidth`**——`fwidth` 的结果取决于驱动怎么算导数，而 golden 基线的容差是 0。
3. **渲染是 `(Scene, Theme, Camera, 选中集)` 的纯函数**：core 侧已经用排序保证了确定性（§4.6 第 5 条），这里补一条单测逐字节比对两次 `build_layers` 的输出，像素侧再比对两次 `render()` 的回读。基线可信全靠这两条。
4. **铬件与形状的分界写进类型**：`css_to_world(css) = css/zoom` 用于选中环、折叠圆点、连线宽、边界描边；`chrome(css, world_floor) = max(css/zoom, floor)` 复现旧版 `Math.max(2.2, 3.2/z)`（缩到极小时恒定像素会糊成一团，得给世界单位兜底）。形状、圆角、字号是世界单位，随缩放放大。
5. **细分容差用物理像素**（`FLATNESS_PHYS 0.3`）：放大时曲线自动更顺滑，缩小时段数自动减少——这是 M1c LOD 之前的第一道闸门，实测见下表。

**像素级验收**（`crates/mw-render/tests/offscreen.rs`，5 项）：`chrome_stays_constant_while_shapes_scale` 从节点中心向右逐像素扫描，缩放 1 与 3 各一次：

| 量 | 归属 | 缩放 1 | 缩放 3 | 判据 |
| --- | --- | --- | --- | --- |
| 选中环实心宽度 | 铬件 | **2 px** | **2 px** | `== SELECT_RING_W`，跨缩放不变 |
| 环起点到盒边距离 | 铬件 | **3 px** | **3 px** | `SELECT_RING_GAP 2.5` ±2 |
| 盒右边界离中心 | 形状 | **35 px** | **108 px** | `半宽 × zoom` ±3 |

一次渲染跑在真实 Vulkan 适配器上，回读 900×600 与基线 `shots/golden/canvas-ppp1.00.png` 逐像素容差 0 比对。基线此刻**还没有字形**（文本层是 M1b-2），所以 M1b-2 落地后这张 diff 应当只多出文字——这就是它现在要先定下来的理由。

每帧 CPU 成本（`cargo run -p mw-render --release --example perf_layers`，度量器是占位实现）：

| 规模 | 缩放 | L1 线段实例 | `build_layers` 中位 | p95 |
| --- | --- | --- | --- | --- |
| 500 | 1.000 | 9 206 | 0.28 ms | 0.55 ms |
| 5 000 | 1.000 | **87 147** | **4.87 ms** | 5.40 ms |
| 5 000 | 0.007（整树适配 1600×900） | 15 080 | 1.37 ms | 1.59 ms |

两个结论，都归 M1c：① **一条连线在 100% 下平均摊 17.4 个胶囊段**，这是旧 SVG 不需要付的钱（浏览器自己栅格化 `<path>`），所以视口裁剪不是可选项——5k 节点里任一时刻可见的只有几十到几百个，裁掉之后这项成本直接掉一个数量级；② 缩小时段数确实随容差收缩（87 147 → 15 080），但 5 124 个节点矩形仍然全量提交，**裁剪要连面族一起做**。

五个坑：

- **`wgpu::Color` 的分量是原样交给后端的**（`cv.r as f32` → Vulkan float clear），所以 unorm 目标必须填 **0..1**。按老习惯填 0..255，`247.0` 被钳成 `1.0`，画布底色 `#f7f8fb` 渲染成纯白。像素断言抓到的第一个真 bug——纯看截图很难发现浅灰底变白了。
- **`meta` 是 WGSL 保留字**，实例字段名撞上会直接编译失败（`name 'meta' is a reserved keyword`），已改名 `pen`；存储布局不变。
- `InstanceDescriptor` 在 wgpu 30 **没有 `Default`**，用 `new_without_display_handle()` 再赋 `backends`；`PollType::Wait` 是结构体变体 `{submission_index, timeout}`，得把 `queue.submit()` 返回的 `SubmissionIndex` 一路传进回读；`get_mapped_range()` 返回 `Result`。全部对着 `~/.cargo/registry` 里的 wgpu 30.0.1 源码核过，没有猜。
- **测试探针选错节点会量到别的东西**：最初拿「叶子甲」当探针，它的描边正好是 `palette[0]`，而 snow 的 `palette[0]` **恰好等于强调色**，于是环和描边在像素上连成一段；而且这个叶子带标签胶囊和备注角标，右侧扫描线全部撞上。改成「分支三」（绿色描边、右侧无元素）后一次通过，并补了一条前置断言：探针描边不得与强调色相近。
- 回读要 256 字节行对齐（`padded_row`），`Image::pixel()` 按 `stride` 取点、`rgba()` 去填充——忘记对齐会读到整幅斜切。

**验证**：`cargo test --workspace` **138 项全绿**（M1b-1 净增 11：`mw-render` 6 单测 + 5 集成、`mw-golden` 5 项迁移），`cargo check --workspace --all-targets` 0 警告，`cargo fmt --all -- --check` 干净。拆分没有动摇 M0：`mw-shell --golden` 在容差 0 下仍与 `shell-ppp1.50.png` 逐像素一致。

**这一段故意没做**：文本层（M1b-2）、命中与交换链接入（M1b-3）、视口裁剪与 LOD（M1c）。当时还欠一笔虚线账，这里一并更正：旧 SVG 里**只有概要主题**是 `stroke-dasharray: 4 3`，边界一直是实线（原文写成「边界与概要框」，不准确）；`RectInst`/`rect.wgsl` 当时只画面和实线描边。M1c 在 `rect.wgsl` 里按圆角周长连续采样相位补上（§4.11 第 6 条），没有另加路径族。基线还有一层性质要记住：**它绑这台机器的驱动**（容差 0），换显卡或升 wgpu 会报漂移——这是有意的，届时用 `MW_GOLDEN_UPDATE=1` 重拍并说明原因。

---

### 4.8 M1b-2a：DirectWrite 文本度量与中文禁则（2026-10-17）

痛点 **C2（换行 / 禁则 / 标点）** 的代码半边。docs/07 §4 当年把这三件事划给「浏览器排版引擎（CSS 内建），**无需自研**」——浏览器一走，这笔债就到期了：`mw-core` 里的 `MonoMeasurer` 是「汉字 1 em、拉丁 0.55 em」的近似器，它撑得起 M1a 的移植对账，撑不起一个要交付的编辑器。这一节把真字体度量接进来，并且把**禁则拆成策略与引擎两层**：策略（哪些标点不许起行、哪些不许收尾、怎么搬）在 `mw-core::text`，纯函数、不碰 Windows，能在任何机器上单测；引擎（DWrite 工厂/格式/布局、UTF-16↔字节映射、真实宽度）在 `mw-render::dwrite`。

| 模块 | 行 | 职责 |
| --- | --- | --- |
| `mw-core/src/text.rs`（增补） | 308 | `FORBIDDEN_LINE_START`（20 字）/ `FORBIDDEN_LINE_END`（6 字）两张表 + `enforce_kinsoku()`：把落到行首的收尾标点拉回上一行、把挂在行尾的开首括号推到下一行。**只搬字符，不改字符** |
| `mw-render/src/dwrite.rs`（新） | 386 | `DWrite`（工厂 + 字体族择一 + 按 `(字号,字重)` 缓存 `IDWriteTextFormat`）与 `DWriteMeasurer`（实现 `TextMeasurer`，按 `(文本,字号,粗体,限宽)` 缓存 `Measured`，上限 20 000 条） |
| `mw-render/tests/text_measure.rs`（新） | 238 | 12 项，**痛点 C2 的验收门**。没有 DWrite 就 `panic!`，不许跳过 |
| `mw-render/examples/probe_text.rs`（新） | 101 | 把原始 `DWRITE_LINE_METRICS`（length / trailing / newline / height / baseline / isTrimmed）与两种布局宽度并排打出来。下表所有实测数字都出自它 |

**接口约定**：`DWriteMeasurer::measure_result()` 返回 `windows::core::Result<Measured>`，失败原样抛出——**测试路径不许被近似回落蒙过去**；只有 trait 实现 `measure()` 才在 `unwrap_or_else` 里退回 `MonoMeasurer`，那是给非 Windows 与降级场景用的。字体族按 `FONT_CANDIDATES = 微软雅黑 UI → 雅黑 → Noto Sans SC → Segoe UI` 用 `FindFamilyName` 择一，locale 固定 `zh-Hans`，粗体字重 **600**（对齐旧版 CSS，不是 700）。

**六条不显然的决定**：

1. **自然段自己按 `\n` 切，再逐段喂 DWrite**。DWrite 的行断结果包含它自己的分段判断，一次性丢整段文本进去，硬换行与禁则兜底会互相踩；切开后每段独立折行、独立兜底、独立重量。
2. **兜底用「搬字符」而不是「重排文本」**：`enforce_kinsoku` 只在行间移动已有的字符，绝不做插入或删除。这样 `concat(lines) == original`（除首尾空白）成为一条可断言的不变量——`prose_corpus_keeps_every_line_legal` 就对 12 条语料 × 3 种宽度同时断言「每行合法」与「字符没丢」。
3. **兜底之后必须按新行重新量宽**。搬过标点的行已经不是 DWrite 那次布局产出的行了，`DWRITE_TEXT_METRICS.width` 是旧行的数。所以 `width_of()` 把每个最终行当成一条不折行的布局重建再取最大值（行数极少，整条缓存，成本摊得掉）。
4. **悬挂后要重折剩余部分**。把 `——` 拉回上一行会留下「本行只剩一个字、下一行还很满」的孤儿；`orphan_index()` 找到它，把从该行起的尾巴重新喂一次 DWrite，最多 `REFINE_PASSES = 3` 轮、无进展即停。`"破折号——后面还有字"` @56px 由此得到与浏览器逐行一致的 `["破折号——", "后面还有", "字"]`。
5. **中英之间 0.1 em 的间隙留到渲染期做**（docs/07 §4 那条规则）。它属于字距，不属于文本内容——在度量阶段插入空格会污染行文本、光标定位与撤销栈，M1b-2b 用字形步进偏移实现。
6. **度量口径全在 DIP**，与 M1a/M1b-1 一致；物理像素只在栅格化那一层出现（M1b-2b 的图集键 `size_px = round(font_size × zoom × ppp)`）。

**DWrite 实测（Win11 / 微软雅黑 UI / 14 px，`probe_text`）**——四条和文档或 MSDN 说法不一样，都被写成了代码里的注释与断言：

| 现象 | 期望（文档/直觉） | 实测 | 处理 |
| --- | --- | --- | --- |
| `trailingWhitespaceLength` | MSDN 措辞读起来像「不含换行」，直觉要再加 `newlineLength` | **已经包含**换行 | 内容长度 = `length - trailingWhitespaceLength`。按直觉写会吃掉硬换行前那个字：`"甲\n乙"` 度量成 `["", "乙"]` |
| 尾部空行 | 以为 DWrite 不计空尾行，要手动补 | `"甲\n"` 报 2 行、`"甲\n\n"` 报 3 行，**如实报出** | 删掉自己加的补行逻辑（原本会双计） |
| 汉字/全角标点步进 | 约 1 em | **精确 1 em**（`中` 14.000、`一二` 28.000）；空格 4.143、`Rust` 29.463、`0` 8.210、`%` 12.455 | 断言写成「不等于 `MonoMeasurer` 的 4×14×0.55」，把近似器与真度量钉死区分开 |
| `——` / `…` 起行 | 以为在 DWrite kinsoku 表内 | **不在**：实测切成 `["破折号", "——后", "面还有字"]` | 这正是项目自建禁则表比 DWrite 宽的理由，兜底不可省 |

另有一处必须记住的**坐标系差**：14 px 下 DWrite 给 `lineHeight 17.78 / baseline 14.22`，而本项目沿用旧版的 `line_height(14) = round(14 × 1.45) = 20`。度量层继续报 20（改了会动摇 M1a 已钉死的全部布局基线），**差值由 M1b-2b 在盒内定位基线时消化**——图集与文本实例族要把字形安进我们自己的 20 px 行盒里。

**度量成本**（`cargo run -p mw-render --release --example perf_layers`，5 000 合成节点，含布局与场景装配、不是纯文本）：

| 度量器 | 冷（全量重排，P8 预算 120 ms） | 热（增量，P7 预算 16 ms） |
| --- | --- | --- |
| `Noop`（占位，零宽度成本） | 12.89 ms | — |
| `MonoMeasurer`（近似） | 16.74 ms | — |
| `DWriteMeasurer` 冷 | **47.79 ms** | — |
| `DWriteMeasurer` 热 | — | 中位 **12.95 ms**，p95 **19.64 ms**（超） |

三点解读：① 真度量把全量重排推到近似器的 2.9×，但 5k 下仍在 P8 内，**代价可付**；② 热路径 p95 19.64 > 16，和 M1b-1 的 `build_layers` 5.11 > 4 是**同一个结论的两次印证——不裁剪就不进不了帧预算**，视口裁剪连同 `ResolvedStyle` 缓存一起做，归 M1c；③ 换真度量后合成树的包围盒从 2408 宽收到 2158（`MonoMeasurer` 把拉丁按 0.55 em 摊宽了），**高度 137 838 一毫不变**：`max_node_width` 默认 300 而合成标题都短，每节点恒 1 行——M1a 记的「近似度量歪曲包围盒」要按这条修正。

五个坑：

- **`windows_core::Result` 不存在**，windows-rs 0.62.2 要写 `windows::core::Result`。五处一起报 unresolved，是照其它 crate 的旧习惯写的。
- **`windows::core::BOOL` 是 newtype**（`pub struct BOOL(pub i32)`，来自 windows-result），不是 `i32`。`FindFamilyName` 的 out 参数按 `&mut i32` 传直接不过类型检查；`exists.0 != 0` 才是「这个族在」。
- **每个 Win32 调用都得单独 `unsafe` 包**：`CreateTextLayout` 返回 `Result` 但参数是裸指针语义，`?` 不能省 unsafe 块；E0133 报在 `SetWordWrapping` 那一行时，第一反应以为是函数本身 unsafe，其实是整条链都必须在块内。
- **COM 接口不是 `Send`/`Sync`**：`IDWriteTextFormat` 存在 `RefCell<HashMap<..>>` 里是**有意的**，度量器从此不能跨线程共享。M1b-3 的结论是把画布与度量都留在 UI 线程（§4.10），没有引入跨线程共享；将来若要把度量搬离主线程，得按线程各建一个 `DWrite`，不能加锁复用。
- **测试的诚实性**：如果 DWrite 缺失就 `return`（本仓库惯例是优雅跳过），禁则那 26 个用例全部空转，绿灯毫无意义。改成 `panic!("…禁则用例不能靠近似度量蒙过去")`，并且前置断言 `family() == "Microsoft YaHei UI"`——**换机器字体族变了会红，这是要的效果**。

**验证**：`cargo test --workspace` **155 项全绿**（M1b-2a 净增 17：`mw-core` 5 条禁则单测 + `mw-render` 12 条 `text_measure` 集成），`cargo check --workspace --all-targets` 0 警告，`cargo fmt --all -- --check` 干净。`measurement_is_deterministic_and_cached` 断言同一输入两次度量结果逐字节相同（痛点 C2 出口判据之一）；M1b-1 的画布 golden 不受影响——度量换真了，但此刻基线**仍一个字形都没有**，正是 M1b-2b 想要的对照条件。

**这一段故意没做**：字形栅格化与图集、文本实例族与采样着色器、0.1 em 中英间隙、ClearType（画布要缩放，**只做灰度 1×1**，不做次像素 3×1）、IME 组合串的行内渲染。全部进 M1b-2b（#13）。

### 4.9 M1b-2b：字形图集与文本实例族（2026-10-17）

痛点 **C3（文字发虚 / 字体回退）** 的代码半边。§4.8 把宽度算准了，但画布上一个字形都还没画——旧栈里「排版 + 栅格化」全由浏览器代劳，现在这两件事都回到我们手里。这一节把 DirectWrite 的栅格化接进四层合成器：字形 → 单通道位图 → 4096² 图集（**历史值；P13 起 3072² 且 CPU 无镜像，见 §4.33**）→ 每字形一个四边形实例 → L2/L3 采样。图集键、栅格化路径与两趟绘制顺序是下面所有不显然决定的来源。

| 模块 | 行 | 职责 |
| --- | --- | --- |
| `mw-render/src/glyph.rs`（新） | 551 | `GlyphKey`（face / glyph / 字号 / 量化 ppd）、`GlyphAtlas`（4096² R8、货架装箱、1 texel 间隙、脏矩形、满则重置；**P13 起 3072²、`AtlasRect` 增量上传、无 CPU 镜像**）、双栅格化路径、`GlyphSlot`（位图偏移 + 世界坐标四边形 + UV） |
| `mw-render/src/text.rs`（新） | 557 | `TextEngine`：`IDWriteTextRenderer` 收集字形、字体面去重、形状缓存（20 000 条）、0.1 em 中英间隙、节点/胶囊/摘要/关系标签四类定位、**两趟 `paint`** |
| `mw-render/src/shaders/text.wgsl`（新） | 42 | 图集 `textureSample(...).r` × 实例色，输出预乘 |
| `mw-render/tests/text_render.rs`（新） | 394 | 6 项集成：层归属、两行行距、像素归属、重绘稳定、四档清晰度、文字 golden |

**六条不显然的决定**：

1. **两条栅格化路径，主路径不碰次像素**。主路径 `IDWriteFactory2::CreateGlyphRunAnalysis` + `GRAYSCALE` + `ALIASED_1x1`，得到 1 通道 8 位抗锯齿；兜底用基工厂 `GDI_NATURAL` + `CLEARTYPE_3x1`，把三通道按亮度合成灰度。实测 **Factory2 上 `CLEARTYPE_3x1` 的 texture bounds 是空的**——这不是参数没设对，是 Factory2 的 ClearType 纹理只对不透明背景产出；兜底必须有。画布要自由缩放，次像素渲染在任何非整数缩放下都会出彩边，所以**永远不用 3×1 输出**。
2. **ppd 按桶量化、向上取整**：`[0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0]`。图集键是 `(face, glyph, 字号 bits, ppd×100)`，所以同一字号在 1.0× 与 1.25× 下是两份位图——这正是「任意缩放不糊」的代价与来源。主路径把桶值当 ppd 传给 `CreateGlyphRunAnalysis`；兜底路径的 API 没有 ppd 参数，只能把 `fontEmSize` 乘上桶值再栅格化，`GlyphSlot` 统一记下自己的 ppd 供四边形换算（世界尺寸 = 位图像素 ÷ ppd）。
3. **图集满了就整体重置，但实例 UV 不会失效**。`paint` 分两趟：先把所有行形状化、把所有字形塞进图集（满了就 reset 再来，最多 2 轮），第二趟才发实例。若边发实例边插字形，中途 reset 会让前半帧的 UV 指向已清空的区域——两趟是**正确性要求**，不是优化。`AtlasUpdate` 带 `generation + reset + 脏矩形`：GPU 侧 generation 变了就整张重传，否则只 `write_texture` 脏矩形（stride 固定 4096，256 对齐）。
4. **形状与栅格化解耦**。`IDWriteTextLayout.Draw` 只用来拿字形索引、步进、偏移和 UTF-16 簇位置（`IDWritePixelSnapping` 禁用、ppd 传 1.0），结果按 `(文本, 字号, 粗体)` 缓存——形状不随缩放变，只有位图随 ppd 变。字形 `dy = -ascenderOffset`；一行里混了多个字体面时按 family/weight/stretch/style/simulations 去重后存 `IDWriteFontFace`，所以中文回退到雅黑、拉丁留在 Segoe UI 的混排天然支持。
5. **0.1 em 中英间隙落在簇边界上**：`cjk_latin_gaps` 扫 UTF-16 簇位置，只在「汉字 ↔ ASCII 字母数字」直接相邻时插间隙；空格本身已承担分隔，不再加。间隙计入 `ShapedLine.width` 供居中，但**绝不写回文本内容**（否则光标、撤销与检索都会被污染）。
6. **基线用旧 SVG 的公式，`0.72` 消化两套行盒差**：节点逐行 `rect.y + (h - label_h - lines×line_height)/2 + i×line_height + line_height×0.72`；胶囊中心 + `fontSize×0.36`；摘要 `topic.center.y + 4.5`；关系标签 `label_at.y + 4`。§4.8 记的 DWrite 自然基线 14.22 与我们的 20 px 行盒差，在这里由 `BASELINE_RATIO = 0.72`（20 × 0.72 = 14.4）吸收；文字位置因此与旧版截图同源，而不是跟着 DirectWrite 的自然行高漂。

**实测**（Win11 / 微软雅黑 UI）：

| 项目 | 结果 |
| --- | --- |
| 14 px `中` 位图 | ppd 1.0 → 12×12；ppd 2.0 → 27 px 高（栅格化按整像素取整，测试容差 ±4 px） |
| 两行行距 | 第二行基线严格等于 `line_height(14) = 20`，与布局层的行盒一致 |
| 重绘 | 同一帧 `paint` 两次：第二次零新脏矩形，`TextInst` 逐字节相同，渲染回读相同 |
| 像素归属 | 开/关文本两次渲染的差异像素（>1000 个）全部落在字形四边形内（+1 px 抗锯齿边） |
| 四档清晰度 | ppp 1.0 / 1.25 / 1.5 / 2.0 下文字包围盒的物理高度比与 ppp 比一致（≤25% 容差，抗锯齿边会贡献 ±1 px） |

**四个坑**：

- **`DWRITE_GLYPH_RUN` 不能指向栈上的临时变量**。探针里把 `glyphIndices` / `advanceWidths` 放进辅助函数就返回，`CreateGlyphRunAnalysis` 读到的全是垃圾——症状是所有字形都变成空心 `.notdef` 方框，而不是报错。现在 run 与它的两个数组在同一帧构造、同一帧消费，并留了 `real_glyph_is_not_the_dotnotdef_box` 回归测试（断言 `中` 的位图不是方框）。
- **`GetAlphaTextureBounds` 为空不是「字形不存在」**。同一个字形在 `ALIASED_1x1` 有边界、在 `CLEARTYPE_3x1` 可能为空；空格则两种都为空。`Insert::Failed` 与 `Insert::Ready(空 slot)` 要分开处理：前者下次还会重试，后者直接跳过绘制。
- **`write_texture` 的源切片要按整幅 stride 算**。脏矩形 `(x, y, w, h)` 的源字节是 `y×4096 + x` 到 `(y+h-1)×4096 + x + w`，中间包含行间空隙；按 `w×h` 紧凑拷贝会错位，且 `bytes_per_row` 必须给 4096（256 字节对齐）。
- **形状缓存只按文本，不按 ppd**。把 ppd 放进形状缓存键会让 5k 节点在缩放时整表失效，而形状本来与 ppd 无关；反过来，把字号放进键是必须的——同一句话在标签 11 px 与正文 14 px 下步进不同。

**验证**：`cargo test --workspace` **166 项全绿**（M1b-2b 净增 11：`mw-render` 5 条单测 + 6 条 `text_render` 集成），`cargo check --workspace --all-targets` 0 警告，`cargo fmt --all -- --check` 干净。5 条单测覆盖 ppd 向上取桶、真字形不是 `.notdef`、CJK 随 ppd 放大、货架装箱与脏矩形、中英边界判定；6 条集成把「文字只画在该画的地方」钉在像素上。新基线 `shots/golden/canvas-text-ppp1.00.png` 容差 0；M1b-1 的几何基线 `canvas-ppp1.00.png` **原样未动**（`offscreen` 测试不调用 `paint`）——两张基线分工明确。人眼验收图落在 `shots/render/text-ppp{1.00,1.25,1.50,2.00}.png`：根节点、中英混排、两行节点、标签胶囊、摘要与关系标签全部清晰，字重与回退正确。

**这一段故意没做**：IME 组合串、光标与选区的文字绘制、命中测试（当时留到 M1b-3，已完成 → §4.10）、ClearType 次像素（画布缩放 ⇒ 明确不做）、自定义字体加载与导出时的文字矢量化（M4 导出器复用同一份 `TextEngine` 时再定）、文本视口裁剪（M1c，与几何同一套）。

---

### 4.10 M1b-3：uniform grid 命中 + 平移缩放选择 + 合成纹理进 egui（2026-10-17）

M1b 的最后一块：画布第一次能点、能拖、能缩，也第一次出现在真实窗口里。前两节解决「画什么」，这一节解决「点哪个」和「怎么把自有纹理交给 egui」——**docs/12 §3.1 的命中设计在 Rust 落地，壳层与渲染层第一次真正握手**。命中索引只吃 `Scene`，交互语义全部留在 `mw-shell::canvas`，render 层不知道鼠标存在。

| 模块 | 行 | 职责 |
| --- | --- | --- |
| `mw-render/src/hit.rs`（新） | 620 | `Target` 七类目标、`owner_node` 归属映射、`HitIndex`（128 世界单位网格 + 大件列表）、`Shape`（Rect/Circle/Band/Curve/Triangle）、屏幕空间容差、7 项单测 |
| `mw-render/src/camera.rs`（增补） | 115 | `screen_to_world`：画布局部屏幕点 → 世界坐标（ppp 在往返中抵消），1 项往返单测 |
| `mw-shell/src/canvas.rs`（新） | 374 | 画布面板：样例文档、场景/命中/相机/合成器/文本引擎、首帧适配、Home 适配、Ctrl± / 捏合缩放、滚轮与拖拽平移、点击选择/折叠、悬停环与光标、纹理注册与更新 |
| `mw-shell/src/main.rs`（改造） | 402 | 从 `cc.wgpu_render_state` 建画布（共享 device/queue），CentralPanel 交给 `Canvas::show`；窗口 `centered` + 钳制到显示器，修掉「大窗口被摆到屏幕外」 |
| `scripts/m1b-canvas.ps1`（新） | 139 | 真实输入探针：DPI 感知坐标，选择 / 折叠 / 拖拽平移 / Ctrl+= 缩放四步截图 |

**七条不显然的决定**：

1. **跨类型优先级独立于绘制顺序**：`Toggle 6 > Relationship 5 > SummaryTopic 4 > Label 3 > Node 2 > SummaryBrace 1 > Boundary 0`，同类型再按 `z`（`draw_order` / 场景下标）反序。关系线的曲线端点天然落在节点盒边框上，若只按绘制顺序（节点后画）判，端点永远点不中关系线；按 docs/12 §3.1 的类型优先级，关系线必须压过节点。折叠点压过父节点则保证「点圆点折叠」不会变成「选中父节点」。
2. **容差是屏幕空间，不是世界空间**：关系线 14 CSS px、花括号 / 连接线 8、边界描边 5、折叠点 `TOGGLE_R + 2`。查询时经 `Camera::css_to_world` 换算，放大后不会「隔老远也能点中」。折叠点用 `chrome(10, 4.5)`，**与渲染的 `chrome(TOGGLE_R, 4.5)` 是同一个公式**——早先命中半径用了 5.5 世界单位下限，缩放后比画出来的圆大一倍，点圆外也能折叠。
3. **边界只算描边带，不算内部**：`Band { rect, pad_css }` 判定「外扩 5 px 内、内缩 5 px 外」。边界中间通常是节点和空白，把整个矩形算命中会让点空白选中边界、点子节点也被边界抢走——夹具测试直接把这条钉死。
4. **128 格 + 大件列表**：目标包围盒登记进 128 世界单位见方的网格；跨越超过 256 格的目标（全图边界、长关系线）放进 `oversized`，查询时无条件检查一次。否则一个覆盖全图的边界会在每个格子塞一份索引，构建时间与内存都被它拖垮。查询按 `max_tol + 2` 外扩格数，保证边缘目标不漏。
5. **HitIndex 与 Scene 同生命周期**：折叠改文档 → 重建 Scene → 重建 HitIndex。**节点下标会整体前移**，所以选择不能存一个 `u32` 跨重建，得先记节点 id、重建后再 `scene.find(id)` 映射回来；悬停也在重建后按新索引重算一次。这是本层唯一的状态陷阱。
6. **合成纹理共享 device，当帧可见**：`Compositor::from_device(render_state.device/queue)` 复用 eframe 的连接；离屏目标带 `TEXTURE_BINDING`，首帧 `register_native_texture`，尺寸变化时 `render_texture` 重建目标并返回 `recreated`，随后 `update_egui_texture_from_wgpu_texture`。渲染命令在 egui update 阶段提交，egui 同一帧的绘制命令随后入队，**队列顺序保证采样到的就是这一帧**——不需要等一帧，也不需要第二个 device。度量器与合成器都留在 UI 线程，§4.8 的 COM `Send` 约束没有变成跨线程问题。
7. **交互只改相机与选择，不改 render 接口**：`Frame { scene, theme, camera, selected, hovered, viewport }` 仍是纯输入。平移缩放不写进合成器，`Canvas` 负责把 egui 的指针/键盘翻译成 Camera 变化。首帧按场景包围盒适配（56 CSS px 留白、0.2–1.5×），Home 重新适配；滚轮平移、Ctrl+滚轮 / 捏合以指针为锚点缩放（锚点世界坐标在缩放前后保持不动），主键拖拽平移，`MIN_ZOOM 0.1 / MAX_ZOOM 4.0`。**M1c 把 `MIN_ZOOM` 与适配下限降到 0.005**（5k 全图需要 ≈0.0065，理由见 §4.11 第 7 条）。

**验证**：

- `cargo test --workspace` **174 项全绿**（M1b-3 净增 8：`hit` 7 + `camera` 1），`cargo check --workspace --all-targets` 0 警告，`cargo fmt --all -- --check` 干净。`cargo test -p mw-render` 42 项（19 单测 + 5 `offscreen` + 12 `text_measure` + 6 `text_render`）。
- 7 项命中单测：节点中心、折叠点压过父节点、关系线压过重叠节点（扫遍关系线折线，找真正落在节点盒内的点）、标签胶囊与摘要主题压过背景、花括号 / 边界描边可点而边界内部空白不可点、容差随缩放保持屏幕恒定、空场景 / 大件路径与 `owner_node` 归属。
- 壳层 golden 基线重拍为 `shots/golden/shell-ppp1.50.png`（1620×1080 @ ppp 1.50）：M0 时代只有标题栏与背景，现在整张画布经 egui 原生纹理上屏。`--golden --update --warmup 5` 重拍后，再跑一次 `--golden` **容差 0 逐像素一致**。
- 真实输入端到端（`scripts/m1b-canvas.ps1`，150% DPI）：点「DirectWrite 度量」出蓝色选中环；点「纯 Rust 原生」左侧圆点后两棵子树与概要花括号消失、节点保留选中、圆点变 ⊕；拖拽空白画布内容跟随指针；Ctrl+= 三次后文字与几何同步放大且 DirectWrite 字形仍清晰。截图 `shots/render/m1b3-{initial,selected,collapsed,panned,zoomed}.png`。
- 滚轮路径另用 `PostMessage(WM_MOUSEWHEEL)` + `keybd_event(VK_CONTROL)` 验证：滚轮平移、Ctrl+滚轮以指针为锚点缩放（`zoom_delta≈1.0088`）均生效。

**五个坑**：

- **自动化里的客户区坐标 ≠ 画布局部坐标**：画布在合并标题栏（40 pt）之下，脚本点折叠圆点用画布局部 y=251 时实际点到了 y=211 的空白。输入统一用「客户区逻辑坐标」，`Canvas` 内部再减 `rect.min`。
- **`mouse_event(MOUSEEVENTF_WHEEL)` 在当前远程会话里不会变成 egui 的滚轮事件**：winit 收不到合成的 wheel，改用 `PostMessage(WM_MOUSEWHEEL)` 才触发；键盘 Ctrl 仍可用 `keybd_event`。这是测试基建的坑，不是产品路径的坑。
- **纹理尺寸变化必须重新绑定**：`render_texture` 在物理尺寸变化时重建目标，旧的 egui 绑定会指向已释放的纹理。`recreated` 为真时调用 `update_egui_texture_from_wgpu_texture`，否则只画不更新。
- **悬停环不是「鼠标下最上层节点」**：关系线、边界、花括号命中后不画节点悬停环，只有 Toggle / Node / Label 映射到节点；但光标仍显示手型。否则鼠标划过关系线时整条分支都在发光。
- **窗口默认位置可能出屏**：1080×720 逻辑 @150% = 1620×1080 物理，winit 在 1920×1200 上把原点放在 (266,266)，底部与右侧被裁掉。`centered: true` + `with_clamp_size_to_monitor_size(true)` 后窗口完整可见（原点 150,61）。

**这一段故意没做**：节点拖拽 / 框选 / 拖出连线手势、键盘焦点与 Tab 导航、双击编辑与 IME（M2）、右键菜单、视口裁剪与 LOD（M1c，已完成 → §4.11）、GPU 帧时间埋点（M1c 只做到离线串行上界，见 §4.11）。画布里的文档是写死的验收夹具，`Document` / `Command` 的编辑入口已经接上（折叠就走 `Command::SetCollapsed`），但没有新建 / 删除 / 文本编辑的 UI。

---

### 4.11 M1c：视口裁剪 + LOD + 帧预算 + 摘要虚线（2026-10-17）

M1 的最后一块，回答的不再是「怎么画」而是「**这一帧值不值得画**」。§4.7、§4.8、§4.9、§4.10 的四次实测把视口裁剪列成硬性前置：不裁剪时 5k 节点 `build_layers` 中位 4.87 ms、DWrite 热路径 p95 19.64 ms，都进不了 16.6 ms 的帧预算。这一节把裁剪、LOD、降级与同源全图路径一次补齐，并顺手把 §4.7 记的虚线欠账还掉。

| 模块 | 行 | 职责 |
| --- | --- | --- |
| `mw-render/src/instances.rs`（增补） | 581 | `Frame.viewport` + `view_world` / `sees` / `text_visible` / `labels_visible` / `badges_visible`；L1/L2/L3 全族裁剪；`path_bounds`（控制点凸包）与 `relationship_bounds`（曲线+箭头+圆点+标签整组）；`rect_dashed` |
| `mw-render/src/text.rs`（增补） | 587 | 与几何**同一套** `Frame` 谓词：节点盒、行文字与图标、标签胶囊、摘要、关系标签都先问可见性与 LOD |
| `mw-render/src/budget.rs`（新） | 148 | `FrameBudget`：连续 3 s 低于 45 fps 进省渲染；空闲与长间隔不计；4 项单测 |
| `mw-render/src/compositor.rs`（增补） | 793 | `set_low_render`（省渲染只关 L0 点阵）、`wait_idle`（离线帧预算的 GPU 串行上界） |
| `mw-render/src/shaders/rect.wgsl`（增补） | 118 | 沿圆角矩形**连续周长**采样虚线的相位，`params.zw` = 实/虚长度（世界单位） |
| `mw-render/examples/perf_layers.rs`（重写） | 405 | 100% / 80% 可见 / 全图三档 × 裁剪/不裁剪；`--frame` 追加真实 DWrite + 合成器 + `wait_idle` 全帧 |
| `mw-shell/src/canvas.rs`（增补） | 381 | `Frame.viewport = Some(面板逻辑尺寸)`；每帧 `budget.tick(dirty)` 驱动 `set_low_render`；缩放下限 0.1 → 0.005 |
| `mw-render/src/camera.rs` / `flatten.rs`（增补） | 115 / 139 | 相机缩放下限 0.005；细分容差的缩放分母下限 0.005（低于它才钳制，避免小缩放白付段数） |

**七条不显然的决定**：

1. **裁剪是「可选输入」，不是渲染器的新模式**：`Frame.viewport: Option<[f32; 2]>`，`None` 就是全图——单测、离屏 golden、将来的导出都走同一条 `build_layers` / `text::paint`，不存在第二套绘制路径。可见世界 = 视口 / zoom 再**外扩 64 CSS px**，给拖拽/惯性的一帧留余量，避免边缘弹出；小缩放下外扩与视口本身都会变得巨大，裁剪自然退化为全画，**不会误裁**。
2. **包围盒宁大勿小，路径用控制点凸包**：贝塞尔曲线落在控制点凸包内，所以 `path_bounds` 取起点 + 全部控制点就是严格上界；再按线宽外扩。关系线以「曲线 + 箭头 + 起点圆点 + 标签胶囊」整组判可见——只要标签还在屏内，曲线就得画，否则会出现「标签悬空」。节点、折叠点、选中环、角标同理按各自外扩后的矩形判。
3. **文字与几何必须同一拍**：`text.rs::collect_jobs` 先 `sees` 节点矩形，再按同一 `text_visible` 决定是否整形文字/图标；标签胶囊还要单独 `sees`。两处用一个 `Frame` 谓词是刻意的——若文字按自己的视口裁，屏边会出现「有字无盒」或「有盒无字」，比不裁剪更刺眼。golden 里屏边没有半截字，靠的就是这条。
4. **LOD 是三条阈值，不是一档「低清」**：① 字号 × zoom × ppp < **7 物理像素**时不整形、不栅格化（此时字形已不可读，DWrite 与图集纯属浪费）；② 场景节点数 ≥ **5 000** 时不再画标签胶囊与备注/链接角标；③ 节点盒、连线、边界**永远画**，折叠、选择、悬停等交互件也不受 LOD 影响。省的是装饰与不可读文字，不是结构。阈值集中在 `instances.rs` 顶部，调档只改一处。
5. **帧预算只统计「连续重绘」**：egui 按需重绘，空闲时两帧可能隔几秒，按墙钟间隔算会立刻误判成 0 fps。`Canvas` 只在拖拽 / 缩放 / 点击等 `dirty` 帧调用 `tick(true)`，其余帧 `tick(false)` 清零累计；两帧间隔 >250 ms（窗口被拖住、系统挂起）也清零。连续 3 s 低于 45 fps 才**锁存**省渲染——降级诱因（低档 GPU、超大窗口）不会自己消失，锁存避免 44/46 fps 之间来回抖；省渲染只关点阵背景，节点、文字、命中与交互一律照旧，**没有任何功能降级**。45 而不是 55：55 是目标线，不是「已经坏了」的线，拿目标线当降级线会在正常波动时误触发。
6. **虚线只属于摘要主题，且在 SDF 片元里做**：旧 SVG 里只有概要主题是 `stroke-dasharray: 4 3`，边界一直是实线（§4.7 原先写的「边界与概要框」不准确，这里更正）。实现不新增路径族，也不在 CPU 上切段：`RectInst.params.zw` 带实/虚长度，`rect.wgsl` 的 `perimeter_s` 从**上边左端点**起顺时针算整圈周长参数（与 SVG 圆角矩形路径起点一致），四个圆角共享同一相位，不会在每个角上重置节奏。虚实长度是世界单位，缩放时与旧 SVG 同步伸缩；片元只多一个 `fract` 与比较。
7. **0.005 的缩放下限是测量逼出来的，不是拍脑袋**：5k 夹具包围盒 2408×137 838，要把它塞进 1600×900 需要 zoom ≈ 0.0065，80% 可见约 0.0100；旧的 `MIN_ZOOM 0.1` 让这两个档位在产品里根本到不了，性能探针等于在测一个不存在的视口。相机与 `flatten` 的分母下限一起降到 0.005，基准与产品共享同一取值范围；细分容差仍在 0.005 处钳制——再小已经没有产品路径，不能让浮点尾巴把段数放大。

**性能（i5-12450H / Intel UHD / Vulkan 18083 / 1600×900，5000 节点）**：

裁剪开关对 `build_layers` 的影响（中位 / p95，ms；L1 线段数含曲线细分后的胶囊段）：

| 档位 | 可见节点 | L1 线段 | 不裁剪 | 裁剪 | `build+paint` 裁剪 |
| --- | --- | --- | --- | --- | --- |
| 100% (z=1.0) | 26 | 975 | **4.55 / 5.23** | **0.29 / 0.34** | 0.31 / 0.42 |
| 80% 可见 (z=0.0100) | 4 000 | 6 553 | 1.06 / 1.20 | 0.64 / 0.76 | 0.94 / 1.12 |
| 全图 (z=0.0065) | 5 000 | 5 425 | 1.01 / 1.53 | 1.07 / 1.24 | 1.06 / 1.18 |

100% 下裁剪把几何构建压到 **1/15**（4.55 → 0.29 ms）；后两档几乎全图可见，裁剪的收益本来就该接近零——它保证的是「放大后不白算」，不是「任何缩放都更快」。全图 L1 从 §4.7 的 15 080 降到 5 425，是细分容差随 0.005 下限继续变粗的预期结果（此时 46 世界单位 ≈ 0.3 物理像素）。

全帧（`--frame`：真实 DWrite 场景 + 文本整形 + 图集上传 + 合成器提交 + `wait_idle` 串行等待）：

| 档位 | CPU | GPU | 串行合计（中位/p95） | 串行 fps 上界 | 按 max(CPU,GPU) 估 |
| --- | --- | --- | --- | --- | --- |
| 100% | 0.33 | 1.38 | 1.72 / 2.14 | 581.8 | 725.7 |
| 80% 可见 | 0.70 | 3.00 | 3.70 / 4.16 | 269.9 | 333.0 |
| 全图 | 0.81 | 3.44 | **4.24 / 4.47** | **236.0** | 291.0 |

P5 预算 18.18 ms（55 fps），最差档位仍有 **4.3× 余量**。注意这是**离线串行上界**：产品路径故意不 `wait_idle`，CPU 与 GPU 重叠，真实帧时间应接近 max 列；串行值只能更悲观。全帧只测渲染，不含场景重建——平移/缩放不重建 `Scene`，编辑才重建（DWrite 热 `build_scene` 中位 9.13 ms，冷 37.40 ms，属编辑路径而非每帧路径）。

**验证**：

- `cargo test --workspace` **181 项全绿**；`cargo check --workspace --all-targets` 0 警告，`cargo fmt --all -- --check` 干净。`mw-render` 48 项（23 单测 + 6 `offscreen` + 12 `text_measure` + 7 `text_render`）。
- `budget` 4 项：30 fps 连续 90 帧（2.999 s）不降级、第 91–92 帧触发；中途恢复 60 fps 清零；空闲帧不计；400 ms 长间隔不接上中断前的累计。
- `culling_drops_geometry_outside_the_viewport`：220×160 小视口下 L1/L2 实例数下降，**留下的每个节点矩形都与 `view_world()` 相交**；`viewport: None` 时 L2 矩形数回到 `节点数 + 2`（夹具的 1 个标签胶囊 + 1 个摘要主题）——全图路径没有被裁剪改坏。
- `text_lod_and_viewport_culling`：zoom 0.1 时字号 14 × 0.1 × 1 = 1.4 px < 7，字形数为 0 而节点盒仍在，标签也不画；缩小视口后字形数进一步下降。
- 三张 golden 基线重拍（`MW_GOLDEN_UPDATE=1`）：`canvas-ppp1.00.png`、`canvas-text-ppp1.00.png`、`shell-ppp1.50.png`（1620×1080 @ 1.50）。视觉差异只出现在**摘要主题的虚线**；壳层基线重拍后连跑一次**容差 0**。
- 真实输入复跑 `scripts/m1b-canvas.ps1`（150% DPI）：选择 / 折叠 / 平移 / Ctrl+= 缩放五张截图 `shots/render/m1b3-{initial,selected,collapsed,panned,zoomed}.png`，屏边无半截文字或缺失盒。

**六个坑**：

- **文字裁剪漏一处就是「幽灵字」**：最初只裁了节点文字，标签胶囊与摘要仍全量整形；屏边会出现没有盒的胶囊字。统一成 `Frame` 谓词后消失——以后新增文字族也必须走 `sees` + `text_visible`，不能只信 `draw_order`。
- **路径包围盒不能取起终点**：关系线两端都在屏外、弓形顶部却在屏内时，按端点判会整条消失。控制点凸包是严格上界，且比逐段采样便宜；线宽外扩别忘了，否则粗线边缘会在屏边被切。
- **虚线相位若按四条边各自算，圆角处会「跳拍」**：`perimeter_s` 用八个基址拼出整圈周长，四个圆角与四条直边共享一个 s；起点必须与 SVG 的上边左端点一致，否则与旧版截图对不上。
- **`view_world` 的缩放分母不能直接用产品下限**：早先把 `zoom.max(0.02)` 写进裁剪，结果低于 2% 时视口被高估得不够，反而裁掉了本应可见的内容。正确做法是只用 `1e-4` 防 0 除，让外扩自然吃掉一切。
- **省渲染不能看墙钟**：egui 空闲不重绘，正常停手 5 秒会被算成 0.2 fps。必须由壳层把「这一帧是不是用户连续操作」传进来，并在 >250 ms 的间隔上清零。
- **基准不能活在产品能力之外**：`MIN_ZOOM 0.1` 时 5k 全图根本缩不到，探针的「全图」档是伪造的。把产品下限一起改到 0.005，才让基准与真实路径同口径；这类「探针比产品能到得更远」的偏差不报错，只会悄悄给出一份好看的假数据。

**这一段故意没做**：真正的 SVG/PNG/PDF 导出器（按 §9 在 M4；同源所需的 `viewport: None` 全图路径已具备，导出器直接复用 `Scene`/`Frame`/`build_layers`，不另写绘制）。因此「导出与画布同源」目前是**架构约束已成立、导出器未落地**，M4 还要给大文档加一个显式关闭 LOD 的开关。另没做：增量场景重建（编辑仍全量 `build_scene`，实测在 P8 内）、动态分辨率、点阵密度分档、GPU timestamp query（跨后端可用性不一，P5 先用 `wait_idle` 串行上界）、视口裁剪命中索引（命中查询本身是网格查找，不需要按视口重建）、节点淡入动画。

---

### 4.12 M2a：节点内 IME 编辑 + 核心快捷键 + 撤销重做（2026-10-17）

M2 的第一刀切在**痛点 C1** 上：能不能在节点里用微软拼音打出一句中文，字不丢、候选窗不错位。这一节同时把编辑闭环（改文本、加子/同节点、删、撤销重做）打通，让画布从「能看能选」变成「能改」。

| 模块 | 行 | 职责 |
| --- | --- | --- |
| `mw-shell/src/canvas.rs`（增补编辑层） | 892 | `Editor` 状态（原文/当前文本/已排版快照/IME 组合位）、`editor_ui` 覆盖层、`sync_editing_layout` 实时重排、`shortcuts` 键盘语义、`add_relative` / `delete_selected` / `commit_edit` / `cancel_edit`、焦点归还与焦点锁 |
| `mw-shell/src/fonts.rs`（新） | 71 | 把系统**雅黑 TTC 索引 1（UI 版）**装进 egui 字体表，另注册 `zhisi-bold` 族；缺字体时退回 egui 默认，英文仍可读 |
| `mw-render/src/instances.rs`（增补） | 587 | `Frame.editing: Option<u32>` → 合成器对编辑中的节点**跳过自己的强调环**；`SELECT_RING_W` / `SELECT_RING_GAP` 导出给壳层，环几何只有一个来源 |
| `mw-render/src/text.rs`（增补） | — | 同一 `Frame.editing` 谓词：编辑中的节点不整形、不进图集，避免与覆盖层重影 |
| `mw-shell/src/main.rs`（增补） | — | 启动时 `fonts::install`；标题栏拖拽区与 8 条缩放热区改成**不可聚焦**；harness 下钉住 `focused` |
| `scripts/m2a-edit-probe.ps1`（新） | 172 | 真实窗口 + SendKeys + 逐屏截图的编辑流回归；每串按键前断言前台归属，被抢焦点就重试并记录 |

**八条不显然的决定**：

1. **编辑器用 egui `TextEdit` 覆盖层，而不是自绘 caret**：C1 的四个子问题（候选窗定位、组合串回显、语义键被吞、丢字）里前三个正是文本控件厂商花十年磨的部分。egui-winit 每帧拿 caret 矩形调 `set_ime_cursor_area`，组合串、光标、选区、UTF-16 代理对边界全部白送；自绘 caret 意味着这四件事都要自己接管 winit 事件循环，而它们恰好是验收项。**代价**是编辑期间该节点由 egui 排版、平时由 DirectWrite 排版，两套度量——于是让合成器让位（决定 2）而不是去对齐两套排版器。
2. **合成器与编辑器共用一个 `Frame.editing` 谓词**：`text.rs` 不整形该节点文字、`instances.rs` 不画它的环，节点**盒子和连线照旧**。覆盖层随后在盒外 `SELECT_RING_GAP + 环宽/2` 处画强调环——环必须画在盒**外**，画在盒内会被根节点那种饱和填充整个吃掉，等于没有编辑态反馈。
3. **进入编辑不跳字**：覆盖层的字号 = `节点字号 × zoom`，margin 左 = `节点.text_x × zoom`、右 = `PAD_X × zoom`、上 = `PAD_Y + 居中余量`，全部从同一份 `NodeGeom` 换算。实测编辑前后节点盒尺寸相差 ≤3 px（150% DPI 截图逐列量出来的，不是目测）。低于 `EDIT_MIN_PX = 13` 屏幕像素时先**围绕节点中心**放大画布——候选窗贴在一个 6 px 高的光标上没有意义。
4. **节点盒随输入实时增长**：每帧把 `editor.text` 喂给 `build_scene` 的 editing 槽，但只在 `laid_out != text` 时重排（`sync_editing_layout`）。这一步必须在**读输入之后、算命中之前**：晚一帧会先看到文字溢出盒子、下一帧才追上，早一帧会丢掉本帧刚收到的字符。
5. **IME 组合期挂起 Enter / Esc**：`ed.ime` 由本帧最后一条 `ImeEvent`（`Preedit` 非空 = 组合中，`Commit` = 结束）推出。组合期间这两个键归输入法，抢过来会把候选串当空文本提交——这是 C1「丢字」最常见的成因。提交动作放在编辑器 UI **之后**（`commit_after`），保证同一帧先到的字符已经落进 `editor.text`。
6. **画布必须主动锁键**：Tab / 方向键在 egui 里默认是「移动焦点」，Escape 默认「交出焦点」，而且这个判定发生在 `allocate_exact_size` 的**分配阶段**——比本帧后面读键盘更早。所以每帧对画布 `set_focus_lock_filter(tab, escape, horizontal_arrows, vertical_arrows)`，并在每次提交/取消/删除后 `refocus_canvas` 把焦点要回来。
7. **点击判定只认鼠标**：`Response::clicked()` 含 `FAKE_PRIMARY_CLICKED`（焦点控件上按空格/回车，egui 的无障碍激活语义）。画布正是那个焦点控件，于是「Enter 加同级节点」会在同一帧被当成「点了画布」，刚打开的编辑器立刻提交，留下一个空节点。改用 `clicked_by(Primary)` / `double_clicked_by(Primary)` 后消失。
8. **撤销粒度 = 一次提交一条 `SetText`**：编辑期间的 Ctrl+Z 交给 `TextEdit` 自己的文本栈（用户预期是「退一个字」），退出编辑后 Ctrl+Z 才是文档栈（「退一次提交」）。两套栈不合并是刻意的——合并要做字符级命令，代价远大于收益。空文本与原文相同则不产生命令，撤销栈不被无意义条目污染。根节点不可删，删除后选择落到父节点。

**验证**：

- `cargo test --workspace` **183 项全绿**（+2）、`cargo check --workspace --all-targets` 0 警告、`cargo fmt --all -- --check` 干净。`mw-render` 50 项（24 单测 + 6 `offscreen` + 12 `text_measure` + 8 `text_render`）。
- 新增两项：`editing_node_keeps_its_box_but_not_its_glyphs`（`Frame.editing` 指向某节点时 L2 仍有它的矩形，但字形归属里没有它——盒子在、字让位）；`screen_to_world_round_trips_through_physical_pixels`（覆盖层定位依赖的反投影，含 ppp 抵消）。
- `scripts/m2a-edit-probe.ps1` 在真实窗口跑完整链路，11 张截图 + 临时键盘日志逐帧佐证：F2 → 粘贴「M2a 行内编辑」→ Enter 提交 → Ctrl+Z 撤销 → Ctrl+Y 重做 → Tab 加子节点「子主题」→ Enter 加同级「拼音: zhi si」→ F2 重开 + 粘贴 + Esc 取消（**日志里没有对应的 commit 行**，确认取消不落盘）→ Delete 删除。`shots/m2a/09-sibling-committed.png` 与 `11-deleted.png` 肉眼核对：同级节点在、删掉后无残留空节点。
- 壳层 golden 重拍：标题栏现在用雅黑，`shell-ppp1.50.png`（1620×1080 @ 1.50）变更后连跑两次**容差 0** 一致。

**五个坑**：

- **隐形控件混进 Tab 序列，症状却是「快捷键全废」**：8 条缩放热区用的是 `Sense::drag()`——egui 里 `Sense::drag()` / `click_and_drag()` 带 `FOCUSABLE`，而 `Sense::DRAG` / `Sense::CLICK` 常量组合不带。按一次 Tab，焦点就掉进一个看不见的热区，画布再也收不到键。日志里 `focused=Some("mw-resize-overlay")` 是唯一线索；**光看现象永远查不出来**。所有无键盘语义的隐形层一律用不可聚焦的常量组合。
- **`unwrap_or(true)` 挡不住 harness 的随机漂移**：golden 基线第一次重拍后，比对报 `1318 px / 包围盒 x[21..193] y[6..25]`——正是标题栏那行字。原因不是渲染不确定（临时目录自比两次干净），而是隐藏窗口的 `viewport().focused` 取决于**截图那一刻谁在前台**，标题栏 `dim` 在 1.0 / 0.55 之间跳。修法是 harness 里把 `focused` 钉成 true：验收基线必须与桌面状态无关，否则它会在别人改代码之前先自己漂。
- **egui 自带字体没有 CJK**：不装雅黑，行内编辑器与标题栏里的中文全是豆腐块，而画布（DirectWrite）一切正常——看起来像「编辑器的 bug」。装的必须是 `msyh.ttc` **索引 1**（UI 版），与 DirectWrite 用的族名一致，否则进入编辑会换字面、跳一行高。
- **度量近似值会骗人**：最初用「1 em = 汉字步进」手算节点内边距，报出 `pad=3` 的统一结果，据此以为文字溢出胶囊。实际是脚本把**蓝色连线**当成了节点填充；改成「取最长的连续饱和蓝段」并直接量像素后，编辑态与非编辑态尺寸一致。凡是「视觉可疑」的结论，先用像素量一遍再动手。
- **`--golden` 的退出码会被管道吃掉**：`... | tail -2` 之后 `$?` 是管道的状态。判断漂移要看输出文本，或单独跑不带管道。

**这一段故意没做**：鼠标点击定位 caret（进入编辑一律光标在末尾）、选区与拖拽选中文字、超长文本与粘贴换行的边界用例、跨提交的字符级撤销、方向键在节点间移动选择（M2c 的快捷键全集）、拖拽幽灵 / 对齐吸附 / 拖出连线 / 框选（M2b）。**C1 的人工矩阵仍未签字**（任务 #4）：探针走的是 SendKeys 粘贴，能证明「文本进得来、提交得了、组合期挂键」，但证明不了微软拼音 / 搜狗 / QQ / 微信输入法下候选窗真的贴在对的位置——这一步只能人眼做。

---

### 4.13 M2b-2：壳层接多选 + 框选 + 平移改键 + 编辑期锁缩放（2026-10-17）

M2b-1 把「拖拽手感」里最容易出错的那几件事拆成了纯逻辑 crate `mw-ui`（27 项单测，无窗口无 GPU）。这一节做的是**接线**：让壳层真的用它，并把 docs/06 §5 里「单击空白 → 拖拽 >4px 变框选」「空格 / 中键拖 = 平移」这两条从纸面变成手感。范围严格不含拖拽重排（那是 M2b-3）。

| 改动 | 内容 |
| --- | --- |
| `crates/mw-shell/Cargo.toml` | 加 `mw-ui` 依赖——它是 workspace 成员却一直没人依赖，接线缺口就在这里 |
| `canvas.rs::Canvas` | `selected: Option<u32>` → `sel: mw_ui::Selection`（有序、末尾主选）；新增 `drag: Option<Drag>` 与 `swallow_click` |
| `canvas.rs::handle_pointer`（替掉 `handle_zoom_and_pan`） | 按下定手势 → 拖动（平移 / 拉框）→ 松手提交；缩放与滚轮平移留在同一函数尾部 |
| `canvas.rs::rebuild` | 搬运的不是索引而是 **id**：收集旧选中的 id → 重排后逐个 `scene.find`，找不回的丢掉；整组都找不回才回落根 |
| `canvas.rs::delete_selected` | 一次 `DeleteNodes{ids}` 删整组（跳过根），删前先存主选的祖先链，删完落回第一个还活着的祖先 |
| `canvas.rs::paint_marquee`（新） | 虚线框 + 半透明填充，画在**合成纹理之上**的 egui 层（不进 `Frame`，否则离屏导出会带上橡皮筋） |
| `scripts/m2b2-canvas.ps1`（新） | 真实鼠标/键盘回归：框选 / 加选 / 减选 / 清空 / 整组删除 / 空格拖 / 中键拖 / 编辑期锁缩放，13 张截图 |
| `scripts/m1b-canvas.ps1`（修订） | 「左键拖空白 = 平移」这一步在新语义下已经不成立，改成中键拖；不加这一步，旧回归脚本会从今天起静默地说谎 |

**五条不显然的决定**：

1. **手势在按下瞬间定死，且用 `press_origin` 而不是 `drag_started()` 判定**。`Sense::click_and_drag()` 的 `drag_started` 要等指针越过 `max_click_dist`（6px）才报——那时候再决定「这是框选还是平移」已经晚了半拍，而且 6px 内用户看不到任何反馈。按下点用 `ui.input(|i| i.pointer.press_origin())`，命中判定用**按下点自己的** `hit_at`，不用本帧悬停（快速连点时两者会差一帧）。
2. **框选阈值 4px 必须自己判，且松手那一帧要吞掉 `clicked`**。docs/06 要求 4px，egui 的 `max_click_dist` 是 6px：位移落在 4–6px 之间时，用户已经看见橡皮筋、松手却仍被算作「单击空白」——不处理就是「框完了立刻被清空」。做法是 `swallow_click` 每帧复位的标志位（不是跨帧状态），在点击分支最前面 `mem::take` 掉。
3. **框选提交不靠 `drag_stopped()`**。同一个 4/6 差值：位移 4–6px 时 egui 根本不认为发生过拖拽，那个回调永远不会来，橡皮筋会一直挂在屏幕上。改成在 `any_released()` 那帧自己收尾。另外 Escape 会中止 egui 的拖拽但仍给一帧 `drag_stopped`，这条路径也得能干净地清掉状态。
4. **平移改为自己结算增量，不用 `drag_delta()`**。`drag_delta` 同样要等越过 6px 才开始，画布会永远落后指针 6px（拖起来像有橡皮筋）；改成「按下点 → 当前点」的增量，配 3px 死区吃掉按下抖动，第一个像素起就 1:1 跟手。中键拖不需要读原始事件：`is_decidedly_dragging` 与按钮无关，`Sense::click_and_drag` 对中键同样报 `dragged()`。
5. **编辑态只锁缩放、不锁平移、不开口子给框选**。缩放会让 caret 与候选窗错位（docs/06:108），平移不会；而编辑期拉框会把光标从节点里拽走。所以 `zoom_delta` 分支加 `&& !editing`，`classify` 在编辑态直接返回 `Pan`，滚轮平移照旧可用。

**验证**：

- `cargo test --workspace` **216 项全绿**（+9，全部落在 `mw-shell`：`classify` 4 条 + 选择搬运 5 条），`cargo fmt --all -- --check` 干净，`cargo clippy --workspace --all-targets` 新增代码 0 警告。
- 新增测试里有两条是**行为断言而不是实现断言**：`survivors_keep_their_order_and_primary_after_a_relayout` 先断言「删掉中间节点后丙的索引确实前移」（否则这条测试证明不了「搬的是 id」），再断言顺序与主选不变；`a_selection_that_vanishes_falls_back_to_the_root` 与 `an_empty_selection_stays_empty` 互为反例——一个要求回落根，一个要求**不**回落。
- 断言先证伪过：临时把 `restore_selection` 的 `!old_ids.is_empty()` 去掉、把 `classify` 的空白分支改成 `if middle || space || on_empty`，对应两条测试立刻红，改回后恢复全绿。
- `cargo golden` **容差 0** 通过（选择态为空，画布像素不该有任何变化——这一条同时证明橡皮筋没有漏进合成器）。
- `scripts/m2b2-canvas.ps1` 真实窗口跑通，13 张截图；逐像素比对给出硬证据：`edit-mode` 与 `edit-zoom-locked` 只差 **87 px、包围盒 3×29（1471,407）**——正是那根闪烁的 caret，缩放确实被锁住。对照组 `m2b2-zoom-before` vs `m2b2-zoom-free` 差 **348,702 px**（整个画布），证明同一套滚轮输入在非编辑态**确实**会缩放——没有这个对照，「锁住了」什么也证明不了。
- `scripts/m1b-canvas.ps1` 复跑通过：单击选叶子、点折叠点收子树、中键拖平移、Ctrl+= 放大，与 M1b 基线一致（单选取值改成 `Selection` 之后没有回归）。

**六个坑**（前三个是验收环境，后三个是 egui 语义）：

- **`SetForegroundWindow` 会静默失败，症状是「代码没反应」**：探针第一轮 8 张截图里有 4 张与基线**逐字节相同**，看着像框选压根没实现。真相是窗口没被拉前台，`CopyFromScreen` 抓的是压在织思上面的编辑器，点击也落在那个窗口上——**一次都没点进画布**。修法：`SetWindowPos(HWND_TOPMOST)` + `WScript.Shell.AppActivate` + `SetForegroundWindow`，然后**断言 `GetForegroundWindow() == hwnd`**，不相等直接告警（这一轮的图不可信）。收尾再 `HWND_NOTOPMOST` 还回去。
- **`.ps1` 带中文必须存成 UTF-8 with BOM**：Windows PowerShell 5.1 无 BOM 时按 ANSI 读，中文注释里的引号被吃掉一半，报的是「字符串缺少终止符」——错误位置还指向文件末尾。旧脚本全是英文注释所以从没踩到；新脚本一律 BOM + CRLF。
- **`mouse_event` 的滚轮量在第 4 个参数（dwData），不是 dy**：写成 `($WHEEL, 0, 120, 0, ...)` 时函数成功返回、什么也不发生，于是「编辑期锁缩放」这条测试会**假通过**（两边都没动）。是加了 A/B 对照组之后才暴露的：非编辑态同样的滚轮必须真的放大，否则测试无效。
- **`Response::dragged()` 对中键同样成立**，但 `drag_started()`/`drag_delta()` 要等 6px——中键平移别去读原始 `PointerEvent`，也别指望 `drag_started` 当「按下」用。
- **`hover_pos` 在指针离开面板后变 `None`，`interact_pointer_pos` 不会**：拖到画布外再松手是常态，收尾逻辑用后者。
- **重排后索引全变，选择必须按 id 搬**：`Scene` 的索引是布局 DFS 前序，删一个节点就能让后面所有索引前移一位。沿用旧代码「按索引取 id 再按 id 找回」只对单选成立；多选要整组搬，且**末尾（主选）顺序不能乱**——它决定 Tab/Enter/F2/Delete 作用在谁身上。

**这一段故意没做**：对齐吸附参考线、拖出连线（M2b-4，料备在 `mw_ui::snap`）；方向键在节点间移动选择、Ctrl+A 全选（M2c 快捷键全集）；右键菜单（M3）。

---

### 4.14 M2b-3：拖拽重排 / 改父 + 合成器幽灵 + 三分区落点（2026-10-17）

M2b-1 把落点判定做成了纯函数，M2b-2 把它旁边的手势（多选、框选）接进了壳层。这一节补上最后一块：**按住节点拖动 = 改结构**，并且拖的过程要看得见——半透明幽灵跟手、原位留残影、目标上出现三分区指示、落到自己肚子里变红。docs/06 §5 那条「拖拽节点」整行至此才算落地。

| 改动 | 内容 |
| --- | --- |
| `mw-render/src/instances.rs` | 新 `Ghost { nodes, offset, illegal }` + `Frame.ghost`；L2 里被拖的子树**原位淡化到 28%**，再在偏移处压一份 **55%** 的副本（节点盒 + 标签胶囊）；多节点时 L3 加一条并集虚线框；`ghost_mask()` 供几何与文字两条管线共用 |
| `mw-render/src/text.rs` | 原位的字形跟着盒子一起淡化（`dim()` 只乘 alpha，不动 RGB）；幽灵副本的字形整体平移一份——**文字不跟手等于幽灵是个空盒** |
| `canvas.rs::DragKind::Move` | 按下落在节点上=改结构。`Moving` 持有按下的树快照、拖拽根、子树集合、屏幕偏移与落点 |
| `canvas.rs::commit_move` | 松手时 `plan_moves` → `move_commands` → 一次 `apply`（多根走 `Batch`，一次撤销）；非法/原地不动则不产生命令 |
| `canvas.rs::paint_drop_target` | 三分区指示画在 egui 层：上/下缘 3px 条、整框 2px 描边 + 12% 填充；非法转红 |
| `scripts/m2b3-drag.ps1`（新） | 真实鼠标回归：改父 / 非法目标 / 同父重排 / 多选整组，四个场景 |

**七条不显然的决定**：

1. **幽灵归合成器，指示条归 egui**。幽灵要和真节点用同一套着色、圆角、DirectWrite 字形，只有合成器做得到（它手里有 `Scene` 和文本实例族）；三分区指示是 3px 条 + 2px 描边，全是**屏幕空间常量**，egui 天然不随缩放变粗，也不必为几个矩形新建一套实例。两边都在 `Frame` 之外——**导出不应该带上幽灵**，不变量 2 照旧成立。
2. **原位留残影，不是直接消失**。整块消失会被读成「节点被删了」，而用户明明在拖它。残影 28%、幽灵 55%，两个数都试过：残影再淡就看不出「从哪来」，幽灵再淡就被目标节点盖住。
3. **树快照在越过阈值那一刻拍，不是按下那一刻**。按下还可能是一次单击（<4px），提前建 `Tree` 会让单击多付一次 O(n) 的父子表。越过阈值时才 `Moving::start`，此后 `Scene` 不重排，落点索引的参照系就是稳的。
4. **文字必须跟着幽灵走**。第一步只做了盒子，截图一看就是「半透明的空框在飘」——中文节点里字占的面积比盒子还显眼。补法是给 `collect_jobs` 加一段幽灵 job：同样的整形缓存、加偏移、乘 alpha，几乎零额外成本（`shape` 命中缓存）。
5. **单节点幽灵不画并集虚线框**。第一版对任何幽灵都画，结果它和落点指示条同为强调色、叠在一起，看不出哪个是「将要落到哪」。改成 `nodes.len() > 1` 才画——那时它才有不可替代的作用：把散着的一组圈成「一个整体」。
6. **非法落点：幽灵照常跟手 + 轮廓转红 + 禁止光标**。旧 Svelte 版直接从命中里剔掉，用户拖到自己孩子头上什么提示都没有，以为拖拽卡住。这里三层反馈都给，但**不改幽灵本身的位置**——用户需要看见「我正拖着自己」。
7. **Esc 取消手势要吃掉按键**。原来 Esc 只是「清空选择」；拖拽中途按 Esc，如果照样往下走，会先取消拖拽再顺手把选择也清了——用户想的是「这一下不算」。现在 `handle_pointer` 开头 `consume_key` 掉它并直接返回。另外补了一条「指针已经不在按下状态」的兜底：窗口失焦时收不到 release，否则幽灵会永远挂在画布上。

**验证**：

- `cargo test --workspace` **219 项全绿**（+3，全在 `mw-render/tests/offscreen.rs`：幽灵副本 + 原位淡化、单/多节点的并集框差异、非法幽灵的警示色）。`cargo fmt --all -- --check` 干净，新增代码 0 clippy 警告。
- `cargo golden` **容差 0**：选择态为空、没有幽灵时画布像素一个不动——这一条同时证明幽灵没有漏进默认渲染路径。
- `scripts/m2b3-drag.ps1` 真实窗口四个场景，逐张截图核对：
  - **改父**：把「禁则换行」拖到「纯 Rust 原生」中央 → 成为它的子节点，重排后仍带选中环；
  - **非法**：把「纯 Rust 原生」拖到自己的子节点上 → 红色整框 + 红色并集框 + 禁止光标，松手后**树形一字未动**（对比 `m2b3-illegal-initial` 与 `-released`，差异只在选中环）；
  - **同父重排**：把「中文排版」拖到「纯 Rust 原生」上缘 → 蓝条出现在上缘，松手后它变成第一个子节点；
  - **多选整组**：框选三张卡后拖其中一张 → 三张一起跟手（并集虚线框），松手后三张一起挂到「交互与性能」下且顺序不变。
- 幽灵的中间态是真截下来的，不是推断：`m2b3-reorder-hover.png` 里能看到「中文排版」整棵子树（含 3 个子节点）在偏移处半透明重影、原位那份发白、目标节点上缘一条蓝条。

**五个坑**：

- **拖拽会改布局，硬编码坐标一轮就废**：第一版脚本按初始布局写好坐标，跑完第一步（改父）之后树重排，第二步的坐标落在空白处——于是「非法落点」测试变成了框选，还差点被当成通过。修法是场景开始时先按 `Home` 重排布（`fit` 只做一次，窗口尺寸变化后不会自己跟上），坐标才跨轮稳定；四个场景各自独立起一次进程。
- **按下时选中的节点若不在选中组里，必须先换选中**：单击的选中发生在**松手**那一帧，而拖拽会把那次 click 吃掉——不提前选，拖的会是上一次的选择。
- **`pointer.any_released()` 不足以收尾**：窗口失焦、别的窗口抢走鼠标时收不到 release，`drag`/`moving` 会一直留着，幽灵停在空中。用 `!any_down()` 兜底。
- **`f.editing` 的节点不能出幽灵文字**：那个节点的字形归 egui 编辑器，合成器再画一份就是重影（和 M2a 同一条约束，幽灵这段也得守）。
- **多根拖拽的根要先去掉「祖先也被选中」的**：否则父的幽灵带着子，子又被单独移一次，落点索引会差位。`mw_ui::drag_roots` 就是干这个的，别在壳层另写一遍。

**这一段故意没做**：拖到空白处**分离为自由主题**（docs/06 §5 的终态，需要一个「脱离父级」命令——`MoveNode` 必须有父；现在松在空白是干净的 no-op）；Ctrl+拖拽复制（docs/12 E1，P0 但属 M2 后续）。

---

### 4.15 M2b-4：对齐吸附参考线上屏 + 拖出连线（2026-10-17）

拖拽的手感由两件事决定：**落到哪儿**（上一节的幽灵与三分区）和**停在哪儿**（这一节的吸附与对齐）。同时补上「节点之间的线」：从节点四边的锚点拉一条曲线到另一个节点，松手就是一条关系线。至此 docs/06 §5 的输入表里，除「拖到空白分离自由主题」外全部有实现。

| 改动 | 内容 |
| --- | --- |
| `mw-render/src/instances.rs` | 新 `GuideLine { at, from, to, vertical }` + `Frame.guides`；L3 最后画 1 CSS px 的强调色细线（`cam.chrome(1.0, 0.3)`） |
| `mw-ui/src/link.rs`（新） | `Side`（四边）+ `anchor_point` / `anchor_at`（最近锚点、屏幕半径换算世界单位）/ `target_node`（包含指针的面积最小节点，排除起点自己） |
| `canvas.rs::Moving` | 加 `union`（幽灵并集盒）、`snap`、`snap_since`；拖动时对**其余节点**求 `snap_offsets`，把 `dx/dy` 计入 offset |
| `canvas.rs::snap_ready` | 60ms 延迟的判定，纯函数（可断言） |
| `canvas.rs::DragKind::Link` | 按住四边锚点拖：`Link` 态记起点/方向/当前点/目标；松手发 `Command::AddRelationship` |
| `canvas.rs::paint_anchors` / `paint_link` | 悬停节点露出四个锚点小圆点；拖动时画预览贝塞尔 + 目标节点环 |
| `scripts/m2b4-canvas.ps1`（新） | 真实鼠标回归：吸附（参考线 + 拉齐）与拖出连线，含 Esc 取消 |

**六条不显然的决定**：

1. **参考线归渲染层，但 `GuideLine` 是渲染层自己的类型**。让 `mw-render` 去认 `mw_ui::snap::Guide` 会让「渲染层不依赖手势层」这条分层破掉。字段一一对应，壳层做一次三行转换——比让渲染层认识「吸附」这个概念便宜得多。
2. **吸附只做「同类线配对」**（左↔左、中↔中、右↔右），这是 `snap_offsets` 既有的口径，也是 docs/11 写明的口径。所以「把 A 的左缘贴到 B 的右缘」**不会**吸——这是取舍不是 bug（要做 any-to-any 是另一个决策，得连测试一起改）。第一版验收脚本恰好挑了「左缘贴右缘」，于是看起来像功能没实现。
3. **参照集必须剔除被拖的子树**，否则它永远以自己为参照，一动就吸上。
4. **60ms 延迟才是「吸附」而不是「粘连」**：候选出现先记账，稳定存在 60ms 才生效。这段的可见效果是「快速掠过不粘」，而 60ms 内的两帧既截不到图也数不出来——所以抽成 `snap_ready(since, now)` 用断言锁住（`0ms`/`30ms` 不吸、`60ms` 吸）。
5. **锚点优先于节点本体**：锚点就长在节点边上，`classify` 里不先判它，「想拉线」每次都会变成「搬节点」。命中半径 7 CSS px 再换世界单位，缩放档位不同也点得中。
6. **锚点只在悬停时露出**。四个小圆点常显会让整张图变成「一堆把手」；悬停露出来既有发现性又不吵。连线预览用三次贝塞尔，控制柄沿锚点法线出——否则线会贴着节点边缘斜出去。

**验证**：

- `cargo test --workspace` **227 项全绿**（+8：`mw-ui` 的 link 5 项、`mw-render` 的参考线 1 项、`mw-shell` 的延迟 1 项 + 锚点手势 1 项）；`cargo fmt --all -- --check` 干净，新增代码 0 clippy 警告，`cargo golden` **容差 0**。
- `scripts/m2b4-canvas.ps1` 真实窗口：
  - **吸附**：把「导出」拖到「交互与性能」上缘附近，400ms 后截图里出现一条横贯的强调色参考线，幽灵顶边与目标顶边**逐像素对齐**（629–630 同一行），Esc 取消后回到原位、树形不变；
  - **拖出连线**：悬停「中文排版」露出四个锚点 → 从右锚点拖到「交互与性能」→ 预览曲线跟手且目标节点亮环 → 松手后文档里多出一条关系线（截图 `m2b4-link-created.png`）。
- 参考线的「屏幕空间恒定」有单测兜底：100% 缩放下线宽 1，200% 下世界单位线宽 0.5。

**三个坑**：

- **`chrome(css, world_floor)` 的第二个参数是「最小世界单位」，不是「最小像素」**：给参考线写 `chrome(1.0, 1.0)` 会让 200% 缩放下线变成 2px（世界 1 单位 × 2）。应当是 `0.3`——只在极小缩放下兜底，不干扰正常档位。这条是单测抓出来的（断言 200% 下 = 0.5）。
- **先看口径再设计验收用例**：同类线配对意味着「左缘贴右缘」不吸，我第一版恰好测了这个，差点把「没吸」当成实现错误去改代码。
- **锚点命中要把屏幕半径换成世界单位**：直接拿 7 当世界单位，在 0.5× 缩放下锚点几乎点不中（7 世界单位 = 3.5 屏幕 px）。

**这一段故意没做**：拖到空白处分离自由主题（缺「脱离父级」命令）、Ctrl+拖拽复制（docs/12 E1）、连线中点拖动改形状、连线标签编辑（右键菜单，M3）、吸附的「距离标注」（docs/12 D5 的进阶项）。另外**性能上有一条已知上界**：`snap_offsets` 每帧遍历全部可见节点算六条线，5k 节点时是 3 万次比较/帧——还没做空间索引，等 M5 性能打磨时按 P5 预算一起量。

---

### 4.16 M3-a：文档会话撤出画布 + 打开/保存/另存为（2026-10-17）

M2b 让画布「能改」，但改完关掉就全没了——这不算工具。M3 的第一刀不是面板，是**文件**：把文档从画布里抬到壳层，接上 `mw-format` 的落盘与原生文件对话框。这一节做完，「打开磁盘上的 `.zmind` → 改 → `Ctrl+S` → 关掉 → 再打开还在」这条最基本的闭环才成立。

| 改动 | 内容 |
| --- | --- |
| `mw-shell/src/session.rs`（新，242 行 / 5 测试） | `Session { path, doc, settings, theme, saved_revision }`；`open` / `save` / `save_to` / `title` / `dirty`；夹具文档（原 `canvas.rs` 末尾的 `sample_document`）搬到这里并成为「未命名文档」的初始内容 |
| `mw-shell/src/files.rs`（新，104 行） | Win32 `IFileOpenDialog` / `IFileSaveDialog` 封装，返回 `Result<Option<PathBuf>>`——「用户取消」是 `Ok(None)`，不是错误 |
| `mw-shell/src/canvas.rs` | 字段 `doc/settings/theme` → `session`（内部访问改 `self.session.doc` 等）；新增 `session()` / `replace_session()` / `save()` / `save_to()` |
| `mw-shell/src/main.rs` | 窗口级 `Ctrl+O` / `Ctrl+S` / `Ctrl+Shift+S`、标题栏真实文件名 + 未保存点、关闭拦截、左下状态条、`MW_OPEN` 启动即打开 |
| `scripts/m3-file-e2e.ps1`（新） | 真实输入回归：打开→加节点→保存→重启仍在；以及 Ctrl+O 弹框、Alt+F4 拦截 |

**六条不显然的决定**：

1. **会话必须离开 `Canvas`**。`Canvas` 之前自己持有 `doc`，于是「打开/保存」无处落脚：壳层拿不到文档，画布也不该管路径。抬到 `Session` 之后，画布只负责「画出来」和「把命令发进去」，落盘只有一个入口。
2. **脏标记只看修订号差**（`saved_revision != doc.revision`），不看「文本变过没有」。`Document::revision` 每条成功命令自增，是唯一不会骗人的口径；用「有没有操作过」会在撤销回原状时仍然显示未保存。
3. **夹具文档按「已保存」对待**。否则启动第一帧标题栏就顶着 `•`，golden 基线也跟着漂。新建文档的语义（一动手就脏）由命令路径保证，不需要让初始状态假装脏。
4. **文件对话框用 `windows` crate，不引 `rfd`**。一个对话框不值得多一棵依赖树；`rfd` 在 Windows 上包的也是这同一个 COM 接口。封装只做「拿到路径」，`.zmind` 的读写全在 `mw-format`。
5. **文件快捷键挂窗口级，且必须 `consume_key`**。现在的快捷键都在 `canvas.rs::shortcuts` 里且要求画布有焦点；文件操作不该有这个前提（编辑中也得能存）。消费掉是为了防止同一帧画布的键盘判断再处理一次。
6. **两个丢数据的入口都要拦**：关窗（`CancelClose` + 确认框）与「打开另一份文档」（脏就弹「先保存 / 直接打开 / 取消」）。只拦关窗等于给 `Ctrl+O` 留了一个一键吞改动的后门。

**验证**：

- `cargo test --workspace` **232 项全绿**（+5，全在 `mw-shell::session`：标题/脏标记、命令即脏、保存→重开往返、无路径时 `save()` 报错、打开不存在的文件不 panic）。`cargo fmt --all -- --check` 干净，新增代码 0 clippy 警告，**`cargo golden` 容差 0**（标题栏没变，说明脏标记没有污染基线）。
- `scripts/m3-file-e2e.ps1` 端到端实拍（截图在 `shots/e2e/`）：
  1. `node scripts/make-sample.mjs` 生成一份 **Node 写的** `.zmind` → `MW_OPEN` 启动 → 标题栏显示 `演示.zmind`、画布显示 Node 生成的内容（同时验证了跨实现互读）；
  2. `Tab` 加一个节点 → 标题立刻出现 `•`（脏标记）；
  3. `Enter` 提交 → `Ctrl+S` → 左下角出现「已保存 演示.zmind」、`•` 消失（有路径时不弹对话框）；
  4. **杀掉进程再打开同一份文件** → 新节点还在：改动真的落盘了，不是内存幻觉。
- `Ctrl+O` 弹出系统原生「打开织思文档」对话框（截图 `m3-open-dialog.png`），点取消后文档不变；脏状态下 `Alt+F4` 被拦下并弹出「还有未保存的改动 / 保存并关闭 · 不保存 · 取消」（`m3-close-guard.png`）。

**四个坑**：

- **对话框不给 owner 就跑到主窗口背后**。第一版 `Show(None)`，对话框没有父子关系；而验收探针会把主窗口设成 TOPMOST，于是对话框被压住——截图里什么都没有，现象像「Ctrl+O 根本没反应」。必须传主窗口 HWND（`eframe::Frame` 的 `HasWindowHandle`，和 `apply_backdrop` 同一条路子）。
- **那个原生对话框上 Esc 不可靠**：探针按 Esc 没关掉它，后续所有按键都进了对话框（三张截图逐字节相同）。改成点「取消」按钮才继续。验收脚本要么点按钮，要么干脆别在这类对话框上做连锁步骤。
- **`MW_OPEN` 这类测试钩子值得留**：没有它，打开路径只能靠人手点对话框，自动化验收无从谈起；它同时是将来「双击 .zmind 打开」的入口。
- **`Session::sample()` 要显式标成已保存**：不然 golden 会因为标题栏多一个 `•` 而漂——基线必须与「文件是否新鲜」无关。

**这一段故意没做**：拖放文件入窗、最近文件、多标签、自动保存与崩溃恢复（`docs/14 §6` 里排在第 7 位，依赖会话结构稳定）、导出菜单（M4）、面板（大纲/检查器/命令面板）。另外**没有做「新建」**——现在启动即是夹具文档，`Ctrl+N` 要等空白文档的语义定下来（docs/05 的根节点约定）再做。

---

### 4.17 M3-b：拖放打开 + 最近文件 + 新建空白 + 自动保存/恢复（2026-10-17）

M3-a 让文档「存得下来」，这一节让它「丢不了」：拖文件进窗口就能开、标题栏能翻最近、`Ctrl+N` 从空白开始、编辑过程中每 3 秒往恢复点写一份。最后一项是这节的重点——它不是「顺手加个定时器」，而是**要先把"空闲时根本不跑帧"这件事想明白**。

| 改动 | 内容 |
| --- | --- |
| `session.rs` | `blank()`（一个「中心主题」根，按已保存对待）、`accepts(path)`（扩展名分流）、`import(path)`（`.md/.markdown/.opml/.mm/.txt/.xmind` → 新会话）、`open_recovered(recovery, doc)`（内容来自恢复点、**路径指回正本**） |
| `recent.rs`（新，~200 行 / 4 测试） | `Recent`：`%APPDATA%\织思\recent.json`，8 条上限、去重前置、坏文件降级为空、`prune_missing` 清理已删项 |
| `autosave.rs`（新，~160 行 / 4 测试） | `slot_for` / `write` / `clear` / `pending_for`；恢复点写 `<文档目录>/.autosave/<stem>.autosave.zmind`，未命名文档写 `%APPDATA%\织思\autosave\` |
| `main.rs` | `PendingAction{Open,Import,New}` + `request()`（脏就挂起）、`Ctrl+N`、拖放分流、标题栏 `▾` 最近文件菜单、每 3 秒自动保存、启动恢复提示 |

**六条不显然的决定**：

1. **三个会换掉当前文档的入口共用一条 `request()`**。打开、新建、导入都会让手里这份消失，没有理由只拦其中一个——第一版只拦了「打开」，`Ctrl+N` 是条后门。
2. **最近文件写 `%APPDATA%`，不写文档目录**。用户的资料夹不该多出一个只对本机有意义的 `recent.json`；同理，列表坏了就当空的，不能因为缓存文件挂了起不来。
3. **"新建"是干净文档**（`saved_revision = Some(rev)`）。新建就该没有未保存点，`Ctrl+S` 时才弹另存为；夹具文档 `sample()` 同理，否则 golden 基线会跟着脏标记漂。
4. **恢复点永远不碰正本**。它写在 `.autosave/` 子目录里，只有用户点「恢复」后按 `Ctrl+S` 才写回；「正常保存」会顺手删掉恢复点，否则下次启动又弹一遍，比丢数据更烦。
5. **只认"比正本新"的恢复点**。mtime 比较放在 `pending_for` 里，正常退出后再启动不会被打扰。
6. **自动保存必须自己催帧**（`ctx.request_repaint_after(400ms)`）。这是本节唯一一个"看起来能跑、实际完全不工作"的地方，见下面的坑。

**验证**：

- `cargo test --workspace` **244 项全绿**（+12：session 4、recent 4、autosave 4）。`cargo fmt --all -- --check` 干净，新增代码 0 clippy 警告。
- `cargo golden` 容差 0；标题栏新增的 `▾` 菜单按钮是有意外观变化，基线**重拍并连跑两次**确认确定。
- `scripts/m3b-shell-e2e.ps1` + `m3-file-e2e.ps1 -ShotOnly` 实拍（`shots/e2e/`）：
  - `Ctrl+N` → 空白「中心主题」+ toast「已新建空白文档」；
  - 标题栏 `▾` → 列出「演示.zmind」（`recent.json` 里确有该条）；
  - 加节点后等 4 秒 → 磁盘上出现 `.autosave/演示.autosave.zmind`（并且随着继续编辑又写了一次，`.bak` 是证据）；
  - **`taskkill /F` 模拟崩溃** → 重新打开同一份文件 → 弹「发现未保存的恢复点」→ 点「恢复」→ 编辑过的两个空节点回来了，标题带 `•`，toast 提示「Ctrl+S 写回原文件」。

**四个坑**：

- **空闲时 egui 不跑帧，定时器永远不会到期**。第一版 `autosave_tick` 只是"每帧看一眼时钟"，结果编辑完等 4 秒，磁盘上一个字节都没多。修法是脏状态时 `request_repaint_after(400ms)` 主动催帧——不修的话这个功能**看起来实现了，实际从未触发**。
- **并行测试不能用毫秒当临时目录名**：`mw_core::now_ms()` 在同一毫秒内会撞车，两个用例共用目录、互相删文件，报的是「拒绝访问 (os error 5)」，看起来像保存功能的 bug。改成 `进程 id + 自增序号`。
- **`MW_OPEN` 走的是 `Shell::new` 而不是 `open_path`**，第一版忘了在那儿记最近文件，于是"打开过的文件不在最近里"。
- **拖放拿到的 `DroppedFile` 没有 `path` 字段**（0.37 是 `path()` 方法且返回 `&Path`），`find_map(|f| f.path())` 编译不过；同时要过滤空路径（字符串拖放没有文件系统路径）。

**这一段故意没做**：**拖放打开没有自动化验证**（要从资源管理器拖，探针做不到；代码路径与「打开」共用 `request()`，逻辑上等价，但没实拍过）；多标签、标签栏拖动、窗口状态记忆；面板（大纲/检查器/命令面板）；`Ctrl+N` 的"模板"语义。自动保存目前是固定 3 秒，没有"每 M 条命令"的触发条件。

---

### 4.18 M3-c-a：大纲面板 + 状态栏 + 命令面板（2026-10-18，交接中断点）

产品第一次有了"不是一个画布"的界面骨架：左侧大纲、底部状态栏、`Ctrl+K` 命令面板。这一节做到一半停下（见文末"下一步"），交接时按此节为准。

| 改动 | 内容 |
| --- | --- |
| `panels/mod.rs` + `panels/outline.rs`（新，~300 行 / 3 测试） | 大纲面板：`Row` 行缓存（**只在 `doc.revision` 或折叠指纹变化时重建**）、显式栈遍历（防深树爆栈）、`ScrollArea::show_rows` **虚拟滚动**（只实例化可见行）；折叠箭头、选中高亮 + 左侧指示条、空节点灰字"（未命名）"、折叠行尾"…" |
| `panels/command.rs`（新，~260 行 / 5 测试） | 命令面板：11 条命令常量表、子串过滤（中文可用，快捷键提示也可搜）、`↑↓/Enter/Esc` 键盘优先、绘制走 `Order::Foreground` 的 `Area`（调用早于画布也能盖在最上） |
| `canvas.rs` | 新公开 API：`selected_ids` / `zoom` / `select_node`（选中 + 相机居中 + `fitted=true` 防首帧 fit 拽回）/ `toggle_collapsed`（走命令层）/ `fit_view` / `zoom_by` / `undo` / `redo` / `delete_selected` 转 pub、`set_input_blocked`（浮层盖住时画布"聋"掉）、`last_viewport`（fit 用） |
| `main.rs` | `Panel::left("outline")`（232px，不可拖拽）+ `Panel::bottom("status")`（24px）；`F8` 切大纲、`Ctrl+K` 开面板；`run_palette` 把面板动作落地；状态栏显示保存状态/节点数/选中数/缩放 |

**性能与视觉，这次是这样保的**（这是用户核心目标，以后每一节都要回答）：

- **帧耗时**：大纲缓存不命中时每帧只做两次整数比较；虚拟滚动让 5k 节点的滚动区每帧只画十几行。这是"流畅顺滑"在大文档下的保险。
- **内存**：行缓存只存 `id/文本/深度/两个标志`（5k 节点约 1MB），不克隆 `Node`。命令表是编译期常量，零分配。
- **体积**：面板全部走 egui 即时模式，没有引入任何新依赖（检查过 `Cargo.toml`，只加了 `mw-io/serde`，那是 M3-a/b 文件功能的）。
- **视觉**：面板底色 `rgba(252,252,253,236)` 让 Mica 透出；选中用主题强调色 10% 底 + 2px 指示条而不是整行变蓝；命令面板圆角 12 + 28px 阴影 + 快捷键提示右对齐——都刻意贴 MD3 的密度与 Apple HIG 的克制。

**验证**：

- `cargo test --workspace` **252 项全绿**（+8：outline 3 + palette 5）；`cargo fmt --all -- --check` 干净；`cargo golden` **重拍后连跑两次容差 0**（布局加了面板与状态栏，是有意外观变化）。
- `scripts/m3c-panels.ps1` 实拍（`shots/e2e/`）：点大纲第 8 行 → 该行高亮 + 画布选中环 + 相机居中 + 状态栏"选中 1"；点「中文排版」箭头 → 大纲少 3 行、画布折叠、状态栏变"● 未保存"；`F8` → 面板收起、画布撑满；`Ctrl+K` → 面板浮层（截图）→ `Enter` 执行第一项「新建文档」→ 空白文档 + toast（截图）。

**性能基线的第一次实测（M5 预算的起点）**：

| 指标 | 实测（1080×720、夹具 13 节点、150% DPI） | 说明 |
| --- | --- | --- |
| 安装体积 | **exe 17.6 MB**（未 strip、未 UPX） | 对比 Electron 版 `织思.exe` ~90MB + `ui/dist`；对标目标 <15MB（strip 后大概率达成） |
| 工作集 | 375 MB | 大头是 wgpu/DX12 图形栈与共享驱动页 |
| 私有内存 | **318 MB**（历史：M3 实测基线；P13 A+B 后空应用 276.6MB，见 §4.33） | 已知占用源：字形图集 4096² R8（当时 CPU+GPU 各 16MB；P13 起 3072² 且无 CPU 镜像）、渲染目标 1620×1080 RGBA、DX12 后端基线 ~200MB |
| 线程 | 19 | 全是图形栈（wgpu 的 DX12 后端 + winit 事件泵），业务代码没有自己开线程 |

**M5 要立的预算**：空文档私有内存 < 200MB（先动字形图集到 2048²，再查 DX12 与 Vulkan 后端的差值）；安装包 < 15MB（`strip = true` + 图标压缩后重测）。**量法**：`Get-Process mw-shell` 的 `PrivateMemorySize64`，冷启动 5 秒后读。

**四个坑**：

- **`Color32::from_rgba_unmultiplied` 不是 const fn**，写进 `const` 直接编译错。面板底色改成了 `panel_bg()` 函数。
- **egui 0.37 的 `DroppedFile` 是 `path()` 方法返回 `&Path`**（不是字段）；`Ui::id` 已弃用，要用 `scope_id()`；`Area` 的 id 用 `Id::unique`。
- **大纲遍历不能用递归**：显式栈。自由主题根要先压底再压子节点（栈是后进先出），出栈顺序才与 `doc_order` 一致——第一版顺序反了。
- **egui 空闲不跑帧**（M3-b 的坑换了件衣服又来）：挂在每帧上的 `Outline::sync` 没事，但任何"每 X 秒"的后台逻辑都必须自己 `request_repaint_after` 催帧，否则永远不会触发。

**下一步（接手从这里开始）**：

1. **检查器面板**（右，检查器读 `canvas.session()` + 发 `SetStyle/SetNote/SetLink/SetIcon`，多选批量）——**这是本轮唯一没动的"像 Xmind"的关键件**；✅ 已完成，见 §4.19；
2. **小地图 + 状态栏选中路径**；✅ 已完成，见 §4.19；
3. **多标签**（`Vec<Canvas>`，注意每个 Canvas 各建一份字形图集 ~16MB，要么共享 `TextEngine` 要么先不做）；
4. **M2c 快捷键全集 + 动效表**；
5. 拖放打开的人工验收（拖一个真 `.md` 进窗口）。

---

### 4.19 M3-c-b：右侧检查器 + 小地图 + 状态栏选中路径（2026-10-18）

产品有了完整的编辑入口：右侧 260px 检查器（`F9`）、右下小地图（可折叠成角标；视觉 v2 后为 190×130）、状态栏选中路径。**M3 至此只剩多标签**；"改样式只能改代码"这句话不再成立。

| 改动 | 内容 |
| --- | --- |
| `panels/inspector.rs`（新，1487 行 / 7 测试） | 四个页签：**样式 / 备注 / 链接 / 标记**。无选中 → 文档属性（标题只读、主题宫格、布局 3 选 + 间距/最大宽度滑杆，拖动期间不发命令）；单选 → 形状/填充/文字色/字号/粗斜体/子树文字、备注、链接、12 个图标、标签；多选 → 每项显示**混合态**，改动按节点各自的 `StyleOverride` 合并后打包 `Batch`。备注草稿在 `doc.revision` 变化时**不覆盖正在写的字**，但干净草稿会跟随外部写入；备注 `Ctrl+Enter` 提交 / `Esc` 取消，链接 `Enter` 提交。图标与标签走 `SetIcon/AddLabel/RemoveLabel`，上限沿用 `MAX_LABELS=5` / `MAX_LABEL_LEN=64` |
| `canvas.rs` | 新公开 API：`apply_command`（面板命令唯一入口，返回是否改了文档）、`request_canvas_focus`、`selection_path`（根→主选，空白节点显示"未命名"）、`minimap(card)`、`set_overlay`。小地图用 `Scene` 直接画：`draw_order` **等步长确定性抽样到 ~900 个矩形**（`MINIMAP_NODES=900`）、分支色块 + 根描边 + 选中环 + 视口淡染；点击/拖动把指针映射回世界坐标移动相机，光标 `Grab/Grabbing` |
| `main.rs` | `Panel::right("inspector")`（260px，不可拖拽）+ `F9` 开关；检查器动作回了闭包再执行并 `request_canvas_focus()`，避免键盘焦点留在面板；小地图卡片由 `minimap_rect(central, open)` 统一定位（右下 16px 边距、折叠后角标；v2 后为 34×26），画布先注册 `overlay` 矩形，浮层下的按下/单击/滚轮不再漏进画布；状态栏加选中路径（48 字截断、悬停看全文）。`sync_accent` 让 egui 的选中/链接/焦点环跟主题薄荷色，不出现默认蓝 |

**这一节修掉的三个交互回归**（都是真实输入探针抓到的，单测抓不到）：

- **Ctrl+点击把多选清空**：按下阶段看到"点中的节点不在组里"就把选择收成单选，松手时 additive 分支又把它 toggle 掉。修法：additive 按下**不窄化选择**；真拖过阈值时再以光标下节点为准收成单选，保证幽灵不拖错人。
- **小地图的点击漏进画布**：浮层画在画布之后，但画布仍先收到 pointer。修法：`set_overlay(rect)` + 三处闸门（按下起点、`clicked_by(Primary)`、滚轮缩放），浮层区域内的输入画布一概不接。
- **检查器里按 Enter 会多出一个同级节点**：备注 `Ctrl+Enter` / 链接 `Enter` 提交后焦点回到画布，同帧的快捷键又把 Enter 解释成"加同级"。修法：`just_refocused` 一帧护栏，重新拿焦点的那一帧跳过画布快捷键。

**性能与视觉**：

- **内存**：冷启动 7 秒私有 **318 MiB**（333,803,520 B，与 M3-c-a 持平）；exe **18.0 MB**（18,906,112 B，+0.4 MB）。检查器只读主选字段、不克隆 `Node`；小地图最多画 ~900 个矩形，几何仍只有 `Scene` 一份。
- **视觉**：面板沿用 M3-c-a 的 `rgba(252,252,253,236)` 让 Mica 透出；图标是 32×32 印刷符号按钮、选中描主题色环；节点上的备注/链接/图标/标签指示仍是**右上角小圆点**，不在画布里加噪声。检查器宽度 260 而不是设计稿的 300，是为了 1080p 下画布不被挤扁。

**验证**：

- `cargo test --workspace` **261 项全绿**（+9：inspector 7 + canvas 2）；`cargo fmt --all -- --check` 干净；`cargo clippy --workspace --all-targets` **本里程碑新增代码 0 警告**（存量 38 条与本节无关，见 docs/14 §9-12）。
- `cargo golden --update --warmup 5` 重拍（右侧面板 + 小地图是合法外观变化），随后 `cargo golden --tol 0` **连跑两次容差 0**。
- `scripts/m3c2-inspector.ps1` 在 150% DPI 真实窗口跑完整链路，14 张截图 `shots/e2e/m3c2-*.png`：选中 Rust → 填充改蓝；备注页写草稿 → `Ctrl+Enter` 提交（节点数仍 3，没有多出同级）；链接页粘贴 URL → `Enter` 提交（两个指示点）；标记页点 ① + 加标签"待确认"；Ctrl+点第二个节点 → "2 个主题 / 多选·改动批量应用"；小地图拖拽平移、折叠成角标、再展开；`F9` 隐藏/显示检查器——全程选择不丢。
- 多选加选的**回归**单独验过：Ctrl+点第二个节点 → 选中 2；再 Ctrl+点它 → 选中 1。

**已知缺口（下一节如实带过去）**：

- 检查器面板自绘控件用固定浅色（灰阶文字/浅底），**深色主题下不跟随**；画布侧 4 套主题在本轮视觉 v2 后均可切换（§4.20），但面板 chrome 仍是浅色，切到深色主题会露馅。
- 标签输入框回车添加后**不自动聚焦**，连续加标签要再点一次；备注/链接没有"打开"按钮（链接只存储与显示）。
- 小地图折叠按钮只有 17×17，探针第一版点在 785,544 时打偏、把小地图当成画布拖了；已把命中改到 792,552，但**按钮本身仍偏小**，是待改的 UX 项。折叠后的 32×24 角标也偏含蓄。
- ~~仍然没有动效（折叠/新增/面板开合都是硬切，M2c）~~ → **已补齐**：面板/搜索动效见 §4.26，结构过渡见 §4.30。

---

### 4.20 视觉 v2：以本机 XMind 2026 实拍为基准的壳层与画布重做（2026-10-18）

用户在本机安装了 XMind 2026 并保持打开，指出旧文档描述的是 2020 年的观感、当前界面"太丑"。本轮不改功能面，只把**观感与可发现性**拉到 2026 基准，并留下可复跑的验收。

| 改动 | 内容 |
| --- | --- |
| 主题 v2（`mw-core::theme`） | 4 套主题（`snow` / `mint` / `inkNight` / `forest`）统一成 2026 卡片体系：根节点白底 22px、一级 17px、二级 15px、三级及以下 14px；**一级主题直接用 `palette_fill` 实色卡片**（不再是一排白盒），根节点保持白卡 + 1.5px 边；画布底 `#FBFBFC`、连线 1.5px（根 2.0px）。级联仍只有一份：`resolve_all`，节点覆盖优先于主题。 |
| 画布铬件（`mw-shell::canvas`） | 选中环 2px + 3px 间隙（沿用合成器常量）；选中/悬停节点右侧露出**蓝色「＋」加子主题按钮**（与 Tab 同源，按下直接进编辑）；悬停四边锚点露出关系线入口。小地图改成 190×130 的白卡 + 8px 圆角 + 柔和投影，视口外区域不再压暗成灰板；折叠角标 34×26。 |
| 标题栏（`mw-shell::main`） | 顶栏 56px：左侧两行标题（文件名 + 保存状态），中间 **40px 白色悬浮胶囊工具条**（6 个 34×34 矢量按钮，悬停淡灰底），右侧 46px 原生窗口按钮；窗口变窄时胶囊整条收起而不是挤压标题。工具条与按钮区吃掉拖拽，点按钮不会把窗口拖走。 |
| 汉堡菜单 | 258px 白卡 + 12px 圆角 + 1px 边 + 26px 柔和投影；行高 30px、悬停 `#F1F3F6`；**矢量图标**（新建/打开/保存/另存为/文档/面板）+ 右对齐快捷键；「最近文件」分段标题 + 最多 5 条记录；大纲/检查器带勾选态。替换 egui 默认菜单样式（默认菜单的 `button_padding=(2,0)` 会让右侧快捷键贴边）。 |
| 检查器（`panels/inspector.rs`） | 四个页签的正文统一 14px 左缩进；备注/链接/标签输入框改为**可见的浅灰描边输入框**（`#FAFBFC` 底 + `#E1E5EB` 边 + 6px 圆角），修掉"白底白框看不见输入区"；备注/链接提交逻辑不变。 |
| 最近文件（`recent.rs`） | 修掉菜单里同一文件出现两行：历史数据同时存过相对路径与绝对路径。`load_from` 读盘时归一化为绝对路径并用 canonicalize 去重，`touch`/`forget` 同样用 `same_file()`；新增 1 项单测。 |

**验证**：

- `cargo test --workspace` **262 项全绿**（+1：最近文件相对/绝对路径折叠）；`cargo fmt --all -- --check` 干净；`cargo clippy --workspace --all-targets` 仍是**存量 38 条**，本轮新增代码 0 条。
- golden 重拍：`cargo golden --update --warmup 5`（壳层）与 `MW_GOLDEN_UPDATE=1 cargo test -p mw-render --test text_render text_canvas_matches_golden`（文字基线），随后壳层 `cargo golden --tol 0` **连跑两次容差 0**；文字基线在重拍前后各渲一次 SHA256 一致，证明不是抖动。
- release 真实窗口（1620×1080 @150%）截图 5 张 `shots/qa/final-{initial,menu,selected,inspector,inspector-fit}-2026.png`：默认只开大纲、菜单最近文件只剩一条、点节点出现选中环与「＋」、`F9` 检查器四页签、适应画布后整图可见。
- 交互动作用手点验过：菜单「检查器」与 `F9` 都能开合；改填充/备注的完整链路在 debug 实例验过（release 本轮只验开合、选中与适应画布）；小地图拖拽与折叠正常。

**已知缺口（留给下一步）**：

- **面板开合不重算相机**：`F9` 打开检查器后画布变窄，原先居中的图会被右边裁掉，需要按 `Home`/适应画布。应在面板宽度变化时平移相机保持视觉中心（不动缩放），排进 M2c 壳层收尾。
- 工具条只有撤销/重做/加子/加同级/删除/适应画布 6 个入口；布局、主题、搜索、导出仍没有按钮位。
- 汉堡菜单只有文件四项 + 最近文件 + 两个面板开关；没有"编辑/视图/插入"分组。
- 检查器 chrome 仍固定浅色，`inkNight` 下不跟随；深色主题的面板配色要单独做。
- ~~没有动效（菜单开合、面板宽度变化、折叠都是硬切）~~ → **面板/搜索动效见 §4.26、结构过渡见 §4.30**；菜单首帧的 egui fade-in 在截图上会呈现半透明灰底，稳定后才是白色。

### 4.21 搜索条：Ctrl+F 检索与定位（2026-10-18）

检索核心（`mw-core::search`）早在 M2 就位（§5 C4），缺的一直是**入口**。本轮把它接到界面上：`Ctrl+F` 弹出画布顶部浮卡，输入即搜，`↑↓`/`Enter` 逐条跳转并把相机居中到命中节点。

| 改动 | 内容 |
| --- | --- |
| 新面板 `panels/search.rs` | 纯状态（查询词 / 结果行 / 当前行 / 上次查询词与修订号 / 聚焦标记）+ 卡片 UI。`HitRow { id, text, kind, range }` 与 `Document` 解耦；结果由壳层通过 `lookup` 回调喂入。`Action::{Jump(id), Close}` 只表达意图，落地在壳层。 |
| 键盘纪律 | `handle_keys` 在**画布之前**消费 `Esc` / `Enter` / `Shift+Enter` / `↑` / `↓`——这些键漏下去会变成「加同级主题 / 清空选择」。`Enter` 首次落在第一条，之后逐条前进并绕回；`Shift+Enter` 反向；从空态反向走落在最后一条。 |
| 查询时机 | 只在**查询词或文档修订号**变化时调用 `lookup`；结果重算时按 id 保住当前行（同一节点还在列表里就不跳回第一条）。文档被替换（新建/打开/导入/恢复）时由壳层关掉搜索，避免跳到不存在的 id。 |
| 画布 API（`canvas.rs`） | `search_nodes(&mut self, q) -> Vec<mw_core::SearchHit>`（走 `Document` 里的拼音缓存，重复查询不重建键）、`node_text(id)`、`select_node_keep_focus(id)`——选节点并把相机居中，但**不抢键盘**，搜索框继续持有焦点，`Enter` 才能走下一条。 |
| 壳层接线（`main.rs`） | `Ctrl+F` 打开（节点内编辑中不打开，避免两处抢键盘）；`Ctrl+K` 打开命令面板时顺手关搜索；汉堡菜单新增「查找…」（矢量放大镜图标）；命令面板新增「查找主题…」；有搜索/命令面板时不收画布输入。 |
| 视觉 | 560px 白卡（12px 圆角 + 1px 边 + 26px 投影）：48px 输入行（放大镜 + 输入框 + `3/17` 计数 + 上下翻跳 + 关闭）+ 最多 6 行结果 + 28px 快捷键脚注。结果行 32px、命中字符**暖黄底**、非文本命中标 `拼音` / `首字母`；空态脚注「输入中文、全拼或拼音首字母」，无结果时给一行引导「试试全拼或首字母，如 xiangmu、xm」；点卡片外或 `Esc` 关闭。 |

**验证**：

- `cargo test --workspace` **268 项全绿**：`mw-shell` 52（+6：搜索结果步进绕回、空态不可步进、关闭清态、连按 `Ctrl+F` 不丢查询词、`Action` 相等性等）；`cargo fmt --all -- --check` 干净；`cargo clippy --workspace --all-targets` 新增代码 0 条（存量仍 38）。
- golden 无变化：搜索卡片只在打开时出现，默认界面与基线一致；`cargo golden --tol 0` 连跑两次容差 0。
- release 真实窗口截图 `shots/qa/search-{open,hits,jumped,text}.png`（`scripts/m3cc-search.ps1`）：`Ctrl+F` 打开空态卡片（占位「搜索主题，支持拼音（如 xm）」）→ 输入 `xing` 得 3 条拼音命中（`字形图集`/`禁则换行`/`交互与性能`，命中字暖黄 + `拼音` 角标，计数 `3 项`）→ `Enter` 后计数变 `1/3`、首行高亮、画布 `字形图集` 出现选中环且相机居中、大纲同步行同步 → 再查 `winit` 得 1 条文本命中（无角标）。
- **探针与 IME**：本机开启中文输入法时逐键注入会进组合态（候选窗吞掉 `Enter`），故探针改用剪贴板粘贴注入；代码侧同步在 `handle_keys` 顶部加了「`ImeEvent::Preedit` 非空即挂起整条键盘横路」的守卫（与节点编辑器同款纪律）。

**已知缺口（留给下一步）**：

- ~~命中节点在画布上没有独立的「外环高亮」（只有普通选中环），也没有呼吸动画~~ → **M2c 已补齐**：命中外环 + 两拍呼吸后常亮，尊重系统「减少动画」（§4.26）。
- 结果行不显示节点路径，同名的两个「目标」只能靠上下文猜。
- 只支持 `Ctrl+F` 打开；`F3`/`Shift+F3` 的「下一个/上一个」沿用 `Enter`/`Shift+Enter`。
- 没有「替换」概念——思维导图也不需要。

---

### 4.22 多标签：单画布会话对调 + 标签条 + 关闭确认（2026-10-18）

M3 的最后一块「多标签」。难点不在 UI，而在**资源预算**：一个 `Canvas` 独占一个 `Compositor` + 一张 4096² R8 字形图集（16MB，**当时值；P13 起 3072²/9MB 且无 CPU 镜像**）+ 一个 DirectWrite 度量器。最直白的做法 `Vec<Canvas>` 会把这三样按标签**线性复制**，开 8 个标签就是 128MB 纯图集——这正是 docs/14 §6 点名的「先解决字形图集共享」那一刀。

做法是**一个画布 + 会话对调**：字形键（字体/字号/字形 id）与文档无关，图集天然可跨文档复用。于是把「每标签的状态」缩到最小——`Tab { session, camera, sel_ids }`——切页时用 `std::mem::swap` 把画布的权威会话换进槽位、把目标槽位的会话换进画布，相机与选择再单独恢复。全程只有一个 `Canvas`、一张图集。

| 改动 | 内容 |
| --- | --- |
| 画布 API（`canvas.rs`） | 新 `camera()`（切页前把相机存回槽位）、`swap_session(&mut Session)`（`mem::swap` 换文档，**顺带清掉编辑/拖拽/连线/悬停等瞬时手势态**，因为那些 id 属于旧文档）、`set_view(camera, sel_ids)`（选择按 **id 重映射**到新场景索引——场景索引随文档而变，不能直接搬；`fitted=true` 免得首帧 fit 把对准的相机拽走）。 |
| 标签槽（`main.rs`） | `Tab { session, camera, sel_ids }` + `Tab::new`。壳层持有 `tabs: Vec<Tab>`、`active: usize`、`confirm_close_tab: Option<usize>`；`canvas` 字段仍是**唯一的活动画布**。 |
| 切页纪律 | `stash_active`：先 `commit_editing()`（把正在编辑的文字落成一条 `SetText`），再存相机/选择，最后 `swap_session` 把会话换进槽位。`load_active`：`swap_session` 换出目标会话，再 `set_view` 恢复相机与选择。`switch_tab`/`new_tab`/`close_tab` 都走这对函数，并在换页时关掉搜索与命令面板（它们的 id 属于旧文档）。 |
| 纯下标函数 | `next_tab(cur, delta, n)`（环绕）与 `active_after_close(idx, active, remaining)`（关页后活动位落在哪）从方法里拎成自由函数——这段运算是切页语义的全部，独立出来才能脱离 GPU/`Shell` 直接测。 |
| 快捷键 | `Ctrl+T` 新建、`Ctrl+W` 关闭（脏则弹确认）、`Ctrl+Tab`/`Ctrl+PageDown` 下一个、`Ctrl+Shift+Tab`/`Ctrl+PageUp` 上一个、`Alt+1..9` 跳到第 N 个。同组用 `||` 合并消费，避免 `if_same_then_else`。 |
| 标签条 UI | 只在 **≥2 个标签**时画（`Panel::top("tabbar")`，34px）：横向滚动区 + 每页 `selectable_label`（脏时前缀 `● `，标题超 22 字符按**字符**截断加 `…`）+ 每页 `×` 关闭 + 末尾 `＋`。绘制只收集 `TabAction`，出了循环才落地（与面板动作同一套纪律）。单标签时这一条**整根不画**，帧与旧版逐像素一致。 |
| 菜单与去重 | 汉堡菜单加「新建标签页」（`Ctrl+T`）。`request(Open)` 入口先做**同文件去重**：目标路径按 `canonicalize` 归一后若已在某个标签里，就切过去并提示，不再开第二个副本。 |
| 关闭确认 | 关脏标签弹三选一「保存并关闭 / 不保存关闭 / 取消」。选「保存并关闭」得**先切到该标签**再 `action_save`（存盘必须在该标签自己的会话上做；无路径时转另存为），若用户取消另存为（仍脏）则停在原地不关。最后一个标签关掉后补一个空白页，窗口不留空。 |

**验证**：

- `cargo test --workspace` **274 项全绿**：`mw-shell` 58（+6：切页双向环绕、单页/空集不环绕、关非活动页活动位右移/不动、关活动页顺位与夹取、标签名按字符截断、新槽初值）；`cargo fmt` 干净；`cargo clippy -p mw-shell --all-targets` **0 新增**（bin 回到存量 6 条，test 目标 6 条全为重复）。
- golden 无变化：单标签时标签条不画，默认界面与基线一致；`cargo golden --tol 0` 连跑两次容差 0。
- release 真实窗口截图 `shots/qa/tabs-{new,dirty,switched,close-prompt,closed}.png`（`scripts/m3cc-tabs.ps1`）：`Ctrl+T` 后标签条出现（两页 `未命名.zmind` + `＋`，新页高亮，左下 toast「已新建标签页」）→ `F2` 改名第二页根节点（`无选中时回落根`）并回车提交，该页标签出现脏点 `●`、标题栏「已编辑」、状态栏「● 未保存」→ `Ctrl+Shift+Tab` 切回第一页，**夹具文档、相机、大纲、状态栏（13 个主题 · 已保存）全部复原**，第二页仍带 `●` → `Ctrl+W` 关脏页弹三选一确认 → 点「不保存关闭」丢弃该页，回到单标签（标签条消失，帧同旧版）。
- **探针与编码**：脚本内不写任何非 ASCII 字面量——PowerShell 5.1 把无 BOM 的文件按 ANSI 读，中文字面量会吞掉引号解析失败（docs/14 §9 第 37 条）。改名文本用 `[char]0x7B2C` 之类码点拼出，剪贴板粘贴注入（绕开中文输入法的组合态）。

**已知缺口（留给下一步）**：

- 自动保存只覆盖**活动标签**：其余标签没有恢复点，崩溃会丢它们的未存改动（`autosave` 目前只读活动 `session`）。
- 标签条不能拖拽重排，也没有「关闭其他 / 关闭全部」与溢出下拉（超宽靠横向滚动）。
- 标签页不记住各自的搜索词/面板开合；切页一律关掉搜索与命令面板（有意为之，避免 id 串档）。
- 窗口状态记忆当时未做；**几何/最大化/面板开合已由 §4.27 补齐，开始页由 §4.28 补齐**；标签集合与各自相机仍不跨进程恢复。

---

### 4.23 导出：五种格式 + 离屏 PNG 与透明底（2026-10-18）

M4 的第一半：把「导出与画布同源」这条架构不变量（§2 不变量 2：`Frame.viewport = None` 即全图）变成真能落盘的按钮。**出口标准是「一张图 + 四份文本」都对得上内容**，不是「菜单里有个导出」。范围定在 PNG / Markdown / 缩进大纲文本 / OPML / FreeMind 五种——**SVG 与 PDF 当时有意不做**（§4.24 已补齐）：矢量导出要另一条几何→标记的管线（把 `Frame` 丢给 GPU 不算数），PDF 先嵌图的路子也不是这一次能收敛的；宁可枚举里不留半个变体，也不要「选项在、点了没反应」。

代码切成**两半**，GPU 无关的那半单独成模块，正是为了能脱离窗口单测：`mw-shell/src/export.rs` 管格式清单、文件名消毒、纯文本写出、PNG 尺寸预算与预乘 alpha 还原；真正的离屏渲染留在 `canvas::export_png`（要合成器），原生保存对话框在 `files::save_export`。

| 改动 | 内容 |
| --- | --- |
| 格式枚举（`export.rs`） | `Format { Png, Markdown, Outline, Opml, Freemind }` + `ALL`（顺序即界面顺序，图像在前）；`ext()`/`label()`/`is_image()`/`text(doc)`。文本四种直接转发 `mw-io` 的 `export_markdown`/`export_outline`/`export_opml`/`export_freemind`（各有往返测试），这里只做「选哪个格式 → 调哪个函数 → 写哪个后缀」的接线。 |
| 文件名消毒（`export.rs`） | `file_stem(title)`：先**剥掉 `.zmind` 后缀**（`Session::title()` 带后缀，原样拿去当默认名会存出「未命名.zmind」的 PNG），再擦掉 Windows 非法字符与控制字符、去首尾空格与结尾的点，空则回落「未命名」，按**字符**截到 48。`with_ext` 给漏后缀的路径补上。 |
| 预乘还原（`export.rs`） | 合成器回读的是**预乘** alpha，PNG 存直通 alpha；`unpremultiply` 在透明底时对每个半透明像素除回 alpha（全透明像素 RGB 归零，避免部分编辑器渗出黑边）。不透明导出（alpha 恒 255）下这一步是恒等变换，但**仍走同一条路径**，免得两套分支各写一遍。 |
| 尺寸预算（`export.rs`） | `png_size(w, h, scale)`：单边 `PNG_MAX_SIDE = 8192`、总像素 `PNG_MAX_PIXELS = 32M`（≈128MB 回读缓冲）。超限按比例缩回并**如实回报生效比例**（`Ok(eff)`）——界面据此给「已按 N 倍导出」的提示，而不是默默出一张被裁掉的图。长条形地图光卡单边拦不住，所以要两个上限一起卡。 |
| 保存对话框（`files.rs`） | `save_export(stem, ext, label, hwnd)`：`IFileSaveDialog`，按格式装过滤器，初始文件名走 `file_stem`。取消返回 `Ok(None)`，调用方什么都别做（**先问路径再干活**，不需要回滚一次导出）。 |
| 离屏渲染（`canvas.rs`） | `export_png(render_state, path, scale, opaque)`：`Frame { viewport: None, selected: 空, hovered: None, editing: None, ghost: None, guides: 空 }`——导出图里没有选中环/锚点/橡皮筋/编辑器（它们本来就走 egui 层）。相机 `Camera::new(bounds.center(), eff, 1.0)`。**用一台一次性 `Compositor::from_device`**，不借画布那台：`render` 会按视口重建内部目标纹理，而画布纹理正被 egui 采样，借用会让画布白屏一帧再重注册；独立实例只多一次管线创建。 |
| 图集增量的两条纪律 | 导出比例可能带来画布上从没用过的字形（另一档 ppd）。① 这笔增量**先喂给画布**那台合成器（`upload_atlas`）——丢了的话下一帧画布按代次判断会以为无事发生，字形永远传不上去；② 一次性合成器是新建的、代次从 0 起，**直接全量重传**（`generation: u64::MAX, reset: true`），别赌增量恰好对得上。 |
| 透明底（`compositor.rs`） | 新开关 `Compositor::set_transparent`：清屏色与点阵底一起退成透明，**主题一动不动**。这是本轮**唯一的产品级缺陷修复**——早先的写法是导出前把 `theme.ui.canvas_bg` 改成 `TRANSPARENT`，而这条令牌同时兼着**挖空色**（标签胶囊里的字、关系线标签的底、折叠圆点的底都拿它当橡皮擦，`text.rs:430` / `instances.rs:398,423,655`），于是透明底 PNG 里这些前景一起消失、标签胶囊成了空壳。清屏走独立开关、令牌留原值，两边才各得其所；上屏路径不许打开它。 |
| 对话框（`main.rs`） | `ExportOpts { open, fmt, scale, opaque }`，默认 PNG / 2× / 文档底色。`egui::Window "导出"`：格式 chips → 位图再露「比例 1×/2×/3×」与「背景 文档底色/透明」→ 位图给一行按包围盒与比例算出的**预测尺寸**（`size_note`）。回车确认 / Esc 取消**先于按钮绘制**读一次即可——浮层开着时画布已被 `set_input_blocked` 屏蔽，这两个键没人抢。三个入口：`Ctrl+E`、命令面板、汉堡菜单。`open_export` 先 `commit_editing()`（编辑器的 Enter/Esc 与浮层是同一批键）。 |

**验证**：

- `cargo test --workspace` **282 项全绿**（本轮 +8）：`mw-shell` **65**（+7，全是 `export.rs` 的：格式后缀/名互不撞、只有 PNG 是位图、四种文本都能序列化且包含内容、`file_stem` 消毒与 `.zmind` 剥离、`with_ext` 只补缺、`unpremultiply` 逆变换、`png_size` 缩限与退化盒）；`mw-render` **55**（+1：见下）。`cargo fmt --all -- --check` 干净；`cargo clippy -p mw-shell --all-targets` **0 新增**（bin 仍为存量 6 条）；`cargo clippy -p mw-render --all-targets` 仅存量。
- **透明底回归测试**（`mw-render/tests/text_render.rs`）：把相机对准「中文Rust混排」节点的「重要」标签胶囊放大 3×，先渲染不透明帧、再 `set_transparent(true)` 渲染透明帧。断言三件事：① 不透明帧里没有亚 255 的 alpha，且几何包围盒**之外**存在纯底色像素；② 那些像素在透明帧里**恰好是 `[0,0,0,0]`**；③ 胶囊内部缩进 15% 的窗口里，**超过 20 个像素是不透明的 `canvas_bg` 色**（即挖空字形还在）。**判「背景像素」只能靠几何包围盒**——挖空字用的就是画布底色，颜色上跟真背景分不开（第一版按颜色判，把挖空字误判成背景，也把抗锯齿连线边缘误判成背景）。这条测试**做过破坏性验证**：临时把胶囊文字色改成 `[0,0,0,0]`，测试立刻报「标签胶囊『重要』的字在透明导出里没了」，再改回。
- golden 无变化：导出是独立离屏路径，默认界面与基线一致；`cargo golden --tol 0` 连跑两次容差 0。
- release 真实窗口 `scripts/m4-export.ps1`：`Ctrl+E` 弹对话框 → 选 `PNG + 1× + 透明` → 保存为 `m4-sample-1x-transparent.png`（**1090×338，72950 B**；实测 76% 像素真透明，叠白底看「验收」标签字仍在）→ 再选 `PNG + 2× + 文档底色` → `m4-sample-2x.png`（**2179×676，161097 B**，全不透明，字节数与修复前逐字节相同，证明上屏路径一字未动）→ 四种文本各导一份（`.md` 252 B / `.txt` 273 B / `.opml` 713 B / `.mm` 564 B）。截图 `shots/qa/m4-png1xt-{dialog,options,savedlg,path,confirm,toast,done}.png` 与 `m4-png2x-*`，日志 `shots/qa/m4-png{1xt,2x}.log`。
- **探针加固**（两个会产出「假已验」的坑）：① 保存对话框的**覆盖确认默认键是「否」**——一次回车等于取消保存，而探针只看文件存在性会误报成功；现在断言覆盖确认（`#32770` 小窗）出现后按 `Alt+Y`（是），并回报文件 `mtime` 与当前时刻是否同步。② 应用被别的窗口抢前台时 `SetForegroundWindow` 会静默失败，键全落进 Chrome；现在**在任何按键之前**做 12 次前台断言，不通过直接 `exit 1`。

**已知缺口（留给 M4 后半段）**：

- **SVG / PDF 未做（已由 §4.24 补齐）**：矢量导出要独立的几何→标记管线，PDF 先嵌图也要单开一条路；当时枚举里刻意不留变体，免得出现「选项在、点了没反应」。
- 导出的是**整棵主题树**（折叠分支照样包含），没有「只导当前选中子树 / 只导可视区域」的选项。
- 对话框不记住上次的格式与背景选择（`ExportOpts` 在会话内保留，重启即回到 PNG/2×/文档底色）；原生保存对话框自带的「上次目录」是系统的，不算我们记住的。
- 超大图靠 `png_size` 缩限兜底，用户拿到的是「已按 N 倍导出」的提示，而不是分块出图。

### 4.24 SVG / PDF 导出 + 导入全回归与 UI 入口（2026-10-18，M4 后半）

M4 的后半把 §4.23 有意留下的两个坑补齐，并让五种导入格式第一次拥有真正的 UI 入口。出口标准：**SVG 打开是真矢量、PDF 能打印、五种导入在真实菜单/对话框里各走一遍且不覆盖当前文档**。

**PDF 走「先嵌图」的诚实降级**：PDF 内容流是另一套标记语言（坐标变换、字体嵌入、文本矩阵），与 SVG 的几何→标记管线不共享代码；先嵌一张位图能立刻拿到能打印、能转发的成品，代价是**文字不可选中、放大能看出像素**。对话框把这句话原样写给用户，不把位图 PDF 包装成矢量。矢量 PDF 留作明确的后续项（见「已知缺口」）。

| 改动 | 内容 |
| --- | --- |
| 格式枚举（`export.rs`） | `Format { Png, Svg, Pdf, Markdown, Outline, Opml, Freemind }`、`ALL` 七项。`is_image()` = `Png \| Pdf`（才露比例选择），`has_background()` = `Png \| Svg \| Pdf`（透明选项），`text()` 对四种绘制格式返回 `None`。对话框与 `run_export` 的分支全部由这三个谓词驱动，不再散落 `matches!`。 |
| 公共包围盒（`export.rs`） | `content_bounds(scene)`：节点/胶囊/摘要盒之外的**连线、花括号、关系线弧与箭头、折叠圆点、备注角标**都并进包围盒，再外扩 2 世界单位。这是三份绘制导出的同一取景框；PNG 的像素尺寸、SVG 的 `viewBox`、PDF 的 `MediaBox` 都由它推导，三张图内容才对得上。 |
| SVG 管线（`export_svg.rs`，678 行） | `Scene` → 标记，**不是**「渲染位图再包 `<image>`」：L1 边界/连线/概要括号 → L2 节点/标签胶囊/摘要盒 → L2 文字 → L3 关系线/箭头/标签/折叠圆点/备注角标 → L3 文字。文字位置直接调 `mw_render::text_jobs`（与字形图集同一份换行/基线/锚点结果），输出真实 `<text>`；贝塞尔按 `PathSeg` 原样序列化；颜色取主题令牌；点阵用 `<pattern>`（`dot[3] == 0` 时不写）；透明底既不画底也不画点阵。`font-family` 按 DWrite 候选栈写出，查看器缺字体时自行回退——这是矢量导出唯一无法保证逐像素一致的地方，字号/基线/锚点仍一致。 |
| PDF 编码（`export_pdf.rs`，284 行） | `encode(rgba, px_w, px_h, page, alpha)`：RGB 走 `FlateDecode`，透明通道单独一张 `/DeviceGray` `/SMask`（PDF 1.4）；页面尺寸 = 世界单位 × 72/96 **点**，与取样倍率无关（2× 只让同一页更清晰，不会让页面翻倍大）；文件结构固定为 Catalog → Pages → Page → Image(+SMask) → Content → xref，对象流一律不用，任何阅读器都吃。xref 每项严格 20 字节（10 位偏移 + 空格 + 5 位代次 + 空格 + `n` + 空格 + `\n`）。 |
| 离屏共用（`canvas.rs`） | PNG 与 PDF 共用私有 `render_export`：算包围盒 → `png_size` 定像素 → 离屏合成 → 透明时 `unpremultiply`。PDF 额外把包围盒换算成点做 `MediaBox`；PNG 直接写像素。一次性合成器与图集两条纪律（§4.23）原样生效。 |
| 对话框（`main.rs`） | PDF：页面毫米数 + 实际像素位图 + 「文字不可选中」+ 「导出整棵树」三行说明；PNG：像素尺寸；SVG：矢量说明。比例只对 `is_image()` 露出，所以 SVG 不再显示无效的 1×/2×/3×。 |
| 导入 UI 入口（`files.rs` / `main.rs` / `command.rs`） | `files::open_import`：`IFileOpenDialog`，过滤器 `*.md;*.markdown;*.txt;*.opml;*.mm;*.xmind` + `*.*`。汉堡菜单「导入 Markdown / OPML / FreeMind / XMind…」、命令面板同一条、拖放非 `.zmind` 文件也汇到 `import_path`。导入**开新标签页**，当前文档原封不动（`PendingAction::Import` 已删——它不替换文档，不该走那条确认脏数据的路）。 |
| 导入标题（`session.rs`） | `Session` 新增 `import_name`：导入的会话没有 `path`（Ctrl+S 必须先另存为），但标题栏/标签页显示**来源文件名**（如 `大纲.md`）而不是「未命名.zmind」；另存为成功后清空，标题改跟新 `.zmind` 走。`export::file_stem` 同步剥掉七种源后缀（`.zmind/.md/.markdown/.txt/.opml/.mm/.xmind`），否则导入文档导 PNG 会默认叫 `大纲.md.png`。 |

**导入全回归**（`mw-shell/src/session.rs` 测试 + 真实 UI）：

- 扩展名分流：7 种后缀（含全大写 `.MD`/`.XMIND`）全部接受，`png/pdf/docx/空` 一律拒绝。
- Markdown：标题层级 + 列表嵌套成树；缩进大纲：2/4 空格与 Tab 的深度；OPML：嵌套 + 备注；FreeMind：`FOLDED`/`LINK`/richcontent 备注/`COLOR`；XMind：现场用 `zip::ZipWriter` 造一个最小 Zen 容器，验证 `content.json` 走通。
- `.zmind` 往返：保存→重开，逐节点 `assert_eq!`（文字/位置/覆盖样式/标签/备注/链接/标记/折叠），并断言 标签/概要/边界/关系线 都还在，`validate()` 通过。
- 真实 UI（release，`scripts/m4-import.ps1`）：`Ctrl+K` → 粘贴「导入」→ Enter → 原生「导入文件」对话框 → 粘贴路径 → Enter。结果：**两个标签页**（旧文档保留）、新页标题 `import-test.md`、状态栏「未保存 9 个主题」、结构 `导入验收 → 渲染底座/交互/换行` 与 Markdown 一致。再按 `Ctrl+S` 走另存为 → `import-saved.zmind`（1,168 B），标签与标题栏立即改名、脏点消失。截图 `shots/qa/import-{menu,palette,dialog,path,opened,saved}.png`。

**验证**：

- `cargo test --workspace` **300 项全绿**（M4 后半 +18：`mw-shell` 65 → **83**，其中 SVG 7 项、PDF 5 项、导出谓词 1 项、导入回归 5 项）；`cargo fmt --all -- --check` 干净；`cargo clippy --workspace --all-targets` 仍为存量 **38 条**（`mw-shell` 6），0 新增。
- golden：无外观变化，`cargo golden --tol 0` 连跑两次通过。**第一次曾报 2367 像素差（0.135%，最大差 8）**，包围盒正好是左上汉堡按钮——是上一次真实输入探针把鼠标留在按钮上，golden 截到了 hover 高亮；把指针移出窗口后两次全绿。golden 对比的是「当前指针状态下的界面」，跑之前必须清场（§14 教训 48）。
- release 真实导出（`scripts/m4-export.ps1`，鼠标坐标为**逻辑像素**，探针内部乘 DPI）：
  - SVG `shots/qa/m4-svg.svg`：9,173 B；`viewBox="-623.26 -171 1093.55 342"`；16 个 `<text>`、15 条 `<path>`、18 个 `<rect>`；`[xml]` 解析通过；浏览器打开为真实可选文字，卡片/边界/关系线/概要完整（`m4-svg-browser.png`）。
  - PDF `shots/qa/m4-pdf.pdf`：106,935 B；PyMuPDF 读回 **1 页、820.16×256.5 pt、1 张图、0 个可选字符**；渲染预览 `m4-pdf-rendered.png`（1254×392）与 SVG/PNG 内容一致——这正是「先嵌图」的预期。
  - PNG `shots/qa/m4-png.png`：**2187×684** RGBA，2× 文档底色导出，四角为 `#fbfbfc` 不透明；与对话框预测尺寸一致，证明 `content_bounds` 纳入离盒几何后取景正确。
  - 样例世界包围盒 **1093.55×342**：PNG 2× = 2187×684；PDF 页面 = ×72/96 = 820.16×256.5 pt。

**已知缺口（M4 后半之后）**：

- **PDF 是位图**：文字不可选中、放大会糊；没有 PDF 大纲/链接/矢量字体。真矢量 PDF 需要内容流 + 字体嵌入，单独立项。
- **SVG 不自带字体**：`font-family` 交给查看器回退，缺字体时字形宽度可能与画布略有差异（位置/字号/锚点一致）。
- 两种矢量/位图导出都**不包含选中环、悬停、对齐线**（与 PNG 同规矩），也仍然只导整棵树，没有「只导选中子树/可视区域」。
- **导入是同步读取**：没有 docs/02 原先写的「>5MB 走后台线程」，大文件会卡住 UI 一次；也**没有**「丢失项报告」（XMind 的评论/甘特等丢弃是静默的）。拖放导入仍无自动化验证（要真的从资源管理器拖），但它与菜单/面板共用 `import_path`。
- 导入不按源路径去重：同一个 `.md` 可以开成多个各自未命名的标签页，直到另存为才有路径。

### 4.25 鱼骨 / 时间轴布局（E5，2026-10-18，M4 收尾）

M4 的最后一项是 docs/12 §2 的 E5：`LayoutKind` 从三种扩到五种，鱼骨图与时间轴都走「轴线 + 上下分支」这一族几何。出口标准：**切换布局即得可读的鱼骨/时间轴结构，且这条结构在画布、导出、检查器三处一致**。

| 改动 | 内容 |
| --- | --- |
| 枚举与场景（`mw-core`） | `LayoutKind { Mindmap, LogicRight, OrgChart, Fishbone, Timeline }`，`from_kind`/`as_str` 同步；`Orientation` 新增 `Straight`；`Layout.spines: Vec<Path>` 装轴线，`Scene.spines: Vec<SceneSpine>`（`path` + `color = theme.ui.accent` + `width = theme.edge_width_root`）随场景走。浮动主题合并时轴线跟着平移（`shift_path`），不是只搬盒。 |
| 鱼骨几何（`layout.rs` 5c） | 根居中于原点；直接子级按**文档顺序上下交替**（偶上奇下）。每个分支先按绝对深度算各层最大盒宽，再**向左排成列**（子级在父级更左），整条分支带在上/下两侧分别向外堆叠。肋线（bone）从轴上的 `(join_x, 0)` 连到分支根盒的近轴边中点，`join_x = min(盒中心 + 28, 根左缘 - 4)`，避免肋线扎进根盒。主轴从根中心向左延伸到最左槽位再外扩 16。 |
| 每侧独立槽位（本轮真缺陷修复） | 第一版让所有分支根共享一条右缘列、只把分支带上下错开，**同侧相邻分支的肋线会斜穿对方的盒**（截图 `shots/qa/e5-fishbone.png` 留作证据）。改为 `above_slot` / `below_slot` 两条游标：每排完一条分支，该侧槽位继续左移「层宽和 + 间距」，**同侧分支的 x 区间互不重叠**；主轴长度随之取到最左槽位。第二版 `e5-fishbone-v2.png` 无交叉。 |
| 时间轴几何 | 轴从根盒右缘向右延伸；所有分支共用一条**向右推进的全局槽位游标**，每个分支的子级再向右排成列（与鱼骨镜像）。分支仍上下交替；每个分支在轴上留一个刻度 `join_x = max(盒中心, 根右缘 + 4)`，轴长取最右刻度 + 16。 |
| 边与折叠件 | 肋线/刻度边用 `Orientation::Straight`，`edge_path` 直连不画贝塞尔；分支内部仍用原有折线/曲线路径。鱼骨的折叠圆点与悬停「＋」加子在**根盒左侧**（时间轴在右侧），与阅读方向一致。 |
| 渲染（`mw-render`） | L1 先画 `scene.spines` 再画 edges：同相机、同细分容差、同屏幕空间线宽换算；轴线也走 `path_bounds` + 线宽外扩的视口裁剪，不因它是新族就漏裁。 |
| SVG（`export_svg.rs`） | `<g id="lines">` 里顺序为 边界 → **轴线** → 连线，轴线序列化为普通 `<path>`，颜色/线宽取主题令牌；PNG/PDF 由同一 `Scene` 离屏渲染，天然包含轴线。 |
| 检查器（`inspector.rs`） | 布局 chips 从一行改成 **3 + 2 两行**（思维导图/逻辑图/组织架构 + 鱼骨图/时间轴），260px 面板内不挤字、不截断。 |

**验证**：

- `cargo test --workspace` **309 项全绿**（本轮 +9）：`mw-core` 106 → **114**（布局 6 项：上下交替与左向主轴、时间轴左右推进、后代沿轴成列、只有轴布局发 spines、Straight 边是单段线、鱼骨折叠件在左；场景 2 项：轴线继承主题色、浮动主题轴线随动），`mw-shell` 83 → **84**（SVG 轴线断言：鱼骨场景导出后含轴线的 `d` 与主题色 stroke）。`cargo fmt --all -- --check` 干净；clippy 按「文件:行:消息」去重仍为存量（`mw-core` 11、`mw-shell` 6），0 新增。
- golden：基线样例是默认思维导图，E5 不改默认外观；`cargo golden --tol 0` 连跑两次容差 0（槽位重设计后重跑确认）。
- release 真实窗口：用 13 节点样例（根 + 4 分支 × 2 子级）分两份 `.zmind` 打开——鱼骨 `shots/qa/e5-fishbone-v2.png`（四分支两上两下、肋线均不相交、主轴延伸到最左列）、时间轴 `shots/qa/e5-timeline.png`（四分支两上两下、刻度落在轴上、分支向右成列）；检查器 `shots/qa/e5-inspector.png` 显示 3+2 chips 且鱼骨选中。另用真实点击在**同一份文档**里切换鱼骨 ↔ 时间轴，`e5-switch-timeline.png` / `e5-switch-fishbone2.png` 证明几何即时重排、无需重开文档。
- 探针教训：第一次按估算坐标点 chip，实际落到「逻辑图」（`e5-inspector-crop.png` 裁图后才发现行高与 DWM 偏移），说明**面板控件的探针坐标必须先裁图核对**，不能拿设计稿尺寸直接算（docs/14 教训继续累计）。

**已知缺口**：

- 鱼骨只有一种肋线角度（`BONE_SLANT = 28`，且被根盒边界夹住），没有 XMind 那种「主骨粗细 / 分支配色 / 肋线夹角」的可调参数；轴线与肋线同色同宽。
- 时间轴是**一条横向主轴**，子级沿轴向右成列；没有「里程碑等距 / 日期自动排布 / 竖向时间轴」。分支的日期语义全靠节点文字，没有时间字段。
- 上下两侧按文档顺序机械交替，不做**带高平衡**：一侧若有一条特别高的分支，主轴与另一侧的留白会明显变长（不会重叠，但不好看）。
- 布局切换只换几何，不记忆每份文档上次用过的布局之外的东西（布局值本身存在文档里，随 `.zmind` 往返）。

### 4.26 快捷键全集与动效（M2c，2026-10-18）

M2c 的出口是 docs/02 §10 的键位主体与 docs/06 §8 的动效表。本轮把「日常编辑闭环里会按到的键」一次补齐，并把**已经能给到反馈的动效**落到画布与面板；节点增删/折叠/布局过渡那几张表项受「硬切改插值」的架构成本限制，当时留到后面，逐条写在缺口里——没有假装做完；**后由 §4.30 的场景插值补齐**。

| 键位 | 语义落点 | 备注 |
| --- | --- | --- |
| `Ctrl+C / X / V` | 节点子树复制 / 剪切 / 粘贴 | 见下方「剪贴板事件拦截」；粘贴落到主选为子、无主选落根，克隆保留图标/备注/链接/标签与显式样式，关系线只保留**两端都在克隆集内**的边 |
| `Ctrl+D` | 复制节点（副本紧随原节点） | 与 `Ctrl+C/V` 共用 `clone_subtree`，不经过系统剪贴板 |
| `Ctrl+Shift+C / V` | 复制 / 粘贴样式 | 只搬**显式 `StyleOverride`**；主题继承值与分支色不复制（见下） |
| `Alt+↑ / ↓` | 主选同级上移 / 下移 | 多选时对主选操作；移动到边界为 no-op，不发空命令 |
| `Ctrl+Shift+← / →` | 升级（交给祖父）/ 降级（交给前一个同级） | 先于不带 Shift 的分支消费；根节点与首同级为 no-op |
| `/`、`Ctrl+/`、`Ctrl+Shift+/` | 折叠切换 / 全部折叠 / 全部展开 | `Ctrl+Shift+/` 在 winit 里报 `Questionmark`，两种键码都查 |
| `Ctrl+0 / 1 / 2` | 100% / 适应画布 / 适应选中 | 与 `Ctrl+=/-`、Ctrl+滚轮同一条相机路径 |
| `Alt+Enter` | 聚焦主选子树（视图态，不改文档） | 必须在裸 `Enter`（加同级）**之前**消费；Esc 逐级退回，画布顶部出现面包屑 |
| `F2` / `Tab` / `Enter` / `Delete` | 编辑 / 加子 / 加同级 / 删除 | M2a 已有；无主选时统一兜底到根，不静默失效 |
| `F1` | 快捷键速查浮层（`panels/help.rs`） | 640px 非模态浮层；**只列已实现的键**，这张表就是承诺 |
| `Ctrl+P` | 导出 PDF（复用导出对话框并预选 PDF） | 与 `Ctrl+E` 同一个 `ExportOpts`，比例/背景仍可选 |
| `F11` | 专注模式（铺满当前显示器 + 置顶） | 实现见下；退出恢复进入前的窗口矩形与最大化状态 |

**剪贴板事件拦截（egui-winit 的真实行为）**：`egui-winit` 会把 `Ctrl+C/X/V` 提前翻译成 `Event::Copy/Cut/Paste` 并 `return`，按键事件根本到不了 `Canvas::shortcuts`。画布必须在 `ui.input_mut` 里**读并移除**这三类事件，自己派发节点剪贴板；复制时还要把节点文本写进系统剪贴板——否则系统剪贴板为空时连 `Event::Paste` 都不会发，粘贴看起来「按了没反应」。这段派发顺序放在 `consume_key` 组之前，`Ctrl+Shift+C/V`（样式）再由 `consume_key` 处理。

**样式剪贴板的「显式覆盖」规则**：`Ctrl+Shift+C` 复制的是 `Node.style` 里的 `StyleOverride`，不含主题解析后的最终值。演示文档里多数节点的填充是**主题继承**的，从它们复制样式等于复制空集——这不是 bug，而是「复制样式」在级联模型里的正确语义（粘贴后仍跟随主题）。探针第一版正是因此得到了一张「粘贴前后完全一样」的假截图；改为先用检查器给源节点设一个显式红色填充再复制，`m2c-style-set.png` / `m2c-style-paste.png` 才证明链路成立。

**F11 专注模式：一次同步 `SetWindowPos`，绕开 winit 的异步路径**。最初的实现走 `ViewportCommand::Fullscreen(true)` / `InnerSize` / `OuterPosition`，真实窗口里出现「按 F11 后 DWM 仍显示旧帧，鼠标一动才刷新」的假死。根因是 winit 对这些命令统一用 `SWP_ASYNCWINDOWPOS`：窗口矩形已经改了，但 DWM 组合器要等下一次同步尺寸变化才提交新帧。最终实现是：

1. `MonitorFromWindow` + `GetMonitorInfoW` 取当前显示器矩形；若窗口已最大化，先 `ShowWindow(SW_RESTORE)` 再量 `GetWindowRect`，把「还原矩形 + 是否最大化」存进 `FocusRestore`；
2. 用**同步** `SetWindowPos(HWND_TOPMOST, 显示器矩形, SWP_SHOWWINDOW)` 一次完成位置、尺寸、置顶；退出时同步 `SetWindowPos(HWND_NOTOPMOST, 保存的矩形)`，若进入前是最大化再 `ShowWindow(SW_MAXIMIZE)`；
3. 进出时分别 `DwmSetWindowAttribute(DWMWCP_DONOTROUND / DWMWCP_ROUND)`，专注态去掉 Win11 圆角；
4. 失焦时主动 `SetWindowPos(HWND_NOTOPMOST, SWP_NOMOVE|SWP_NOSIZE|SWP_NOACTIVATE)` 掉置顶。**不能**用 `ViewportCommand::WindowLevel(Normal)`：winit 缓存了窗口层级，原始 API 置顶后它认为「本来就是 Normal」，这条命令是 no-op。

实测（150% DPI，客户端 1620×1080）：F11 前 1620×1080 → 专注态覆盖 1920×1200 物理像素 → 退出精确回到 1620×1080；`f11-accept-full.png` / `f11-accept-back.png` 是同一进程前后帧。探针还验证了专注态下 `Ctrl+K` 浮层与 `F9` 面板仍可用。

**动效（docs/06 §8 已实现项）**：

| 动效 | 实现 | 减少动画时 |
| --- | --- | --- |
| 搜索命中 | 命中节点外环：0.7 s 内两拍扩散（半径 3→7px、alpha 衰减），之后常亮细环；颜色取 `theme.ui.accent`，屏幕空间恒定 | 直接常亮（spread 4 / alpha 120），不做扩散 |
| 面板开合 | `F8`/`F9` 0.2 s `cubic_out` 宽度 + 内容透明度；隐藏阈值 `t <= 0.005`；golden harness 下直接给终态 | 直接终态 |
| 减少动画来源 | `SPI_GETCLIENTAREAANIMATION == 0`（设置 → 辅助功能 → 视觉效果 → 动画效果），启动读一次缓存 | — |

**验证**：

- `cargo test --workspace` **315 项全绿**（本轮 +6：壳层快捷键/浮层相关纯逻辑；`mw-shell` 84 → **90**）。fmt 干净；clippy 按「文件:行:消息」去重仍为存量 38（shell 6、core lib 11），0 新增。
- `cargo golden --tol 0` 连跑三次容差 0（1620×1080 @ ppp 1.50）。动效在 harness 下走终态，基线不含动画中间帧。
- 真实输入 `scripts/m2c-keys.ps1`：24 张截图 `shots/m2c/m2c-*.png`，覆盖复制粘贴（16 节点子树）、Alt+↓ 重排、检查器显式红填充下的样式复制、降级/升级、折叠/全折叠/全展开、`Ctrl+0/1/2`、`Alt+Enter` 聚焦与面包屑、`F1` 速查、`Ctrl+P` PDF 对话框（预设 PDF、页面 289×90 mm / 2187×684 px）、`F8/F9` 面板动画三帧、搜索跳转高亮、`F11` 进出。
- release 指标：exe **19,879,424 B**；冷启动到窗口 **458–669 ms**（三次）；私有内存 **319.1–321 MB**、工作集 **371.5–373.5 MB**（冷启动 5 s 后三次）。

**已知缺口**：

- ~~**节点创建 / 删除 / 折叠展开 / 布局切换 / 演示步进仍无动效**（硬切）~~：前四项已由 §4.30 的场景插值补齐，演示步进由 §4.32 的相机缓动补齐。
- ~~**没有演示模式**（F5 未实现）~~：已由 §4.32 落地（先序步进 / 350ms 缓动 / 路径高亮 / 控制条 / 退出还原视口）；`F11` 仍是窗口专注态，与演示是两回事。
- **没有右键菜单**，因此上下文菜单里的批量动作仍不可达；快捷键未提供设置界面，冲突项不能禁用（docs/02 §10 脚注的「可配置」未做）。
- 面板开合已有动效，但**打开面板仍不重算相机**（既有缺口，见 §4.20）：动画结束图仍可能被右侧裁掉，需要 `Home`。

---

### 4.27 窗口状态记忆（M3 打磨第一项，2026-10-18）

**目标**：重开应用回到上次的窗口位置/大小/最大化态，以及大纲、检查器、小地图三个面板的开合。这是壳层打磨里最容易被用户当成「原生不原生」的一项——每次启动都把窗口摆回屏幕中央，等于把上一轮的工作上下文丢掉。

**存储**（`crates/mw-shell/src/window_state.rs`，357 行）：

```json
{
  "rect": [220, 140, 1400, 900],
  "maximized": false,
  "show_outline": false,
  "show_inspector": true,
  "minimap_open": true
}
```

- 位置 `%APPDATA%\织思\window.json`（与 `recent.json` 同一目录，不碰文档目录）。
- **外框物理像素**：DPI 缩放变化不换算、多显示器之间可原样搬回。正常态取 `GetWindowRect`；最大化时取 `GetWindowPlacement().rcNormalPosition`——`GetWindowRect` 此时给的是铺满屏幕的矩形，直接存会把「还原」位置覆盖掉。
- **恢复前校验**：矩形尺寸 ≥320×240 且坐标有限；标题栏中心点（顶边下 20px）必须还落在某台显示器的 `rcMonitor` 里（`MonitorFromPoint(MONITOR_DEFAULTTONULL)`）。显示器拔掉、分辨率改小、窗口被拖出屏幕，都退回默认居中；**面板开关仍然生效**（只丢几何，不丢布局）。
- 坏 JSON/缺字段退化成默认值；`MW_WINDOW_STATE=<path>` 换存储位置，`MW_WINDOW_STATE=off` 关闭持久化。golden harness 直接走 `WindowState::disabled()`，**不读也不写**，基线不会因为本机上次摆过窗口而漂移。

**接线**：

- `Shell::new` 用存档里的三个面板开关做初值；首次拿到 HWND 后 `restore_window_state` 用同步 `SetWindowPos` 搬窗口（不走 `ViewportCommand::InnerSize/OuterPosition`，那套是排队命令，会先画一帧默认位置），最大化再发 `ViewportCommand::Maximized(true)` 交给 winit，保持它自己的最大化状态与按钮/双击手势一致。
- 保存按 **700ms 防抖**轮询 `read_placement`（`Instant` 计时，原生文件对话框阻塞 UI 线程也不会让到期点错乱），内容没变不落盘；`on_exit` 再兜底一次，正常关窗（含 Alt+F4）不会丢掉 700ms 内的最后一次移动。专注模式（F11）下几何是临时的，只存面板开关。
- **恢复 resize 后的重新取景**：`SetWindowPos` 落地比第一帧画布的 fit 晚一两帧，窗口从 1620×1080 缩到 1400×900 时首帧 fit 用的是旧视口，构图会偏左。`Canvas::request_fit()` + 500ms 的 `refit_until` 窗口让恢复后每帧按当前视口重 fit，落定后自动停（无恢复时不触发，golden 不受影响）。

**验证**：

- `cargo test --workspace` **324 项全绿**（本轮 +9：矩形尺寸/坐标校验、标题栏锚点、半开区间、缺文件默认值、坏文件降级、坏矩形丢几何保留面板、往返读写、未变化不重写、disabled 不落盘；`mw-shell` 90 → **99**）。fmt 干净；clippy 按「文件:行:消息」去重仍为存量 38（shell 6、core lib 11），0 新增。
- `cargo golden --tol 0` 连跑两次容差 0（1620×1080 @ ppp 1.50）：harness 不读存档，默认界面逐像素不变。
- 真实输入 `scripts/m3-window-state.ps1`（4 个场景、6 张截图 `shots/m3/m3-ws-*.png`，临时存档经 `MW_WINDOW_STATE` 隔离）：
  1. 首启默认 1620×1080 @ (150,60) → `SetWindowPos` 到 1400×900 @ (220,140) → F8 关大纲、F9 开检查器 → 正常退出；JSON 里矩形逐像素一致，两个面板开关正确。
  2. 重启：窗口**逐像素回到 220,140 1400×900**，大纲关、检查器开；点标题栏最大化按钮后 `IsZoomed = true`，退出时存下 `maximized: true` 且常态矩形仍是 1400×900。
  3. 再重启：直接以最大化启动；点还原按钮回到 220,140 1400×900（±16px，`rcNormalPosition` 与 `GetWindowRect` 的外框差）。
  4. 写入 `rect: [-50000,-50000,1400,900]`：几何被拒、窗口回默认 1620×1080 @ (150,60)，但 `show_outline:false / show_inspector:true / minimap_open:false` 三个面板开关照常生效（截图中大纲不在、检查器在、小地图只剩角标）。

**已知缺口**：

- 跨不同 DPI 显示器恢复时，winit 收到 `WM_DPICHANGED` 会按系统建议矩形再调一次尺寸，可能与存档有 1–2px 偏差；本机单显示器（150%）未覆盖这条路径，列为后续多显示器验收项。
- 窗口最小化状态不记忆（刻意：启动就最小化等于没启动）；标签页列表与各自相机不跨进程恢复（属于会话恢复，不和窗口布局混在一起）。
- 右键菜单当时仍未做（开始页已在 §4.28 补齐），现已在 §4.29 补齐。

---

### 4.28 开始页（M3 打磨第二项，2026-10-18）

**目标**：把「启动即打开一份夹具文档」换成真正的产品入口，并补齐关掉最后一页之后的去处。XMind 2026 的启动页承担三件事：新建空白、从模板开始、回到最近文件；本轮按同一信息结构做，但不引入账号/云文档。

**启动落点**（`Shell::new` + 纯函数 `start_at_home`）：

- 没有待打开文件（无 `MW_OPEN`）、不是 golden harness 时，**默认落在开始页**；`MW_OPEN` 成功打开文档则直接显示文档。
- `MW_HOME=0` 强制走文档（老探针依赖夹具文档），`MW_HOME=1` 强制开始页；harness 永远走文档，基线不受影响。
- 开始页背后**不留夹具文档**：若落在开始页且没有打开文件，画布里的会话换成 `Session::blank()`，避免用户误以为那是一份可保存的示例、也避免「新建空白」多开一页。

**页面结构**（`crates/mw-shell/src/start.rs`，754 行）：

- 全区域浅灰底（`F7F8FA`）+ 右上/左下两团极淡主题色圆；内容最大宽 980px 居中，纵向可滚动。
- 品牌头：「织思」字标 + 副标题 + 右侧「打开文件」次按钮与「新建空白图」主按钮；从菜单进入时多一个「返回当前文档」。
- 欢迎条只在最近列表为空且未关闭时出现，可 × 关闭（本进程记住）。
- 5 张 172×138 模板卡：思维导图 / 逻辑图 / 组织架构 / 鱼骨图 / 时间轴。预览**全部矢量绘制**（不依赖图标字体），卡片有 hover 强调色与编号 1–5。
- 最近文件卡（最多 8 条）：文件名、父目录、相对时间、kind 图标；空态给拖入提示。
- 底部提示：`.zmind / Markdown / OPML / FreeMind / XMind` 可直接拖进窗口。

**模板纪律**：`Template::session()` 只通过 `mw_core::Command`（`SetLayout` / `SetText` / `InsertSubtree`）构造文档，节点 id 走 `new_id()`；产物是 `Session::fresh_named` 的**未保存但非 pristine** 会话（标题为模板名，`Ctrl+S` 弹另存为）。5 个模板文档合法性有单测覆盖，不依赖窗口。

**交互闭环**：

- 开始页只发 `start::Action`（`NewBlank` / `OpenFile` / `Return` / `OpenRecent` / `Template`），绘制循环外由 `run_home` 落地——与面板同一纪律，开始页不直接改文档。
- 「新建空白 / 模板」优先**就地复用**当前未动过的空白页，否则另开标签页；已打开的其他文档不受影响。
- 关闭最后一页自动回开始页（`close_tab` 里 `home = true`，无返回按钮）；汉堡菜单「开始页」则是**盖住**当前文档（`home_return = true`），`Esc` 或「返回当前文档」退回。
- 开始页期间文档类快捷键整体挂起，只接 `Ctrl+N` / `Ctrl+O` / `Enter`（空白）/ `1–5`（模板）/ `Esc`（仅菜单进入时返回）/ `F1`——避免 Tab/Delete 改到隐藏的图。

**窗口标题与几何**：

- `sync_window_title`：文档态「文档名 — 织思」，开始页「织思」；只在真的变化时发 `ViewportCommand::Title`（每帧发等于每帧调一次 winit）。
- 默认窗口尺寸改为按 `SPI_GETWORKAREA` 工作区（已扣任务栏）居中，物理→逻辑用 `pixels_per_point` 换算；本机 1920×1200 @150%、工作区高 1128，旧逻辑按整屏居中会让窗口底边压进任务栏 12px，开始页状态栏被裁。现在 1620×1080 @ (150,60) 完整可见。

**验证**：

- `cargo test --workspace` **330 项全绿**（本轮 +6：模板文档合法性、空白 pristine 而模板不 pristine、相对时间分桶、目录提示、按字符截断、启动落点；`mw-shell` 99 → **105**）。fmt 干净；clippy 去重仍为存量 38（shell 6、core lib 11），0 新增。
- `cargo golden --tol 0` 连跑两次容差 0（harness 强制文档，默认界面逐像素不变）。
- 真实输入 `scripts/m3-home.ps1`（两个场景、7 张截图 `shots/m3/m3-home-*.png`，临时 `APPDATA` + `MW_WINDOW_STATE` 隔离）：
  1. 默认启动标题精确等于「织思」→ `4` 选鱼骨图（标题含「鱼骨图」）→ `Ctrl+W` 回开始页 → `Ctrl+N` 建空白 → 汉堡菜单「开始页」→ `Esc` 返回 → 关页回开始页。
  2. 用 `MW_OPEN` 打开 `quarterly-review.zmind` 后退出；重启落在开始页且最近列表出现该文件；点第一行重新打开，标题含文件名。
- `scripts/m3-window-state.ps1` 补 `MW_HOME=0` 后四场景回归通过（默认落点变化不影响窗口/面板记忆）。

**已知缺口**：

- 最近文件列表只能用鼠标点，没有 ↑↓ 键盘导航；欢迎条关闭状态不跨进程记忆。
- 模板固定 5 套、内容写死在 `start.rs`，还不能从用户文档另存为模板。
- Toast 仍压在状态栏上方，开始页截图里可见；属既有壳层层级问题，未在本轮改。
- 开始页的两团装饰圆在低对比度显示器上略抢眼，后续可做成可关的纹理层。

---

### 4.29 右键菜单：三套菜单 + 键盘导航 + 真实输入闭环（M3 打磨第三项，2026-10-18）

**目标**：补齐 M3 壳层最后一项——节点 / 多选 / 空白三套右键菜单。样式对齐 XMind 2026 的白卡（240px 宽、12px 圆角、快捷键右列），并保证纯键盘可操作。M3 到此收口。

**管线**（`canvas.rs` → `main.rs` → `panels/context.rs`）：

- 画布在 `PointerButton::Secondary` 点击时先提交未完成的编辑；右击未选中节点 → 置为唯一选中，右击已选组内节点 → **保留整组**（壳层据此弹多选菜单）；右击空白不清空选择，但弹空白菜单。
- 画布只产出 `ContextRequest { pos, target: Node | Blank }`，壳层用 `take_context_request()` 取走。空白 = 没有命中节点；命中折叠点/胶囊/关系线等非节点目标也按空白处理。
- 壳层按目标与 `selected_count()` 组装 `Item`，菜单构建只读画布状态（`primary_collapsed` / `has_clipboard` / `has_style_clipboard` / `can_delete` / `can_collapse_selection` / `can_move_same_parent` / `can_focus_primary` / `zoom`），**不碰文档**。
- `Action` 只表达意图，`run_context_action` 落回 Canvas 方法；改文档的入口全部发 `Command`（多选折叠是一条 `Batch`，一次撤销可回退）。复制/剪切复用画布的进程内节点剪贴板，同时把先序缩进文本写进系统剪贴板。

**三套菜单**（`build_context_menu`）：

| 目标 | 条目 | 禁用条件 |
| --- | --- | --- |
| 节点 | 新建子主题 `Tab` · 新建同级主题 `Enter` · —— · 剪切 · 复制 · 粘贴 · 删除 · —— · 编辑文字 `F2` · 复制样式 · 粘贴样式 · —— · 折叠/展开 `/` · 聚焦 `Alt+Enter` | 根主题不可剪切/删除；粘贴需进程内剪贴板；粘贴样式需样式剪贴板；无子节点时折叠禁用；主选已是聚焦根时聚焦禁用 |
| 多选 | 设置样式 · 折叠 · 移入同一父级 · —— · 复制 · 删除 | 折叠需至少一个可折叠成员；移入同一父级需至少一个散落根；选中全是根主题时删除禁用 |
| 空白 | 粘贴 `Ctrl+V` · —— · 适应画布 `Ctrl+1` · 100% `Ctrl+0` · —— · 全部展开 `Ctrl+Shift+/` · 全部折叠 `Ctrl+/` · —— · 打开文档所在文件夹 | 粘贴需节点剪贴板；已 100% 时该项禁用；文档未落盘时「打开文件夹」禁用 |

两个语义容易写错：① 空白菜单的粘贴**始终落到根主题**（`paste_at_root_action`），不沿用残留选择；② 多选「移入同一父级」的落点是**主选的父级**，只移动散落根（祖先也在选区里的后代不重复移动），已经在同一父级下的不发空命令。「设置样式」不展开二级菜单，直接打开检查器。

**卡片与几何**（`panels/context.rs`）：240 宽、30 行高、9 分隔高、6 内边距、8 屏幕边距；白底、1px `#E0E4EA` 描边、12 圆角、`0/10/28` 阴影；正文 13.5、快捷键 11.5 右对齐（墨色 38/142，禁用 176），hover `#F2F5F9`。实测高度：节点 369、多选 171、空白 219。默认在落点右下方；底部放不下时**整卡上翻**到落点上方，再水平/垂直夹进内容区——本机空白菜单因此出现在指针上方（`m3-context-blank.png`）。菜单是 `Order::Foreground` 的 egui `Area`，画在中央面板之后。

**键盘与关闭**：

- `↑↓` 只在**启用且非分隔**的行间移动，首尾环绕；无高亮时 `↓` 落第一行、`↑` 落最后一行。`Enter` 执行高亮行；`Esc` 收起；其余任意按键**收起但不吞事件**（`Ctrl+Tab` 等窗口快捷键继续生效）。
- 菜单键盘处理排在画布之前，否则 `Esc` 会先被画布当成清空选择、`↑↓` 会变成节点导航。
- 鼠标移动后才用 hover 接管高亮；点击卡片外关闭；打开那一帧的右键不会把菜单立刻关掉（`just_opened`）。
- 菜单打开期间 `canvas.set_input_blocked(true)`，点击与滚轮不漏进画布；开始页 / 命令面板 / 搜索条 / 帮助浮层 / 导出对话框存在时不再弹右键菜单，避免两层浮层叠加。

**验证**：

- `cargo test --workspace` **335 项全绿**（本轮 +5：菜单键盘 4 项 + 多选散落根 `same_parent_command` 1 项；`mw-shell` 105 → **110**）。fmt 干净；clippy 去重仍为存量 38（shell 6、core lib 11），0 新增。
- `cargo golden --tol 0` 连跑两次容差 0（菜单只在交互时出现，默认界面逐像素不变）。
- 真实输入 `scripts/m3-context.ps1`（自建 5 节点 `.zmind` 夹具，临时 `APPDATA` + `MW_WINDOW_STATE` 隔离，脚本自行起停 release 进程）：
  1. 节点菜单截图 `m3-context-node.png`（粘贴/粘贴样式禁用）→ `Esc`；`↓×2` 高亮「新建同级主题」`m3-context-nav.png` → `Esc`。
  2. 空白菜单像素三连：画布底色 → 白卡 → `Esc` 后回到画布底色（`251,251,252 → 255,255,255 → 251,251,252`），卡内探测点 = 落点 + (120,105)，证明卡片真的画在夹取后的位置、`Esc` 真的收掉。
  3. 选 b 后 `Ctrl` 点 a1 → 多选菜单 `m3-context-multi.png`（选中 2、移入同一父级可用）→ 点「移入同一父级」（落点 = a1 的父级 a），b 带自身子树挪到 a 下。
  4. 结构变化后**重新扫描调色板填充**取得 a 的新坐标（547,376.5），再右击 a → 菜单「复制」→ 右击空白 →「粘贴」：`m3-context-paste.png` 出现 8 个主题（根 3 子，a 的子树复制一份）。
  5. 空白菜单 `↓×5` 高亮「全部折叠」（`m3-context-collapse-highlight.png`）→ `Enter` → `m3-context-collapsed.png`；`Ctrl+S` 后解包 `.zmind` 断言：nodeCount 8（manifest 8）、`b.parentId = a`、根 3 子、项目乙与子项甲一各 2 份、`a.collapsed = true`、`root.collapsed = false`。
- release 产物 `target/release/mw-shell.exe` **19,986,944 B（19.99MB）**。

**已知缺口**：

- docs/06 §6 的「整理布局」未做（布局切换在检查器里，菜单暂无入口）；**大纲行菜单**（含「设为根主题」）与**标签页菜单**（关闭其他/复制路径等）未做；自由主题、边界、概要的右键创建入口未做。
- 菜单无图标、无二级子菜单；粘贴只认进程内节点剪贴板，不解析系统剪贴板里的纯文本/图片。
- 菜单与画布一样是 egui 自绘，不暴露无障碍角色；Windows 高对比度模式下颜色不跟随（全应用同一现状）。

---

### 4.30 场景插值：节点增删 / 折叠展开 / 布局切换过渡（M2c 收尾，2026-10-18）

**目标**：补齐 M2c 动效表最后三项。不另起一套布局或几何，只在两帧 `Scene` 之间插值：任何结构命令生效后，旧场景淡出、新场景落位，中间帧由纯函数 `mw_core::blend_scenes(from, to, t)` 生成。M2c 至此收口。

**数据层**（`mw-core/src/scene.rs`）：

- 所有可绘制项补 `alpha: f32`（节点 / 连线 / 轴线 / 边界 / 概要 / 关系线），默认 1；合成器与文字光栅化都按 alpha 混合。
- `blend_scenes(from, to, t)` 的规则：
  - `t ≥ 1` 直接返回目标场景。**刻意没有 `t ≤ 0` 提前返回**：t=0 也保留目标场景的 `ids/index/draw_order`，只用起点几何绘制。选择环、命中索引、面板取节点都按目标场景索引走；若返回旧场景，删除/新增节点后索引立刻错位。
  - 共同节点（按 id 配对）：`geom.rect` 与 `text_x` 线性插值。
  - 只在目标出现的节点：`alpha = t`；起点 = 目标父节点在旧场景中的盒子中心按同尺寸居中（`centered_at`），再插到目标盒——视觉上「从父盒里长出来」。
  - 只在起点出现的节点：克隆追加在目标节点之后，`alpha = 1 − t`，向父节点在目标场景中的中心滑出；父/自由主题根按 **id** 重映射（父可能也在本次删除里）。
  - 连线按端点 id 对 `(from_id, to_id)` 配对：两端都在则插值 `start/end`；新线 `alpha = t` 且终点从父侧端点「长」向子节点（否则子节点还在半路，线已先连到终点）；旧线 `alpha = 1 − t` 并向两端节点中心收缩。
  - 边界 / 概要按 id：框体、花括号、连接线、摘要主题盒走 `lerp_path`；出现/消失只改 alpha。关系线同理（曲线、箭头、标签位置、标签宽度）。
  - 轴线：同布局按序插值 path；**布局类型变化时旧轴 `1 − t` 淡出、新轴 `t` 淡入**（两条轴线的路径点数往往不同，交叉淡化比强行插值稳）。
  - `bounds` / `kind` 取目标场景。
- 折线点数不同则 `lerp_path` 返回 `None`、保留目标路径——形状差异过大时不硬插。

**壳层**（`mw-shell/src/canvas.rs`）：

- `Motion { from, start, duration }`；`progress()` 按 duration 归一，`scene(&target)` 返回 `blend_scenes(&from, target, ease_out_cubic(progress))`。
- `motion_duration(span) = (0.20 + span/4000).clamp(0.20, 0.34)` 秒；span 取所有共同节点矩形四边的最大差值；布局类型变化直接把 span 置 600（→ 0.34 s 上限）；`motion_span` 返回 `None` 就不起动画（换主题、纯样式变化不动位置）。
- `rebuild` 捕起点：已有在飞动画时，把**此刻屏幕上的 blend 结果**固化为新起点（连续按 Tab 不会位置回跳）；否则用上一帧场景。行内编辑期间不启动新过渡（编辑器必须贴着节点走），系统「减少动画」或 golden harness 则完全不建 Motion。
- `show`：动画未完成时每帧 `request_repaint`；合成器用 blend 帧，命中 / 选择 / 面板仍按**目标场景**算；播完立刻丢掉起点场景，避免每帧白算。
- 编辑覆盖层跟随插值盒：`EditorPlacement.rect` 取 blend 场景中该节点的矩形——新节点从父盒滑出时，文字不会先落在终点、盒子后到。
- `replace_session` / `swap_session` 直接清 motion：跨文档 id 大多对不上，动起来只会像闪屏。刻意不缓动相机：fit / 跳转是瞬时定位，只动内容。

**语义取舍**：新增节点从第一帧起就命中在目标位置，动画只是「看起来从父盒长出」；删除节点在文档里已消失，淡出的旧盒不可点。折叠 = 目标场景不含被折叠子树，子树整体向父盒收缩淡出；展开反之。这样选择、光标、连线锚点从不追随视觉中间态，不会出现「看着在 A、点得到 B」。

**验证**：

- `cargo test --workspace` **346 项全绿**（本轮 +11：`blend_scenes` 7 项——共同节点插值 / 删除向父盒滑出并正确追加 / 新增从父盒淡入 / 被删父级按 id 重映射 / 连线插值 / 端点精确 / 换布局轴线交叉淡化；Motion 4 项——几何未变不启动 / 新增与改盒触发 / 时长随距离并夹取 / 缓动端点与单调性。mw-core 114 → **121**、mw-shell 110 → **114**）。
- clippy 去重仍为存量 38（shell 6、core lib 11），0 新增；`editor_ui` 因插值盒参数变成 8 参超限，用 `EditorPlacement { viewport, rect }` 收口。fmt 干净。
- `cargo golden --tol 0` 连跑两次容差 0（harness 强制 `reduce_motion`，默认界面逐像素不变）。
- 真实输入 `scripts/m2c-anim.ps1`（7 节点夹具，临时 `APPDATA` + `MW_WINDOW_STATE` 隔离，脚本自行起停 release 进程；先在检查器里校准鱼骨/思维导图按钮坐标）：20 张截图覆盖五组——加节点（`anim-add-early/mid/settled`，半透明节点从根盒长出、连线跟着长）、提交（点空白让编辑器失焦提交，绕开 IME 组合期 Enter 被输入法吃掉）、删除（`anim-delete-*`，文档已少一个节点但旧盒仍半透明，随后消失）与撤销恢复、折叠（`anim-collapse-*`，旧布局仍在、子节点渐隐并向父盒滑）、展开（`anim-expand-*`，先看到折叠态再淡入）、布局切换与切回（`anim-layout-*` / `anim-layout-back-*`，中间帧节点明显重叠、轴线交叉淡化，0.34 s 后落位）。每次按键前把光标停到标题栏，避免悬停锚点入镜；`-early` 在按键后立即截，因为 0.20–0.34 s 的窗口很容易被截图延迟吃掉。
- release 产物 **20,054,016 B（20.05MB）**；冷启动 485–532 ms；私有内存 320.4–321.9 MB、工作集 373.4–375.0 MB。

**已知缺口**：

- 没有应用内「减少动画」开关，只跟随系统 `SPI_GETCLIENTAREAANIMATION`；系统开关的人工切换未纳入自动化。
- 时长只随跨度粗调，没有按帧率/负载降级；动画期间未单独计量帧耗时（合成路径与常帧相同，但没做专项探针）。
- 演示模式（F5）已由 §4.32 补齐；快捷键配置仍未做；拖拽 / 平移 / 缩放本身不插值，只有场景结构变化走 Motion。

---

### 4.31 M5 性能实测与打包（2026-10-18）

> **本节是 P13 攻关前的实测记录**（2026-10-18 白天）。P13 的字体 mmap、图集缩容与 CPU 镜像移除见 **§4.33**；本节「P13 红」的结论已被 A+B+C 的 **10k 五分钟稳态 284.4–287.9MB** 取代，**P13 判 PASS**（§4.33 末尾有判定口径与限制条件）。

**目标**：把 P1–P14 从「架构上应该够快」变成「release 真实窗口里有数字」。本轮只做**测量基建 + 现状取证 + 便携产物**，不为了过预算改渲染架构；P13 红就写红，不调口径。

**探针**（都在 release 构建里，环境变量不设时零开销）：

- `crates/mw-shell/src/perf.rs`：`MW_PERF_LOG=<path>` 打开后按「名字<TAB>相对毫秒」逐行落盘。里程碑 `process_start`（`Instant` 在 `run_native` 之前创建）→ `shell_new` → `doc_open_start` → `doc_open` → `first_ui`（开始页）/ `first_canvas`（首个画布帧）。每行立即 flush，进程被强杀也留得下；写失败静默。
- `crates/mw-shell/src/stats.rs`：`MW_STATS=1` 在右上角画 286px 卡片（FPS、墙钟 p50/p95、UI p50/p95、节点总数/可见数、缩放、省渲染、过渡态）。240 帧滚动窗；**帧间隔 >250 ms 视为停手，整窗丢弃重算**——否则空闲时按需重绘的几百毫秒会把 FPS 拉成个位数。`MW_STATS_LOG` 每 60 帧追加一行 TSV，脚本读最后一行断言 P5。
- `Canvas::perf_snapshot()`：按视口统计可见节点、读 `budget.low_render()` 与 `motion.is_some()`，只给探针用，不改变渲染路径。
- `scripts/make-sample.mjs --nodes=N`：BFS 树生成任意规模 `.zmind`（第 53 个加长文本、第 40 个加标签、第 97 个加图标、第 211 个加备注，确定性）。**大文档必须用 `mindmap` 布局**：`logicRight` 会退化成一列，5k 时只有 48% 可见，拿它测 P5 等于放水。
- `scripts/m5-perf.ps1`：隔离 `APPDATA`、`MW_WINDOW_STATE=off`，等**尺寸 ≥600×400** 的主窗口（避开 winit 的 15×15 辅助窗口）；`AttachThreadInput` + `SetForegroundWindow` 后注入真实 Win32 输入——滚轮 ±120 交替（相机贴住 fit，保证全程 93% 可见）+ 40 步中键拖拽，截图后解析最后一条统计。`-StartPage` 测 P2，`-Interact` 才跑 P5。
- `scripts/m5-memory.ps1`：同一套窗口等待逻辑，按 30 s 采 `PrivateMemorySize64` / `WorkingSet64`，默认 5 分钟（P13 口径）。
- `scripts/package-release.ps1`：`cargo build -p mw-shell --release` → 暂存 exe + README + LICENSE → 便携 ZIP + SHA256；ZIP 超过 30MB 直接 `exit 1`（P14）。**没有 LICENSE 文件时只告警**，产物仍生成。

**启动实测**（ms，同机 1620×1080 @150%，热文件缓存）：

| 语料 | shell_new | doc_open_start | doc_open | first_canvas | P1 进程→首帧 | P3 打开→首帧 |
| --- | --- | --- | --- | --- | --- | --- |
| 5k（logicRight，48% 可见） | 237.4 | 286.4 | 341.8 | 350.8 | 350.8 | 55.4 |
| 5k（mindmap，93% 可见） | 227.0 | 279.2 | 328.3 | 338.6 | 338.6 | 49.1 |
| 5k（重打包后复测） | 208.8 | 262.6 | 316.7 | 325.8（first_ui 317.6） | **325.8** | **63.2** |
| 10k | 213.2 | 262.9 | 374.2 | 386.4 | 386.4 | 111.3 |
| 20k | 207.4 | 265.1 | 504.6 | 522.3 | 522.3 | **257.2** |
| 开始页 | 237.0 | — | — | first_ui 291.8 | **291.8**（P2） | — |

口径与取舍：P1 预算 2.0s、P3 预算 1.0s、P4 预算 3.0s，全部远低于预算。但这是**热文件缓存**下的进程启动，不是重启机器后的真冷启动——没有清 standby list，也没有做三次取中位；表格能证明「不是启动慢」，不能证明「冷启动 325.8ms」。`shell_new` 稳定在 207–237ms，说明 egui/wgpu 初始化是启动固定成本；文档规模主要影响 `doc_open`（5k 54ms、10k 111ms、20k 240ms）。

**P5 帧率**（5k，mindmap，93% 可见，滚轮缩放 + 中键拖拽约 4 秒）：

| 帧数 | FPS | 墙钟 p50 | 墙钟 p95 | UI p50 | UI p95 | 可见 |
| --- | --- | --- | --- | --- | --- | --- |
| 60 | 43.0 | 16.65 | 40.25 | 1.69 | 2.31 | 4671 |
| 120 | 50.2 | 16.67 | 19.21 | 1.67 | 2.18 | 4675 |
| 300 | 60.0 | 16.67 | 17.59 | 1.65 | 1.78 | 4639 |
| 360 | 60.0 | 16.65 | 17.83 | 1.64 | 1.78 | 4664 |

- 稳态 **60fps**，墙钟 p95 17.73ms、UI p95 1.78ms，**P5 PASS**（≥55fps）。
- 第一行 43fps 是窗口创建/输入注入初期的冷缓存，**不作为稳态**；第 300 帧起稳定 60fps。UI 线程只占 1.6–1.8ms，说明瓶颈在呈现/垂直同步，不在布局或 egui 逻辑。
- `logicRight` 的 5k（48% 可见）同样 60fps，但**不用于验收**——P5 的可见比例前提不满足。10k 的统计也跑到了 60fps，但 10k 语料是 `logicRight`（47% 可见），只作内存与规模参考。

**内存**（`PrivateMemorySize64` / 工作集；**下表是 P13 攻关前的历史基线，A+B/C 见 §4.33**）：

| 场景 | 私有 | 工作集 | 判定 |
| --- | --- | --- | --- |
| 开始页空应用 | 318.2–320.8MB | 372.9MB | 已超 300MB |
| 5k mindmap | 330.7MB | 384.2MB | — |
| 10k | 340.1MB | ~390MB | — |
| 10k 空闲 5 分钟稳态 | 335.2MB（区间 334.8–337.6） | 379.0MB（区间 379.0–390.4） | **P13 FAIL** |
| 20k | 357.2MB | — | — |

- **P13 当时不达标**：预算 300MB，10k 稳态仍 ~335MB。**归因不是文档规模**——空开始页已 318–321MB，10k 文档只多 ~16–19MB。基线来自 DX12/wgpu（适配器 + 交换链 + 管线）、系统雅黑两套字体（`msyh.ttc` + `msyhbd.ttc` ≈37MB）、4096² R8 字形图集（CPU + GPU 各 16MB）、全窗口渲染目标。**后续 A+B+C 已把空应用降到 276.6 → 264.6MB、10k 五分钟 299.6 → 284.4–287.9MB，P13 判 PASS，见 §4.33。**
- 缩图集到 2048² 只能省约 24MB，340→316 仍红；要过预算得动 wgpu/DX12 基线或字体驻留策略，属于独立优化项，**本轮不假装解决**。
- 10k 五分钟后从 340.1 降到 335.2（第 2.5–4.5 分钟稳定在 334.8，末次工作集回落到 379.0），说明没有持续泄漏；但「稳态低于 300MB」仍未达到。

**编辑 / 保存 / 检索**（release，核心层示例，不含 GPU 合成）：

- `mw-core/examples/perf_scene.rs` 扩出 P6/P8/P9：5k `build_scene` 中位 **8.4ms**（p95 16.2ms，P8 预算 120ms）；改字 + 全量重建 p95 **9.8ms**（P6 预算 50ms，代理口径）；撤销/重做 + 重建 p95 **9.0 / 8.0ms**（P9 预算 30ms）。**P7 仍是全量重排，没有增量布局**，9.8ms 不能当作 P7 达标。末尾跑 3 次 20k 冷缓存拼音查询：**38.2–41.7ms**，二次查询 **11.7–14.3ms**，**P12 PASS**（预算 100ms）。优化把逐字 `Vec<String>`/`to_lowercase` 与整份 `Key` 克隆去掉，缓存上限提到 32k，20k 文档首次搜索后可全量驻留。缓存上限 8000 对 20k 文档偏小，但一次查询超限不是泄漏。
- 新增 `mw-format/examples/perf_save.rs` 量 P10（release）：5k 保存 p50/p95 **13.5/14.0ms**、10k **24.6/27.4ms**、20k **49.0/50.3ms**，全部低于 150ms 预算。**自动保存已移出 UI 线程**（2026-10-18 补做）：`autosave::Background` 在 UI 线程只克隆 `Snapshot`（doc id/meta/root/nodes/revision，不含撤销栈与搜索缓存），命名字线程 `zmind-autosave` 里用 `Document::from_parts` 复核结构后调 `mw_format::save`；单飞 + 后续修订合并为最新一份，完成结果按 `epoch` 隔离（旧文档的完成不冒充新文档修订号），`Ctrl+S` 的 `invalidate` 把在途槽位记入 `stale`、迟到写完成后再删一次，避免恢复点「复活」。UI 快照成本 5k p50/p95 **2.1/2.7ms**、10k **4.4/4.7ms**、20k **8.9/9.2ms**，均在 16.7ms 帧内——**不是零成本，但不再是 28ms 的长任务**。失败静默退避重试、写线程起不来时降级为同步写、`on_exit` 最多等 1.5s 收尾。真实窗口抽验：脏文档 3s 后恢复点落盘（mtime 比正本新），`Ctrl+S` 后槽位删除、正本重写。
- **P11 导出已移到工作线程**（2026-10-18）：新 `export_job.rs`——UI 线程只克隆 `Scene`/`Theme`（5k 0.94ms、20k 4.0ms），命名字线程 `zmind-export` 自建 `TextEngine` 与一次性 `Compositor`（画布那台会重建导出尺寸的目标纹理，不能借），排版、离屏渲染、PNG/PDF 编码与写盘都在工作线程；先写同目录 `.part` 再原子改名，失败删除临时文件。单飞、在途每 120ms 催一帧、退出最多等 4s。实测 5k 工作线程 **70.9ms**、提交→完成 107.6ms；20k 63.8ms / 101.4ms。**口径限制**：5k/20k BFS 语料是 1:40 极端长宽比，8192 单边 / 32MP 护栏把 2× 缩到 0.057×/0.014×（206×8192 / 56×8192），不是字面 2×，也不能代表 32MP 满幅成本；真实 2× 由 3 节点样例验证（910×300，PNG 38.8ms、PDF 30.7ms）。
- **P15 休眠往返、P11 满幅 32MP 2× 与导出期间逐帧卡顿仍未测**；P6/P9 是核心层下界，命令提交帧的 egui/GPU 开销没有单独计量。

**打包**：`scripts/package-release.ps1` 产出 `dist/zhisi-0.1.0-win64.zip`，**7,751,953 B（7.39MB）**，exe **20,073,984 B**，SHA256 `2eab308c…cbd9`；ZIP ≤30MB，**P14 PASS**。注意三点：① 这是便携 ZIP 不是安装器（本机无 Inno Setup/NSIS，MSI/EXE 安装包未产出）；② 仓库根没有 `LICENSE` 文件，脚本只告警，产物内只有 README；③ exe 从 20,054,016 B 增到 20,073,984 B，来自 `perf.rs`/`stats.rs` 与采样代码，量级符合预期。

**验证**：`cargo test --workspace` **353 项全绿**（P11 导出任务 1 项 + P10 后台保存 3 项 + M5 stats 2 项 + P12 单测 1 项；mw-shell 120、mw-core 122）；clippy 去重仍为存量 38（shell 6），P11 新增代码 0 条；fmt 干净；golden 本轮未改渲染默认路径（探针不设环境变量时零行为），`cargo golden --tol 0` 连跑两次通过。

**已知缺口**：P7 没有增量布局（只有全量重排代理）、P15 休眠唤醒未测；P11 的时间预算已过，但 5k/20k 合成语料被像素护栏缩限、不是字面 2×，满幅 32MP 与导出期间逐帧卡顿未量；P10 虽已后台化，但 UI 快照克隆仍有 4.7–9.2ms（帧内，非零成本），且自动保存只覆盖活动标签、未做整帧卡顿采样；P1/P2 是热缓存口径，未做真冷启动三次中位；只有一台机器、没有低档机降级验证；**P13 在本轮为红，攻关 A+B/C 见 §4.33**；无安装器、无 LICENSE 文件。

---

### 4.32 演示模式 F5（M2c 最后一项功能，2026-10-18）

**目标**：把 docs/02 §9 冻结的 P2 规格落地——`F5` 从选中节点（否则根）开始，先序逐节点展示，每次居中 + 350ms 缓动，当前路径高亮、其余压暗，底部控制条给进度与上一步/下一步/退出，`H` 隐藏控制条，`Esc` 退出并精确还原进入前的视口。不新增一条绘制管线。

**实现**（`mw-shell/src/demo.rs` + `canvas.rs` / `main.rs` 接线）：

- **状态机**：`Demo` 保存可见前序 `order`、当前下标、起止相机与起始时刻、进入前相机 `saved`、场景副本和主题副本。`F5` 进入时先把正在编辑的文本提交（演示中禁止编辑），清掉框选/拖拽/连线/悬停/右键菜单，再把画布输入整体锁住；壳层隐藏标题栏、标签条、状态栏、大纲、检查器、小地图、面包屑、搜索条、加子按钮与悬停锚点。
- **前序**：`Scene` 节点本身就是文档前序（`compute_layout` 逆序入栈遍历，折叠分支不下探），因此索引升序就是演示顺序；自由主题按各自前序排在主树之后。**第一版按 `draw_order` 的「深度 + y/x」重排同父兄弟，真实探针立刻抓到根的第 2 步跳到了视觉更高的「项目乙」而不是文档里的「项目甲」**——已改为索引升序，并加了「左右分叉时仍按文档顺序」的回归测试。
- **高亮与深色**：进入时克隆 `Scene`，把非路径节点 alpha 压到 0.16、连线/轴线/边界/概要/关系线压到 0.10，路径节点与路径连线保持 1.0；再克隆主题只改 `ui.canvas_bg`/`ui.bg` 为深灰蓝、`ui.dot` 透明。**不整体换深色主题**——场景里的节点颜色是按原主题解析好的，换主题会和已解析样式打架。文字管线与 `build_layers` 本来就尊重 alpha，所以压暗对盒、连线、文字同时生效。
- **相机**：每步目标 = 节点矩形中心 + `min(视口×0.45 / 节点宽高)` 夹在 0.55–1.25 倍；350ms ease-out-cubic，中心线性插值、缩放按对数插值（人眼感知的是倍率）。连续按方向键时从**当前插值相机**重新起步，不跳回上一段起点；系统「减少动画」时 `duration = 0` 直接落位。退出把 `saved` 相机还给画布，所以 `Esc` 后与进入前逐帧一致。
- **控制条**：底部居中 460px（窄窗最小 280px）、距底 76px、深色半透明圆角卡；左侧 `n / N` 与当前主题名（按字符截断 16），右侧 `‹`、`›`、`退出`，到端点时按钮禁用；`H` 隐藏/再按恢复，点击与键盘走同一组 `Canvas` 方法。按键路由在演示态整体接管：`Esc`/`F5` 退出，`→`/空格 下一步，`←` 上一步；其余编辑键不生效。
- **不切系统全屏**：演示只隐藏壳层 chrome，窗口仍是原窗口（与 `F11` 专注态分开）。这是对规格的刻意偏离：切全屏会改窗口几何与 DPI 状态，退出时多一层还原风险，且演示条需要窗口内空间。

**验证**：

- `cargo test --workspace` **361 项全绿**（demo 8 项：可见前序遍历、左右分叉仍按文档序、折叠分支跳过、从选中节点起步、路径 alpha、首尾步进边界、相机缓动中点与端点、减少动画直接落位）；`mw-shell` 120 → **128**。
- clippy 去重存量 **38**（shell 6，demo 新增代码 0 条）；fmt 干净；`cargo golden --tol 0` 连跑两次容差 0（演示只在按 `F5` 后接管，默认界面逐像素不变）。
- 真实输入 `scripts/m2c-demo.ps1`（release、150%、临时 `APPDATA`/`MW_WINDOW_STATE`、脚本自起停）：15 张截图 `shots/m2c-demo/demo-*.png`，断言标题栏像素亮度 247 → 演示态 20 → `Esc` 后 247；覆盖进入、根/甲/甲一/乙步进、350ms 早/中/落定帧、`H` 隐藏/恢复、控制条鼠标点击下一步/上一步/退出、`Esc` 退出与相机还原、**相机在飞时 `Esc`**；标题与进程存活均断言。
- release exe **20,251,136 B**（较 M5 打包时的 20,073,984 B 增 177,152 B）；本轮未重测启动/内存，M5 指标仍是上一构建的数字。

**已知缺口**：没有应用内「减少动画」开关（只跟随系统设置）；演示没有自动播放/计时器、演讲者备注、多显示器演示窗口；演示态不持久化；控制条没有可拖拽位置。快捷键配置仍是 M2c 唯一未做项。

---

## 4.33 P13 内存攻关（2026-10-18 起，2026-10-19 凌晨 A+B+C 全部实测通过）

**目标**：P13 = 10k 文档空闲五分钟稳态 `PrivateMemorySize64` ≤ 300MB。攻关前基线：空开始页 **317.5MB**、10k **338.3MB**（短采样），原始五分钟稳态 **335.2MB**（工作集 379–390MB）。归因：DX12/wgpu 基线 + egui 侧两份雅黑 TTC（约 37MB 私有堆）+ 4096² R8 图集（CPU 16MB）+ 全窗口渲染目标；文档数据本身只占约 20MB。

**优化 A：egui 系统字体改只读内存映射（已用 release 验证）**

- `crates/mw-shell/src/fonts.rs` 不再 `fs::read` 整个 `msyh.ttc`/`msyhbd.ttc`，改为 Win32 `CreateFileMappingW` + `MapViewOfFile`（`PAGE_READONLY` / `FILE_MAP_READ`），并交给 epaint 0.37 的 `FontData::from_blob(Arc<MappedFont>, index)`；映射在最后一个 `Arc` 释放时 `UnmapViewOfFile`。
- 关键前提已核实：epaint 0.37 的 `FontFace` 用 `self_cell` 持有 `skrifa::FontRef`，**直接借用 blob**，不会走 `ab_glyph::FontVec` 那种整份复制；否则 mmap 没有意义。
- `crates/mw-shell/Cargo.toml` 增加 `Win32_System_Memory` 特性。非 Windows 分支不存在（壳层本来就仅 Windows）。
- 实测收益：空应用 317.5 → **283.0MB**（只上 A 时），10k 338.3 → **304.4MB**；约 **−34MB**，符合两份 TTC 的预期。

**优化 B：字形图集 4096² → 3072²（已用 release 验证）**

- `crates/mw-render/src/glyph.rs` 的 `ATLAS_SIZE = 3072`：单通道 16MB → 9MB；按 14px 字形约 4 万槽位，真实 10k 中文文档按三档字号估约 1.2–1.5 万，留约两倍余量。3072 是 256 的倍数，满足 r8 行对齐。
- 视觉影响：图集 UV 末位变化导致 **527 个像素最大差 1**（0.030%，两次运行完全一致）。已按纪律 `cargo golden --update` 重拍基线，随后 `cargo golden --tol 0` **连续两次通过**。
- 实测收益：空应用 283.0 → **276.6MB**；10k 304.4 → **296.9MB**（短采样）。
- **五分钟稳态**（`scripts/m5-memory.ps1 -Minutes 5 -SampleSeconds 30`，tag `atlas3072-10k-5min`）：297.0 → 296.9 → 296.7（0.5–3.5 分钟）→ 297.2（4 分钟）→ **299.5 / 299.6MB（4.5–5 分钟）**；工作集 357.0 → 360.1MB。结论：**首次低于 300MB，但末段有约 +2.9MB 的台阶，余量只有 0.4MB**，不能算稳健通过。日志 `target/m5-run/memory-atlas3072-10k-5min.tsv`。

**优化 C：去掉图集 CPU 镜像（已完成，release 实测 −12MB）**

- 背景：A+B 后只剩 0.4MB 余量，而 CPU 侧仍常驻一张 3072² 镜像（9MB），只为把脏矩形从整张图集里切片上传。
- 改动：`AtlasUpdate.rects` 从 `[u32;4]` 改为 `AtlasRect { x, y, w, h, pixels }`——光栅化出的位图**紧排**随增量直接交给 GPU（`bytes_per_row = w`），CPU 不再保留整张图集；`GlyphAtlas::data()/size()` 删除，`reset()` 不再 `fill(0)`。
- 失去镜像后无法「整张重传」，所以代次断档改为显式恢复：`Compositor::upload_atlas(&update)` 发现 `generation` 不连续时置 `atlas_desync`，新增 `take_atlas_desync()`；画布看到后调用新增的 `TextEngine::reset_atlas()`，下一帧重排全部字形。图集重排不清 GPU 纹理——旧 UV 已失效，残留像素采不到。
- `export_job.rs` 不再手工构造 `AtlasUpdate { generation: u64::MAX, reset: true }` 强制全量，而是消费 `text.paint()` 返回的真实增量；导出器与图集都是新建的，代次 1 接 0。
- 已完成的验证（2026-10-18 20:5x，当前树）：`cargo check --workspace --all-targets` 无错误；`cargo test --workspace` **361 项全绿**（含 `mw-render` 55 项离屏上传/渲染、增量两次上传、透明底与字形像素断言）；`cargo clippy --workspace --all-targets` 去重存量仍 **38**（shell 6，新代码 0 条）；`cargo fmt --all -- --check` 干净。
- **C 的复测结果（2026-10-19 02:1x–02:2x，同一个 release 二进制：`mw-shell.exe` 20,292,096 B，2026-10-19 01:58 构建，含 A+B+C 与 v3/v4/v5-a）**：
  - **10k 五分钟稳态 · run 1**（`scripts/m5-memory.ps1 -Minutes 5 -SampleSeconds 30 -Tag p13c-final`，日志 `target/m5-run/memory-p13c-final.tsv`）：私有内存 **287.9 → 287.8（0.5–2.0 min）→ 284.4（2.5–4.0 min）→ 285.0MB（5.0 min）**，全程区间 **284.4–287.9MB**，工作集 357.2 → **354.7MB**。**没有上行台阶**（A+B 那次 4.5 min 的 +2.9MB 没有复现，这一轮的台阶是向下的 −3.4MB）。对 300MB 预算：**最差样本留 12.1MB 余量，末次留 15.0MB**。
  - **10k 五分钟稳态 · run 2（独立复采样，日志 `memory-p13c-final-2.tsv`）**：**289.6 → 289.5（0.5–2.0 min）→ 289.4（2.5–5.0 min）MB**，全程区间 **289.4–289.6MB**，工作集 358.5 → **358.7MB**。整条曲线是平的（0.2MB 内摆动），**同样没有上行台阶**。两轮合看：进程起点比 A+B 那轮低约 7MB，且第一轮「2.5 min 掉 3.4MB」那种台阶**没有复现**——它是一次性的分配/装箱行为，不是持续增长的证据。**两次都不反弹**才是判 PASS 的口径，只看 run 1 会把余量误算成 15MB。
  - 两轮最差样本 **289.6MB** ⇒ 对 300MB 预算**留 10.4MB 余量**。
  - **开始页空应用**（`-Doc "" -Minutes 1 -SampleSeconds 15 -Tag empty-c-current`）：**264.6 / 264.6 / 264.6 / 264.5MB**，工作集 333.8 → 333.7MB。对 A+B 的 276.6MB 是 **−12.0MB**——比「约省 9MB」的预期还多，因为省掉的是整张 3072² R8（9MB）**加上**装箱期间的临时行缓冲与 `reset()` 的 `fill(0)` 触碰页。
  - 相对 A+B 五分钟稳态 299.6MB：**−14.6MB**。
  - **导出路径实拍**（C 把 `export_job.rs` 的「手工构造 `reset:true` 全量包」换成消费真实增量，是这轮风险最高的一处）：release 真实输入 `Ctrl+E` → 默认（PNG / 1× / 文档底色）→ 原生保存对话框 → `scripts/m4-export.ps1 -Tag p13c-export`，产物 `shots/qa/out/p13c-export.png` **950×300 / 35,832 B**，逐像素复核：中西文混排（`由 Node 脚本生成`）、粗体根节点、分支色卡片、关系线 `①` 标签全部到位，抽样暗色墨迹 3200/71250——**图集增量在新建合成器（代次 1 接 0）上没有丢字形**。
  - 其余门槛按纪律复跑（在 v5-a 那轮一起跑的，同一个二进制）：`cargo test --workspace` **380 项全绿**、`cargo golden --tol 0` **连跑两次通过且未重拍基线**、`cargo fmt --all -- --check` 干净、`cargo clippy --workspace --all-targets` 去重 **37**（`mw-shell` 5，**0 新增**）。
- **判定：P13 PASS。** 依据是**同一次 C 落地后的两轮独立五分钟采样都不反弹**（284.4–287.9 与 289.4–289.6，最差 289.6MB，余量 10.4MB）。口径限制必须一起留着：这是**本机、150 % DPI、1620×1080、隔离 `APPDATA`、各一次五分钟稳态**的数字，不是长时间 soak，也不含显存（工作集比私有内存高约 69–70MB，其中大部分是 DX12/wgpu 侧）；A+B 那轮 4.5 min 的台阶与 run 1 的 −3.4MB 台阶说明这个量级上还有 1–3MB 的分配器噪声，**余量 10MB 是「够睡觉」而不是「可以随便花」**。往后再吃掉 10MB 以上的改动，P13 要重新复测，不能引用今天的数字。

**如果哪天 C 之后又不达标，下一步选项（按性价比，2026-10-19 起是备用清单）**：

1. 图集 3072 → 2048（再省 5.2MB 堆；容量约 1.9 万槽位，10k 真实文档勉强够，20k 或字号档位多时可能触发整张重排，需用真实中文长文档压测）。
2. `DWriteMeasurer`/`ShapeCache` 各 20k 条缓存（10k 文档实测常驻约 1–4MB）——不能简单缩上限，否则每次重排全量重测，需做可见集裁剪或 LRU。
3. DX12/wgpu 基线（空应用仍 ~277MB）：可做独立实验比较 Vulkan/GL 后端或延迟创建交换链，但会牵动渲染验收，风险大，不建议在没有数据前动。
4. 启动页/恢复槽的常驻快照：确认空闲时没有额外的文档副本。

**纪律**：P13 只认 release + `PrivateMemorySize64` + 五分钟稳态；工作集与 GPU 显存另记，不混用。本轮 A/B 的数据来自同一台机器、150% DPI、1620×1080 窗口、隔离 `APPDATA`，与 §4.31 基线同口径。

---

## 4.34 v3 核心打磨：配色 / 交互 / 字体（2026-10-18 晚）

**方向调整**：用户明确「先暂停复刻 XMind 全部功能，只聚焦思维导图核心基础功能，重点打磨整体配色、交互体验、字体样式，不做附加/高级功能」。本轮**不新增任何功能入口**，只改已有核心路径的观感与手感。

**配色：从「红绿灯」到分支色系**

- `snow`（默认）的 `palette` / `palette_fill` 整体降饱和一档：teal `#1f9e86` / coral `#e4635b` / amber `#de9440` / green `#4e9e6b` / indigo `#5578d0` / violet `#8464c4` / rose `#c9648f` / slate `#7a8698`；一级卡片提亮表同步换成 `#8fd0bc` / `#f09b94` / `#f0be8c` / `#9bcba8` / `#a8bdea` / `#bfaee0` / `#e3aec8` / `#bac2cc`。
- 新增 `Rgba::mix(accent, t)`（保 alpha、端点钳制）。`resolve_all` 里 **depth ≥ 2 的卡片**按所属分支色混 **7% 填充、20% 描边**（`level.fill.mix(branch, 0.07)` / `level.stroke.mix(branch, 0.20)`）。一条分支因此读成一个色系，而不是「一排白卡 + 一根彩线」；节点上显式 `fill`/`stroke` 覆盖仍然优先。
- 文字色不动（近黑），一级卡片仍是实色 + 深字，避免白字在浅底上的低对比。

**字体：拉开层级**

- 根节点 22 → **26**（四套主题统一），层级字号变成 **26 / 17 / 15 / 14**；行高仍由 `text::line_height = font_size × 1.45` 决定，CJK 多行不挤。
- 中英混排、禁则、DirectWrite 度量与字形图集**一行未改**——这轮只调令牌，不碰排版引擎。

**交互：方向键导航 + 面板开合重算相机**

- **方向键在节点间移动选择**（此前只有 `Ctrl+Shift+←→` 升降级、`Alt+↑↓` 重排，裸方向键无响应）。`directional_neighbor` 纯几何打分：主轴距离 + 1.8 × 侧向偏移，侧向偏移超过主轴 1.6 倍的候选直接排除；无选中时第一下落到根。`reveal_node` 只做**最小平移**（四周留 48 CSS px），连按不会一跳一跳地重新居中。排在 `Ctrl+Shift+←→`、`Alt+↑↓` 之后消费，修饰键优先级不变。
- **面板开合重算相机**（§4.20 遗留）：新增 `last_view_center` / `last_window_size`，每帧比较画布矩形中心在窗口里的位移，按 `Δcenter / zoom` 反向平移相机——`F8`/`F9` 的 0.2 s 宽度动画期间画面**原地不动**，缩放不变；整窗尺寸变化（用户缩放窗口）不补偿，否则画面会被钉死在屏幕上。忽略阈值取 `1e-3 px`，不是 0.1 px。

**验证**

- `cargo test --workspace` **368 项全绿**（`mw-core` 122 → 124：`mix` 端点/alpha、二级染色；`mw-shell` 128 → 133：面板补偿 3 项、方向键几何 2 项）；`cargo fmt --all -- --check` 干净；`cargo clippy --workspace --all-targets` 无新增（存量 38 条不变）。
- golden：主题令牌变了 ⇒ `cargo golden --update --warmup 5` 重拍壳层基线，随后 `--tol 0` **连跑两次通过**；画布两份基线同步重拍（`MW_GOLDEN_UPDATE=1 cargo test -p mw-render --test offscreen -- canvas_matches_golden_baseline`、`... --test text_render`），全库复测通过。
- 真实输入 `scripts/v3-core.ps1`（release、150% DPI、1620×1080）：`F8` 关大纲 → `Home` → 点空白 → `→` 选根 → `→` 换节点 → `←` 回根。**`arrow_back_diff = 0`**（一去一回逐像素一致）、**`closed_diff = 0`**（开检查器再关，画面逐像素还原）、`panel_shift_diff = 0.751%`（开面板期间只有亚像素重采样差，裁图 `shots/qa/crop-before.png` 与 `crop-open.png` 逐像素相同）。截图 `shots/qa/v3-*.png`。
- release exe **20,246,528 B（20.25 MB）**，22:25 构建。

**踩坑**

1. **egui 0.37 没有 `Context::screen_rect`**：`RawInput.screen_rect` 在 `InputState` 里叫 `viewport_rect`，要用 `ctx.input(|i| i.viewport_rect())`。
2. **裸 `Esc` 会丢掉画布焦点**（§9-4 的老坑）：探针里「先清空选择、再按方向键」必须先点一下空白把焦点要回来，否则方向键根本没进画布。
3. **补偿的忽略阈值不能取 0.1 px**：面板动画十几帧，每帧丢掉 0.05 px，开合一次就累计约 1 px，`closed_diff` 从 0 变成 1.1%——看着像"关不回去"。阈值压到 `1e-3 px` 后归零。

---

## 4.35 v4 核心打磨第二轮：面板暗色 / 大纲观感 / 编辑器字体一致（2026-10-19 凌晨）

**承接 v3**：口径不变——只做思维导图核心，打磨**配色 / 交互 / 字体**，不加附加功能、不开新命令入口。v3 把画布本体拉齐了，这轮收尾三处「一眼看出去还不是同一个应用」的裂缝：暗色主题下 egui 控件仍是浅色的、大纲面板像文件管理器、以及**一按 `F2`，节点里的字会跳位**。

**① 大纲面板（任务 #61）**

- 行改成固定 27 CSS px 的自绘行（`show_rows` 虚拟滚动照旧），三态互斥：选中 = 5 px 圆角 `accent_soft` 底 **+ 左侧 2×19 px 强调色指示条**；hover = `ch.hover`；其余无底。整行变色不如「一条竖线」读得快，也更像编辑器（VS Code / XMind 大纲都是这个形状）。
- **缩进引导线**：`INDENT = 15 px`/层，每层在该层的 x 上画一条 `ch.border` 竖线，x 先 `.round()`——不取整会在半像素位置糊出两条淡线。「这行属于哪个分支」不再靠数格子。
- **分支色跟画布同源**：`OutlineRow` 多带一个 `branch`，重建行时直接从场景样式取 `branch_color`（孙辈继承父分支），chevron/圆点用 `theme.ui.dim.mix(branch, 0.68)` 染色，hover 时切强调色。有子节点画**手画小三角**（两条 `line_segment`），叶子画圆点——9 px 的字体 `▾` 在 125 %/150 % DPI 下会糊成一团，这是像素级验证过的取舍。三角区可点，直接发 `Action::Toggle`。
- 文字分三层：根 `ch.text`、正文 `ch.faded(text, 0.88)`、辅助 `ch.dim`。
- 新单测：`rows_carry_the_same_branch_colors_as_the_canvas`（含「孙辈继承父分支色」）、`switching_theme_repaints_the_branch_colors`。

**② 面板与浮层的暗色跟随（任务 #62）**

症状：切到 `inkNight` 后画布和面板底色都变深了，但 egui 控件（输入框、hover 底、焦点环、**光标**）还吃 `Visuals::light()`，表现为「深底上一块白斑」。

- `ui_visuals_for(chrome)`（`chrome.rs:208`）把所有槽位改成从 `Chrome` 取：`window_fill` / `panel_fill` / `extreme_bg_color` / `text_edit_bg_color` 走 `panel`，`faint_bg_color` / `code_bg_color` 走 `panel2`，`widgets.{noninactive,inactive,hovered,active}` 四态的 `fg_stroke/bg_fill/weak_bg_fill/bg_stroke/corner_radius` 全部重写，`weak_text_color = ch.dim`、`selection = { accent_soft 底, ch.text 描边 }`。
- **光标与输入法下划线跟字色走**：`text_cursor.stroke = 1.5 px ch.text`，`ime_composition.active/inactive = ch.text / ch.dim`。默认那支 egui 深蓝贴在近黑面板上等于隐形，而中文输入时这根下划线是「IME 到底收到键没有」的唯一可见反馈——暗色下不可省。
- `sync_chrome`（`main.rs:1867`）只在主题 id 变化时 `set_visuals` 一次；每帧调等于自己给自己造重排抖动。
- `Tone::of`（浮层：搜索条 / 命令面板 / 右键菜单 / 帮助）按外观分档：暗色下阴影 alpha 40 → 120、描边 `mix(border, dim, 0.25)`、发丝线抬到 `border`，否则卡片边缘直接化进黑色画布；「当前行」文字 `accent.gamma_multiply(1.35 / 0.45)` 按深浅提亮压暗；关键词命中底**语义固定暖黄**不跟主题（暗色 `#6B581C` / 字 `#FFE9A8`），因为它要和选中态竞争，跟主题走会一起消失。
- 检查器与画布编辑器里的局部覆盖（`inspector.rs:1059/1255`、`canvas.rs:3280`）保留：卡片上的输入框需要比面板更实的一档底；编辑器区段改完在 `canvas.rs:3303` 还原 `text_cursor`，不污染全局帧。

**③ 行内编辑器与画布字体一致（任务 #63）**

这是本轮唯一真正的**排版**修复。此前 `F2` 起的编辑器走 egui 默认排版，与画布那份 DirectWrite 排版是两套：字号不跟节点、行高是雅黑自身的 ≈1.17 em（画布是 `line_height = 字号 × 1.45`）、基线按 egui 的行盒走。结果多行节点一进编辑态文字**当场收紧并上下跳**。现在四条都对齐到 `mw-core` 那一份几何：

- **字号与字重**：`FontId::new(n.geom.font_size × zoom, bold ? BOLD : Proportional)`（下限 6 px）。字号来自节点几何而不是 egui 全局比例，所以任何缩放级别下都与画布同宽同高。
- **行高锁定**：`line_h = (n.geom.line_height × zoom).max(font_size)`，走自定义 `layouter` → `edit_galley`，给**每个 section** 显式 `format.line_height = Some(line_h)`；`keep_trailing_whitespace = true`（吞掉行尾空格会让 caret 看起来跳到上一个字前面）。
- **首行基线测出来，不算**：画布那行由 `line_baseline(box_h, label_h, lines, line_h, 0)` 定，`dy = 画布基线 − first_baseline(probe)`，而 `first_baseline` 取的是同一条排版路径量出的 `row.pos.y + glyph.pos.y`（空草稿回落 `line_h × BASELINE_RATIO = 0.72`）。必须实测：egui 一旦拿到显式 `line_height`，就把字形在行盒内按 CSS 方式居中并**取整到设备像素**，任何公式预测都会差 ±1 px。
- **只补「还没打出来的行」**：`first_base_offset = line_baseline(...) + (lines − rows).max(0) × line_h × 0.5`。几何要下一帧才重排，打字期间 `rows` 暂时小于 `lines`，差额按居中补；行数多出来的部分只能往下长——两边都不许让已经落好的文字跳位。
- **内边距**：`PAD_X / PAD_Y × zoom` 取整挂到 `Frame::NONE.inner_margin`（`.margin()` 与 `.frame()` 同时给会被 egui 直接丢掉），小数部分折进 `edit_rect` 的平移；`Margin` 四个字段是 `i8`，所以要 `clamp(0.0, 127.0)`。
- 强调环画到节点盒**外侧**（与合成器共用 `SELECT_RING_W` / `SELECT_RING_GAP`）：画在盒内会被根节点那种饱和填充吃掉，等于没有编辑态反馈；合成器对 editing 节点跳过自己的环。编辑期锁缩放、进入编辑前若屏幕字号 < `EDIT_MIN_PX = 13.0` 先放大画布（否则 caret 与候选窗都无从谈起）。
- 4 项单测：行距与 `line_height(14)` 差 < 1.0 px、实测基线落在行盒内（0 < base < line_h）、空草稿回落 0.72、`first_base_offset` 三种 `lines/rows` 组合只吃未打出的行。

**验证**

- `cargo test --workspace` **375 项全绿**（`mw-shell` 133 → **140**）；`cargo check --workspace --all-targets` 0 警告；`cargo fmt --all -- --check` 干净；`cargo clippy --workspace --all-targets` 去重 **37**（`mw-shell` 6 → **5**：本轮顺手清掉自己引入的 `expect`-after-`is_some` 与 `clone_on_copy` 各一条），**0 新增**。
- golden：面板配色与编辑器像素都变了 ⇒ `cargo golden --update --warmup 5` 重拍壳层基线，随后 `--tol 0` **连跑两次通过**（`shell-ppp1.50.png`）。
- 新增 `scripts/v4-edit.ps1`（release、150 % DPI）：自备两份三行中文样例（UTF-8 无 BOM 的 `content.json` + `ZipFile::CreateFromDirectory` 打成 `snow.zmind` / `inkNight.zmind`），`Home` 适应画布 → 按墨色聚类取节点包围盒 → 盒心点选 → `F2` → `End`，再用「行投影聚类 + 列质心」量编辑态三行相对画布态的位移。**实测：行距 86 物理 px（14 CSS px × 1.45 × zoom 1.5 吻合）；`snow` 竖移 ≤0.65 px、横移 ≤0.73 px；`inkNight` ≤0.51 / ≤0.64——明暗两套都做到亚 CSS 像素一致**，断言口径 `|dy| ≤ 2.0` 且打印 `EDITOR_PARITY OK`。截图 `shots/qa/v4-{snow,inkNight}-{canvas,editing}.png`，节点裁图 `v4-crop-snow-edit.png` / `v4-crop-ink-edit.png`（雅黑粗体边缘干净、snow 下 caret 可见、暗色卡片蓝色选中环）。
- release exe **20,290,560 B（20.29 MB）**。

**未做 / 遗留**

- **主题切换没有核心入口**：`command.rs` 的命令清单里没有任何外观/主题项，菜单里也没有，普通用户只能从检查器的 4 套宫格进——这是配色这条线上剩下的最大缺口，排进 v5。
- caret 高度目前仍占满 1.45 em 行盒（egui 的行为），比 XMind 那根贴着字高的光标明显偏长；改它要覆盖 `TextEdit` 的 caret 几何，属于「手感」级细节，暂缓。
- IME 人工矩阵（docs/07 §8，5 款输入法）仍未签字——本轮的下划线配色改动让「暗色下看不到组合串」这条更不可能被自动测出，见 §5 C1 / 任务 #4。
- ~~P13-C 五分钟稳态仍未复测（任务 #58），P13 不许写 PASS。~~ **2026-10-19 凌晨已复测通过：10k 两轮独立五分钟 284.4–287.9MB / 289.4–289.6MB，P13 判 PASS，见 §4.33。**

**踩坑**

1. **`TextEdit::frame(Some(..))` 会吞掉 `.margin(..)`**：内边距必须挂在 `Frame` 的 `inner_margin` 上，写在 `TextEdit` 上静默失效（不是断言、不是警告，就是没有）。
2. **egui 默认行高 ≈1.17 em，画布是 1.45 em**：不显式给 `line_height`，多行节点一进编辑态就收紧——这类错只在多行中文节点上看得见，单行样例永远测不出来。
3. **基线不能算、只能测**：显式 `line_height` 后 epaint 把字形在行内居中并取整到设备像素，公式预测恒差 1 px 上下。
4. **无头 `egui::Context::default()` 直接 `fonts_mut` 会 panic**（"No fonts available until first call to `Context::run()`"）：测试里先 `run_ui(RawInput::default(), |_| {})` 走一帧把字体装进去。
5. **`TexturesDelta` 的 `debug_assert`**（epaint `textures.rs:337`）：排版会把新字形写进图集增量，没被平台取走就 drop 直接炸。无头测试必须把每次排版的输出 `drop_without_applying_deltas()` 排干——0.37 里没有旧版的 `clear_textures_delta`。
6. **PS 5.1 的 `Add-Type` 只接受老版 C#**：方法签名里内联 `out int n` 编译不过，得先声明再 `out n`。
7. **PowerShell 函数参数别取名 `$profile`**：它是自动变量（`Object[]`），一旦拿去做减法直接抛 `op_Subtraction`，报错信息还落在完全无关的行上。
8. **墨迹行聚类要先剔脏**：卡片描边会让整块落进一个 312 行的「行」，圆角弧又造出 9–16 行的假簇。裁图先**内缩 5 px**、再按簇高度 `minH = 25` 过滤，剩下的才是真的三行——阈值来自实测（86 px 行距，假簇最高 16 px）。

---

## 4.36 v5-a 主题入口 + 浮层选中行墨水修复（2026-10-19 凌晨）

§4.35「未做」里排下来的第一条就是：**主题切换只有一个入口，而且在检查器宫格里**——要改外观先得知道检查器存在。这一轮补入口，顺带修掉补入口过程中被截图暴露出来的一个真缺陷。

**① 两个入口，一条执行路径（任务 #64）**

- 命令面板（`Ctrl+K`）加 `Action::SetTheme(&'static str)`，四条 `主题：素白 / 薄荷 / 墨夜 / 深林`，右侧 hint 写 **`浅色` / `深色`** 而不是快捷键——四行之间唯一的区别就是外观，用户搜「深色」也该命中它们（新增单测 `the_appearance_words_also_find_themes` 锁住这条）。
- 标题栏菜单（☰）末尾加主题区，`TitleMenuAction::SetTheme`，图标 `MenuIcon::Theme`（圆环 + 下半实心圆，1.7 px 强调色描边）。当前主题行画勾，其余三行只画图标。**该区只在有画布时出现**：没有文档时这四条点了也不会生效，摆出来就是骗人。
- 两条路径都收敛到 `apply_theme(id)`（`main.rs`）：发 `Command::SetTheme` → `rebuild()` → `session.theme`，成功后 `request_canvas_focus()` 把焦点还给画布；失败（无文档）弹 toast「主题切换未生效：没有可改的文档」。**入口一致性是断言出来的**，不是看出来：探针比较两条路径落到同一主题后的整帧，`palette_vs_menu_diff = 0`。
- 面板加了滚动窗口：`Palette` 多存一个 `top`，纯函数 `window_top(cursor, top, hits, cap)` **只在光标出窗时才挪**（进出各一行才动，避免每按一次方向键整列表跳半屏），`open()` / `close()` 复位。没有它，四条主题在 8 行窗口里会被顶掉，键盘用户看不见自己停在哪儿。

**② 选中行墨水：`gamma_multiply` 会连 alpha 一起乘**

`v5-palette.png` 里选中行「主题：墨夜」的字是**淡蓝**的，几乎读不出来。逐像素量下来：行底 `#EAF2FF`（`accent_soft`，正确），带内最强墨迹 `(149,188,250)`，对行底 **≈1.4:1**；同一行的 11 pt hint 能到 `(163,169,178)` = `ch.faint` 精确值，说明覆盖率是够的——不是抗锯齿糊了，是墨水本身半透明。

根因在 `Tone::of`：`hit = accent.gamma_multiply(if dark { 1.35 } else { 0.45 })`。ecolor 0.37 的 `gamma_multiply` 对 **RGBA 四个通道一起乘**，`0.45` 于是同时把 alpha 打到 115——本意是「压暗强调色」，实际是「45 % 不透明的强调色」，贴在浅蓝行底上就是浅蓝。暗色档 `1.35` 侥幸没事，只因为 `255 × 1.35` 饱和回 255，alpha 又变实了。

改成从主题令牌 **mix 到本主题字色**、永不带透明度：

```rust
hit: mix(u.accent, u.text, if dark { 0.35 } else { 0.55 }),
```

浅色压深、深色提亮， hue 还在但对比度由 `u.text` 兜底。四套主题实测（单测 `the_selected_row_ink_stays_legible` 同时断言 `a() == 255` 与比值 ≥ 4.5）：素白 **8.04:1**、薄荷 6.81:1、墨夜 **6.54:1**、深林 8.21:1；真实截图复核 8.04 / 6.54，与单测吻合。同一个 `tone.hit` 也是搜索条的选中行墨水（`search.rs:413`），一处改对两处。**教训**：凡从主题令牌派生「墨水」，`gamma_multiply` / `linear_multiply` 都是错的工具，它们是为「整块半透明底」设计的。

**③ 探针 `scripts/v5-theme.ps1`（release、150 % DPI、真实输入）**

三条主张，全部**自校准、不写死几何**：

1. **往返无损**：`Ctrl+K` → 剪贴板粘 `主题：墨夜` → `Enter` → 再回 `主题：素白`，与切换前的整帧比 `roundtrip_diff_body = 0`。口径要**剔掉状态栏**：切主题会把文档标脏，`已保存 → ● 未保存` 是合法变化，与主题渲染无关（第一版没剔，误报 4797 个差异像素）。
2. **菜单入口等价**：菜单图标列里**唯一带饱和度的墨**就是那个勾，于是按行投影聚类找它的质心（`Ticks`），再点「勾下面两行」= 墨夜；切换后重开菜单，勾必须正好下移两行（实测 774.6 → 867.7，行距 46 物理 px，**误差 1.1 px**）。同样要排除状态栏那个 warn 色的 `●`，它会被当成第二个勾。
3. **选中行可读**：`RowInk` 先按 `accent_soft` 令牌找出**连续行带**（≥55 % 列命中），再取带内离行底最远的像素算 WCAG 比值，两套外观各测一次（素白 8.04:1 / 墨夜 6.54:1，门槛 4.5:1）。这条正是为 ② 而生——回归时它会直接红。

**验证**

- `cargo test --workspace` **380 项全绿**（`mw-shell` 140 → 144 → **145**）；`cargo fmt --all -- --check` 干净；`cargo clippy --workspace --all-targets` 去重 **37**、`mw-shell` **5**，**0 新增**。
- golden `--tol 0` **连跑两次通过**，未重拍基线：主题入口都在浮层里，默认帧一个像素都没变——这也是「改配色但护栏不响」的正确形态。
- 探针 `THEME_ENTRY OK`：`snow canvas_bg = 251,251,252`、`inkNight = 23,25,31`（两套都精确等于令牌）、`roundtrip_diff_body = 0`、`palette_vs_menu_diff = 0`、`tick_move_err = 1.1 px`。截图 `shots/qa/v5-{snow,palette,palette-inknight,palette-inknight-open,snow-again,menu-snow,menu-inknight,menu-inknight-tick}.png`。
- release exe **20,292,096 B（20.29 MB）**。

**未做 / 遗留**

- 主题只有这 4 套预设，**没有自定义强调色 / 节点配色**（那是检查器宫格之外的另一件事）。
- caret 高度仍占满 1.45 em 行盒（承 §4.35），比 XMind 贴着字高的光标明显偏长。**v5-b 已把可行性查到底，结论是「不改」**：egui 0.37 的 `TextCursorStyle` 只有 `stroke / blink / on_duration / off_duration / preview`，**没有高度旋钮**；caret 矩形是 `cursor_rect()` → `galley.pos_from_cursor()` 直接取**整行行盒** `row.min_y..row.max_y` 再 `expand(1.5)`，而行盒高度就是我们在 `edit_galley` 里显式写进 `TextFormat::line_height` 的那个数——**它必须等于画布的 1.45 em，否则 §4.35 的 `EDITOR_PARITY` 立刻红**。想只缩短 caret 不动行盒，只剩自绘一条路：关掉 egui 的 caret 笔、自己按 `TextEditState` 的光标重排 galley、算出它在 `edit_rect` 内的落点、复刻闪烁。风险恰好落在最贵的两样东西上——**caret 与文字错位**（`TextEdit` 内部还有垂直对齐与截断偏移，不在我们的 `dy` 模型里）和 **IME 候选窗定位**（`set_ime_cursor_area` 吃的就是这个矩形，C1 验收项）。一根偏长但永远是对的光标，胜于一根短了 6 px 却可能飘的光标。
- IME 人工矩阵（docs/07 §8）未签字。**P13 已于 2026-10-19 凌晨判 PASS**（10k 两轮独立五分钟 284.4–287.9MB / 289.4–289.6MB，见 §4.33），任务 #58 关闭。

**踩坑**

1. **`Color32::gamma_multiply` 乘的是 RGBA 四个通道**：拿它「压暗」一支不透明墨水，得到的是半透明墨水。要压暗就 mix 到字色，别乘因子。
2. **暗色档的因子 > 1 会掩盖同一个 bug**：`255 × 1.35` 饱和回 255，alpha 又是实的，于是只有浅色档出问题——两套外观都得测，测一套等于没测。
3. **切主题会 dirty 文档**：任何「整帧往返必须一致」的断言都得先划掉状态栏与标题栏的保存态，否则合法变化会被当成回归。
4. **PS 5.1 脚本里别出现非 ASCII 路径字面量**：无 BOM 文件按 ANSI 读，`FromFile` 拿到乱码路径后返回 null，报错落在完全无关的一行上。用 `(Get-Location).Path` 拼相对路径。

---

## 4.37 v5-b 编辑期指针归属：拖选文字不再搬走整张图（2026-10-19 凌晨）

caret 那件事查到底、决定不改之后，v5-b 给自己排的第一条是「交互体验」里最便宜也最容易露馅的一处：**在节点里划选一个字**。读代码看不出问题——`classify()` 里明写着「编辑期 → `DragKind::Pan`」（§4.20 note 5），像是故意的设计；只有把手真的放到文字上拖一下，才知道这条 `Pan` 把「选中这三个字」一起吃掉了。所以先写探针 [scripts/v5-sel.ps1](../scripts/v5-sel.ps1)，用真实输入判，再决定改不改。

**探针的两个主张，各自跑一个进程**（一条失败不能污染另一条的参照）：

1. **编辑态里点某一行，光标就落在那儿**。视觉断言在这件事上不可信，改成闭环：点第 1 行 45 % 处 → `Ctrl+V` 粘一个 `X` → `Ctrl+S` → 解包 `content.json` 看 `X` 落在第几行。「光标跑到末尾」和「这一下其实把编辑提交了」是两种不同的失败，报告里也分开写。
2. **编辑态里从第 1 行拖到第 3 行，选中文字、不平移画布**。平移的证据用**卡片墨盒**：编辑态先量一次 `InkBox`，拖完再量一次，`minX/minY` 不许动。选中的证据用**像素差对零输入对照**：同一时长什么都不按先拍一张，得到 caret 闪烁的地板（实测 263 px），拖完那张必须明显高出它。

**修复前的实测**：主张 1 **通过**（`X` 落进第 1 行）。这一条顺带证伪了另一个嫌疑——面板级的 `response.clicked_by(Primary)` 提交路径并没有把编辑器里的一次点击当成「点空白提交」，egui 的 interest 归位已经挡住了。主张 2 **失败**：卡片墨盒 `854,407 → 948,579`，**平移 94×172 px**，正好等于拖拽位移本身；变化像素 16479 是整张图在跑，不是选中。**根因**：`handle_pointer` 读的是原始 `press_origin`，谁拿到 interest 它一概不看，于是编辑期任何落在画布内的按下都会 arm 一个手势，`classify` 一律给 `Pan`，越过 `PAN_DEADZONE` 就把相机推着走。

**改法（三处，最小口径）**：

- `editor_ui` 把这一帧**真实占用**的盒子随 `EditorAction.rect` 回传（取 `response.rect`，不是 `node_rect`——只有被 `TextEdit` 占住的那块才该挡住画布，卡片里没被占的空白照旧走「提交 / 平移」）。
- `Canvas` 多一个 `edit_rect` 字段：编辑器在就每帧刷新，编辑器一没就置 `None`。它给下一帧的 `handle_pointer` 用，画布与编辑器共用同一份相机，一帧的滞后不会让盒子飘。
- 按下那一帧用纯函数 `editor_owns_press(edit_rect, primary, middle, space, at)` 过滤：**只有「左键 + 没按住空格 + 点在盒内」让给编辑器**。中键与空格+左键仍然平移（编辑长文字时把视野挪开是刚需），盒外仍是老语义。手势规则改错了立刻毁手感，所以它和 `classify` 一样抽成脱离窗口、脱离 GPU 的纯函数，单测直接钉。

**验证**

- `cargo test --workspace` **381 项全绿**（`mw-shell` 145 → **146**，新增 `a_left_drag_inside_the_editor_belongs_to_the_editor`）；`cargo fmt --all -- --check` 干净；`cargo clippy --workspace --all-targets` 去重 **37**、`canvas.rs` **0 条**，**0 新增**。
- golden `--tol 0` **连跑两次通过，未重拍基线**：指针归属不改一个像素，护栏不该响。
- 同一份 release 上重跑两条探针：`v5-sel.ps1` → **`EDIT_POINTER OK`**（主张 1 仍 `X` 进第 1 行；主张 2 `shift = 0x0px`、变化像素 **30358** 对地板 **263**——拖选真的发生了）；`v4-edit.ps1` → **`EDITOR_PARITY OK`**（snow 竖 ≤0.65 / 横 ≤0.73 px、inkNight ≤0.51 / ≤0.64 px，与修复前同一量级，编辑态排版没被动）。
- release exe **20,292,608 B（20.29 MB）**，2026-10-19 02:51 构建。

**踩坑**

1. **「画布在编辑器底下，按下自然归编辑器」是错的**：egui 的 interest 只管 `Response` 的语义，不管我们自己的手势状态机。画布读原始输入，就必须自己判断这块地方归谁。
2. **只断言「卡片没动」会漏掉另一半**：没平移 ≠ 选中了。必须有零输入对照，否则一个像素都没选、只有 caret 在闪，也会被「画面有变化」这种粗断言放过去。
3. **两条主张必须分进程**：主张 1 的粘贴会改文字，主张 2 的墨盒参照就脏了；反过来，主张 1 若真的失败（编辑被提交），主张 2 量的已经不是编辑态，`shift` 会变成「搬节点」而不是「平移相机」，报错会把人引到完全错误的方向。

**未做 / 下一步**：编辑期**出盒拖拽**（按住左键划到卡片外继续扩展选择）目前交给 egui 自己处理，没量过会不会重新落到画布手里；双击/三击选词与选段还没做；v5-c 排**自定义强调色 / 节点配色**。IME 人工矩阵（docs/07 §8）仍是唯一人工签字项。

---

## 4.38 v5-c 自定义强调色：只借色相的一整套配色（2026-10-19 凌晨）

§4.36 排下来的第三条：**四套预设主题之外，用户改不了颜色**。这一轮做的是「自定义强调色」——不是给节点刷色板（那是另一件事，且要先解决逐节点配色进撤销栈的粒度问题），而是让用户挑一支颜色，整套配色围着它转。

**① 语义先定死：覆盖只借色相**

`DocMeta.theme_accent: Option<String>`（`#rrggbb`）存的是**用户挑的那支颜色**，生效方式是把它自己的强调色沿 OKLCH 色相环**刚体旋转**过去（`Theme::with_accent` → `accented_to`）：`accent`、`accent_soft`、`palette`、`palette_fill` 全部转同一个角度，**彩度与感知明度逐令牌保住**。三条理由，都写在 `theme.rs:296` 的文档注释里：

- 挑到 `#ffff00` 也不会得到一堆读不清的黄卡片——卡片文字对比度**基本没动**，因为决定对比度的 `L` 和彩度还是主题自己的；
- 切主题时同一份覆盖自动落到新主题的明度上，不必为四套主题各存一支颜色；
- 无彩色（`chroma < CHROMA_FLOOR = 0.012`）没有色相可借，按「未设置」处理——8 bit 量化在近灰上能把色相抖出十几度，谈不上方向。

**为什么不选 HSL**：HSL 的 `L` 不是感知量，`#ffff00` 的 HSL 明度 0.5、相对亮度 0.757。按 HSL 转色相，实测一级卡片文字对比度从 **9.57 漂到 6.46**——「换个强调色」顺手把可读性换掉了。OKLab 的 `L` 是感知明度，转色相时相对亮度基本钉住（本轮实测 7.89 → **8.44**，方向还是变好的）。

**② 出界怎么办：沿半径回收彩度，不钳通道**

素白强调色彩度 0.198，转到 60° 落在 sRGB 之外。逐通道 `clamp` 会把色相偷走 14°（**实测落到 46.2°**，用户明明点了 60°）；改成在 `from_oklch` 里**沿半径二分回收彩度**，落点 **60.2°**，代价只有彩度 0.198 → 0.142。二分端点必可行：起点 0 是纯灰，永远在色域内（`theme.rs:110`）。单测 `an_out_of_gamut_hue_still_lands_on_the_angle_you_asked_for` 同时钉住三件事：色相误差 < 2°、**明度不许顺手被改掉**（< 0.01）、彩度必须往回收而不是硬钳。

**踩到的真 bug（正逆变换单测抓的）**：`linear_to_srgb` 返回 0–1，写回 u8 时漏乘 255，于是**每个通道都被四舍五入成 0 或 1**——浅绿卡片变近黑。这个错误在「只看正向输出」的测试里完全看不出来，因为它对任何输入都自洽地错。回归 `oklch_round_trips_the_colors_we_actually_ship` 因此只测**会真的被转的色**（主题自己的中低彩度令牌 + 六个边界 hex，一圈回来每通道误差 ≤ 1）；纯 `#0000ff` 这种色域顶点**故意不测**——它坐在 sRGB 立方体的角上，正逆矩阵常数差一点就被判越界，彩度回收 0.01 就能让绿通道从 0 涨到 48，而用户那支颜色**从来只贡献一个色相数字**，不进 `from_oklch`。

**③ 数据、命令与容器**

- `Command::SetThemeAccent { accent: Option<String> }`：`None` = 清除覆盖回主题预设。命令名「更换强调色」（`the_accent_command_is_named_for_the_toast` 钉住，toast 与撤销栈共用同一份文案）。
- **非法值先拒后存**：非 `#rrggbb` 直接返回带 `#rrggbb` 字样的错误，且 `assert!(!d.can_undo())`——**被拒绝的命令不能留撤销足迹**（`a_non_hex_accent_is_rejected_instead_of_stored`）。大小写原样保留（`#F5A623` 存进去就是 `#F5A623`），因为它是用户挑的颜色，不是我们算出来的。
- **切主题不清覆盖**：`SetTheme` 之后 `theme_accent` 还在，`SetThemeAccent{None}` 才恢复预设（`switching_theme_keeps_the_accent_and_clearing_it_restores_the_preset`）——用户挑的色相是他的决定，换一套明度体系不该把它冲掉。
- 容器：`content.json` 里 `themeAccent` **有则写、无则整个键不出现**（`mw-format` 双向单测），旧文件缺键 = `None` = 预设，不报错。
- 撤销/重做单测 `accent_override_applies_undoes_and_redoes` 走完整一圈：设置 → 撤销（还回 `None`）→ 重做 → 清除 → 撤销（又回到设置值）。

**④ 检查器 UI：12 格色条 + 取色器 + 主题默认**

`ACCENT_HUES` 每 30° 一格绕一圈——取整圈而不是几条「品牌色」，因为覆盖只借色相，用户心里想的是「我要蓝一点」而不是一个具体 hex。**画的是这套主题转到那个色相之后会长成什么样**（`accent_at_hue`），不是用户挑的原始颜色：色条和画布必须是同一个颜色，否则「预览」是假的。

- 22 px 格子、7.5 px 实心圆盘 + 1 px 中性描边；当前生效的那一格画 2 px 强调色外环（半径 10 vs 8.6），**选中与焦点分两层**：焦点环画在半径 11、用中性灰 `ch.dim`，于是「哪格生效」和「光标在哪」能同时看见而不互相冒充。egui 只给内置控件画焦点环，自绘圆盘不在其列——这一圈是自己画的。
- 自定义取色器 `color_edit_button_srgba` 旁边写明「自定义：只取这支颜色的色相」。
- **拖动期间不发命令**：一次拖动 `changed` 几十次，撤销栈会被同一件事填满。本地存 `accent_draft` + `accent_dirty`，`pointer.any_down()` 一停才发一条 `SetThemeAccent`（单测 `a_drag_in_progress_keeps_its_accent_draft`）。
- 「主题默认」按钮只在有覆盖时可点，点下去发 `SetThemeAccent{None}`。
- 壳层跟着换：`sync_chrome` 的判据是**「主题 id + 生效强调色」这一对**（`main.rs:1965`）。只比 id 不行——`SetThemeAccent` 不换主题 id 却要换一整套 accent，面板描边、选中行、输入法下划线全从 `Chrome` 派生；而 `install_visuals` 会让所有控件重新布局，每帧调一次等于自己造重排抖动。

**⑤ 键盘可达（任务 #72）**

色板是 12 个自绘圆盘，鼠标能点，键盘用户必须也能到。`Sense::click()` 自带 `FOCUSABLE`，所以它们本来就在 Tab 环里——**看不见**才是问题：焦点环（④ 里那一圈中性灰）用 `resp.has_focus()`，而 egui 的 `Response::has_focus()` = `ctx.input(|i| i.focused) && memory.has_focus(id)`，**要求窗口是 OS 前台**。验收机器的输入锁（见 §4.39）让这一半在探针里永远不亮，看起来像「Tab 走不动」。于是把「焦点在哪」和「焦点画成什么样」拆开：轨迹登记走 `trace::name`（只看 `memory`，与前台无关），焦点环仍看 `has_focus`（真实用户永远在前台，两件事在用户那儿是同一件）。

**⑥ 探针 `scripts/v5-accent.ps1`（release、150 % DPI、真实输入）**

八条主张，全部从应用真的画出来的像素里量：F6 交接只重画状态条不动文档 / 色板生效 / 对比度不掉崖 / 相机不动 / 撤销重做各退进一步 / 「主题默认」落回原帧 / `themeAccent` 双向进容器 / 面板开合不让画布失声。实测：`card 240,155,148 → 166,193,119`（色相真的转了 120°）、`ink 7.89:1 → 8.44:1`、`ink box 243,435,1052,692` 逐字节不变、`undo vs base = 0 px`、`reset vs base = 0 px`、恢复点两次读包一对一无。**最终判定 `ACCENT OK`（8/8，0 failed，0 blocked）**。

**验证**

- `cargo test --workspace` **405 项全绿**（`mw-core` **136**、`mw-shell` **156**）；`cargo fmt --all --check` 干净；`cargo golden --tol 0` **连跑两次通过、未重拍基线**（默认帧没有强调色覆盖，一个像素不该动）。
- release exe **20,302,848 B（20.30 MB）**。

**未做 / 遗留**

- 只有全局强调色，**没有逐节点配色**；`levels` 色阶不随覆盖变（分支色表转了，层级明度表没转）。
- 色条 12 格固定，用户不能存常用色；覆盖不写进窗口状态（它本来就是文档字段，跟着文档走）。
- 面板控件仍用 egui 自动 id：面板开合会让「正停在某一格上」的焦点丢一次（画布已经钉死，见 §4.39）。要修得给整个面板换持久 id，收益小、风险是打乱 Tab 顺序。

---

## 4.39 v5-d 键盘交接 F6：一次输入锁逼出来的三个修复（2026-10-19 凌晨）

v5-c 的键盘可达要把「Tab 走到第几格」变成可断言的事实，于是这一轮真正的产出不是功能，是**一套看得见焦点的验收工具**，以及它顺手抓出来的两个交互缺陷。

**① 焦点轨迹 `MW_TRACE`（`crates/mw-shell/src/trace.rs`）**

为什么不让探针看像素：焦点环画得再准，「这一圈环属于第几格」仍要靠坐标反推，缩放或面板宽度一变就得重算。焦点是壳层在绘制那一刻就知道的事，写一行文本比猜像素便宜得多，也让无头测试能直接断言 Tab 的落点。

- `trace::name(ctx, id, label)`：控件把自己的**语义名**登记在自己的 `Id` 上（`data_mut.insert_temp`），绘制路径上调，零判断。
- `trace::observe(ctx)`：`fn ui()` **最后一行**统一读 `memory.focused()` 并落盘 `focus <label>`。放最后是因为语义名由各控件在本帧绘制时登记，而焦点可能又被这一帧后面的 `request_focus` 改掉——只有这里读到的才是本帧的结论。
- `trace::event(<label>)`：状态转换（`pane-panel` / `pane-canvas`）与焦点状态分开记，否则「按了 F6」和「焦点在画布」会被混成同一种证据。
- 不设 `MW_TRACE` 时整个进程留一个 `None`，`focus` 退化成一个原子读加一次空锁；发布版默认零输出。这不是用户功能，是验收工具，和 `MW_STATS` 同一条纪律。

**② 真缺陷：面板一开合，画布键盘全体失声（任务 #73）**

探针 F8 之后 Tab 不再加子主题，且**没有任何报错**。根因链在 egui 的分配身份里：

1. `CentralPanel` 的 `Ui` 身份由「这一帧注册了哪几个面板」决定——开大纲（F8）、开检查器（F9）、多一个标签页，上层 `Ui` 的 auto-id 盐整体漂移；
2. 画布区域原来用 `allocate_exact_size`，id 是自动分配的，跟着漂；
3. egui 焦点有个死人开关：`Focus::end_pass` 发现本帧 `used_ids` 里查不到握着焦点的那个 id，就**清空焦点**；
4. 画布的裸键快捷键只认 `has_focus` → Tab 加子、方向键选节点**全体失灵**，直到用户重新点一下画布。

修法是把身份钉死（`canvas.rs:377` 附近）：`canvas_id()` 返回一个固定 `Id`，`canvas_area()` 用 `allocate_space` + `ui.interact(rect, canvas_id(), …)` 手工走完 `allocate_exact_size` 内部同样的路径（同样消耗一个 auto-id 盐，像素与布局等价）。回归 `a_panel_toggle_keeps_the_canvas_keyboard_focus` 直接断言两帧的 id 相等且焦点还在——**修复前它是红的**（`Id::unique(15707054694645992836) != Id::unique(11952710319151270945)`）。

**③ F6 只认自己的模式位，不认 egui 的记忆**

`keys_in_panels()` 就是 `!self.wants_keys`，**只看我们自己的位**，不看实时焦点。理由：焦点会被点面板、`Esc`、面板控件抢走，按实时焦点判断会让同一个键走进不同分支，而状态栏那行「键盘：面板 · F6 收回」会跟着说谎——指示和用户按下去发生的事必须同源。交接动作直接作用在钉死的 id 上（`surrender_focus(canvas_id())` / `request_focus(canvas_id())`），所以 `holds_keys()` 这种「从记忆层反推状态」的方法被整个删掉了。

**④ 真缺陷：撤销快捷键跟着画布焦点一起死（任务 #74）**

探针这一轮拿到了一次「chords 真的能送到」的环境（输入锁自己松了），于是 **`Ctrl+Z` 在焦点不在画布时不生效**这条从推断变成了实测。根因：撤销/重做原来写在 `canvas.rs` 的 `shortcuts()` 里，而那段整块被画布 `has_focus` 挡着——**鼠标点一下工具条的「撤销」，焦点就交给那个按钮，键盘撤销静默失灵**；F6 交接给面板之后同理。

改法是换个唯一主人：`canvas.rs` 删掉那 12 行，`main.rs` 的窗口级链（原 `file_shortcuts`，改名 `window_shortcuts`）用 `consume_key` 接管，落到 `step_history(redo)`。三个细节：

- **`consume_key` 会把事件从 `Input::events` 里摘掉**，所以窗口级先跑（`main.rs:3657` 在画布绘制之前）就不会和画布的双击撤销；
- **带 Shift 的先查**：`consume_key(COMMAND, Z)` 的修饰键匹配会放过多余的 Shift，`Ctrl+Shift+Z` 会先被 `Ctrl+Z` 吃掉——顺序即语义；
- `step_history` 只挡一件事：`canvas.is_editing()` 时不接，那一下 `Ctrl+Z` 归文本框自己（egui 的 `TextEventFilter` 本来就会在有 `TextEdit` 焦点时把这些键摘走，这层是显式表达意图，不是兜底）。

工具条的 6 个按钮也登记了语义名（`tool-undo` / `tool-redo` / …），于是探针能 Tab 到它们并按 Space——**同一支命令、同一个 widget，只是不用修饰键**。这条路径是本轮撤销/重做主张的真正执行者：`undo vs base = 0 px`、`redo vs spun = 0 px`。

**⑤ 探针的相机必须先确认「看得见」**

输入锁一松，别的东西也会跑到窗口上面来：一次 Windows 设置窗落在应用正中，于是「撤销没回到原帧」（35,852 px）、「清覆盖没回到原帧」（1,190,288 px）、「应用色板把键盘抢回去了」（条带墨迹 6365 → 5941）三条**全是假的 FAIL**——量的都是那个对话框。

现在 `Shot` 抓帧之前先 `SetWindowPos(HWND_TOP | NOACTIVATE)` 把自己抬回来（后台进程抢不到前台，但抬 Z 序不需要前台权限），再用 `WindowFromPoint` 按 90 px 网格采样 216 个点问「这儿真的是我的窗口吗」；**无标题的根窗口跳过**（IME 候选窗、tooltip、光标辅助窗这些自家助手窗，一格压一像素不算瞎）。仍然被盖住 → 记一笔、退出码 **3 = INCONCLUSIVE**，不再把桌面的动作算成产品的账。

**⑥ 顺手清掉的 32 条 clippy 警告**

`excessive precision` 满屏——OKLab 的参考矩阵是 f64 常数，塞进 f32 只是把常数截短（而截短带来的色相抖动会顺着「换强调色」传到每张卡片上）。两段变换整体改在 f64 里算、进出口各转一次：`mw-core` lib **43 → 11 条**，即 v5-c 期间新增的 32 条清零，明眼数字回到 §4.36 记的基线口径。**像素没动**：探针复跑同一批数值（`card 166,193,119`、`ink 8.44:1`）与改 f64 之前逐字相同。

**验证**

- `cargo test --workspace` **405 项全绿**（`mw-shell` 146 → **156**，含 `a_panel_toggle_keeps_the_canvas_keyboard_focus`、`handing_over_the_keyboard_moves_tab_into_the_panels`、`tab_walks_onto_the_accent_chips_and_space_applies_one`、`a_focus_is_written_once_per_change`）；
- `cargo golden --tol 0` **连跑两次通过、未重拍基线**；`cargo test -p mw-render --test offscreen`（画布几何）与 `--test text_render`（文字）两份基线同样全绿；
- `cargo clippy --workspace`：`mw-core` **11** / `mw-render` **7** / `mw-io` **1** / `mw-probe` **2** / `mw-shell` **5**，本轮**新增 0**（v5-c 带进来的 32 条已清）；`cargo fmt --all --check` 干净；
- `scripts/v5-accent.ps1` → **`ACCENT OK`**：八条主张全过，其中两条曾经 blocked——`after F8 the keyboard sits on: 'focus canvas'`、`Tab after the toggle repainted the document over 40338 px`。

**踩坑**

1. **egui 的自动 id 不是实现细节，是行为**：只要上层 `Ui` 的构成变了，下面所有自动 id 一起漂，而死人开关会把焦点一起带走。任何「这一帧必须一直握着键盘」的 widget 都得钉 id。
2. **`Response::has_focus()` ≠ `memory.has_focus(id)`**：前者还要求窗口是 OS 前台。用前者做「键盘归谁」的判断，等于把应用行为绑在桌面此刻有没有别人抢焦点上；用后者做验收证据，又会在无头环境里什么都看不见。所以判定与证据分家（④⑤ 两处都是这个分法）。
3. **像素差不能当焦点证据**：状态栏那行指示整条是强调色画的，改一次配色它就重画一遍。判「键盘有没有被抢走」要看条带**墨迹覆盖**，不是像素差。
4. **恢复点路径要按文档目录算**：`autosave::slot_for` 落在 `<文档目录>/.autosave/<stem>.autosave.zmind`，探针一开始拿 ZIP 解包目录去比，永远「没落盘」。
5. **相机被盖住时，所有像素主张一起变红**：一次外部窗口落进来能伪造四条产品缺陷。先证明「我看得见我要量的东西」，再谈结果。

**未做 / 下一步**：`Ctrl+C/X/V` 与 `Ctrl+Shift+C/V` 仍在画布焦点门后（同 ④ 的病，只是工具条上没有对应按钮，探针没抓到）——**下一轮 §4.40 已修并实测**；面板控件的自动 id 未钉（§4.38 遗留）；快捷键配置（M2c 最后一项）仍未做；IME 人工矩阵（docs/07 §8）仍是唯一人工签字项。

---

## 4.40 v5-e 剪贴板和弦升到窗口级：一次「点了工具条就粘不上」的补齐（2026-10-19 中午）

§4.39 结尾记下的是**已知同病、只是没抓到证据**：五个剪贴板和弦还写在 `canvas.rs` 的 `shortcuts()` 里，整块被画布 `has_focus` 挡着。这一轮把它搬到窗口级，顺便回答一个更难的问题——**验收工具怎么在后台进程里送出带修饰键的按键**。上一轮「chords 送到不了」是猜，这一轮量出来了。

**① 判定与摘事件分家（`canvas.rs:422-462`）**

`ClipboardChord` 枚举 + `clipboard_chord(style_copy, style_paste, copy, cut, paste) -> Option<_>`：一个纯函数只做「哪条该执行」，不碰 `Input`。拆开的原因写在函数注释里——**「一条都不该执行」的时候必须一个事件都没被摘走**。`Event::Copy/Cut/Paste` 是 `events` 里的普通条目，egui 0.37 没有替 `TextEdit` 预过滤剪贴板键的全局机制，谁先 `retain` 谁赢，而面板的 `TextEdit` 要到本帧**后面**才画：窗口级先摘，等于把文本框的 Ctrl+C 抢走。

顺序即语义，三条各有理由：
- **带 Shift 的先判**：`consume_key(COMMAND, C)` 的修饰匹配会放过多余的 Shift，`Ctrl+Shift+C` 会被 `Ctrl+C` 吃掉；
- **`Cut` 排在 `Copy` 前**：剪切本身就是「复制 + 删除」，两条同帧到过必须落到带删除的那条——这是画布时代 `if copy_ev || cut_ev` 的语义，一条不能丢；
- **`style_*` 命中就不再摘普通事件**：`Event::Paste` 一旦 retain 掉就再也还不回文本框。

三条新单测把顺序钉住（`a_style_chord_beats_a_plain_copy_in_the_same_frame`、`cut_wins_over_copy_because_cut_is_copy_plus_delete`、`no_clipboard_chord_consumes_nothing`）。

**② 门必须打在 `consume_key` 之前（`main.rs:1479-1487`）**

`window_clipboard` 的守卫是三个条件的析取：`canvas.input_blocked() || canvas.is_editing() || ctx.text_edit_focused()`，位置在**任何一次输入读取之前**。放在之后就是错的——事件已经被摘走，本帧后面才画的 `TextEdit` 什么也收不到，那一下 Ctrl+C 两头落空。撤销/重做同一个分法：`window_shortcuts` 里把判断提成 `history_is_ours`（`main.rs:1379`），后面两个分支共用一个结论，不出现「撤销让位、重做不让」的裂缝。

前两个门是自家状态（命令面板/搜索条的遮挡态、行内编辑器覆盖层），第三个是 egui 的记忆：搜索条、命令面板、检查器的标题与备注都是 `TextEdit`，`ctx.text_edit_focused()` 一次覆盖全部，不必每加一个输入框就补一个位。

**③ 「键盘在输入框里」要由应用自己说出来**

`panels/search.rs:215-218` 给搜索框登记语义名 `search-input`。理由不是方便调试：主张②的证据是「焦点在输入框时 Ctrl+Z 不动文档」，如果焦点那一行读出来是 `none` 或一串 `id <hex>`，这条主张只能靠「文档没动」**倒推**「键盘大概还在框里」——而「什么都没发生」和「按到了但让位错了」像素上长得一样。有了 `after Ctrl+F the keyboard sits on: 'search-input'`，前提和结论各有一条独立证据。

**④ 硬件按键路由：修饰键是发不进去的，只能按进去**

探针第一轮给的判词是 `preflight: sent Ctrl+K repainted 0 px`——**SendMessage 送不达**。原因已定位：winit 的修饰键取自异步键盘状态（`GetKeyboardState`），`WM_KEYDOWN` 不更新那份状态，于是裸键（F6/Escape/Tab）能送到，**任何带 Ctrl 的组合都送不到**。这不是应用缺陷，是相机瞎。

改法分三层：
- `ChordSent`（旧路径，留着兜底）/ `ChordHw`（`keybd_event` 真按下去：Ctrl 按下 → K 按下 → K 抬起 → Ctrl 抬起）；
- `Ensure-Fg()`：硬件按键打进的是**前台窗口**，所以先用 `TakeForeground`（`AttachThreadInput` 到当前前台线程共享输入队列 → `BringWindowToTop` + `SetForegroundWindow` →  detach；失败再退到挂应用自己的 UI 线程重试）。抢不到前台就不硬按，改走 sent 路径并记一笔 `hwMiss`；
- **开场路由选择 + 结尾复核**：先 sent 试一次 Ctrl+K（0 px → 送不达），再硬件试一次（368,086 px → 按得到），据此定 `$hw`；**整轮跑完再复核一次**同一支和弦（`368,055 px`，`0 chord(s) fell back to the sent route`）。复核不通过 → 判词降级成 `CLIP INCONCLUSIVE`、**退出码 3**：中途别人把前台抢走，这轮的键盘主张全部作废，不能拿桌面的动作出产品的结论。

**⑤ 证据选型：`nodeCount` 差分，不是像素计数**

像素只能证明「画布重画了」，不能证明「正好加了 4 个节点」，也看不出**一次按键执行了两遍**。这轮的主要证据换成文档恢复点里 `manifest.json` 的 `nodeCount`（由 `mw_format::save` 写，自动保存走同一支函数，所以不需要 Ctrl+S），而**期望增量取自应用自己的剪贴板文本**：`clipboard_text()` 每个非空节点标题一行，于是「复制了 4 行 ⇒ 粘贴应当 +4 个节点」是应用报出来的数，跟几何、缩放、面板宽度全部无关。差分不为 0 或为 2 倍都会当场露出来。像素带差降为旁证（`document band repainted 9259 px`）。

**验证（`scripts/v5-clip.ps1` → `CLIP OK`，六条主张全过，blocked 0、fail 0）**

1. **焦点离开画布之后剪贴板还能用**：点节点选中整棵（`selected subtree carries 4 node(s)`）→ F6 交接面板（`focus sits on: 'none'`，前提由应用报出）→ `Ctrl+C` 复制回同样 4 行，且**纯文本确实出了应用**（系统剪贴板从哨兵值变成节点文本）。
2. **粘贴落在文档上**：`nodeCount 4 → 8`，条带重画 9,259 px。
3. **一支和弦只执行一次**：`Ctrl+X` 让 `nodeCount 8 → 4`（恰好减掉剪贴板里的 4），随后 `Ctrl+V` 回到 8，来回同值——多执行一次就是 4→0 或 8→12。
4. **样式和弦不碰结构**：`Ctrl+Shift+C` + `Ctrl+Shift+V` 之后 `nodeCount` 仍是 8，一条新恢复点都没刷出来。
5. **让位给文本框，且不是空转**：`Ctrl+F` 后键盘在 `search-input`，`Ctrl+Z` 时文档条带 **0 px**（逐字节不变）；同一支键在 Escape 之后重画 **9,259 px**。两半必须成对读——只报前半，「门把键吃掉了」也能长得一样。
6. **零输入对照**：不按任何键连拍两帧，整幅 body diff **0 px**。这轮没有呼吸动画在跑，所以非 0 就只可能是别的东西落进了镜头。

- 回归 `scripts/v5-accent.ps1` → **`ACCENT OK`**：撤销链没被改坏，`undo vs base = 0`、`redo vs spun = 0`（仍走 toolbar Tab + Space 路由，因为该探针没接硬件按键路由）；`recovery point drops themeAccent` 也仍然成立。
- `cargo test --workspace` **408 项全绿**（`mw-shell` 156 → **159**）；`cargo golden --tol 0` 连跑两次通过、未重拍基线；`offscreen` / `text_render` 两份画布基线全绿；
- `cargo clippy --workspace --all-targets`：`mw-core` **11** / `mw-render` **7** / `mw-io` **1** / `mw-probe` **2** / `mw-shell` **5**，与 §4.39 逐项相同，本轮**新增 0**；`cargo fmt --all --check` 干净。

**踩坑**

1. **`SendMessage` 送不达修饰键**：裸键能进去是因为 winit 读 `WM_KEYDOWN`；组合键读的是异步键盘状态，而帖消息不改那份状态。后台探针想知道「和弦能不能用」，唯一诚实的办法是**开场分别试两条路由**，而不是假设其中一条行。
2. **硬件按键必须问「前台是不是我的」**：`keybd_event` 打进前台窗口，抢不到前台时它就是把 Ctrl+K 送给了别人的应用。所以每次和弦之前重做一次 `Ensure-Fg`，抢不到就记账（`hwMiss`）而不是静默换路由——否则一轮里一半主张走的是不同的物理路径，结论没法合成一句。
3. **「剪贴板可读」要先证明可写**：`Get-Clipboard` 在 STA 之外的进程里直接抛异常。探针开场做一次哨兵写回读（写 `MWCLIP-SENTINEL` → 读回来必须相等），读不回来就记 blocked，不让它冒充成产品的 FAIL。
4. **点节点的位置不能猜**：墨迹包围框的左右缘各点一次，用「复制出来的行数」判断有没有选中。第一个候选 `345,528` 返回 `-1`（没选中任何节点），第二个 `1133,528` 返回 4 行——证据自己会挑点。
5. **别在文本框上做像素差**：搜索框里有闪烁光标，任何 diff 都是噪声。主张⑤只问**文档条带**动没动，输入框自己的状态交给 `MW_TRACE` 的焦点行。

**未做 / 下一步**：`v5-accent.ps1` 还没接硬件按键路由（它的撤销主张目前只由 toolbar 路由证明）；面板控件的自动 id 仍未钉（§4.38 遗留）；快捷键配置（M2c 最后一项）未做；松开鼠标落在空白处「分离为自由主题」仍缺一条 `MoveNode` 之外的命令（`canvas.rs:commit_move` 的 `None` 分支）；IME 人工矩阵（docs/07 §8）仍是唯一人工签字项。

---

## 4.41 v6 字体样式：用 XMind 的实拍把字阶与字重钉死（2026-10-19 下午）

三块打磨里的「字体样式」此前只有 v3 的阶梯（26/17/15/14、一级加粗）和 v4 的编辑器同源，**没有一项目标值是量过 XMind 的**。这一轮不动排版链路，只改 `LevelStyle` 的字号与字重两个令牌，并把依据落到像素上。

**① 先量，再改：`PrintWindow` + 墨迹包围盒**

参考机 150%（XMind 窗口 `GetDpiForWindow = 144`，CSS px = 物理 px ÷ 1.5）。用 `PrintWindow(PW_RENDERFULLCONTENT)` **只读**抓用户已打开的一张 `rust` 测试图，没有点击、没有改 XMind 的状态；实拍存 `shots/reference/xmind2026-type-ladder.png`。然后只在节点盒内部按亮度 < 110 扫暗像素，取包围盒与水平扫描线上的笔画宽度：

| 量 | 物理 px | CSS px | 推出的值 |
| --- | --- | --- | --- |
| 根 `rust` 的 x-height（去掉 `t` 升部，只量 `rus`） | 25 | 16.7 | 拉丁 x-height ≈ 0.52 em → 字号 ≈ **32** |
| 一级「分支主题 4」汉字墨高 | 25 | 16.7 | 雅黑 CJK 墨高 ≈ 0.87 em → 字号 ≈ **18.7** |
| 根笔画宽（多条水平扫描线） | 7–9 | 4.7–6.0 | Bold |
| 一级笔画宽 | 2–4 | 1.3–2.7 | Regular |

两条结论直接推翻旧值：**根:一级 ≈ 1.7（旧 26:17 = 1.53）**，且 **XMind 只有中心主题加粗**（旧实现一级也加粗）。25 物理 px 的汉字墨高与 14 pt（96 DPI 下 18.67 CSS px）对得上；32 CSS px 对应 24 pt。这也解释了 v5 截图中「一级比中心只小一圈、整张图偏重」的观感。

**② 一个常量管四套主题**

`mw-core::theme::LEVEL_TYPE` 成为唯一字阶（DIP）：**32 粗体 / 18 常规 / 16 / 14**。四套主题的 `levels` 全部引用它，`Theme::level` 的空表兜底也取第 4 档。新增单测 `every_theme_shares_one_type_ladder` 同时钉两件事：换主题不许顺带换字号；只有中心主题是粗体。

为什么不是「保留 17 加粗、只把根抬到 32」：参考图里一级分支是常规体，保留加粗会变成「大根 + 一排粗分支」，比 XMind 重一档；减重后 32:18 的 1.78 才真正撑起层级。L2 15→16 补的是 18 与 14 之间的空档，L3 保持 14（最小可读档）。行高 `text::line_height = 字号 × 1.45` 不动：参考图一级盒高约 44.7 CSS、字号 18.7，扣掉上下内边距后正落在 1.4–1.45 区间；我们的 18→26 在同一档。

代码面没有第二处：画布、行内编辑器、PNG/SVG/PDF 导出都从 `geom.font_size/bold` 走；检查器的字号菜单本来就含 18/32，无需改。

**验证**

- `cargo test --workspace` **409 项全绿**（+1 字阶单测；`mw-shell` 159 不变）；
- 画布基线**必然漂移**：`offscreen` 报 `18814/540000 像素超容差（3.484%），最大差 224`，包围盒 `x[0..712] y[227..369]`。这是外观变更本身，不是回归：用 `MW_GOLDEN_UPDATE=1` 重拍 `canvas-ppp1.00.png` 与 `canvas-text-ppp1.00.png`；壳层 `cargo golden --update --warmup 5` 重拍 `shell-ppp1.50.png`（1620×1080 @ ppp 1.50），随后 `cargo golden --tol 0` **连跑两次**均 0 差异；
- `cargo clippy --workspace --all-targets` 各 crate 计数与 v5-e 逐项相同（`mw-core` 11 / `mw-render` 7 / `mw-io` 1 / `mw-golden` 1 / `mw-probe` 2 / `mw-shell` 5），**新增 0**；`cargo fmt --all --check` 干净；
- release 重编译后 `target/release/mw-shell.exe` 仍 **20,312,064 B**（改的是常量表，无新增代码段；时间戳 13:09）。

**踩坑**

1. **主题默认值不能猜**：XMind 的 `app.asar` 是 217 MB 打包体，直接 `strings` 只会把 asar 头喷一屏；能读到的是**它画出来的墨迹**。量 x-height、墨高、笔画宽度，比翻 JS 更省，也更接近用户看到的东西。
2. **字重是排版令牌，不是节点属性**：把一级从加粗改成常规，不该做成一堆 per-node override——那会让导入文档和新建文档的行为分叉。改在 `LevelStyle` 默认层，级联规则原样不动。
3. **基线要跟着外观走，但要留证据**：重拍前先记下漂移像素与包围盒；否则下一次看到 `tol 0` 通过，没人知道中间换过一版字号。

**未做 / 下一步**：一级以下的 16/14 是插值（参考图只有两层，未展开更深层）；界面 chrome 字号散点已在 H2 收敛（见 §4.43），画布字号未动；编辑态 caret 仍占满 1.45 em（v4 遗留）；IME 人工矩阵（docs/07 §8）仍是唯一人工签字项。

---

## 4.43 H2 界面 chrome 字阶令牌化（2026-10-19 夜）

v6 只管画布，壳层从标题栏到状态栏仍散着 10.0–28.0 十几个字号。H2 只做一件事：给 chrome 定一套最小字阶，把散点换掉，**不碰画布**。

**① 令牌（`mw-shell/src/chrome.rs`）**

```rust
pub struct Type;
impl Type {
    pub const CAPTION: f32 = 10.5; // 角标、键帽、页签计数
    pub const SMALL: f32 = 11.5;   // 状态栏、次级说明
    pub const BODY: f32 = 12.5;    // 面板正文、列表、按钮
    pub const LABEL: f32 = 13.5;   // 输入框、菜单项、大纲根行
    pub const HEADING: f32 = 15.0; // 面板标题、节点名
    pub const DISPLAY: f32 = 24.0; // 开始页品牌字
}
pub struct Glyph; // 18 图标 / 19 演示条单字符 / 23 开始页徽标
```

`Type` 与 `Glyph` 分开：字形尺寸不是文字字阶，混进同一组会让「图标 18」看起来像正文档。画布字阶 `LEVEL_TYPE = 32/18/16/14` 保持在 `mw-core`，两套令牌**禁止互相引用**。

**② 迁移**

- 标题栏、开始页、状态栏、FPS 卡、命令面板、右键菜单、帮助、搜索、大纲、检查器逐处替换；
- 大纲行保留原有层级：根行 `LABEL`、其余 `BODY`（不是一刀切）；
- `canvas.rs` 两处字号（加子按钮 18、无窗口测试助手 14）刻意保留，属于画布/测试，不归 chrome；
- 新增单测 `the_chrome_type_scale_has_six_monotonic_tiers`，锁住 6 档严格递增。

**③ 验证**

- `cargo test --workspace` **412 项全绿**（shell 161，+1 字阶单测）；
- `cargo fmt --all -- --check` 干净；`cargo clippy --workspace --all-targets` 仍 **40 处/36 条**，新增 0；
- 壳层 golden 先量后拍：`4593/1749600 px（0.263%），最大差 185`，包围盒 `x[16..1604] y[20..1072]`，差异集中在文字边缘（字号上调），不是布局回归。`cargo golden --update --warmup 5` 重拍后 `--tol 0` 连跑两次 0 差异；两张画布基线各 0 差异；
- release 重建 `target/release/mw-shell.exe` **20,317,184 B**；
- 截图：golden actual、`shots/qa/v7-after.png`（检查器）、`m3c-palette-open.png`（命令面板）、`h2-home.png`（开始页）均无文字截断或行高塌陷。

**踩坑**

1. **字号不是「越大越统一」**：把 11.0 和 11.5 都并到 11.5 会让状态栏和键帽抢同一档；CAPTION 留 10.5 是给角标的，不再往下拆。
2. **字形尺寸必须单列**：检查器图标 18 与正文 12.5 不是同一语义，混进 `Type` 后下一次改正文会误伤图标。
3. **golden 漂移要能解释到像素级**：本次包围盒是整窗，但 diff 只在字形边缘；如果差异出现在面板分隔线或行高上，就是布局回归，不能直接重拍。
4. **探针的前台/遮挡问题不是产品问题**：本轮复跑 `v7-branch.ps1` 时，Chrome 置顶导致所有采样点被遮，探针按设计退 3。修复是给探针加 `AttachThreadInput` 抢前台 + 测量期间临时 `TOPMOST`，`Finish` 恢复；用户窗口不动。

**未做**：真实物理 4K/150% DPI 截图（golden 是离屏 1620×1080 @ ppp 1.50）；H3 面板控件持久 id；H4 色板键盘可达。

---

## 4.42 v7 逐节点配色：分支线进入检查器，卡片跟着线换色（2026-10-19 傍晚）

v6 之后三块打磨里配色还剩一个缺口：节点能改填充与文字，却**不能改「这条分支的线」**；`StyleOverride.branch_color` 在数据结构里已存在，但检查器没有入口，`resolve_all` 也没有把它反馈到一级卡片。v7 只补这一条链路，不扩功能面。

**① 解析：线换色，卡也必须换**

`StyleResolver::resolve_all` 的新规则（`core/theme.rs`）：

- 一级节点显式 `branch_color` → 卡片填充 = `Theme::branch_fill(branch)`；亮色主题 = 白与分支色 `mix 0.5`，暗色 = 面板底色 `mix 0.2`；
- 未设置 → 仍按兄弟序号取 `palette_fill`（内置主题外观不变）；
- 二级及更深：`level.fill.mix(branch, 0.07)`、描边 `mix(branch, 0.20)`，让一条分支读成一个色系；
- 显式节点 `fill` 仍压过上述派生（级联顺序未变）。

这样避免了最刺眼的半成品：用户只把线改成红色，卡片却还是原来的薄荷绿。

**② 检查器：三行色板，顺序即语义**

样式页固定为 **填充 → 分支线 → 文字**：

- 「分支线」行直接写 `style.branch_color`，hover 文案说明「子树连线与二三级卡片染色都跟它走」；
- 每行保留「已覆盖」圆点与 ↺ 恢复继承；文字行另有「文字样式应用到子树」复选；
- 色块 17px（内嵌 6px 圆点、间距 3px），自定义色按钮 22px；点击只产生一条 `SetStyle`，走既有撤销栈。

**③ 验证与诚实边界**

- 新增两条 core 单测：`an_explicit_branch_color_repaints_its_level_one_card`、`branch_color_is_a_per_node_override_that_clears_back_to_inheritance`；全库 **411 测试全绿**；
- golden `--tol 0` 连跑两次通过，`cargo clippy` 新增 0；
- **真实输入已 PASS（2026-10-19 晚，本机）**：`scripts/v7-branch.ps1` 退出码 0，6 张截图逐张肉眼核对均为 mw-shell。探针先 F8 关大纲、点空白清选；RIGHT 选中 Root，LEFT 选中左侧 Gamma，`selection_diff=2710`；F9 打开检查器后，第二条宽带（y=436–449、7 个色块）上点击 `#e4635b`（1290,442）。结果：画布 `canvas_repaint=12587` px，自动保存 `content.json` 出现 `"branchColor":"#e4635b"` 且归属节点 `c`（Gamma），Ctrl+Z 后 `undo_vs_before=0`。探针已加 `WindowFromPoint`+`GetAncestor` 遮挡守卫与点击点 `At` 断言，任一帧被外部窗口覆盖即退出码 3（INCONCLUSIVE）；本次没有 blind 帧。
- 口径说明：色板扫描本次得到 5 条高饱和横带（主三行之外，y=723/759 还有两条更窄的色带，`max=91`），探针固定取第二条宽带的中心；`runs=7`，与「三行色板、固定顺序」的 UI 不矛盾。
- 已知缺口：三行色板都只有鼠标路径，键盘可达性待 H4；「填充/文字」的逐节点真实输入同样未单独验收。

---

## 5. 中文四痛点：归属与验收

| # | 痛点 | 归属 | 解法 | 验收 |
| --- | --- | --- | --- | --- |
| C1 | IME 候选窗错位 / 组合输入丢字 / 语义键被吞 | 视图层 | ✅ **代码半边已落地**（M2a，见 §4.12）：行内编辑走 egui `TextEdit` 覆盖层，egui-winit 每帧按 caret 矩形调 `set_ime_cursor_area`（等价于 `set_ime_position`）；组合串直接显示在节点内；`ed.ime` 为真时 Enter/Esc 交还输入法，快捷键整体挂起；画布每帧 `set_focus_lock_filter` 保住 Tab/方向键/Esc | docs/07 §8 人工矩阵 5 款输入法（微软拼音 Win10/11、搜狗、QQ、微信）签字 → **仍未做**（任务 #4，可与 M2b 并行）。SendKeys 粘贴链路已自动化验证（`scripts/m2a-edit-probe.ps1`），但它替代不了候选窗定位的人眼核对 |
| C2 | 换行 / 禁则 / 标点位置错误 | 视图层文本 | DirectWrite 按 `zh-Hans` locale 行断；docs/07 §4 禁则表作为兜底与测试语料。✅ **代码半边已落地**（M1b-2a，见 §4.8）：策略层 `mw-core::text`、引擎层 `mw-render::dwrite`；实测 DWrite 的 kinsoku 表**比我们的窄**（`——`/`…` 仍会起行），兜底不可省 | 20 条 CJK 禁则用例全绿（`，。、；：！？）」』` 不落行首等）→ **已达成并扩到 26 个字符逐个断言 + 12 条语料×3 宽度不变量**（13 §4.8） |
| C3 | 文字发虚 / 字体回退不对 | 视图层文本 + DPI | DirectWrite 栅格化（ClearType/自然度量）+ Per-Monitor V2 感知 + 雅黑 UI↔Segoe UI 回退链。✅ **全部落地**：回退链与度量在 M1b-2a（§4.8），灰度 1×1 字形图集 + 文本实例族在 M1b-2b（§4.9）；画布缩放 ⇒ 明确不做次像素 3×1 | 100/125/150/200% 截图与物理尺寸断言已自动化（`text-ppp*.png`，文字包围盒随 DPI 线性）；中英混排同一基线由 `<IDWriteTextLayout>` 单次整形保证 → **代码验收达成**，人眼签字随 M0 ④ 一并做 |
| C4 | 拼音搜索不准 | **`mw-core/src/search.rs`（与 UI 无关）** | ✅ **已于本轮修复**：多音字按 `heteronym` 表展开（主音优先，≤8 变体）、全拼匹配要求**音节边界对齐**（`an` 不再假命中「项目」）、允许末音节不完整（`xiangm` 命中）、首字母匹配独立打分、命中映射回原文**字符区间**（供高亮）、同分按**文档前序**稳定排序（上一/下一个可预测） | 新增 6 项单测；`cargo test` 49 项全绿 |

**C4 是本轮唯一与路线无关、已交付的部分**——它证明一件事：核心逻辑不必陪 WebView 一起作废。

---

## 6. 里程碑（≈17–19 周）

| 阶段 | 周 | 内容 | 出口标准 |
| --- | --- | --- | --- |
| **M0 定案与骨架** | 2 | ① 三探针：**IME 跟随**（候选窗定位到画布任意节点光标）、**文本质量**（禁则/DPI/混排，egui vs DirectWrite 并排截图）、**性能**（5k 节点 pan/zoom 帧率）；② 窗口骨架：Mica、合并标题栏、自绘窗口按钮；③ §7 验收 harness（离屏渲染 + golden image）搭好 | 三探针有数据 → **UI 框架定案并记 ADR**；空窗可拖拽缩放无撕裂 |
| **M1 渲染引擎** | 4 | Scene 移植为 Rust（布局进 core）；wgpu 四层合成；屏幕空间恒定铬件；uniform grid 命中；平移/缩放/选择；LOD 与帧预算 | 5k 节点 ≥55fps；任意缩放档位控件度量恒定；导出与画布同源 |
| **M2 编辑与手势** | 4 | 节点内 IME 编辑（C1）；快捷键全集（docs/02 §10）；撤销接 `mw-core`；拖拽幽灵跟手 + 对齐吸附 + 拖出连线 + 框选整组；docs/06 §8 动效表 | docs/07 §8 IME 矩阵通过；拖拽无跳帧；连线 3 步可达 |
| **M3 面板与壳** | 3 | 大纲 / 检查器 / 命令面板 / 搜索 / 小地图 / 主题 / 导出 / 多标签 / 恢复槽；原生菜单与对话框；文件关联、单实例、跳转列表、窗口状态记忆 | 与 Win11 原生应用并排截图盲测不违和 |
| **M4 互通与导出** | 2 | `.zmind` 全回归；MD/OPML/FreeMind/XMind 导入回归；SVG/PNG/**PDF**（E4）；鱼骨/时间轴布局（E5） | docs/11 §2 全部验证项在新栈复现 |
| **M5 生产打磨** | 2 | docs/08 §2 P1–P14 预算逐项；DPI/多屏/休眠唤醒；低档机降级；安装包；文档修订 | 预算表全绿；11-实现状态更新至 v0.3 |

**关键路径**：M0 → M1（文本管线是 M1 内最大不确定项，约 3–4 周）。M3 的壳层可与 M1/M2 部分并行。

**M0 进度（2026-10-16）**：① 三探针出数 → §4.2；② 窗口骨架（Mica / 合并标题栏 / 自绘按钮 / 原生拖拽缩放）→ §4.4，九项验证全过；③ §7 验收 harness（离屏渲染 + golden image）**已建并做过负向测试** → §4.5；④ ADR-011 仍为提案，卡在 5 款输入法人工矩阵与 200%/深色底补拍——**M0 只剩 ④ 这一项人工签字**。

**M1 进度（2026-10-17）**：**M1a 已完成** → §4.6（含 5k 节点重排实测）；**M1b-1「`mw-render` 四层合成器骨架」已完成** → §4.7（离屏渲染 + 铬件恒定像素验收 + 画布 golden 基线 + 每帧实例成本实测）；**M1b-2a「DirectWrite 度量 + 中文禁则」已完成** → §4.8（26 字符禁则门 + 12 条语料不变量 + 真字体宽度 + 度量成本）；**M1b-2b「字形图集 + 文本实例族」已完成** → §4.9（灰度图集 + 双栅格化路径 + 四类文字定位 + 四档 DPI 像素验收）；**M1b-3「uniform grid 命中 + 平移缩放选择 + 合成纹理进 egui」已完成** → §4.10（真实窗口里可选 / 可折叠 / 可平移 / 可缩放，壳层 golden 重拍且容差 0）；**M1c「视口裁剪 + LOD + 帧预算 + 摘要虚线」已完成** → §4.11（5k 节点全帧串行上界 **236 fps**，P5 预算余量 **4.3×**；三张 golden 重拍；无裁剪 4.55 ms → 裁剪 0.29 ms）。**M1 完成**。出口标准两条已实测达标（5k ≥55 fps、任意缩放档位控件度量恒定）；第三条「导出与画布同源」的**架构约束已成立**（同一 `Scene`/`Frame`/`build_layers`，`viewport: None` 即全图），真正的 SVG/PNG/PDF 导出器按 §9 留在 M4，不再挡 M1。M0 的 ④（IME 人工矩阵）仍是唯一未签字项，可与 M2 并行。


**M2 进度（2026-10-17）**：**M2a「节点内 IME 编辑 + 核心快捷键 + 撤销重做」已完成** → §4.12（egui `TextEdit` 覆盖层 + caret 区域喂 IME + 组合期挂键 + 实时重排 + F2/双击/Tab/Enter/Delete/Ctrl+Z/Y 全链路，真实窗口 SendKeys 回归 11 张截图；183 项测试全绿；壳层 golden 因标题栏中文字体重拍并连跑容差 0）。**M2b-1「手势纯逻辑 crate `mw-ui`」已完成**（2026-10-17）：`select.rs` 有序多选（末尾=主选）、`marquee.rs` 相交框选、`drag.rs` 三分区落点 + 子树禁令 + 多根插入索引逐条模拟（对齐 `Document::MoveNode` 的「以移动前兄弟数组为参照」口径）、`snap.rs` 六线最近对齐，**27 项单测**、`cargo clippy -p mw-ui --all-targets` 0 警告。**M2b-2「壳层接多选 + 框选 + 平移改键」已完成** → §4.13：`mw-shell` 依赖 `mw-ui`、`Selection` 有序多选、按下定手势的 `handle_pointer`、框选虚线框、整组删除、编辑期锁缩放；真实输入 13 张截图回归，全库 **216 项**全绿。**M2b-3「拖拽重排 / 改父 + 合成器幽灵 + 三分区落点」也已完成** → §4.14：`Frame.ghost`（原位 28% 残影 + 55% 幽灵副本 + 多节点并集虚线框）、文字跟手、`DragKind::Move` 接 `plan_moves`/`move_commands`、三分区指示与禁止光标、Esc 取消；真实输入四个场景（改父 / 非法 / 同父重排 / 多选整组），全库 **219 项**全绿。**M2b-4「对齐吸附参考线 + 拖出连线」也已完成** → §4.15：`Frame.guides`（1px 屏幕恒定细线）、`Moving` 内吸附（同类线配对 + 60ms 延迟 + 剔除自身子树）、`mw-ui::link`（四边锚点 / 最近锚点 / 落点节点）、`DragKind::Link` 预览曲线 + `AddRelationship`。**M2b 只剩「拖到空白分离自由主题」一项**（缺「脱离父级」命令）。**M3-a「文档会话 + 打开/保存/另存为」已完成** → §4.16：会话从画布撤到 `Session`、`mw-format::save/load` 接上原生文件对话框、窗口级 `Ctrl+O/S/Shift+S`、标题栏真实文件名与未保存点、关窗与「打开」两个丢数据入口都拦一层；端到端实拍证明「改完存下、重启还在」。**M3-b「拖放打开 + 最近文件 + 新建空白 + 自动保存/恢复」也已完成** → §4.17：`Ctrl+N` 空白文档、标题栏 `▾` 最近文件、按扩展名分流导入、每 3 秒写恢复点（不碰正本）、崩溃后启动弹「发现恢复点」。**M3-c-a「大纲面板 + 状态栏 + 命令面板」已完成** → §4.18（性能基线首次实测：exe 17.6MB、私有内存 318MB）。**M3-c-b「检查器 + 小地图 + 状态栏选中路径」也已完成** → §4.19（右侧 260px `F9` 四页签、200×140 小地图、选中路径；修掉加选清空/浮层漏点击/面板 Enter 多建同级三个回归；exe 18.0MB、私有内存 318MB）。**M3 只剩多标签**（现已完成，见 §4.22）；之后 M2c 快捷键全集 + 动效表 → M4 导出。**视觉 v2**（2026-10-18，§4.20）以本机 XMind 2026 为基准重做了标题栏、汉堡菜单、主题与检查器；随后接上**搜索条**（2026-10-18，§4.21：`Ctrl+F` 输入即搜、命中字符高亮、`Enter` 逐条跳转）与**多标签**（2026-10-18，§4.22：单画布会话对调，一个 16MB 图集跨文档复用；标签条 + `Ctrl+T/W/Tab` + 同文件去重 + 脏页关闭确认），全库 274 项、golden 容差 0×2。至此 **M3 面板与壳的功能项全部落地**，只剩「右键菜单」这一项壳层打磨（窗口状态记忆 §4.27、开始页 §4.28 已完成）。M2 出口标准里的「docs/07 §8 IME 矩阵通过」仍是唯一人工签字项（任务 #4）；「拖拽无跳帧」还缺帧耗时实测（真实输入交互已验，不等于性能预算达标）；「连线 3 步可达」已由 M2b-4 实现。

**M4 进度（2026-10-18）**：**M4 前半「导出」已完成** → §4.23：`mw-shell/src/export.rs`（GPU 无关的格式清单/文件名消毒/文本写出/尺寸预算/预乘还原）+ `canvas::export_png` 走 `Frame.viewport = None` 离屏全图路径 + `Compositor::set_transparent` 透明底开关，五种格式（PNG / Markdown / 缩进大纲文本 / OPML / FreeMind）配 `Ctrl+E` 对话框（比例 1×/2×/3×、文档底色/透明）+ 原生保存对话框。**顺手修掉一个真实缺陷**：透明底导出原先改的是 `theme.ui.canvas_bg`，而这条令牌兼着挖空色，导致标签胶囊文字/关系线标签底/折叠圆点底一起消失（§4.23）。**M4 后半也已完成** → §4.24：SVG 走独立「Scene → 真实 `<text>`/贝塞尔」管线、PDF 按「先嵌图」路线嵌入 Flate RGB(+SMask) 位图并按 96dpi 定页面；导入五种格式接上汉堡菜单/命令面板/原生「导入文件」对话框，**开新标签页、不碰当前文档**，并让标签显示来源文件名；`.zmind` 深往返与 MD/大纲/OPML/FreeMind/XMind 结构断言补齐。实测 **300 项全绿**、clippy 存量 38（shell 6）、fmt 干净、golden 容差 0×2；真实产物 SVG 1093.55×342 世界单位 / PDF 820.16×256.5 pt 单页单图 0 可选字符 / PNG 2× 2187×684，导入另存为 1,168 B 的 `.zmind`。**M4 最后一项 E5「鱼骨 / 时间轴布局」也已完成** → §4.25：`LayoutKind` 扩到五种，鱼骨走「向左主轴 + 上下交替分支 + 每侧独立槽位」，时间轴走「向右轴 + 全局槽位推进」；`Layout.spines`/`Scene.spines` 进 L1 与 SVG，检查器 chips 改 3+2。全库 **309 项**（mw-core 114 / mw-shell 84）、0 新增警告、fmt 干净、golden 容差 0×2；真实窗口鱼骨/时间轴截图与同文档即时切换均已验证。**M4 完成**。

**M2c 进度（2026-10-18）**：**快捷键主体与两项动效已完成** → §4.26：`Ctrl+C/X/V/D`、`Ctrl+Shift+C/V`（只搬显式样式覆盖）、`Alt+↑↓`、`Ctrl+Shift+←→`、折叠三键（`/`、`Ctrl+/`、`Ctrl+Shift+/`，注意 winit 的 `Questionmark`）、`Ctrl+0/1/2`、`Alt+Enter` 聚焦 + 面包屑、`F1` 速查浮层（`panels/help.rs`）、`Ctrl+P` PDF、`F11` 专注模式。两处绕开框架：画布读走 egui-winit 的 `Event::Copy/Cut/Paste` 自己派发节点剪贴板；F11 用同步 `SetWindowPos` 铺满显示器并置顶（winit 的异步尺寸命令会让 DWM 停在旧帧）。动效落地面板开合 0.2 s `cubic_out` 与搜索命中两拍呼吸环，读 `SPI_GETCLIENTAREAANIMATION` 尊重系统「减少动画」，golden harness 下走终态。全库 **315 项**（mw-shell 84 → 90）、0 新增警告、fmt 干净、golden 容差 0×3；真实输入 24 张截图、F11 1620×1080→1920×1200→1620×1080；release 19,879,424 B / 私有 319.1–321 MB / 冷启动 458–669 ms。**结构动效也已完成 → §4.30**：`Scene` 全面补 alpha、`mw_core::blend_scenes` 纯函数插值、壳层 `Motion`（0.20–0.34 s ease-out-cubic，跨度决定时长，连续命令从屏幕中间态续接），节点增删从父盒长出/淡出、折叠展开整棵子树收缩与淡入、布局切换节点落位 + 轴线交叉淡化；目标场景索引恒定，选择/命中/编辑器不追视觉中间态。全库 **346 项**（mw-core 114 → 121、mw-shell 110 → 114）、0 新增警告、fmt 干净、golden 容差 0×2；真实输入 20 张截图（`scripts/m2c-anim.ps1`）。**演示模式也已完成 → §4.32**：`F5` 从选中节点起按文档前序步进，350ms 中心 + 对数缩放缓动，当前路径 alpha 保持 1、其余压暗，深色纯背景 + 底部控制条（进度/上一步/下一步/退出，`H` 隐藏），`Esc`/`F5` 退出并精确还原视口；全库 **361 项**（mw-shell 120 → 128）、0 新增警告、fmt 干净、golden 容差 0×2、真实输入 15 张截图（`scripts/m2c-demo.ps1`）。**M2c 只剩**快捷键配置（非动效表项）。**下一步是 M5 预算与打包**（M3 已全部完成）。

**M3 进度（2026-10-18）**：**窗口状态记忆已完成** → §4.27：`window_state.rs` 存外框物理像素 + 最大化态 + 大纲/检查器/小地图开合到 `%APPDATA%\织思\window.json`；最大化时用 `GetWindowPlacement` 的常态矩形，恢复前校验标题栏锚点还在某台显示器上（坏几何只丢几何、面板照常），同步 `SetWindowPos` 恢复、winit 接管最大化，700ms 防抖 + `on_exit` 兜底保存，harness 走 disabled；补上恢复 resize 后的 500ms 重新取景。全库 **324 项**（mw-shell 90 → 99）、0 新增警告、fmt 干净、golden 容差 0×2；真实输入 4 场景 6 图（`scripts/m3-window-state.ps1`）证明移动/缩放/最大化/离屏回退与面板状态跨进程往返。**开始页也已完成** → §4.28：默认启动落点从夹具文档改为开始页（`MW_OPEN` / harness / `MW_HOME=0` 例外）、5 模板 + 最近文件、关最后一页回开始页、菜单可盖住文档并 `Esc` 返回、动态窗口标题、按 `SPI_GETWORKAREA` 工作区居中；全库 **330 项**（mw-shell 99 → 105）、golden 容差 0×2、真实输入 7 图（`scripts/m3-home.ps1`）。**右键菜单已完成 → §4.29，M3 收口**：节点/多选/空白三套菜单、240px 白卡、键盘 `↑↓`/`Enter`/`Esc`、贴底上翻、开着时画布输入锁定；全库 **335 项**（mw-shell 105 → 110）、0 新增警告、fmt 干净、golden 容差 0×2、真实输入 8 图（`scripts/m3-context.ps1`）并在 `Ctrl+S` 后解包断言结构。**M2c 结构动效随后完成 → §4.30**（全库 **346 项**、golden 容差 0×2、真实输入 20 图、release 20,054,016 B）。之后只剩 M5 预算与打包。

**M5 进度（2026-10-18）**：**测量基建、P1–P5/P6/P8/P9/P10/P11/P12/P13/P14 实测与便携打包已完成** → §4.31：`MW_PERF_LOG` 启动里程碑、`MW_STATS` 运行时卡片 + TSV、`scripts/make-sample.mjs --nodes=N` 合成语料、`scripts/m5-perf.ps1` 真实输入探针、`scripts/m5-memory.ps1` 五分钟稳态、`scripts/package-release.ps1` 便携 ZIP、`MW_EXPORT_BENCH` 导出计时。P1 **325.8ms**、P2 **291.8ms**、P3 **63.2ms**、P4 **257.2ms**、P5 **60fps**（93% 可见，墙钟 p95 17.73ms、UI p95 1.78ms）、P14 **7.39MB** 全部通过；**P13 攻关前为红**（10k 五分钟稳态 ~335MB > 300MB，空开始页已 318–321MB，归因 DX12/wgpu + 字体 + 图集，非文档规模）。**P10 后台自动保存已补齐**：UI 线程只克隆快照（10k p95 4.7ms）、worker 写盘 p95 27.4ms，合并/epoch/invalidate/降级/退出收尾齐全，真实窗口两向验证通过。**P11 后台导出已补齐**：5k 工作线程 70.9ms、UI 克隆 0.94ms；但合成语料的长宽比触发 8192/32MP 护栏，2× 实际按 0.057× 输出，真实 2× 由 3 节点样例验证（910×300，38.8ms）。全库 **353 项**（shell 120、core 122）、clippy 存量 38（shell 6，P11 新增代码 0 条）、fmt 干净、golden 容差 0×2（**其后演示模式使全库变为 361 / shell 128，见 §4.32**）。**P13 攻关 → §4.33（2026-10-19 凌晨收尾）**：字体改只读内存映射（−34MB）、图集 4096→3072（−7MB）后 10k 五分钟稳态 **299.6MB**（余量仅 0.4MB），再移除图集 CPU 镜像（实测 −12MB 空应用 / −14.6MB 10k）后为 **284.4–287.9MB**，**P13 判 PASS**。**M5 剩余**：P15、P7 增量布局、真安装器（当前只有便携 ZIP）、仓库缺 LICENSE 文件、低档机降级校准。

---

## 7. 验收体系（取代 CDP 冒烟）

| 手段 | 覆盖 | 说明 |
| --- | --- | --- |
| Rust 单测（**现有 408 项**：mw-core 136 / mw-render 55（24 单元 + 31 集成）/ mw-ui 32 / mw-shell 159 / mw-io 14 / mw-format 7 / mw-golden 5；增长轨迹 M1a +70、M1b-1 +11、M1b-2a +17、M1b-2b +11、M1b-3 +8、M1c +6、M2a +2、M2b-1 +27、M2b-2 +9、M2b-3 +3、M2b-4 +8、M3-a +5、M3-b +12、M3-c-a +8、M3-c-b +9、视觉 v2 +1、搜索条 +6、多标签 +6、M4 前半导出 +8、M4 后半 SVG/PDF/导入 +18、E5 布局 +9、M2c 快捷键/浮层 +6、M3 窗口状态 +9、M3 开始页 +6、M3 右键菜单 +5、M2c 场景插值 +11（blend 7 + Motion 4）、M5 stats +2、P12 检索优化 +1、P10 后台保存 +3、P11 导出任务 +1、M2c 演示模式 +8（前序/折叠/选中起步/路径 alpha/步进边界/相机缓动/减少动画）、v3 核心打磨 +7、v4 编辑器与暗色 +7、v5-a 主题入口 +5、v5-b 指针归属 +1、**v5-c/v5-d 强调色与键盘交接 +24（core +12 / format +2 / shell +10）、v5-e 剪贴板和弦顺序 +3（shell）**；原 `mw-app` 的 3 项随 WebView 壳一起移出构建） | 领域逻辑、布局、场景、主题级联、排版折行、检索、格式、**快照比对逻辑**、**曲线细分**、**中文禁则与真字体宽度**、**字形图集与文本像素归属**、**uniform grid 命中优先级与屏幕空间容差**、**屏幕 ↔ 世界反投影**、**视口裁剪保守性**、**文字与几何同拍 LOD**、**连续掉帧才降级**、**编辑节点让位给覆盖层**、**多选主选语义 / 框选相交 / 落点三分区 / 子树禁令 / 多根插入索引 / 六线吸附**、**按下定手势的判定规则 / 选择跨重排按 id 搬运**、**检查器草稿/批量合并/混合态、小地图映射、选中路径、最近文件路径去重、搜索条结果步进与绕回、标签页环绕/关页活动位/标签名按字符截断、导出格式清单与文件名消毒、PNG 尺寸缩限与预乘还原、透明底挖空色守恒、SVG 转义与贝塞尔序列化、PDF xref 偏移与 RGB/SMask 解码、七种导入格式结构、`.zmind` 深往返、窗口矩形校验与标题栏锚点、坏存档降级、状态未变不重写、开始页模板文档合法性 / 启动落点 / 相对时间分桶 / 标题截断、右键菜单只走可执行行 / 禁用条件 / 多选同父合并、场景插值（共同节点 / 增删从父盒 / 连线端点 / 轴线交叉淡化，t=0 保留目标索引）、过渡时长随跨度夹取与缓动单调、演示前序按文档顺序（不被视觉 y 打乱）/ 折叠跳过 / 路径 alpha / 相机端点与中点、强调色 OKLCH 往返与出界仍落在请求的角度 / 非 hex 被拒且不留撤销足迹 / 切主题保留覆盖而清除回预设 / 拖动中的草稿不发命令 / 非法色阶不污染容器、面板开合不丢画布键盘焦点（id 钉死）/ F6 交接把 Tab 送进面板 / 焦点每变化只写一行 / 色板 Tab 落点与空格生效** | 与 UI 无关的部分零迁移成本；`cargo test --workspace` + `cargo check --workspace --all-targets` 0 警告 + `cargo fmt --all -- --check` 干净是每次提交的门槛 |
| **离屏渲染 golden image** ✅ 已建（壳层 + 画布） | 渲染器、排版、主题、DPI | 比对/容差/差异图三份实现收在 `crates/mw-golden`，壳层与画布共用同一口径。`mw-shell --golden`：帧画到自有纹理再回读，**隐藏窗口也能截**，所以与壁纸/遮挡无关；基线名带缩放，容差默认 0，漂移时出 `actual/` + 红色标记的 `diff/`（§4.5）。画布侧 `cargo test -p mw-render`：`Scene` 直接离屏渲染，`MW_GOLDEN_UPDATE=1` 重拍基线（§4.7） |
| **输入注入回归** ✅ 已建（壳层 + 画布 + 键盘） | 拖拽、缩放、窗口按钮、画布选择 / 折叠 / 平移 / 缩放、**编辑流与快捷键**、**M2c 快捷键/动效/F11**、**节点增删 / 折叠展开 / 布局切换的过渡帧**、**窗口状态跨进程往返** | `scripts/m0-interact.ps1` 用真实鼠标输入驱动完整 OS 模态循环，九项见 §4.4；`scripts/m1b-canvas.ps1` 在 150% DPI 下对画布注入真实输入并逐张截图（§4.10）；`scripts/m2a-edit-probe.ps1` 用 SendKeys + 剪贴板跑完整编辑链（F2 → 粘贴 → 提交 → 撤销/重做 → Tab → Enter → Esc 取消 → Delete）并逐张截图（§4.12）；`scripts/m2b2-canvas.ps1` 用置顶 + 前台校验驱动多选/框选/整组删除/平移改键/编辑期锁缩放，13 张截图（§4.13）；M2b-3/M2b-4 四场景拖拽、吸附与连线各有独立探针（§4.14/§4.15）；`scripts/m2c-keys.ps1` 覆盖剪贴板/样式/层级/折叠/缩放/聚焦/F1/PDF/面板动画/搜索环/F11，24 张截图（§4.26）；`scripts/m3-window-state.ps1` 用真实移动/缩放/F8/F9/最大化/正常退出驱动四次进程重启，6 张截图（§4.27）；`scripts/m3-home.ps1` 用真实键盘/鼠标驱动默认启动→模板→关页→菜单→Esc→最近文件跨重启，7 张截图（§4.28）；`scripts/m3-context.ps1` 用真实右键/键盘驱动三套菜单、复制粘贴、全部折叠，并在保存后解包断言结构，8 张截图（§4.29）；`scripts/m2c-anim.ps1` 用真实 Tab/Delete/`/`/`Ctrl+/`/`Ctrl+Shift+/` 与校准后的检查器布局按钮截取增删、折叠展开、布局切换的早期/中期/落定帧，20 张截图（§4.30）；`scripts/m5-perf.ps1` 用真实滚轮缩放 + 中键拖拽驱动 5k 文档并断言 P5，`scripts/m5-memory.ps1` 五分钟采样 P13（§4.31）。**每串按键前断言前台归属**，被抢焦点就重试并计数，否则半截序列会发给别的窗口，现象看起来像「应用没反应」而不是「键没送到」 |
| 人工 IME 矩阵 | C1 | docs/07 §8，5 款输入法 × 8 场景，M2 出口必做 |
| 性能探针 | docs/08 §2 P1–P14 | M0 落地埋点；✅ 布局侧基线 `cargo run -p mw-core --release --example perf_scene`（§4.6 数据表）；✅ 渲染侧 `cargo run -p mw-render --release --example perf_layers`（§4.7 裁剪前基线，§4.11 三档 × 裁剪/不裁剪 + `--frame` 全帧）。5k 节点 1600×900 全帧串行上界 **236 fps**（最差档 4.24 ms vs P5 18.18 ms），100% 缩放裁剪把几何构建从 4.55 ms 压到 0.29 ms；合成纹理已接进 egui 呈现路径（§4.10），GPU 侧用 `wait_idle` 串行上界而非 timestamp query |

---

## 8. 风险与对策

| 风险 | 概率 | 对策 |
| --- | --- | --- |
| 文本管线（DirectWrite 绑定 + 字形图集 + 增量排版）超估 | **高** | M0 探针先打通「一段中文 + 禁则 + 150% DPI」最小闭环；不通则 M1 不启动 |
| UI 框架选错，2 周沉没 | 中 | M0 双候选并排探针后再定；框架层薄封装，Scene/渲染/交互与框架解耦 |
| gpui-kit 为 `gpui-pre-*` 快照，API 破坏性变动 | 中 | 锁版本；把框架调用收进单一 `platform` 模块，便于回退或换框架 |
| 失去 WebView 后自动化验收变难 | 中 → **低** | ✅ 已消解（2026-10-16）：§7 harness 在 M0 就建成——离屏 golden image（§4.5）+ 真实输入注入回归（§4.4），没留到 M5 补 |
| 面板/控件全自绘，Fluent 质感做不出来 | 中 | 严格照 docs/12 §3.3 令牌表执行；M3 出口以「并排盲测」为准，不达标不放行 |
| 3–6 个月内功能 parity 失败，产品长期不可用 | 中 | 每阶段出口「应用可日常使用」；M2 结束即应能完成真实编辑工作，否则说明拆分有问题 |
| 现有 WebView 版在重写期彻底荒废 | 低 | 重写期间 `ui/` 冻结（只修崩溃级 bug），不再投功能；拼音等 core 修复继续 |

---

## 9. 改动面清单

| 层 | 动作 | 位置 |
| --- | --- | --- |
| 领域核心 | ✅ **已完成（M1a）**：新增几何 / 排版 / 主题 / 图标 / 布局 / 场景六个模块，零 UI 依赖 | `crates/mw-core/src/{geom,text,theme,icons,layout,scene}.rs`（新，自 TS 移植，见 §4.6） |
| 应用壳 | 重写：winit/wgpu 事件循环 + Win32 集成。文件侧完整（session/files/recent/autosave）；**界面骨架完整**：大纲 + 检查器 + 小地图 + 状态栏 + 命令面板 + **搜索条** + **多标签条** + 悬浮工具条 + 汉堡菜单（`panels/`，`F8`/`F9`/`Ctrl+K`/`Ctrl+F`/`Ctrl+T`），视觉 v2（§4.20）+ 搜索条（§4.21）+ 多标签（§4.22）+ **导出**（`export.rs` + `Ctrl+E` 对话框，§4.23）+ **窗口状态记忆**（§4.27）+ **开始页**（`start.rs`，§4.28）。**仍未做**：右键菜单 | `crates/mw-shell/`（14,468 行，含 panels / export / start / window_state） |
| 渲染 | ✅ **M1b-1**：四层合成器骨架（离屏渲染 + 铬件恒定 + golden 基线）；✅ **M1b-2a**：`dwrite` 真字体度量接进 `TextMeasurer`；✅ **M1b-2b**：字形图集 + 文本实例族，四类文字上屏；✅ **M1b-3**：uniform grid 命中索引 + 合成纹理进 egui；✅ **M1c**：视口裁剪 + 三级 LOD + 帧预算降级 + 摘要虚线 | `crates/mw-render/src/{camera,flatten,instances,compositor,dwrite,glyph,text,hit,budget}.rs` + `src/shaders/{common,lattice,rect,seg,tri,text}.wgsl`；渲染侧 M1 已完成，下一步是 M2 的编辑/手势与 M4 导出器（复用同一 `Scene`/`Frame`） |
| 交互 | ✅ **M1b-3** 平移 / 缩放 / 选择 / 折叠、✅ **M2a** 行内编辑 / 核心快捷键 / 撤销重做、✅ **M2b-2** 多选 + 框选 + 平移改键 + 编辑期锁缩放（§4.13）；✅ **M2b-1 `mw-ui` crate**（1131 行 / 32 单测）：`select.rs` 有序多选、`marquee.rs` 相交框选、`drag.rs` 三分区落点+子树禁令+多根索引模拟、`snap.rs` 六线吸附，**已在壳层接线**。✅ **M2b-3** 拖拽重排/改父 + 合成器幽灵 + 三分区落点（§4.14）、✅ **M2b-4** 吸附参考线 + 拖出连线（§4.15）、✅ **视觉 v2** 选中「＋」加子按钮与四边锚点露出（§4.20）、✅ **搜索跳转** `search_nodes`/`node_text`/`select_node_keep_focus`（§4.21）、✅ **多标签** `camera`/`swap_session`/`set_view`（§4.22）、✅ **导出** `export_png` 离屏全图（§4.23）、✅ **开始页** 离开文档时提交编辑并清浮层（§4.28）。待补：拖到空白分离自由主题（需新命令）、Ctrl+拖拽复制、右键菜单、动效 | `crates/mw-shell/src/canvas.rs`（当前，3376 行）+ `crates/mw-ui/src/{select,marquee,drag,snap}.rs`；后续待补 `interact/`（状态机、动效） |
| 面板 | 新增（框架相关）。**位置已改定：`crates/mw-shell/src/panels/`**——`mw-ui` 在 M2b-1 被定名为「无窗口、无 GPU 依赖的手势纯逻辑」crate（可脱离显示器单测），egui 控件不放进去，避免这个定位被稀释 | `crates/mw-shell/src/panels/`（原计划写 `mw-ui/src/panels/`，见 14 §6） |
| 导出 | ✅ **M4 导出已完成**（§4.23 + §4.24）：`mw-shell/src/export.rs`（格式枚举 + 文件名消毒 + 文本写出 + `content_bounds` + `png_size` + `unpremultiply`）+ `canvas::export_png/export_pdf`（`Frame.viewport = None` 离屏全图 + 一次性合成器）+ 透明底开关 `Compositor::set_transparent` + SVG 独立管线 `export_svg.rs`（真实 `<text>`/原始贝塞尔/主题令牌/可选点阵）+ PDF 编码 `export_pdf.rs`（Flate RGB + 可选 `/SMask`，页面按 96dpi）。七种格式 PNG/SVG/PDF/Markdown/缩进大纲/OPML/FreeMind；文本序列化复用 `mw-io`，位图复用画布合成路径。导入 UI 入口也在此期补齐（`files::open_import` + 菜单/面板 + 新标签页）。**未做**：真矢量 PDF、只导选中子树/可视区域 | `crates/mw-shell/src/{export,export_svg,export_pdf,canvas,files}.rs` + `crates/mw-render/src/compositor.rs`（原计划写 `mw-io/src/export.rs`，实际文本序列化留在 `mw-io`、格式接线与绘制导出落在 `mw-shell`） |
| 前端 | **删除**。**2026-10-17 进度**：`ui/node_modules`(74 MB)、`ui/dist`、`织思.exe`(13 MB)、`scripts/ui-smoke.mjs`、`scripts/feature-smoke.mjs` 已删/移出；**`ui/src`(31 文件 298 KB) 暂留作手势参考实现**（`Canvas.svelte` 的三分区/框选/拖出连线语义仍是对照物，`lib/snap.ts` 已对照移植），M2b-3/4 完成后可整目录删除 | `ui/`、`_archive/` |
| 文档 | 修订 03（技术选型）、04（架构分工）、09（里程碑）；11 待重写后更新 | `docs/` |

---

## 10. 本轮已落地（2026-10-16 起）

| 项 | 内容 | 验证 |
| --- | --- | --- |
| 路线定案 | **ADR-010**：纯 Rust 原生 UI 重写，取代 docs/12 §2 路线 A | 本文档 |
| C4 拼音检索修复 | 多音字展开、音节边界对齐、末音节不完整匹配、首字母独立打分、命中字符区间、文档顺序稳定排序 | `mw-core` 30 项单测（+6）；全库 49 项全绿；`svelte-check` 0 错误 |
| 现状取证 | 实测 release 版截图确认 docs/12 的 D1（缩放连线变形）、D5（三行壳层）、D6/D7（emoji 图标与假阴影）仍然成立 | `shots/shot-03-canvas-restored.png`、`shots/shot-05-zoomin.png` |
| M0 探针（egui 侧） | `crates/mw-probe` + `probe_egui`：Mica / 禁则 / 字体回退 / IME 管线 / 帧时间矩阵，全部命令行可复现 | §4.2 数据表；`scripts/m0-capture.ps1`、`scripts/m0-chroma.ps1`、`scripts/m0-perf.sh`；`shots/m0-egui-*.png` |
| **ADR-011 UI 框架定案（提案）** | **egui 0.37** 为壳层与控件框架；画布仍走 §3 自绘 wgpu 分层，不用逐节点 `Shape` 提交 | §4.2 实测；**待 5 款输入法人工矩阵签字后转正式** |
| M0 壳层骨架 | `crates/mw-shell`：无框 eframe 窗口 + Mica + `DWMWCP_ROUND` + 合并标题栏（自绘 ─/□/✕，关闭键 Win11 红悬停）+ 8 条 `BeginResize` 热区 | §4.4 九项全过；`scripts/m0-interact.ps1` 注入真实鼠标输入复现 |
| M0 验收 harness | `mw-shell --golden`：离屏渲染一帧 → 与基线逐像素比对；`--update` 写基线，`--tol` 调容差，漂移时落 `actual/` 与红色标记的 `diff/`；退出码 0/1/2 | §4.5：连跑 3 次容差 0 一致；副标题灰度 110→111 被 `2976 px / 最大差 1 / 包围盒正好那行字` 抓到；`cargo test -p mw-shell` 5 项 |
| **M1a 布局与场景移植** | `ui/src/lib/{layout,scene,themes,measure,icons}.ts` 1140 行 → `mw-core` 六个新模块 3125 行（含单测）：稠密 `u32` 索引、几何路径、`Rgba` 级联一次、确定性排序；`NodeMeasure` / `TextMeasurer` 两个 trait 是 M1b 字形图集的接入口 | §4.6：`cargo test --workspace` **127 项全绿**（+70）、`cargo check --workspace --all-targets` 0 警告；5000 节点 `build_scene` 中位 **8.3 ms**（P8 预算 120 ms）；`examples/perf_scene.rs` 可复现 |
| **M1b-1 四层合成器骨架**（2026-10-17） | 新 crate `mw-render`（Rust 1184 行 + WGSL 243 行：实例层 / 相机 / 曲线细分 / wgpu 管线 + 5 个着色器）：`Scene` → L0 点阵 / L1 连线 / L2 节点 / L3 覆盖层，gamma 空间预乘混合、无 MSAA 的解析抗锯齿、铬件按 CSS px ÷ 缩放。golden 基建拆成 `mw-golden`，壳层与画布共用 | §4.7：`cargo test --workspace` **138 项全绿**（+11）、0 警告、fmt 干净；真实 Vulkan 适配器上离屏渲染，同帧两次回读**逐字节相同**；缩放 1 与 3 下选中环恒 2 px / 间隙恒 3 px，而盒边界 35 → 108 px 随缩放线性；基线 `shots/golden/canvas-ppp1.00.png` 容差 0；`mw-shell --golden` 拆分后仍一致；`examples/perf_layers.rs` 把视口裁剪钉成 M1c 硬性前置 |
| **M1b-2a DirectWrite 度量 + 中文禁则**（2026-10-17） | `mw-core::text` 增补禁则策略（20 字符禁起行 / 6 字符禁收尾 + `enforce_kinsoku` 只搬字符）；新 `mw-render/src/dwrite.rs`（386 行）：工厂 + 字体族回退链 + 格式缓存 + 自然段自切 + 逐段行断 + 兜底后重量宽 + 悬挂孤儿重折（≤3 轮）+ UTF-16↔字节安全映射。`TextMeasurer` 从近似器换成真字体 | §4.8：全库 **155 项全绿**（+17）、0 警告、fmt 干净；26 个禁则字符逐个断言不落错行，12 条语料 × 3 宽度「合法且不丢字」；汉字步进精确 1 em、`Rust` 29.463；同一输入两次度量逐字节相同（带缓存）；5000 节点全量重排 **47.79 ms**（P8 120 ms 内）、缓存热 p95 **19.64 ms**（超 P7，与 `build_layers` 同一结论：视口裁剪是 M1c 硬前提） |
| **M1b-2b 字形图集 + 文本实例族**（2026-10-17） | 新 `mw-render/src/glyph.rs`（551 行）+ `text.rs`（557 行）+ `shaders/text.wgsl`（42 行）：Factory2 灰度 `ALIASED_1x1` 主路径（基工厂 ClearType 亮度兜底）、4096² R8 货架图集（ppd 十档向上取桶、1 texel 间隙、脏矩形/整表重传）、`IDWriteTextRenderer` 整形与字体面去重、0.1 em 中英簇间隙、节点/胶囊/摘要/关系标签四类基线；**两趟 paint** 保证图集重置不使已发 UV 失效 | §4.9：全库 **166 项全绿**（+11）、0 警告、fmt 干净；两行节点行距严格 20 px；开/关文本的像素差异全部落在字形四边形内；重绘逐字节稳定；ppp 1.0/1.25/1.5/2.0 文字高度随 DPI 线性（`shots/render/text-ppp*.png`）；新基线 `shots/golden/canvas-text-ppp1.00.png` 容差 0，几何基线未动 |
| **M1b-3 uniform grid 命中 + 画布交互**（2026-10-17） | 新 `mw-render/src/hit.rs`（620 行）：七类目标 + 128 世界单位网格 + 大件列表 + 屏幕空间容差 + 类型优先级；`camera` 增 `screen_to_world`；新 `mw-shell/src/canvas.rs`（374 行）：场景/命中/相机/合成/文本五件套 + 首帧适配 + 平移缩放选择折叠 + 原生纹理注册；窗口 `centered` + 钳制修复出屏 | §4.10：全库 **174 项全绿**（+8）、0 警告、fmt 干净；壳层 golden 重拍 `shell-ppp1.50.png`（画布已入镜）连跑容差 0；`scripts/m1b-canvas.ps1` 真实输入验证选择 / 折叠 / 平移 / 缩放，截图 `shots/render/m1b3-*.png` |
| **M1c 视口裁剪 + LOD + 帧预算**（2026-10-17） | `Frame.viewport` + `sees` / `text_visible` / `labels_visible` / `badges_visible`：L1/L2/L3 全族与文本**同拍**裁剪；路径用控制点凸包 + 线宽外扩，关系线整组判可见；7 物理像素文字门槛、5k 节点隐藏标签/角标；新 `budget.rs` 连续 3 s <45 fps 锁存省渲染（只关点阵）；`rect.wgsl` 按圆角周长连续采样摘要虚线；缩放下限 0.1 → 0.005、细分容差分母同步 | §4.11：全库 **181 项全绿**（M1c +6）、0 警告、fmt 干净；5k 节点 1600×900 全帧串行上界 **236 fps**（4.24 ms vs P5 18.18 ms，余量 4.3×）；100% 缩放 `build_layers` **4.55 → 0.29 ms**；三张 golden 重拍后壳层容差 0；真实输入五张截图无屏边半截字 |
| **M2a 行内 IME 编辑 + 核心快捷键**（2026-10-17） | 编辑器走 **egui `TextEdit` 覆盖层**（不自绘 caret）：caret 矩形喂 `set_ime_cursor_area`、组合期挂起 Enter/Esc、每帧把 `editor.text` 喂 `build_scene` 让节点盒实时增长、`Frame.editing` 让合成器让位（不整形该节点字、不画它的环）、新 `fonts.rs` 把雅黑 UI 装进 egui 字体表；快捷键 F2 / 双击 / Tab / Enter / Delete / Esc / Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z；画布 `set_focus_lock_filter` + 提交后 `refocus_canvas`；隐形热区与标题栏拖拽区改不可聚焦；点击判定改 `clicked_by(Primary)` | §4.12：全库 **183 项全绿**（M2a +2）、0 警告、fmt 干净；`scripts/m2a-edit-probe.ps1` 真实窗口跑完「粘贴中文 → 提交 → 撤销 → 重做 → 加子 → 加同级 → Esc 取消 → 删除」11 张截图 + 逐键日志；壳层 golden 因标题栏中文字体重拍，连跑两次容差 0 |
| **M2b-1 手势纯逻辑 crate `mw-ui`**（2026-10-17） | 新 crate（987 行，只依赖 `mw-core`，无窗口无 GPU）：`select.rs` 有序多选（**末尾=主选**，组内成员被普通单击时只提主选不拆组）、`marquee.rs` 相交框选（按 `draw_order` 浅→深，末尾主选）、`drag.rs` 三分区落点（外扩 6×4 CSS px、面积最小者胜、根只收子级）+ 子树禁令 + **多根插入索引逐条模拟**（替换旧版 `+seq` 近似，口径与 `Document::MoveNode` 的「以移动前兄弟数组为参照」严格对齐）+ `move_commands` 打包 `Batch`、`snap.rs` 六线（左中右/上中下）同类线最近配对，阈值 `6/zoom` | `cargo test -p mw-ui` **27 项全绿**、`clippy --all-targets` 0 警告、fmt 干净；对照移植自 `ui/Canvas.svelte:571-599` 与 `ui/lib/snap.ts:50-128`，并补上旧版缺的**非法目标显式红色判定**（`DropTarget::illegal`） |
| **M2b-2 壳层接多选 + 框选 + 平移改键**（2026-10-17） | `mw-shell` 加 `mw-ui` 依赖；`selected: Option<u32>` → `mw_ui::Selection`（末尾主选）；新 `handle_pointer`：按下瞬间用 `press_origin` 定手势（中键/空格=平移、空白左键=框选、节点左键暂仍平移），平移自己结算增量绕开 egui 的 6px 启动阈值；框选 4px 阈值自判 + 松手吞 `clicked`；选择跨重排按 **id** 搬运、整组丢失才回落根；`DeleteNodes{ids}` 整组删除并落回主选第一个存活祖先；橡皮筋走 egui 虚线层（不进 `Frame`）；`zoom_delta` 分支加 `&& !editing`（docs/06:108 那条补完）；`scripts/m1b-canvas.ps1` 平移步骤改中键、新增 `scripts/m2b2-canvas.ps1` 13 张截图回归 | §4.13：全库 **216 项全绿**（+9，全在 `mw-shell`：手势判定 4 + 选择搬运 5）、0 新增警告、fmt 干净；`cargo golden` 容差 0；真实输入实测框选/加选/减选/清空/整组删除/空格拖/中键拖全部通过，编辑期锁缩放由「87 px、3×29 的 caret 差」与「非编辑态 348,702 px 的对照组」共同佐证 |
| **M2b-3 拖拽重排 + 合成器幽灵**（2026-10-17） | `Frame.ghost`（`Ghost{nodes,offset,illegal}`）：被拖子树原位淡化 28% + 偏移处 55% 副本（盒 + 标签 + 文字），多节点再加并集虚线框；壳层 `DragKind::Move` + `Moving`（按下树快照 / drag_roots / subtree / 落点）、`commit_move` 发一次 `MoveNode` 或 `Batch`；三分区指示与禁止光标画在 egui 层；Esc 取消手势并吃掉按键，指针失联时兜底收尾 | §4.14：全库 **219 项全绿**（+3，offline 幽灵三例）、0 新增警告、fmt 干净、`cargo golden` 容差 0；`scripts/m2b3-drag.ps1` 真实鼠标四场景（改父 / 非法 / 同父重排 / 多选整组）逐张截图，非法场景 tree 一字未动 |
| **M2b-4 吸附参考线 + 拖出连线**（2026-10-17） | `GuideLine` + `Frame.guides`（1px 屏幕恒定，L3 最后画）；`Moving` 内 `snap_offsets`（参照剔除自身子树、60ms 延迟抽成 `snap_ready`）；新 `mw-ui::link`（四边锚点/最近锚点/落点节点）；`DragKind::Link` + 预览贝塞尔 + 目标亮环 + `AddRelationship`；悬停露出 4 个锚点 | §4.15：全库 **227 项全绿**（+8）、0 新增警告、fmt 干净、`cargo golden` 容差 0；`scripts/m2b4-canvas.ps1` 真实鼠标验证吸附参考线与幽灵顶边逐像素齐平、锚点拖出关系线成功、Esc 取消无副作用 |
| **M3-a 文档会话 + 打开/保存**（2026-10-17） | 新 `mw-shell/src/session.rs`（`Session`：path/doc/settings/theme/saved_revision，`open`/`save`/`save_to`/`title`/`dirty`）与 `files.rs`（`IFileOpenDialog`/`IFileSaveDialog`，必传 owner HWND）；画布字段换成 `session`；`main.rs` 加窗口级 `Ctrl+O/S/Shift+S`、标题栏真实文件名 + 未保存点、关闭拦截（`CancelClose`）、「有改动时打开」确认、左下状态条、`MW_OPEN` 启动钩子 | §4.16：全库 **232 项全绿**（+5）、0 新增警告、fmt 干净、`cargo golden` 容差 0；`scripts/m3-file-e2e.ps1` 端到端实拍「Node 写 → 打开 → 加节点 → Ctrl+S → 重启仍在」，并验证 Ctrl+O 原生对话框与 Alt+F4 拦截 |
| **M3-b 拖放 / 最近 / 新建 / 自动保存**（2026-10-17） | 新 `recent.rs`（`%APPDATA%\织思\recent.json`，8 条、坏文件降级、启动清失效项）与 `autosave.rs`（`.autosave/` 恢复点、只认"比正本新"、保存后自动清除）；`session.rs` 加 `blank`/`accepts`/`import`/`open_recovered`；`main.rs` 用 `PendingAction` 统一三种换文档入口，加 `Ctrl+N`、拖放分流、标题栏 `▾` 菜单、自动保存（**主动催帧**）、启动恢复提示 | §4.17：全库 **244 项全绿**（+12）、0 新增警告、fmt 干净、`cargo golden` 重拍后连跑容差 0；实拍 `Ctrl+N` 空白、最近文件菜单、自动保存落盘、`taskkill /F` 后恢复提示与内容复原 |
| **M3-c-a 大纲 + 状态栏 + 命令面板**（2026-10-18） | 新 `panels/`（`outline.rs` 行缓存 + 虚拟滚动；`command.rs` 11 条命令 + 子串过滤 + 键盘优先）；画布新公开 API（`select_node`/`toggle_collapsed`/`fit_view`/`zoom_by`/`set_input_blocked` 等）；`Panel::left` 大纲（232px）+ `Panel::bottom` 状态栏 + `F8`/`Ctrl+K`；性能基线首次实测（exe 17.6MB / 私有 318MB，M5 预算已立） | §4.18：全库 **252 项全绿**（+8）、0 新增警告、fmt 干净、golden 重拍后连跑两次容差 0；实拍四张（行选中+相机居中、折叠联动、F8 隐藏、Ctrl+K 开面板 + Enter 执行） |
| **M3-c-b 检查器 + 小地图 + 选中路径**（2026-10-18） | 新 `panels/inspector.rs`（四个页签：样式/备注/链接/标记；无选中=文档属性，多选=混合态批量 `Batch`；备注草稿防冲掉）；`canvas.rs` 新 API（`apply_command`/`request_canvas_focus`/`selection_path`/`minimap`/`set_overlay`，小地图 900 个确定性抽样矩形 + 视口框）；`main.rs` 右侧 260px `F9`、浮层挡画布输入、状态栏路径；修掉加选被清空、浮层点击漏画布、检查器 Enter 多建同级三个回归；冷启动私有 318MB / exe 18.0MB | §4.19：全库 **261 项全绿**（+9）、0 新增警告、fmt 干净、golden 重拍后连跑两次容差 0；`scripts/m3c2-inspector.ps1` 14 张实拍（样式/备注/链接/标记/多选/小地图平移·折叠·展开/F9 隐藏） |
| **视觉 v2：标题栏 / 菜单 / 主题 / 检查器**（2026-10-18） | `main.rs` 重做标题栏：56px 两行标题 + 中央 40px 白色悬浮胶囊（6 个 34px 矢量按钮）+ 46px 原生窗口键；汉堡菜单换成 258px 白卡 + 矢量图标 + 右对齐快捷键 + 最近文件 + 面板勾选；`theme.rs` 4 套主题统一 22/17/15/14 字号，一级主题取 `palette_fill` 实色卡片；`canvas.rs` 加选中「＋」加子与悬停四边锚点，小地图改 190×130 白卡；`inspector.rs` 四页签统一 14px 缩进、输入框改可见描边；`recent.rs` 相对/绝对路径归一化去重 | §4.20：全库 **262 项全绿**（+1 recent 去重）、0 新增警告、fmt 干净；壳层与画布文字 golden 重拍后 `cargo golden --tol 0` 连跑两次容差 0；release 截图 5 张 `shots/qa/final-*-2026.png` |
| **搜索条：Ctrl+F 检索与定位**（2026-10-18） | 新 `panels/search.rs`（状态 + 卡片 UI，结果由壳层回调喂入）；`canvas.rs` 加 `search_nodes`/`node_text`/`select_node_keep_focus`；`main.rs` 接 `Ctrl+F`（编辑中不抢）、菜单「查找…」、命令面板「查找主题…」、换文档时关搜索；键盘 `Esc`/`Enter`/`Shift+Enter`/`↑↓` 先于画布消费且组合期挂起；560px 白卡 + 命中字符暖黄高亮 + `拼音`/`首字母` 角标 | §4.21：全库 **268 项全绿**（shell +6）、0 新增警告、fmt 干净；golden 容差 0×2（默认界面不变）；release 截图 `shots/qa/search-{open,hits,jumped,text}.png` |
| **多标签：单画布会话对调 + 标签条 + 关闭确认**（2026-10-18） | `canvas.rs` 加 `camera`/`swap_session`（`mem::swap` 换会话并清瞬时手势态）/`set_view`（选择按 id 重映射）；`main.rs` 加 `Tab{session,camera,sel_ids}`/`tabs`/`active`、`stash_active`/`load_active`/`switch_tab`/`new_tab`/`close_tab`/`request_close_tab`/`cycle_tab`/`tab_index_of_path`、`tab_bar`（仅 ≥2 页画，`selectable_label` + `●` 脏点 + `×` + `＋`）、`tab_close_prompt`（保存并关闭/不保存关闭/取消）；纯下标函数 `next_tab`/`active_after_close`；快捷键 `Ctrl+T`/`Ctrl+W`/`Ctrl+Tab`/`Ctrl+Shift+Tab`/`Ctrl+PgUp`/`Ctrl+PgDn`/`Alt+1..9`；汉堡菜单「新建标签页」；`Open` 入口同文件去重 | §4.22：全库 **274 项全绿**（shell +6）、0 新增警告、fmt 干净；golden 容差 0×2（单标签不画标签条，界面不变）；release 截图 `shots/qa/tabs-{new,dirty,switched,close-prompt,closed}.png` |
| **导出：五种格式 + 离屏 PNG 与透明底（M4 前半）**（2026-10-18） | 新 `mw-shell/src/export.rs`（280 行，GPU 无关）：`Format` 五枚举 + `file_stem` 消毒（剥 `.zmind`/擦非法字符）、`write_text`（UTF-8）、`with_ext`、`unpremultiply`、`png_size`（8192 边 / 32M 像素双上限 + 回报生效比例）；`canvas.rs` 加 `export_png`（`Frame.viewport = None` 离屏全图、一次性合成器、图集增量回喂画布 + 全量重传）；`compositor.rs` 加 `set_transparent`（清屏与点阵底退透明、**主题令牌不动**）；`main.rs` 加 `ExportOpts` + `export_dialog`（格式 chips / 比例 1×2×3× / 文档底色·透明 / 预测尺寸 / 回车确认）+ `run_export` + `Ctrl+E`/菜单/命令面板三入口；`files.rs` 加 `save_export`。**修掉真缺陷**：透明底原先改 `canvas_bg`，而它兼着挖空色，导致标签胶囊字/关系线标签底/折叠圆点底一起消失 | §4.23：全库 **282 项全绿**（shell +7 全在 `export.rs`、render +1 透明底挖空色守恒）、0 新增警告、fmt 干净；golden 容差 0×2（离屏导出不影响上屏）；`shots/qa/out/m4-sample-1x-transparent.png` 1090×338 72950 B（76% 真透明）、`m4-sample-2x.png` 2179×676 161097 B（全不透明，字节数与修复前相同）+ 四份文本；`scripts/m4-export.ps1` 真实对话框截图 `shots/qa/m4-png{1xt,2x}-*.png` |
| **SVG / PDF 导出 + 导入全回归与 UI 入口（M4 后半）**（2026-10-18） | 新 `mw-shell/src/export_svg.rs`（678 行）：`Scene` → 真实 `<text>`/原始贝塞尔/主题令牌/可选点阵，文字位置复用 `mw_render::text_jobs`；新 `export_pdf.rs`（284 行）：单页 PDF 1.4、Flate RGB + 可选 `/SMask`、页面 = 世界单位 × 72/96 pt、固定对象布局与 20 字节 xref；`export.rs` 扩到七格式并加 `content_bounds`（并入连线/花括号/关系线/折叠圆点/角标再外扩 2 单位），`canvas.rs` 抽 `render_export` 供 PNG/PDF 共用；`files.rs` 加 `open_import`，汉堡菜单 / 命令面板 / 拖放三条入口汇到 `import_path`（开新标签页，不碰当前文档，`PendingAction::Import` 删除）；`Session.import_name` 让导入标签显示来源文件名、另存为后归还新路径，`file_stem` 剥七种源后缀 | §4.24：全库 **300 项全绿**（shell 65→83：SVG 7 / PDF 5 / 导入回归 5 等）、0 新增警告、fmt 干净；golden 容差 0×2（首跑捕到探针遗留的汉堡 hover，移开指针后两次全绿）；真实产物 SVG 9,173 B / viewBox 1093.55×342 / 16 text、PDF 106,935 B / 820.16×256.5 pt / 1 图 0 可选字符、PNG 2× 2187×684 161,555 B；导入实拍 `shots/qa/import-*.png` + 另存 `import-saved.zmind` 1,168 B；探针 `scripts/m4-import.ps1` |
| **E5：鱼骨 / 时间轴布局（M4 收尾）**（2026-10-18） | `LayoutKind` 扩到五种、`Orientation::Straight`、`Layout.spines` / `Scene.spines`（主题色 + 根级线宽）；`layout.rs` 5c 分支：鱼骨向左主轴、上下交替分支、**每侧独立槽位**（修掉首版肋线斜穿同级盒的真缺陷）、肋线 `BONE_SLANT` 夹在根盒外；时间轴向右轴 + 全局槽位推进 + 刻度；L1 先画轴线再画连线（同裁剪/同容差）、SVG `<g id="lines">` 同序导出、浮动主题轴线随动；检查器布局 chips 改 3+2；鱼骨折叠点/加子在左 | §4.25：全库 **309 项全绿**（mw-core 106→114：布局 6 + 场景 2；mw-shell 83→84：SVG 轴线 1）、0 新增警告、fmt 干净、golden 容差 0×2（默认思维导图不受影响）；release 实拍 `shots/qa/e5-fishbone-v2.png` / `e5-timeline.png` / `e5-inspector.png`，同文档即时切换 `e5-switch-{timeline,fishbone2}.png`；13 节点样例 `.zmind` 双份 |
| **工程清理与交接**（2026-10-17） | 删 `target/debug`(16 GB)、`ui/node_modules`(74 MB)、`ui/dist`、`织思.exe`(13 MB) 与会话级临时产物；`crates/mw-app` 与两个 CDP 冒烟脚本移入 `_archive/` 并从 workspace 摘除（tauri 依赖不再参与全库构建）；`.cargo/config.toml` 别名改为 `core-test`/`shell`/`golden`；README 的「开发与构建」整节按纯 Rust 栈重写，并订正 docs/12 与 README 里把路线定案误记成 ADR-008 的地方（应为 **ADR-010**）；新增入口文档 [14-项目交接说明](archive/14-项目交接说明.md) | 清理后 `cargo test --workspace` **207 项全绿**（原 210，差的 3 项属已移出的 `mw-app`）；`cargo golden` 逐像素**容差 0** 通过（1620×1080 @ ppp 1.50）；`ui/src` 与 `scripts/make-sample.mjs` 按用户决定保留 |

---

## 4.44 H3/H4：面板焦点恢复与三排色板键盘可达（2026-10-19 深夜）

**背景**：H2 把界面 chrome 字阶收敛后，剩下两处键盘可达性缺口：① 面板开合会改变 egui 自动 id，停在某一格的焦点丢一次；② v7 新增的填充 / 分支线 / 文字三排色板只有鼠标路径（v5-c 只覆盖强调色板）。

**H3 做法**：

- 新增 `crates/mw-shell/src/panels/focus.rs`：`PanelId` 分桶、`FocusKey`（`base` + 可选 `tail`）稳定身份、`FocusMemory`（`begin_frame` / `note` / `observe` / `set_open` / `restore` / `last`）。id 派生 `Id::unique(("mw-focus", base, tail))`，不掺 UI 树序号与布局位置。
- 大纲与检查器分别用 `FocusKey::dynamic`（节点 id）和 `FocusKey::indexed`（色点序号 / 行号）标记控件；`MW_TRACE` 登记语义名。
- **修掉真缺陷**：`restore` 原先在面板绘制前调用，用上一帧的 `known` 判定目标；目标本帧被文档变化删除时会请求不存在的 id，被 egui 死人开关清零。现在 `set_open` 在绘制前（只识别「由关到开」帧），`restore` 移到两面板绘制之后，`known` 已是本帧真实键。轨迹里补 `event restore <键>`，便于区分「恰好又是它」与「真的恢复」。
- 锚点语义：大纲 `anchor()` 是虚拟键（不在 `known`，用来验证「不请求焦点也不回首项」）；检查器 `anchor()` 为分支线首格。
- 5 条单测覆盖：同键恢复 / 兄弟回落 / 目标消失不回首项 / 锚点兜底 / 增删其他控件不影响身份。

**H4 做法**：

- `inspector.rs` 的 `swatches` 引入 `SwatchRow { current, fallback, tip }`，并接收 `row: &FocusKey`；每格 `ui.interact` + `Sense::click()`。egui 0.37 的 `Sense::click()` 自带 FOCUSABLE 与 Space/Enter 激活，因此不另开键盘激活路径，键鼠必然产出同一条 `Command`。
- 左右方向键在同排色点间移动焦点；自绘焦点环半径 9.8（选中环外缘 9.0 之外）。三排色板、形状、B/I、页签、布局全部接入 `FocusKey`。
- 2 条组件测试：鼠标与 Space 返回同一色；改色后一次撤销回原色。

**验证**：

- `cargo test --workspace` **419 passed / 0 failed**（412 → 419：shell 161 → 168，+5 focus +2 色板）。
- `cargo fmt --all -- --check` 干净；`cargo clippy --workspace --all-targets` 40 处 / 36 条，新增 0。
- `cargo golden --tol 0`：**0 差异**（1620×1080 @ ppp 1.50）。焦点环只在 `has_focus()` 时绘制，静态基线不出现，故不重拍。
- release 重建：`mw-shell.exe` 20,343,808 B。

**未做 / 下一步**：H3/H4 的真实输入探针（`MW_TRACE` 轨迹 + `shots/qa/` 截图）尚未编写和运行；因此这两项目前只有无窗口单测证据，文档不得写成「真实输入 PASS」。下一位接手者应先补探针，再完成 H5 的文档终检。

---

## 4.45 v8 画布排版与锥形分支线：用 XMind 2026 的官方主题令牌定标（2026-10-19 下午）

**背景**：v6 的字阶（32/18/16/14）来自早前对 XMind 截图的目测记录，没有可核对的令牌。本机装有 XMind 26.5.1107，安装包 `resources/app.asar` 里有 `static/styles/themes/`——**44 套官方主题、7 种模板**，每套都是完整的属性表（`centralTopic` / `mainTopic` / `subTopic` / `relationship` / `boundary` / `map`）。默认主题 `M01.json`（Snowbrush）给出：

| 角色 | 令牌 |
| --- | --- |
| centralTopic | `fo:font-size 28pt`、`font-weight 600`、`line-width 4`、`border-line-width 0` |
| mainTopic | `fo:font-size 20pt`、`font-weight 500`、`line-width 3`、左对齐 |
| subTopic | `fo:font-size 14pt`、`font-weight 500`、`roundedRect`、`svg:fill #F8F7F7` |
| map | `line-tapered: tapered`；Colorful 等主题另有 `multi-line-colors`（六色序列） |

**定标过程**：把 XMind 窗口用 `PrintWindow`（不点击、不影响用户实例）抓成 1919×1127 @150%，再按 1.5 折成 CSS px 量：一级主题文字墨迹高 **16.7 CSS px**（≈0.835 em → 字号 20px），卡片高 **46.7px**、圆角 ≈**10px**、文字左内距 ≈**19px**。于是确认 XMind 画布上的 pt 与 CSS px 是 1:1，v6 的 32/18 是目测偏差。

**改动**

1. `LEVEL_TYPE` → `(28, true), (20, false), (15, false), (14, false)`。
2. `PAD_X` 16→18、`PAD_Y` 10→9：一级卡片高 = 20×1.45+18 ≈ 47.0px。
3. `NODE_RADIUS` 8→10。
4. `edge_width/edge_width_root` → 2.0/4.0；新增 `Theme::edge_widths(child_depth)` 返回 `(w_start, w_end)`：根→一级 4→3，一级→二级 3→2，二级以下 2→1.5。
5. `SceneEdge` 新增 `width_end`；`mw-render::push_curve` 按**弧长**插值（曲线细分后段长不等，按段序号会让锥度在弯道突然变陡），圆头胶囊相邻覆盖，接缝 <0.2px 不可见。
6. 复原 `.cargo/config.toml`：`cargo shell` / `cargo golden` / `cargo core-test`；`golden` 别名必须是 `run -p mw-shell --release -- --golden`（程序收的是 `--golden` 开关，不是子命令）。

**验证**

- `cargo test --workspace` **419 passed / 0 failed**（数量不变；更新 `edges_are_colored_by_the_child_branch`、`wide_text_is_wrapped_at_the_configured_limit`、`every_theme_has_the_declared_shape` 的期望）。
- `cargo fmt --all -- --check` 干净；`cargo clippy --workspace --all-targets` 存量不变，新增 0。
- golden 按外观变更协议先量化再重拍：壳层 **57,458/1,749,600 px（3.284%），最大差 232**，包围盒 x[446..1589] y[409..1069]；画布 **12,245/540,000 px（2.268%），最大差 224**。重拍后 `cargo golden --tol 0` 连跑两次 **0 差异**。
- release 增量构建 1 分 25 秒，`mw-shell.exe` **20,345,344 B**；实拍 `shots/qa/v8-canvas.png`。

**未做**：颜色未动（仍是四套自有主题，没有搬 `multi-line-colors`）；阴影/发光仍未实现；字重仍是 400/600 两档，没有 500。

---

## 4.46 v8.1 雪枝主题：把「一级主题没有卡片」做进样式引擎（2026-10-19 下午）

**背景**：读完 `M01.json` 才发现 XMind 默认主题的关键不在配色：`mainTopic` 只写了 `font-size 20pt / font-weight 500 / line-width 3 / text-align left`，**既没有 `svg:fill` 也没有 `fo:color`**。也就是说一级主题在 XMind 里是「一条从 4 收到 3 的彩线 + 一行同色字」，卡片从二级（`subTopic`，`#F8F7F7` + `#333333`）才开始。我们的样式引擎从 v1 起假设「每层都是一张卡」，所以先补能力再加主题。

**能力（`mw-core::theme`）**

| 字段 | 语义 | 级联位置 |
| --- | --- | --- |
| `Theme::branch_text` | 一级主题文字取分支色；节点显式 `text_color` 与「应用到子树」仍然优先 | `resolve_all` 里 `depth == 1 && own_text.is_none() && inherited_text.is_none()` |
| `Theme::main_card` | `false`：一级主题填充为层级默认（雪枝是透明），显式 `fill` 覆盖仍生效；同时关闭「二三级跟分支色 7% 染色」，让二级保持中性 `#F8F7F7` | `resolve_all` 的 `level_fill` |

两个字段的默认值是 `false / true`，四套旧主题原样不动，所以 golden 基线不变。

**雪枝主题 `snowbrush`**：中心 `#F6212D` 圆角卡 + `#F8F7F7` 白字 28px；一级透明 + 分支色 20px；二级 `#F8F7F7` + `#333333` 15px；画布纯白、无点阵；分支色用官方 `M05 Colorful` 的 `multi-line-colors` 六色（`#FD6D5A #FEB40B #6DC354 #518CD8 #443295 #994487`，`M01` 自身不写调色板）。同时进 `themes()`、`get_theme`、`THEME_IDS` 与命令面板（「主题：雪枝 / 浅色」），检查器的主题网格按 `chunks(2)` 自动多出一行。

**验证**

- `cargo test --workspace` **421 passed / 0 failed**（+2：`snowbrush_is_the_xmind_default_look`、`a_plain_level_one_topic_takes_the_branch_color_and_no_card`）。
- 为通过 OKLCH 往返测试，把 8 bit 容差从「所有色 ±1」放宽为「彩度 ≥0.15 时 ±2」：`#FEB40B` 坐在 sRGB 边界上，往返只差蓝通道 2，再收一格就偏离官方色板（`M05` 的黄色是不可改的）。
- `cargo fmt --all -- --check` 干净；`cargo clippy --workspace --all-targets` **40 处**（与基线一致，新增 0）。
- `cargo golden --tol 0`：**0 差异**（默认主题仍是素白，基线不动）。
- 端到端实拍：临时把 `shots/qa/e5-fishbone.zmind` 的 `theme` 改成 `snowbrush` 后用 `MW_OPEN` 打开，`PrintWindow` 取 1620×1080 @150%，像素统计——中心卡 `#F6212D` **6,292 px**、二级卡 `#F8F7F7` **12,041 px**、一级彩色像素只有文字量级（约 700 px/条）且高度 46.7 CSS px 处**没有整块填充**。证据 `shots/qa/v8-snowbrush.png`。

---

## 4.47 v9 编辑入口：概要 / 边界 / 自由主题（2026-10-19 下午）

**缺口**：`Command::AddSummary / AddBoundary / AddFloatingTopic / DetachNode` 与画布绘制（花括号、`BOUNDARY_RADIUS` 圆角框、浮动子树）早就存在，但壳层没有创建入口，文档里记的是「创建入口不全」。

**做法**

1. `panels::context::Action` 增加 `AddSummary` / `AddBoundary` / `Detach` / `AddFloating`；`main.rs` 三类菜单各加行：单选节点 3 条、多选 2 条、空白 1 条。
2. 画布新增 7 个方法（`can_annotate_selection` / `add_summary_action` / `add_boundary_action` / `can_detach_primary` / `detach_primary_action` / `add_floating_world` / `add_floating_at`），全部经 `apply_structural` → 一步撤销。
3. 落点规则抽成纯函数 `annotation_targets`：容器 = 主选的父节点；目标 = **同一父级**的选中项，按文档兄弟顺序；一个都不剩返回 `None`（菜单置灰）。跨父级选中被丢弃而不是让 `child_targets` 校验整体失败。
4. 双击空白新建自由主题：`Canvas` 新增 `last_rect`（上一帧画布矩形），把窗口坐标减原点后经 `Camera::screen_to_world` 换成世界坐标再发 `AddFloatingTopic`。

**验证**

- `cargo test --workspace` **423 passed**（shell 170，+2 单测：同一父级按兄弟顺序、根不可加注解）。
- 真实输入：`CopyFromScreen` 抓 1620×1080；双击画布空白 (600,700) 后同一位置出现新圆角节点并带选中环（`#3367d1` 占 1.97%）。证据 `shots/qa/v9-float-{before,after}.png`。
- **探针纪律更新**：`PrintWindow` 对 wgpu 画布返回**旧帧**（chrome 会更新、节点与选中环不更新），真实输入验收必须用 `CopyFromScreen`；这一条已写进 `docs/11 §4.6` 与 `scripts/README.md` 待同步。

**未做**：菜单项没有快捷键；拖到空白分离仍为 no-op；新建的自由主题不自动进编辑态。

---

## 4.48 v9.1 拖到空白 = 分离为自由主题（2026-10-19 傍晚）

**背景**：`docs/06 §5` 的终态一直写着「拖到空白分离为自由主题」，而 `commit_move` 在落点为空白时直接 `return false`。顺带发现：**自由主题根本不能用拖拽挪位置**——`MoveNode` 要求有父级，`SetNodePos` 又没人发。

**改动**

| 位置 | 内容 |
| --- | --- |
| `detach_commands(doc, dropped, dx, dy)` | 纯函数：树内节点 → `DetachNode`（落到拖到的位置）；自由主题 → `SetNodePos`；主根跳过；多条合成 `Batch` |
| `detach_on_blank_command(..., pointed_inside, ...)` | 过一道「指针还在画布里」的闸：拖到面板上松手是放弃，不改结构 |
| `Moving.inside` | 拖动过程中每帧记指针是否在画布内；松手那一帧再读 `pointer.latest_pos()` 会读到已被清掉的状态 |
| `commit_move(m, pointed_inside)` | 空白落点走 detach 分支；提交后保持选中并请求画布焦点 |

**验证**

- `cargo test --workspace` **424 passed**（+1：四种情况——树内分离、自由主题挪位、主根不参与、指针在画布外放弃）。
- `cargo fmt --all -- --check` 干净；clippy **40 处**（新增 0）；golden `--tol 0` ×2 **0 差异**；release 20,365,312 B。
- **真实输入 PASS（同日第二次尝试）**：`mouse_event` 的相对移动在按住期间被吞掉，换成 `SendInput` 的绝对坐标（`MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK`，dx/dy 归一化到 65535）后一次通过：
  - 输入序列：按下 `#8fd0bc` 一级卡片 (273,563) → 20 步移动到空白 (520,880) → 松手 → `Ctrl+S`；
  - `content.json`：`root.children` `[a,b,c]` → `[b,c]`，新增 1 个 `parentId: null` 的自由主题（a），`pos {x:-232.67, y:346.16}`，`children [a1]` 原样保留；
  - 截图 `shots/qa/v9.1-drag-detach.png`：落点处是带 `#2f7cf6` 选中环的绿卡。
- 复现脚本 `scripts/win-drag.ps1`（SendInput 绝对坐标拖拽，ASCII-only）。

---

## 4.49 v9.2 主题库扩建：经典（M03）与夜幕（M14）（2026-10-19）

**选择标准**：不为了「多两套配色」而加，而是每套主题都必须逼出引擎的一个新能力。

| 主题 | 官方令牌 | 新能力 |
| --- | --- | --- |
| 经典 `M03 Classic II` | 中心白卡无描边、一级 `shape-class: underline` + `border-line-width 2`、二级起无填充、画布 `#F7F7FA`、`line-tapered: tapered`、六色 `multi-line-colors` | `Shape::Underline` |
| 夜幕 `M14 Dark` | 画布 `#000000`、中心 `#1B1B1D`、一级 `#3A3A3C`、二级 `#636366`、白字、`line-width` 3/2、不写 `line-tapered` | `Theme::taper = false`（等粗线） |

**实现要点**

1. `Shape::Underline` 在四个地方都要到位，少一处就会「画布上下划线、导出还是卡片」：`instances.rs`（盒子底边压一条 `SegInst`，粗细 = `stroke_width`）、`export_svg.rs`（`<line>`）、`canvas.rs`（编辑态环退化成直角）、`inspector.rs`（形状行第四项）。
2. `Theme::taper` 把 `edge_widths` 从「按 child_depth 查表」改成「父端 `edge_width_at(d-1)`、子端 `edge_width_at(d)`」，不锥形时两端取父端值；`edge_width_at` 是 4 / 3 / 2 / 1.5 的唯一来源。
3. 夜幕的 `ui.accent` 必须是**有彩色**（取 `#5AA9E6`）。M14 自己没有强调色定义，而「只借色相」的覆盖逻辑在无彩色上会退化成恒等，既有单测 `the_override_lands_on_the_hue_the_user_picked` 会直接抓出来——这条约束值得记住。

**验证**

- `cargo test --workspace` **427 passed**；`cargo fmt --all -- --check` 干净；clippy 40 处（新增 0）；golden `--tol 0` ×2 **0 差异**；release **20,368,896 B**。
- 端到端像素（`CopyFromScreen` 1620×1080，先 `cargo build --release` 再开——第一次忘了重建，旧二进制不认识 `night` 于是回退素白，白跑一轮）：
  - 夜幕：`#000000` 63.85%、`#1B1B1D` 18,891 px、`#3A3A3C` 18,861 px、`#636366` 164 px；
  - 经典：`#F7F7FA` 66.55%、白色中心卡、以及 y=586/587 两条 2px 高、跨度 338px 与 98px 的水平线——下划线确实是一条线而不是卡片。
- 证据 `shots/qa/v9.2-night.png`、`shots/qa/v9.2-classic.png`。

---

## 4.50 H3/H4 真实输入补测：焦点轨迹代替肉眼（2026-10-19 傍晚）

**背景**：H3（面板开合后焦点不丢）与 H4（三排色板键盘可达）从落地起只有无窗口单测，`docs/14` 的交接清单一直把它们标成「代码 + 单测，真实输入未验」。补测卡了两轮，原因不在产品：`mouse_event` 的相对移动被吞、`PrintWindow` 拍 wgpu 画布拿到旧帧。这一轮把三件工具凑齐了——`AttachThreadInput` 抢前台、`ShowWindow(SW_RESTORE)` 唤醒窗口、`MW_TRACE` 焦点轨迹。

**探针**：`scripts/v10-focus.py`（Python：SendInput 绝对坐标 + 轨迹解析 + ZIP 内 `content.json` 断言）。退出码 0=PASS / 2=环境 / 3=产品回归。

**H3 轨迹**（关键三行）：

```
focus inspector/layout/kind mindmap      ← 第 49 次 Tab 到达
focus none                               ← F9 关掉检查器，焦点被清
event restore inspector/layout/kind mindmap
focus inspector/layout/kind mindmap      ← F9 再开，回到同一格
```

**H4**：点中一级分支卡 → F9 → F6 → Tab 到 `inspector/style/branch/swatch 1` → `→` → `Space` → `Ctrl+S`；`content.json` 里节点 a 出现 `"branchColor": "#4e9e6b"`；`Ctrl+Z` + `Ctrl+S` 后 `style` 整键消失。

**结论**：H3/H4 从「代码 + 单测」升级为「代码 + 单测 + 真实输入 PASS」，`HANDOFF.md §10` 的 DoD 勾选完成。证据 `shots/qa/h3h4/{h3,h4}-focus-trace.txt`，文档回填 `docs/11 §4.9`、`HANDOFF.md §0/§2.2/§4/§5.2/§10`、`scripts/README.md`。

**未做**：探针依赖本机 Python，不是 CI 项；`F6` 之后 Tab 环会先经过浮动工具条再进面板（49 次才到检查器），这个顺序本身没有断言，只是被记录下来。

---

## 4.51 v10 Ctrl+拖拽复制（2026-10-19）

**缺口**：XMind 按住 Ctrl 拖主题是复制；我们只有 `Ctrl+D`（原地复制同级），拖拽一律是移动。

**实现**

1. `Drag.copy` / `Moving.copy`：**按下那一刻**取 `modifiers.command` 并存进拖拽状态。拖动途中松开 Ctrl 不改判——否则拖到一半松手会变成移动，用户无法预期。
2. `copy_into_command(doc, roots, targets)`：每个拖拽根 `copy_subtrees` → `remap_subtrees` → `InsertSubtree`；同一父级多次插入时下标按已插入条数补偿（与 `duplicate_command` 同一条理由）。
3. 落点复用 `plan_moves` 的输出（`roots` 与 `moves` 一一对应），「插到哪个父、第几位」和移动用同一套几何判断。
4. 复制后选中副本（`remap_subtrees` 返回的新 id），方便接着摆放。
5. F1 帮助面板新增「鼠标」一节：拖拽 / Ctrl+拖拽 / 拖到空白 / 双击空白 / 双击主题——这四类手势此前只散落在文档里。

**验证**

- `cargo test --workspace` **428 passed**（+1）；fmt 干净；clippy 40 处（新增 0）；golden `--tol 0` ×2 **0 差异**（帮助面板不在 golden 帧里）。
- 真实输入：按住 Ctrl 把「甲分支」从 (751,564) 拖到「乙分支」中心 (1441,564)，松手后 `Ctrl+S`：
  - 节点 4 → **6**；`root.children` 仍是 `["a","b"]`；
  - `b.children` 多出副本，副本还带着自己的子节点。
- 截图 `shots/qa/v10-ctrl-drag-copy.png`。

**未做**：Ctrl+拖到空白（复制成自由主题）没有对应命令；多选 Ctrl+拖拽只有代码路径，未单独做真实验证。

---

## 4.52 v10.1 Ctrl+拖到空白：复制成自由主题（2026-10-19）

**做法**：不为它新增命令。`copy_floating_command` 先把副本 `InsertSubtree` 挂到主根下，再对副本根发 `DetachNode { x, y }` 把它摘下来并给坐标，两条包成一条 `Batch`：

- 一条撤销就能全部回退（`Batch` 在 `Document::apply` 里本来就是单条 changeset）；
- `InsertSubtree` 自带的剪枝与校验（关系线只留可解析目标、边界/概要收敛到仍在子列表里的目标、标签去重限量）继续生效，不必为「浮动子树」再写一份；
- 自由主题的 `pos` 由 `DetachNode` 写，和「拖到空白分离」走同一条语义。

**验证**

- `cargo test --workspace` **429 passed**（+1：`ctrl_drag_to_blank_copies_as_a_floating_topic`，覆盖副本为自由主题、坐标 = 原位置 + 拖拽位移、子树跟随、原件留位、一次撤销）。
- 真实输入（SendInput）：Ctrl+拖「甲分支」到空白 (560,900) → 节点 4→6；`root.children` 仍是 `["a","b"]`；`content.json` 多一个 `parentId: null` 的副本，`pos {x:-393.93, y:205.07}`，`children` 带着 `甲的子节点`。
- 像素：落点附近 6,098 个非背景像素；绿色填充 bbox 延伸到 y=943（原卡片在 531–598）。证据 `shots/qa/v10-ctrl-drag-blank.png`。

---

## 4.53 v11 导出范围：整张导图 / 选中子树（2026-10-19）

**做法**：范围只在「有画面」的格式上出现（PNG/SVG/PDF），文本四种照旧整份导出并在对话框里写明。`Canvas::export_scene(selection)` 以主选为 `layout_root` 调一次 `build_scene`——布局与主题直接取 `doc.meta`，**不传行内编辑快照**，于是「导出的一定是已提交的文档」这条不变量继续成立。PNG/PDF 把整份场景交给 `export_job`（包围盒由 `export::content_bounds` 按子树算），SVG 走 `export_svg::render`，两条路共用同一份场景。

**验证**

- `cargo test --workspace` **430 passed**（+1）。
- 端到端用既有 `MW_EXPORT_BENCH` 探针扩展了 `MW_EXPORT_SCOPE=selection`（没有选中时自动选中根的第一个孩子，再走产品路径导出）：
  - 整张导图 `nodes=5` → **1566×308 / 46,938 B**；
  - 选中子树 `nodes=2` → **646×166 / 11,358 B**。
  节点数与像素同时缩小，能排除「整图裁一刀」这种假实现。
- 产物 `shots/qa/v11-export-{whole,selection}.png`。

**未做**：可视区域范围（要把包围盒裁到相机视口）；文本格式的当前子树导出（需要临时子文档）。

---

## 4.54 v11.1 导出范围铺到文本格式（2026-10-19）

**想法**：四个文本序列化器（Markdown / 大纲 / OPML / FreeMind）只认 `Document`。给每个格式各加一个「只导子树」变体是最差的做法；正确做法是**先造一份子树文档**，序列化器一行不改。

`subtree_document(doc, root)` 做三件清理，少一件 `Document::validate` 就会拒：

1. 根的 `parent_id` 清空（在新文档里它就是主根）；
2. 所有节点 `pos` 清空（只有自由主题能带坐标，这份文档里没有自由主题）；
3. 指向子树外部的关系线剪掉（外部目标在新文档里不存在）。

对话框的「范围」行从「只对有画面的格式可见」改成**七种格式都可见**，底部提示按格式与范围分别措辞。

**验证**：`cargo test --workspace` **431 passed**（+1：摘出的子树文档 `validate()` 通过、节点数正确、外部关系被剪、`meta.title` 取根文字，并且 `Format::Markdown.text(&sub)` 含「甲」不含「乙」）。这条只有单测证据——`MW_EXPORT_BENCH` 的探针只覆盖 PNG/PDF。

---

## 4.55 v12 快捷键配置（2026-10-19）

**结构**：`shortcuts.rs` 三件套——`Action`（22 个窗口级命令）、`Chord`（键 + 三个修饰位，显示名与配置名同构）、`Bindings`（默认表 → 文件覆盖）。主链仍按原顺序判断，只是把 `consume_key` 换成 `Bindings::consume(ctx, action)`；默认表与原代码逐条对齐，包括 `Ctrl+Y`（重做备用）与 `Ctrl+PgUp/PgDn`（切标签备用）。

**两条设计约束**

1. **冲突在绑定时拒绝**，不在匹配时抢。重复和弦返回占用者，面板提示、不落盘；这样匹配顺序继续依赖「更具体的先判」，不会出现「两个动作抢同一个键」的隐式行为。
2. **捕获态短路整条链**。否则把 `Ctrl+S` 改成别的键的那一帧，会先执行一次保存——改键本身就是一次按键。

**踩到的坑**：PowerShell 5.1 的 `Set-Content -Encoding UTF8` 会写 **BOM**，`serde_json` 见 BOM 直接报错，整份配置静默失效（探针里 F10 明明写了却不生效，查了两轮）。`load_from` 现在先 `trim_start_matches('\u{feff}')`，并补了 BOM 测试。

**验证**：`cargo test --workspace` **435 passed**（+4）；真实输入 F9→0px / F10→100,853px / Ctrl+Comma→130,900px、Esc 后恢复；证据 `shots/qa/v12-*`；fmt 干净、clippy 新增 0、golden 0 差异。

---

## 4.56 v12.1 快捷键提示跟随改键（2026-10-19）

v12 只让「按键」可配，界面上印的仍是出厂值——用户把检查器改成 F10，菜单还写着 F9。这一轮把提示接上：

- `Bindings::hint(action)` 返回当前和弦文本（没绑定时空串）。
- 汉堡菜单：**在闭包外**先把 9 条提示算成 `String`，闭包只借用它们——否则 `self.shortcuts` 会和闭包里读的 `self.recent/show_outline` 抢借用。
- 命令面板：`shortcut_of(Action)` 把面板动作映射到窗口级动作，行渲染时才取 hint；主题 / 导入这类没有快捷键的行继续用表里的说明。

单测 +2（提示跟随、面板行映射）；`cargo test --workspace` **437 passed**；fmt 干净、clippy 新增 0、golden 0 差异。提示是文字，像素探针断言不了内容，所以证据是单测 + 编译期接线。

---

## 4.57 v11.2 可视区域导出（2026-10-19）

导出范围的第三档：**只导出相机当前看到的范围**。实现上没有新管线——`export_job::Request` 与 `export_svg::Options` 各加一个 `bounds: Option<Rect>`：

- 位图 / PDF：`render()` 里 `bounds.unwrap_or_else(|| content_bounds(scene))` 决定 `png_size` 与相机中心；
- SVG：同一个 bounds 直接当 `viewBox`，视口外内容由查看器按 viewBox 裁剪；
- 文本四种格式不显示这个选项——没有视口概念，选了也没有意义。

取景框来自 `Canvas::view_bounds()`：视口像素 ÷ 缩放，得到世界坐标矩形。探针侧加了 `MW_EXPORT_ZOOM`，否则首帧 fit 之后视口几乎等于整图，量不出差别。

**证据**（h4 文档、scale 2）：

| 范围 | 尺寸 | 字节 |
| --- | --- | --- |
| 整张导图 | 1566×308 | 46,938 |
| 视口 @ 2× | 898×677 | 41,879 |
| 视口 @ 4× | 449×339 | 21,203 |

4× 的高度是 2× 的一半、宽度也近似一半——取景框确实跟着相机缩放，而不是把整图裁一刀。

---

## 4.58 v13 大纲面板行内改名（2026-10-19）

大纲一直是只读的：结构、折叠、跳转都有，就是不能改字。这一轮补上，入口两个——**双击行**，或**行有键盘焦点时按 F2**（与画布 F2 编辑文字同义）。

实现要点：

1. `Outline` 加 `editing: Option<Edit>`，`show` 从 `&self` 变 `&mut self`；主循环里先把 `editing` `take()` 到局部、结束后放回——`show_rows` 的闭包已经借了 `&self.rows`，不能同时借 `&mut self`。
2. 编辑态在行内盖一个无边框 `TextEdit`，第一帧 `request_focus()`；回车或失焦提交，Esc 取消。
3. 提交走 `Command::SetText`——与画布行内编辑同一条命令，撤销栈里同样只有一步。
4. 编辑期间这一行的 `clicked` 不再改成「选中」，否则点一下输入框就把选择挪走了。

**证据**：F6 → 11 次 Tab 落到 `outline/row r` → F2 → Ctrl+A → Ctrl+V（"RENAMED"）→ 回车 → Ctrl+S，`content.json` 里根节点文字确认为 "RENAMED"；轨迹 `shots/qa/h3h4/v13-outline-trace.txt`、截图 `shots/qa/v13-outline-renamed.png`。`cargo test --workspace` **438 passed**。

---

## 4.59 v14 主题 9 套：派对（M09）与微光（M06）（2026-10-19）

两套都不用改引擎能力，靠现成的 `main_card` / `taper` / `Shape` 组合出来：

- **派对**：画布 `#FAD8DF`、中心 `#318DE1` 实色卡、一级为主题定义里的 `ellipticrectangle`（映射成 `Shape::Capsule`）白胶囊、二级起无填充；调色板保留一组高饱和色。
- **微光**：四层全部 `TRANSPARENT` 填充、`#333333` 文字、`main_card = false`，于是整张图只剩文字与分支线——XMind「Light」的极简骨架。

证据：派对画布 `#FAD8DF` 占 63.67%、`#318DE1` 命中；微光白色画布 90.33%、文字 `#333333` 命中、分支线 `#859EC5`。`cargo test --workspace` **440 passed**（+2），fmt/clippy/golden 全绿。

---

## 4.60 v15 大纲拖拽重排（2026-10-19）

行的 `Sense` 改成 `click_and_drag`；`drag_started` 记下 id，松手时按指针落点找目标行，按行内相对高度分三档（上 30% 前、下 30% 后、中间子节点）。落点语义抽成 `move_target(row, zone)` 纯函数——根没有同级，前/后都回落成「挂成子节点」；「不能拖进自己子树」由 `is_self_or_descendant` 沿行表 parent 链判断，不额外借文档。

松手发 `Action::Move` → `Command::MoveNode`，与画布拖拽同一条命令、一步撤销。拖动期间画 2px 强调色横线（前/后）或高亮目标行（子节点）。

**证据**：真实输入把「甲的叶子」拖到「乙分支」中部，`content.json` 里 `a1.parentId == "b"`、`b.children == ["a1"]`。`cargo test --workspace` **442 passed**（+2）。

---

## 4.61 v16 搜索结果带节点路径（2026-10-19）

搜索只给标题，同名主题分不清。这一轮把路径补上：

- `path_of(doc, id)`：沿 parent 链到主根、用「›」连接、**不含自己**；空标题祖先用「（未命名）」占位；超过 42 字符从左侧截断（保留离自己最近的父级）。
- `HitRow.path` 由壳层填；结果行 32 → 44px，两行式（标题 + 路径），顶层命中没有路径、标题回落到垂直居中。

**验证**：`cargo test --workspace` **443 passed**（+1 覆盖不含自己/空祖先/长路径截断）；真实输入 `Ctrl+F` + 粘贴「分」后截图里结果行成对出现标题与淡色路径（行距 44、两行相距 17 CSS px）。

---

## 4.62 v17 主题微调：文档级自定义配色（2026-10-19）

三个槽位（画布底色 / 中心卡 / 一级卡）住在 `DocMeta.theme_overrides` 里，于是「随文档保存」「撤销/重做跟着 meta 走」两件事自动成立，不需要全局主题库那套解析改造。

两个坑：

1. **一级卡有两个来源**：`levels[1].fill`（无卡片主题）与 `palette_fill`（有卡片主题按分支取色）。只改一个，就会在另一类主题上「换了色但没反应」。
2. **坏颜色要拒绝而不是静默忽略**：否则界面显示已设置、画布却纹丝不动，比报错难查得多。

检查器文档页加「配色微调」三行（每行 6 色点 + 恢复），当前值用强调色圆环标出。真实输入：点「画布」的 `#fad8df` 后粉色像素 270 → 732,952，`Ctrl+S` 落盘 `themeOverrides`。`cargo test --workspace` **446 passed**。

---

## 4.63 v18 我的主题：命名预设（2026-10-19）

v17 的微调只活在当前文档里。这一轮把它变成可复用的预设：

- `Preset { name, theme, overrides, accent }` 存 `%APPDATA%\织思\theme-presets.json`。**只存组合、不存整套 Theme**：内置主题以后改配色，预设自动跟着变。
- 套用抽成纯函数 `apply_command`，产出 `SetTheme + SetThemeOverrides + SetThemeAccent` 的 `Batch`——一次撤销回到底。
- 检查器主题网格下方渲染预设卡（2 列 112×40），底色与两个色点用「基础主题 + 覆盖」实时算；右上角 × 删除。

**验证**：`cargo test --workspace` **450 passed**（+4）。v18.1 给预设控件登记语义名（`preset N` / `preset-save` / `preset-del N`），于是 `MW_TRACE` 能直接定位卡片：F9 → F6 → 29 次 Tab 到 `focus preset 0` → 空格 → `Ctrl+S`，文档变成该预设（snow + `canvasBg #fad8df`）。**仍未验**：保存按钮的空格激活（Tab 环太深）。

---

## 4.64 v19 概要/边界的查看与删除（2026-10-19）

边界与概要此前只能加不能删，是个单向门。这一轮把它打开：

- `Document::annotations_covering(id)`：只扫**直接父级**上的 summaries / boundaries，返回 `AnnotationRef { parent, kind, id, label }`；概要在前、边界在后。
- 检查器「标记」页底部列出这些注解，每条一个 ×，发既有的 `DeleteSummary` / `DeleteBoundary`——一步撤销。
- 每个 × 有语义名 `annot-del N`，所以能像预设卡片那样用 `MW_TRACE` 定位。

**真实输入**：点选被概要盖住的子节点 → F9 → F6 → Tab 到 `inspector/tab/mark` → 空格 → Tab 到 `annot-del 0` → 空格 → `Ctrl+S`，`content.json` 里 `summaries` 变成 `[]`。`cargo test --workspace` **451 passed**。

---

## 4.65 v20 概要文字可改（2026-10-19）

v19 让概要可删，但改一个字仍要删了重建。这一轮补上行内编辑：

- 概要行加 ✎（`annot-edit N`），点开是 `summary-field` 输入框；Enter 提交、Esc 取消、失焦提交。
- 草稿只活在 `Inspector::summary_edit` 里，**值真的变了**才发 `SetSummaryText`——否则点一下就会在撤销栈里留一条空改动。

**真实输入**：选节点 → 标记页 → Tab 到 `annot-edit 0` → 空格 → Ctrl+A → 粘贴 → 回车 → Ctrl+S，`summaries[0].text` 由「结论」变成「NEWTEXT」。`cargo test --workspace` **451 passed**（命令侧已有覆盖，本轮只动面板状态机）。

---

## 4.66 v21 注解归属 + 边界颜色（2026-10-19）

两件事一起做：

1. **归属**：`annotations_owned(id)` 与 `annotations_covering(id)` 互补，`AnnotationRef` 加了 `owned` 与 `color`。检查器把「我持有的」和「盖住我的」都列出来——以前选中父节点时看不到自己刚加的概要，很反直觉。
2. **边界颜色**：新命令 `SetBoundaryColor`，校验 hex、缺失 id 报错、父节点快照一步撤销；边界行给 5 个色点（默认 + 红蓝绿橙），语义名 `boundary-color <注解> <色>`。

**真实输入**：Tab 到 `boundary-color 1 2` 空格 → `Ctrl+S`，`boundaries[0].color` 变成 `#318de1`。`cargo test --workspace` **452 passed**。

---

## 4.67 v22 画布上选中注解并删除（2026-10-19）

注解此前只能从检查器操作。这一轮把它做成画布上的一等选择对象：

- `annotation_hit` 抽成纯函数：先排除「点在节点盒里」，再判概要摘要盒（外扩 4）与边界矩形——所以点卡片永远不会被边界抢走。
- 点击优先级 **折叠箭头 → 注解 → 节点 → 空白**：箭头在节点盒外，排在注解前面才不会让边界里的折叠点失灵。
- 选中环按屏幕坐标画 2px 强调色；`Delete` 选中注解时删注解，否则删节点。

**真实输入**：点边界内空白 → `event select boundary b1` → Delete → `event delete 边界 b1` → Ctrl+S，`boundaries` 变 `[]`。

**踩到的坑**：探针里用 `startswith("select ")` 找事件，而 trace 写的是 `event select …`——一度以为点击没生效，其实是匹配写错。纯函数单测先证明了逻辑正确，才把怀疑转到探针。

---

## 4.68 v23 选中注解时检查器联动（2026-10-19）

v22 能在画布上选中边界，但检查器不为所动——等于选了个寂寞。这一轮把它接上：

- 画布暴露 `annotation_selection()`；检查器新增注解页（边界：5 个色点；概要：文字框），底部「删除」「取消选择」。
- `Inspector::show` 参数已经到 7 个上限，clippy 报 `too_many_arguments 8/7`。把「预设 + 注解选择」打成 `Extras` 结构体，参数回到 7，调用点也更短。

**真实输入**：点选边界 → F9 → F6 → Tab 到 `annotation-color 2` → 空格 → Ctrl+S，`boundaries[0].color` 变 `#318de1`。`cargo test --workspace` **453 passed**。

---

## 4.69 v24 查找替换（2026-10-19）

- `replace_all` 落在 core：字面全替换 + 计数，空查询不动。
- 搜索条加 `Ctrl+H` 替换行；提交用 **Ctrl+Enter**（裸回车早被搜索条的 `handle_keys` 吃掉，回车 = 跳下一条）。
- 壳层把命中节点拼成一条 `Batch`，一次撤销全回去；拼音命中没有可替换位置，按 0 跳过。

**探针的教训**：键盘全链路（Ctrl+F → Ctrl+H → 输入 → Ctrl+Enter）在搜索条里不稳定，Tab 环两次跑出两种结果。于是加了 `MW_REPLACE="旧=>新"` 启动钩子，跑同一条链路并打印 `replace_probe query=… replacements=3 nodes=3 applied=true`——**逻辑用钩子做确定性验证，UI 只保留截图**，文档里也照实写明这一点。

---

## 4.70 v25 主题微调扩到文字色与线宽（2026-10-19）

- `ThemeOverrides` 增加 `text` 与 `edge_scale`（钳 0.4–2.5）。
- `with_overrides`：文字色覆盖四层；线宽倍率**同时**乘 `edge_width` 与 `edge_width_root`——只乘一个会让根分支与子分支的粗细比例失衡。
- 检查器多「文字」色点行与「线宽」三档。

**像素证据**：基线绿色 0 / 深色 6476 → 加 `text:"#0a7d3f"` 后绿色 5468 / 深色 5569。`cargo test --workspace` **454 passed**。

---

## 4.71 v26 概要跨度可调（2026-10-19）

- 新命令 `SetSummaryTargets`，复用 `child_targets` 的三条校验（直接子节点 / 去重 / 非空），父节点快照一步撤销。
- 「标记」页与「注解页」各一排跨度开关（`annot-span <注解> <子序>`）。
- **空目标点击无效**：全去掉等于删概要，必须走「删除」——避免用拖开关的方式把概要清空成一个空壳。

**真实输入**：Tab 到 `annot-span 0 1` 空格 → Ctrl+S，`targets` 从 `["a"]` 变 `["a","b"]`。`cargo test --workspace` **455 passed**。

---

## 4.72 v27 边界跨度也接上（2026-10-19）

- `SetBoundaryTargets` 与 `SetSummaryTargets` 共用 `child_targets` 校验与父节点快照。
- `ann_span` 泛化成按 `AnnotationKind` 查（概要或边界），一套开关两处复用。
- 注解页边界行下面加了跨度开关（`boundary-span k`），空目标点不动。

**真实输入**：点选边界 → F9 → F6 → 23 次 Tab 到 `boundary-span 0` → 再一次到 `boundary-span 1` → 空格 → Ctrl+S，`targets` 由 `["a"]` 变 `["a","b"]`。`cargo test --workspace` **456 passed**。

---

## 4.73 v28 全量冒烟：7 格式 / 5 布局 / 9 主题（2026-10-19）

`MW_EXPORT_BENCH` 原先只认 PNG/PDF，另外五种格式在探针里根本跑不到。这一轮按后缀分流：`svg` → `export_svg::render`、`md/txt/opml/mm` → `Format::text`，都打印 `ok=`。

矩阵（同一份 5 节点文档）：7 种格式全部落盘（svg 2,629B / md 85B / txt 85B / opml 347B / mm 248B，位图与 PDF 正常）；5 种布局导出尺寸各不相同（1782×308 / 1350×450 / 984×684 / 1670×538 / 1990×538），9 套主题全部出图且字节各异。

**没固化成 CI**：钩子仍要启动进程 + 等 3 秒，属于脚本级验证。

---

## 4.74 v29 渲染矩阵固化成测试（2026-10-19）

`crates/mw-render/tests/render_matrix.rs`：不建窗口，直接用 `Compositor` 离屏合成，对 5 布局 / 9 主题各算「非背景占比、颜色数、内容哈希」，断言都画出了东西且**两两不同**。无 GPU 时跳过。

阈值要按主题下限走："微光" 整张图只有文字与线，非背景占比只有 0.23%——拿有卡片主题的 1% 去卡它必然误判。

**golden 的坑**：残留的 `mw-shell.exe` 会让 golden 截到「悬停/焦点高亮」的一帧（蓝框围节点卡，1406 px 超容差）。清干净进程后连跑四次 0 差异。**golden 之前先杀进程**。

---

## 4.75 v30 文字色按层级分开（2026-10-19）

`ThemeOverrides` 增加 `text_central` / `text_main` / `text_sub`，老字段 `text` 降级成「整体值」。优先级写成**先整体、后分层**的两段覆盖，老文档与新的分层设置互不干扰。

检查器把「文字」一行拆成三行（字·中心 / 字·分支 / 字·子级），标签列从 28px 加宽到 44px。

**像素证据**：基线 绿 0 / 蓝 0 → 分层后 绿 344（中心）、蓝 433（子级）。`cargo test --workspace` **459 passed**。

---

## 4.76 v31 线色统一微调（2026-10-19）

分支线的颜色来自主题 `palette`（每支一种），所以「统一线色」不必新加 Theme 字段——把 `palette` 刷成一个颜色即可；`palette_fill`（节点填充）保持不动，否则改线会连卡片一起变。

副作用：带 `branch_text` 的主题（雪枝）文字也跟着变，这正是单色线主题该有的样子，但没有单独开关。

**像素证据**：基线绿 0 → 设 `edgeColor:"#0a7d3f"` 后绿 788。`cargo test --workspace` **459 passed**。

---

## 4.77 v32 导出设置记忆（2026-10-19）

`export_prefs.rs` 把 `{format, scope, scale, opaque}` 记到 `%APPDATA%\织思\export.json`。键用**后缀**（png/svg/md…）与**稳定键**（whole/selection/viewport），不存 Debug 名、不存中文 label——人类可读，且认不出就回落（PNG / 整张导图 / 倍率钳 0.5–8.0）。

打开对话框（含 Ctrl+P 的 PDF 预设）时写回，`ExportOpts::default()` 时读回。

**真实输入**：清空后 `Ctrl+E`+Esc → 文件出现默认值；预写 `md/viewport/4.0/false` 再开对话框 → 文件仍是这组值（说明还原生效且没被覆盖）。`cargo test --workspace` **462 passed**。

---

## 4.78 v33 拖拽自动滚动（2026-10-19）

拖到边缘 26px 内每帧推 14px，世界位移按缩放折算（放大时不会飙车）。方向抽成 `autoscroll_step` 纯函数单测——**符号写反的话画布朝反方向跑**，而这种错误在拖动截图里几乎看不出来。

**诚实标注**：真实拖拽探针两次都是 0 条 autoscroll 事件（合成拖拽没进入节点拖动状态），所以只有逻辑单测，没有端到端证据。

---

## 4.79 v34 大纲拖拽自动滚动（2026-10-19）

贴上下缘 24px 内每帧 `scroll_with_delta` 推 12px，方向抽成 `outline_autoscroll` 单测。

**与 v33 的差别**：大纲行是普通可拖控件，合成拖拽能稳定触发——**拿到了端到端证据**（136 条 `outline_autoscroll dy=12.0`）；而画布节点拖拽的探针至今没进入拖动状态，只有逻辑单测。`cargo test --workspace` **464 passed**。

---

## 4.80 v35 全量核对与交接整理（2026-10-19）

v28 之后又改了 6 轮，先复跑矩阵确认没回退：7 种格式的字节数与 v28 **完全一致**（png 46,938 / svg 2,629 / pdf 29,254 / md 85 / txt 85 / opml 347 / mm 248）。

然后纠交接文档：`HANDOFF.md §2.2`「还没做」里那 4 条其实早就完成了（快捷键 UI 是 v12.1、边界/概要入口 v9、拖到空白 v9.1、Ctrl+拖拽 v10.1、主题管理 v18.1）。**交接文档比代码更容易过期**——每轮只 append 不回头核对，就会积累出"以为没做、其实做了"的假缺口。已按真实状态重写，只留 5 条真缺口。

