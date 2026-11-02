$ErrorActionPreference = 'Continue'
$root = 'C:\Users\douha\Pictures\思维导图rust'
Set-Location $root
cargo fmt --all | Out-Null
cargo fmt --all -- --check | Out-Null
'FMT=' + $LASTEXITCODE
git diff --stat -- crates/mw-shell/src/menu.rs | Out-Host
powershell -NoProfile -File scripts/x100-stage-cm-gate.ps1 *>&1 | Out-File -Encoding utf8 target/x100/cm-gate-r26b.log
'GATE_EXIT=' + $LASTEXITCODE
Get-Date -Format 'yyyy-MM-dd HH:mm:ss'