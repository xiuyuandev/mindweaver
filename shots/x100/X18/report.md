# X18

## Scope

开始页重做：把「最近文件」提到模板之前，压缩空状态与提示区，补齐键盘焦点、
Enter/Space 激活和响应式断点，并给开始页建立独立 golden 基线。只重排与打磨
开始页，不改文档模型、命令链和 `.zmind` 版本；开始页仍只发 `start::Action`。

## Changes

- **信息层级**：固定为品牌头 → 快速开始 → 最近文件 → 模板 → 56px 底部提示条
  → 本地优先脚注。最近文件在模板之前；1080×720 首屏完整露出快速开始与三条
  最近文件（`actual/start-1080-ppp1.50.png`）。宽度 ≥ 880 模板三列，≥ 600 两列。
- **单断点**：新增纯函数 `start::layout(avail)`，返回页面宽、快速开始是否横排、
  是否收短副标题、模板列数、是否显示模板区提示、是否显示底部键帽。页面宽
  `(avail - 36).clamp(240, 1000)`；绘制函数不再各自比较宽度。断点单测覆盖
  559/560、699/700、879/880、599/600 与 240 下限。
- **快速开始**：新建空白图（主）/ 打开文件… / 卡片笔记库；页面宽 ≥ 560 横排，
  主卡占 42%，其余均分；< 560 全宽纵排（84px）。< 700 副标题收短为
  `Ctrl+O` / `卡片与标签`。三个图标改走 `Icon::New/Open/Library` 统一矢量，
  删除开始页手绘的文件夹与卡片图标。
- **最近文件**：最多 8 行、行高 52、卡片圆角 `radius::PANEL`；每行文件图标、
  标题、父目录与相对时间。空列表改为 76px 紧凑空状态，只有一行标题与
  「打开或新建后，会在这里快速回来」，不再放重复的打开按钮。有记录时区段标题
  右侧出现「清除列表」文字按钮，只清 `recent.json`，清空后回空状态并 toast。
- **模板**：5 套模板 + 空白画布顺序与内容不变；`1–5` 与 Enter 提示保留；
  预览仍是示意图，不复制 Xmind 素材。
- **底部提示条**：原 148/216px 两张提示卡合并为 56px 横条：左侧文件图标 +
  「拖入 .zmind / Markdown / OPML 即可打开」，右侧键帽「Enter 新建 ·
  Ctrl+O 打开 · 1–5 模板」；< 560 只留左侧。文字按可用像素宽截断，不换行。
- **键盘与焦点**：快速开始、模板、空白卡、最近文件行、清除按钮、返回文档全部
  可 Tab 聚焦；卡片/行画 1.4px 强调色外侧焦点环，主操作卡用 2px 内侧浅色环。
  Enter / Space 等价点击，焦点轨迹写入中文语义名（如
  「开始页 · 模板 思维导图」）。Tab 顺序按绘制顺序，清除按钮在最近文件行之前。
  首帧无焦点时自动聚焦「新建空白图」；`open_home` 通过
  `request_new_focus()` 把菜单返回的焦点也送回主卡。窗口级 Enter 新建只在
  `memory.focused().is_none()` 时生效，避免卡片上的 Enter 再开一份新文档。
- **验收钩子**：`MW_START_DEMO=1` 在内存注入三条固定最近文件（2 分钟前 /
  3 小时前 / 2 天前，通用示例路径），不写盘；`MW_START_FOCUS=1` 强制
  首帧聚焦主操作卡。新增 `Recent::clear()` 与 `Recent::seed_demo()`。
- **golden**：新增第 17 个 `start`（1440×1000、ppp 1.0、`MW_HOME=1`
  `MW_START_DEMO=1`）与第 18 个 `start-narrow`（720×620、ppp 1.5、
  无最近文件）；`scripts/ci.ps1` 用隔离 `APPDATA` 接入。顺带把
  `ui_kit::outline_button` 改为返回 `Response` 以支持焦点环与键盘激活。

## Verification

- `cargo fmt --all -- --check` 干净；`cargo clippy -p mw-shell --all-targets`
  中 mw-shell 自身仍只有 `export_pdf.rs` 3 条历史告警，新增 0
  （`traces/fmt.txt`、`traces/clippy.txt`）。
- `cargo test --workspace -- --test-threads=1`：**735 passed / 0 failed**
  （X17 为 731，新增 4 条：2 条开始页布局/高度、2 条最近文件清空/演示种子）。
- 行为实拍（最终构建、ppp 1.0、1440×1000）：
  - **自动焦点**：首帧焦点 `开始页 · 新建空白图`；`start-full` 实拍可见主卡
    内侧 2px 浅色焦点环。
  - **Tab 顺序**（`inputs/tab-template.txt`）：七次 Tab 依次
    `打开文件 → 卡片笔记库 → 清除最近文件 → 最近文件 产品路线图 →
    最近文件 读书笔记 → 最近文件 旅行计划 → 模板 思维导图`；再 Enter
    打开思维导图，焦点回 `canvas`，截图见 `actual/tab-template-open-ppp1.00.png`。
  - **模板焦点环**（`inputs/focus-template.txt`）：七次 Tab 后停在模板卡，
    `actual/focus-template-ppp1.00.png` 可见卡片外侧 1.4px 强调色环。
  - **清除列表**（`inputs/clear-recents.txt`）：三次 Tab 到清除按钮，Enter 后
    回到空状态、toast「已清除最近文件列表」、焦点回主卡
    （`actual/clear-recents-ppp1.00.png`）。
  - **响应式实拍**：1080×720 ppp 1.5 首屏含三条最近文件；640×460 ppp 1.5
    最小窗口无横向裁剪；720×620 ppp 1.5 窄屏显示紧凑空状态与两列模板。
- 金样矩阵：18 个场景 × 2 次 `--tol 0` 全部「✓ 无超容差差异」（36/36，
  `traces/ci-pass1.log`、`traces/ci-pass2.log`）。新增基线 SHA256：
  `start` `4ec10e68…b054544f`、`start-narrow` `5be44212…f2db11e8`；
  16 个既有场景的基线与 X17 逐字节一致（`traces/hashes.txt`）。
- 构建：`target/release/mw-shell.exe` **23,353,344 B**，
  SHA256 `6ccedb54…d26350f`。

## Known gaps

- 从汉堡菜单进入开始页后的「返回文档」按钮与焦点回移只有代码路径
  （`open_home` → `request_new_focus`），没有脚本化截图；所有行为实拍都从
  默认启动开始页进入。
- 页面宽 < 560 的三张全宽纵排快速开始卡没有独立 golden 或截图：窗口最小逻辑
  宽度 640，扣两侧留白后页面宽 604，始终落在横排档；该分支只有 `layout` 单测。
- 演示最近文件指向通用示例路径（`C:/示例/…`），点击会走真实的「文件不存在」
  失败态；按 X19 计划重做该状态，本轮未截图。
- `start` golden 只覆盖 snow 主题与 ppp 1.0；`start-narrow` 只覆盖 ppp 1.5
  的空列表。深色/雾青/暖砂主题、带最近文件的窄屏、页面滚到底部的状态都没有
  单独基线。
- 清除列表的实拍基于内存演示种子；真实 `recent.json` 的清空只有单测覆盖，
  没有在隔离 APPDATA 下截「有记录 → 清空 → 重启仍为空」的实拍。
- 开始页没有专用的鼠标悬停/按下实拍；按钮反馈沿用 `ui_kit` 既有组件与
  golden 基线，本轮的键盘路径覆盖更充分。
