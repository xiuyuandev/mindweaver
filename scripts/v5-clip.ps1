param(
    [string]$Exe = "target/release/mw-shell.exe",
    [string]$ShotDir = "shots/qa"
)

# v5 window-level clipboard chords probe (real input, release build).
#
# Copy, cut, paste and their Shift variants used to live inside the canvas shortcut block,
# which only runs while the canvas widget holds the keyboard. That makes the chords die for
# the reason the undo chord died in v5-d: the keyboard moved somewhere else - F6 hands it to
# the panels, a click on a toolbar button hands it to that button - and the document stops
# answering Ctrl+C/X/V without any message. The fix gives these chords the same single owner
# the undo chord got (the window), with one addition: a text field that owns the keyboard
# keeps its own Ctrl+Z, so the gate is evaluated *before* anything is consumed.
#
# Six claims, measured from what the app actually did:
#   1. With the keyboard handed to the panels (the focus trail says it is not on the canvas),
#      Ctrl+C still copies - proven by the system clipboard, which the app only writes when a
#      real subtree was copied. That also proves plain text leaves the app.
#   2. Ctrl+V in that same state still edits the document: a new revision reaches the
#      container and the document band repaints.
#   3. Ctrl+X removes exactly as many nodes as it copied, and the following Ctrl+V puts the
#      same number back. Cut is copy+delete, so it has to win over plain copy in one frame;
#      and "exactly N" is what proves one chord ran one execution (a double dispatch doubles
#      it). Claims 2 and 3 split the work: 2 shows the chord lands, 3 does the arithmetic.
#   4. Ctrl+Shift+C then Ctrl+Shift+V (the style chords) change the node count by zero. The
#      plain copy/paste chord match tolerates an extra Shift, so checking the Shift variants
#      first is the only defence - this is where a fall-through would show up.
#   5. With the search field focused, Ctrl+Z leaves the document band byte-identical - and
#      with the bar closed the very same chord does repaint it. Both halves are needed: the
#      first alone would also pass if the chord had been swallowed on its way in.
#   6. Control: two frames with no keystroke between them are identical over the body, so
#      "byte-identical" above is a claim about the document and not about a moving camera.
#
# Node counts come from the recovery point the app writes every 3s (same container writer as
# Ctrl+S), read from manifest.json's nodeCount. That is why the probe never presses Ctrl+S:
# a modifier chord is not guaranteed to reach the app here (see the preflight), while the
# interval writer needs nothing. A count of -1 means no new revision was flushed in the
# window, which is itself evidence - claim 3's cut/paste pair is the positive control that
# shows the writer does answer when something really changed.
#
# Selection is established by clicking a card, because a click is the only way to pick a node
# without typing: SendMessage carries no WM_CHAR, so nothing can ever be typed from a process
# that does not own the foreground. A click hands the keyboard *back* to the canvas - which
# is precisely the state the old code needed - so the F6 handover has to come after it, or
# every claim below would be measuring the behaviour that was already working.
#
# Every frame is grabbed with CopyFromScreen, which photographs whatever is on top, so Shot
# raises our own window (HWND_TOP, no activation) and asks WindowFromPoint whether the client
# rect is really visible. A frame captured through someone else's dialog is counted, and the
# run ends INCONCLUSIVE (exit 3) instead of blaming the app for a window it could not see.
#
# ASCII only (PowerShell 5.1 reads BOM-less files as ANSI); CJK strings are built from codes.

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.IO.Compression.FileSystem
Add-Type -ReferencedAssemblies System.Drawing -Language CSharp @'
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

