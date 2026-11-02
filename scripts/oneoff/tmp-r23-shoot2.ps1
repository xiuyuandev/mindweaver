$ErrorActionPreference = 'Continue'
$root = 'C:\Users\douha\Pictures\思维导图rust'
Set-Location $root
cargo build --release -p mw-shell 2>&1 | Out-File -Encoding utf8 target/x100/r23-build3.log
'BUILD=' + $LASTEXITCODE
$app = Join-Path $root 'target/ci-appdata/cm-gate-cm-outline-focus'
New-Item -ItemType Directory -Force -Path $app | Out-Null
$env:APPDATA = $app
$env:LOCALAPPDATA = "$app-local"
$env:MW_CONCEPT_OUTLINE = '1'
$env:MW_CONCEPT_FOCUS = '1'
$env:MW_WINDOW_SIZE = '1280x800'
'--- baseline update ---'
& cargo golden --ppp 1.0 --tol 0 --warmup 6 --dir shots/golden/cm-outline-focus --update 2>&1 | Select-Object -Last 3 | Out-Host
'CODE=' + $LASTEXITCODE
'--- verify 1 ---'
& cargo golden --ppp 1.0 --tol 0 --warmup 6 --dir shots/golden/cm-outline-focus 2>&1 | Select-Object -Last 3 | Out-Host
'CODE=' + $LASTEXITCODE
'--- verify 2 ---'
& cargo golden --ppp 1.0 --tol 0 --warmup 6 --dir shots/golden/cm-outline-focus 2>&1 | Select-Object -Last 3 | Out-Host
'CODE=' + $LASTEXITCODE
'--- long question wrap ---'
$env:MW_CONCEPT_FOCUS_TEXT = '睡眠质量到底由哪些日常习惯、作息与情绪共同决定，又该先改哪一条？'
& cargo golden --ppp 1.0 --tol 0 --warmup 6 --dir target/x100/r23-wrap --update 2>&1 | Select-Object -Last 3 | Out-Host
'CODE=' + $LASTEXITCODE