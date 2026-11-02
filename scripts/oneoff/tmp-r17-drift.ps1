$ErrorActionPreference='Continue'
cargo test -p mw-shell --bin mw-shell chrome::tests::shell_sources_do_not_hardcode_font_sizes 2>&1 | Select-String -Pattern 'test result|panicked|offenders' | Select-Object -First 3
cargo build --release -p mw-shell 2>&1 | Select-String -Pattern '^error' | Select-Object -First 2
$env:MW_CONCEPT_DEMO='1'; $env:MW_WINDOW_SIZE='1280x800'
$env:MW_CONCEPT_CARD='1'
cargo golden --ppp 1.0 --tol 0 --warmup 6 --dir shots/golden/cm-card-link 2>&1 | Select-Object -Last 1
(Get-FileHash shots/golden/cm-card-link/shell-ppp1.00.png -Algorithm SHA256).Hash.Substring(0,12) + ' base'
(Get-FileHash shots/golden/cm-card-link/actual/shell-ppp1.00.png -Algorithm SHA256).Hash.Substring(0,12) + ' actual1'
cargo golden --ppp 1.0 --tol 0 --warmup 6 --dir shots/golden/cm-card-link 2>&1 | Select-Object -Last 1
(Get-FileHash shots/golden/cm-card-link/actual/shell-ppp1.00.png -Algorithm SHA256).Hash.Substring(0,12) + ' actual2'
exit 0