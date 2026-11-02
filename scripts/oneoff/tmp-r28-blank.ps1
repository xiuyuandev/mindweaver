$ErrorActionPreference = 'Continue'
Set-Location 'C:\Users\douha\Pictures\思维导图rust'
$app = Join-Path (Get-Location) 'target/ci-appdata/r28-blank'
New-Item -ItemType Directory -Force -Path $app | Out-Null
$env:APPDATA = $app
$env:LOCALAPPDATA = $app
$env:MW_CONCEPT_DEMO = '1'
$env:MW_CONTEXT_DEMO = 'blank'
$env:MW_WINDOW_SIZE = '1280x800'
& cargo golden --ppp 1.0 --tol 0 --warmup 8 --dir shots/golden/cm-blank-menu --update 2>&1 | Select-Object -Last 3 | Out-Host
'CODE=' + $LASTEXITCODE