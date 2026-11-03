param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^X\d{2,3}$')]
    [string]$Round,
    [switch]$Init,
    [switch]$Check
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$roundDir = Join-Path $root "shots/x100/$Round"
$manifestPath = Join-Path $roundDir 'manifest.json'

function Fail([string]$message) {
    Write-Host "X100 EVIDENCE FAIL $Round :: $message" -ForegroundColor Red
    exit 1
}

if ($Init) {
    foreach ($sub in @('reference', 'actual', 'traces', 'golden')) {
        New-Item -ItemType Directory -Force -Path (Join-Path $roundDir $sub) | Out-Null
    }
    if (-not (Test-Path -LiteralPath $manifestPath)) {
        $manifest = [ordered]@{
            round = $Round
            status = 'in_progress'
            referenceVersion = '26.5.1107.202609082009'
            date = (Get-Date -Format 'yyyy-MM-dd')
            build = [ordered]@{ commit = $null; binarySha256 = $null }
            scenarios = @()
            tests = @()
            knownGaps = @()
        }
        $manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $manifestPath -Encoding UTF8
    }
    Write-Host "initialized $roundDir"
}

if (-not $Check) { exit 0 }

if (-not (Test-Path -LiteralPath $manifestPath)) { Fail 'manifest.json is missing' }

try {
    $manifest = Get-Content -Raw -Encoding UTF8 -LiteralPath $manifestPath | ConvertFrom-Json
} catch {
    Fail "manifest.json is invalid JSON: $($_.Exception.Message)"
}

foreach ($name in @('round', 'status', 'referenceVersion', 'date', 'scenarios', 'tests', 'knownGaps')) {
    if ($null -eq $manifest.$name) { Fail "manifest field '$name' is missing" }
}
if ($manifest.round -ne $Round) { Fail "manifest round is '$($manifest.round)'" }
if ($manifest.referenceVersion -ne '26.5.1107.202609082009') {
    Fail "referenceVersion is '$($manifest.referenceVersion)'"
}

$allowed = @('not_started', 'in_progress', 'implemented', 'verified', 'blocked', 'deferred')
if ($allowed -notcontains $manifest.status) { Fail "invalid status '$($manifest.status)'" }

$evidenceCount = 0
foreach ($scenario in @($manifest.scenarios)) {
    foreach ($field in @('id', 'reference', 'actual', 'trace', 'result', 'notes')) {
        if ($null -eq $scenario.$field) { Fail "scenario field '$field' is missing" }
    }
    $paths = @()
    $paths += @($scenario.reference)
    $paths += @($scenario.actual)
    $paths += @($scenario.trace)
    foreach ($rel in $paths) {
        if ([string]::IsNullOrWhiteSpace($rel)) { continue }
        $full = Join-Path $root $rel
        if (-not (Test-Path -LiteralPath $full)) { Fail "evidence path does not exist: $rel" }
        $evidenceCount++
    }
}

if ($manifest.status -eq 'verified' -and $evidenceCount -eq 0) {
    Fail 'verified rounds require at least one existing evidence path'
}
if ($manifest.status -eq 'verified' -and @($manifest.scenarios | Where-Object { $_.result -ne 'pass' }).Count -gt 0) {
    Fail 'verified rounds require every listed scenario result to be pass'
}

Write-Host "X100 EVIDENCE PASS $Round status=$($manifest.status) scenarios=$(@($manifest.scenarios).Count) evidencePaths=$evidenceCount"
exit 0

