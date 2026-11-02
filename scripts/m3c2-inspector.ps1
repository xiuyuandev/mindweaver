param(
    [string]$ProcessName = 'mw-shell',
    [string]$ShotDir = 'shots/e2e'
)

# M3-c-b probe: right inspector + minimap with real mouse/keyboard at 150% DPI.
#
# Preconditions: the app is already running with the sample document open, e.g.
#   $env:MW_OPEN='target/smoke-sample.zmind'; .\target\debug\mw-shell.exe
# and uses the default window (1080x720 logical), 232px outline, 260px inspector.
# The probe intentionally edits the document (fill/note/link/icon/label), so run it
# against a throwaway copy and discard the changes afterwards.

Add-Type -AssemblyName System.Drawing

if (-not ('I' -as [type])) {
    Add-Type -Language CSharp @'
using System;
using System.Runtime.InteropServices;
public static class I {
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
[void][I]::SetProcessDPIAware()

$HWND_TOPMOST = [IntPtr](-1)
$HWND_NOTOPMOST = [IntPtr](-2)
$SWP_NOMOVE = 0x0002
$SWP_NOSIZE = 0x0001
$SWP_SHOWWINDOW = 0x0040
$LEFTDOWN = 0x0002
$LEFTUP = 0x0004
$VK_CONTROL = 0x11
$VK_RETURN = 0x0D
$VK_A = 0x41
$VK_V = 0x56
$VK_F9 = 0x78

# UTF-8 payloads kept as base64 so the script's own encoding cannot corrupt them.
$noteText = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String(
    '5qOA5p+l5Zmo5YaZ5YWl55qE5aSH5rOo77ya6aqM6K+BIFNldE5vdGUg5LiO5pKk6ZSA5qCI44CC'))
$labelText = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('5b6F56Gu6K6k'))
$linkText = 'https://xmind.app/share/example?from=zhisi'

$proc = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $proc) { Write-Error "process not running: $ProcessName"; exit 1 }
$hwnd = $proc.MainWindowHandle
if ($hwnd -eq [IntPtr]::Zero) { Write-Error 'no main window yet'; exit 1 }

[void][I]::ShowWindow($hwnd, 9)
[void][I]::SetWindowPos($hwnd, $HWND_TOPMOST, 0, 0, 0, 0, ($SWP_NOMOVE -bor $SWP_NOSIZE -bor $SWP_SHOWWINDOW))
[void][I]::BringWindowToTop($hwnd)
try { (New-Object -ComObject WScript.Shell).AppActivate($proc.Id) | Out-Null } catch { }
[void][I]::SetForegroundWindow($hwnd)
[void][I]::SetFocus($hwnd)
Start-Sleep -Milliseconds 700
$foreground = [I]::GetForegroundWindow()
if ($foreground -ne $hwnd) {
    Write-Warning "foreground is not the app (hwnd=$hwnd, fg=$foreground); screenshots are not trustworthy"
} else {
    Write-Host 'foreground ok'
}

$rect = New-Object I+Rect
[void][I]::GetClientRect($hwnd, [ref]$rect)
$origin = New-Object I+P
[void][I]::ClientToScreen($hwnd, [ref]$origin)
$scale = [I]::GetDpiForWindow($hwnd) / 96.0
$cw = $rect.Right
$ch = $rect.Bottom
Write-Host "client ${cw}x${ch} @ ${scale}x"

function Px([double]$lx, [double]$ly) {
    [pscustomobject]@{ x = [int]($origin.X + $scale * $lx); y = [int]($origin.Y + $scale * $ly) }
}
function Move-To($p) {
    [void][I]::SetCursorPos($p.x, $p.y)
    Start-Sleep -Milliseconds 120
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
    Move-To $p
    [I]::mouse_event($LEFTDOWN, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 40
    [I]::mouse_event($LEFTUP, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 450
}
function Drag-FromTo($a, $b) {
    Move-To $a
    [I]::mouse_event($LEFTDOWN, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 80
    for ($i = 1; $i -le 10; $i++) {
        [void][I]::SetCursorPos([int]($a.x + ($b.x - $a.x) * $i / 10), [int]($a.y + ($b.y - $a.y) * $i / 10))
        Start-Sleep -Milliseconds 25
    }
    [I]::mouse_event($LEFTUP, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 500
}
function Press-Key([byte]$vk) {
    [I]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [I]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 500
}
function Ctrl-Key([byte]$vk) {
    [I]::keybd_event($VK_CONTROL, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [I]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [I]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [I]::keybd_event($VK_CONTROL, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 600
}
function Paste-Text([string]$text) {
    Set-Clipboard -Value $text
    Start-Sleep -Milliseconds 200
    Ctrl-Key $VK_V
}

# Warm-up click on empty canvas: absorbs the activation click and clears selection.
Click-At (Px 500 500)
Save-Shot 'm3c2-inspector-blank'

# Select "由 Rust 读取": node page, style tab.
Click-At (Px 609 408)
Save-Shot 'm3c2-inspector-node'

# Fill swatch (blue, second row position after the selected orange/green).
Click-At (Px 872 190)
Save-Shot 'm3c2-inspector-style'

# Note: select-all + clipboard + Ctrl+Enter.
Click-At (Px 921 135)
Click-At (Px 950 210)
Ctrl-Key $VK_A
Paste-Text $noteText
Save-Shot 'm3c2-inspector-note-draft'
Ctrl-Key $VK_RETURN
Save-Shot 'm3c2-inspector-note'

# Link: plain Enter commits. The canvas must NOT create a sibling on the same Enter.
Click-At (Px 990 135)
Click-At (Px 950 178)
Paste-Text $linkText
Save-Shot 'm3c2-inspector-link-draft'
Press-Key $VK_RETURN
Save-Shot 'm3c2-inspector-link'

# Mark: priority-1 icon + label.
Click-At (Px 1037 135)
Click-At (Px 847 185)
Save-Shot 'm3c2-inspector-icon'
Click-At (Px 880 355)
Paste-Text $labelText
Press-Key $VK_RETURN
Save-Shot 'm3c2-inspector-mark'

# Minimap: drag pans the camera without clearing the multiselect/single selection.
Drag-FromTo (Px 640 620) (Px 700 620)
Save-Shot 'm3c2-minimap-pan'
Click-At (Px 792 552)
Save-Shot 'm3c2-minimap-fold'
Click-At (Px 795 668)
Save-Shot 'm3c2-minimap-open'

# F9 hides and restores the inspector.
Press-Key $VK_F9
Save-Shot 'm3c2-inspector-hidden'
Press-Key $VK_F9
Save-Shot 'm3c2-inspector-shown'

# Cleanup: drop clipboard content and restore normal z-order.
Set-Clipboard -Value ' '
Move-To (Px 640 600)
[void][I]::SetWindowPos($hwnd, $HWND_NOTOPMOST, 0, 0, 0, 0, ($SWP_NOMOVE -bor $SWP_NOSIZE))
Write-Host 'done'
