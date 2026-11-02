param(
    [string]$Exe = 'target/release/mw-shell.exe',
    [string]$ShotDir = 'shots/m3',
    [string]$WorkDir = "$env:TEMP\mw-home-probe"
)

# M3 probe: the start page (home) through real input, real restarts.
# A: default launch -> home -> fishbone template -> close last tab -> home
#    -> blank -> menu -> home (can return) -> Esc back -> close.
# B: open a file (records a recent) -> relaunch -> home lists it -> click opens it.
# APPDATA and window state are isolated so the developer's recents/layout are untouched.

Add-Type -AssemblyName System.Drawing
if (-not ('C' -as [type])) {
    Add-Type -Language CSharp @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class C {
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out Rect r);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out Rect r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref P p);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
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

$LEFTDOWN = 0x0002
$LEFTUP = 0x0004
$VK_CONTROL = 0x11
$VK_ESCAPE = 0x1B
$VK_N = 0x4E
$VK_W = 0x57

# Chinese strings are built from code points so this file stays pure ASCII.
$Brand = -join ([char]0x7EC7, [char]0x601D)
$Fishbone = -join ([char]0x9C7C, [char]0x9AA8, [char]0x56FE)

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

function Start-App([hashtable]$envs) {
    $env:MW_WINDOW_STATE = Join-Path $WorkDir 'window.json'
    foreach ($k in $envs.Keys) { Set-Item -Path "Env:\$k" -Value $envs[$k] }
    $script:p = Start-Process -FilePath $Exe -PassThru
    foreach ($k in $envs.Keys) { Remove-Item "Env:\$k" -ErrorAction SilentlyContinue }
    Remove-Item Env:\MW_WINDOW_STATE -ErrorAction SilentlyContinue

    $deadline = (Get-Date).AddSeconds(20)
    $ready = $false
    while ((Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 150
        $script:p.Refresh()
        if ($script:p.HasExited) { Fail 'app exited before showing a window' }
        $h = $script:p.MainWindowHandle
        if ($h -ne [IntPtr]::Zero) {
            # Winit creates tiny helper windows before the real frame.
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

    Start-Sleep -Milliseconds 1400
    [void][C]::BringWindowToTop($script:hwnd)
    try { (New-Object -ComObject WScript.Shell).AppActivate($script:p.Id) | Out-Null } catch { }
    [void][C]::SetForegroundWindow($script:hwnd)
    Start-Sleep -Milliseconds 400
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

function Get-Title {
    $sb = New-Object System.Text.StringBuilder 512
    [void][C]::GetWindowTextW($script:hwnd, $sb, 512)
    $sb.ToString()
}

function Assert-Title([string]$needle, [string]$why) {
    $t = Get-Title
    if ($t -notlike "*$needle*") {
        Fail "$why (title was '$t', wanted '*$needle*')"
    }
    Write-Host "title ok: $t"
}

# Document titles also end with the product name, so home needs an exact match.
function Assert-Home([string]$why) {
    $t = Get-Title
    if ($t -ne $Brand) {
        Fail "$why (title was '$t', wanted '$Brand')"
    }
    Write-Host "home ok: $t"
}

function Shot([string]$name) {
    # Park the cursor on the title bar so hover styles do not leak into the capture.
    $pt = New-Object C+P
    [void][C]::ClientToScreen($script:hwnd, [ref]$pt)
    [void][C]::SetCursorPos($pt.X + 4, $pt.Y + 4)
    Start-Sleep -Milliseconds 300
    $r = New-Object C+Rect
    [void][C]::GetWindowRect($script:hwnd, [ref]$r)
    $w = $r.Right - $r.Left; $h = $r.Bottom - $r.Top
    $bmp = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size $w, $h))
    if (-not (Test-Path -LiteralPath $ShotDir)) { New-Item -ItemType Directory -Path $ShotDir | Out-Null }
    $path = Join-Path $ShotDir $name
    if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Force }
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    Write-Host "shot $path"
}

function Key([byte]$vk) {
    if ([C]::GetForegroundWindow() -ne $script:hwnd) { Fail "foreground lost before key 0x$('{0:X2}' -f $vk)" }
    [C]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [C]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 700
}

function CtrlKey([byte]$vk) {
    if ([C]::GetForegroundWindow() -ne $script:hwnd) { Fail "foreground lost before ctrl key 0x$('{0:X2}' -f $vk)" }
    [C]::keybd_event($VK_CONTROL, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 50
    [C]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 50
    [C]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 50
    [C]::keybd_event($VK_CONTROL, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 800
}

# Client-area logical coordinates -> physical screen click. Layout is DPI-scaled.
function Click-Logical([double]$x, [double]$y) {
    if ([C]::GetForegroundWindow() -ne $script:hwnd) { Fail 'foreground lost before click' }
    $dpi = [C]::GetDpiForWindow($script:hwnd)
    $scale = if ($dpi -gt 0) { $dpi / 96.0 } else { 1.0 }
    $pt = New-Object C+P
    [void][C]::ClientToScreen($script:hwnd, [ref]$pt)
    $px = $pt.X + [int][math]::Round($x * $scale)
    $py = $pt.Y + [int][math]::Round($y * $scale)
    [void][C]::SetCursorPos($px, $py)
    Start-Sleep -Milliseconds 180
    [C]::mouse_event($LEFTDOWN, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 50
    [C]::mouse_event($LEFTUP, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 800
}

# Probe-owned scratch only.
if (Test-Path -LiteralPath $WorkDir) {
    Remove-Item -LiteralPath $WorkDir -Recurse -Force
}
New-Item -ItemType Directory -Path $WorkDir | Out-Null
$env:APPDATA = Join-Path $WorkDir 'appdata'
New-Item -ItemType Directory -Path $env:APPDATA | Out-Null

# --- Case A: default launch is the start page ---------------------------------
Start-App @{}
Assert-Home 'default launch should show the start page'
Shot 'm3-home-empty.png'

Key 0x34  # '4' = fishbone template
Assert-Title $Fishbone 'template card should open a named document'
Shot 'm3-home-template-fishbone.png'

CtrlKey $VK_W  # clean template, no prompt
Assert-Home 'closing the last tab should return to the start page'
Shot 'm3-home-after-close.png'

CtrlKey $VK_N
Assert-Title '.zmind' 'Ctrl+N on the start page should open a blank document'

# Hamburger -> ????? (8th row; menu opens under the button at x=10..268).
Click-Logical 27 28
Shot 'm3-home-menu.png'
Click-Logical 145 289
Assert-Home 'menu entry should cover the document with the start page'
Shot 'm3-home-return.png'

Key $VK_ESCAPE
Assert-Title '.zmind' 'Esc should return to the document that the menu covered'

CtrlKey $VK_W
Assert-Home 'closing the blank tab should return to the start page'
Close-App

# --- Case B: recent files -----------------------------------------------------
$seed = Join-Path $WorkDir 'quarterly-review.zmind'
Copy-Item -LiteralPath 'shots/qa/e5-fishbone.zmind' -Destination $seed
Start-App @{ MW_OPEN = $seed }
Assert-Title 'quarterly-review.zmind' 'MW_OPEN should load the seeded document'
Close-App

Start-App @{}
Assert-Home 'relaunch with a recent should still start at home'
Shot 'm3-home-recents.png'
Click-Logical 400 420  # first row of the recent-files card
Assert-Title 'quarterly-review.zmind' 'clicking the first recent row should open it'
Shot 'm3-home-recent-opened.png'
CtrlKey $VK_W
Assert-Home 'closing the opened recent should return to home'
Close-App

Write-Host 'M3 home probe passed'
