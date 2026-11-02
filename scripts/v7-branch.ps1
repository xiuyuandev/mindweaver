param(
    [string]$Exe = "target/release/mw-shell.exe",
    [string]$ShotDir = "shots/qa"
)

# v7 per-node branch-colour probe (real input, release build).
#
# Opens a small fixture with MW_OPEN, closes the outline, then selects a level-1
# topic with the arrow keys (RIGHT from nothing selects the root, LEFT moves to
# the nearest child on the left). Left is deliberate: the pixel measurements below
# stop at the inspector edge, and the fixture puts Beta on the right - a right-side
# selection would sit outside the measured band and read as "nothing changed".
# F9 opens the inspector.
#
# The style tab paints three rows of ~13pt colour discs, top to bottom:
#   fill / branch line / text colour. Every disc is a saturated palette colour,
# the two "custom..." buttons are 22pt squares and too narrow to count as a
# band, so the first three wide bands are the colour rows; the scan may also
# catch narrower saturated rows lower in the panel, which are ignored. The
# second band is the new branch-line row. Clicking a disc in it must:
#   1. repaint the canvas (the L1 card is derived from the branch colour),
#   2. persist `"branchColor"` on that node in the autosave container,
#   3. and Ctrl+Z must restore the frame pixel-for-pixel in the canvas area.
#
# Input is hardware (SetCursorPos/mouse_event/keybd_event) like v3-core.ps1.
# If the desktop refuses the foreground or the cursor is frozen, the rig says
# INCONCLUSIVE and exits 3 instead of blaming the app for input it never sent.
#
# ASCII only (PowerShell 5.1 reads BOM-less files as ANSI).

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
Add-Type -ReferencedAssemblies System.Drawing -Language CSharp @'
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Collections.Generic;

