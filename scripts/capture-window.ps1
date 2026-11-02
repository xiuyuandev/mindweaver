# Capture a top-level window by process name via PrintWindow.
# Usage: powershell -File capture-window.ps1 -Process Xmind -Out C:\path\shot.png [-Index 0] [-List]
param(
  [string]$Process = "Xmind",
  [string]$Out = "",
  [int]$Index = 0,
  [switch]$List
)
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
using System.Text;
public class WinCap {
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr lp);
  public delegate bool EnumProc(IntPtr h, IntPtr lp);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder sb, int n);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
  [DllImport("user32.dll")] public static extern IntPtr GetParent(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h, uint flags);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
"@
[WinCap]::SetProcessDPIAware() | Out-Null

$targets = @()
$pids = @(Get-Process -Name $Process -ErrorAction SilentlyContinue | ForEach-Object { $_.Id })
$cb = [WinCap+EnumProc]{
  param($h, $lp)
  $wpid = 0
  [WinCap]::GetWindowThreadProcessId($h, [ref]$wpid) | Out-Null
  if ($pids -contains [int]$wpid) {
    if (-not [WinCap]::IsWindowVisible($h)) { return $true }
    $sb = New-Object System.Text.StringBuilder 512
    [WinCap]::GetWindowTextW($h, $sb, 512) | Out-Null
    $r = New-Object WinCap+RECT
    [WinCap]::GetWindowRect($h, [ref]$r) | Out-Null
    $w = $r.Right - $r.Left; $hh = $r.Bottom - $r.Top
    if ($w -gt 200 -and $hh -gt 200) {
      $script:targets += [pscustomobject]@{ Hwnd = $h; Pid = [int]$wpid; Title = $sb.ToString(); W = $w; H = $hh; L = $r.Left; T = $r.Top; Dpi = [WinCap]::GetDpiForWindow($h) }
    }
  }
  return $true
}
[WinCap]::EnumWindows($cb, [IntPtr]::Zero) | Out-Null

if ($List -or -not $Out) {
  $targets | ForEach-Object { "hwnd={0} pid={1} {2}x{3} at {4},{5} dpi={6} title='{7}'" -f $_.Hwnd, $_.Pid, $_.W, $_.H, $_.L, $_.T, $_.Dpi, $_.Title }
  if (-not $Out) { exit 0 }
}
if ($Index -ge $targets.Count) { "no window at index $Index (count=$($targets.Count))"; exit 2 }
$t = $targets[$Index]
"capturing hwnd=$($t.Hwnd) title='$($t.Title)' $($t.W)x$($t.H) dpi=$($t.Dpi)"
$bmp = New-Object System.Drawing.Bitmap($t.W, $t.H)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $g.GetHdc()
$ok = [WinCap]::PrintWindow($t.Hwnd, $hdc, 2)
$g.ReleaseHdc($hdc)
$g.Dispose()
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
"printwindow=$ok saved=$Out bytes=$((Get-Item $Out).Length)"
