# X19

## Scope

标签页与文件状态：重做标签条几何、选中态、关闭/重排/右键菜单/键盘焦点，
把保存扩为六态并保证「保存中」先画一帧，把打开改成两段式并补齐加载模态与
失败弹窗。不改 `.zmind` 版本，不新增线程、依赖、crate 或构建工具；所有文档
变更仍只走 `mw_core::Command`，落盘仍只有 `Session::save_to` 一个入口。

## Changes

- **令牌与几何**：`design::component` 新增 `TABBAR_H=34 / TAB_H=28 /
  TAB_MIN_W=108 / TAB_MAX_W=220 / TAB_GAP=2 / TAB_INSET=8 / TAB_CLOSE=22`，
  圆角 `TAB_RADIUS=6`。标签条仍在标题栏下、只在 ≥2 个标签时出现；标签宽
  `clamp(文字宽 + 脏点 12 + 关闭 22 + 内距 16, 108, 220)`，超宽标题用
  `tab_strip::fit_label` 做字符级尾部省略。
- **纯函数模块**：新增 `crates/mw-shell/src/tab_strip.rs`：`tab_width`、
  `past_drag_threshold`、`drop_slot`、`reorder`、`reorder_active`、
  `fit_label`，附带 8 条单测。`ui_kit::tab` 改为消费这些函数，标签绘制
  不再各算各的宽度。
- **选中态与关闭叉**：活动标签 `accent_soft` 底 + 2px `accent` 下划线
  （左右内缩 8px）+ `text` 墨色；非活动悬停 `hover` 底、`dim` 墨色。
  6px `warn` 脏点固定在文字左侧，非活动标签同样显示。关闭叉 14px 图标装在
  22×22 命中格内，悬停给淡底与手型；加号固定在标签条右端 28×28 命中，
  提示「新建标签页（Ctrl+T）」，不随横向滚动移走。
- **标签操作**：单击 / 焦点上 Enter、Space 激活；中键关闭；关闭叉关闭。
  脏标签关闭继续走三选弹窗（保存并关闭 / 不保存关闭 / 取消），非活动标签
  先在自己的会话上保存。新增 `close_tab` / `reorder_tab` 的
  `tab_close` / `tab_reorder` trace。
- **拖拽重排**：按住标签横拖超过 4px 进入重排。阈值按「按下原点落在该标签
  矩形内 + 本次位移超过 4px」判定，不再用 `is_pointer_button_down_on`
  （指针离开标签后它会变假，判定恰恰发生在那之后）。原标签淡出，指针处画
  60% `accent_soft` 幽灵，两个标签间隙画 2px `accent` 竖线（高 20）；松手
  移到插入槽、活动文档跟随移动，`Esc` 取消，拖拽期间不触发单击激活。
- **右键菜单**：标签右键迁移到 X16 菜单引擎，条目固定为「关闭（Ctrl+W）·
  关闭其他 · —— · 复制文件路径 · 在资源管理器中显示」。复制与定位对未落盘
  文档禁用；定位对已不存在的路径禁用。菜单开着时画布与标签条都不收输入；
  `↑↓ / Enter / Esc` 与画布右键菜单共用状态机。复制路径写剪贴板并 toast；
  定位失败给错误 toast，不弹第二个对话框。
- **键盘与焦点**：标签体、关闭叉、加号都画 1.4px `accent` 焦点环，焦点名
  为「标签页 · 标题 / 关闭标签页 · 标题 / 新建标签页」；Tab 顺序严格为
  标签 1 → 关闭 1 → 标签 2 → 关闭 2 … → 加号。既有 `Ctrl+Tab /
  Ctrl+Shift+Tab / Alt+1…9` 行为不变；切换标签仍用 `swap_session` 搬权威
  会话，切换前先把一帧前刚触发的保存写完。
- **六态保存**：`SavePhase { Idle, Saving, Failed { rev, message } }` 叠加到
  `SaveState`，得到 Home / Untitled / Saving / Saved / Dirty / Failed 六态，
  标题芯片与状态栏共用同一判定。`Saving` 文案「保存中…」、`Failed` 文案
  「保存失败」并配 `warn` 色；`Failed` 可点击或用 Ctrl+S 重试。Ctrl+S 的
  路径文档先进入 `Saving` 并画满一帧，下一帧开头 `pump_save()` 才写盘；
  成功记录的是写入那一刻的修订号，期间有新编辑仍是 Dirty；失败绑定失败时的
  修订号，不改 `path`、不动恢复点、不关标签，继续编辑退回 Dirty。
