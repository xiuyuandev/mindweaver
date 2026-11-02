param(
    [string]$Exe = "target/release/mw-shell.exe",
    [string]$ShotDir = "shots/qa"
)

# v3 core polish probe (real input, release build):
#   F8 close outline -> click root -> RIGHT/LEFT arrow navigation -> F9 open/close inspector.
# Asserts: arrow keys move the selection; opening the inspector does not shift the map.
# ASCII only (PowerShell 5.1 reads BOM-less files as ANSI).

Add-Type -AssemblyName System.Drawing
Add-Type -ReferencedAssemblies System.Drawing -Language CSharp @'
using System;
using System.Drawing;
using System.Runtime.InteropServices;
public static class V3 {
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref P p);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, int data, IntPtr extra);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint f, IntPtr extra);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct P { public int X, Y; }
    public static int Diff(Bitmap a, Bitmap b, int x0, int y0, int x1, int y1, int tol) {
        int n = 0;
        for (int y = y0; y < y1; y++) {
            for (int x = x0; x < x1; x++) {
                Color pa = a.GetPixel(x, y), pb = b.GetPixel(x, y);
                if (Math.Abs(pa.R - pb.R) > tol || Math.Abs(pa.G - pb.G) > tol || Math.Abs(pa.B - pb.B) > tol) n++;
            }
        }
        return n;
    }
}
'@
[void][V3]::SetProcessDPIAware()

$env:MW_HOME = "0"
$env:MW_WINDOW_STATE = "off"
$env:MW_STATS = $null
$tmp = Join-Path $env:TEMP ("mw-v3-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
$env:APPDATA = $tmp

$proc = Start-Process -FilePath $Exe -PassThru
$hwnd = [IntPtr]::Zero
for ($i = 0; $i -lt 60; $i++) {
    Start-Sleep -Milliseconds 250
    $proc.Refresh()
    $h = $proc.MainWindowHandle
    if ($h -ne [IntPtr]::Zero) {
        $r = New-Object V3+RECT
        [void][V3]::GetClientRect($h, [ref]$r)
        if ($r.Right -ge 600 -and $r.Bottom -ge 400) { $hwnd = $h; break }
    }
}
if ($hwnd -eq [IntPtr]::Zero) { Write-Error "main window never appeared"; exit 1 }

[void][V3]::ShowWindow($hwnd, 9)
[void][V3]::BringWindowToTop($hwnd)
try { (New-Object -ComObject WScript.Shell).AppActivate($proc.Id) | Out-Null } catch { }
[void][V3]::SetForegroundWindow($hwnd)
Start-Sleep -Milliseconds 900
if ([V3]::GetForegroundWindow() -ne $hwnd) { Write-Warning "app is not foreground" }

$rect = New-Object V3+RECT
[void][V3]::GetClientRect($hwnd, [ref]$rect)
$origin = New-Object V3+P
[void][V3]::ClientToScreen($hwnd, [ref]$origin)
$cw = $rect.Right
$ch = $rect.Bottom
$scale = [V3]::GetDpiForWindow($hwnd) / 96.0
Write-Host ("client=" + $cw + "x" + $ch + " scale=" + $scale)

if (-not (Test-Path -LiteralPath $ShotDir)) { New-Item -ItemType Directory -Path $ShotDir | Out-Null }

function Shot([string]$name) {
    $bmp = New-Object System.Drawing.Bitmap $cw, $ch, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($origin.X, $origin.Y, 0, 0, (New-Object System.Drawing.Size $cw, $ch))
    $full = (Resolve-Path -LiteralPath $ShotDir).Path + "\" + $name + ".png"
    if (Test-Path -LiteralPath $full) { Remove-Item -LiteralPath $full -Force }
    $bmp.Save($full, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose()
    Write-Host ("  shot -> " + $full)
    return $bmp
}
function Press([byte]$vk) {
    [V3]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 50
    [V3]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 420
}
function Click([int]$x, [int]$y) {
    [void][V3]::SetCursorPos($x, $y)
    Start-Sleep -Milliseconds 120
    [V3]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 50
    [V3]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 350
}

$VK_F8 = 0x77; $VK_F9 = 0x78; $VK_LEFT = 0x25; $VK_RIGHT = 0x27; $VK_HOME = 0x24

# Close the outline, re-fit, then click empty canvas: that focuses the canvas and
# clears the selection (a bare Esc would drop focus, see 14 section 9 pitfall 4).
Press $VK_F8
Start-Sleep -Milliseconds 400
Press $VK_HOME
Click ($origin.X + [int](90 * $scale)) ($origin.Y + [int](150 * $scale))
Press $VK_RIGHT
$root = Shot "v3-root"

Press $VK_RIGHT
$right = Shot "v3-arrow-right"
Press $VK_LEFT
$left = Shot "v3-arrow-left"

# Inspector open/close: the map must not slide when the canvas narrows.
$before = Shot "v3-before-inspector"
Press $VK_F9
$open = Shot "v3-inspector"
Press $VK_F9
$closed = Shot "v3-closed"

# Compare the map area only: skip the bottom-right minimap, which legitimately
# re-anchors to the canvas edge when the inspector opens.
$y0 = [int](80 * $scale)
$y1 = [int]($ch * 0.80)
$x1 = [int]($cw * 0.75)
$shift = [V3]::Diff($before, $open, 0, $y0, $x1, $y1, 6)
$total = $x1 * ($y1 - $y0)
$closeDiff = [V3]::Diff($before, $closed, 0, $y0, $x1, $y1, 6)
Write-Host ("panel_shift_diff=" + $shift + "/" + $total + " (" + [math]::Round(100.0 * $shift / $total, 3) + "%)")
Write-Host ("closed_diff=" + $closeDiff + "/" + $total)

# Arrow navigation: RIGHT must move the selection; LEFT must land back on the root.
$arrowDiff = [V3]::Diff($root, $right, 0, $y0, $x1, $y1, 6)
$backDiff = [V3]::Diff($root, $left, 0, $y0, $x1, $y1, 6)
Write-Host ("arrow_right_diff=" + $arrowDiff)
Write-Host ("arrow_back_diff=" + $backDiff)

$root.Dispose(); $before.Dispose(); $open.Dispose(); $closed.Dispose(); $right.Dispose(); $left.Dispose()

[void][V3]::SetWindowPos($hwnd, [IntPtr](-2), 0, 0, 0, 0, (0x0002 -bor 0x0001))
Stop-Process -Id $proc.Id -Force
Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
Write-Host "done"
