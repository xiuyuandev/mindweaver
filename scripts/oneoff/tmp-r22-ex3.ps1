$ErrorActionPreference = 'Continue'
Remove-Item target/x100/r22-focus-ex.png, target/x100/r22-plain-ex.png, target/x100/r22-focus.pdf, target/x100/r22-plain.pdf -ErrorAction SilentlyContinue
Remove-Item Env:MW_CONCEPT_FOCUS, Env:MW_CONCEPT_DEMO, Env:MW_OPEN, Env:MW_EXPORT_BENCH -ErrorAction SilentlyContinue
$env:MW_OPEN='target/x100/r22-plain.zmind'
$env:MW_EXPORT_BENCH='target/x100/r22-plain-ex.png'
& target/release/mw-shell.exe 2>&1 | Out-String -Width 200
$env:MW_OPEN='target/x100/r22-focus.zmind'
$env:MW_EXPORT_BENCH='target/x100/r22-focus-ex.png'
& target/release/mw-shell.exe 2>&1 | Out-String -Width 200
Remove-Item Env:MW_OPEN, Env:MW_EXPORT_BENCH -ErrorAction SilentlyContinue
& target/release/mw-shell.exe --export-pdf target/x100/r22-plain.zmind --out target/x100/r22-plain.pdf 2>&1 | Out-String -Width 200
& target/release/mw-shell.exe --export-pdf target/x100/r22-focus.zmind --out target/x100/r22-focus.pdf 2>&1 | Out-String -Width 200