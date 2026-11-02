param(
    [string]$Exe = "target/release/mw-shell.exe",
    [string]$ShotDir = "shots/qa"
)

# v4 inline-editor probe (real input, release build).
#
# Claim under test: pressing F2 must not move a single glyph. The canvas draws node
# text through DirectWrite, the editor through egui/epaint - two rasterizers, one
# baseline rule (mw_core::layout::line_baseline). Here we measure the vertical ink
# profile of a 3-line node before and during editing, in light and dark themes.
#
# Asserts: same number of text rows, and every row cluster centroid within 2 px.
# ASCII only (PowerShell 5.1 reads BOM-less files as ANSI); CJK text is built from
# char codes so the fixture stays readable in any editor.

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.IO.Compression.FileSystem
Add-Type -ReferencedAssemblies System.Drawing -Language CSharp @'
using System;
using System.Drawing;
using System.Runtime.InteropServices;
public static class V4 {
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref P p);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, int data, IntPtr extra);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint f, IntPtr extra);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct P { public int X, Y; }

    static bool Inked(Color c, int bgR, int bgG, int bgB, int tol) {
        return Math.Abs(c.R - bgR) + Math.Abs(c.G - bgG) + Math.Abs(c.B - bgB) > tol;
    }

    /// Modal colour of the band = the canvas background (the band is mostly empty map).
    public static int[] Modal(Bitmap bmp, int x0, int y0, int x1, int y1) {
        var hist = new System.Collections.Generic.Dictionary<int, int>();
        for (int y = y0; y < y1; y += 2) {
            for (int x = x0; x < x1; x += 2) {
                Color c = bmp.GetPixel(x, y);
                int key = (c.R << 16) | (c.G << 8) | c.B;
                int n;
                hist.TryGetValue(key, out n);
                hist[key] = n + 1;
            }
        }
        int bestKey = 0, best = -1;
        foreach (var kv in hist) {
            if (kv.Value > best) { best = kv.Value; bestKey = kv.Key; }
        }
        return new int[] { (bestKey >> 16) & 0xFF, (bestKey >> 8) & 0xFF, bestKey & 0xFF };
    }

    /// Bounding box of pixels that differ from the sampled canvas background.
    /// Returns {minX,minY,maxX,maxY}, or null when nothing is inked.
    public static int[] InkBox(Bitmap bmp, int x0, int y0, int x1, int y1, int bgR, int bgG, int bgB, int tol) {
        int minX = int.MaxValue, minY = int.MaxValue, maxX = -1, maxY = -1;
        for (int y = y0; y < y1; y++) {
            for (int x = x0; x < x1; x++) {
                if (Inked(bmp.GetPixel(x, y), bgR, bgG, bgB, tol)) {
                    if (x < minX) minX = x;
                    if (x > maxX) maxX = x;
                    if (y < minY) minY = y;
                    if (y > maxY) maxY = y;
                }
            }
        }
        if (maxX < 0) return null;
        return new int[] { minX, minY, maxX, maxY };
    }

    /// Count of inked pixels per row inside the box (the vertical text profile).
    public static int[] Rows(Bitmap bmp, int x0, int y0, int x1, int y1, int bgR, int bgG, int bgB, int tol) {
        int[] p = new int[y1 - y0];
        for (int y = y0; y < y1; y++) {
            int n = 0;
            for (int x = x0; x < x1; x++) {
                if (Inked(bmp.GetPixel(x, y), bgR, bgG, bgB, tol)) n++;
            }
            p[y - y0] = n;
        }
        return p;
    }

    /// Text line clusters: flat triples {firstY, lastY, weightedCentroidY} for runs of
    /// rows holding at least `min` inked pixels. Gaps of one row are merged (CJK glyphs
    /// leave a bright seam between their top and bottom strokes). Runs shorter than
    /// `minH` are dropped: those are the card's rounded corners, not glyphs.
    public static double[] Lines(Bitmap bmp, int x0, int y0, int x1, int y1, int bgR, int bgG, int bgB, int tol, int min, int minH) {
        int[] p = Rows(bmp, x0, y0, x1, y1, bgR, bgG, bgB, tol);
        var outp = new System.Collections.Generic.List<double>();
        int start = -1, prev = -1;
        double sum = 0.0; int cnt = 0;
        for (int i = 0; i < p.Length; i++) {
            if (p[i] >= min) {
                if (cnt == 0) { start = i; }
                else if (i - prev > 1) {
                    Push(outp, y0, start, prev, sum, cnt, minH);
                    start = i; sum = 0.0; cnt = 0;
                }
                sum += (double)(y0 + i) * p[i]; cnt += p[i]; prev = i;
            }
        }
        if (cnt > 0) { Push(outp, y0, start, prev, sum, cnt, minH); }
        return outp.ToArray();
    }

    static void Push(System.Collections.Generic.List<double> outp, int y0, int start, int prev, double sum, int cnt, int minH) {
        if (prev - start + 1 < minH) return;
        outp.Add((double)y0 + start);
        outp.Add((double)y0 + prev);
        outp.Add(sum / cnt);
    }

    /// Weighted mean x of the inked pixels in a band: the horizontal text placement.
    public static double ColCentroid(Bitmap bmp, int x0, int y0, int x1, int y1, int bgR, int bgG, int bgB, int tol) {
        double sum = 0.0; int cnt = 0;
        for (int y = y0; y < y1; y++) {
            for (int x = x0; x < x1; x++) {
                if (Inked(bmp.GetPixel(x, y), bgR, bgG, bgB, tol)) { sum += x; cnt++; }
            }
        }
        return cnt > 0 ? sum / cnt : -1.0;
    }
}
'@
[void][V4]::SetProcessDPIAware()

