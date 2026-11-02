param(
    [string]$ProcessName = 'mw-shell',
    [string]$ShotDir = 'shots/m2c'
)

# M2c probe: real keyboard/mouse through the shortcut surface.
# Coordinates are logical client px (1080x720 at 150% DPI), matching the demo doc.
#
# Every document mutation is undone before the next case, so later screenshots
# stay comparable. Motion shots are taken right after the key with short sleeps;
# they may land early or late, so treat them as evidence, not pixel baselines.

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

$VK_SHIFT = 0x10
$VK_CONTROL = 0x11
$VK_ALT = 0x12
$VK_ESCAPE = 0x1B
$VK_ENTER = 0x0D
$VK_UP = 0x26
$VK_DOWN = 0x28
$VK_LEFT = 0x25
$VK_RIGHT = 0x27
$VK_F1 = 0x70
$VK_F8 = 0x77
$VK_F9 = 0x78
$VK_F11 = 0x7A
$VK_0 = 0x30
$VK_1 = 0x31
$VK_2 = 0x32
$VK_C = 0x43
$VK_D = 0x44
$VK_P = 0x50
$VK_V = 0x56
$VK_X = 0x58
$VK_Z = 0x5A
$VK_OEM_2 = 0xBF

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
    Write-Warning "foreground is not MindWeaver, input and screenshots are unreliable"
} else {
    Write-Host "foreground ok"
}

$rect = New-Object C+Rect
[void][C]::GetClientRect($hwnd, [ref]$rect)
$origin = New-Object C+P
[void][C]::ClientToScreen($hwnd, [ref]$origin)
$scale = [C]::GetDpiForWindow($hwnd) / 96.0
Write-Host ("client={0}x{1} origin={2},{3} scale={4}" -f $rect.Right, $rect.Bottom, $origin.X, $origin.Y, $scale)

function Px([double]$lx, [double]$ly) {
    [pscustomobject]@{ x = [int]($origin.X + $scale * $lx); y = [int]($origin.Y + $scale * $ly) }
}

$N_RUST = Px 585 301
$N_TYPESET = Px 817 349
$N_INTERACT = Px 573 416
$N_EXPORT = Px 814 441
$EMPTY = Px 450 650

function Move-To($p) {
    [void][C]::SetCursorPos($p.x, $p.y)
    Start-Sleep -Milliseconds 120
}

