# 织思 1.0 开发手册

> 本文是唯一工程手册。产品范围看 `README.md`，进度看 `STATUS.md`。
> 代码注释里的 `docs/xx`、`shots/x100/*/report.md` 等是历史引用；1.0 以这三份文档为准。

---

## 1. 硬约束

1. Windows 10/11 x64；Rust + egui/eframe + wgpu + DirectWrite。
2. 禁止 WebView / HTML / CSS / JS / Electron / Tauri。
3. 文档修改只能走 `mw_core::Command` + 撤销栈；UI 只发 Action。
4. `.zmind` 旧文件必须可读；不破坏字段兼容。
5. 测试/截图隔离 `APPDATA`、`LOCALAPPDATA`、`TEMP`；不读用户真实数据。
6. 不提交 git，除非用户明确要求；不清理/重置未提交工作。
7. 不新增依赖/crate，除非先在本轮说明理由并得到同意。
8. 状态词只用 `未开始 / 进行中 / 已实现 / 已验证 / 阻塞 / 延后`；`已验证` 只由人工门签字。

---

## 2. 仓库与 crate

```
crates/
  mw-core/     领域模型、Command、撤销、布局、Scene、主题、文本策略、检索、概念图生成树
  mw-render/   wgpu 四层合成器、DirectWrite 排版、字形图集、离屏导出
  mw-shell/    eframe/egui 壳层、画布交互、面板、菜单、标签、文件对话框、卡片页、笔记库、起始页
  mw-ui/       手势纯逻辑（无窗口/GPU 依赖）
  mw-format/   .zmind 容器（ZIP + JSON）、原子保存、备份
  mw-io/       Markdown / OPML / FreeMind / XMind 导入与多格式导出
  mw-markdown/ 卡片笔记 Markdown 解析（零内部依赖）
  mw-golden/   壳层截图 golden harness
  mw-probe/    性能与启动探针（一次性证据）
```

依赖方向：

```
mw-shell → mw-render / mw-core / mw-format / mw-io / mw-markdown
mw-render → mw-core
mw-format / mw-io → mw-core
mw-markdown 零内部依赖
mw-core 零 UI 依赖
mw-ui → mw-core
```

关键入口：

- 文档/命令/撤销：`crates/mw-core/src/document.rs`、`command.rs`
- 布局：`crates/mw-core/src/layout.rs`
- 场景装配：`crates/mw-core/src/scene.rs`
- 主题与样式级联：`crates/mw-core/src/theme.rs`、`design.rs`
- 命中：`crates/mw-render/src/hit.rs`
- 画布交互：`crates/mw-shell/src/canvas/canvas_methods_*.rs`
- 外壳/标签/文件：`crates/mw-shell/src/main.rs`、`app_methods_*.rs`、`files.rs`
- 检查器：`crates/mw-shell/src/panels/inspector/`
- 卡片/库：`crates/mw-shell/src/note_view.rs`、`note_library/`、`note_store.rs`
- 导出：`crates/mw-render/src/export*.rs`、`crates/mw-shell/src/export_svg.rs`、`export_pdf.rs`
- 格式：`crates/mw-format/src/lib.rs`、`crates/mw-io/src/`

---

## 3. 构建、测试与门

### 3.1 本机必须

裸跑 `cargo test` 前必须把 `TEMP`/`TMP` 指到仓库内，否则写盘用例会以 `os error 5` 假红：

```bash
export TEMP="$(pwd)/target/tmp" TMP="$(pwd)/target/tmp"
mkdir -p target/tmp
```

Windows 基准机用 PowerShell：

```powershell
cargo build --release -p mw-shell
powershell -File scripts/test.ps1
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
cargo golden --tol 0
powershell -File scripts/x100-stage-b-gate.ps1
powershell -File scripts/x100-stage-cm-gate.ps1
```

### 3.2 Linux 便携验证（当前环境）

```bash
cargo test -p mw-core -p mw-format -p mw-io -p mw-markdown -p mw-ui -- --test-threads=1
cargo fmt --all -- --check
```

Windows 交叉类型检查（无 wgpu/DirectWrite 运行）：

```bash
rustup target add x86_64-pc-windows-gnu
sudo apt-get install -y gcc-mingw-w64-x86-64 g++-mingw-w64-x86-64
cargo check -p mw-shell --target x86_64-pc-windows-gnu --tests
```

### 3.3 golden

