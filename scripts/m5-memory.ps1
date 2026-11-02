param(
    [string]$Exe = "target\release\mw-shell.exe",
    [string]$Doc = "target\perf-10000.zmind",
    [int]$Minutes = 5,
    [int]$SampleSeconds = 30,
    [string]$Tag = "10k-idle"
)

# M5 steady-state memory probe (docs/08 P13).
# Opens the release app on -Doc with an isolated APPDATA, waits for the real
# main window (>= 600x400, see 14 section 9 pitfall 61), then samples private
# bytes / working set every -SampleSeconds until -Minutes elapse.
# Prints a TSV path and the final sample. All output is ASCII.

$ErrorActionPreference = "Stop"
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class M5Mem {
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT r);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
"@

$exePath = (Resolve-Path $Exe).Path
$root = Join-Path (Get-Location) "target\m5-run"
New-Item -ItemType Directory -Force -Path $root | Out-Null
$log = Join-Path $root "memory-$Tag.tsv"
Remove-Item $log -ErrorAction SilentlyContinue

$env:MW_WINDOW_STATE = "off"
Remove-Item Env:MW_STATS -ErrorAction SilentlyContinue
Remove-Item Env:MW_STATS_LOG -ErrorAction SilentlyContinue
Remove-Item Env:MW_PERF_LOG -ErrorAction SilentlyContinue
if ($Doc) {
    $env:MW_OPEN = (Resolve-Path $Doc).Path
    $env:MW_HOME = "0"
} else {
    Remove-Item Env:MW_OPEN -ErrorAction SilentlyContinue
    $env:MW_HOME = "1"
}
$env:APPDATA = Join-Path $root "appdata"

$p = Start-Process -FilePath $exePath -PassThru
$deadline = (Get-Date).AddSeconds(60)
$h = [IntPtr]::Zero
while ((Get-Date) -lt $deadline) {
    if ($p.HasExited) { throw "app exited early, code=$($p.ExitCode)" }
    $p.Refresh()
    $cand = $p.MainWindowHandle
    if ($cand -ne [IntPtr]::Zero) {
        $r = New-Object M5Mem+RECT
        [void][M5Mem]::GetWindowRect($cand, [ref]$r)
        if (($r.Right - $r.Left) -ge 600 -and ($r.Bottom - $r.Top) -ge 400) {
            $h = $cand
            break
        }
    }
    Start-Sleep -Milliseconds 150
}
if ($h -eq [IntPtr]::Zero) { throw "main window not ready in 60s" }

"minute`tprivate_mb`tworking_set_mb" | Set-Content -Path $log -Encoding ASCII
$start = Get-Date
$end = $start.AddMinutes($Minutes)
while ((Get-Date) -lt $end) {
    Start-Sleep -Seconds $SampleSeconds
    if ($p.HasExited) { throw "app exited during sampling" }
    $mem = Get-Process -Id $p.Id
    $elapsed = [Math]::Round(((Get-Date) - $start).TotalMinutes, 2)
    $privateMb = [Math]::Round($mem.PrivateMemorySize64 / 1MB, 1)
    $workingMb = [Math]::Round($mem.WorkingSet64 / 1MB, 1)
    "$elapsed`t$privateMb`t$workingMb" | Add-Content -Path $log -Encoding ASCII
}
$mem = Get-Process -Id $p.Id
$privateMb = [Math]::Round($mem.PrivateMemorySize64 / 1MB, 1)
$workingMb = [Math]::Round($mem.WorkingSet64 / 1MB, 1)
"MEMORY tag=$Tag doc=$Doc minutes=$Minutes private_mb=$privateMb working_set_mb=$workingMb"
"LOG $log"

$null = $p.CloseMainWindow()
if (-not $p.WaitForExit(8000)) {
    Stop-Process -Id $p.Id -Force
    Write-Output "WARN app did not exit; killed"
}
