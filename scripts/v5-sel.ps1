param(
    [string]$Exe = "target/release/mw-shell.exe",
    [string]$ShotDir = "shots/qa"
)

# v5-b editor pointer-ownership probe (real input, release build).
#
# Two claims about the inline editor, both about who owns a left press that lands
# inside the node being edited. They are tested in separate process runs so a
# failure of the first cannot confuse the second.
#
#   1. A single click inside the editing text must MOVE THE CARET there. Proof is
#      not visual: paste a marker, save, unzip content.json and check which line
#      the marker ended up in. "Caret jumps to the end" and "the click committed
#      the edit" both fail this in different ways, and the report tells them apart.
#   2. A left drag inside the editing text must select text, NOT pan the camera.
#      `classify()` returns DragKind::Pan while editing (docs/13 4.20 note 5), so
#      the canvas may arm a pan gesture on that same press. If it fires, the whole
#      card moves: compare the card's ink box before/after. Selection itself is
#      measured against a no-input control shot over the same interval, because the
#      caret blinks and would otherwise look like a change.
#
# ASCII only (PowerShell 5.1 reads BOM-less files as ANSI); the CJK fixture text is
# built from char codes so the file stays readable in any editor.

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.IO.Compression.FileSystem
Add-Type -ReferencedAssemblies System.Drawing -Language CSharp @'
using System;
using System.Drawing;
using System.Runtime.InteropServices;
public static class V5S {
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref P p);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
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

    /// First/last column holding ink inside a band: the horizontal span of one text line.
    public static int[] ColSpan(Bitmap bmp, int x0, int y0, int x1, int y1, int bgR, int bgG, int bgB, int tol) {
        int first = -1, last = -1;
        for (int x = x0; x < x1; x++) {
            bool hit = false;
            for (int y = y0; y < y1 && !hit; y++) {
                if (Inked(bmp.GetPixel(x, y), bgR, bgG, bgB, tol)) hit = true;
            }
            if (hit) { if (first < 0) first = x; last = x; }
        }
        return new int[] { first, last };
    }

    /// Text line clusters: flat triples {firstY, lastY, weightedCentroidY}.
    public static double[] Lines(Bitmap bmp, int x0, int y0, int x1, int y1, int bgR, int bgG, int bgB, int tol, int min, int minH) {
        var outp = new System.Collections.Generic.List<double>();
        int[] p = new int[y1 - y0];
        for (int y = y0; y < y1; y++) {
            int n = 0;
            for (int x = x0; x < x1; x++) {
                if (Inked(bmp.GetPixel(x, y), bgR, bgG, bgB, tol)) n++;
            }
            p[y - y0] = n;
        }
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

    /// Pixels that changed between two shots inside a band (selection evidence).
    public static int Changed(Bitmap a, Bitmap b, int x0, int y0, int x1, int y1, int tol) {
        int n = 0;
        for (int y = y0; y < y1; y++) {
            for (int x = x0; x < x1; x++) {
                Color ca = a.GetPixel(x, y), cb = b.GetPixel(x, y);
                if (Math.Abs(ca.R - cb.R) + Math.Abs(ca.G - cb.G) + Math.Abs(ca.B - cb.B) > tol) n++;
            }
        }
        return n;
    }
}
'@
[void][V5S]::SetProcessDPIAware()

