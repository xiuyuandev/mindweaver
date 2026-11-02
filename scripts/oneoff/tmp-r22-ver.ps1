$ErrorActionPreference = 'Continue'
cargo build --release -p mw-shell 2>&1 | Out-File -Encoding utf8 target/x100/r22-build5.log
'BUILD=' + $LASTEXITCODE
Remove-Item target/x100/r22-focus-ex.png, target/x100/r22-plain-ex.png -ErrorAction SilentlyContinue
Remove-Item Env:MW_CONCEPT_FOCUS, Env:MW_CONCEPT_DEMO, Env:MW_OPEN, Env:MW_EXPORT_BENCH -ErrorAction SilentlyContinue
$env:MW_OPEN='target/x100/r22-plain.zmind'
$env:MW_EXPORT_BENCH='target/x100/r22-plain-ex.png'
& target/release/mw-shell.exe 2>&1 | Out-Null
$env:MW_OPEN='target/x100/r22-focus.zmind'
$env:MW_EXPORT_BENCH='target/x100/r22-focus-ex.png'
& target/release/mw-shell.exe 2>&1 | Out-Null
Remove-Item Env:MW_OPEN, Env:MW_EXPORT_BENCH -ErrorAction SilentlyContinue
& 'C:\Users\douha\.dsh\dsh-runtimes\dsh-primary-runtime\dependencies\python\python.exe' target/x100/r22-shift2.py