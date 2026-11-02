param(
    [string]$Exe = "target\release\mw-shell.exe",
    [string]$Doc = "target\perf-5000.zmind",
    [string]$ShotDir = "shots/m5",
    [switch]$Interact,
    [switch]$StartPage,
    [int]$Seconds = 4,
    [int]$IdleSeconds = 1,
    [string]$Tag = "5k"
)

# M5 performance probe (docs/08 P1/P3/P5).
# Starts the release app on a generated .zmind, reads startup milestones from
# MW_PERF_LOG, optionally drives continuous zoom + middle-drag pan, then reads
# the last aggregate line from MW_STATS_LOG and prints a PASS/FAIL summary.
# All output is ASCII; screenshots land in -ShotDir.

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class M5 {
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr ctx);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT r);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hWnd, out RECT r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr hWnd, ref POINT p);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
    [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint idAttach, uint idAttachTo, bool attach);
    [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, int d, UIntPtr e);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr hWnd);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
}
"@
[void][M5]::SetProcessDpiAwarenessContext([IntPtr](-4))

function Focus-Window([IntPtr]$h) {
    $fg = [M5]::GetForegroundWindow()
    if ($fg -eq $h) { return }
    $fgPid = 0
    $fgThread = [M5]::GetWindowThreadProcessId($fg, [ref]$fgPid)
    [void][M5]::AttachThreadInput([M5]::GetCurrentThreadId(), $fgThread, $true)
    [void][M5]::BringWindowToTop($h)
    [void][M5]::SetForegroundWindow($h)
    [void][M5]::AttachThreadInput([M5]::GetCurrentThreadId(), $fgThread, $false)
    Start-Sleep -Milliseconds 500
}

function Read-Markers([string]$path) {
    $map = @{}
    if (-not (Test-Path $path)) { return $map }
    foreach ($line in Get-Content $path) {
        $parts = $line -split "`t"
        if ($parts.Count -eq 2) { $map[$parts[0]] = [double]$parts[1] }
    }
    return $map
}

$exePath = (Resolve-Path $Exe).Path
$docPath = (Resolve-Path $Doc).Path
$root = Join-Path (Get-Location) "target\m5-run"
New-Item -ItemType Directory -Force -Path $root, $ShotDir | Out-Null
$perfLog = Join-Path $root "startup-$Tag.txt"
$statsLog = Join-Path $root "stats-$Tag.txt"
$shot = Join-Path $ShotDir "m5-perf-$Tag.png"
Remove-Item $perfLog, $statsLog -ErrorAction SilentlyContinue

$env:MW_STATS = "1"
$env:MW_STATS_LOG = $statsLog
$env:MW_PERF_LOG = $perfLog
$env:MW_WINDOW_STATE = "off"
if ($StartPage) {
    Remove-Item Env:MW_OPEN -ErrorAction SilentlyContinue
    $env:MW_HOME = "1"
} else {
    $env:MW_OPEN = $docPath
    $env:MW_HOME = "0"
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
        $r = New-Object M5+RECT
        [void][M5]::GetWindowRect($cand, [ref]$r)
        $w = $r.Right - $r.Left
        $hh = $r.Bottom - $r.Top
        if ($w -ge 600 -and $hh -ge 400) { $h = $cand; break }
    }
    Start-Sleep -Milliseconds 150
}
if ($h -eq [IntPtr]::Zero) { throw "main window not ready in 60s" }

# first_canvas / first_ui is the interactive endpoint for P1/P2.
$endpoint = if ($StartPage) { "first_ui" } else { "first_canvas" }
$deadline = (Get-Date).AddSeconds(60)
while ((Get-Date) -lt $deadline) {
    $m = Read-Markers $perfLog
    if ($m.ContainsKey($endpoint)) { break }
    Start-Sleep -Milliseconds 100
}
$m = Read-Markers $perfLog
if (-not $m.ContainsKey($endpoint)) { throw "$endpoint marker missing" }

Focus-Window $h
Start-Sleep -Milliseconds 300