function Click-At($p) {
    Move-To $p
    [C]::mouse_event($LEFTDOWN, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 40
    [C]::mouse_event($LEFTUP, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 350
}

function Key-Down([byte]$vk) {
    [C]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 40
}

function Key-Up([byte]$vk) {
    [C]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 40
}

function Press-Combo([byte[]]$keys, [int]$tail = 420) {
    foreach ($vk in $keys) { Key-Down $vk }
    [array]::Reverse($keys)
    foreach ($vk in $keys) { Key-Up $vk }
    Start-Sleep -Milliseconds $tail
}

function Press-Key([byte]$vk, [int]$tail = 420) {
    Press-Combo @($vk) $tail
}

function Save-Shot([string]$name) {
    if (-not (Test-Path -LiteralPath $ShotDir)) { New-Item -ItemType Directory -Path $ShotDir | Out-Null }
    $r = New-Object C+Rect
    [void][C]::GetClientRect($hwnd, [ref]$r)
    $o = New-Object C+P
    [void][C]::ClientToScreen($hwnd, [ref]$o)
    $w = $r.Right
    $h = $r.Bottom
    $bmp = New-Object System.Drawing.Bitmap $w, $h, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($o.X, $o.Y, 0, 0, (New-Object System.Drawing.Size $w, $h))
    $out = Join-Path $ShotDir "$name.png"
    if (Test-Path -LiteralPath $out) { Remove-Item -LiteralPath $out -Force }
    $bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    Write-Host "  shot -> $out"
}

# A warm-up click consumes the activation click Windows may swallow.
Click-At $EMPTY
Move-To $EMPTY
Save-Shot 'm2c-initial'

# 1. Alt+Down / Alt+Up: reorder among siblings; undo restores.
Click-At $N_INTERACT
Save-Shot 'm2c-selected'
Press-Combo @($VK_ALT, $VK_DOWN)
Save-Shot 'm2c-move-down'
Press-Combo @($VK_CONTROL, $VK_Z)
Move-To $EMPTY

# 2. Ctrl+C / Ctrl+V clones the subtree as a sibling; one undo removes it.
Click-At $N_RUST
Press-Combo @($VK_CONTROL, $VK_C)
Press-Combo @($VK_CONTROL, $VK_V)
Move-To $EMPTY
Save-Shot 'm2c-paste'
Press-Combo @($VK_CONTROL, $VK_Z)

# 3. Ctrl+Shift+C / V copies an explicit style override.  The theme fills of
#    the demo nodes are inherited (copying from them would carry nothing), so
#    set an explicit red fill on the export node through the inspector first.
#    Inspector-open geometry is measured after fit-all with both panels shown:
#    export (691,435), interact (439,414), red swatch (861,225) in logical px.
Press-Key $VK_F9 500
Press-Combo @($VK_CONTROL, $VK_1) 700
$N_EXPORT_F9 = Px 691 435
$N_INTERACT_F9 = Px 439 414
$SWATCH_RED = Px 861 225
Click-At $N_EXPORT_F9
Click-At $SWATCH_RED
Move-To $EMPTY
Save-Shot 'm2c-style-set'
Press-Combo @($VK_CONTROL, $VK_SHIFT, $VK_C)
Click-At $N_INTERACT_F9
Press-Combo @($VK_CONTROL, $VK_SHIFT, $VK_V)
Move-To $EMPTY
Save-Shot 'm2c-style-paste'
Press-Combo @($VK_CONTROL, $VK_Z)
Press-Combo @($VK_CONTROL, $VK_Z)
Press-Key $VK_F9 500
Press-Combo @($VK_CONTROL, $VK_1) 700

# 4. Ctrl+Shift+Right / Left: demote then promote, two undos restore.
Click-At $N_EXPORT
Press-Combo @($VK_CONTROL, $VK_SHIFT, $VK_RIGHT)
Save-Shot 'm2c-demote'
Press-Combo @($VK_CONTROL, $VK_SHIFT, $VK_LEFT)
Save-Shot 'm2c-promote'
Press-Combo @($VK_CONTROL, $VK_Z)
Press-Combo @($VK_CONTROL, $VK_Z)

# 5. Slash collapses the selected branch; Ctrl+Slash collapses all.
Click-At $N_INTERACT
Press-Key $VK_OEM_2
Save-Shot 'm2c-fold'
Press-Key $VK_OEM_2
Press-Combo @($VK_CONTROL, $VK_OEM_2)
Save-Shot 'm2c-collapse-all'
Press-Combo @($VK_CONTROL, $VK_SHIFT, $VK_OEM_2)
Save-Shot 'm2c-expand-all'

# 6. Ctrl+0 / Ctrl+1 / Ctrl+2: 100%, fit all, fit selection.
Press-Combo @($VK_CONTROL, $VK_0)
Save-Shot 'm2c-zoom-100'
Press-Combo @($VK_CONTROL, $VK_1)
Click-At $N_RUST
Press-Combo @($VK_CONTROL, $VK_2)
Save-Shot 'm2c-fit-selection'
Press-Combo @($VK_CONTROL, $VK_1)

# 7. Alt+Enter focuses a subtree and shows the breadcrumb trail; Esc pops back.
Click-At $N_INTERACT
Press-Combo @($VK_ALT, $VK_ENTER)
Move-To $EMPTY
Save-Shot 'm2c-focus'
Press-Key $VK_ESCAPE
Save-Shot 'm2c-focus-back'

# 8. F1 help cheat sheet; Esc closes.
Press-Key $VK_F1
Save-Shot 'm2c-help'
Press-Key $VK_ESCAPE

# 9. Ctrl+P opens the export dialog with PDF preselected.
Press-Combo @($VK_CONTROL, $VK_P)
Save-Shot 'm2c-pdf'
Press-Key $VK_ESCAPE

# 10. Search: clipboard paste avoids the IME, Enter jumps and starts the glow.
Press-Combo @($VK_CONTROL, 0x46)
Start-Sleep -Milliseconds 300
# ASCII-only source: the literal for the export node is built from code points.
Set-Clipboard -Value ([string][char]0x5BFC + [char]0x51FA)
Press-Combo @($VK_CONTROL, $VK_V)
Press-Combo @($VK_ENTER) 180
Save-Shot 'm2c-search-glow'
Start-Sleep -Milliseconds 1100
Save-Shot 'm2c-search-glow-settled'
Press-Key $VK_ESCAPE

# 11. Panel open/close motion: one frame near the start, one after settle.
Press-Key $VK_F8 60
Save-Shot 'm2c-panel-closing'
Start-Sleep -Milliseconds 400
Save-Shot 'm2c-panel-closed'
Press-Key $VK_F8 380
Save-Shot 'm2c-panel-open'

Press-Key $VK_F9 380
Save-Shot 'm2c-inspector-open'
Press-Key $VK_F9

# 12. F11 fullscreen: print client sizes before/after so the change is provable.
$b0 = New-Object C+Rect
[void][C]::GetClientRect($hwnd, [ref]$b0)
Press-Key $VK_F11 800
$b1 = New-Object C+Rect
[void][C]::GetClientRect($hwnd, [ref]$b1)
Save-Shot 'm2c-fullscreen'
Press-Key $VK_F11 800
$b2 = New-Object C+Rect
[void][C]::GetClientRect($hwnd, [ref]$b2)
Write-Host ("fullscreen: before {0}x{1} full {2}x{3} after {4}x{5}" -f $b0.Right, $b0.Bottom, $b1.Right, $b1.Bottom, $b2.Right, $b2.Bottom)

# 13. Clean hover rings and release topmost.
Move-To $EMPTY
Move-To (Px 5 5)
[void][C]::SetWindowPos($hwnd, $HWND_NOTOPMOST, 0, 0, 0, 0, ($SWP_NOMOVE -bor $SWP_NOSIZE))
Write-Host 'done'
