# Serial workspace tests with TEMP redirected into the repository.
#
# Why this wrapper exists: under a restricted token the system %TEMP% is not
# writable, so 56 mw-shell + 8 mw-format disk tests fail with "os error 5"
# (Access denied). That is an environment false red, not a code regression.
# The stage gates already redirect TEMP; bare `cargo test` does not.
#
# Usage:
#   powershell -File scripts/test.ps1
#   powershell -File scripts/test.ps1 -Package mw-shell
param(
    [string]$Package = ""
)

$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $PSScriptRoot
$repoTemp = Join-Path $root 'target\tmp'
New-Item -ItemType Directory -Force -Path $repoTemp | Out-Null
$env:TEMP = (Resolve-Path $repoTemp).Path
$env:TMP = $env:TEMP
Set-Location $root

if ($Package -ne "") {
    cargo test -p $Package -- --test-threads=1
} else {
    cargo test --workspace -- --test-threads=1
}
exit $LASTEXITCODE
