param(
    [string]$Exe = 'target/release/mw-shell.exe',
    [string]$ShotDir = 'shots/m2c-anim',
    [string]$WorkDir = "$env:TEMP\mw-anim-probe",
    [switch]$Calibrate
)

# M2c motion probe: real keyboard input on a fixture document.
# Cases: add child (Tab), delete (Delete), collapse/expand all (Ctrl+/ and
# Ctrl+Shift+/), and layout switch from the inspector (F9). For each case we
# grab early / mid / settled frames. Motion shots are timing evidence, not
# pixel baselines, so a frame may land early or late.
#
# Calibration: -Calibrate only opens the inspector and saves one shot; the
# layout button coordinates live in $FISHBONE / $MINDMAP below.
# ASCII only.

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
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
$VK_SHIFT = 0x10
$VK_ESCAPE = 0x1B
$VK_ENTER = 0x0D
$VK_TAB = 0x09
$VK_DELETE = 0x2E
$VK_F9 = 0x78
$VK_X = 0x58
$VK_Y = 0x59
$VK_OEM_2 = 0xBF

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

function New-Fixture([string]$path) {
    $manifest = '{"formatVersion":1,"app":"MindWeaver/0.1.0","docId":"anim-probe","title":"anim-probe","createdAt":1760000000000,"modifiedAt":1760000000000,"nodeCount":7}'
    $content = @'
{"schema":1,"rootId":"root","nodes":{
"root":{"id":"root","parentId":null,"children":["a","b","c"],"text":"\u4E2D\u5FC3\u4E3B\u9898"},
"a":{"id":"a","parentId":"root","children":["a1","a2"],"text":"\u9879\u76EE\u7532"},
"a1":{"id":"a1","parentId":"a","children":[],"text":"\u5B50\u9879\u7532\u4E00"},
"a2":{"id":"a2","parentId":"a","children":[],"text":"\u5B50\u9879\u7532\u4E8C"},
"b":{"id":"b","parentId":"root","children":["b1"],"text":"\u9879\u76EE\u4E59"},
"b1":{"id":"b1","parentId":"b","children":[],"text":"\u5B50\u9879\u4E59\u4E00"},
"c":{"id":"c","parentId":"root","children":[],"text":"\u9879\u76EE\u4E19"}
},"theme":"snow","layout":{"kind":"mindmap","siblingGap":24,"levelGap":80,"maxNodeWidth":300}}
'@
    $fs = [System.IO.File]::Open($path, [System.IO.FileMode]::Create)
    $zip = New-Object System.IO.Compression.ZipArchive($fs, [System.IO.Compression.ZipArchiveMode]::Create)
    function Add-Entry([string]$name, [string]$text, [System.IO.Compression.CompressionLevel]$level) {
        $entry = $zip.CreateEntry($name, $level)
        $s = $entry.Open()
        $bytes = [System.Text.Encoding]::UTF8.GetBytes($text)
        $s.Write($bytes, 0, $bytes.Length)
        $s.Dispose()
    }
    Add-Entry 'mimetype' 'application/vnd.mindweaver.zmind' ([System.IO.Compression.CompressionLevel]::NoCompression)
    Add-Entry 'manifest.json' $manifest ([System.IO.Compression.CompressionLevel]::Optimal)
    Add-Entry 'content.json' $content ([System.IO.Compression.CompressionLevel]::Optimal)
    $zip.Dispose(); $fs.Dispose()
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
            $probe = New-Object C+Rect
            if ([C]::GetWindowRect($h, [ref]$probe)) {
                $pw = $probe.Right - $probe.Left
                $ph = $probe.Bottom - $probe.Top
                if ($pw -ge 600 -and $ph -ge 400) { $script:hwnd = $h; $ready = $true; break }
            }
        }
    }
    if (-not $ready) { Fail 'no usable main window after 20s' }
    Start-Sleep -Milliseconds 1400
    [void][C]::BringWindowToTop($script:hwnd)
    try { (New-Object -ComObject WScript.Shell).AppActivate($script:p.Id) | Out-Null } catch { }
    [void][C]::SetForegroundWindow($script:hwnd)
    Start-Sleep -Milliseconds 500
    if ([C]::GetForegroundWindow() -ne $script:hwnd) {
        Fail 'window did not come to foreground; input would go to another app'
    }
    Write-Host ("app up: pid={0} hwnd={1}" -f $script:p.Id, $script:hwnd)
}

function Get-Title {
    $sb = New-Object System.Text.StringBuilder 512
    [void][C]::GetWindowTextW($script:hwnd, $sb, 512)
    $sb.ToString()
}

function Get-Scale {
    $dpi = [C]::GetDpiForWindow($script:hwnd)
    if ($dpi -gt 0) { return $dpi / 96.0 }
    return 1.0
}

function Move-Logical([double]$x, [double]$y) {
    $scale = Get-Scale
    $pt = New-Object C+P
    [void][C]::ClientToScreen($script:hwnd, [ref]$pt)
    [void][C]::SetCursorPos(($pt.X + [int][math]::Round($x * $scale)), ($pt.Y + [int][math]::Round($y * $scale)))
}

function Save-Frame([string]$name) {
    if (-not (Test-Path -LiteralPath $ShotDir)) { New-Item -ItemType Directory -Path $ShotDir | Out-Null }
    $r = New-Object C+Rect
    [void][C]::GetWindowRect($script:hwnd, [ref]$r)
    $w = $r.Right - $r.Left; $h = $r.Bottom - $r.Top
    $bmp = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size $w, $h))
    $out = Join-Path $ShotDir "$name.png"
    if (Test-Path -LiteralPath $out) { Remove-Item -LiteralPath $out -Force }
    $bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    Write-Host "  shot $name"
}

