param(
    [int]$ProcessId,
    [int]$X,
    [int]$Y,
    [string]$OutFile
)
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class HV {
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr ctx);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
"@
[void][HV]::SetProcessDpiAwarenessContext([IntPtr](-4))
$p = Get-Process -Id $ProcessId -ErrorAction Stop
$h = $p.MainWindowHandle
if ($h -eq [IntPtr]::Zero) { throw "no main window" }
[void][HV]::SetForegroundWindow($h)
Start-Sleep -Milliseconds 400
[void][HV]::SetCursorPos($X, $Y)
Start-Sleep -Milliseconds 1200
$r = New-Object HV+RECT
[void][HV]::GetWindowRect($h, [ref]$r)
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
