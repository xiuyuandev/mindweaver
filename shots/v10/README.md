# V10 页面矩阵

当前已生成的页面（均为 golden 离屏实拍，1620×1080 @150%）：

| 页面 | 主题 | 文件 |
| --- | --- | --- |
| 画布 | 纸感 | [../theme/snow/shell-ppp1.50.png](../theme/snow/shell-ppp1.50.png) |
| 画布 | 墨夜 | [../theme/ink/shell-ppp1.50.png](../theme/ink/shell-ppp1.50.png) |
| 画布 | 雾青 | [../theme/mist/shell-ppp1.50.png](../theme/mist/shell-ppp1.50.png) |
| 画布 | 暖砂 | [../theme/sand/shell-ppp1.50.png](../theme/sand/shell-ppp1.50.png) |
| 检查器 260 | 纸感 | [../inspector/260/shell-ppp1.50.png](../inspector/260/shell-ppp1.50.png) |
| 检查器 360 | 纸感 | [../inspector/360/shell-ppp1.50.png](../inspector/360/shell-ppp1.50.png) |
| 检查器 520 | 纸感 | [../inspector/520/shell-ppp1.50.png](../inspector/520/shell-ppp1.50.png) |
| 卡片阅读页 | 纸感 | [card-snow](card-snow/shell-ppp1.50.png) |
| 卡片阅读页 | 墨夜 | [card-ink](card-ink/shell-ppp1.50.png) |
| 卡片库（空库） | 纸感 | [library-snow](library-snow/shell-ppp1.50.png) |
| 设置弹窗 | 纸感 | [settings-snow](settings-snow/shell-ppp1.50.png) |
| 空画布 | 纸感 | [empty-snow](empty-snow/shell-ppp1.50.png) |
| Toast | 纸感 | [toast-snow](toast-snow/shell-ppp1.50.png) |
| 开始页 | 纸感 | [start-snow](start-snow/shell-ppp1.50.png) |
| 开始页 | 墨夜 | [start-ink](start-ink/shell-ppp1.50.png) |
| 开始页 | 雾青 | [start-mist](start-mist/shell-ppp1.50.png) |
| 开始页 | 暖砂 | [start-sand](start-sand/shell-ppp1.50.png) |

## 还没生成

- 每个页面 × 4 主题 × 3 档 DPI 的完整矩阵；当前是“关键页 + 代表性主题”。
- 开始页在 `MW_HOME_SCREEN` 下需要 harness 的 `std::process::exit` 兜底（已修）。

## 生成钩子

| 环境变量 | 作用 |
| --- | --- |
| `MW_THEME=ink/mist/sand` | 切主题 |
| `MW_INSPECTOR=1` + `MW_INSPECTOR_W=260/360/520` | 展开检查器并定宽 |
| `MW_CARD_DEMO=1` | 打开示例卡片阅读页 |
| `MW_SETTINGS_DEMO=1` | 打开设置 |
| `MW_HOME_SCREEN=1` | 落开始页（当前 golden 下不稳定） |
| `MW_TOAST_DEMO=<text>` | 启动即弹 Toast |
