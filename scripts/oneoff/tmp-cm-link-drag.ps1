$ErrorActionPreference='Continue'
$env:MW_CONCEPT_DEMO='1'
$env:MW_WINDOW_SIZE='1280x800'
$env:MW_CONCEPT_LINK_DRAG='用户痛点|240|40'
cargo golden --ppp 1.0 --tol 0 --warmup 10 --update --dir shots/golden/cm-link-drag 2>&1 | Select-Object -Last 1 | Out-File -Encoding utf8 target/x100/cm-link-drag.log
Remove-Item Env:MW_CONCEPT_LINK_DRAG
cargo golden --ppp 1.0 --tol 0 --warmup 10 --update --dir target/x100/ab-nodrag 2>&1 | Select-Object -Last 1 | Out-File -Encoding utf8 target/x100/ab-nodrag.log
exit 0