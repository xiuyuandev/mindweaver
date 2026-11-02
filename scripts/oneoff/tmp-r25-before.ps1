$ErrorActionPreference = 'Continue'
$root = 'C:\Users\douha\Pictures\思维导图rust'
Set-Location $root
$app = Join-Path $root 'target/ci-appdata/r25-before'
New-Item -ItemType Directory -Force -Path $app | Out-Null
$env:APPDATA = $app
$env:LOCALAPPDATA = "$app-local"
$env:MW_CONCEPT_DEMO = '1'
$env:MW_CONCEPT_FOCUS = '1'
$env:MW_CONCEPT_FOCUS_TEXT = '优质睡眠由哪些日常习惯、作息时间、情绪起伏、卧室光照、咖啡因摄入、运动安排与睡前屏幕时间共同决定？而在这四十来条里，哪一条才是今晚最该先动、动了就能立刻感觉到差别的那一条？'
$env:MW_WINDOW_SIZE = '1280x800'
& cargo golden --ppp 1.0 --tol 0 --warmup 6 --dir target/x100/r25-before --update 2>&1 | Select-Object -Last 2 | Out-Host
'CODE=' + $LASTEXITCODE