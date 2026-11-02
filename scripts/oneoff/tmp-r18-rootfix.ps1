$ErrorActionPreference='Continue'
$out='target/x100/r18-rootfix.log'; Remove-Item -Force $out -ErrorAction SilentlyContinue
foreach ($p in '1.0','1.5','2.0') {
  "--- update ppp $p" | Out-File -Append -Encoding utf8 $out
  cargo golden --ppp $p --tol 0 --update 2>&1 | Select-String -Pattern '✗|✓|写入' | Select-Object -Last 4 | Out-File -Append -Encoding utf8 $out
}
foreach ($p in '1.0','1.5','2.0') {
  "--- verify ppp $p" | Out-File -Append -Encoding utf8 $out
  cargo golden --ppp $p --tol 0 2>&1 | Select-String -Pattern '✗|✓' | Out-File -Append -Encoding utf8 $out
  cargo golden --ppp $p --tol 0 2>&1 | Select-String -Pattern '✗|✓' | Out-File -Append -Encoding utf8 $out
}
foreach ($f in 'shell-ppp1.00','shell-ppp1.50','shell-ppp2.00','canvas-ppp1.00','canvas-text-ppp1.00') {
  $h=(Get-FileHash "shots/golden/$f.png" -Algorithm SHA256).Hash.Substring(0,12)
  "$f $h" | Out-File -Append -Encoding utf8 $out
}
exit 0