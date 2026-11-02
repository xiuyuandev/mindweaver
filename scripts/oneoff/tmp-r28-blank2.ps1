$ErrorActionPreference = 'Continue'
Set-Location 'C:\Users\douha\Pictures\思维导图rust'
$app = Join-Path (Get-Location) 'target/ci-appdata/r28-blank2'
New-Item -ItemType Directory -Force -Path $app | Out-Null
$env:APPDATA = $app
$env:LOCALAPPDATA = $app
$env:MW_CONCEPT_DEMO = '1'
$env:MW_CONTEXT_DEMO = 'blank'
$env:MW_WINDOW_SIZE = '1280x800'
$env:MW_TRACE = 'target/x100/r28-blank-trace.log'
Remove-Item 'target/x100/r28-blank-trace.log' -ErrorAction SilentlyContinue
& cargo golden --ppp 1.0 --tol 0 --warmup 8 --dir target/x100/r28-blank-verify --update 2>&1 | Select-Object -Last 2 | Out-Host
'CODE=' + $LASTEXITCODE