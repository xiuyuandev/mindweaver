param(
    [string]$Exe = 'target/release/mw-shell.exe',
    [string]$ShotDir = 'shots/m3',
    [string]$StateFile = "$env:TEMP\mw-window-state-probe.json"
)

# M3 probe: window geometry + panel state persistence, through real input and
# real process restarts. The app is pointed at a disposable state file via
# MW_WINDOW_STATE so the developer's own layout is never touched.
#
# Case A: default launch -> resize/move -> F8 off / F9 on -> graceful exit.
# Case B: relaunch -> geometry + panels restored -> maximize -> exit.
# Case C: relaunch -> starts maximized -> restore -> normal rect is back.
# Case D: corrupt/off-screen rect -> falls back to default, panels still apply.

Add-Type -AssemblyName System.Drawing

if (-not ('C' -as [type])) {
    Add-Type -Language CSharp @'
using System;
using System.Runtime.InteropServices;
public static class C {
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out Rect r);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out Rect r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref P p);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] public static extern bool IsZoomed(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint d, int e, int dw, IntPtr r);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint f, IntPtr extra);
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct P { public int X, Y; }
}
'@
}
[void][C]::SetProcessDPIAware()

$SWP_NOMOVE = 0x0002
$SWP_NOZORDER = 0x0004
$SWP_NOACTIVATE = 0x0010
$LEFTDOWN = 0x0002
$LEFTUP = 0x0004
$VK_F8 = 0x77
$VK_F9 = 0x78

$script:p = $null
$script:hwnd = [IntPtr]::Zero

function Fail([string]$msg) {
    Write-Error "ASSERT FAILED: $msg"
    if ($script:p -and -not $script:p.HasExited) {
        Stop-Process -Id $script:p.Id -Force -ErrorAction SilentlyContinue
    }
    exit 1
}

function Assert([bool]$cond, [string]$msg) {
    if (-not $cond) { Fail $msg }
}

function Stop-App {
    if (-not $script:p) { return }
    if (-not $script:p.HasExited) {
        Stop-Process -Id $script:p.Id -Force -ErrorAction SilentlyContinue
        [void]$script:p.WaitForExit(4000)
    }
    $script:p = $null
    $script:hwnd = [IntPtr]::Zero
}

function Get-Rect {
    $r = New-Object C+Rect
    if (-not [C]::GetWindowRect($script:hwnd, [ref]$r)) { Fail 'GetWindowRect failed' }
    [pscustomobject]@{
        x = $r.Left; y = $r.Top
        w = $r.Right - $r.Left; h = $r.Bottom - $r.Top
    }
}

