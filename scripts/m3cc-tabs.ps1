param(
    [string]$ProcessName = 'mw-shell',
    [string]$ShotDir = 'shots/qa'
)

# M3-c-c probe: multi-tab (Ctrl+T / Ctrl+Tab / Ctrl+W) real-input flow.
# Ctrl+T -> second tab; F2 rename -> dirty marker; Ctrl+Shift+Tab -> back; Ctrl+W -> close prompt.

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
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr SetFocus(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
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

$HWND_TOPMOST = [IntPtr](-1)
$HWND_NOTOPMOST = [IntPtr](-2)
$SWP_NOMOVE = 0x0002
$SWP_NOSIZE = 0x0001
$SWP_SHOWWINDOW = 0x0040
$LEFTDOWN = 0x0002
$LEFTUP = 0x0004
$VK_CONTROL = 0x11
$VK_SHIFT = 0x10
$VK_A = 0x41
$VK_V = 0x56
$VK_T = 0x54
$VK_W = 0x57
$VK_TAB = 0x09
$VK_F2 = 0x71
$VK_RETURN = 0x0D

$proc = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $proc) { Write-Error "process not running: $ProcessName"; exit 1 }
$hwnd = $proc.MainWindowHandle
if ($hwnd -eq [IntPtr]::Zero) { Write-Error "no main window yet"; exit 1 }

[void][C]::ShowWindow($hwnd, 9)
[void][C]::SetWindowPos($hwnd, $HWND_TOPMOST, 0, 0, 0, 0, ($SWP_NOMOVE -bor $SWP_NOSIZE -bor $SWP_SHOWWINDOW))
[void][C]::BringWindowToTop($hwnd)
try { (New-Object -ComObject WScript.Shell).AppActivate($proc.Id) | Out-Null } catch { }
[void][C]::SetForegroundWindow($hwnd)
[void][C]::SetFocus($hwnd)
Start-Sleep -Milliseconds 700
if ([C]::GetForegroundWindow() -ne $hwnd) {
    Write-Warning "foreground is not the app (hwnd=$hwnd); screenshots are not trustworthy"
} else {
    Write-Host "foreground ok"
}

$rect = New-Object C+Rect
[void][C]::GetClientRect($hwnd, [ref]$rect)
$origin = New-Object C+P
[void][C]::ClientToScreen($hwnd, [ref]$origin)
$cw = $rect.Right
$ch = $rect.Bottom

function Px([double]$lx, [double]$ly) {
    $scale = [C]::GetDpiForWindow($hwnd) / 96.0
    [pscustomobject]@{ x = [int]($origin.X + $scale * $lx); y = [int]($origin.Y + $scale * $ly) }
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
function Click-At($p) {
    [void][C]::SetCursorPos($p.x, $p.y)
    Start-Sleep -Milliseconds 120
    [C]::mouse_event($LEFTDOWN, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 40
    [C]::mouse_event($LEFTUP, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 450
}
function Ctrl-Key([byte]$vk) {
    [C]::keybd_event($VK_CONTROL, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [C]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [C]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [C]::keybd_event($VK_CONTROL, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 600
}
function CtrlShift-Key([byte]$vk) {
    [C]::keybd_event($VK_CONTROL, 0, 0, [IntPtr]::Zero)
    [C]::keybd_event($VK_SHIFT, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [C]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [C]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [C]::keybd_event($VK_SHIFT, 0, 2, [IntPtr]::Zero)
    [C]::keybd_event($VK_CONTROL, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 600
}
function Press-Key([byte]$vk) {
    [C]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [C]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 450
}
# Paste instead of per-key typing: the local Chinese IME turns typed keys into a
# composition state (the candidate window eats Enter). Pasting exercises the same
# TextEdit path while bypassing the IME.
function Paste-Str([string]$s) {
    Set-Clipboard -Value $s
    Start-Sleep -Milliseconds 150
    Ctrl-Key $VK_V
    Start-Sleep -Milliseconds 300
}

# Warm-up click on empty canvas absorbs the activation click and gives the canvas focus.
Click-At (Px 620 620)

Ctrl-Key $VK_T
Start-Sleep -Milliseconds 500
Save-Shot 'tabs-new'

# F2 falls back to the root when nothing is selected, so a blank tab can be renamed
# without first clicking a node. Committing with Enter dirties the tab (adds the dot).
Press-Key $VK_F2
Start-Sleep -Milliseconds 300
# U+7B2C U+4E8C U+4EFD = the two characters shown; built from code points so the
# script stays pure ASCII (PowerShell 5.1 reads a BOM-less file as the ANSI codepage
# and would mangle any literal CJK).
$rename = [string][char]0x7B2C + [char]0x4E8C + [char]0x4EFD
Paste-Str $rename
Press-Key $VK_RETURN
Start-Sleep -Milliseconds 400
Save-Shot 'tabs-dirty'

CtrlShift-Key $VK_TAB
Start-Sleep -Milliseconds 400
Save-Shot 'tabs-switched'

Ctrl-Key $VK_TAB
Start-Sleep -Milliseconds 400
Ctrl-Key $VK_W
Start-Sleep -Milliseconds 500
Save-Shot 'tabs-close-prompt'

# The prompt is centered; the middle button is "discard and close". Dismiss it and land back
# on a single tab (the strip hides, so the frame matches the pre-tab layout again).
Click-At (Px ($cw / 2 / ([C]::GetDpiForWindow($hwnd) / 96.0)) ($ch / 2 / ([C]::GetDpiForWindow($hwnd) / 96.0) + 30))
Start-Sleep -Milliseconds 500
Save-Shot 'tabs-closed'

[void][C]::SetWindowPos($hwnd, $HWND_NOTOPMOST, 0, 0, 0, 0, ($SWP_NOMOVE -bor $SWP_NOSIZE))
Write-Host 'done'
