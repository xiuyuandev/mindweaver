$ErrorActionPreference = 'Continue'
$root = 'C:\Users\douha\Pictures\思维导图rust'
Set-Location $root
cargo build --release -p mw-shell 2>&1 | Out-File -Encoding utf8 target/x100/r23-build.log
'BUILD=' + $LASTEXITCODE
function Shoot($extra) {
  $app = Join-Path $root 'target/ci-appdata/cm-gate-cm-outline-focus'
  New-Item -ItemType Directory -Force -Path $app | Out-Null
  $env:APPDATA = $app
  $env:LOCALAPPDATA = "$app-local"
  $env:MW_CONCEPT_OUTLINE = '1'
  $env:MW_CONCEPT_FOCUS = '1'
  $env:MW_WINDOW_SIZE = '1280x800'
  & cargo golden --ppp 1.0 --tol 0 --warmup 6 --dir shots/golden/cm-outline-focus @extra 2>&1 | Out-Host
  'CODE=' + $LASTEXITCODE
  Remove-Item Env:MW_CONCEPT_OUTLINE, Env:MW_CONCEPT_FOCUS, Env:MW_WINDOW_SIZE
}
Shoot @('--update')
Shoot @()
Shoot @()