- **关闭与另存**：「保存并关闭」在关闭前同步泵一次写盘，失败停在弹窗原位；
  另存取消不改状态，另存写失败进入 `Failed` 且不采纳新路径。每个标签的脏点
  只读自己会话的修订号。
- **两段式打开**：`OpenPlan { path, painted, demo }` 先挂起并请求重绘，
  至少完整画一帧「正在打开…」模态后才在下一帧开头读盘；模态无按钮、不可
  Esc 关闭、遮挡期间画布与菜单不收输入。文件不存在直接进失败态，不空转。
  打开成功后沿用去重切页、刷新最近文件、抢文件锁、指纹与恢复点探测。
- **失败弹窗**：`OpenFailure { path, reason, from_recent }`，标题「打不开
  文档」，正文给文件名、完整路径与原因；动作为「重试（主）/ 打开其他文件… /
  从最近列表移除 / 关闭」，来源不在最近列表时收为三键，默认「关闭」，Esc
  关闭。重试重新走两段式；移除只动 `recent.json` 并 toast；失败不替换当前
  文档、不关标签、不静默删除最近记录。
- **验收钩子**：`MW_TABS_DEMO=1` 在内存里放三个标签（「产品路线图」干净示例、
  「访谈纪要」有未保存改动、「灵感速记」未落盘空白），不读写任何文件；
  `MW_OPENING_DEMO=1` 与 `MW_OPEN_FAILED=1` 分别显示固定假路径的加载模态与
  失败弹窗。`trace::event` 新增 `save_start/save_ok/save_fail/open_start/
  open_ok/open_fail`。
- **golden**：新增第 19/20/21 个场景 `tabs`（1080×720、ppp 1.5、
  `MW_TABS_DEMO=1`、`--warmup 8`）、`opening`（960×680、ppp 1.5、
  `MW_OPENING_DEMO=1`、`--warmup 8`）、`open-error`（960×680、ppp 1.5、
  `MW_OPEN_FAILED=1`、`--warmup 8`），`scripts/ci.ps1` 用隔离 `APPDATA`
  接入；`overlay` 模态数量期望由 19 更新为 21。

## Verification

- `cargo fmt --all -- --check` 干净（`traces/fmt-final.txt` 为空）。
- `cargo clippy -p mw-shell --all-targets`：mw-shell 自身仍只有
  `export_pdf.rs` 3 条历史告警，新增 0（`traces/clippy-final.txt`；依赖 crate 的 mw-core 10 / mw-render 7 条历史告警与 X18 相同）。
- `cargo test --workspace -- --test-threads=1`：**746 passed / 0 failed**
  （X18 为 735，新增 11 条：`tab_strip` 8 条、失败弹窗动作映射与保存态
  优先级 2 条、`Recent::contains` 1 条；`traces/tests-final.txt`）。
