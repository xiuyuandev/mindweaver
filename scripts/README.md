# scripts/ · 真实输入验收与诊断

这里的脚本用 **Windows 真实窗口 + 合成输入** 验收那些单测抓不到的行为（前台焦点、IME、真实文件对话框、黄金截图）。它们不是 CI 测试，必须在交互式桌面会话里跑。

## 纪律

- **ASCII-only**：`Add-Type` 的 C# 源码与脚本正文都用 ASCII；中文只出现在被写入的文档/夹具内容里。
- **`.ps1` 文件本身要 UTF-8 带 BOM**：本机 `powershell` 是 5.1，按 ANSI(GBK) 读无 BOM 的 UTF-8，
  中文字符串会被拆成非法字节——表现为「表达式或语句中包含意外的标记 `)`」，而不是乱码提示。
  2026-10-30 改 `doc-hygiene.ps1` 时踩过：内容没坏，只是 BOM 被编辑器丢了。修法是
  `[IO.File]::WriteAllText($p,$t,(New-Object System.Text.UTF8Encoding($true)))`。
- **release 优先**：`cargo build --release -p mw-shell`；debug 下 DirectWrite 与合成器慢到测不出手感。
- 从仓库根目录运行；脚本默认把截图写到 `shots/qa/` 或 `shots/m3/`。
- **截图口径**：`PrintWindow` 对 **wgpu 画布**返回旧帧——chrome（面板、菜单）会更新，画布里的节点/选中环/连线不更新。凡是要证明「画布变了」的验收，必须用 `CopyFromScreen`（整屏抓取后按窗口矩形裁剪）。2026-10-19 v9 探针踩过这个坑：`PrintWindow` 前后差 1,479 px 全是标题栏，画布上一动不动。
- **合成拖拽只认 SendInput**：`mouse_event` 的相对移动（`MOUSEEVENTF_MOVE` + dx/dy）在按住期间会被合并/丢弃——按下与松手到得了应用、中间的移动到不了，表现为「拖拽毫无反应」。用 `SendInput` + 绝对坐标（`MOUSEEVENTF_MOVE | ABSOLUTE | VIRTUALDESK`，dx/dy 归一化到 65535），可复用脚本 `win-drag.ps1`。2026-10-19 v9.1 拖拽分离探针因此浪费了两轮。
- **前台与遮挡**：Windows 不允许后台进程可靠注入输入。脚本应断言 `GetForegroundWindow() == mw-shell`；用 `SetWindowPos(HWND_TOP|NOACTIVATE)` 抬 Z 序后，再用 `WindowFromPoint` 抽样确认没有被别的窗口盖住。拿不到前台/输入被安全软件吞掉时，正确结果是退出码 **3 = INCONCLUSIVE**，不是 PASS。
- **Xmind 只读隔离**：动态参考只能由 `x100-xmind-capture.ps1` 在无现有 Xmind 进程时启动临时配置；脚本只截图，不向 Xmind 注入输入。用户已有实例和个人文档绝不触碰。
- 探针会隔离 `APPDATA` 或使用 `MW_HOME=0`，不要拿它去改真实用户配置。

## 当前验收探针

