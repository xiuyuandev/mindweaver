$ErrorActionPreference = 'Continue'
Set-Location 'C:\Users\douha\Pictures\思维导图rust'
$app = Join-Path (Get-Location) 'target/ci-appdata/r27-key'
New-Item -ItemType Directory -Force -Path $app | Out-Null
$env:APPDATA = $app
$env:LOCALAPPDATA = $app
$env:MW_CONCEPT_ARROW = 'off'
$env:MW_CONCEPT_ARROW_MENU = '1'
$env:MW_GOLDEN_INPUT = 'shots/golden/inputs/cm-arrow-menu-key.txt'
$env:MW_TRACE = 'target/x100/r27-key-trace.log'
$env:MW_WINDOW_SIZE = '1280x800'
Remove-Item 'target/x100/r27-key-trace.log' -ErrorAction SilentlyContinue
& cargo golden --ppp 1.0 --tol 0 --warmup 32 --dir target/x100/r27-key --update 2>&1 | Select-Object -Last 1 | Out-Host
'CODE=' + $LASTEXITCODE