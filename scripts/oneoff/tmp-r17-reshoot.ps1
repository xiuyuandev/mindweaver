$ErrorActionPreference='Continue'
$out='target/x100/r17-reshoot.log'; Remove-Item -Force $out -ErrorAction SilentlyContinue
$env:MW_CONCEPT_DEMO='1'; $env:MW_CONCEPT_CARD='1'; $env:MW_WINDOW_SIZE='1280x800'
cargo golden --ppp 1.0 --tol 0 --warmup 6 --update --dir shots/golden/cm-card-link 2>&1 | Select-Object -Last 1 | Out-File -Append -Encoding utf8 $out
cargo golden --ppp 1.0 --tol 0 --warmup 6 --dir shots/golden/cm-card-link 2>&1 | Select-Object -Last 1 | Out-File -Append -Encoding utf8 $out
cargo golden --ppp 1.0 --tol 0 --warmup 6 --dir shots/golden/cm-card-link 2>&1 | Select-Object -Last 1 | Out-File -Append -Encoding utf8 $out
$h=(Get-FileHash shots/golden/cm-card-link/shell-ppp1.00.png -Algorithm SHA256).Hash.Substring(0,12)
"hash=$h" | Out-File -Append -Encoding utf8 $out
exit 0