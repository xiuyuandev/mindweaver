$ErrorActionPreference = 'Continue'
$root = 'C:\Users\douha\Pictures\思维导图rust'
Set-Location $root
$app = Join-Path $root 'target/ci-appdata/cm-gate-cm-outline-focus'
$env:APPDATA = $app
$env:LOCALAPPDATA = "$app-local"
$env:MW_CONCEPT_OUTLINE = '1'
$env:MW_CONCEPT_FOCUS = '1'
$env:MW_WINDOW_SIZE = '1280x800'
$env:MW_CONCEPT_FOCUS_TEXT = '睡眠质量到底由哪些日常习惯、作息时间、情绪起伏、卧室光照与运动安排共同决定，而这四十来条里，哪一条才是今晚最该先改的？'
& cargo golden --ppp 1.0 --tol 0 --warmup 6 --dir target/x100/r23-wrap --update 2>&1 | Select-Object -Last 3 | Out-Host
'CODE=' + $LASTEXITCODE