public static class V7B {
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref P p);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint idAttach, uint idAttachTo, bool attach);
    [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(P p);
    [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h, uint flags);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out P p);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, int data, IntPtr extra);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint f, IntPtr extra);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct P { public int X, Y; }

    /// Move the cursor 37x21 and back. False when synthetic input is dead.
    public static bool CursorFrozen() {
        P a; if (!GetCursorPos(out a)) return true;
        SetCursorPos(a.X + 37, a.Y + 21);
        P b; GetCursorPos(out b);
        bool moved = (b.X == a.X + 37 && b.Y == a.Y + 21);
        if (moved) SetCursorPos(a.X, a.Y);
        return !moved;
    }

    /// Title of whatever window last covered the measured rect (see Covered).
    public static string Blocker = "";

    /// Sample points of the screen rect that resolve to a *titled* foreign window.
    /// CopyFromScreen photographs whatever is on top, so without this check a dialog
    /// parked over the app would be measured as if the app had painted it.
    public static int[] Covered(IntPtr h, int x0, int y0, int x1, int y1, int step) {
        int n = 0, total = 0;
        Blocker = "";
        for (int y = y0 + step / 2; y < y1; y += step) {
            for (int x = x0 + step / 2; x < x1; x += step) {
                total++;
                P p = new P(); p.X = x; p.Y = y;
                IntPtr w = WindowFromPoint(p);
                IntPtr root = w == IntPtr.Zero ? IntPtr.Zero : GetAncestor(w, 2); // GA_ROOT
                if (root == h) continue;
                IntPtr top = root == IntPtr.Zero ? w : root;
                if (top == IntPtr.Zero) continue;
                string t = Title(top);
                if (t.Length == 0) continue;
                n++;
                if (Blocker.Length == 0) Blocker = t;
            }
        }
        return new int[] { n, total };
    }

    /// True when the screen point belongs to this window (or an untitled helper).
    public static bool At(IntPtr h, int x, int y) {
        P p = new P(); p.X = x; p.Y = y;
        IntPtr w = WindowFromPoint(p);
        if (w == IntPtr.Zero) return false;
        IntPtr root = GetAncestor(w, 2);
        if (root == h) return true;
        IntPtr top = root == IntPtr.Zero ? w : root;
        return Title(top).Length == 0;
    }

    public static string Title(IntPtr h) {
        var sb = new System.Text.StringBuilder(512);
        GetWindowText(h, sb, sb.Capacity);
        return sb.ToString();
    }

    // ---- frame access -----------------------------------------------------------
    public static int[] Grab(Bitmap b) {
        int w = b.Width, h = b.Height;
        int[] a = new int[2 + w * h];
        a[0] = w; a[1] = h;
        BitmapData d = b.LockBits(new Rectangle(0, 0, w, h), ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
        int[] buf = new int[w * h];
        Marshal.Copy(d.Scan0, buf, 0, w * h);
        b.UnlockBits(d);
        for (int i = 0; i < w * h; i++) a[2 + i] = buf[i] & 0xFFFFFF;
        return a;
    }

    public static int Diff(int[] a, int[] b, int x0, int y0, int x1, int y1, int tol) {
        int n = 0;
        int w = Math.Min(a[0], b[0]), h = Math.Min(a[1], b[1]);
        x1 = Math.Min(x1, w); y1 = Math.Min(y1, h);
        for (int y = y0; y < y1; y++) {
            for (int x = x0; x < x1; x++) {
                int pa = a[2 + y * w + x], pb = b[2 + y * w + x];
                int dr = ((pa >> 16) & 0xFF) - ((pb >> 16) & 0xFF);
                int dg = ((pa >> 8) & 0xFF) - ((pb >> 8) & 0xFF);
                int db = (pa & 0xFF) - (pb & 0xFF);
                if (Math.Abs(dr) > tol || Math.Abs(dg) > tol || Math.Abs(db) > tol) n++;
            }
        }
        return n;
    }

    static bool Hot(Color c) {
        int hi = Math.Max(c.R, Math.Max(c.G, c.B));
        int lo = Math.Min(c.R, Math.Min(c.G, c.B));
        return hi >= 80 && hi - lo >= 60;
    }

    /// Wide horizontal bands of saturated pixels: {y0, y1, maxRunCount}.
    public static int[][] Bands(Bitmap b, int x0, int x1, int y0, int y1, int minCount) {
        List<int[]> res = new List<int[]>();
        int start = -1, max = 0;
        for (int y = y0; y < y1; y++) {
            int n = 0;
            for (int x = x0; x < x1; x++) {
                if (Hot(b.GetPixel(x, y))) n++;
            }
            if (n >= minCount) {
                if (start < 0) start = y;
                if (n > max) max = n;
            } else if (start >= 0) {
                res.Add(new int[] { start, y - 1, max });
                start = -1; max = 0;
            }
        }
        if (start >= 0) res.Add(new int[] { start, y1 - 1, max });
        return res.ToArray();
    }

    /// Saturated runs on one scanline: {centerX, r, g, b, width}.
    public static int[][] Runs(Bitmap b, int x0, int x1, int y, int minLen) {
        List<int[]> res = new List<int[]>();
        int start = -1;
        for (int x = x0; x <= x1; x++) {
            bool on = false;
            if (x < x1) on = Hot(b.GetPixel(x, y));
            if (on) {
                if (start < 0) start = x;
            } else if (start >= 0) {
                int len = x - start;
                if (len >= minLen) {
                    int cx = start + len / 2;
                    Color m = b.GetPixel(cx, y);
                    res.Add(new int[] { cx, m.R, m.G, m.B, len });
                }
                start = -1;
            }
        }
        return res.ToArray();
    }
}
'@
[void][V7B]::SetProcessDPIAware()

$root = Split-Path -Parent $PSScriptRoot
if (-not [System.IO.Path]::IsPathRooted($Exe)) { $Exe = Join-Path $root $Exe }
if (-not [System.IO.Path]::IsPathRooted($ShotDir)) { $ShotDir = Join-Path $root $ShotDir }
if (-not (Test-Path -LiteralPath $ShotDir)) { New-Item -ItemType Directory -Path $ShotDir | Out-Null }

$tmp = Join-Path $env:TEMP ("mw-v7-" + [Guid]::NewGuid().ToString("N"))
$docDir = Join-Path $tmp "doc"
New-Item -ItemType Directory -Force -Path $docDir | Out-Null
$fixture = Join-Path $docDir "v7.zmind"

$manifest = '{"formatVersion":1,"app":"v7-probe","docId":"v7-doc","title":"v7-branch","createdAt":1,"modifiedAt":1,"nodeCount":5}'
$content = '{"schema":1,"rootId":"root","nodes":{'
$content += '"root":{"id":"root","parentId":null,"children":["a","b","c"],"text":"Root"},'
$content += '"a":{"id":"a","parentId":"root","children":["a1"],"text":"Alpha"},'
$content += '"a1":{"id":"a1","parentId":"a","children":[],"text":"Alpha One"},'
$content += '"b":{"id":"b","parentId":"root","children":[],"text":"Beta"},'
$content += '"c":{"id":"c","parentId":"root","children":[],"text":"Gamma"}},'
$content += '"theme":"snow","layout":{}}'