| 脚本 | 覆盖 | 备注 |
| --- | --- | --- |
| `m1b-canvas.ps1` | 选择 / 折叠 / 中键平移 / 缩放 | 需已启动的 `mw-shell`（`-ProcessName`） |
| `m2a-edit-probe.ps1` | 行内编辑全链路 + 快捷键 | 中文用剪贴板粘贴注入，绕开 IME |
| `m2b2-canvas.ps1` | 框选 / 多选 / 整组删除 / 平移改键 / 编辑期锁缩放 | — |
| `m2b3-drag.ps1 -Scenario reparent\|illegal\|reorder\|multi` | 拖拽改父 / 非法落点 / 同级重排 | — |
| `m2b4-canvas.ps1 -Scenario snap\|link` | 对齐吸附 / 拖出关系线 | — |
| `m2c-keys.ps1` | 快捷键 / 动效 / F11 | **先手动启动应用**，24 张截图 |
| `m2c-anim.ps1 -Exe ... -ShotDir ...` | 增删 / 折叠 / 布局过渡 | 自行起停，20 张截图 |
| `m2c-demo.ps1 -Exe ... -ShotDir ...` | F5 演示进入 / 步进 / H / 退出 | 自行起停，15 张截图 |
| `m3-file-e2e.ps1` | 打开 → 改 → `Ctrl+S` → 重启验落盘 | 真实文件对话框 |
| `m3b-shell-e2e.ps1 -Scenario new` | `Ctrl+N` / 最近文件 / 自动保存 / 崩溃恢复 | — |
| `m3-window-state.ps1` | 窗口几何 / 最大化 / 面板状态持久化 | 隔离状态文件 |
| `m3-home.ps1 -Exe ... -ShotDir ...` | 开始页默认入口 / 模板 / 菜单 / 最近文件 | 自行起停 |
| `m3-context.ps1 -Exe ... -ShotDir ...` | 右键菜单节点 / 键盘 / 空白 / 多选 | 断言落盘结构 |
| `m3cc-search.ps1` | `Ctrl+F` 检索与定位 | 粘贴注入，绕开中文 IME |
| `m3cc-tabs.ps1` | 多标签 `Ctrl+T/W/Tab` | 纯 ASCII |
| `m4-export.ps1 -SaveTo ...` | `Ctrl+E` 导出对话 + PNG / 透明 | 真实保存对话框 + 覆盖确认 |
| `m4-import.ps1 -SaveAs ...` | 导入 → 新标签页 → 另存落盘 | 真实打开对话框 |
| `v3-core.ps1 -Exe ... -ShotDir ...` | 方向键导航 + 面板开合相机 | 断言 `arrow_back_diff=0` / `closed_diff=0` |
| `v4-edit.ps1` | 行内编辑器与画布字体/行高对齐 | 150% DPI，`EDITOR_PARITY OK` |
| `v5-accent.ps1` | 自定义强调色 / 撤销 / 持久化 | 含遮挡守卫与键盘-only 分支 |
| `v5-sel.ps1` | 编辑期左键拖选归属 | `EDIT_POINTER OK` |
| `v5-theme.ps1` | 主题三入口等价 / 选中行对比度 | `THEME_ENTRY OK` |
| `v5-clip.ps1` | 窗口级剪贴板五个和弦 | `CLIP OK` |
| `v7-branch.ps1` | **v7 分支线改色 → 落盘 → 撤销** | **PASS 2026-10-19**：遮挡守卫生效，重绘 12,587 px、`branchColor` 落盘节点 `c`、撤销 0 px |
| `v9-float.ps1` | **v9 双击空白新建自由主题** | **PASS 2026-10-19**：双击 (600,700) 后同位置出现带选中环的新节点；截图必须用 `CopyFromScreen`（见「截图口径」） |
| `win-drag.ps1 -X1 .. -Y1 .. -X2 .. -Y2 ..` | **SendInput 绝对坐标拖拽**（v9.1 拖到空白分离自由主题用它验收） | **PASS 2026-10-19**：拖完 `Ctrl+S` 后 `content.json` 里 `root.children` 少一项、多一个 `parentId: null` 的自由主题且子树完整 |
| `x100-xmind-capture.ps1 -Mode check` | Xmind 版本、哈希、现有进程和隔离门禁 | 有用户实例时只报告 `launchBlocked=true`，不启动/点击/关闭任何进程 |
| `x100-xmind-capture.ps1 -Mode launch` | 用临时配置启动独立 Xmind | 仅当没有任何 Xmind 进程时允许；不会发送按键 |
| `x100-xmind-capture.ps1 -Mode capture -ProcessId N` | 对隔离 Xmind 整窗截图并写 sidecar | 自动记录版本、可执行文件哈希和截图哈希 |
| `x100-reference-index.ps1` | 索引既有 Xmind 参考截图 | 记录尺寸、字节、SHA256、修改时间，不复制品牌资产 |
| `x100-contact-sheet.ps1` | 生成 Xmind/织思并排审计图 | 标题包含各自像素尺寸；不伪装成同状态像素门 |
| `x100-fixtures.ps1` | 生成 tiny/complex/20k 固定语料 | tiny 与 complex 入库；20k 写 `target/` 并记录 SHA256 |
| `x100-evidence.ps1 -Round X07 -Init/-Check` | 创建/检查 X100 每轮证据包 | 校验状态、字段、文件存在和 verified 场景结果 |
| `x100-stage-a-gate.ps1` | Stage A 一条命令机器门 | 参考/证据/语料/脚本/fmt/clippy/681 tests/golden 双跑；输出 `target/x100/stage-a-gate.log` |
| `v10-focus.py h3` / `h4` | **H3 面板焦点恢复 / H4 三排色板键盘链路**（Python + `MW_TRACE`；h4 还要 `MW_OPEN` 指向已存盘文档） | **PASS 2026-10-19**：H3 第 49 次 Tab 到 `inspector/layout/kind mindmap`，F9 关→开后 `event restore` + 焦点回到同一控件；H4 键盘改分支色 `#4e9e6b` 落盘、`Ctrl+Z` 后样式清空 |

## 需要 Python 的探针

