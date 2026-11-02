$ErrorActionPreference = 'Continue'
Remove-Item target/x100/r22-focus-ex.png, target/x100/r22-plain-ex.png, target/x100/r22-focus.pdf -ErrorAction SilentlyContinue
Remove-Item Env:MW_CONCEPT_FOCUS, Env:MW_CONCEPT_DEMO -ErrorAction SilentlyContinue
$env:MW_OPEN='target/x100/r22-plain.zmind'
$env:MW_EXPORT_BENCH='target/x100/r22-plain-ex.png'
& target/release/mw-shell.exe 2>&1 | Out-String -Width 200
'plain exit=' + $LASTEXITCODE
$env:MW_OPEN='target/x100/r22-focus.zmind'
$env:MW_EXPORT_BENCH='target/x100/r22-focus-ex.png'
& target/release/mw-shell.exe 2>&1 | Out-String -Width 200
'focus exit=' + $LASTEXITCODE
Remove-Item Env:MW_OPEN, Env:MW_EXPORT_BENCH -ErrorAction SilentlyContinue
& target/release/mw-shell.exe --export-pdf target/x100/r22-focus.zmind --out target/x100/r22-focus.pdf 2>&1 | Out-String -Width 200
'pdf exit=' + $LASTEXITCODE
Get-ChildItem target/x100/r22-focus-ex.png, target/x100/r22-plain-ex.png, target/x100/r22-focus.pdf -ErrorAction SilentlyContinue | Select-Object Name,Length | Format-Table -AutoSize | Out-String -Width 100