- 视觉改动必须重拍；`--tol 0` 连跑两遍零差异。
- 不赌宿主指针；hover/点击/拖拽用 `MW_GOLDEN_INPUT` 脚本。
- 时间相关用帧计数虚拟时钟；不要读墙钟。
- 基线在 `shots/golden/<scene>/shell-*.png`；输入脚本在 `shots/golden/inputs/`。
- 门的结论只认日志原文，不认转述。

### 3.4 clippy 基线

- 基线：`scripts/clippy-baseline.txt`（标题 + 文件:行）。
- 改源码导致行号漂移时同步更新基线；新增告警必须为 0。

---

## 4. 工作流

1. 读 `STATUS.md` 找到当前 V1 包。
2. 先写测试/验收脚本，再改代码。
3. 遵守 crate 边界；文档变更走 `Command`；不改 `.zmind` 字段。
4. 跑本地门槛；视觉改动重拍并双跑。
5. 只回填 `STATUS.md`，不新建报告/计划文档。
6. 不提交 git，除非用户明确要求。

---

## 5. 数据与命令规则

- `Document` 是唯一文档树；节点/卡片/分组/关系线都在它下面。
- 所有变更走 `Command`；批量操作合成 `Batch`，一次撤销。
- `ChangeSet` 记录前后快照；`apply_coalesced` 只用于同一手势合并（首次建节点+文字、500ms 样式拖动）。
- `Document.read_only` 是内存闸：`apply/apply_coalesced/undo/redo` 统一拒绝；壳层每帧从 `TabGuard` 灌入。
- `Document.content_rev` 只对样式/结构内容 +1；`SetText`/`SetCollapsed` 不动；`undo/redo` 保守 +1。场景缓存用它跳过内容指纹遍历。
- `Scene.ids/index/nodes` 都用 `Arc` 共享；结构不变时复用；只在并自由主题/过渡帧用 `Arc::make_mut`。
- 盒子缓存键包含文字、字号、字重、图标、标签、图片、最大宽、形状、内距；少一项会错位。
- `BoxSpec.pad` / `NodeBox.pad` 是主题级内距生效后的唯一值；测量、标签、渲染锚点、编辑器同读它。

---

## 6. 关键设计规格

### 6.1 排版

- 画布层级：28/600、20/400、16/400、15/400；行高倍数 1.40/1.44/1.44/1.46；内距 26×12、18×10、14×8、14×8。
- 单行合同盒高：63/49/39/38；实际行盒以 DirectWrite 度量为准。
- 内容字阶以 `mw_core::design::content::TYPE_SCALE` 为准；正文 16/28。
- 画布与内容面是两把尺：不要互相借字号。
- 中英混排间距在**度量与渲染两侧同源**（V1-01 要收口的点）。

### 6.2 主题

- 主题令牌在 `mw_core::theme`；语义色/圆角/间距在 `mw_core::design`。
- `Theme::canvas_dots()` 是点阵唯一出口；`compositor::dot_layer`、SVG、PDF 都读它。
- 画布节点不投影；浮层四档阴影在 `design::elevation`。
- 自定义强调色只借色相；`accented_to` 同时旋转 `levels[].fill/text/stroke`。

### 6.3 画布交互

- 屏幕恒定：折叠圆点、锚点、手柄、箭头、悬停工具条。
- 拖拽阈值 4 px；落点三区；禁止拖入自身子树；落空白成自由主题。
- `Esc` 分层：连线态/手柄拖拽 → 选中道 → 聚焦 → 选择。
- 选中互斥：节点 / 关系线 / 注解同一时刻只有一道。
- 命中与绘制同源：形状用 `rounded_rect_contains` + `corner_style`；椭圆走椭圆方程。

### 6.4 检查器

- 面板 288（240–360）；头行 40；对象行 44；分段 30；分组 28；组内行距 6；左右 12。
- 样式页：色板 8×24、字号/边宽 24×72 stepper、形状四段、圆角、对齐、字重三段。
- 内容页：备注 3 行起/8 行封顶、标签色点、来源行。
- 概念排布：钉住开关、整理选中/全部。
- 作用域「本节点/含子树」在分组标题右侧；500ms 合并撤销。
- 键盘：`Ctrl+Shift+I` 打开，色板数字/Enter，`Esc` 两级，错误条 28。

### 6.5 内容表面

