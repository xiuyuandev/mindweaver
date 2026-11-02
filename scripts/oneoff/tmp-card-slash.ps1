$ErrorActionPreference='Continue'
$env:MW_CARD_DEMO='1'; $env:MW_CARD_SLASH='1'
cargo golden --ppp 1.5 --tol 0 --warmup 3 --update --dir shots/golden/card-slash 2>&1 | Select-Object -Last 2 | Out-File -Encoding utf8 target/x100/card-slash.log
exit 0