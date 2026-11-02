$ErrorActionPreference='Continue'
$env:MW_CARD_DEMO='1'
cargo golden --ppp 1.5 --tol 0 --warmup 3 --update --dir shots/golden/card-default 2>&1 | Select-Object -Last 1 | Out-File -Encoding utf8 target/x100/card-default.log
Remove-Item Env:MW_CARD_DEMO
$env:MW_CARD_EMPTY='1'
cargo golden --ppp 1.5 --tol 0 --warmup 3 --update --dir shots/golden/card-empty 2>&1 | Select-Object -Last 1 | Out-File -Encoding utf8 target/x100/card-empty.log
exit 0