$fs = [System.IO.File]::Open($fixture, [System.IO.FileMode]::Create)
$zip = New-Object System.IO.Compression.ZipArchive($fs, [System.IO.Compression.ZipArchiveMode]::Create)
foreach ($pair in @(@("mimetype", "application/vnd.zmind"), @("manifest.json", $manifest), @("content.json", $content))) {
    $entry = $zip.CreateEntry($pair[0], [System.IO.Compression.CompressionLevel]::Optimal)
    $stream = $entry.Open()
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($pair[1])
    $stream.Write($bytes, 0, $bytes.Length)
    $stream.Dispose()
}
$zip.Dispose()
$fs.Dispose()

$env:MW_HOME = "0"
$env:MW_WINDOW_STATE = "off"
$env:MW_STATS = $null
$env:MW_OPEN = $fixture
$env:APPDATA = $tmp

$slot = Join-Path $docDir ".autosave\v7.autosave.zmind"
$proc = Start-Process -FilePath $Exe -PassThru
$hwnd = [IntPtr]::Zero
for ($i = 0; $i -lt 80; $i++) {
    Start-Sleep -Milliseconds 250
    $proc.Refresh()
    $h = $proc.MainWindowHandle
    if ($h -ne [IntPtr]::Zero) {
        $r = New-Object V7B+RECT
        [void][V7B]::GetClientRect($h, [ref]$r)
        if ($r.Right -ge 600 -and $r.Bottom -ge 400) { $hwnd = $h; break }
    }
}

function Finish([int]$code, [string]$why) {
    if ($why) { Write-Host $why }
    if ($proc -and -not $proc.HasExited) {
        [void][V7B]::SetWindowPos($hwnd, [IntPtr](-2), 0, 0, 0, 0, (0x0002 -bor 0x0001))
        Stop-Process -Id $proc.Id -Force
    }
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    exit $code
}

if ($hwnd -eq [IntPtr]::Zero) { Finish 1 "FAIL: main window never appeared" }

[void][V7B]::ShowWindow($hwnd, 9)
[void][V7B]::BringWindowToTop($hwnd)
try { (New-Object -ComObject WScript.Shell).AppActivate($proc.Id) | Out-Null } catch { }

# Windows may refuse SetForegroundWindow to a process launched from an IDE/agent
# (foreground lock). Attaching to the current foreground thread lifts that refusal;
# retry a few times because the previous foreground window can steal focus back.
function Focus-App {
    for ($i = 0; $i -lt 10; $i++) {
        if ([V7B]::GetForegroundWindow() -eq $hwnd) { return $true }
        $fg = [V7B]::GetForegroundWindow()
        $fgPid = 0
        $fgThread = [V7B]::GetWindowThreadProcessId($fg, [ref]$fgPid)
        [void][V7B]::AttachThreadInput([V7B]::GetCurrentThreadId(), $fgThread, $true)
        [void][V7B]::SetForegroundWindow($hwnd)
        [void][V7B]::AttachThreadInput([V7B]::GetCurrentThreadId(), $fgThread, $false)
        Start-Sleep -Milliseconds 250
    }
    return ([V7B]::GetForegroundWindow() -eq $hwnd)
}

[void](Focus-App)
Start-Sleep -Milliseconds 400
if ([V7B]::GetForegroundWindow() -ne $hwnd) {
    Finish 3 "INCONCLUSIVE: app never became foreground; hardware input cannot be aimed"
}
if ([V7B]::CursorFrozen()) {
    Finish 3 "INCONCLUSIVE: cursor is frozen (input lock); cannot click a swatch"
}

$rect = New-Object V7B+RECT
[void][V7B]::GetClientRect($hwnd, [ref]$rect)
$origin = New-Object V7B+P
[void][V7B]::ClientToScreen($hwnd, [ref]$origin)
$cw = $rect.Right
$ch = $rect.Bottom
$scale = [V7B]::GetDpiForWindow($hwnd) / 96.0
Write-Host ("client=" + $cw + "x" + $ch + " scale=" + $scale)
Write-Host ("window title='" + [V7B]::Title($hwnd) + "'")

