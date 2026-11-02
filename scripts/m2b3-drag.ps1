param(
    [string]$ProcessName = 'mw-shell',
    [string]$ShotDir = 'shots/render',
    [ValidateSet('calibrate', 'reparent', 'illegal', 'reorder', 'multi')]
    [string]$Scenario = 'calibrate'
)

# M2b-3 drag probe: real mouse drag for re-parent / reorder / illegal target.
# Coordinates are logical CSS px inside the client area, matched to the demo document
# in canvas.rs (client 1620x1080 @ 150% => 1080x720 logical). Re-shoot m2b3-initial
# before trusting them: the sample doc re-lays-out after every drop.

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
$VK_HOME = 0x24
$VK_ESCAPE = 0x1B

$proc = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $proc) { Write-Error "process not running: $ProcessName"; exit 1 }
$hwnd = $proc.MainWindowHandle
if ($hwnd -eq [IntPtr]::Zero) { Write-Error "no main window yet"; exit 1 }

# Foreground + topmost, then VERIFY. SetForegroundWindow fails silently, and a probe
# that clicks into another window looks exactly like "the feature does not work".
[void][C]::ShowWindow($hwnd, 9)
[void][C]::SetWindowPos($hwnd, $HWND_TOPMOST, 0, 0, 0, 0, ($SWP_NOMOVE -bor $SWP_NOSIZE -bor $SWP_SHOWWINDOW))
[void][C]::BringWindowToTop($hwnd)
try { (New-Object -ComObject WScript.Shell).AppActivate($proc.Id) | Out-Null } catch { }
[void][C]::SetForegroundWindow($hwnd)
[void][C]::SetFocus($hwnd)
Start-Sleep -Milliseconds 700
if ([C]::GetForegroundWindow() -ne $hwnd) {
    Write-Warning "foreground is not the app (hwnd=$hwnd); screenshots and clicks are not trustworthy"
} else {
    Write-Host "foreground ok"
}

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
    [C]::mouse_event($LEFTDOWN, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 40
    [C]::mouse_event($LEFTUP, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 400
}

function Press-Key([byte]$vk) {
    [C]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [C]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 500
}

# Drag and HOLD the button so a mid-drag screenshot can prove the ghost exists
# (docs/06 §5: 半透明幽灵跟随 + 三分区指示). Returns nothing; caller releases.
function Drag-Hold($a, $b, [int]$steps = 16) {
    Move-To $a
    [C]::mouse_event($LEFTDOWN, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 120
    for ($i = 1; $i -le $steps; $i++) {
        $k = $i / $steps
        [void][C]::SetCursorPos([int]($a.x + ($b.x - $a.x) * $k), [int]($a.y + ($b.y - $a.y) * $k))
        Start-Sleep -Milliseconds 25
    }
    Start-Sleep -Milliseconds 350
}

function Drag($a, $b, [int]$steps = 16) {
    Drag-Hold $a $b $steps
    Release-Here
}

function Release-Here {
    [C]::mouse_event($LEFTUP, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 500
}

# Warm-up click: the very first press after SetForegroundWindow may be eaten by the
# window-activation click.
Click-At (Px 640 640)

# Pin the view: the window's first fit depends on the size it happened to have when the
# first frame ran, and ShowWindow(SW_RESTORE) can change that afterwards. Home re-fits
# against the *current* viewport, so the same coordinates apply on every run.
Press-Key $VK_HOME
Move-To (Px 640 640)
Save-Shot "m2b3-$Scenario-initial"

switch ($Scenario) {
    'calibrate' {
        # Only the reference shot; coordinates are read off it by a human/agent.
    }
    'reparent' {
        # 「禁则换行」-> middle of「纯 Rust 原生」: becomes its child.
        Drag-Hold (Px 917 352) (Px 452 292)
        Save-Shot 'm2b3-ghost-child'
        Release-Here
        Save-Shot 'm2b3-child-dropped'
    }
    'illegal' {
        # 「纯 Rust 原生」-> its own descendant「winit + windows-rs」: red + no-op.
        Drag-Hold (Px 452 292) (Px 242 263)
        Save-Shot 'm2b3-illegal'
        Release-Here
        Save-Shot 'm2b3-illegal-released'
    }
    'multi' {
        # Box-select the three children of「中文排版」, then drag one of them:
        # the whole group must move (docs/06 §5「多选后拖拽 = 整组移动」).
        Drag (Px 858 262) (Px 1032 434)
        Save-Shot 'm2b3-multi-selected'
        Drag-Hold (Px 944 288) (Px 446 440)
        Save-Shot 'm2b3-multi-hover'
        Release-Here
        Save-Shot 'm2b3-multi-dropped'
    }
    'reorder' {
        # 「中文排版」-> upper edge of「纯 Rust 原生」: same-parent reorder (Before zone).
        # 落点必须是「换个位置」才算数：把它丢到它现在就在的位置后面，计划会是空的。
        Drag-Hold (Px 760 347) (Px 452 282)
        Save-Shot 'm2b3-reorder-hover'
        Release-Here
        Save-Shot 'm2b3-reordered'
    }
}

Move-To (Px 640 640)
[void][C]::SetWindowPos($hwnd, $HWND_NOTOPMOST, 0, 0, 0, 0, ($SWP_NOMOVE -bor $SWP_NOSIZE))
Write-Host 'done'
