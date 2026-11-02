$ErrorActionPreference = 'Continue'
$root = 'C:\Users\douha\Pictures\思维导图rust'
Set-Location $root
$app = Join-Path $root 'target/ci-appdata/cm-gate-cm-focus-long'
New-Item -ItemType Directory -Force -Path $app | Out-Null
$env:APPDATA = $app
$env:LOCALAPPDATA = "$app-local"
$env:MW_CONCEPT_DEMO = '1'
$env:MW_CONCEPT_FOCUS = '1'
$env:MW_CONCEPT_FOCUS_TEXT = '优质睡眠由哪些日常习惯、作息时间、情绪起伏、卧室光照、咖啡因摄入、运动安排与睡前屏幕时间共同决定？而在这四十来条里，哪一条才是今晚最该先动、动了就能立刻感觉到差别的那一条？如果改一条就够，那它应该先改谁旁边的谁，才不至于让别的习惯白改一场、也不必等到明天夜里才知道有没有用？'
$env:MW_WINDOW_SIZE = '1280x800'
& cargo golden --ppp 1.0 --tol 0 --warmup 6 --dir shots/golden/cm-focus-long --update 2>&1 | Select-Object -Last 2 | Out-Host
'CODE=' + $LASTEXITCODE
& cargo golden --ppp 1.0 --tol 0 --warmup 6 --dir shots/golden/cm-focus-long 2>&1 | Select-Object -Last 2 | Out-Host
'V1=' + $LASTEXITCODE
& cargo golden --ppp 1.0 --tol 0 --warmup 6 --dir shots/golden/cm-focus-long 2>&1 | Select-Object -Last 2 | Out-Host
'V2=' + $LASTEXITCODE