$r = New-Object M5+RECT
[void][M5]::GetWindowRect($h, [ref]$r)
$cx = [int](($r.Left + $r.Right) / 2)
$cy = [int](($r.Top + $r.Bottom) / 2)
# Keep the gesture inside the canvas below the 56 DIP titlebar (~84 px at 150%).
$cy = $cy + 40

if ($Interact -and -not $StartPage) {
    [void][M5]::SetCursorPos($cx, $cy)
    Start-Sleep -Milliseconds 200
    $until = (Get-Date).AddSeconds($Seconds)
    $i = 0
    while ((Get-Date) -lt $until) {
        # Alternate one notch in / one out: the camera stays near fit, so the
        # 5k-visible-80% acceptance condition holds for the whole run.
        $delta = if (($i % 2) -eq 0) { 120 } else { -120 }
        [M5]::mouse_event(0x0800, 0, 0, $delta, [UIntPtr]::Zero)
        $i++
        Start-Sleep -Milliseconds 45
    }
    # Middle-drag pan for the second half of the budget.
    [void][M5]::SetCursorPos($cx, $cy)
    [M5]::mouse_event(0x0020, 0, 0, 0, [UIntPtr]::Zero)
    for ($k = 0; $k -lt 40; $k++) {
        $ang = $k * 0.16
        $x = $cx + [int]([Math]::Cos($ang) * 90)
        $y = $cy + [int]([Math]::Sin($ang) * 55)
        [void][M5]::SetCursorPos($x, $y)
        Start-Sleep -Milliseconds 40
    }
    [M5]::mouse_event(0x0040, 0, 0, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 600
}

# Screenshot while the stats card still shows the last continuous-run summary.
& (Join-Path $PSScriptRoot "win-capture.ps1") -ProcessId $p.Id -OutFile $shot | Write-Output

Start-Sleep -Milliseconds (800 + $IdleSeconds * 1000)
$mem = Get-Process -Id $p.Id
$privateMb = [Math]::Round($mem.PrivateMemorySize64 / 1MB, 1)
$workingMb = [Math]::Round($mem.WorkingSet64 / 1MB, 1)

$m = Read-Markers $perfLog
$p1 = if ($m.ContainsKey($endpoint)) { $m[$endpoint] } else { -1 }
if ($StartPage) {
    "STARTUP tag=$Tag process_to_first_ui_ms=$([Math]::Round($p1,1))"
} else {
    $p3 = if ($m.ContainsKey("doc_open_start") -and $m.ContainsKey("first_canvas")) {
        $m["first_canvas"] - $m["doc_open_start"]
    } else { -1 }
    "STARTUP tag=$Tag process_to_first_frame_ms=$([Math]::Round($p1,1)) open_to_first_frame_ms=$([Math]::Round($p3,1))"
}

if ($Interact -and -not $StartPage -and (Test-Path $statsLog)) {
    $rows = @(Get-Content $statsLog | Where-Object { $_ -notmatch '^#' -and $_.Trim().Length -gt 0 })
    if ($rows.Count -eq 0) { throw "stats log has no aggregate rows" }
    $f = $rows[-1] -split "`t"
    $fps = [double]$f[1]; $w95 = [double]$f[3]; $u50 = [double]$f[4]; $u95 = [double]$f[5]
    $vis = [int]$f[6]; $total = [int]$f[7]; $zoom = [double]$f[8]; $low = [int]$f[9]
    $p5 = if ($fps -ge 55.0) { "PASS" } else { "FAIL" }
    $ui = if ($u95 -le 8.0) { "PASS" } else { "FAIL" }
    "P5 tag=$Tag fps=$([Math]::Round($fps,1)) wall_p95_ms=$([Math]::Round($w95,2)) ui_p50_ms=$([Math]::Round($u50,2)) ui_p95_ms=$([Math]::Round($u95,2)) visible=$vis total=$total zoom=$zoom low_render=$low verdict=$p5 ui95_budget8ms=$ui"
} else {
    "P5 tag=$Tag skipped (no interaction)"
}
"MEMORY tag=$Tag private_mb=$privateMb working_set_mb=$workingMb"
"SHOT $shot"

$null = $p.CloseMainWindow()
if (-not $p.WaitForExit(8000)) {
    Stop-Process -Id $p.Id -Force
    Write-Output "WARN app did not exit; killed"
}