# Frames captured through a foreign window are not evidence. Count them, and make the
# whole run INCONCLUSIVE even if the numbers below happen to look like a pass.
# Raise to topmost while measuring: a user's always-on-top browser can otherwise sit
# above the app even when the app owns the foreground. Finish restores NOTOPMOST.
$blind = 0
function Raise() {
    [void][V7B]::SetWindowPos($hwnd, [IntPtr](-1), 0, 0, 0, 0, (0x0001 -bor 0x0002 -bor 0x0010))
    Start-Sleep -Milliseconds 180
}
function CoveredPts() {
    $c = [V7B]::Covered($hwnd, $origin.X, $origin.Y, ($origin.X + $cw), ($origin.Y + $ch), 90)
    return @([int]$c[0], [int]$c[1], [V7B]::Blocker)
}
function Shot([string]$name) {
    Raise
    $c = CoveredPts
    if ($c[0] -gt 0) {
        Start-Sleep -Milliseconds 700
        Raise
        $c = CoveredPts
    }
    if ($c[0] -gt 0) {
        Write-Host ("  note " + $name + ": " + $c[0] + "/" + $c[1] + " sample points are covered by '" + $c[2] + "'")
        $script:blind++
    }
    $bmp = New-Object System.Drawing.Bitmap $cw, $ch, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($origin.X, $origin.Y, 0, 0, (New-Object System.Drawing.Size $cw, $ch))
    $full = (Resolve-Path -LiteralPath $ShotDir).Path + "\" + $name + ".png"
    if (Test-Path -LiteralPath $full) { Remove-Item -LiteralPath $full -Force }
    $bmp.Save($full, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose()
    Write-Host ("  shot -> " + $full)
    return $bmp
}
function Press([byte]$vk) {
    [V7B]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [V7B]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 450
}
function Chord([byte]$mod, [byte]$vk) {
    [V7B]::keybd_event($mod, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [V7B]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [V7B]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 40
    [V7B]::keybd_event($mod, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 600
}
function Click([int]$x, [int]$y) {
    [void][V7B]::SetCursorPos($x, $y)
    Start-Sleep -Milliseconds 140
    [V7B]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 50
    [V7B]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 400
}

$VK_F8 = 0x77; $VK_F9 = 0x78; $VK_LEFT = 0x25; $VK_RIGHT = 0x27
$VK_CONTROL = 0x11; $VK_Z = 0x5A

# Close the outline so the canvas owns the whole width, then focus it by clicking
# empty space - that also clears the selection, so the first RIGHT selects the root.
Press $VK_F8
Start-Sleep -Milliseconds 800
Click ($origin.X + [int](90 * $scale)) ($origin.Y + [int](150 * $scale))
$empty = Shot "v7-empty"

Press $VK_RIGHT
$rootShot = Shot "v7-root"
Press $VK_LEFT
$l1Shot = Shot "v7-l1"

$canvasX1 = $cw - [int](262 * $scale)
$canvasY0 = [int](70 * $scale)
$canvasY1 = [int]($ch * 0.90)
$selectDiff = [V7B]::Diff([V7B]::Grab($empty), [V7B]::Grab($l1Shot), 0, $canvasY0, $canvasX1, $canvasY1, 6)
Write-Host ("selection_diff=" + $selectDiff)

# Open the inspector. The map area must stay put; the panel is measured afterwards.
Press $VK_F9
Start-Sleep -Milliseconds 1200
$before = Shot "v7-before"

$panelX0 = $cw - [int](258 * $scale)
$panelX1 = $cw - 4
$scanY0 = [int](100 * $scale)
$scanY1 = [Math]::Min($ch - [int](40 * $scale), [int](640 * $scale))
$minCount = [int](60 * $scale)
$bands = [V7B]::Bands($before, $panelX0, $panelX1, $scanY0, $scanY1, $minCount)
Write-Host ("bands=" + $bands.Length)
for ($i = 0; $i -lt $bands.Length; $i++) {
    Write-Host ("  band " + $i + ": y=" + $bands[$i][0] + ".." + $bands[$i][1] + " max=" + $bands[$i][2])
}
if ($bands.Length -lt 2) {
    $bmp = Shot "v7-no-bands"
    $bmp.Dispose()
    Finish 1 "FAIL: inspector did not paint at least two colour rows"
}

# Band 1 is the branch-line row (band 0 is fill, band 2 is text colour).
$bandY = [int](($bands[1][0] + $bands[1][1]) / 2)
$minRun = [int](6 * $scale)
$runs = [V7B]::Runs($before, $panelX0, $panelX1, $bandY, $minRun)
Write-Host ("runs at y=" + $bandY + ": " + $runs.Length)
for ($i = 0; $i -lt $runs.Length; $i++) {
    Write-Host ("  run " + $i + ": x=" + $runs[$i][0] + " rgb=" + $runs[$i][1] + "," + $runs[$i][2] + "," + $runs[$i][3] + " w=" + $runs[$i][4])
}
if ($runs.Length -lt 2) {
    $bmp = Shot "v7-no-runs"
    $bmp.Dispose()
    Finish 1 "FAIL: second band had fewer than two clickable colour discs"
}

# Swatch 1 is red (#e4635b). Both possible left children resolve to other branch
# colours (Alpha green, Gamma orange), so this choice always changes something;
# swatch 0 is green and would be a no-op if LEFT landed on Alpha.
$pick = $runs[1]
$hex = "#" + $pick[1].ToString("x2") + $pick[2].ToString("x2") + $pick[3].ToString("x2")
Write-Host ("clicking branch swatch " + $hex + " at " + $pick[0] + "," + $bandY)

if ([V7B]::GetForegroundWindow() -ne $hwnd) {
    Finish 3 "INCONCLUSIVE: app lost the foreground before the swatch click"
}
if (-not [V7B]::At($hwnd, $origin.X + $pick[0], $origin.Y + $bandY)) {
    Finish 3 "INCONCLUSIVE: the swatch point is covered by another window"
}
Click ($origin.X + $pick[0]) ($origin.Y + $bandY)
Start-Sleep -Milliseconds 500
$after = Shot "v7-after"

$beforePx = [V7B]::Grab($before)
$afterPx = [V7B]::Grab($after)
$repaint = [V7B]::Diff($beforePx, $afterPx, 0, $canvasY0, $canvasX1, $canvasY1, 6)
Write-Host ("canvas_repaint=" + $repaint)

# Autosave writes through the same mw_format::save every 3s; poll for the new field.
$json = ""
$deadline = (Get-Date).AddSeconds(15)
do {
    Start-Sleep -Milliseconds 400
    if (Test-Path -LiteralPath $slot) {
        try {
            $archive = [System.IO.Compression.ZipFile]::OpenRead($slot)
            $entry = $archive.GetEntry("content.json")
            if ($entry) {
                $reader = New-Object System.IO.StreamReader($entry.Open(), [System.Text.Encoding]::UTF8)
                $json = $reader.ReadToEnd()
                $reader.Dispose()
            }
            $archive.Dispose()
        } catch { $json = "" }
        if ($json -match '"branchColor"') { break }
    }
} while ((Get-Date) -lt $deadline)

$saved = $json -match ('"branchColor":"' + [regex]::Escape($hex) + '"')
Write-Host ("autosave has branchColor " + $hex + ": " + $saved)
$owner = ""
if ($saved) {
    try {
        $parsed = $json | ConvertFrom-Json
        foreach ($prop in $parsed.nodes.PSObject.Properties) {
            if ($prop.Value.style.branchColor -eq $hex) { $owner = $prop.Name; break }
        }
    } catch { $owner = "" }
}
$ownerOk = ($owner -eq "a" -or $owner -eq "c")
Write-Host ("branchColor owner=" + $owner + " leftChild=" + $ownerOk)

# Undo must land back on the exact frame we measured before the swatch click.
if ([V7B]::GetForegroundWindow() -ne $hwnd) { [void][V7B]::SetForegroundWindow($hwnd); Start-Sleep -Milliseconds 400 }
Chord $VK_CONTROL $VK_Z
$undo = Shot "v7-undo"
$undoPx = [V7B]::Grab($undo)
$undoDiff = [V7B]::Diff($beforePx, $undoPx, 0, $canvasY0, $canvasX1, $canvasY1, 6)
Write-Host ("undo_vs_before=" + $undoDiff)

$empty.Dispose(); $rootShot.Dispose(); $l1Shot.Dispose(); $before.Dispose(); $after.Dispose(); $undo.Dispose()

$ok = $true
if ($selectDiff -le 300) { Write-Host "FAIL: arrow keys did not move the selection"; $ok = $false }
if ($repaint -le 500) { Write-Host "FAIL: swatch click did not repaint the canvas"; $ok = $false }
if (-not $saved) { Write-Host "FAIL: autosave did not persist branchColor"; $ok = $false }
if ($saved -and -not $ownerOk) { Write-Host ("FAIL: branchColor landed on '" + $owner + "', not a left child"); $ok = $false }
if ($undoDiff -gt 80) { Write-Host ("FAIL: undo left " + $undoDiff + " canvas pixels changed"); $ok = $false }

if ($ok -and $blind -gt 0) {
    Finish 3 ("INCONCLUSIVE: " + $blind + " frame(s) were captured through another window; neither PASS nor FAIL is trustworthy")
}
if ($ok) { Finish 0 "PASS: branch colour repaints, persists, and undoes" }
Finish 1 "FAIL: see measurements above"
