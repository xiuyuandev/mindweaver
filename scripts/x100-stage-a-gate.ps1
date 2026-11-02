param(
    [switch]$SkipFullTests,
    [switch]$SkipGolden,
    [switch]$SkipClippy
)

$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$log = New-Object System.Collections.Generic.List[string]
$failures = New-Object System.Collections.Generic.List[string]

function Say([string]$text) {
    Write-Host $text
    $log.Add($text)
}

function Run-Step([string]$name, [scriptblock]$body) {
    Say ""
    Say "==> $name"
    $global:LASTEXITCODE = 0
    try {
        $output = & $body 2>&1
        $code = $LASTEXITCODE
        $ok = $?
    } catch {
        $output = @($_)
        $code = 1
        $ok = $false
    }
    foreach ($line in $output) { Say ([string]$line) }
    if (-not $ok -or $code -ne 0) {
        $failures.Add("$name (exit $code)")
        Say "FAIL $name"
        return $false
    }
    Say "PASS $name"
    return $true
}

function Get-TestCounts($output) {
    $passed = 0
    $failed = 0
    foreach ($line in $output) {
        if ([string]$line -match '^test result: ok\.\s+(\d+) passed;\s+(\d+) failed') {
            $passed += [int]$Matches[1]
            $failed += [int]$Matches[2]
        }
    }
    return @{ Passed = $passed; Failed = $failed }
}

Say "X100 Stage A machine gate"
Say "root=$root"
Say "started=$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss zzz')"

$null = Run-Step 'reference version and isolation status' {
    & powershell -NoProfile -ExecutionPolicy Bypass -File scripts/x100-xmind-capture.ps1 -Mode check
    if ($LASTEXITCODE -ne 0) { throw 'reference isolation check failed' }
}

$null = Run-Step 'X01-X10 evidence manifests' {
    $code = 0
    foreach ($i in 1..10) {
        $round = 'X{0:D2}' -f $i
        & powershell -NoProfile -ExecutionPolicy Bypass -File scripts/x100-evidence.ps1 -Round $round -Check
        if ($LASTEXITCODE -ne 0) { $code = $LASTEXITCODE }
    }
    if ($code -ne 0) { throw "evidence check failed: $code" }
}

$null = Run-Step 'reference corpus metadata' {
    $index = Get-Content -Raw -Encoding UTF8 shots/reference/xmind-26.5.1107/corpus.json | ConvertFrom-Json
    if ($index.version -ne '26.5.1107.202609082009') { throw "reference version mismatch: $($index.version)" }
    if ([int]$index.count -lt 50) { throw "reference corpus too small: $($index.count)" }
    if (@($index.files).Count -ne [int]$index.count) { throw 'reference corpus count mismatch' }
    Write-Output "reference images=$($index.count)"
}

$null = Run-Step 'X04 contact sheets' {
    $sheets = @(Get-ChildItem shots/x100/X04 -File -Filter *.png)
    if ($sheets.Count -lt 10) { throw "contact sheets missing: $($sheets.Count) < 10" }
    Write-Output "contact sheets=$($sheets.Count)"
}

$null = Run-Step 'committed fixtures present' {
    foreach ($path in @('tests/fixtures/x100/tiny-cn.zmind', 'tests/fixtures/x100/complex-style.zmind', 'tests/fixtures/x100/manifest.json')) {
        if (-not (Test-Path -LiteralPath $path)) { throw "missing $path" }
    }
    Write-Output 'tiny-cn + complex-style + manifest present'
}

$null = Run-Step 'PowerShell syntax' {
    foreach ($file in Get-ChildItem scripts/x100-*.ps1) {
        $null = [scriptblock]::Create((Get-Content -Raw -Encoding UTF8 $file.FullName))
        Write-Output "syntax-ok $($file.Name)"
    }
}

$null = Run-Step 'cargo fmt --check' { cargo fmt --all -- --check }

if (-not $SkipClippy) {
    $null = Run-Step 'clippy with zero new X100 warnings' {
        $output = & cargo clippy -p mw-shell -p mw-format --all-targets 2>&1
        $code = $LASTEXITCODE
        $output | ForEach-Object { Write-Output $_ }
        $new = @($output | Where-Object { [string]$_ -match '(x100_fixtures\.rs|golden_input\.rs)' })
        if ($new.Count -gt 0) {
            $new | ForEach-Object { Write-Output "X100-warning $_" }
            throw 'new X100 clippy warning'
        }
        if ($code -ne 0) { throw "clippy failed: $code" }
    }
}

$null = Run-Step 'format fixture tests' {
    cargo test -p mw-format --test x100_fixtures -- --test-threads=1
}

$null = Run-Step 'golden input tests' {
    cargo test -p mw-shell golden_input::tests -- --test-threads=1
}

if (-not $SkipFullTests) {
    Say ""
    Say '==> full serial workspace tests'
    $output = & cargo test --workspace -- --test-threads=1 2>&1
    $code = $LASTEXITCODE
    $counts = Get-TestCounts $output
    foreach ($line in ($output | Select-Object -Last 30)) { Say ([string]$line) }
    Say "tests passed=$($counts.Passed) failed=$($counts.Failed) exit=$code"
    if ($code -ne 0 -or $counts.Failed -ne 0 -or $counts.Passed -lt 681) {
        $failures.Add("full tests (passed $($counts.Passed), failed $($counts.Failed), exit $code)")
        Say 'FAIL full serial workspace tests'
    } else {
        Say 'PASS full serial workspace tests'
    }
}

if (-not $SkipGolden) {
    foreach ($i in 1..2) {
        $null = Run-Step "default golden run $i" { cargo golden --tol 0 }
    }
}

Say ""
if ($failures.Count -gt 0) {
    Say "MACHINE GATE FAIL: $($failures -join '; ')"
    $code = 1
} else {
    Say 'MACHINE GATE PASS; HUMAN X10 REVIEW IS STILL REQUIRED'
    $code = 0
}
Say "finished=$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss zzz')"

$logDir = Join-Path $root 'target/x100'
New-Item -ItemType Directory -Force -Path $logDir | Out-Null
$logPath = Join-Path $logDir 'stage-a-gate.log'
$log | Set-Content -LiteralPath $logPath -Encoding UTF8
Write-Host "log=$logPath"
exit $code