- 大纲：页头 40、版心 800、行 32、缩进 24、页脚 24；行尾 24 px 动作道。
- 卡片页：单一表面，正文 640；H1 30/42；无分栏/预览切换。
- 笔记库：侧栏 232（200–280）、列表 360（320–440）、行 60；断点 1200/840；批量条 44。
- 起始页：版心 880、四张 72 px 快速卡、最近文件异步 + 缺失行标灰。

### 6.6 搜索

- 索引：节点标题/备注/链接/标签/路径 + 概念图命题。
- 拼音：原文 1000/700/500 等权重，备注/链接次之；240 字后只做原文。
- `Ctrl+F` 搜索条、`Ctrl+H` 替换；结果分组与 ±24 摘录。
- 跳转：先展开折叠祖先（一条 Batch）→ 选中 → 居中；聚焦外先退出聚焦。

### 6.7 概念图

- 概念无父子树语义；位置 `pos`；关系线有向 + 短语。
- 默认椭圆（`cm-paper`/`cm-mint`），`cm-slate`/`cm-clay` 圆角；显式覆盖优先。
- 分组派生包围盒；钉住不参与整理。
- `.cxl` 保真槽优先；标准 CXL 读概念/命题/坐标/字号/颜色/箭头。

### 6.8 文件与格式

- `.zmind`：ZIP + `manifest.json` + `content.json` + `notes/` + `assets/`。
- 原子写：临时文件 + rename；失败保留原文件。
- 自动保存：3 秒恢复点；崩溃启动检测；两阶段保存。
- 外部修改：长度+mtime 指纹；保存前比较；冲突行内条。
- 未知顶层/节点键：加载时记账，保存时写回。
- 导入导出：见 `README.md` §1.8；导出读同一份 `Scene`。

### 6.9 中文与 DPI

- IME 组合期间 Enter/Esc/快捷键让位；提交/取消按组合态判断。
- CJK 禁则：行首/行尾禁则、标点挤压按 `mw_core::text`。
- 字体栈：Microsoft YaHei UI → Microsoft YaHei → Noto Sans SC → Segoe UI。
- DPI：`pixels_per_point` 驱动；屏幕恒定元素用 `Camera::chrome/css_to_world`。
- 对比度：文字 ≥4.5:1，图形 ≥3:1；浅主题 `dim`/`faint` 已有硬门。

### 6.10 性能预算

- 20k 打字重建 P7：布局函数 p95 ≤16 ms（Windows 基准机）；端到端目标同量级。
- 撤销/重做 P9：≤30 ms。
- 5k 全量重排 P8：≤120 ms。
- 启动首屏、卡片打开、IME 提交不超预算；具体实测数字记录在 `STATUS.md`。
- 当前 Linux 实测：20k 缓存改字 16.0 ms 中位 / 17.4 p95；Windows 需复跑。

---

## 7. 1.0 剩余工程包（实现指引）

> 顺序与状态在 `STATUS.md`；这里只写「动哪里、做到什么算完成」。

### V1-01 排版与混排
- 动 `mw-render/src/text.rs`、`mw-core/src/text.rs`、`design.rs`、`theme.rs`。
- 中英混排间距在测量与渲染同一函数出宽；补混排单测。
- 跑字重探针（400/500/600 实际落面），按探针决定 500/600 映射；不猜。
- 重拍含中英混排的画布/卡片基线。

### V1-02 形状与圆角
- 动 `mw-core/src/scene.rs`、`theme.rs`、`mw-render/src/instances.rs`、`hit.rs`、`export_svg.rs`、`export_pdf.rs`。
- 椭圆思维导图盒外扩；圆角上限与盒高统一；概念图形状随主题；命中/导出同源。
- 24 与 18 圆角必须可见差异；五种形状逐场基线。

### V1-03 画布角标
- 动 `scene.rs`（角标数据）、`instances.rs`（绘制）、`canvas_methods_*`（命中/点击）。
- 任务状态、链接角标、折叠数量；颜色对比度 ≥3:1。
- 画布/大纲/检查器三处一致。

### V1-04 交互状态
- 动 `instances.rs`/覆盖层、`canvas.rs`、`outline.rs`、`inspector`。
- 悬停 1 px、选中 1.5 px、焦点 1.5+1 px alpha 0.80；主选/非主选/键盘焦点三档。
- 8 状态 × 3 表面逐格有证据。

### V1-05 手柄与加子
- 动 `canvas_methods_2.rs`、`canvas_methods_3.rs`、`consts.rs`。
- 选中态也出加子按钮；关系线起点手柄视觉；跨缩放视觉/命中证据。
- 150/200% 各一张对照；不改变已有 D2/D4/D5/D7/D9 行为。

