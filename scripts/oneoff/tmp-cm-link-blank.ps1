$ErrorActionPreference='Continue'
$env:MW_CONCEPT_DEMO='1'
$env:MW_CONCEPT_LINK_BLANK='用户痛点|240|40'
$env:MW_WINDOW_SIZE='1280x800'
$env:MW_TRACE='target/x100/t-cm-link-blank.txt'
cargo golden --ppp 1.0 --tol 0 --warmup 10 --update --dir shots/golden/cm-link-blank 2>&1 | Select-Object -Last 3 | Out-File -Encoding utf8 target/x100/cm-link-blank.log
exit 0