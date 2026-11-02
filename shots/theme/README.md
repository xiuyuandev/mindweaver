# 四主题视觉基线

同一份夹具文档、同一视口，仅切换主题（`MW_THEME`），用 golden harness 直接离屏截取。

| 主题 | 文件 |
| --- | --- |
| 纸感 | [snow](snow/shell-ppp1.50.png) |
| 墨夜 | [ink](ink/shell-ppp1.50.png) |
| 雾青 | [mist](mist/shell-ppp1.50.png) |
| 暖砂 | [sand/shell-ppp1.50.png](sand/shell-ppp1.50.png) |

生成命令：

```powershell
$env:MW_THEME = 'ink'
target\release\mw-shell.exe --golden --ppp 1.5 --dir shots\theme\ink --update --timeout 30000
Remove-Item Env:MW_THEME
```

说明：

- 这是**当前代码**的真实离屏输出，不是设计稿。
- 只覆盖画布默认夹具；检查器、卡片阅读页、卡片库、开始页、弹窗矩阵还需要逐个打开后补拍。
- 四张图的平均色（sRGB）分别为：纸感 `#F7F7F4`、墨夜 `#1E2124`、雾青 `#F1F5F4`、暖砂 `#F6F1EA`，确认四套不是只改一个强调色。