$tmp = Join-Path $env:TEMP ("mw-v4-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
if (-not (Test-Path -LiteralPath $ShotDir)) { New-Item -ItemType Directory -Path $ShotDir | Out-Null }
$shotFull = (Resolve-Path -LiteralPath $ShotDir).Path
$envNoU8 = New-Object System.Text.UTF8Encoding $false

# 第一行 / 第二行 / 第三行 - three explicit lines, so the row count does not depend
# on wrap width and stays identical between the canvas and the editor.
$l1 = [string]([char]0x7B2C) + [char]0x4E00 + [char]0x884C
$l2 = [string]([char]0x7B2C) + [char]0x4E8C + [char]0x884C
$l3 = [string]([char]0x7B2C) + [char]0x4E09 + [char]0x884C
$text = ($l1, $l2, $l3) -join "\n"

function New-Fixture([string]$theme, [string]$path) {
    $dir = Join-Path $path ("doc-" + $theme)
    Remove-Item -Recurse -Force $dir -ErrorAction SilentlyContinue
    [void](New-Item -ItemType Directory -Force -Path $dir)
    $content = '{"schema":1,"rootId":"probe-root","nodes":{"probe-root":{"id":"probe-root","parentId":null,"children":[],"text":"' + $text + '","collapsed":false,"modifiedAt":0}},"theme":"' + $theme + '","layout":{"kind":"mindmap","siblingGap":24,"levelGap":80,"maxNodeWidth":300}}'
    $manifest = '{"formatVersion":1,"app":"probe/1.0","docId":"probe-' + $theme + '","title":"probe","createdAt":0,"modifiedAt":0,"nodeCount":1}'
    [System.IO.File]::WriteAllText((Join-Path $dir "content.json"), $content, $envNoU8)
    [System.IO.File]::WriteAllText((Join-Path $dir "manifest.json"), $manifest, $envNoU8)
    [System.IO.File]::WriteAllText((Join-Path $dir "mimetype"), "application/vnd.mindweaver.zmind", $envNoU8)
    $zip = Join-Path $path ($theme + ".zip")
    Remove-Item -Force $zip -ErrorAction SilentlyContinue
    [System.IO.Compression.ZipFile]::CreateFromDirectory($dir, $zip)
    $out = Join-Path $path ($theme + ".zmind")
    Remove-Item -Force $out -ErrorAction SilentlyContinue
    Move-Item -LiteralPath $zip -Destination $out
    return $out
}

$VK_HOME = 0x24; $VK_F2 = 0x71; $VK_END = 0x23; $VK_ESCAPE = 0x1B

function Press([byte]$vk) {
    [V4]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 50
    [V4]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 420
}
function Click([int]$x, [int]$y) {
    [void][V4]::SetCursorPos($x, $y)
    Start-Sleep -Milliseconds 120
    [V4]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 50
    [V4]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 400
}

$fail = 0
foreach ($theme in @("snow", "inkNight")) {
    $fx = New-Fixture $theme $tmp
    $env:APPDATA = (Join-Path $tmp ("app-" + $theme))
    New-Item -ItemType Directory -Force -Path $env:APPDATA | Out-Null
    $env:MW_HOME = "0"
    $env:MW_WINDOW_STATE = "off"
    $env:MW_OPEN = $fx

    $proc = Start-Process -FilePath $Exe -PassThru
    $hwnd = [IntPtr]::Zero
    for ($i = 0; $i -lt 60; $i++) {
        Start-Sleep -Milliseconds 250
        $proc.Refresh()
        $h = $proc.MainWindowHandle
        if ($h -ne [IntPtr]::Zero) {
            $r = New-Object V4+RECT
            [void][V4]::GetClientRect($h, [ref]$r)
            if ($r.Right -ge 600 -and $r.Bottom -ge 400) { $hwnd = $h; break }
        }
    }
    if ($hwnd -eq [IntPtr]::Zero) { Write-Error "window never appeared ($theme)"; exit 1 }
    [void][V4]::ShowWindow($hwnd, 9)
    [void][V4]::BringWindowToTop($hwnd)
    try { (New-Object -ComObject WScript.Shell).AppActivate($proc.Id) | Out-Null } catch { }
    [void][V4]::SetForegroundWindow($hwnd)
    Start-Sleep -Milliseconds 1000

    $rect = New-Object V4+RECT
    [void][V4]::GetClientRect($hwnd, [ref]$rect)
    $origin = New-Object V4+P
    [void][V4]::ClientToScreen($hwnd, [ref]$origin)
    $cw = $rect.Right; $ch = $rect.Bottom
    $scale = [V4]::GetDpiForWindow($hwnd) / 96.0

    function Shot([string]$name) {
        $bmp = New-Object System.Drawing.Bitmap $cw, $ch, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
        $g = [System.Drawing.Graphics]::FromImage($bmp)
        $g.CopyFromScreen($origin.X, $origin.Y, 0, 0, (New-Object System.Drawing.Size $cw, $ch))
        $g.Dispose()
        $p = $shotFull + "\" + $name + ".png"
        if (Test-Path -LiteralPath $p) { Remove-Item -LiteralPath $p -Force }
        $bmp.Save($p, [System.Drawing.Imaging.ImageFormat]::Png)
        return $bmp
    }

    Press $VK_HOME
    $a = Shot ("v4-" + $theme + "-canvas")
    # Scan band = middle of the canvas, away from panels, title bar and minimap.
    $bx0 = [int]($cw * 0.30); $bx1 = [int]($cw * 0.70)
    $by0 = [int]($ch * 0.20); $by1 = [int]($ch * 0.80)
    $bg = [V4]::Modal($a, $bx0, $by0, $bx1, $by1)
    $box = [V4]::InkBox($a, $bx0, $by0, $bx1, $by1, $bg[0], $bg[1], $bg[2], 90)
    if ($null -eq $box) { Write-Error "no node ink found ($theme)"; exit 1 }
    # Inset instead of expand: the box is the card, and its border strokes would make
    # every single row look inked. Text starts at PAD_X/PAD_Y (>=15 physical px) inside,
    # so a 5 px inset drops the border and keeps every glyph.
    $inset = 5
    $cx0 = $box[0] + $inset; $cy0 = $box[1] + $inset
    $cx1 = $box[2] + 1 - $inset; $cy1 = $box[3] + 1 - $inset

    Click ($origin.X + [int](($box[0] + $box[2]) / 2)) ($origin.Y + [int](($box[1] + $box[3]) / 2))
    Press $VK_F2
    Press $VK_END
    $b = Shot ("v4-" + $theme + "-editing")

    $la = [V4]::Lines($a, $cx0, $cy0, $cx1, $cy1, $bg[0], $bg[1], $bg[2], 90, 3, 25)
    $lb = [V4]::Lines($b, $cx0, $cy0, $cx1, $cy1, $bg[0], $bg[1], $bg[2], 90, 3, 25)
    $na = [int]($la.Length / 3); $nb = [int]($lb.Length / 3)
    Write-Host ($theme + " bg=" + ($bg -join ",") + " box=" + ($box -join ",") + " scale=" + $scale)
    Write-Host ("  lines canvas=" + $na + " editing=" + $nb)
    if ($na -ne 3 -or $nb -ne $na) {
        Write-Host "  FAIL expected 3 text lines in both states"
        $fail++
    }
    $n = [math]::Min($na, $nb)
    for ($i = 0; $i -lt $n; $i++) {
        $d = [math]::Abs($la[$i * 3 + 2] - $lb[$i * 3 + 2])
        $hA = $la[$i * 3 + 1] - $la[$i * 3] + 1
        $hB = $lb[$i * 3 + 1] - $lb[$i * 3] + 1
        Write-Host ("  line " + ($i + 1) + " y=" + [math]::Round($la[$i * 3 + 2], 2) + " -> " + [math]::Round($lb[$i * 3 + 2], 2) + " dy=" + [math]::Round($d, 2) + "px h=" + $hA + "/" + $hB)
        if ($d -gt 2.0) { Write-Host ("  FAIL line " + ($i + 1) + " moved " + [math]::Round($d, 2) + "px"); $fail++ }
    }
    # Horizontal placement on the middle line only: the caret sits at the end of the last
    # line and would drag a whole-crop centroid to the right.
    if ($na -ge 2 -and $nb -ge 2) {
        $m = 1
        $xa = [V4]::ColCentroid($a, $cx0, [int]$la[$m * 3], $cx1, [int]($la[$m * 3 + 1] + 1), $bg[0], $bg[1], $bg[2], 90)
        $xb = [V4]::ColCentroid($b, $cx0, [int]$la[$m * 3], $cx1, [int]($la[$m * 3 + 1] + 1), $bg[0], $bg[1], $bg[2], 90)
        $dx = [math]::Abs($xa - $xb)
        Write-Host ("  line 2 x=" + [math]::Round($xa, 2) + " -> " + [math]::Round($xb, 2) + " dx=" + [math]::Round($dx, 2) + "px")
        if ($dx -gt 2.0) { Write-Host ("  FAIL text slid " + [math]::Round($dx, 2) + "px sideways"); $fail++ }
    }
    $b.Dispose(); $a.Dispose()

    Press $VK_ESCAPE
    Start-Sleep -Milliseconds 300
    [void][V4]::SetWindowPos($hwnd, [IntPtr](-2), 0, 0, 0, 0, (0x0002 -bor 0x0001))
    Stop-Process -Id $proc.Id -Force
    Start-Sleep -Milliseconds 400
}

Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
if ($fail -gt 0) { Write-Host ("EDITOR_PARITY FAIL (" + $fail + ")"); exit 1 }
Write-Host "EDITOR_PARITY OK"
