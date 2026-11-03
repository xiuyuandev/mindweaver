# Build a portable Windows ZIP for Zhisi / MindWeaver.
# Usage:
#   powershell -File scripts/package.ps1
#   powershell -File scripts/package.ps1 -Version 0.1.0 -SkipBuild
param(
    [string]$Version = '0.1.0',
    [switch]$SkipBuild,
    # Code signing is optional until a certificate exists: configure MW_SIGN_PFX
    # or MW_SIGN_THUMBPRINT (see docs/20-签名与发布.md). -RequireSignature turns
    # "no cert" from a warning into a hard failure for release builds.
    [switch]$RequireSignature
)

$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

if (-not $SkipBuild) {
    Write-Host "==> cargo build --release -p mw-shell"
    cargo build --release -p mw-shell
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

$exe = Join-Path $root 'target\release\mw-shell.exe'
if (-not (Test-Path $exe)) { Write-Host "missing $exe"; exit 1 }

$name = "zhisi-v$Version-win64"
$stage = Join-Path $root "dist\$name"
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Force $stage | Out-Null
New-Item -ItemType Directory -Force (Join-Path $stage 'docs') | Out-Null
New-Item -ItemType Directory -Force (Join-Path $stage 'scripts') | Out-Null
Copy-Item 'scripts\install.ps1' (Join-Path $stage 'scripts')

$stagedExe = Join-Path $stage 'MindWeaver.exe'
Copy-Item $exe $stagedExe

# Sign the staged exe (not target\release) so a failed signature never leaves a
# half-signed local build behind. No cert configured -> SKIP and keep packaging.
# Splat a hashtable: an array would be passed positionally, making "-Path" the
# value of Path rather than the parameter name.
$signArgs = @{ Path = $stagedExe }
if ($RequireSignature) { $signArgs['Require'] = $true }
& (Join-Path $PSScriptRoot 'sign.ps1') @signArgs
if ($LASTEXITCODE -ne 0) {
    Write-Host "package aborted: code signing failed (exit $LASTEXITCODE)"
    exit $LASTEXITCODE
}

foreach ($f in @('README.md', 'LICENSE', 'LICENSE-MIT', 'LICENSE-APACHE')) {
    if (Test-Path $f) { Copy-Item $f $stage }
}


$zip = Join-Path $root "dist\$name.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -CompressionLevel Optimal
$hash = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLowerInvariant()
"$hash  $name.zip" | Set-Content -Encoding ASCII "dist\$name.zip.sha256"

$size = (Get-Item $zip).Length
Write-Host "PACKAGE OK: dist\$name.zip ($size bytes)"
Write-Host "SHA256: $hash"
exit 0
