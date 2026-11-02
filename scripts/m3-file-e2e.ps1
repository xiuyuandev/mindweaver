param(
    [string]$ProcessName = 'mw-shell',
    [string]$ShotDir = 'shots/e2e',
    # 只截一张启动画面：重启后用它验证"改动真的落盘了"。
    [switch]$ShotOnly,
    # dialogs = 打开对话框与关窗拦截；留空 = 打开→编辑→保存的常规链路。
    [string]$Scenario = ''
)

# M3 file I/O probe: open a real .zmind at startup, edit it, Ctrl+S, then verify the
# change survived a restart (the second half is done by the caller: relaunch + screenshot).
#
# The app must be started with MW_OPEN=<absolute path> for the open path to be exercised.

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
$VK_S = 0x53
$VK_TAB = 0x09
$VK_RETURN = 0x0D
$VK_O = 0x4F
$VK_ESCAPE = 0x1B
$VK_MENU = 0x12
$VK_F4 = 0x73

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
    Start-Sleep -Milliseconds 400
}
function Press-Key([byte]$vk) {
    [C]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [C]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 500
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

if ($Scenario -eq 'dialogs') {
    # Ctrl+O：必须弹出系统的打开对话框；Esc 取消后文档不变。
    Ctrl-Key $VK_O
    Start-Sleep -Milliseconds 900
    Save-Shot 'm3-open-dialog'
    # 点「取消」而不是按 Esc：Esc 在这个对话框上不可靠（也验证了左下角按钮的位置）。
    Click-At (Px 576 442)
    Start-Sleep -Milliseconds 800
    Save-Shot 'm3-open-dialog-cancelled'
    # 改一点东西，再 Alt+F4：关窗必须被拦下并弹确认。
    Press-Key $VK_TAB
    Press-Key $VK_RETURN
    Start-Sleep -Milliseconds 300
    [C]::keybd_event($VK_MENU, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 80
    [C]::keybd_event($VK_F4, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 80
    [C]::keybd_event($VK_F4, 0, 2, [IntPtr]::Zero)
    [C]::keybd_event($VK_MENU, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 800
    Save-Shot 'm3-close-guard'
    [void][C]::SetWindowPos($hwnd, $HWND_NOTOPMOST, 0, 0, 0, 0, ($SWP_NOMOVE -bor $SWP_NOSIZE))
    Write-Host 'dialogs done'
    exit 0
}

# Warm-up click absorbs the window-activation click.
Click-At (Px 640 640)
Move-To (Px 640 640)
Save-Shot 'm3-open-initial'

if ($ShotOnly) {
    [void][C]::SetWindowPos($hwnd, $HWND_NOTOPMOST, 0, 0, 0, 0, ($SWP_NOMOVE -bor $SWP_NOSIZE))
    Write-Host 'shot only'
    exit 0
}

# Tab adds a child under root and opens the inline editor: proves the document loaded
# from disk is a *live* document, not a read-only snapshot.
Press-Key $VK_TAB
Start-Sleep -Milliseconds 400
Save-Shot 'm3-node-added'

# Commit the (empty) new node with Enter, then save back to the same path: no dialog because the
# session already has a path. The toast at the bottom-left is the visible receipt.
Press-Key $VK_RETURN
Ctrl-Key $VK_S
Save-Shot 'm3-saved'

Move-To (Px 640 640)
[void][C]::SetWindowPos($hwnd, $HWND_NOTOPMOST, 0, 0, 0, 0, ($SWP_NOMOVE -bor $SWP_NOSIZE))
Write-Host 'done'
