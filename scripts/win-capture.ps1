param(
    [int]$ProcessId,
    [string]$OutFile = "shots/qa/window.png",
    [int]$ClickX = -1,
    [int]$ClickY = -1,
    [int]$WheelDelta = 0
)
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class WC {
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr ctx);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT r);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
    [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint idAttach, uint idAttachTo, bool attach);
    [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, int d, UIntPtr e);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
"@
[void][WC]::SetProcessDpiAwarenessContext([IntPtr](-4))

$proc = Get-Process -Id $ProcessId -ErrorAction Stop
$h = $proc.MainWindowHandle
if ($h -eq [IntPtr]::Zero) { throw "no main window" }
if ([WC]::IsIconic($h)) { [void][WC]::ShowWindow($h, 9) }

$fg = [WC]::GetForegroundWindow()
$fgPid = 0
$fgThread = [WC]::GetWindowThreadProcessId($fg, [ref]$fgPid)
if ($fg -ne $h) {
    [void][WC]::AttachThreadInput([WC]::GetCurrentThreadId(), $fgThread, $true)
    [void][WC]::BringWindowToTop($h)
    [void][WC]::SetForegroundWindow($h)
    [void][WC]::AttachThreadInput([WC]::GetCurrentThreadId(), $fgThread, $false)
    Start-Sleep -Milliseconds 600
}
Start-Sleep -Milliseconds 400

if ($ClickX -ge 0 -and $ClickY -ge 0) {
    [void][WC]::SetCursorPos($ClickX, $ClickY)
    Start-Sleep -Milliseconds 120
    if ($WheelDelta -ne 0) {
        [WC]::mouse_event(0x0800, 0, 0, [int]$WheelDelta, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 700
    } else {
        [WC]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 90
        [WC]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 900
    }
}

$r = New-Object WC+RECT
[void][WC]::GetWindowRect($h, [ref]$r)
$w = $r.Right - $r.Left
$hh = $r.Bottom - $r.Top
$bmp = New-Object System.Drawing.Bitmap $w, $hh
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size)
$dir = Split-Path $OutFile
if ($dir) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
$bmp.Save((Join-Path (Get-Location) $OutFile), [System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
Write-Output "saved $OutFile rect=$($r.Left),$($r.Top) ${w}x${hh}"
