$ErrorActionPreference = 'Continue'
Remove-Item Env:MW_CONCEPT_FOCUS, Env:MW_CONCEPT_DEMO -ErrorAction SilentlyContinue
$env:MW_EXPORT_BENCH='target/x100/r22-plain.png'
& target/release/mw-shell.exe 2>&1 | Out-String -Width 200
'plain exit=' + $LASTEXITCODE