public static class V5C {
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref P p);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out P p);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, int data, IntPtr extra);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint f, IntPtr extra);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(P p);
    [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h, uint flags);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool join);
    [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int n);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct P { public int X, Y; }

    public const uint WM_KEYDOWN = 0x0100, WM_KEYUP = 0x0101;
    public const uint KEYEVENTF_KEYUP = 0x0002;

    public static string Blocker = "";

    /// Attach our input queue to the window's thread, take the foreground, detach. Without
    /// the attach, SetForegroundWindow is refused for a background process and half of these
    /// claims cannot be reached at all.
    public static bool TakeForeground(IntPtr h) {
        uint pid;
        IntPtr fg = GetForegroundWindow();
        uint fgTid = GetWindowThreadProcessId(fg, out pid);
        uint myTid = GetCurrentThreadId();
        uint appTid = GetWindowThreadProcessId(h, out pid);
        bool joined = false;
        if (fgTid != 0 && fgTid != myTid) joined = AttachThreadInput(myTid, fgTid, true);
        BringWindowToTop(h);
        SetForegroundWindow(h);
        bool ok = GetForegroundWindow() == h;
        // The app's own thread queue is what winit reads the modifier state from: a sent
        // WM_KEYDOWN for VK_CONTROL never reaches that async state, so the letter arrives
        // unmodified. Sharing the queue is what makes a real chord possible from here.
        if (!ok && appTid != 0 && appTid != myTid) {
            AttachThreadInput(myTid, appTid, true);
            SetForegroundWindow(h);
            ok = GetForegroundWindow() == h;
            AttachThreadInput(myTid, appTid, false);
        }
        if (joined) AttachThreadInput(myTid, fgTid, false);
        return ok;
    }

    /// How many sample points of this rect resolve to a *titled* window other than `h`.
    /// Untitled roots are the app's own helper windows (IME, tooltip, caret) - one of them
    /// over one sample point is not a blind camera, it is a border pixel.
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

    /// Keys go through SendMessage, not keybd_event: hardware injection is delivered to
    /// whatever the OS calls the foreground window, and that is not ours.
    public static void Key(IntPtr h, int vk, bool up) {
        uint l = 0x00000001u | (1u << 16);
        if (up) l |= 0xC0000000u;
        SendMessage(h, up ? WM_KEYUP : WM_KEYDOWN, (IntPtr)vk, (IntPtr)l);
    }

    public static string Title(IntPtr h) {
        var sb = new System.Text.StringBuilder(512);
        GetWindowText(h, sb, sb.Capacity);
        return sb.ToString();
    }

    /// True when synthetic mouse input is dead. Moves the cursor 37x21 and back.
    public static bool CursorFrozen() {
        P a; if (!GetCursorPos(out a)) return true;
        SetCursorPos(a.X + 37, a.Y + 21);
        P b; GetCursorPos(out b);
        bool moved = (b.X == a.X + 37 && b.Y == a.Y + 21);
        if (moved) SetCursorPos(a.X, a.Y);
        return !moved;
    }

    // ---- pixel access: LockBits once per frame ----------------------------------------
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
    static int At(int[] a, int x, int y) { return a[2 + y * a[0] + x]; }
    public static int R(int p) { return (p >> 16) & 0xFF; }
    public static int G(int p) { return (p >> 8) & 0xFF; }
    public static int B(int p) { return p & 0xFF; }
    static int Sat(int p) {
        int hi = R(p) > G(p) ? R(p) : G(p); if (B(p) > hi) hi = B(p);
        int lo = R(p) < G(p) ? R(p) : G(p); if (B(p) < lo) lo = B(p);
        return hi - lo;
    }
    static int Dist(int[] a, int x, int y, int r, int g, int b) {
        int p = At(a, x, y);
        return Math.Abs(R(p) - r) + Math.Abs(G(p) - g) + Math.Abs(B(p) - b);
    }

    public static int Diff(int[] a, int[] b, int x0, int y0, int x1, int y1, int tol) {
        int n = 0;
        int w = Math.Min(a[0], b[0]), h = Math.Min(a[1], b[1]);
        x1 = Math.Min(x1, w); y1 = Math.Min(y1, h);
        for (int y = y0; y < y1; y++) {
            for (int x = x0; x < x1; x++) {
                int pa = At(a, x, y), pb = At(b, x, y);
                if (Math.Abs(R(pa) - R(pb)) > tol || Math.Abs(G(pa) - G(pb)) > tol || Math.Abs(B(pa) - B(pb)) > tol) n++;
            }
        }
        return n;
    }

    /// Where a non-zero diff actually lives - "6174 pixels differ" is not actionable.
    public static int[] DiffBox(int[] a, int[] b, int x0, int y0, int x1, int y1, int tol) {
        int minx = 1 << 20, miny = 1 << 20, maxx = -1, maxy = -1, n = 0;
        int w = Math.Min(a[0], b[0]), h = Math.Min(a[1], b[1]);
        x1 = Math.Min(x1, w); y1 = Math.Min(y1, h);
        for (int y = y0; y < y1; y++) {
            for (int x = x0; x < x1; x++) {
                int pa = At(a, x, y), pb = At(b, x, y);
                if (Math.Abs(R(pa) - R(pb)) <= tol && Math.Abs(G(pa) - G(pb)) <= tol && Math.Abs(B(pa) - B(pb)) <= tol) continue;
                n++;
                if (x < minx) minx = x; if (x > maxx) maxx = x;
                if (y < miny) miny = y; if (y > maxy) maxy = y;
            }
        }
        return new int[] { minx, miny, maxx, maxy, n };
    }

    public static int[] Modal(int[] a, int x0, int y0, int x1, int y1, int minSat, int step) {
        var hist = new System.Collections.Generic.Dictionary<int, int>();
        for (int y = y0; y < y1; y += step) {
            for (int x = x0; x < x1; x += step) {
                int p = At(a, x, y);
                if (minSat > 0 && Sat(p) < minSat) continue;
                int n; hist.TryGetValue(p, out n);
                hist[p] = n + 1;
            }
        }
        int bestKey = 0, best = -1;
        foreach (var kv in hist) if (kv.Value > best) { best = kv.Value; bestKey = kv.Key; }
        return new int[] { R(bestKey), G(bestKey), B(bestKey), best };
    }

    /// Bounding box of pixels far from the background - where the document lives on screen.
    /// Used only to aim a click; every claim is measured against the whole band.
    public static int[] InkBox(int[] a, int x0, int y0, int x1, int y1, int br, int bg, int bb, int thr) {
        int minx = 1 << 20, miny = 1 << 20, maxx = -1, maxy = -1, n = 0;
        for (int y = y0; y < y1; y++) {
            for (int x = x0; x < x1; x++) {
                if (Dist(a, x, y, br, bg, bb) <= thr) continue;
                n++;
                if (x < minx) minx = x; if (x > maxx) maxx = x;
                if (y < miny) miny = y; if (y > maxy) maxy = y;
            }
        }
        return new int[] { minx, miny, maxx, maxy, n };
    }
}
'@
[void][V5C]::SetProcessDPIAware()