$tmp = Join-Path $env:TEMP ("mw-v5s-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
if (-not (Test-Path -LiteralPath $ShotDir)) { New-Item -ItemType Directory -Path $ShotDir | Out-Null }
$shotFull = (Resolve-Path -LiteralPath $ShotDir).Path
$encNoU8 = New-Object System.Text.UTF8Encoding $false

# Three lines ("line 1".."line 3" in CJK) so a click "inside line 1" is
# unambiguous once we read the text back.
$l1 = [string]([char]0x7B2C) + [char]0x4E00 + [char]0x884C
$l2 = [string]([char]0x7B2C) + [char]0x4E8C + [char]0x884C
$l3 = [string]([char]0x7B2C) + [char]0x4E09 + [char]0x884C
$text = ($l1, $l2, $l3) -join "\n"

function New-Fixture([string]$name) {
    $dir = Join-Path $tmp ("doc-" + $name)
    [void](New-Item -ItemType Directory -Force -Path $dir)
    $content = '{"schema":1,"rootId":"probe-root","nodes":{"probe-root":{"id":"probe-root","parentId":null,"children":[],"text":"' + $script:text + '","collapsed":false,"modifiedAt":0}},"theme":"snow","layout":{"kind":"mindmap","siblingGap":24,"levelGap":80,"maxNodeWidth":300}}'
    $manifest = '{"formatVersion":1,"app":"probe/1.0","docId":"probe-' + $name + '","title":"probe","createdAt":0,"modifiedAt":0,"nodeCount":1}'
    [System.IO.File]::WriteAllText((Join-Path $dir "content.json"), $content, $script:encNoU8)
    [System.IO.File]::WriteAllText((Join-Path $dir "manifest.json"), $manifest, $script:encNoU8)
    [System.IO.File]::WriteAllText((Join-Path $dir "mimetype"), "application/vnd.mindweaver.zmind", $script:encNoU8)
    $zip = Join-Path $tmp ($name + ".zip")
    Remove-Item -Force $zip -ErrorAction SilentlyContinue
    [System.IO.Compression.ZipFile]::CreateFromDirectory($dir, $zip)
    $out = Join-Path $tmp ($name + ".zmind")
    Remove-Item -Force $out -ErrorAction SilentlyContinue
    Move-Item -LiteralPath $zip -Destination $out
    return $out
}

$VK_HOME = 0x24; $VK_F2 = 0x71; $VK_END = 0x23; $VK_ESCAPE = 0x1B; $VK_CTRL = 0x11; $VK_S = 0x53; $VK_V = 0x56

function Press([byte]$vk) {
    [V5S]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 50
    [V5S]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 420
}
function Ctrl([byte]$vk) {
    [V5S]::keybd_event($VK_CTRL, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 40
    [V5S]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 50
    [V5S]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 40
    [V5S]::keybd_event($VK_CTRL, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 700
}
function Click([int]$x, [int]$y) {
    [void][V5S]::SetCursorPos($x, $y)
    Start-Sleep -Milliseconds 150
    [V5S]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [V5S]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 420
}
function Drag([int]$x0, [int]$y0, [int]$x1, [int]$y1) {
    [void][V5S]::SetCursorPos($x0, $y0)
    Start-Sleep -Milliseconds 150
    [V5S]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    for ($i = 1; $i -le 10; $i++) {
        $x = [int]($x0 + ($x1 - $x0) * $i / 10)
        $y = [int]($y0 + ($y1 - $y0) * $i / 10)
        [void][V5S]::SetCursorPos($x, $y)
        Start-Sleep -Milliseconds 40
    }
    [V5S]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 450
}

# Launch, wait for a real client rect, and hand back the window handles plus the
# canvas-band measurements every claim needs.
function Start-Session([string]$tag, [string]$fx) {
    $env:APPDATA = (Join-Path $tmp ("app-" + $tag))
    [void](New-Item -ItemType Directory -Force -Path $env:APPDATA)
    $env:MW_HOME = "0"
    $env:MW_WINDOW_STATE = "off"
    $env:MW_OPEN = $fx

    $p = Start-Process -FilePath $Exe -PassThru
    $h = [IntPtr]::Zero
    for ($i = 0; $i -lt 60; $i++) {
        Start-Sleep -Milliseconds 250
        $p.Refresh()
        $w = $p.MainWindowHandle
        if ($w -ne [IntPtr]::Zero) {
            $r = New-Object V5S+RECT
            [void][V5S]::GetClientRect($w, [ref]$r)
            if ($r.Right -ge 600 -and $r.Bottom -ge 400) { $h = $w; break }
        }
    }
    if ($h -eq [IntPtr]::Zero) { Write-Error "window never appeared ($tag)"; exit 1 }
    [void][V5S]::ShowWindow($h, 9)
    [void][V5S]::BringWindowToTop($h)
    try { (New-Object -ComObject WScript.Shell).AppActivate($p.Id) | Out-Null } catch { }
    [void][V5S]::SetForegroundWindow($h)
    Start-Sleep -Milliseconds 1200

    $rc = New-Object V5S+RECT
    [void][V5S]::GetClientRect($h, [ref]$rc)
    $op = New-Object V5S+P
    [void][V5S]::ClientToScreen($h, [ref]$op)

    $s = [ordered]@{
        proc = $p; hwnd = $h; cw = $rc.Right; ch = $rc.Bottom
        ox = $op.X; oy = $op.Y; tag = $tag
        scale = ([V5S]::GetDpiForWindow($h) / 96.0)
    }
    return $s
}

function Shot($s, [string]$name) {
    $cw = $s.cw; $ch = $s.ch
    $bmp = New-Object System.Drawing.Bitmap $cw, $ch, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($s.ox, $s.oy, 0, 0, (New-Object System.Drawing.Size $cw, $ch))
    $g.Dispose()
    $p = $shotFull + "\" + $name + ".png"
    if (Test-Path -LiteralPath $p) { Remove-Item -LiteralPath $p -Force }
    $bmp.Save($p, [System.Drawing.Imaging.ImageFormat]::Png)
    return $bmp
}

function End-Session($s) {
    Press $VK_ESCAPE
    Start-Sleep -Milliseconds 300
    [void][V5S]::SetWindowPos($s.hwnd, [IntPtr](-2), 0, 0, 0, 0, (0x0002 -bor 0x0001))
    Stop-Process -Id $s.proc.Id -Force
    Start-Sleep -Milliseconds 500
}

function CardLook($s, $bmp) {
    $cw = $s.cw; $ch = $s.ch
    $bx0 = [int]($cw * 0.30); $bx1 = [int]($cw * 0.70)
    $by0 = [int]($ch * 0.20); $by1 = [int]($ch * 0.80)
    $bg = [V5S]::Modal($bmp, $bx0, $by0, $bx1, $by1)
    $box = [V5S]::InkBox($bmp, $bx0, $by0, $bx1, $by1, $bg[0], $bg[1], $bg[2], 90)
    return [ordered]@{ bx0 = $bx0; bx1 = $bx1; by0 = $by0; by1 = $by1; bg = $bg; box = $box }
}

$fail = 0

# -- Claim 1: a click inside the editing text puts the caret where it was clicked.
$fx1 = New-Fixture "caret"
$s = Start-Session "caret" $fx1
Press $VK_HOME

$canvas = Shot $s "v5s-canvas"
$look = CardLook $s $canvas
if ($null -eq $look.box) { Write-Error "no node ink found"; exit 1 }
$box = $look.box; $bg = $look.bg
$inset = 5
$cx0 = $box[0] + $inset; $cy0 = $box[1] + $inset
$cx1 = $box[2] + 1 - $inset; $cy1 = $box[3] + 1 - $inset
$lines = [V5S]::Lines($canvas, $cx0, $cy0, $cx1, $cy1, $bg[0], $bg[1], $bg[2], 90, 3, 25)
$nlines = [int]($lines.Length / 3)
Write-Host ("[caret] client=" + $s.cw + "x" + $s.ch + " scale=" + $s.scale + " card=" + ($box -join ","))
if ($nlines -lt 3) { Write-Host "  FAIL expected 3 text lines on the canvas"; exit 1 }

# Line 1's own horizontal span -> a click target 45 % through it, well clear of both
# the caret at the end and any word boundary at the very start.
$span = [V5S]::ColSpan($canvas, $cx0, [int]$lines[0], $cx1, [int]($lines[1] + 1), $bg[0], $bg[1], $bg[2], 90)
if ($span[0] -lt 0) { Write-Host "  FAIL line 1 has no ink"; exit 1 }
$clickX = $s.ox + [int]($span[0] + ($span[1] - $span[0]) * 0.45)
$clickY = $s.oy + [int](($lines[0] + $lines[1]) / 2)
Write-Host ("[caret] line1 span=" + ($span -join ",") + " click=(" + $clickX + "," + $clickY + ")")

Click ($s.ox + [int](($box[0] + $box[2]) / 2)) ($s.oy + [int](($box[1] + $box[3]) / 2))
Press $VK_F2
Press $VK_END
Set-Clipboard -Value "X"
Click $clickX $clickY
Ctrl $VK_V
Start-Sleep -Milliseconds 500
Shot $s "v5s-caret-after" | Out-Null

Click ($s.ox + [int]($s.cw * 0.12)) ($s.oy + [int]($s.ch * 0.90))
Ctrl $VK_S
Start-Sleep -Milliseconds 900
End-Session $s
$canvas.Dispose()

$z = [System.IO.Compression.ZipFile]::OpenRead($fx1)
$entry = $z.GetEntry("content.json")
$sr = New-Object System.IO.StreamReader($entry.Open())
$json = $sr.ReadToEnd()
$sr.Close(); $z.Dispose()

$m = [regex]::Match($json, '"text":"(.*?)"')
Write-Host "[caret] claim 1"
if (-not $m.Success) { Write-Host "  FAIL saved content.json has no text field"; $fail++ }
else {
    $segs = $m.Groups[1].Value -split '\\n'
    Write-Host ("       saved lines=" + ($segs -join " | "))
    if ($segs.Count -ne 3) { Write-Host "  FAIL the click or paste broke the line structure"; $fail++ }
    elseif ($segs[0].Contains("X")) { Write-Host ("  OK caret followed the click: " + $segs[0]) }
    elseif ($segs[2].Contains("X")) {
        Write-Host "  FAIL marker landed on the last line: the click did not move the caret"
        $fail++
    } else {
        Write-Host "  FAIL marker is nowhere in the file: the click committed the edit or the paste was eaten"
        $fail++
    }
}

# -- Claim 2: a left drag inside the editing text selects, it does not pan.
$fx2 = New-Fixture "drag"
$s2 = Start-Session "drag" $fx2
Press $VK_HOME

$cv2 = Shot $s2 "v5s-drag-canvas"
$look2 = CardLook $s2 $cv2
if ($null -eq $look2.box) { Write-Error "no node ink found (drag)"; exit 1 }
$box2 = $look2.box; $bg2 = $look2.bg
$cx0b = $box2[0] + $inset; $cy0b = $box2[1] + $inset
$cx1b = $box2[2] + 1 - $inset; $cy1b = $box2[3] + 1 - $inset
$lines2 = [V5S]::Lines($cv2, $cx0b, $cy0b, $cx1b, $cy1b, $bg2[0], $bg2[1], $bg2[2], 90, 3, 25)
if (([int]($lines2.Length / 3)) -lt 3) { Write-Host "  FAIL expected 3 text lines (drag)"; exit 1 }
$span2 = [V5S]::ColSpan($cv2, $cx0b, [int]$lines2[0], $cx1b, [int]($lines2[1] + 1), $bg2[0], $bg2[1], $bg2[2], 90)

Click ($s2.ox + [int](($box2[0] + $box2[2]) / 2)) ($s2.oy + [int](($box2[1] + $box2[3]) / 2))
Press $VK_F2

$editShot = Shot $s2 "v5s-drag-editing"
$editLook = CardLook $s2 $editShot
if ($null -eq $editLook.box) { Write-Host "  FAIL the card lost its ink in the editing state"; exit 1 }
$beforeBox = $editLook.box; $bgE = $editLook.bg
Write-Host ("[drag] editing card=" + ($beforeBox -join ","))

# Same interval, no input: this is the caret-blink floor the drag must clear.
Start-Sleep -Milliseconds 1150
$ctrl = Shot $s2 "v5s-drag-control"
$noise = [V5S]::Changed($editShot, $ctrl, $cx0b, $cy0b, $cx1b, $cy1b, 40)

$dx0 = $s2.ox + [int]($span2[0] + ($span2[1] - $span2[0]) * 0.45)
$dy0 = $s2.oy + [int](($lines2[0] + $lines2[1]) / 2)
$dy1 = $s2.oy + [int](($lines2[6] + $lines2[7]) / 2)
Drag $dx0 $dy0 ($s2.ox + $span2[1] - 2) $dy1

$after = Shot $s2 "v5s-drag-after"
$afterLook = CardLook $s2 $after
Write-Host "[drag] claim 2"
if ($null -eq $afterLook.box) {
    Write-Host "  FAIL the card vanished after the drag"
    $fail++
} else {
    $ab = $afterLook.box
    $mvX = [math]::Abs($ab[0] - $beforeBox[0]); $mvY = [math]::Abs($ab[1] - $beforeBox[1])
    Write-Host ("       card after=" + ($ab -join ",") + " shift=" + $mvX + "x" + $mvY + "px")
    if ($mvX -gt 2 -or $mvY -gt 2) {
        Write-Host ("  FAIL the camera panned (" + $mvX + "x" + $mvY + "px) instead of selecting text")
        $fail++
    } else { Write-Host "  OK card stayed put: no camera pan during a text drag" }
    $chg = [V5S]::Changed($editShot, $after, $cx0b, $cy0b, $cx1b, $cy1b, 40)
    Write-Host ("       changed px inside card=" + $chg + " (blink floor=" + $noise + ")")
    if ($chg -le $noise) { Write-Host "  FAIL the drag selected nothing visible"; $fail++ }
    else { Write-Host "  OK drag changed the text area beyond caret blink" }
}

End-Session $s2
$cv2.Dispose(); $editShot.Dispose(); $ctrl.Dispose(); $after.Dispose()
$editLook | Out-Null

Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
if ($fail -gt 0) { Write-Host ("EDIT_POINTER FAIL (" + $fail + ")"); exit 1 }
Write-Host "EDIT_POINTER OK"
