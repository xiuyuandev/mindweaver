param(
    [string]$Version = "",
    [string]$OutDir = "dist",
    [switch]$SkipBuild
)

# M5 packaging (docs/08 P14): build the release shell and produce a portable
# win64 ZIP with a SHA256 file. This is not an MSI/EXE installer yet; the ZIP is
# the honest interim artifact. P14 budget is 30MB - fail the script if exceeded.

$ErrorActionPreference = "Stop"
$root = (Get-Location).Path

if (-not $Version) {
    $line = Select-String -Path (Join-Path $root "Cargo.toml") -Pattern '^version\s*=\s*"([^"]+)"' |
        Select-Object -First 1
    if (-not $line) { throw "workspace version not found" }
    $Version = $line.Matches[0].Groups[1].Value
}

if (-not $SkipBuild) {
    & cargo build -p mw-shell --release
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
}

$exe = Join-Path $root "target\release\mw-shell.exe"
if (-not (Test-Path $exe)) { throw "release exe missing: $exe" }

$stage = Join-Path $root "target\package\zhisi-$Version-win64"
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Force -Path $stage, (Join-Path $root $OutDir) | Out-Null
Copy-Item $exe (Join-Path $stage "mw-shell.exe")
Copy-Item (Join-Path $root "README.md") (Join-Path $stage "README.md")

$license = Join-Path $root "LICENSE"
if (Test-Path $license) {
    Copy-Item $license (Join-Path $stage "LICENSE")
} else {
    Write-Output "WARN no LICENSE file at repo root; package has README only"
}

$zip = Join-Path $root "$OutDir\zhisi-$Version-win64.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $stage "*") -DestinationPath $zip -CompressionLevel Optimal
$hash = Get-FileHash $zip -Algorithm SHA256
$shaFile = "$zip.sha256"
"$($hash.Hash.ToLower())  zhisi-$Version-win64.zip" | Set-Content -Encoding ASCII $shaFile

$exeBytes = (Get-Item $exe).Length
$zipBytes = (Get-Item $zip).Length
$limit = 30 * 1024 * 1024
$verdict = if ($zipBytes -le $limit) { "PASS" } else { "FAIL" }
"PACKAGE version=$Version exe_bytes=$exeBytes zip_bytes=$zipBytes zip_mb=$([Math]::Round($zipBytes / 1MB, 2)) p14_limit_mb=30 verdict=$verdict"
"PACKAGE zip=$zip"
"PACKAGE sha256=$($hash.Hash.ToLower())"
if ($zipBytes -gt $limit) { exit 1 }
