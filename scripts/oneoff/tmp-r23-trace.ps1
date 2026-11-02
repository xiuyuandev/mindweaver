$ErrorActionPreference = 'Continue'
$root = 'C:\Users\douha\Pictures\思维导图rust'
Set-Location $root
cargo build --release -p mw-shell 2>&1 | Out-File -Encoding utf8 target/x100/r23-build2.log
'BUILD=' + $LASTEXITCODE
$app = Join-Path $root 'target/ci-appdata/cm-gate-cm-outline-focus'
$env:APPDATA = $app
$env:LOCALAPPDATA = "$app-local"
$env:MW_CONCEPT_OUTLINE = '1'
$env:MW_CONCEPT_FOCUS = '1'
$env:MW_WINDOW_SIZE = '1280x800'
$env:MW_TRACE = (Join-Path $root 'target/x100/r23-trace.txt')
& cargo golden --ppp 1.0 --tol 0 --warmup 6 --dir shots/golden/cm-outline-focus 2>&1 | Select-Object -Last 4 | Out-Host
'CODE=' + $LASTEXITCODE