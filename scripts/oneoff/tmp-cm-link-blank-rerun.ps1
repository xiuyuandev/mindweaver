$ErrorActionPreference='Continue'
$env:MW_CONCEPT_DEMO='1'
$env:MW_CONCEPT_LINK_BLANK='用户痛点|240|40'
$env:MW_WINDOW_SIZE='1280x800'
cargo golden --ppp 1.0 --tol 0 --warmup 10 --dir shots/golden/cm-link-blank 2>&1 | Select-Object -Last 2 | Out-File -Encoding utf8 target/x100/cm-link-blank-rerun.log
exit 0