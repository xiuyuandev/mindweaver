$ErrorActionPreference='Continue'
$env:MW_CONCEPT_OUTLINE='1'; $env:MW_OUTLINE_SEARCH='增长'; $env:MW_WINDOW_SIZE='1280x800'
$env:MW_GOLDEN_INPUT='shots/golden/inputs/label-popup-keys.txt'
$env:MW_TRACE='target/x100/t27-label.txt'
cargo golden --ppp 1.0 --tol 0 --warmup 40 --update --dir target/x100/tmp-label3 2>&1 | Select-Object -Last 2 | Out-File -Encoding utf8 target/x100/t27.log
exit 0