- 行为证据（最终构建、ppp 1.5、1080×720）：
  - **中键关闭**（`inputs/middle-close.txt`）：干净标签
    `tab_close index=2 remaining=2 active=0`，实拍
    `actual/middle-close-ppp1.50.png`。脏标签中键关闭弹三选确认，
    默认焦点「取消」（`actual/dirty-middle-close-ppp1.50.png`）；点
    「不保存关闭」后 `tab_close index=1 remaining=2 active=1`
    （`actual/dirty-close-guard-ppp1.50.png`）。
  - **拖拽重排**（`inputs/drag-hold.txt` / `inputs/drag-reorder.txt`）：
    按住拖动时实拍可见原标签淡出、幽灵与 2px 插入线
    （`actual/drag-hold-ppp1.50.png`）；松手
    `tab_reorder from=0 slot=3 active=2`，活动文档跟随到新下标
    （`actual/drag-reorder-ppp1.50.png`）。
  - **右键菜单**（`inputs/tab-menu.txt`）：
    `menu_open anchor=62,65 entries=5`，实拍为 X16 菜单引擎的五条目，
    未落盘文档的「复制文件路径 / 在资源管理器中显示」为禁用态
    （`actual/tab-menu-ppp1.50.png`）。
  - **键盘焦点与激活**（`inputs/tab-focus-{tab,close,enter,plus}.txt`）：
    画布握着键盘时裸 Tab 是「加子主题」，故先用 F6 交给面板，再走焦点环。
    14 次 Tab 落到标签体「产品路线图」、15 次落到关闭叉、20 次落到加号，
    顺序严格为 标签1 → 关闭1 → 标签2 → 关闭2 → 标签3 → 关闭3 → 加号；
    四处都可见 1.4px `accent` 焦点环（`actual/tab-focus-*-ppp1.50.png`，
    `traces/tab-focus-*.trace`）。第 16 次 Tab 停在「访谈纪要」后 Enter，
    画面切到该文档（`actual/tab-focus-enter-ppp1.50.png`）。四条 trace 均无
    `reveal`，文档保持 13 个主题，不产生额外子主题。
  - **Ctrl+S 保存**（`inputs/ctrl-s-save.txt`）：F2 改名根主题并回车，
    Ctrl+S 后 trace 为 `save_start` → `save_ok rev=1`；实拍标题芯片与
    状态栏均为「已保存」、toast「已保存 save-evidence.zmind」
    （`actual/ctrl-s-saved-ppp1.50.png`）。
  - **打开失败→重试**（`inputs/open-retry.txt`）：`open_start` →
    `open_fail` → 点「重试」再次 `open_start` → `open_fail`，按钮按失败
    当时真实的最近列表重算（演示假路径不在 `recent.json`，4 键收为 3 键）；
    背景文档与标签始终保留（`actual/open-retry-ppp1.50.png`、
    `traces/open-retry.trace`）。
- 金样矩阵：21 个场景 × 2 次 `--tol 0` 全部「✓ 无超容差差异」（42/42，最终二进制复跑，
  `traces/ci-final1.log`、`traces/ci-final2.log`）。既有 18 个场景
  基线与 X18 逐字节一致；新增基线 SHA256：`tabs`
  `f11988fde7e93547467f3f29b17ba0c593ec86545e909368d7f520d07ad2553f`、
  `opening`
  `0d5873698f4609b04b5f77bba87a486dc9cdec065e432a909add6758c4c79962`、
  `open-error`
  `dcd234ecb3f7a1a3788e0200d760dad7bf2a71fb50a485e232bb153ff2a51a7d`
  （`traces/hashes.txt`）。
- 构建：`target/release/mw-shell.exe` **23,452,672 B**，
  SHA256 `f4d4ba43…ffd60250`。

## Known gaps

- 「保存中…」中间帧没有截图：trace 的 `save_start` → `save_ok rev=1` 只
  证明顺序，最终实拍是已保存态；Saving 文案与芯片由 `SavePhase` 单测覆盖。
- 保存失败态（Failed）没有实拍，需要只读目录或占用文件；目前只有
  `SavePhase` 单测与代码路径。
- 脏标签关闭弹窗的「保存并关闭」写盘失败停在弹窗原位没有实拍；本轮只截了
  「不保存关闭」路径。
- Esc 取消拖拽只有代码路径，没有输入脚本、trace 或截图。
- 标签横向溢出后的内部横向滚动、滚轮横滚、加号固定只有宽度夹取与绘制代码，
  没有超宽标签实拍或 golden。
- 右键菜单的启用态「复制文件路径 / 在资源管理器中显示」没有实拍：前者会写
  系统剪贴板，后者会拉起资源管理器；本轮只截未落盘禁用态。
- 打开成功的两段式落地过渡（去重切页、最近文件刷新、抢锁、恢复点探测）没有
  截图；只截了「正在打开」与失败重试。
- 标签体只截了 Enter 激活；Space 走同一条 `ui_kit::activated` 路径但没有
  单独截图。`Ctrl+Tab / Ctrl+Shift+Tab / Alt+1…9` 只有既有单测与代码路径。
- 单标签视图与 X18 逐像素一致由 18 份既有 golden 哈希证明，但没有单独的
  X19 单标签场景基线。
- 标签关闭确认弹窗、右键菜单与打开失败弹窗都用 ppp 1.5 的 1080×720 /
  960×680 实拍；ppp 1.0 与 2.0 只覆盖既有 golden 场景，没有行为实拍。
