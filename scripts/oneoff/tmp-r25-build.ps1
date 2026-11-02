$ErrorActionPreference = 'Continue'
$root = 'C:\Users\douha\Pictures\思维导图rust'
Set-Location $root
cargo fmt --all | Out-Null
cargo check -p mw-shell --all-targets 2>&1 | Out-File -Encoding utf8 target/x100/r25-check.log
'CHECK=' + $LASTEXITCODE
Get-Content target/x100/r25-check.log | Select-String -Pattern '^(error|warning)' | Select-Object -First 8 | Out-Host
cargo test -p mw-shell --bin mw-shell focus_ -- --test-threads=1 2>&1 | Select-String -Pattern 'test result|^test ' | Out-Host
'T=' + $LASTEXITCODE
cargo build --release -p mw-shell 2>&1 | Out-File -Encoding utf8 target/x100/r25-build.log
'BUILD=' + $LASTEXITCODE