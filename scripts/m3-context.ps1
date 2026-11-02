param(
    [string]$Exe = 'target/release/mw-shell.exe',
    [string]$ShotDir = 'shots/m3',
    [string]$WorkDir = "$env:TEMP\mw-context-probe",
    [switch]$Calibrate,
    [switch]$KeyDebug
)

# M3 probe: right-click context menu through real input on a fixture document.
# Node menu, keyboard navigation, blank menu (paste/fit/collapse), multi-select
# "move into same parent", copy via node menu + paste at root via blank menu,
# then save and inspect content.json for the resulting structure.
# The fixture is a 5-node map (root -> a(a1), b, c) with stable ASCII ids.
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
if (-not ('CtxScan' -as [type])) {
    Add-Type -Language CSharp -ReferencedAssemblies System.Drawing @'
using System;
using System.Collections.Generic;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Text;

public static class CtxScan {
    // Palette fills: teal=a, red=b, orange=c. White components: root and a1.
    static readonly string[] Names = new string[] { "teal", "red", "orange", "green", "blue" };
    static readonly int[][] Colors = new int[][] {
        new int[] { 0x7b, 0xcd, 0xaf },
        new int[] { 0xf7, 0x7f, 0x7f },
        new int[] { 0xf7, 0xb7, 0x7f },
        new int[] { 0x7f, 0xc4, 0x8f },
        new int[] { 0x8f, 0xaf, 0xf5 },
    };

    static byte[] Lock(Bitmap bmp, out int stride, out int w, out int h) {
        w = bmp.Width; h = bmp.Height;
        BitmapData d = bmp.LockBits(new Rectangle(0, 0, w, h), ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
        stride = d.Stride;
        byte[] bytes = new byte[stride * h];
        Marshal.Copy(d.Scan0, bytes, 0, bytes.Length);
        bmp.UnlockBits(d);
        return bytes;
    }

    public static string Fills(Bitmap bmp, double scale, int lx0, int ly0, int lx1, int ly1) {
        int stride, w, h;
        byte[] bytes = Lock(bmp, out stride, out w, out h);
        int x0 = Math.Max(0, (int)Math.Round(lx0 * scale)), x1 = Math.Min(w, (int)Math.Round(lx1 * scale));
        int y0 = Math.Max(0, (int)Math.Round(ly0 * scale)), y1 = Math.Min(h, (int)Math.Round(ly1 * scale));
        StringBuilder sb = new StringBuilder();
        for (int t = 0; t < Colors.Length; t++) {
            int tr = Colors[t][0], tg = Colors[t][1], tb = Colors[t][2];
            long count = 0, sumX = 0, sumY = 0;
            for (int y = y0; y < y1; y++) {
                int row = y * stride;
                for (int x = x0; x < x1; x++) {
                    int i = row + x * 4;
                    if (bytes[i] == tb && bytes[i + 1] == tg && bytes[i + 2] == tr) { count++; sumX += x; sumY += y; }
                }
            }
            if (count > 300) {
                sb.AppendLine(string.Format("{0} {1:F1} {2:F1} {3}", Names[t], (double)sumX / count / scale, (double)sumY / count / scale, count));
            } else {
                sb.AppendLine(string.Format("{0} none 0 0", Names[t]));
            }
        }
        return sb.ToString();
    }

    public static string Whites(Bitmap bmp, double scale, int lx0, int ly0, int lx1, int ly1) {
        int stride, w, h;
        byte[] bytes = Lock(bmp, out stride, out w, out h);
        int x0 = Math.Max(0, (int)Math.Round(lx0 * scale)), x1 = Math.Min(w, (int)Math.Round(lx1 * scale));
        int y0 = Math.Max(0, (int)Math.Round(ly0 * scale)), y1 = Math.Min(h, (int)Math.Round(ly1 * scale));
        int rw = x1 - x0, rh = y1 - y0;
        bool[] seen = new bool[rw * rh];
        bool[] white = new bool[rw * rh];
        for (int y = 0; y < rh; y++) {
            int row = (y + y0) * stride;
            for (int x = 0; x < rw; x++) {
                int i = row + (x + x0) * 4;
                white[y * rw + x] = bytes[i] == 255 && bytes[i + 1] == 255 && bytes[i + 2] == 255;
            }
        }
        var comps = new List<int[]>();
        var stack = new Stack<int>();
        for (int start = 0; start < white.Length; start++) {
            if (!white[start] || seen[start]) continue;
            int minX = int.MaxValue, minY = int.MaxValue, maxX = -1, maxY = -1, area = 0;
            stack.Push(start); seen[start] = true;
            while (stack.Count > 0) {
                int p = stack.Pop();
                int py = p / rw, px = p % rw;
                area++;
                if (px < minX) minX = px; if (px > maxX) maxX = px;
                if (py < minY) minY = py; if (py > maxY) maxY = py;
                if (px > 0 && white[p - 1] && !seen[p - 1]) { seen[p - 1] = true; stack.Push(p - 1); }
                if (px < rw - 1 && white[p + 1] && !seen[p + 1]) { seen[p + 1] = true; stack.Push(p + 1); }
                if (py > 0 && white[p - rw] && !seen[p - rw]) { seen[p - rw] = true; stack.Push(p - rw); }
                if (py < rh - 1 && white[p + rw] && !seen[p + rw]) { seen[p + rw] = true; stack.Push(p + rw); }
            }
            if (area >= 800) {
                double cx = (x0 + (minX + maxX) / 2.0) / scale;
                double cy = (y0 + (minY + maxY) / 2.0) / scale;
                comps.Add(new int[] { (int)Math.Round(cx * 10), (int)Math.Round(cy * 10), area,
                    (int)Math.Round((x0 + minX) / scale), (int)Math.Round((y0 + minY) / scale),
                    (int)Math.Round((x0 + maxX) / scale), (int)Math.Round((y0 + maxY) / scale) });
            }
        }
        comps.Sort(delegate (int[] a, int[] b) { return b[2].CompareTo(a[2]); });
        StringBuilder sb = new StringBuilder();
        foreach (int[] c in comps) {
            sb.AppendLine(string.Format("white {0:F1} {1:F1} {2} {3} {4} {5} {6}",
                c[0] / 10.0, c[1] / 10.0, c[2], c[3], c[4], c[5], c[6]));
        }
        return sb.ToString();
    }
}
'@
}
[void][C]::SetProcessDPIAware()

$RIGHTDOWN = 0x0008
$RIGHTUP = 0x0010
$LEFTDOWN = 0x0002
$LEFTUP = 0x0004
$VK_CONTROL = 0x11
$VK_ESCAPE = 0x1B
$VK_HOME = 0x24
$VK_DOWN = 0x28
$VK_ENTER = 0x0D
$VK_S = 0x53

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
    Start-Sleep -Milliseconds 400
    if ([C]::GetForegroundWindow() -ne $script:hwnd) {
        Fail 'window did not come to foreground; input would go to another app'
    }
    Write-Host ("app up: pid={0} hwnd={1}" -f $script:p.Id, $script:hwnd)
}