function Cj([int[]]$codes) {
    $s = ""
    foreach ($c in $codes) { $s += [string]([char]$c) }
    return $s
}
# root + three first-level cards. Every text is non-empty, so the plain text the app pushes
# out has exactly as many lines as the copied subtree has nodes - that is the arithmetic the
# cut/paste claims run on, with no geometry assumption at all.
$rootT = Cj @(0x7EC7, 0x601D)
$kidA = Cj @(0x914D, 0x8272)
$kidB = Cj @(0x4EA4, 0x4E92)
$kidC = Cj @(0x5B57, 0x4F53)

function Node([string]$id, [string]$parent, [string]$text, [string]$kids) {
    return '"' + $id + '":{"id":"' + $id + '","parentId":' + $parent + ',"children":[' + $kids + '],"text":"' + $text + '","collapsed":false,"modifiedAt":0}'
}

$tmp = Join-Path $env:TEMP ("mw-v5e-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
$env:APPDATA = $tmp
if (-not (Test-Path -LiteralPath $ShotDir)) { New-Item -ItemType Directory -Path $ShotDir | Out-Null }
$shotFull = (Resolve-Path -LiteralPath $ShotDir).Path
$encNoU8 = New-Object System.Text.UTF8Encoding $false

$nodes = @(
    (Node "n0" "null" $rootT '"n1","n2","n3"'),
    (Node "n1" '"n0"' $kidA ""),
    (Node "n2" '"n0"' $kidB ""),
    (Node "n3" '"n0"' $kidC "")
)
$content = '{"schema":1,"rootId":"n0","nodes":{' + ($nodes -join ",") + '},"theme":"snow","layout":{"kind":"mindmap","siblingGap":24,"levelGap":80,"maxNodeWidth":300}}'
$manifest = '{"formatVersion":1,"app":"probe/1.0","docId":"probe-v5e","title":"probe","createdAt":0,"modifiedAt":0,"nodeCount":4}'
$dir = Join-Path $tmp "doc"
[void](New-Item -ItemType Directory -Force -Path $dir)
[System.IO.File]::WriteAllText((Join-Path $dir "content.json"), $content, $encNoU8)
[System.IO.File]::WriteAllText((Join-Path $dir "manifest.json"), $manifest, $encNoU8)
[System.IO.File]::WriteAllText((Join-Path $dir "mimetype"), "application/vnd.mindweaver.zmind", $encNoU8)
$fx = Join-Path $tmp "v5e.zmind"
# The recovery point. `autosave::slot_for` hangs it off the *document's own directory*.
$slot = Join-Path (Join-Path $tmp ".autosave") "v5e.autosave.zmind"
[System.IO.Compression.ZipFile]::CreateFromDirectory($dir, $fx)

$env:MW_HOME = "0"
$env:MW_WINDOW_STATE = "off"
$env:MW_OPEN = $fx
# Where the keyboard sits, read from the app instead of guessed from pixels.
$trace = Join-Path $tmp "focus.log"
$env:MW_TRACE = $trace

$proc = Start-Process -FilePath $Exe -PassThru
$hwnd = [IntPtr]::Zero
for ($i = 0; $i -lt 60) {
    Start-Sleep -Milliseconds 250
    $proc.Refresh()
    $h = $proc.MainWindowHandle
    if ($h -ne [IntPtr]::Zero) {
        $r = New-Object V5C+RECT
        [void][V5C]::GetClientRect($h, [ref]$r)
        if ($r.Right -ge 600 -and $r.Bottom -ge 400) { $hwnd = $h; break }
    }
}
if ($hwnd -eq [IntPtr]::Zero) { Write-Error "main window never appeared"; exit 1 }
[void][V5C]::ShowWindow($hwnd, 9)
[void][V5C]::BringWindowToTop($hwnd)
try { (New-Object -ComObject WScript.Shell).AppActivate($proc.Id) | Out-Null } catch { }
[void][V5C]::SetForegroundWindow($hwnd)
Start-Sleep -Milliseconds 1200

$rect = New-Object V5C+RECT
[void][V5C]::GetClientRect($hwnd, [ref]$rect)
$origin = New-Object V5C+P
[void][V5C]::ClientToScreen($hwnd, [ref]$origin)
$cw = $rect.Right
$ch = $rect.Bottom
$scale = [V5C]::GetDpiForWindow($hwnd) / 96.0
Write-Host ("client=" + $cw + "x" + $ch + " scale=" + $scale + " origin=" + $origin.X + "," + $origin.Y)

# --- geometry ---------------------------------------------------------------------------
$yBar = [int](58 * $scale)
$STATUS_Y1 = $ch - [int](40 * $scale)
# The document band: clear of the title bar, the toolbar pill, the minimap, the status strip
# and both panels. The search bar opens above by0, so this band never contains it - which is
# what makes "byte-identical" a claim about the document rather than about a floating overlay.
$bx0 = [int]($cw * 0.13); $bx1 = [int]($cw * 0.70)
$by0 = [int]($ch * 0.24); $by1 = [int]($ch * 0.74)

$fail = 0
$blocked = 0
$blind = 0
$VK_F6 = 0x75
$VK_TAB = 0x09; $VK_ESCAPE = 0x1B
$VK_CONTROL = 0x11; $VK_SHIFT = 0x10
$VK_C = 0x43; $VK_V = 0x56; $VK_X = 0x58; $VK_Z = 0x5A; $VK_F = 0x46; $VK_K = 0x4B

# --- camera -----------------------------------------------------------------------------
function Raise() {
    [void][V5C]::SetWindowPos($hwnd, [IntPtr]::Zero, 0, 0, 0, 0, (0x0001 -bor 0x0002 -bor 0x0010))
    Start-Sleep -Milliseconds 180
}
function CoveredPts() {
    $c = [V5C]::Covered($hwnd, $origin.X, $origin.Y, ($origin.X + $cw), ($origin.Y + $ch), 90)
    return @([int]$c[0], [int]$c[1], [V5C]::Blocker)
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
    $g.Dispose()
    $p = $shotFull + "\" + $name + ".png"
    if (Test-Path -LiteralPath $p) { Remove-Item -LiteralPath $p -Force }
    $bmp.Save($p, [System.Drawing.Imaging.ImageFormat]::Png)
    Write-Host ("  shot -> " + $name)
    return $bmp
}
function ShotPx([string]$name) {
    $bmp = Shot $name
    $px = [V5C]::Grab($bmp)
    $bmp.Dispose()
    return ,$px
}

# --- keys -------------------------------------------------------------------------------
# Two delivery routes, and the difference is the modifier state:
#   sent     - SendMessage WM_KEYDOWN/UP. Always arrives at this window, but it never touches
#              the async key state winit reads the modifiers from, so Ctrl+K arrives as "K".
#              Bare keys (F6, F8, Escape, Tab) work and are the ones v5-a ran on.
#   hardware - keybd_event. Carries the modifier, but goes to whatever the OS currently calls
#              the foreground window - so every call is guarded by Ensure-Fg. An unguarded
#              hardware chord would be typed into someone else's window, which is a lot worse
#              than a blocked claim.
$SETTLE = 620
$script:hwMiss = 0
function Press([int]$vk) {
    [V5C]::Key($hwnd, $vk, $false)
    Start-Sleep -Milliseconds 60
    [V5C]::Key($hwnd, $vk, $true)
    Start-Sleep -Milliseconds $SETTLE
}
function ChordSent([int[]]$mods, [int]$vk) {
    foreach ($m in $mods) { [V5C]::Key($hwnd, $m, $false); Start-Sleep -Milliseconds 45 }
    [V5C]::Key($hwnd, $vk, $false)
    Start-Sleep -Milliseconds 55
    [V5C]::Key($hwnd, $vk, $true)
    foreach ($m in $mods) { [V5C]::Key($hwnd, $m, $true); Start-Sleep -Milliseconds 45 }
    Start-Sleep -Milliseconds $SETTLE
}
function ChordHw([int[]]$mods, [int]$vk) {
    foreach ($m in $mods) { [V5C]::keybd_event([byte]$m, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 45 }
    [V5C]::keybd_event([byte]$vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 55
    [V5C]::keybd_event([byte]$vk, 0, [V5C]::KEYEVENTF_KEYUP, [IntPtr]::Zero)
    foreach ($m in $mods) { [V5C]::keybd_event([byte]$m, 0, [V5C]::KEYEVENTF_KEYUP, [IntPtr]::Zero); Start-Sleep -Milliseconds 45 }
    Start-Sleep -Milliseconds $SETTLE
}
# Is our window the one that would receive a hardware keystroke? Take it if not - the
# documented way round the foreground lock is to share input queues with the current owner.
function Ensure-Fg() {
    if (Is-Foreground) { return $true }
    [void][V5C]::TakeForeground($hwnd)
    Start-Sleep -Milliseconds 200
    return (Is-Foreground)
}
# $hw is set by the preflight, once, from pixels. After that every chord follows the route
# that was proven to arrive - and falls back to the harmless one when the foreground moved.
function Chord([int[]]$mods, [int]$vk) {
    if ($hw) {
        if (Ensure-Fg) { ChordHw $mods $vk; return }
        Write-Host "  note: the foreground moved away; this chord went through the sent route and carries no modifier"
        $script:hwMiss++
    }
    ChordSent $mods $vk
}
function Ctrl([int]$vk) { Chord @($VK_CONTROL) $vk }
function CtrlShift([int]$vk) { Chord @($VK_CONTROL, $VK_SHIFT) $vk }

# --- focus trail ------------------------------------------------------------------------
function TraceText() {
    if (-not (Test-Path -LiteralPath $trace)) { return "" }
    try {
        # FileShare ReadWrite: the app holds this file open for writing.
        $fs = [System.IO.File]::Open($trace, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite)
        $sr = New-Object System.IO.StreamReader($fs, [System.Text.Encoding]::UTF8)
        $text = $sr.ReadToEnd()
        $sr.Dispose(); $fs.Dispose()
        return $text
    } catch { return "" }
}
function TraceLines() {
    $t = TraceText
    if ($t -eq "") { return @() }
    return @($t -split "`r?`n" | Where-Object { $_ -ne "" })
}
function TraceTail() {
    $lines = TraceLines
    if ($lines.Count -eq 0) { return "" }
    return $lines[$lines.Count - 1]
}
function TraceLast([string]$prefix) {
    $found = ""
    foreach ($l in (TraceLines)) { if ($l.StartsWith($prefix)) { $found = $l } }
    return $found
}
function FocusName() {
    $l = TraceLast "focus "
    if ($l.StartsWith("focus ")) { return $l.Substring(6) }
    return ""
}

# --- the recovery point on disk ----------------------------------------------------------
function ReadCount([string]$path) {
    if (-not (Test-Path -LiteralPath $path)) { return -1 }
    try {
        $zip = [System.IO.Compression.ZipFile]::OpenRead($path)
        $n = -1
        $e = $zip.GetEntry("manifest.json")
        if ($null -ne $e) {
            $r = New-Object System.IO.StreamReader($e.Open(), [System.Text.Encoding]::UTF8)
            $t = $r.ReadToEnd(); $r.Dispose()
            if ($t -match '"nodeCount"\s*:\s*(\d+)') { $n = [int]$matches[1] }
        }
        $zip.Dispose()
        return $n
    } catch { return -1 }
}
# The node count of the slot written after `$since`, or -1 when nothing was written inside
# the window. The interval writer only flushes a new revision, so -1 means "unchanged".
function WaitCount([DateTime]$since) {
    for ($try = 0; $try -lt 25; $try++) {
        if ((Test-Path -LiteralPath $slot) -and
            ([System.IO.File]::GetLastWriteTime($slot) -gt $since)) {
            $n = ReadCount $slot
            if ($n -ge 0) { return $n }
        }
        Start-Sleep -Milliseconds 400
    }
    return -1
}

# --- system clipboard --------------------------------------------------------------------
$sentinel = "MWCLIP-SENTINEL"
$clipOk = $false
function ClipSet([string]$s) {
    for ($i = 0; $i -lt 6; $i++) {
        try { [System.Windows.Forms.Clipboard]::SetText($s); return $true }
        catch { Start-Sleep -Milliseconds 150 }
    }
    return $false
}
function ClipGet() {
    try {
        if ([System.Windows.Forms.Clipboard]::ContainsText()) { return [System.Windows.Forms.Clipboard]::GetText() }
    } catch { }
    return $null
}
function Lines([string]$t) {
    if ($null -eq $t) { return -1 }
    return @($t -split '\r?\n' | Where-Object { $_.Trim() -ne "" }).Count
}
# WinForms refuses to touch the clipboard from an MTA thread, and a foreign owner only needs
# a moment to let go - so this is tried, named, and then decides which claims are measurable.
$apt = [System.Threading.Thread]::CurrentThread.GetApartmentState()
Write-Host ("preflight: clipboard apartment=" + $apt + " (STA is required to read and write it)")
$clipOk = ClipSet $sentinel
if ($clipOk) { $clipOk = ((ClipGet) -eq $sentinel) }
Write-Host ("preflight: system clipboard " + $(if ($clipOk) { "readable and writable" } else { "NOT usable from this process" }))
# Press a copy/cut chord and read back what the app pushed out. -1 = the sentinel is still
# there, so nothing was copied; -2 = the clipboard itself became unreadable (rig, not app).
# The app only writes the system clipboard when its own clipboard ended up non-empty, so
# "the sentinel is gone" doubles as proof that a real subtree was selected.
function ChordCopy([int[]]$mods, [int]$vk) {
    if (-not $clipOk) { return -2 }
    [void](ClipSet $sentinel)
    Chord $mods $vk
    $t = ClipGet
    if ($null -eq $t) { $script:clipOk = $false; return -2 }
    if ($t -eq $sentinel) { return -1 }
    return (Lines $t)
}

# --- mouse, if this desktop allows it ----------------------------------------------------
function ClickPt([single]$lx, [single]$ly) {
    if (-not $canInject) { return }
    $x = $origin.X + [int]($lx * $scale)
    $y = $origin.Y + [int]($ly * $scale)
    [void][V5C]::SetCursorPos($x, $y)
    Start-Sleep -Milliseconds 140
    [V5C]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [V5C]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 420
}
function MovePt([single]$lx, [single]$ly) {
    if (-not $canInject) { return }
    [void][V5C]::SetCursorPos($origin.X + [int]($lx * $scale), $origin.Y + [int]($ly * $scale))
    Start-Sleep -Milliseconds 260
}
# Park on bare canvas, below the measured band: a cursor left on a card keeps a hover ring
# painted on it, and egui re-tests hover every frame.
$PARK_LX = [single](($cw * 0.30) / $scale)
$PARK_LY = [single](($ch * 0.92) / $scale)
function Park() { MovePt $PARK_LX $PARK_LY }

# --- settled frame -----------------------------------------------------------------------
# The view interpolates after a structural change, so a frame taken too early differs from
# the same view a moment later. Settle first, then measure: claim 5 says "byte-identical",
# and that only means something for a document that has stopped moving.
function SettlePx([string]$name) {
    $a = ShotPx ($name + "-set-a")
    $d = -1
    for ($t = 0; $t -lt 10; $t++) {
        Start-Sleep -Milliseconds 420
        $b = ShotPx ($name + "-set-b")
        $d = [V5C]::Diff($a, $b, $bx0, $by0, $bx1, $by1, 0)
        if ($d -eq 0) { return ,$b }
        $a = $b
    }
    Write-Host ("  note " + $name + ": band still drifting after 10 tries (" + $d + " px)")
    return ,$a
}

# --- preflight ---------------------------------------------------------------------------
function Is-Foreground() { return ([V5C]::GetForegroundWindow()) -eq $hwnd }
$canInject = -not [V5C]::CursorFrozen()
Write-Host ("preflight: injected mouse " + $(if ($canInject) { "usable" } else { "BLOCKED - no card can be clicked, so no node can be selected" }))
Write-Host ("preflight: foreground = '" + [V5C]::Title([V5C]::GetForegroundWindow()) + "'; ours is " + $(if (Is-Foreground) { "the foreground window" } else { "NOT the foreground window" }))
if (-not $clipOk) { Write-Host "preflight: the system clipboard is not reachable - the copy claims cannot be measured" }

# Do modifier chords reach the app at all, and through which route? A sent WM_KEYDOWN does
# not run through the system key-state bookkeeping, so winit reads the letter unmodified -
# measured here, not assumed. Every claim in this file is a chord claim, so this is settled
# once, up front, before anything is reported.
$hw = $false
$chords = $false
$b0 = ShotPx "v5e-00-pre"
ChordSent @($VK_CONTROL) $VK_K
$b1 = ShotPx "v5e-00-palette"
$dSent = [V5C]::Diff($b0, $b1, 0, $yBar, $cw, $STATUS_Y1, 0)
Write-Host ("preflight: sent Ctrl+K repainted " + $dSent + " px")
if ($dSent -gt 0) {
    $chords = $true
} else {
    if (Ensure-Fg) {
        ChordHw @($VK_CONTROL) $VK_K
        $b1h = ShotPx "v5e-00-palette-hw"
        $dHw = [V5C]::Diff($b0, $b1h, 0, $yBar, $cw, $STATUS_Y1, 0)
        Write-Host ("preflight: hardware Ctrl+K repainted " + $dHw + " px, foreground is ours")
        if ($dHw -gt 0) { $chords = $true; $hw = $true }
    } else {
        Write-Host "preflight: the foreground cannot be taken - a hardware chord would be typed into someone else's window, so it is not attempted"
    }
}
Write-Host ("preflight: chords " + $(if ($chords) { ("arrive via " + $(if ($hw) { "hardware" } else { "sent" }) + " keys") } else { "DO NOT arrive" }))
if ($chords) {
    Press $VK_ESCAPE
    $b2 = ShotPx "v5e-00-closed"
    $d2 = [V5C]::Diff($b0, $b2, 0, $yBar, $cw, $STATUS_Y1, 0)
    if ($d2 -ne 0) {
        Write-Host ("  Escape left " + $d2 + " px different; trying once more")
        Press $VK_ESCAPE
        $b3 = ShotPx "v5e-00-closed2"
        $d3 = [V5C]::Diff($b0, $b3, 0, $yBar, $cw, $STATUS_Y1, 0)
        if ($d3 -ne 0) {
            Write-Host "  BLOCKED a floating panel may still be open and holding the keyboard - nothing below can be trusted"
            $chords = $false
        }
    }
    if ($chords) {
        # A frame pair that is not identical would mean the palette is still up; the run needs
        # the same starting frame the claims are measured against, so say which one we have.
        Write-Host ("preflight: keyboard route settled on body diff " + [V5C]::Diff($b0, $b2, 0, $yBar, $cw, $STATUS_Y1, 0) + " px against the pre-test frame")
    }
}
if (-not $chords) {
    Write-Host "  Every claim in this file needs a chord (Ctrl+C/X/V, Ctrl+Shift+C/V, Ctrl+F, Ctrl+Z)."
    Write-Host "  The rig cannot deliver one to this window, so the product is not being measured."
    $blocked += 6
}

# --- establish a selection ---------------------------------------------------------------
$L = -1
if ($chords -and $clipOk -and $blocked -eq 0) {
    $px = ShotPx "v5e-01-loaded"
    $bg = [int[]][V5C]::Modal($px, $bx0, $by0, $bx1, $by1, 0, 2)
    $box = [int[]][V5C]::InkBox($px, $bx0, $by0, $bx1, $by1, $bg[0], $bg[1], $bg[2], 30)
    Write-Host ("  document ink box=" + ($box -join ",") + " bg=" + ($bg[0], $bg[1], $bg[2] -join ","))
    if ($box[4] -lt 400) {
        Write-Host "  FAIL nothing is painted in the canvas band - there is no card to click"
        $fail++
    } elseif (-not $canInject) {
        Write-Host "  BLOCKED the selection needs a click, and clicks cannot be injected here"
        $blocked += 6
    } else {
        # Side points first: in a mindmap the first-level cards sit left and right of the
        # root, and a leaf keeps the arithmetic (one chord, one node) unambiguous.
        $midY = [int](($box[1] + $box[3]) / 2.0)
        $candX = @($box[0], $box[2], $box[0], $box[2], $box[0], $box[2], [int](($box[0] + $box[2]) / 2.0))
        $candY = @($midY, $midY, $box[1], $box[3], $box[3], $box[1], $midY)
        for ($i = 0; $i -lt $candX.Count; $i++) {
            if ($L -gt 0) { break }
            $lx = [single](($candX[$i] - $origin.X) / $scale)
            $ly = [single](($candY[$i] - $origin.Y) / $scale)
            ClickPt $lx $ly
            $n = ChordCopy @($VK_CONTROL) $VK_C
            Write-Host ("    click candidate " + $i + " at " + $candX[$i] + "," + $candY[$i] + " -> copied lines=" + $n)
            if ($n -gt 0) { $L = $n }
        }
        if ($L -gt 0) {
            Write-Host ("  selected subtree carries " + $L + " node(s)")
        } elseif ($n -eq -2) {
            Write-Host "  BLOCKED the clipboard became unreadable mid-loop"
            $blocked += 6
        } else {
            Write-Host "  BLOCKED no click produced a copy: the rig could not select a node (chords do arrive, so this is aim, not the app)"
            $blocked += 6
        }
    }
}

if ($L -gt 0) {
    # --- the handover, and the precondition every claim below rests on -------------------
    Press $VK_F6
    $who = FocusName
    Write-Host ("after F6: '" + (TraceLast "event ") + "'; focus sits on: '" + $who + "'")
    if ($who -eq "canvas") {
        Write-Host "  FAIL F6 never took the keyboard off the canvas - there is nothing to prove"
        $fail++
    } else {
        Write-Host ("  precondition OK: the canvas does not hold the keyboard (focus '" + $who + "')")

        # --- claim 1: copy while the panels hold the keyboard ----------------------------
        Park
        $L2 = ChordCopy @($VK_CONTROL) $VK_C
        if ($L2 -eq -2) {
            Write-Host "  BLOCKED the clipboard became unreadable"
            $blocked++
        } elseif ($L2 -ne $L) {
            Write-Host ("  FAIL Ctrl+C with the keyboard in the panels wrote " + $L2 + " clipboard lines, expected " + $L + " (selection or chord lost on the way)")
            $fail++
        } else {
            Write-Host ("  claim 1 OK: Ctrl+C copied the same " + $L + "-node subtree and pushed its plain text out, with no canvas focus")
        }

        # --- claim 2: paste in that state still edits the document ----------------------
        $before = SettlePx "v5e-02-before-paste"
        $mark = [DateTime]::Now
        Ctrl $VK_V
        $afterPaste = -1
        $dBand = -1
        $after = $null
        $cp = WaitCount $mark
        if ($cp -lt 0) {
            Write-Host "  FAIL Ctrl+V with the keyboard in the panels changed nothing (no new revision reached the container)"
            $fail++
        } else {
            $afterPaste = $cp
            $after = SettlePx "v5e-03-after-paste"
            $dBand = [V5C]::Diff($before, $after, $bx0, $by0, $bx1, $by1, 0)
            Write-Host ("  nodeCount -> " + $afterPaste + "; document band repainted " + $dBand + " px")
            if ($dBand -lt 30) {
                Write-Host "  FAIL the document did not repaint, although a new revision was written"
                $fail++
            } else {
                Write-Host "  claim 2 OK: the paste landed on the document while the panels held the keyboard"
            }
        }

        # --- claim 3: cut removes exactly what it copied, paste puts it back -------------
        $afterCut = -1
        $back = -1
        $mark = [DateTime]::Now
        $Lc = ChordCopy @($VK_CONTROL) $VK_X
        $cu = WaitCount $mark
        if ($Lc -lt 0 -or $cu -lt 0 -or $afterPaste -lt 0) {
            Write-Host ("  FAIL Ctrl+X did not cut (clipboard lines=" + $Lc + ", nodeCount=" + $cu + ")")
            $fail++
        } else {
            $afterCut = $cu
            $cut = $afterPaste - $afterCut
            Write-Host ("  nodeCount " + $afterPaste + " -> " + $afterCut + " (cut removed " + $cut + ", clipboard held " + $Lc + " lines)")
            if ($cut -ne $Lc) {
                Write-Host "  FAIL Ctrl+X removed a different number of nodes than it copied - cut is not one execution of copy+delete"
                $fail++
            } else {
                $mark = [DateTime]::Now
                Ctrl $VK_V
                $pv = WaitCount $mark
                if ($pv -lt 0) {
                    Write-Host "  FAIL the paste after the cut wrote no new revision"
                    $fail++
                } else {
                    $back = $pv
                    $gain = $back - $afterCut
                    Write-Host ("  nodeCount " + $afterCut + " -> " + $back + " (pasted back " + $gain + ")")
                    if ($gain -ne $Lc -or $back -ne $afterPaste) {
                        Write-Host "  FAIL the cut/paste round trip did not restore the same node count"
                        $fail++
                    } else {
                        Write-Host "  claim 3 OK: one chord, one execution - cut took " + $Lc + " away and paste gave " + $Lc + " back"
                    }
                }
            }
        }

        # --- claim 4: the Shift chords do not fall through ------------------------------
        # egui's modifier match tolerates an extra Shift, so Ctrl+Shift+C also looks like a
        # plain copy. Checking (and consuming) the Shift variants first is the only defence,
        # and a fall-through would show up as a pasted subtree: the node count would move.
        $mark = [DateTime]::Now
        CtrlShift $VK_C
        CtrlShift $VK_V
        $styleCount = WaitCount $mark
        if ($styleCount -lt 0) {
            Write-Host ("  claim 4 OK: the style chords flushed no new revision - the node count never moved (still " + $back + ")")
        } elseif ($styleCount -ne $back) {
            Write-Host ("  FAIL Ctrl+Shift+C/V moved the nodeCount " + $back + " -> " + $styleCount + ": a style chord ran a content chord")
            $fail++
        } else {
            Write-Host ("  claim 4 OK: the style chords wrote a revision and left the node count at " + $styleCount)
        }

        # --- claim 5: a text field owns Ctrl+Z ------------------------------------------
        # The search bar focuses its own TextEdit on open and the trail names it, so the
        # precondition is read from the app rather than inferred from the outcome.
        Ctrl $VK_F
        $who = FocusName
        Write-Host ("after Ctrl+F the keyboard sits on: '" + $who + "'")
        if ($who -ne "search-input") {
            Write-Host "  BLOCKED the field never took the keyboard, so the hand-off cannot be judged"
            $blocked++
        } else {
            $withField = SettlePx "v5e-04-field"
            Ctrl $VK_Z
            $afterUndoZ = SettlePx "v5e-05-field-undo"
            $dz = [V5C]::Diff($withField, $afterUndoZ, $bx0, $by0, $bx1, $by1, 0)
            $zbox = [int[]][V5C]::DiffBox($withField, $afterUndoZ, $bx0, $by0, $bx1, $by1, 0)
            Write-Host ("Ctrl+Z with the search field focused: document band moved " + $dz + " px box=" + ($zbox -join ","))
            Press $VK_ESCAPE
            $who2 = FocusName
            Write-Host ("after Escape the keyboard sits on: '" + $who2 + "'")
            if ($who2 -eq "search-input") {
                Write-Host "  BLOCKED Escape did not give the field's keyboard back; the second half cannot be judged"
                $blocked++
            } else {
                $noField = SettlePx "v5e-06-nofield"
                Ctrl $VK_Z
                $afterDocZ = SettlePx "v5e-07-doc-undo"
                $dd = [V5C]::Diff($noField, $afterDocZ, $bx0, $by0, $bx1, $by1, 0)
                Write-Host ("the same chord with no field focused: document band moved " + $dd + " px")
                if ($dz -ne 0) {
                    Write-Host "  FAIL Ctrl+Z stepped the document while a text field held the keyboard"
                    $fail++
                } elseif ($dd -eq 0) {
                    Write-Host "  FAIL Ctrl+Z did nothing even with the field closed - the first half proved nothing"
                    $fail++
                } else { Write-Host "  claim 5 OK: the field kept its own undo, and the document kept its own" }
            }
            $withField = $null; $afterUndoZ = $null; $noField = $null; $afterDocZ = $null
        }
        $before = $null; $after = $null; $px = $null
    }
}

# --- claim 6: control, nothing pressed ---------------------------------------------------
# If this fails, "byte-identical" above would be measuring a camera rather than a product.
$c1 = SettlePx "v5e-08-control"
Start-Sleep -Milliseconds 900
$c2 = ShotPx "v5e-09-control-b"
$dc = [V5C]::Diff($c1, $c2, 0, $yBar, $cw, $STATUS_Y1, 0)
$dcbox = [int[]][V5C]::DiffBox($c1, $c2, 0, $yBar, $cw, $STATUS_Y1, 0)
Write-Host ("claim 6: two frames, no keystroke, body diff=" + $dc + " px box=" + ($dcbox -join ","))
if ($dc -ne 0) {
    Write-Host "  FAIL the view repaints while nothing is pressed - pixel claims in this run mean nothing"
    $fail++
}
$c1 = $null; $c2 = $null

# --- does the keyboard route still deliver chords at the end of the run? -----------------
# This is the camera check, but for keys. A claim measured with a chord that stopped arriving
# halfway is a product failure the app never caused, so the route is re-proved before any
# verdict is printed and the run is downgraded if it broke.
$routeOk = $false
if ($chords) {
    $r0 = ShotPx "v5e-10-recheck-a"
    Ctrl $VK_K
    $r1 = ShotPx "v5e-10-recheck-b"
    $dr = [V5C]::Diff($r0, $r1, 0, $yBar, $cw, $STATUS_Y1, 0)
    Press $VK_ESCAPE
    $routeOk = $dr -gt 0
    Write-Host ("recheck: Ctrl+K repainted " + $dr + " px at the end of the run; " + $hwMiss + " chord(s) fell back to the sent route")
}

Start-Sleep -Milliseconds 300
[void][V5C]::SetWindowPos($hwnd, [IntPtr](-2), 0, 0, 0, 0, (0x0002 -bor 0x0001))
Stop-Process -Id $proc.Id -Force
Start-Sleep -Milliseconds 400
Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
if ($chords -and -not $routeOk) {
    Write-Host "CLIP INCONCLUSIVE: the chord route that everything above was measured with no longer arrives at this window. Re-run."
    exit 3
}
if ($fail -gt 0) { Write-Host ("CLIP FAIL (" + $fail + " failed, " + $blocked + " blocked)"); exit 1 }
if ($blind -gt 0) {
    Write-Host ("CLIP INCONCLUSIVE: " + $blind + " frame(s) were captured through another window - the desktop, not the app, moved in front. Re-run, or close what popped up.")
    exit 3
}
if ($blocked -gt 0) {
    Write-Host ("CLIP PARTLY VERIFIED: " + $blocked + " claim(s) blocked by this desktop, 0 failed")
    exit 2
}
Write-Host "CLIP OK"
