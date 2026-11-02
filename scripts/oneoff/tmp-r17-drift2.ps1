$ErrorActionPreference='Continue'
$out = 'target/x100/r17-probe.log'
Remove-Item -Force $out -ErrorAction SilentlyContinue
$env:MW_CONCEPT_DEMO='1'; $env:MW_CONCEPT_CARD='1'; $env:MW_WINDOW_SIZE='1280x800'
cargo golden --ppp 1.0 --tol 0 --warmup 6 --dir shots/golden/cm-card-link 2>&1 | Select-String -Pattern '超容差|✗|✓' | Out-File -Append -Encoding utf8 $out
$a1 = (Get-FileHash shots/golden/cm-card-link/actual/shell-ppp1.00.png -Algorithm SHA256).Hash.Substring(0,12)
Copy-Item shots/golden/cm-card-link/actual/shell-ppp1.00.png target/x100/cardlink-actual1.png -Force
cargo golden --ppp 1.0 --tol 0 --warmup 6 --dir shots/golden/cm-card-link 2>&1 | Select-String -Pattern '超容差|✗|✓' | Out-File -Append -Encoding utf8 $out
$a2 = (Get-FileHash shots/golden/cm-card-link/actual/shell-ppp1.00.png -Algorithm SHA256).Hash.Substring(0,12)
$b = (Get-FileHash shots/golden/cm-card-link/shell-ppp1.00.png -Algorithm SHA256).Hash.Substring(0,12)
"base=$b a1=$a1 a2=$a2" | Out-File -Append -Encoding utf8 $out
"a1eqa2=$($a1 -eq $a2) a1eqBase=$($a1 -eq $b)" | Out-File -Append -Encoding utf8 $out
exit 0