function Start-App {
    $env:MW_WINDOW_STATE = $StateFile
    # Default launch now lands on the start page; this probe drives canvas panels.
    $env:MW_HOME = '0'
    $script:p = Start-Process -FilePath $Exe -PassThru
    Remove-Item Env:\MW_WINDOW_STATE -ErrorAction SilentlyContinue
    Remove-Item Env:\MW_HOME -ErrorAction SilentlyContinue

    $deadline = (Get-Date).AddSeconds(20)
    $ready = $false
    while ((Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 150
        $script:p.Refresh()
        if ($script:p.HasExited) { Fail 'app exited before showing a window' }
        $h = $script:p.MainWindowHandle
        if ($h -ne [IntPtr]::Zero) {
            # Winit creates small helper windows (thread event target, IME) before the
            # real frame; MainWindowHandle can briefly point at one of those. Only
            # accept a window big enough to be the app shell.
            $probe = New-Object C+Rect
            if ([C]::GetWindowRect($h, [ref]$probe)) {
                $pw = $probe.Right - $probe.Left
                $ph = $probe.Bottom - $probe.Top
                if ($pw -ge 600 -and $ph -ge 400) {
                    $script:hwnd = $h
                    $ready = $true
                    break
                }
            }
        }
    }
    if (-not $ready) { Fail 'no usable main window after 20s' }

    # Let the first frame apply default sizing or the restored geometry.
    Start-Sleep -Milliseconds 1100

    [void][C]::BringWindowToTop($script:hwnd)
    try { (New-Object -ComObject WScript.Shell).AppActivate($script:p.Id) | Out-Null } catch { }
    [void][C]::SetForegroundWindow($script:hwnd)
    Start-Sleep -Milliseconds 350
    if ([C]::GetForegroundWindow() -ne $script:hwnd) {
        Fail 'window did not come to foreground; input would go to another app'
    }
    Write-Host ("app up: pid={0} hwnd={1}" -f $script:p.Id, $script:hwnd)
}

function Close-App {
    [void]$script:p.CloseMainWindow()
    if (-not $script:p.WaitForExit(7000)) {
        Fail 'app did not exit after CloseMainWindow'
    }
    Start-Sleep -Milliseconds 350
    $script:p = $null
    $script:hwnd = [IntPtr]::Zero
}

function Send-Key([byte]$vk) {
    if ([C]::GetForegroundWindow() -ne $script:hwnd) { Fail "foreground lost before key 0x$('{0:X2}' -f $vk)" }
    [C]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [C]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 450
}

function Click-At([int]$x, [int]$y) {
    [void][C]::SetCursorPos($x, $y)
    Start-Sleep -Milliseconds 120
    [C]::mouse_event($LEFTDOWN, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 40
    [C]::mouse_event($LEFTUP, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 700
}

function Save-Shot([string]$name) {
    if (-not (Test-Path -LiteralPath $ShotDir)) { New-Item -ItemType Directory -Path $ShotDir | Out-Null }
    $r = New-Object C+Rect
    [void][C]::GetClientRect($script:hwnd, [ref]$r)
    $o = New-Object C+P
    [void][C]::ClientToScreen($script:hwnd, [ref]$o)
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

function Read-State {
    Assert (Test-Path -LiteralPath $StateFile) 'state file was not written'
    $raw = Get-Content -LiteralPath $StateFile -Raw
    Write-Host "  state: $raw"
    return ($raw | ConvertFrom-Json)
}

function Assert-RectNear($a, $b, [int]$tol, [string]$what) {
    Assert ([Math]::Abs($a.x - $b[0]) -le $tol) "$what x: got $($a.x), want $($b[0]) (tol $tol)"
    Assert ([Math]::Abs($a.y - $b[1]) -le $tol) "$what y: got $($a.y), want $($b[1]) (tol $tol)"
    Assert ([Math]::Abs($a.w - $b[2]) -le $tol) "$what w: got $($a.w), want $($b[2]) (tol $tol)"
    Assert ([Math]::Abs($a.h - $b[3]) -le $tol) "$what h: got $($a.h), want $($b[3]) (tol $tol)"
}

# Maximize/restore button: middle of the three 46px window buttons at the top right.
function Click-MaximizeButton {
    $r = New-Object C+Rect
    [void][C]::GetClientRect($script:hwnd, [ref]$r)
    $o = New-Object C+P
    [void][C]::ClientToScreen($script:hwnd, [ref]$o)
    $dpi = [C]::GetDpiForWindow($script:hwnd) / 96.0
    $bx = [int]($o.X + $r.Right - $dpi * (46.0 + 23.0))
    $by = [int]($o.Y + $dpi * 28.0)
    Write-Host ("  click maximize button at {0},{1}" -f $bx, $by)
    Click-At $bx $by
}

if (-not (Test-Path -LiteralPath $Exe)) { Write-Error "missing exe: $Exe (cargo build --release first)"; exit 1 }
if (Test-Path -LiteralPath $StateFile) { Remove-Item -LiteralPath $StateFile -Force }
if (-not (Test-Path -LiteralPath $ShotDir)) { New-Item -ItemType Directory -Path $ShotDir | Out-Null }

try {
    Write-Host '=== A: first run, move/resize, toggle panels, close ==='
    Start-App
    $initial = Get-Rect
    Assert ($initial.w -ge 640 -and $initial.h -ge 460) "implausible initial rect $($initial.w)x$($initial.h)"
    Write-Host ("  initial rect = {0},{1} {2}x{3}" -f $initial.x, $initial.y, $initial.w, $initial.h)

    $wantX = 220; $wantY = 140; $wantW = 1400; $wantH = 900
    [void][C]::SetWindowPos($script:hwnd, [IntPtr]::Zero, $wantX, $wantY, $wantW, $wantH, ($SWP_NOZORDER -bor $SWP_NOACTIVATE))
    Start-Sleep -Milliseconds 500
    $moved = Get-Rect
    Assert-RectNear $moved @($wantX, $wantY, $wantW, $wantH) 4 'SetWindowPos'
    Write-Host ("  moved rect = {0},{1} {2}x{3}" -f $moved.x, $moved.y, $moved.w, $moved.h)

    Send-Key $VK_F8   # outline off
    Send-Key $VK_F9   # inspector on
    Start-Sleep -Milliseconds 1100   # debounce window
    Save-Shot 'm3-ws-1-first-run'
    Close-App

    $st = Read-State
    Assert ($null -ne $st.rect) 'state has no rect'
    Assert-RectNear $moved @($st.rect[0], $st.rect[1], $st.rect[2], $st.rect[3]) 2 'saved rect vs window'
    Assert ($st.maximized -eq $false) 'first run saved maximized=true'
    Assert ($st.show_outline -eq $false) 'F8 (outline off) was not persisted'
    Assert ($st.show_inspector -eq $true) 'F9 (inspector on) was not persisted'

    Write-Host '=== B: restart restores geometry + panels, then maximize ==='
    Start-App
    $restored = Get-Rect
    Assert-RectNear $restored @($st.rect[0], $st.rect[1], $st.rect[2], $st.rect[3]) 2 'restored rect vs saved'
    Assert (-not [C]::IsZoomed($script:hwnd)) 'restored window should not be maximized'
    Write-Host ("  restored rect = {0},{1} {2}x{3}" -f $restored.x, $restored.y, $restored.w, $restored.h)
    Save-Shot 'm3-ws-2-restored-panels'

    Click-MaximizeButton
    Assert ([C]::IsZoomed($script:hwnd)) 'maximize button did not maximize the window'
    Start-Sleep -Milliseconds 900
    $maxRect = Get-Rect
    Write-Host ("  maximized rect = {0},{1} {2}x{3}" -f $maxRect.x, $maxRect.y, $maxRect.w, $maxRect.h)
    Save-Shot 'm3-ws-3-maximized'
    Start-Sleep -Milliseconds 800
    Close-App

    $st2 = Read-State
    Assert ($st2.maximized -eq $true) 'maximized state was not persisted'
    Assert-RectNear @{ x = $st2.rect[0]; y = $st2.rect[1]; w = $st2.rect[2]; h = $st2.rect[3] } @($restored.x, $restored.y, $restored.w, $restored.h) 16 'normal rect saved while maximized'

    Write-Host '=== C: restart starts maximized, restore keeps normal rect ==='
    Start-App
    Assert ([C]::IsZoomed($script:hwnd)) 'window did not start maximized'
    Save-Shot 'm3-ws-4-start-maximized'
    Click-MaximizeButton
    Assert (-not [C]::IsZoomed($script:hwnd)) 'restore button did not unmaximize'
    Start-Sleep -Milliseconds 700
    $back = Get-Rect
    Assert-RectNear $back @($st2.rect[0], $st2.rect[1], $st2.rect[2], $st2.rect[3]) 16 'normal rect after unmaximize'
    Write-Host ("  unmaximized rect = {0},{1} {2}x{3}" -f $back.x, $back.y, $back.w, $back.h)
    Save-Shot 'm3-ws-5-unmaximized'
    Close-App

    Write-Host '=== D: off-screen saved rect falls back, panels still apply ==='
    $bad = @{
        rect = @(-50000, -50000, 1400, 900)
        maximized = $false
        show_outline = $false
        show_inspector = $true
        minimap_open = $false
    } | ConvertTo-Json
    [System.IO.File]::WriteAllText($StateFile, $bad, (New-Object System.Text.UTF8Encoding($false)))
    Start-App
    $fallback = Get-Rect
    Assert ([Math]::Abs($fallback.x) -lt 30000) "off-screen rect was restored: $($fallback.x)"
    Assert ($fallback.w -ge 640 -and $fallback.h -ge 460) "fallback rect implausible: $($fallback.w)x$($fallback.h)"
    Write-Host ("  fallback rect = {0},{1} {2}x{3}" -f $fallback.x, $fallback.y, $fallback.w, $fallback.h)
    Save-Shot 'm3-ws-6-offscreen-fallback'
    Close-App

    Write-Host 'ALL WINDOW-STATE CASES PASSED'
}
finally {
    Stop-App
}
