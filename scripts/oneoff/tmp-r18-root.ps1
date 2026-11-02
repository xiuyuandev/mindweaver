$ErrorActionPreference='Continue'
$out='target/x100/r18-root.log'; Remove-Item -Force $out -ErrorAction SilentlyContinue
Remove-Item Env:MW_CONCEPT_DEMO -ErrorAction SilentlyContinue
cargo golden --ppp 1.0 --tol 0 2>&1 | Select-String -Pattern '✗|✓' | Out-File -Append -Encoding utf8 $out
$h1=(Get-FileHash shots/golden/actual/shell-ppp1.00.png -Algorithm SHA256).Hash.Substring(0,12)
Copy-Item shots/golden/actual/shell-ppp1.00.png target/x100/root-a1.png -Force
cargo golden --ppp 1.0 --tol 0 2>&1 | Select-String -Pattern '✗|✓' | Out-File -Append -Encoding utf8 $out
$h2=(Get-FileHash shots/golden/actual/shell-ppp1.00.png -Algorithm SHA256).Hash.Substring(0,12)
$hb=(Get-FileHash shots/golden/shell-ppp1.00.png -Algorithm SHA256).Hash.Substring(0,12)
"base=$hb a1=$h1 a2=$h2" | Out-File -Append -Encoding utf8 $out
exit 0