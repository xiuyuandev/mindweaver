$ErrorActionPreference = 'Continue'
$root = 'C:\Users\douha\Pictures\思维导图rust'
Set-Location $root
cargo build --release -p mw-shell 2>&1 | Out-File -Encoding utf8 target/x100/r26-build.log
'BUILD=' + $LASTEXITCODE
function Shoot($dir, $arrow) {
  $app = Join-Path $root ("target/ci-appdata/r26-" + $dir)
  New-Item -ItemType Directory -Force -Path $app | Out-Null
  $env:APPDATA = $app
  $env:LOCALAPPDATA = "$app-local"
  $env:MW_CONCEPT_ARROW = $arrow
  $env:MW_CONCEPT_ARROW_MENU = '1'
  $env:MW_WINDOW_SIZE = '1280x800'
  & cargo golden --ppp 1.0 --tol 0 --warmup 8 --dir $dir --update 2>&1 | Select-Object -Last 1 | Out-Host
  'SHOOT=' + $dir + ':' + $LASTEXITCODE
}
Shoot 'shots/golden/cm-arrow-menu' 'backward'
& cargo golden --ppp 1.0 --tol 0 --warmup 8 --dir shots/golden/cm-arrow-menu 2>&1 | Select-Object -Last 1 | Out-Host
'V1=' + $LASTEXITCODE
& cargo golden --ppp 1.0 --tol 0 --warmup 8 --dir shots/golden/cm-arrow-menu 2>&1 | Select-Object -Last 1 | Out-Host
'V2=' + $LASTEXITCODE
Shoot 'target/x100/r26-off' 'off'