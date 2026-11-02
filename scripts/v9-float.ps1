# Double-click at a fraction of the mw-shell window (proves "double click blank -> floating topic").
param(
  [double]$Fx = 0.35,
  [double]$Fy = 0.86,
  [int]$WaitMs = 900
)
Add-Type @"
using System;
using System.Runtime.InteropServices;
public class Click {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, IntPtr pid);
  [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
  [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool f);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, uint d, IntPtr e);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
"@
[Click]::SetProcessDPIAware() | Out-Null
$p = Get-Process mw-shell -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $p) { "no mw-shell"; exit 2 }
$h = $p.MainWindowHandle
$fg = [Click]::GetForegroundWindow()
if ($fg -ne $h) {
  $tidFg = [Click]::GetWindowThreadProcessId($fg, [IntPtr]::Zero)
  $tidMe = [Click]::GetCurrentThreadId()
  [Click]::AttachThreadInput($tidMe, $tidFg, $true) | Out-Null
  [Click]::SetForegroundWindow($h) | Out-Null
  [Click]::AttachThreadInput($tidMe, $tidFg, $false) | Out-Null
}
Start-Sleep -Milliseconds 400
"foreground_ok=$([Click]::GetForegroundWindow() -eq $h)"
$r = New-Object Click+RECT
[Click]::GetWindowRect($h, [ref]$r) | Out-Null
$x = [int]($r.Left + ($r.Right - $r.Left) * $Fx)
$y = [int]($r.Top + ($r.Bottom - $r.Top) * $Fy)
"window=$($r.Left),$($r.Top),$($r.Right),$($r.Bottom) click=$x,$y"
[Click]::SetCursorPos($x, $y) | Out-Null
Start-Sleep -Milliseconds 250
# 先单击一下把焦点与指针状态喂给 egui，再双击。
[Click]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
[Click]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
Start-Sleep -Milliseconds 120
for ($i = 0; $i -lt 2; $i++) {
  [Click]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
  [Click]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
  Start-Sleep -Milliseconds 60
}
Start-Sleep -Milliseconds $WaitMs
"clicked"
