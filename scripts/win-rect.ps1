param([int]$ProcessId)
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class WR {
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr ctx);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT r);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hWnd, out RECT r);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr hWnd);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
"@
[void][WR]::SetProcessDpiAwarenessContext([IntPtr](-4))
$h = (Get-Process -Id $ProcessId).MainWindowHandle
$r = New-Object WR+RECT
$c = New-Object WR+RECT
[void][WR]::GetWindowRect($h, [ref]$r)
[void][WR]::GetClientRect($h, [ref]$c)
$dpi = [WR]::GetDpiForWindow($h)
"hwnd=$h rect=$($r.Left),$($r.Top) $($r.Right - $r.Left)x$($r.Bottom - $r.Top) client=$($c.Right)x$($c.Bottom) dpi=$dpi"
