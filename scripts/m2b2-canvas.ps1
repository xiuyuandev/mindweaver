param(
    [string]$ProcessName = 'mw-shell',
    [string]$ShotDir = 'shots/render'
)

# M2b-2 canvas probe: 真实鼠标/键盘走多选、框选、平移改键、整组删除。
# 坐标是客户区内的**逻辑** CSS px，对应 canvas.rs 的示例文档（150% DPI 下客户区 1620×1080，
# 即 1080×720 逻辑）。几何变了就重新截一张 m2b2-initial 对一遍，别猜。
#
# 与 m1b-canvas.ps1 的差别（M2b-2 改了语义，旧步骤必须一起改）：
#   空白处左键拖拽 = 框选，不再是平移；平移走中键 / 空格按住 + 左键。

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
$MIDDLEDOWN = 0x0020
$MIDDLEUP = 0x0040
$VK_CONTROL = 0x11
$VK_SPACE = 0x20
$VK_HOME = 0x24
$VK_DELETE = 0x2E
$VK_F2 = 0x71
$VK_ESCAPE = 0x1B
$WHEEL = 0x0800

$proc = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $proc) { Write-Error "process not running: $ProcessName"; exit 1 }
$hwnd = $proc.MainWindowHandle
if ($hwnd -eq [IntPtr]::Zero) { Write-Error "no main window yet"; exit 1 }

[void][C]::ShowWindow($hwnd, 9)
# 置顶 + AppActivate + 显式校验前台归属。
#
# 不然会两头翻车：截图截到压在织思上面的编辑器（CopyFromScreen 抓的是屏幕，不是窗口），
# 点击也落在那个窗口上——表面看像「代码没反应」，其实一次都没点进画布。
[void][C]::SetWindowPos($hwnd, $HWND_TOPMOST, 0, 0, 0, 0, ($SWP_NOMOVE -bor $SWP_NOSIZE -bor $SWP_SHOWWINDOW))
[void][C]::BringWindowToTop($hwnd)
try { (New-Object -ComObject WScript.Shell).AppActivate($proc.Id) | Out-Null } catch { }
[void][C]::SetForegroundWindow($hwnd)
[void][C]::SetFocus($hwnd)
Start-Sleep -Milliseconds 700
if ([C]::GetForegroundWindow() -ne $hwnd) {
    Write-Warning "前台不是织思（hwnd=$hwnd），这一轮的截图和点击都不可信"
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

function Click-At($p, [switch]$Ctrl) {
    Move-To $p
    if ($Ctrl) { [C]::keybd_event($VK_CONTROL, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 60 }
    [C]::mouse_event($LEFTDOWN, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 40
    [C]::mouse_event($LEFTUP, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 120
    if ($Ctrl) { [C]::keybd_event($VK_CONTROL, 0, 2, [IntPtr]::Zero) }
    Start-Sleep -Milliseconds 400
}

function Press-Key([byte]$vk) {
    [C]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [C]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 500
}

function Zoom-Wheel([int]$notches = 3) {
    # Ctrl+滚轮 -> egui 的 zoom_delta，与触控板捏合同一条路径。
    [C]::keybd_event($VK_CONTROL, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 80
    for ($i = 0; $i -lt $notches; $i++) {
        # mouse_event 的轮转量在 dwData（第 4 个参数），不是 dy——写错位置就是「什么都没发生」。
        [C]::mouse_event($WHEEL, 0, 0, 120, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 150
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

# ── -1. 预热：SetForegroundWindow 唤起的窗口，第一下按下可能被系统当「激活点击」吞掉。
#       先在空白处点一下消化掉它，否则第一段框选会被误判成「代码没反应」。
Click-At (Px 640 640)

# ── 0. 基线（指针停在空白处，别让悬停环污染第一张）
Move-To (Px 640 640)
Save-Shot 'm2b2-initial'

# ── 1. 框选：从「中文排版」左侧空白拉一个框，罩住 DirectWrite 度量 / 禁则换行 / 字形图集
#      三张卡。松手那一帧必须吞掉 click——否则刚框出来的组会被「单击空白」立刻清掉。
Drag (Px 858 252) (Px 1020 442)
Save-Shot 'm2b2-marquee'

# ── 2. Ctrl+单击「中文排版」：加进组，不替换
Click-At (Px 760 352) -Ctrl
Save-Shot 'm2b2-additive'

# ── 3. 再 Ctrl+单击一次：移出组
Click-At (Px 760 352) -Ctrl
Save-Shot 'm2b2-toggled-off'

# ── 4. 编辑期锁缩放：F2 进编辑后 Ctrl+滚轮，字号与节点盒都不该变（否则候选窗错位）。
#      Ctrl+= 早在 M2a 就锁了，这一条补的是**滚轮/pinch 分支**（docs/06:108）。
#      先做 A/B 对照：非编辑态同样的滚轮必须真的放大，否则「锁住了」什么也证明不了。
Press-Key $VK_HOME
Move-To (Px 945 287)
Save-Shot 'm2b2-zoom-before'
Zoom-Wheel 3
Save-Shot 'm2b2-zoom-free'

Press-Key $VK_HOME
Click-At (Px 945 287)
Save-Shot 'm2b2-edit-before-f2'
Press-Key $VK_F2
Save-Shot 'm2b2-edit-mode'
Zoom-Wheel 3
Save-Shot 'm2b2-edit-zoom-locked'
Press-Key $VK_ESCAPE
Start-Sleep -Milliseconds 300

# ── 5. 单击空白：清空选择
Click-At (Px 640 640)
Save-Shot 'm2b2-cleared'

# ── 6. 重新框选后按 Delete：整组删除（根不可删，组里混进根时只跳过根）
Drag (Px 858 252) (Px 1020 442)
Press-Key $VK_DELETE
Save-Shot 'm2b2-multi-deleted'

# ── 7. 空格按住 + 左键拖空白：平移（左键单独拖已经是框选了）
Press-Key $VK_HOME
Move-To (Px 620 600)
[C]::keybd_event($VK_SPACE, 0, 0, [IntPtr]::Zero)
Start-Sleep -Milliseconds 120
Drag (Px 620 600) (Px 740 650)
[C]::keybd_event($VK_SPACE, 0, 2, [IntPtr]::Zero)
Start-Sleep -Milliseconds 300
Save-Shot 'm2b2-space-pan'

# ── 8. 中键拖空白：平移
Press-Key $VK_HOME
Drag (Px 620 600) (Px 500 560) $MIDDLEDOWN $MIDDLEUP
Save-Shot 'm2b2-middle-pan'

# ── 9. 干净地注销悬停环，并把手动置顶还回去
Move-To (Px 640 640)
Move-To (Px 5 5)
[void][C]::SetWindowPos($hwnd, $HWND_NOTOPMOST, 0, 0, 0, 0, ($SWP_NOMOVE -bor $SWP_NOSIZE))
Write-Host 'done'
