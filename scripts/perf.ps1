# Cold-start / memory gate for the 20k-node stress document.
#   powershell -File scripts\perf.ps1
# Budgets: first_ui < 1500 ms, working set < 500 MB.
param(
    [string]$Exe = 'target\release\mw-shell.exe',
    [string]$Doc = 'target\big-20k.zmind',
    [int]$FirstUiBudgetMs = 1500,
    [int]$MemoryBudgetMb = 500
)

$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

if (-not (Test-Path $Exe)) { Write-Host "missing $Exe"; exit 1 }
if (-not (Test-Path $Doc)) {
    Write-Host "generating $Doc"
    cargo run -q -p mw-shell --release --example gen_big -- 20000 $Doc
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

$log = Join-Path $root 'target\perf-gate.log'
Remove-Item $log -ErrorAction SilentlyContinue
$env:MW_PERF_LOG = $log
$p = Start-Process -FilePath $Exe -ArgumentList $Doc -PassThru
$firstUi = $null
$deadline = (Get-Date).AddSeconds(12)
while ((Get-Date) -lt $deadline) {
    Start-Sleep -Milliseconds 120
    if (Test-Path $log) {
        $line = Get-Content $log | Where-Object { $_ -match '^first_ui\t' } | Select-Object -Last 1
        if ($line) { $firstUi = [double]($line -split "`t")[1]; break }
    }
    if ($p.HasExited) { break }
}
$ws = 0
if (-not $p.HasExited) {
    $proc = Get-Process -Id $p.Id -ErrorAction SilentlyContinue
    if ($proc) { $ws = [math]::Round($proc.WorkingSet64 / 1MB, 1) }
    Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
}
Remove-Item Env:MW_PERF_LOG -ErrorAction SilentlyContinue

if ($null -eq $firstUi) {
    Write-Host "FAIL: first_ui not recorded"
    Get-Content $log -ErrorAction SilentlyContinue
    exit 1
}
Write-Host ("first_ui={0:N1} ms (budget {1})  working_set={2:N1} MB (budget {3})" -f $firstUi, $FirstUiBudgetMs, $ws, $MemoryBudgetMb)
if ($firstUi -gt $FirstUiBudgetMs) { Write-Host 'FAIL: cold start over budget'; exit 1 }
if ($ws -gt $MemoryBudgetMb) { Write-Host 'FAIL: memory over budget'; exit 1 }
Write-Host 'PERF OK'
exit 0