### V1-06 状态收尾
- 动 `main.rs`/`app_methods_*`/`canvas`/`dialog`。
- 打开冲突检测、失败文案统一、导出失败重试；只读/冲突/保存失败/导出失败各有出口。
- 新增 6 场状态基线。

### V1-07 画布视觉矩阵
- 只跑门与补证据：四主题 × 100/125/150/200% DPI + 关键内容组合。
- 28 场 / 56 次比较，`--tol 0` 双跑零差异。

### V1-08 检查器补缺
- 动 `panels/inspector/*`、`theme.rs`（字段解析）、`document.rs`（命令）。
- 字体斜体/下划线/删除线、线型/箭头、标记/任务/优先级、快速样式/恢复默认、标签颜色/层级。
- 每个可改属性一条命令、一次撤销；全键盘；基线重拍。

### V1-09 布局与整理
- 动 `layout.rs`、`concept_layout.rs`、画布/菜单。
- 五种布局几何校准；整理/对齐/等距/置顶置底/自由主题；整理一条 Batch 一次撤销。
- 五布局逐场基线；20k 整理不超预算。

### V1-10 结构/注解/概念图
- 动 `document.rs`、`scene.rs`、画布、检查器、导出。
- 关系线/概要/边界/分组/自由主题/图片缺口；概念图分组/线型/命题编辑。
- 每条路径可发现、可撤销、可导出；`.cxl` 自往返。

### V1-11 结构综合验收
- 大纲/搜索/折叠/多选/注解组合；卡片页/库回归；四类证据。
- `stage-b`/`stage-cm` 零差异。

### V1-12 文件生命周期
- 动 `files.rs`、`file_guard.rs`、`app_methods_*`、`session.rs`。
- 打开/保存/另存/外部修改/恢复/关闭确认；文件关联 + 单实例。
- 杀进程恢复、外部修改、只读、磁盘失败四条演练。

### V1-13 导入导出
- 动 `mw-io/*`、`export_svg.rs`、`export_pdf.rs`、`export.rs`。
- Xmind 常用字段、Markdown/OPML/FreeMind、PNG/SVG/PDF 常用选项与丢项报告。
- 七格式往返/对照；导出与画布同源。

### V1-14 演示基础
- 现有 F5/步进/控制条/深色舞台；只补一场完整脚本与门。

### V1-15 设置/无障碍/错误
- 动 `settings`/`shortcuts`/`chrome`/`app_helpers`。
- 必要设置项、快捷键、对比度、焦点环、统一 toast/错误；键盘走完设置。

### V1-16 性能
- 动 `layout.rs`/`scene.rs`/`design` 缓存；Windows 复跑。
- 20k 打字/撤销/折叠/搜索、启动、内存；P7/P8/P9 达标。

### V1-17 健壮性/IME/DPI
- 动 `file_guard`/`files`/`note_store`/`fonts`/`canvas`。
- 坏文件/磁盘满/锁/强退恢复；微软拼音实机；100/125/150/200% DPI + 多屏基础。

### V1-18 用户旅程与发布
- 旅程脚本、安装包/绿色包、版本/许可证/用户手册/缺口报告。
- `stage-b`/`stage-cm` 双跑；安装冒烟。

---

## 8. 常见坑

1. 裸跑 `cargo test` 不重定向 `TEMP` → 56+ 条写盘假红。
2. 直接改 `Document` 不发 `Command` → 撤销/重做/保存全部失真。
3. 画布与导出各算一套几何 → 导出错位；只认 `Scene`。
4. 改外观不重拍 golden → 门红；重拍必须 `--tol 0` 双跑。
5. 在 golden 里赌 hover/墙钟 → 每帧清指针、用虚拟时钟。
6. 检查器/画布各写一份颜色/字号/内距 → 改一处漏一处；读 `design`/`Theme`。
7. 概念图新增字段 → 先确认 `.zmind` 兼容与往返测试。
8. Windows 交叉类型检查通过不等于运行通过；IME/DPI/golden 必须 Windows 实机。
9. `scripts/doc-hygiene.ps1` 与 `scripts/x100-evidence.ps1` 检查的是已删除的旧文档，已废弃，不要运行。
10. 不新建 Markdown 报告/计划/日志；进度只回填 `STATUS.md`。
