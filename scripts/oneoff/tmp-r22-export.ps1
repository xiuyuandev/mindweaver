$ErrorActionPreference = 'Continue'
Remove-Item target/x100/r22-focus.png, target/x100/r22-control.png -ErrorAction SilentlyContinue
Remove-Item Env:MW_CONCEPT_FOCUS -ErrorAction SilentlyContinue
$env:MW_CONCEPT_DEMO='1'
$env:MW_EXPORT_BENCH='target/x100/r22-control.png'
& target/release/mw-shell.exe 2>&1 | Out-String -Width 200
'control exit=' + $LASTEXITCODE
Remove-Item Env:MW_CONCEPT_DEMO -ErrorAction SilentlyContinue
$env:MW_CONCEPT_FOCUS='1'
$env:MW_EXPORT_BENCH='target/x100/r22-focus.png'
& target/release/mw-shell.exe 2>&1 | Out-String -Width 200
'focus exit=' + $LASTEXITCODE
Get-ChildItem target/x100/r22-*.png | Select-Object Name,Length | Format-Table -AutoSize | Out-String -Width 100