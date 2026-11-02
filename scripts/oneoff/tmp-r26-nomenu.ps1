$ErrorActionPreference = 'Continue'
$root = 'C:\Users\douha\Pictures\思维导图rust'
Set-Location $root
$app = Join-Path $root 'target/ci-appdata/r26-nomenu'
New-Item -ItemType Directory -Force -Path $app | Out-Null
$env:APPDATA = $app
$env:LOCALAPPDATA = "$app-local"
$env:MW_CONCEPT_ARROW = 'backward'
Remove-Item Env:MW_CONCEPT_ARROW_MENU -ErrorAction SilentlyContinue
$env:MW_WINDOW_SIZE = '1280x800'
& cargo golden --ppp 1.0 --tol 0 --warmup 8 --dir target/x100/r26-nomenu --update 2>&1 | Select-Object -Last 1 | Out-Host
'CODE=' + $LASTEXITCODE