function Close-App {
    [void]$script:p.CloseMainWindow()
    if (-not $script:p.WaitForExit(7000)) { Fail 'app did not exit after CloseMainWindow' }
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
    if ($t -notlike "*$needle*") { Fail "$why (title was '$t', wanted '*$needle*')" }
    Write-Host "title ok: $t"
}

function Get-Scale {
    $dpi = [C]::GetDpiForWindow($script:hwnd)
    if ($dpi -gt 0) { return $dpi / 96.0 }
    return 1.0
}

function Get-ClientLogical {
    $scale = Get-Scale
    $rect = New-Object C+Rect
    [void][C]::GetClientRect($script:hwnd, [ref]$rect)
    $logicalW = ($rect.Right - $rect.Left) / $scale
    $logicalH = ($rect.Bottom - $rect.Top) / $scale
    return @($logicalW, $logicalH)
}

function Get-Frame {
    $r = New-Object C+Rect
    [void][C]::GetWindowRect($script:hwnd, [ref]$r)
    $w = $r.Right - $r.Left; $h = $r.Bottom - $r.Top
    $bmp = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($r.Left, $r.Top, 0, 0, (New-Object System.Drawing.Size $w, $h))
    $g.Dispose()
    return $bmp
}

function Pixel-Logical([System.Drawing.Bitmap]$bmp, [double]$x, [double]$y) {
    $scale = Get-Scale
    $r = New-Object C+Rect
    $org = New-Object C+P
    [void][C]::GetWindowRect($script:hwnd, [ref]$r)
    [void][C]::ClientToScreen($script:hwnd, [ref]$org)
    $px = [int][math]::Round($org.X - $r.Left + $x * $scale)
    $py = [int][math]::Round($org.Y - $r.Top + $y * $scale)
    if ($px -lt 0 -or $py -lt 0 -or $px -ge $bmp.Width -or $py -ge $bmp.Height) { Fail "pixel out of frame: $x,$y" }
    $c = $bmp.GetPixel($px, $py)
    return ("{0},{1},{2}" -f $c.R, $c.G, $c.B)
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

function Key([byte]$vk, [int]$pause = 700) {
    if ([C]::GetForegroundWindow() -ne $script:hwnd) { Fail "foreground lost before key 0x$('{0:X2}' -f $vk)" }
    [C]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [C]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds $pause
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

function Move-Logical([double]$x, [double]$y) {
    $scale = Get-Scale
    $pt = New-Object C+P
    [void][C]::ClientToScreen($script:hwnd, [ref]$pt)
    [void][C]::SetCursorPos(($pt.X + [int][math]::Round($x * $scale)), ($pt.Y + [int][math]::Round($y * $scale)))
}

function Click-Logical([double]$x, [double]$y) {
    if ([C]::GetForegroundWindow() -ne $script:hwnd) { Fail 'foreground lost before click' }
    Move-Logical $x $y
    Start-Sleep -Milliseconds 180
    [C]::mouse_event($LEFTDOWN, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 50
    [C]::mouse_event($LEFTUP, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 800
}

function RightClick-Logical([double]$x, [double]$y) {
    if ([C]::GetForegroundWindow() -ne $script:hwnd) { Fail 'foreground lost before right click' }
    Move-Logical $x $y
    Start-Sleep -Milliseconds 200
    [C]::mouse_event($RIGHTDOWN, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 50
    [C]::mouse_event($RIGHTUP, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 700
}

function CtrlClick-Logical([double]$x, [double]$y) {
    if ([C]::GetForegroundWindow() -ne $script:hwnd) { Fail 'foreground lost before ctrl click' }
    Move-Logical $x $y
    [C]::keybd_event($VK_CONTROL, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 80
    [C]::mouse_event($LEFTDOWN, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 50
    [C]::mouse_event($LEFTUP, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 80
    [C]::keybd_event($VK_CONTROL, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 700
}

# Menu card position as the widget clamps it (WIDTH 240, EDGE 8).
function Card-Pos([double]$x, [double]$y, [double]$height) {
    $client = Get-ClientLogical
    $cw = $client[0]; $ch = $client[1]
    $cx = [math]::Min($x, $cw - 240 - 8)
    $cx = [math]::Max($cx, 8)
    $cy = $y
    if ($cy + $height -gt $ch - 8) { $cy = $y - $height }
    $cy = [math]::Min($cy, $ch - $height - 8)
    $cy = [math]::Max($cy, 8)
    return @($cx, $cy)
}

function Get-FillMap([string]$text) {
    $map = @{}
    foreach ($line in ($text -split "`n")) {
        $parts = $line.Trim() -split ' '
        if ($parts.Count -ge 4 -and $parts[1] -ne 'none') {
            $map[$parts[0]] = @([double]$parts[1], [double]$parts[2])
        }
    }
    return $map
}

function New-Fixture([string]$path) {    $manifest = '{"formatVersion":1,"app":"MindWeaver/0.1.0","docId":"ctx-probe","title":"context-probe","createdAt":1760000000000,"modifiedAt":1760000000000,"nodeCount":5}'
    $content = @'
{"schema":1,"rootId":"root","nodes":{
"root":{"id":"root","parentId":null,"children":["a","b","c"],"text":"\u4E2D\u5FC3\u4E3B\u9898"},
"a":{"id":"a","parentId":"root","children":["a1"],"text":"\u9879\u76EE\u7532"},
"a1":{"id":"a1","parentId":"a","children":[],"text":"\u5B50\u9879\u7532\u4E00"},
"b":{"id":"b","parentId":"root","children":[],"text":"\u9879\u76EE\u4E59"},
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

# Probe-owned scratch only.
if (Test-Path -LiteralPath $WorkDir) { Remove-Item -LiteralPath $WorkDir -Recurse -Force }
New-Item -ItemType Directory -Path $WorkDir | Out-Null
$env:APPDATA = Join-Path $WorkDir 'appdata'
New-Item -ItemType Directory -Path $env:APPDATA | Out-Null
$fixture = Join-Path $WorkDir 'context.zmind'
New-Fixture $fixture

Start-App @{ MW_OPEN = $fixture }
Assert-Title 'context.zmind' 'MW_OPEN should load the fixture'

$scale = Get-Scale
$client = Get-ClientLogical
Write-Host ("scale={0} client={1}x{2}" -f $scale, $client[0], $client[1])

Key $VK_HOME 1000

$bmp = Get-Frame
$fills = [CtxScan]::Fills($bmp, $scale, 240, 100, 1000, 520)
$whites = [CtxScan]::Whites($bmp, $scale, 240, 100, 1000, 520)
$bmp.Dispose()
Write-Host '--- fills ---'
Write-Host $fills
Write-Host '--- white cards ---'
Write-Host $whites

$fillMap = Get-FillMap $fills
Assert $fillMap.ContainsKey('teal') 'fixture should show the teal level-1 node (a)'
Assert $fillMap.ContainsKey('red') 'fixture should show the red level-1 node (b)'
$ax = $fillMap['teal'][0]; $ay = $fillMap['teal'][1]
$bx = $fillMap['red'][0]; $by = $fillMap['red'][1]
Write-Host ("a=({0},{1}) b=({2},{3})" -f $ax, $ay, $bx, $by)

$cards = @()
$i = 0
foreach ($line in ($whites -split "`n")) {
    $parts = $line.Trim() -split ' '
    if ($parts.Count -ge 7 -and $parts[0] -eq 'white') {
        $cards += [pscustomobject]@{ i = $i; x = [double]$parts[1]; y = [double]$parts[2]; area = [int]$parts[3] }
        $i++
    }
}
Assert ($cards.Count -ge 2) "expected at least the root and a1 white cards, found $($cards.Count)"

$cxCanvas = (233 + $client[0]) / 2.0
$cyCanvas = (56 + $client[1] - 20) / 2.0
$rootCard = $cards | Sort-Object { [math]::Pow($_.x - $cxCanvas, 2) + [math]::Pow($_.y - $cyCanvas, 2) } | Select-Object -First 1
$a1Card = $cards | Where-Object { $_.i -ne $rootCard.i } |
    Sort-Object { [math]::Pow($_.x - $ax, 2) + [math]::Pow($_.y - $ay, 2) } | Select-Object -First 1
Write-Host ("root-card=({0},{1}) a1-card=({2},{3})" -f $rootCard.x, $rootCard.y, $a1Card.x, $a1Card.y)
$a1x = $a1Card.x; $a1y = $a1Card.y

if ($Calibrate) {
    Shot 'm3-context-calib-fixture.png'
    Close-App
    Write-Host 'calibration done'
    exit 0
}

# --- 1. Node menu -------------------------------------------------------------
RightClick-Logical $ax $ay
Shot 'm3-context-node.png'
Key $VK_ESCAPE 400

# --- 2. Keyboard navigation (two downs highlight the second row) --------------
RightClick-Logical $ax $ay
Key $VK_DOWN 250
Key $VK_DOWN 250
Shot 'm3-context-nav.png'
Key $VK_ESCAPE 400

# --- 3. Blank menu: open pixel is white, Esc restores the canvas --------------
$blankX = 300.0
$blankY = 600.0
$blankHeight = 219.0
$card = Card-Pos $blankX $blankY $blankHeight
$probeX = $card[0] + 120
$probeY = $card[1] + 105
$bmp = Get-Frame
$before = Pixel-Logical $bmp $probeX $probeY
$bmp.Dispose()

RightClick-Logical $blankX $blankY
Shot 'm3-context-blank.png'
$bmp = Get-Frame
$openPx = Pixel-Logical $bmp $probeX $probeY
$bmp.Dispose()
Assert ($openPx -eq '255,255,255') "blank menu card should be white at $probeX,$probeY (got $openPx)"
Key $VK_ESCAPE 400
$bmp = Get-Frame
$after = Pixel-Logical $bmp $probeX $probeY
$bmp.Dispose()
Assert ($after -eq $before) "Esc should restore the canvas at $probeX,$probeY (before=$before after=$after)"
Write-Host "blank menu open/close ok ($before -> $openPx -> $after)"

if ($KeyDebug) {
    RightClick-Logical $blankX $blankY
    Key $VK_DOWN 400
    Shot 'm3-context-key-1.png'
    Key $VK_DOWN 400
    Shot 'm3-context-key-2.png'
    Key $VK_DOWN 400
    Shot 'm3-context-key-3.png'
    Key $VK_DOWN 400
    Shot 'm3-context-key-4.png'
    Key $VK_DOWN 400
    Shot 'm3-context-key-5.png'
    Key $VK_DOWN 400
    Shot 'm3-context-key-6.png'
    Key $VK_ESCAPE 400
    Close-App
    Write-Host 'key debug done'
    exit 0
}

# --- 4. Multi-select: move b under a (primary a1's parent) --------------------
Click-Logical $bx $by
CtrlClick-Logical $a1x $a1y
RightClick-Logical $a1x $a1y
Shot 'm3-context-multi.png'
$multiHeight = 171.0
$card = Card-Pos $a1x $a1y $multiHeight
Click-Logical ($card[0] + 120) ($card[1] + 81)
Start-Sleep -Milliseconds 900
Shot 'm3-context-multi-after.png'

# The layout re-centers after a structural change; re-locate a before copying.
$bmp = Get-Frame
$fillsAfter = [CtxScan]::Fills($bmp, $scale, 240, 100, 1000, 520)
$bmp.Dispose()
Write-Host '--- fills after move ---'
Write-Host $fillsAfter
$fillAfter = Get-FillMap $fillsAfter
Assert ($fillAfter.ContainsKey('teal')) 'teal node a should be visible after the move'
$ax = $fillAfter['teal'][0]; $ay = $fillAfter['teal'][1]
Write-Host ("a after move=({0},{1})" -f $ax, $ay)

# --- 5. Copy a (node menu) then paste at root (blank menu) --------------------
RightClick-Logical $ax $ay
$nodeHeight = 369.0
$card = Card-Pos $ax $ay $nodeHeight
Click-Logical ($card[0] + 120) ($card[1] + 120)
Start-Sleep -Milliseconds 500
RightClick-Logical $blankX $blankY
$card = Card-Pos $blankX $blankY $blankHeight
Click-Logical ($card[0] + 120) ($card[1] + 21)
Start-Sleep -Milliseconds 900
Shot 'm3-context-paste.png'

# --- 6. Keyboard: five downs then Enter = collapse all ----------------------
RightClick-Logical $blankX $blankY
Key $VK_DOWN 400
Key $VK_DOWN 400
Key $VK_DOWN 400
Key $VK_DOWN 400
Key $VK_DOWN 400
Shot 'm3-context-collapse-highlight.png'
Key $VK_ENTER 900
Shot 'm3-context-collapsed.png'

CtrlKey $VK_S
Start-Sleep -Milliseconds 700
Close-App

# --- 7. Inspect the saved document ------------------------------------------
$zip = [System.IO.Compression.ZipFile]::OpenRead($fixture)
try {
    $entry = $zip.GetEntry('content.json')
    $reader = New-Object System.IO.StreamReader($entry.Open(), [System.Text.Encoding]::UTF8)
    $contentJson = $reader.ReadToEnd()
    $reader.Dispose()
    $entry = $zip.GetEntry('manifest.json')
    $reader = New-Object System.IO.StreamReader($entry.Open(), [System.Text.Encoding]::UTF8)
    $manifestJson = $reader.ReadToEnd()
    $reader.Dispose()
} finally {
    $zip.Dispose()
}
$manifest = $manifestJson | ConvertFrom-Json
$content = $contentJson | ConvertFrom-Json
$nodes = $content.nodes
$count = ($nodes.PSObject.Properties | Measure-Object).Count
$manifestCount = [int]$manifest.nodeCount
Write-Host ("saved: nodeCount={0} (manifest {1}) rootChildren={2}" -f $count, $manifestCount, $nodes.root.children.Count)

Assert ($count -eq 8) "expected 8 nodes after copy+paste (got $count)"
Assert ($manifestCount -eq 8) "manifest nodeCount should be 8 (got $manifestCount)"
Assert ($nodes.b.parentId -eq 'a') "b should move under a (parent is '$($nodes.b.parentId)')"
Assert ($nodes.root.children.Count -eq 3) "root should keep a, c and the pasted copy (got $($nodes.root.children.Count))"
$bCopies = @($nodes.PSObject.Properties | Where-Object { $_.Value.text -eq ([string][char]0x9879 + [string][char]0x76EE + [string][char]0x4E59) }).Count
$a1Copies = @($nodes.PSObject.Properties | Where-Object { $_.Value.text -eq ([string][char]0x5B50 + [string][char]0x9879 + [string][char]0x7532 + [string][char]0x4E00) }).Count
Assert ($bCopies -eq 2) "project-B should exist twice after copy+paste (got $bCopies)"
Assert ($a1Copies -eq 2) "a1 should exist twice after copy+paste (got $a1Copies)"
Assert ($nodes.a.collapsed -eq $true) 'collapse all (keyboard Enter) should collapse a'
Assert ($nodes.root.collapsed -eq $false) 'root must stay expanded'
Write-Host 'M3 context probe passed'
