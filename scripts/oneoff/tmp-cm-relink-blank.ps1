$ErrorActionPreference='Continue'
$env:MW_CONCEPT_DEMO='1'
$env:MW_CONCEPT_RELINK_BLANK='带来|200|140'
$env:MW_WINDOW_SIZE='1280x800'
$env:MW_TRACE='target/x100/relink-blank.trace'
Remove-Item -Force target/x100/relink-blank.trace -ErrorAction SilentlyContinue
cargo golden --ppp 1.0 --tol 0 --warmup 10 --update --dir shots/golden/cm-relink-blank 2>&1 | Select-Object -Last 1 | Out-File -Encoding utf8 target/x100/relink-blank.log
cargo golden --ppp 1.0 --tol 0 --warmup 10 --dir shots/golden/cm-relink-blank 2>&1 | Select-Object -Last 1 | Out-File -Encoding utf8 target/x100/relink-blank-rerun.log
Select-String -Path target/x100/relink-blank.trace -Pattern 'relink_new_concept|rel-edit-open' | Select-Object -First 3
exit 0