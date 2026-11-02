# Authenticode signing hook. No certificate is bundled: without credentials this
# script reports SKIP and exits 0, so packaging keeps working before the cert arrives.
#
# Credentials (environment variables are preferred; never commit them):
#   MW_SIGN_PFX            path to a .pfx code-signing certificate
#   MW_SIGN_PFX_PASSWORD   password for the .pfx (optional if the pfx has none)
#   MW_SIGN_THUMBPRINT     SHA1 thumbprint of a cert in CurrentUser\My (alternative)
#   MW_SIGN_TIMESTAMP_URL  RFC3161 timestamp service (default DigiCert)
#
# Usage:
#   powershell -File scripts/sign.ps1 -Path dist\...\MindWeaver.exe
#   powershell -File scripts/sign.ps1 -Path target\release\mw-shell.exe -Thumbprint ABCD...
#   powershell -File scripts/sign.ps1 -Path <exe> -Require
#   powershell -File scripts/sign.ps1 -Path <exe> -DryRun
#
# Exit codes: 0 signed or intentionally skipped; 1 missing credentials with
# -Require, signing failure, or verification failure.
param(
    [Parameter(Mandatory = $true)][string[]]$Path,
    [string]$Pfx = '',
    [string]$Thumbprint = '',
    [string]$Password = '',
    [string]$TimestampUrl = '',
    [switch]$Require,
    [switch]$DryRun,
    [switch]$NoTimestamp,
    [switch]$NoVerify
)

$ErrorActionPreference = 'Continue'

if (-not $Pfx) { $Pfx = $env:MW_SIGN_PFX }
if (-not $Thumbprint) { $Thumbprint = $env:MW_SIGN_THUMBPRINT }
if (-not $Password) { $Password = $env:MW_SIGN_PFX_PASSWORD }
if (-not $TimestampUrl) {
    $TimestampUrl = $env:MW_SIGN_TIMESTAMP_URL
    if (-not $TimestampUrl) { $TimestampUrl = 'http://timestamp.digicert.com' }
}

foreach ($p in $Path) {
    if (-not (Test-Path $p)) {
        Write-Host "SIGN FAIL: missing file $p"
        exit 1
    }
}

function Find-SignTool {
    $cmd = Get-Command signtool.exe -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    $roots = @(
        (Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'),
        (Join-Path $env:ProgramFiles 'Windows Kits\10\bin')
    )
    foreach ($root in $roots) {
        if (-not (Test-Path $root)) { continue }
        $hit = Get-ChildItem $root -Filter signtool.exe -Recurse -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -match '\\x64\\' } |
            Sort-Object FullName -Descending |
            Select-Object -First 1
        if ($hit) { return $hit.FullName }
    }
    return $null
}

$signtool = Find-SignTool
if (-not $signtool) {
    Write-Host 'SIGN SKIP: signtool.exe not found (install the Windows SDK signing tools).'
    if ($Require) { exit 1 }
    exit 0
}

if (-not $Pfx -and -not $Thumbprint) {
    Write-Host 'SIGN SKIP: no certificate configured (set MW_SIGN_PFX or MW_SIGN_THUMBPRINT).'
    Write-Host '           Portable ZIP stays unsigned; see docs/20 (signing and release).md.'
    if ($Require) { exit 1 }
    exit 0
}
if ($Pfx -and $Thumbprint) {
    Write-Host 'SIGN FAIL: set either -Pfx or -Thumbprint, not both.'
    exit 1
}

$signArgs = @('sign', '/fd', 'SHA256')
if (-not $NoTimestamp) {
    $signArgs += @('/tr', $TimestampUrl, '/td', 'SHA256')
}
if ($Pfx) {
    $signArgs += @('/f', $Pfx)
    if ($Password) { $signArgs += @('/p', $Password) }
} else {
    $signArgs += @('/sha1', $Thumbprint)
}
$signArgs += $Path

if ($DryRun) {
    $shown = @()
    for ($i = 0; $i -lt $signArgs.Count; $i++) {
        if ($signArgs[$i] -eq '/p') {
            $shown += '/p'
            $shown += '******'
            $i++
        } else {
            $shown += $signArgs[$i]
        }
    }
    Write-Host "SIGN DRY RUN: $signtool $($shown -join ' ')"
    exit 0
}

Write-Host "signing $($Path.Count) file(s) with $signtool"
& $signtool @signArgs
if ($LASTEXITCODE -ne 0) {
    Write-Host "SIGN FAIL: signtool exit $LASTEXITCODE"
    exit $LASTEXITCODE
}

if (-not $NoVerify) {
    foreach ($p in $Path) {
        & $signtool verify /pa /v $p | Out-Null
        if ($LASTEXITCODE -ne 0) {
            Write-Host "SIGN FAIL: verify /pa failed for $p (self-signed certs are not trusted by /pa)"
            Write-Host '           For a local smoke test use -NoVerify; production must use a trusted cert.'
            exit $LASTEXITCODE
        }
        $sig = Get-AuthenticodeSignature $p
        Write-Host ("SIGN OK: {0} status={1} signer={2}" -f $p, $sig.Status, $sig.SignerCertificate.Subject)
    }
}

Write-Host 'SIGN DONE'
exit 0
