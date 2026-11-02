$ErrorActionPreference='Continue'
cargo test --workspace -- --test-threads=1 2>&1 | Select-String -Pattern 'test result|^error|FAILED' | Out-File -Encoding utf8 target/x100/ws-tests.log
powershell -NoProfile -File scripts/x100-stage-cm-gate.ps1 2>&1 | Select-Object -Last 24 | Out-File -Encoding utf8 target/x100/cm-gate.log
exit 0