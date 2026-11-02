param(
    [Parameter(Mandatory = $true)][string]$ProcessName,
    [string]$ShotDir = 'shots/render'
)

# M1b-3 canvas probe: real mouse input for select / collapse / pan / ctrl-wheel zoom.
# Coordinates are logical offsets inside the client area and match the demo document
# in crates/mw-shell/src/canvas.rs (DirectWrite node ~945,275; collapse dot ~382,291 (client y, titlebar included)).
# The process is made DPI-aware so client rect, cursor and screen capture all speak
# physical pixels; logical offsets are scaled by GetDpiForWindow/96.

Add-Type -AssemblyName System.Drawing

if (-not ('C' -as [type])) {
    Add-Type -Language CSharp @'
using System;
using System.Runtime.InteropServices;
public static class C {
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out Rect r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref P p);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint d, int e, int dw, IntPtr r);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint f, IntPtr extra);
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct P { public int X, Y; }
}
'@
}
[void][C]::SetProcessDPIAware()

$LEFTDOWN = 0x0002
$LEFTUP = 0x0004
$MIDDLEDOWN = 0x0020
$MIDDLEUP = 0x0040
$WHEEL = 0x0800
$VK_CONTROL = 0x11
$VK_OEM_PLUS = 0xBB

$proc = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $proc) { Write-Error "process not running: $ProcessName"; exit 1 }
$hwnd = $proc.MainWindowHandle
if ($hwnd -eq [IntPtr]::Zero) { Write-Error "no main window yet"; exit 1 }

[void][C]::ShowWindow($hwnd, 9)
[void][C]::SetForegroundWindow($hwnd)
Start-Sleep -Milliseconds 500

$rect = New-Object C+Rect
[void][C]::GetClientRect($hwnd, [ref]$rect)
$origin = New-Object C+P
[void][C]::ClientToScreen($hwnd, [ref]$origin)
$scale = [C]::GetDpiForWindow($hwnd) / 96.0
$cw = $rect.Right
$ch = $rect.Bottom
Write-Host ("client={0}x{1} origin={2},{3} scale={4}" -f $cw, $ch, $origin.X, $origin.Y, $scale)

function Px([double]$lx, [double]$ly) {
    [pscustomobject]@{ x = [int]($origin.X + $scale * $lx); y = [int]($origin.Y + $scale * $ly) }
}

function Move-To($p) {
    [void][C]::SetCursorPos($p.x, $p.y)
    Start-Sleep -Milliseconds 120
}

function Click-At($p) {
    Move-To $p
    [C]::mouse_event($LEFTDOWN, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 40
    [C]::mouse_event($LEFTUP, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 500
}

function Drag($a, $b, [int]$button = $LEFTDOWN, [int]$up = $LEFTUP, [int]$steps = 14) {
    Move-To $a
    [C]::mouse_event($button, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 100
    for ($i = 1; $i -le $steps; $i++) {
        $k = $i / $steps
        [void][C]::SetCursorPos([int]($a.x + ($b.x - $a.x) * $k), [int]($a.y + ($b.y - $a.y) * $k))
        Start-Sleep -Milliseconds 25
    }
    [C]::mouse_event($up, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 500
}

function Zoom([int]$notches = 3) {
    # Ctrl+= avoids relying on wheel delivery through the remote session.
    [C]::keybd_event($VK_CONTROL, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 80
    for ($i = 0; $i -lt $notches; $i++) {
        [C]::keybd_event($VK_OEM_PLUS, 0, 0, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 40
        [C]::keybd_event($VK_OEM_PLUS, 0, 2, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 120
    }
    [C]::keybd_event($VK_CONTROL, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 500
}

function Save-Shot([string]$name) {
    if (-not (Test-Path -LiteralPath $ShotDir)) { New-Item -ItemType Directory -Path $ShotDir | Out-Null }
    $bmp = New-Object System.Drawing.Bitmap $cw, $ch, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($origin.X, $origin.Y, 0, 0, (New-Object System.Drawing.Size $cw, $ch))
    $out = Join-Path $ShotDir "$name.png"
    if (Test-Path -LiteralPath $out) { Remove-Item -LiteralPath $out -Force }
    $bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    Write-Host "  shot -> $out"
}

# Park the pointer on empty canvas so no hover ring pollutes the first shot.
Move-To (Px 700 620)
Save-Shot 'm1b3-initial'

# 1) Click a leaf: selection ring should wrap "DirectWrite 度量".
Click-At (Px 945 275)
Save-Shot 'm1b3-selected'

# 2) Click the collapse dot left of "纯 Rust 原生": its two children + summary vanish,
#    the node keeps the selection ring.
Click-At (Px 382 291)
Save-Shot 'm1b3-collapsed'

# 3) Drag empty canvas with the MIDDLE button: content follows the pointer (pan).
#    Since M2b-2 a plain left drag on empty canvas box-selects instead, so panning
#    moved to middle-drag / space+drag (docs/06 section 5).
Move-To (Px 700 620)
Drag (Px 700 620) (Px 850 680) $MIDDLEDOWN $MIDDLEUP
Save-Shot 'm1b3-panned'

# 4) Ctrl+= at the canvas center: keyboard zoom fallback for the wheel path.
Move-To (Px 600 400)
Zoom 3
Save-Shot 'm1b3-zoomed'

Move-To (Px 5 5)
Write-Host 'done'
