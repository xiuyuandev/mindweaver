param(
    [string]$Exe = 'target/release/mw-shell.exe',
    [string]$ShotDir = 'shots/m2c-demo',
    [string]$WorkDir = "$env:TEMP\mw-demo-probe"
)

# M2c demo probe: real keyboard input on a fixture document.
# Asserts: F5 hides the light chrome (client pixel turns dark), arrows step,
# H hides/shows the control bar, Esc restores the light chrome and the title.
# Screenshots are timing evidence, not pixel baselines.
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
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref P p);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint f, IntPtr extra);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, uint data, IntPtr extra);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out Rect r);
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct P { public int X, Y; }
}
'@
}
[void][C]::SetProcessDPIAware()

$VK_ESCAPE = 0x1B
$VK_LEFT = 0x25
$VK_RIGHT = 0x27
$VK_H = 0x48
$VK_F5 = 0x74

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
    $manifest = '{"formatVersion":1,"app":"MindWeaver/0.1.0","docId":"demo-probe","title":"demo-probe","createdAt":1760000000000,"modifiedAt":1760000000000,"nodeCount":7}'
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
    # Park on the title bar / canvas top so hover styles do not leak in.
    $pt = New-Object C+P
    [void][C]::ClientToScreen($script:hwnd, [ref]$pt)
    [void][C]::SetCursorPos($pt.X + 4, $pt.Y + 4)
    Start-Sleep -Milliseconds 250
    Save-Frame $name
}

# Luminance of a client-area pixel (scaled to physical pixels).
function Client-Lum([double]$lx, [double]$ly) {
    $scale = Get-Scale
    $pt = New-Object C+P
    [void][C]::ClientToScreen($script:hwnd, [ref]$pt)
    $x = $pt.X + [int][math]::Round($lx * $scale)
    $y = $pt.Y + [int][math]::Round($ly * $scale)
    $bmp = New-Object System.Drawing.Bitmap 1, 1
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($x, $y, 0, 0, (New-Object System.Drawing.Size 1, 1))
    $c = $bmp.GetPixel(0, 0)
    $g.Dispose(); $bmp.Dispose()
    return 0.2126 * $c.R + 0.7152 * $c.G + 0.0722 * $c.B
}

function Press-Key([byte]$vk, [int]$tail = 500) {
    if ([C]::GetForegroundWindow() -ne $script:hwnd) { Fail "foreground lost before key 0x$('{0:X2}' -f $vk)" }
    [C]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 25
    [C]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds $tail
}

function Click-Logical([double]$x, [double]$y) {
    if ([C]::GetForegroundWindow() -ne $script:hwnd) { Fail 'foreground lost before click' }
    $scale = Get-Scale
    $pt = New-Object C+P
    [void][C]::ClientToScreen($script:hwnd, [ref]$pt)
    $px = $pt.X + [int][math]::Round($x * $scale)
    $py = $pt.Y + [int][math]::Round($y * $scale)
    [void][C]::SetCursorPos($px, $py)
    Start-Sleep -Milliseconds 60
    [C]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 40
    [C]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 450
}

# Control bar geometry mirrors demo_bar in main.rs: 460 wide (min 280), 76 from the
# bottom, 28-high buttons, 12 margin + 8 spacing. Offsets are from the right edge.
function Bar-Click([double]$dxFromRight, [string]$shot) {
    $r = New-Object C+Rect
    [void][C]::GetClientRect($script:hwnd, [ref]$r)
    $scale = Get-Scale
    $lw = ($r.Right - $r.Left) / $scale
    $lh = ($r.Bottom - $r.Top) / $scale
    $w = [math]::Min(460.0, $lw - 24.0)
    if ($w -lt 280.0) { $w = 280.0 }
    $x = ($lw - $w) * 0.5 + $w - $dxFromRight
    $y = $lh - 55.0
    Click-Logical $x $y
    Shot-Settled $shot
}

# Probe-owned scratch only.
if (Test-Path -LiteralPath $WorkDir) { Remove-Item -LiteralPath $WorkDir -Recurse -Force }
New-Item -ItemType Directory -Path $WorkDir | Out-Null
$env:APPDATA = Join-Path $WorkDir 'appdata'
New-Item -ItemType Directory -Path $env:APPDATA | Out-Null
$fixture = Join-Path $WorkDir 'demo.zmind'
New-Fixture $fixture

Start-App @{ MW_OPEN = $fixture }
$title = Get-Title
if ($title -notlike '*demo.zmind*') { Fail "MW_OPEN did not load the fixture (title '$title')" }
Write-Host "title ok: $title"
Write-Host ("scale={0}" -f (Get-Scale))

Press-Key $VK_ESCAPE 300
Shot-Settled 'demo-before'
$light = Client-Lum 30 30
Write-Host ("chrome luminance before: {0:N0}" -f $light)
Assert ($light -gt 180) 'title bar should be light before demo'

# 1. Enter: chrome disappears, canvas turns dark, camera eases to the root.
Press-Key $VK_F5 80
Save-Frame 'demo-enter-early'
Start-Sleep -Milliseconds 500
Shot-Settled 'demo-root'
$dark = Client-Lum 30 30
Write-Host ("canvas luminance during demo: {0:N0}" -f $dark)
Assert ($dark -lt 90) 'demo should hide the light chrome and paint a dark canvas'

# 2. Step forward: early/mid/settled frames show the 350ms camera glide.
Press-Key $VK_RIGHT 40
Save-Frame 'demo-next-early'
Start-Sleep -Milliseconds 80
Save-Frame 'demo-next-mid'
Start-Sleep -Milliseconds 500
Shot-Settled 'demo-next'
Press-Key $VK_RIGHT 550
Shot-Settled 'demo-next2'
Press-Key $VK_LEFT 550
Shot-Settled 'demo-prev'

# 3. H hides the control bar; press again to bring it back.
Press-Key $VK_H 300
Shot-Settled 'demo-bar-hidden'
Press-Key $VK_H 300
Shot-Settled 'demo-bar-shown'

# 3b. Bar buttons work with real mouse clicks: next, prev, exit.
Bar-Click 75 'demo-bar-next'
Bar-Click 113 'demo-bar-prev'
Bar-Click 32 'demo-bar-exit'
$barBack = Client-Lum 30 30
Assert ($barBack -gt 180) 'the exit button should restore the light chrome'
Assert ((Get-Title) -eq $title) 'bar exit must keep the document'

# 4. Keyboard exit: F5 in again, Esc restores the original viewport and chrome.
Press-Key $VK_F5 500
Press-Key $VK_ESCAPE 500
Shot-Settled 'demo-exit'
$back = Client-Lum 30 30
Write-Host ("chrome luminance after exit: {0:N0}" -f $back)
Assert ($back -gt 180) 'Esc should restore the light chrome'
$title2 = Get-Title
Assert ($title2 -eq $title) 'demo must not rename or replace the document'
$script:p.Refresh()
Assert (-not $script:p.HasExited) 'app should still be running after demo exit'

# 5. Exiting mid-flight must also restore the viewport (press F5 then Esc quickly).
Press-Key $VK_F5 60
Press-Key $VK_RIGHT 50
Press-Key $VK_ESCAPE 500
Shot-Settled 'demo-exit-midflight'
$mid = Client-Lum 30 30
Assert ($mid -gt 180) 'Esc during a camera glide should still restore the chrome'

Stop-Process -Id $script:p.Id -Force -ErrorAction SilentlyContinue
Write-Host 'done'