`v10-focus.py` 用 Python 写：它要 SendInput 绝对坐标 + 解析 `MW_TRACE` 轨迹 + 读 ZIP 里的 `content.json`，PowerShell 做同样的事要长三倍。运行前提与 `make-sample.mjs` 一样——本机装了 Python 3 与 numpy/Pillow。

```powershell
$env:MW_TRACE = "$env:TEMP\h3.txt"; $env:MW_HOME = "0"
Start-Process .\target\release\mw-shell.exe
python scripts\v10-focus.py h3 --trace $env:MW_TRACE
```

## 诊断工具

| 脚本 | 用途 |
| --- | --- |
| `win-capture.ps1 -ProcessId ... -OutFile ...` | 按窗口句柄/进程抓客户区或整窗（可选点击，谨慎使用） |
| `win-rect.ps1` | 打印窗口 / 客户区矩形与 DPI |
| `win-input-check.ps1` | 判断当前桌面会话是否还能注入鼠标/键盘输入；探针 INCONCLUSIVE 时先跑它 |
| `make-sample.mjs --nodes=N` | 生成 5k/10k/20k 合成 `.zmind`（唯一需要 Node 的脚本；产物在 `target/`） |
| `package-release.ps1` | 便携 ZIP + SHA256 + P14 体积断言（不生成安装器） |
| `package-exe.ps1` | **打包 + 安装的当前入口**（2026-11-01 起）：release 构建 → `dist/zhisi-v<ver>-win64/`（`MindWeaver.exe` + `install.ps1` + `setup.cmd` + `README-zh.txt`）→ 同名 `.zip` + `.sha256`；`-Install` 直接装到本机，`-SelfExtract` 尝试出单文件安装器 |
| `install.ps1` | 按用户安装 / 卸载（`-Uninstall` / `-DryRun`）：装到 `%LOCALAPPDATA%\Programs\MindWeaver`，建开始菜单 + 桌面快捷方式、关联 `.zmind`、写「应用和功能」卸载项；**不需要管理员** |
| `doc-hygiene.ps1` | **文档体检**：断链、已知过期断言、索引/归档台账覆盖率；退出码 0 = 干净 |

## 一次性脚本（`scripts/oneoff/`）

`scripts/oneoff/tmp-*.ps1` 是某一轮取证后就不再维护的一次性探针（2026-10-30 从 `scripts/` 根移入，
共 38 个，最后修改都不晚于 2026-10-27）。它们从仓库根运行，不是验收门的一部分；
历史报告里写的 `scripts/tmp-<name>.ps1` 现按 `scripts/oneoff/tmp-<name>.ps1` 找。
需要重跑时优先看 `shots/x100/<ID>/report.md` 记录的等效命令，不要把它们当稳定接口。

## 旧的阶段性探针

`m3c-panels.ps1`、`m3c2-inspector.ps1` 是 M3 面板/检查器阶段的验收脚本，仍可运行，但后续 `v3`–`v7` 的探针覆盖了同类路径；保留用于追溯当时的证据。

## 打包与安装（2026-11-01）

```
# 打包 + 装到本机（不需要管理员）
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package-exe.ps1 -Install

# 只打包（复用已构建的 release）
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package-exe.ps1 -SkipBuild
```

产物：`dist/zhisi-v0.1.0-win64/`（约 25 MB，单文件 exe）、同名 `.zip`（约 9 MB）、`.zip.sha256`。

**坑（都踩过）**：

- `.ps1` 正文带中文会被按 ANSI 解码，**here-string 的终止符会消失**（报「字符串缺少终止符」）。
  所以 `package-exe.ps1` / `install.ps1` / `setup.cmd` 一律 ASCII；中文说明放
  `package-readme.txt`（脚本用 `Get-Content -Encoding UTF8` 读，再以 UTF-8 BOM 写进包）。
- **IExpress 在本机没产出 `setup.exe`**：`iexpress /N zhisi.sed` 静默失败（SED 按官方格式写、
  暂存目录挪到 `%TEMP%` 避开中文路径也不行）。所以「安装」这一步是**解压 → 双击 `setup.cmd`**。
  要真正的单文件安装器，得装 Inno Setup / NSIS / WiX（本机都没有），或另写一个 Rust 引导程序。
- 卸载项指向**安装目录里的** `install.ps1` 副本，不是解压目录——用户删掉解压包也能正常卸载。

## 典型顺序

```powershell
cargo build --release -p mw-shell

# 1) 先确认桌面能注入输入
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/win-input-check.ps1

# 2) 跑当前最需要的 v7 验收（先关掉/最小化 IDE）
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/v7-branch.ps1

# 3) 回归近几轮的核心打磨
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/v5-clip.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/v5-theme.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/v3-core.ps1 -Exe target/release/mw-shell.exe -ShotDir shots/qa
```
