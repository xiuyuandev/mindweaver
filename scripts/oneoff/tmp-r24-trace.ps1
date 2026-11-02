$ErrorActionPreference = 'Continue'
$root = 'C:\Users\douha\Pictures\思维导图rust'
Set-Location $root
cargo build --release -p mw-shell 2>&1 | Out-File -Encoding utf8 target/x100/r24-build2.log
'BUILD=' + $LASTEXITCODE
$app = Join-Path $root 'target/ci-appdata/cm-gate-cm-outline-focus-edit'
$env:APPDATA = $app
$env:LOCALAPPDATA = "$app-local"
$env:MW_CONCEPT_OUTLINE = '1'
$env:MW_CONCEPT_FOCUS = '1'
$env:MW_CONCEPT_FOCUS_EDIT = '1'
$env:MW_WINDOW_SIZE = '1280x800'
$env:MW_TRACE = (Join-Path $root 'target/x100/r24-trace.txt')
Remove-Item -Force $env:MW_TRACE -ErrorAction SilentlyContinue
& cargo golden --ppp 1.0 --tol 0 --warmup 8 --dir target/x100/r24-tmp 2>&1 | Select-Object -Last 2 | Out-Host
'CODE=' + $LASTEXITCODE