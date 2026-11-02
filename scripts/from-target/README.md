# 从 target/ 捡回的脚本

这些 PowerShell 脚本原本散落在 `target/`（通常是 QA/验收过程中生成的临时脚本）。2026-10-24 清理 `target/` 时把它们移到这里，统一保管。

## 注意

- 这些脚本**不是 CI 主路径**，需要时手工调用。
- 脚本内的 `$root` 已修复为项目根目录（`scripts/from-target/` 的上两级）。
- 调用方式示例：
  ```powershell
  powershell -File scripts\from-target\audit-package.ps1
  ```

## 文件

| 文件 | 用途 |
| --- | --- |
| `audit-package.ps1` | 审计发布包内容与哈希 |
| `final-package-smoke.ps1` | 最终包冒烟测试 |
| `inspector-shot.ps1` | 检查器截图探针 |
| `list-windows.ps1` | 列出进程窗口 |
| `probe-window.ps1` | 窗口探测 |
| `qa-hover.ps1` | QA hover 截图 |
| `qa-key.ps1` | QA 按键注入 |
| `qa-launch.ps1` | QA 启动进程 |
| `qa-pixels.ps1` | QA 像素扫描 |
