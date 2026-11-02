param(
    [string]$ProcessName = 'mw-shell',
    [string]$ShotDir = 'shots/e2e',
    [ValidateSet('select', 'collapse', 'hide', 'palette')]
    [string]$Scenario = 'select'
)

# M3-c probe: outline panel interactions.
# Row geometry assumes the sample document and the default 232px panel:
# header ~74px, rows 26px tall starting at y=88.

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
$VK_F8 = 0x77
$VK_CONTROL = 0x11
$VK_K = 0x4B
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
function Press-Key([byte]$vk) {
    [C]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [C]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 500
}

# Warm-up click on empty canvas absorbs the activation click.
Click-At (Px 640 600)

switch ($Scenario) {
    'select' {
        # 第 8 行「字形图集」
        Click-At (Px 70 282)
        Start-Sleep -Milliseconds 400
        Save-Shot 'm3c-outline-select'
    }
    'collapse' {
        # 第 5 行「中文排版」左侧的折叠箭头
        Click-At (Px 20 204)
        Start-Sleep -Milliseconds 400
        Save-Shot 'm3c-outline-collapsed'
    }
    'palette' {
        Ctrl-Key $VK_K
        Start-Sleep -Milliseconds 500
        Save-Shot 'm3c-palette-open'
        # 第一项是「新建文档」：回车应当直接执行并关掉面板
        Press-Key $VK_RETURN
        Start-Sleep -Milliseconds 500
        Save-Shot 'm3c-palette-ran'
    }
    'hide' {
        Press-Key $VK_F8
        Save-Shot 'm3c-outline-hidden'
    }
}

Move-To (Px 640 600)
[void][C]::SetWindowPos($hwnd, $HWND_NOTOPMOST, 0, 0, 0, 0, ($SWP_NOMOVE -bor $SWP_NOSIZE))
Write-Host 'done'
