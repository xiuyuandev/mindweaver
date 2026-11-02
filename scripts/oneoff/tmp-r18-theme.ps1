$ErrorActionPreference='Continue'
$out='target/x100/r18-theme.log'; Remove-Item -Force $out -ErrorAction SilentlyContinue
# 2026-10-30 文档整理：移到 scripts/oneoff/ 后仓库根要多上溯一层
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $root
git log -1 --format='baseline_date=%ci' -- shots/golden/theme-ink/shell-ppp1.50.png | Out-File -Append -Encoding utf8 $out
$themeAppData = Join-Path $root 'target/ci-appdata/theme'
New-Item -ItemType Directory -Force -Path $themeAppData | Out-Null
$env:APPDATA = $themeAppData
foreach ($t in 'ink','mist','sand') {
  $env:MW_THEME = $t
  "--- update $t" | Out-File -Append -Encoding utf8 $out
  cargo golden --ppp 1.5 --tol 0 --update --dir "shots/golden/theme-$t" 2>&1 | Select-String -Pattern '✗|✓' | Out-File -Append -Encoding utf8 $out
  cargo golden --ppp 1.5 --tol 0 --dir "shots/golden/theme-$t" 2>&1 | Select-String -Pattern '✗|✓' | Out-File -Append -Encoding utf8 $out
  cargo golden --ppp 1.5 --tol 0 --dir "shots/golden/theme-$t" 2>&1 | Select-String -Pattern '✗|✓' | Out-File -Append -Encoding utf8 $out
  $h=(Get-FileHash "shots/golden/theme-$t/shell-ppp1.50.png" -Algorithm SHA256).Hash.Substring(0,12)
  "$t hash=$h" | Out-File -Append -Encoding utf8 $out
}
exit 0