function Shot-Settled([string]$name) {
    # Park on the title bar first so hover styles do not leak in.
    $pt = New-Object C+P
    [void][C]::ClientToScreen($script:hwnd, [ref]$pt)
    [void][C]::SetCursorPos($pt.X + 4, $pt.Y + 4)
    Start-Sleep -Milliseconds 250
    Save-Frame $name
}

function Key-Down([byte]$vk) {
    [C]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 25
}

function Key-Up([byte]$vk) {
    [C]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
}

function Press-Key([byte]$vk, [int]$tail = 500) {
    if ([C]::GetForegroundWindow() -ne $script:hwnd) { Fail "foreground lost before key 0x$('{0:X2}' -f $vk)" }
    Key-Down $vk; Key-Up $vk
    Start-Sleep -Milliseconds $tail
}

function Press-Combo([byte[]]$keys, [int]$tail = 500) {
    if ([C]::GetForegroundWindow() -ne $script:hwnd) { Fail 'foreground lost before combo' }
    foreach ($vk in $keys) { Key-Down $vk }
    [array]::Reverse($keys)
    foreach ($vk in $keys) { Key-Up $vk }
    Start-Sleep -Milliseconds $tail
}

# Press a key/combo, then grab frames at two short delays plus the settled end.
function Motion-Shots([string]$name, [byte[]]$keys) {
    if ([C]::GetForegroundWindow() -ne $script:hwnd) { Fail 'foreground lost before motion case' }
    # Park on the header so hover anchors/rings do not leak into the frames.
    Move-Logical 300 40
    Start-Sleep -Milliseconds 150
    foreach ($vk in $keys) { Key-Down $vk }
    [array]::Reverse($keys)
    foreach ($vk in $keys) { Key-Up $vk }
    Save-Frame "$name-early"
    Start-Sleep -Milliseconds 70
    Save-Frame "$name-mid"
    Start-Sleep -Milliseconds 650
    Save-Frame "$name-settled"
}

function Click-Logical([double]$x, [double]$y, [int]$tail = 500) {
    if ([C]::GetForegroundWindow() -ne $script:hwnd) { Fail 'foreground lost before click' }
    Move-Logical $x $y
    Start-Sleep -Milliseconds 180
    [C]::mouse_event($LEFTDOWN, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 45
    [C]::mouse_event($LEFTUP, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds $tail
}

# Probe-owned scratch only.
if (Test-Path -LiteralPath $WorkDir) { Remove-Item -LiteralPath $WorkDir -Recurse -Force }
New-Item -ItemType Directory -Path $WorkDir | Out-Null
$env:APPDATA = Join-Path $WorkDir 'appdata'
New-Item -ItemType Directory -Path $env:APPDATA | Out-Null
$fixture = Join-Path $WorkDir 'anim.zmind'
New-Fixture $fixture

Start-App @{ MW_OPEN = $fixture }
$title = Get-Title
if ($title -notlike '*anim.zmind*') { Fail "MW_OPEN did not load the fixture (title '$title')" }
Write-Host "title ok: $title"
$scale = Get-Scale
Write-Host ("scale={0}" -f $scale)

# Layout buttons in the inspector, logical client px (calibrated on 1080x720 @1.5).
$FISHBONE = @(870, 496)
$MINDMAP = @(870, 467)

if ($Calibrate) {
    Press-Key $VK_F9 700
    Shot-Settled 'calib-inspector'
    Stop-Process -Id $script:p.Id -Force -ErrorAction SilentlyContinue
    exit 0
}

Press-Key $VK_ESCAPE 400
Shot-Settled 'anim-initial'

# 1. Add child: no selection (Esc cleared it) -> Tab adds a child of the root.
Motion-Shots 'anim-add' @($VK_TAB)
Press-Key $VK_X 400
# Commit by clicking empty canvas: Enter may be eaten by the IME composition.
Click-Logical 600 250 500
Shot-Settled 'anim-add-committed'

# 2. Select the new node by click, then delete it (fade toward the root).
Click-Logical 493 420 500
Motion-Shots 'anim-delete' @($VK_DELETE)
# Undo brings it back; the fade-in is the same interpolation running backwards.
Press-Combo @($VK_CONTROL, 0x5A) 800

# 3. Collapse all / expand all: mass fade + layout slide.
Motion-Shots 'anim-collapse' @($VK_CONTROL, $VK_OEM_2)
Motion-Shots 'anim-expand' @($VK_CONTROL, $VK_SHIFT, $VK_OEM_2)

# 4. Layout switch from the inspector: mindmap -> fishbone -> mindmap.
# Clear the selection first: with a node selected the inspector shows styles,
# not the document/layout section.
Press-Key $VK_ESCAPE 300
Press-Key $VK_F9 700
Shot-Settled 'anim-inspector'
Click-Logical $FISHBONE[0] $FISHBONE[1] 40
Save-Frame 'anim-layout-early'
Save-Frame 'anim-layout-mid'
Start-Sleep -Milliseconds 700
Shot-Settled 'anim-layout-settled'
Click-Logical $MINDMAP[0] $MINDMAP[1] 40
Save-Frame 'anim-layout-back-early'
Start-Sleep -Milliseconds 700
Shot-Settled 'anim-layout-back-settled'
Press-Key $VK_F9 500

Move-Logical 8 8
Stop-Process -Id $script:p.Id -Force -ErrorAction SilentlyContinue
Write-Host 'done'
