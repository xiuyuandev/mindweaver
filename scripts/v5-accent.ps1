param(
    [string]$Exe = "target/release/mw-shell.exe",
    [string]$ShotDir = "shots/qa"
)

# v5 custom-accent probe (real input, release build).
#
# A custom accent is allowed to borrow exactly one thing from the picked colour: its hue.
# Everything the user did not ask for must provably stay put. Eight claims, all measured
# from pixels the app actually painted:
#   1. F6 hands the keyboard from the canvas to the panels, and hands it back. Handing
#      over must repaint the mode indicator and nothing inside the document.
#   2. Applying a hue chip repaints the cards - the override is not a dead preference.
#   3. Card ink contrast does not fall off a cliff: same slot, before vs after.
#   4. The camera does not move: the canvas ink bounding box is byte-identical.
#   5. Undo and redo each step exactly one state, landing on a frame pixel-identical to
#      the two known frames (so no half-applied repaint, no double push). Reached by
#      Ctrl+Z when the chord arrives, otherwise by Tab-walking to the toolbar's own
#      undo/redo buttons and pressing Space - the same command, minus the modifier.
#   6. "Theme default" clears the override and lands back on the original frame.
#   7. themeAccent reaches the container, and clearing it reaches the container too.
#   8. Opening a panel does not silence the canvas: after F8 the focus trail still names
#      the canvas, and Tab still adds a topic.
#      The canvas region is registered under a pinned id for exactly this reason - with a
#      layout-supplied id, changing the panel set changes the id, egui's dead-man switch
#      drops the focus, and every bare canvas key dies without a trace (the headless
#      regression for it is a_panel_toggle_keeps_the_canvas_keyboard_focus). The Tab half
#      needs the foreground window, which this desktop will not hand over - see below.
#
# Input is keyboard-only, and that is not a style choice - it is what this machine
# allows. See the preflight below: hardware injection (SetCursorPos/mouse_event) is dead
# while something else holds the foreground, so every step goes through sent WM_KEYDOWN
# plus the app's own Tab order. That lock comes and goes, and when it lifts the rig takes
# the better routes (real chords, real canvas keys) - so both branches stay in the file.
#
# Every frame is grabbed with CopyFromScreen, which photographs whatever is on top. So
# Shot raises our own window first (HWND_TOP, no activation) and samples the client rect
# with WindowFromPoint; a frame still covered by a foreign dialog is reported, and the run
# ends INCONCLUSIVE (exit 3) instead of blaming the app for a window it could not see.
#
# The first keypress of the run is F6, not Tab, and that is load-bearing: the canvas
# holds the keyboard at startup and installs a focus lock for Tab, so Tab there means
# "add a child topic". A probe that pressed Tab to prove the panels were unreachable
# would edit the document it is about to measure. So the "before" half of claim 1 lives
# in the headless Rust test (crates/mw-shell/src/canvas.rs, keyboard_ring), and this rig
# only ever walks Tab once the handover has been confirmed in pixels - if F6 did not
# arrive, it says so and stops instead of walking into Tab.
#
# After the handover the probe walks Tab until the app reports (MW_TRACE) that the
# accent row has focus, then presses Space. That is what a keyboard-only user does, so
# the rig exercises the real path instead of a mouse-only one.
#
# Claim 7 does not use Ctrl+S: a modifier chord never reaches winit here (preflight
# proves it). The app writes a recovery point every 3s through the same mw_format::save,
# so the bytes that land in .autosave/ answer the same question as a manual save.
#
# The chips are found by colour, not by measured geometry: they are the only ~15pt
# saturated discs in the panel (theme-card dots are 8pt and fall out of the row scan).
#
# ASCII only (PowerShell 5.1 reads BOM-less files as ANSI); CJK strings are built from
# char codes.

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.IO.Compression.FileSystem
Add-Type -ReferencedAssemblies System.Drawing -Language CSharp @'
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

public static class V5A {
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
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int n);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct P { public int X, Y; }

    public const uint WM_KEYDOWN = 0x0100, WM_KEYUP = 0x0101;

    /// Title of whatever window last covered the measured rect (see Covered).
    public static string Blocker = "";

    /// How many sample points of this screen rect resolve to a *titled* window other
    /// than `h`. Returns {covered, total}. A foreign dialog parked over the app turns
    /// every pixel claim into a false failure - measured here once, when a Settings
    /// window landed on top mid-run and "undo did not restore the frame" read like a
    /// product bug. Asking the OS who is really on top costs one call per sample and is
    /// the only way the rig can tell "the app painted wrong" from "the app was not
    /// visible". Untitled roots are skipped: those are the helper windows every app owns
    /// (IME candidate, tooltip, caret), and one of them over one sample point is not a
    /// blind camera - it is the app's own border pixel.
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

    /// Keys go through SendMessage, not keybd_event. Hardware injection is delivered to
    /// whatever the OS currently calls the foreground window, and a background process
    /// cannot take the foreground here - SetCursorPos returns FALSE outright (something
    /// holds an input lock). A sent WM_KEYDOWN reaches this window's procedure either way,
    /// and winit builds its key events from those messages. Measured on this machine:
    /// posted F8 repaints 332k pixels; mouse_event on the same window repaints zero.
    public static void Key(IntPtr h, int vk, bool up) {
        uint l = 0x00000001u | (1u << 16);      // repeat = 1, scan code = 0, previous state = up
        if (up) l |= 0xC0000000u;               // transition state + previous state
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

    // ---- pixel access -------------------------------------------------------------
    // LockBits once per frame: GetPixel in a scan loop costs seconds on a 1620x1080
    // client area, and every measurement below walks the frame more than once.
    // Layout is {w, h, 0xRRGGBB * w * h}.
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
    public static int[] RGB(int p) { return new int[] { R(p), G(p), B(p) }; }
    static int Sat(int p) {
        int hi = R(p) > G(p) ? R(p) : G(p); if (B(p) > hi) hi = B(p);
        int lo = R(p) < G(p) ? R(p) : G(p); if (B(p) < lo) lo = B(p);
        return hi - lo;
    }
    static int Dist(int p, int r, int g, int b) {
        return Math.Abs(R(p) - r) + Math.Abs(G(p) - g) + Math.Abs(B(p) - b);
    }
    static double Lum(int r, int g, int b) {
        Func<int, double> lin = v => {
            double x = v / 255.0;
            return x <= 0.04045 ? x / 12.92 : Math.Pow((x + 0.055) / 1.055, 2.4);
        };
        return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
    }
    static double Ratio(double a, double b) {
        double hi = a > b ? a : b, lo = a > b ? b : a;
        return (hi + 0.05) / (lo + 0.05);
    }

    // ---- frame comparison ---------------------------------------------------------
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

    /// Where a non-zero diff actually lives - "7638 pixels differ" is not actionable.
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
                int key = p; int n;
                hist.TryGetValue(key, out n);
                hist[key] = n + 1;
            }
        }
        int bestKey = 0, best = -1;
        foreach (var kv in hist) if (kv.Value > best) { best = kv.Value; bestKey = kv.Key; }
        return new int[] { R(bestKey), G(bestKey), B(bestKey), best };
    }

    /// Bounding box of pixels far from the given background. The canvas camera moves
    /// this box; a colour-only change must not touch it.
    public static int[] InkBox(int[] a, int x0, int y0, int x1, int y1, int br, int bg, int bb, int thr) {
        int minx = 1 << 20, miny = 1 << 20, maxx = -1, maxy = -1, n = 0;
        for (int y = y0; y < y1; y++) {
            for (int x = x0; x < x1; x++) {
                if (Dist(At(a, x, y), br, bg, bb) <= thr) continue;
                n++;
                if (x < minx) minx = x; if (x > maxx) maxx = x;
                if (y < miny) miny = y; if (y > maxy) maxy = y;
            }
        }
        return new int[] { minx, miny, maxx, maxy, n };
    }

    // ---- chip grid ----------------------------------------------------------------
    // Scored by how many *disc-sized* saturated runs a row holds. The theme cards above
    // carry two things that would otherwise win a plain pixel count: a 168px accent
    // stroke along their top edge, and three 9px palette dots. Filtering on run width
    // (the discs are 22px at 150%) leaves the chip row as the only row scoring 9.
    public static int[] BestChipRow(int[] a, int x0, int y0, int x1, int y1, int minSat, int minW, int maxW) {
        int bestY = -1, best = 0;
        for (int y = y0; y < y1; y++) {
            int score = 0, start = -1;
            for (int x = x0; x <= x1; x++) {
                bool on = x < x1 && Sat(At(a, x, y)) >= minSat;
                if (on && start < 0) start = x;
                if (!on && start >= 0) {
                    int wdt = x - start;
                    if (wdt >= minW && wdt <= maxW) score++;
                    start = -1;
                }
            }
            if (score > best) { best = score; bestY = y; }
        }
        return new int[] { bestY, best };
    }

    /// Centres + widths of saturated runs along one scan line.
    public static double[] Runs(int[] a, int y, int x0, int x1, int minSat, int minW) {
        var outp = new System.Collections.Generic.List<double>();
        int start = -1;
        for (int x = x0; x <= x1; x++) {
            bool on = x < x1 && Sat(At(a, x, y)) >= minSat;
            if (on && start < 0) start = x;
            if (!on && start >= 0) {
                if (x - start >= minW) { outp.Add((start + x - 1) / 2.0); outp.Add(x - start); }
                start = -1;
            }
        }
        return outp.ToArray();
    }

    /// Dark, unsaturated column clusters in a band - how a CJK button label is found.
    /// Returns triples: [x centre, y centre, width]. The y centre is the mean y of the
    /// dark pixels in the cluster, not a column mean - the earlier version weighted by x
    /// and handed back a number that looked plausible and pointed at nothing.
    public static double[] Label(int[] a, int x0, int y0, int x1, int y1, double maxLum, int maxSat, int minW) {
        int n = x1 - x0;
        int[] cnt = new int[n];
        double[] ysum = new double[n];
        for (int x = x0; x < x1; x++) {
            int c = 0; double ys = 0.0;
            for (int y = y0; y < y1; y++) {
                int p = At(a, x, y);
                if (Sat(p) <= maxSat && Lum(R(p), G(p), B(p)) <= maxLum) { c++; ys += y; }
            }
            cnt[x - x0] = c; ysum[x - x0] = ys;
        }
        var outp = new System.Collections.Generic.List<double>();
        int start = -1, prev = -1; double sy = 0.0; int px = 0;
        for (int i = 0; i < cnt.Length; i++) {
            if (cnt[i] > 0) {
                if (px == 0) start = i;
                else if (i - prev > 3) { Push(outp, x0, start, prev, sy, px, minW); start = i; sy = 0.0; px = 0; }
                sy += ysum[i]; px += cnt[i]; prev = i;
            }
        }
        if (px > 0) Push(outp, x0, start, prev, sy, px, minW);
        return outp.ToArray();
    }
    static void Push(System.Collections.Generic.List<double> outp, int x0, int start, int prev, double sy, int px, int minW) {
        if (prev - start < minW || px < 40) return;
        outp.Add((x0 + start + x0 + prev) / 2.0);
        outp.Add(sy / px);
        outp.Add(prev - start);
    }

    // ---- card ink contrast ---------------------------------------------------------
    /// Largest connected region of the target colour, shrunk to the solid rectangle
    /// inside it: branch lines are the same colour as the card they belong to, so the
    /// raw bounding box would reach all the way to the sibling it joins.
    public static int[] CardRect(int[] a, int x0, int y0, int x1, int y1, int tr, int tg, int tb, int tol) {
        int w = a[0], h = a[1];
        byte[] seen = new byte[w * h];
        int[] stack = new int[w * h];
        int bestArea = 0;
        int bx0 = 0, by0 = 0, bx1 = 0, by1 = 0;
        for (int sy = y0; sy < y1; sy++) {
            for (int sx = x0; sx < x1; sx++) {
                int s = sy * w + sx;
                if (seen[s] != 0) continue;
                if (Dist(At(a, sx, sy), tr, tg, tb) > tol) { seen[s] = 1; continue; }
                int top = 0, area = 0, minx = sx, maxx = sx, miny = sy, maxy = sy;
                stack[top++] = s; seen[s] = 1;
                while (top > 0) {
                    int cur = stack[--top];
                    int cy = cur / w, cx = cur - cy * w;
                    area++;
                    if (cx < minx) minx = cx; if (cx > maxx) maxx = cx;
                    if (cy < miny) miny = cy; if (cy > maxy) maxy = cy;
                    for (int dy = -1; dy <= 1; dy++) {
                        for (int dx = -1; dx <= 1; dx++) {
                            int nx = cx + dx, ny = cy + dy;
                            if (nx < x0 || ny < y0 || nx >= x1 || ny >= y1) continue;
                            int ni = ny * w + nx;
                            if (seen[ni] != 0) continue;
                            if (Dist(At(a, nx, ny), tr, tg, tb) > tol) { seen[ni] = 1; continue; }
                            seen[ni] = 1; stack[top++] = ni;
                        }
                    }
                }
                if (area > bestArea) { bestArea = area; bx0 = minx; by0 = miny; bx1 = maxx; by1 = maxy; }
            }
        }
        if (bestArea < 400) return new int[] { -1, -1, -1, -1 };
        int bw = bx1 - bx0 + 1, bh = by1 - by0 + 1;
        int[] rowc = new int[bh], colc = new int[bw];
        int rmax = 0, cmax = 0;
        for (int y = by0; y <= by1; y++) {
            for (int x = bx0; x <= bx1; x++) {
                if (Dist(At(a, x, y), tr, tg, tb) > tol) continue;
                rowc[y - by0]++; colc[x - bx0]++;
            }
        }
        for (int i = 0; i < bh; i++) if (rowc[i] > rmax) rmax = rowc[i];
        for (int i = 0; i < bw; i++) if (colc[i] > cmax) cmax = colc[i];
        if (rmax < 12 || cmax < 12) return new int[] { -1, -1, -1, -1 };
        int ty0 = -1, ty1 = -1, run = 0, bestRun = 0, runStart = -1;
        for (int i = 0; i < bh; i++) {
            bool on = rowc[i] * 2 >= rmax;
            if (on) { if (runStart < 0) runStart = i; run++; }
            else { if (run > bestRun) { bestRun = run; ty0 = runStart; ty1 = i - 1; } run = 0; runStart = -1; }
        }
        if (run > bestRun) { ty0 = runStart; ty1 = bh - 1; }
        if (ty0 < 0 || ty1 - ty0 < 8) return new int[] { -1, -1, -1, -1 };
        int hh = ty1 - ty0 + 1;
        int tx0 = -1, tx1 = -1; run = 0; bestRun = 0; runStart = -1;
        for (int i = 0; i < bw; i++) {
            int c = 0;
            for (int y = by0 + ty0; y <= by0 + ty1; y++) {
                if (Dist(At(a, bx0 + i, y), tr, tg, tb) <= tol) c++;
            }
            bool on = c * 2 >= hh;
            if (on) { if (runStart < 0) runStart = i; run++; }
            else { if (run > bestRun) { bestRun = run; tx0 = runStart; tx1 = i - 1; } run = 0; runStart = -1; }
        }
        if (run > bestRun) { tx0 = runStart; tx1 = bw - 1; }
        if (tx0 < 0) return new int[] { -1, -1, -1, -1 };
        return new int[] { bx0 + tx0, by0 + ty0, bx0 + tx1, by0 + ty1 };
    }

    /// WCAG contrast between a card fill and the strongest ink printed on it.
    public static double CardInk(int[] a, int x0, int y0, int x1, int y1, int tr, int tg, int tb) {
        double band = Lum(tr, tg, tb);
        double ink = band; int dist = 0;
        for (int y = y0; y <= y1; y++) {
            for (int x = x0; x <= x1; x++) {
                int p = At(a, x, y);
                int d = Dist(p, tr, tg, tb);
                if (d > dist) { dist = d; ink = Lum(R(p), G(p), B(p)); }
            }
        }
        return dist == 0 ? -1.0 : Ratio(ink, band);
    }
}
'@
[void][V5A]::SetProcessDPIAware()

function Cj([int[]]$codes) {
    $s = ""
    foreach ($c in $codes) { $s += [string]([char]$c) }
    return $s
}
# root + three first-level cards: branch colours and card fills are both on screen.
$rootT = Cj @(0x7EC7, 0x601D)
$kidA = Cj @(0x914D, 0x8272)
$kidB = Cj @(0x4EA4, 0x4E92)
$kidC = Cj @(0x5B57, 0x4F53)
# The reset button label ("theme default", two CJK glyphs + one) is only named here;
# the rig never matches on it.

function Node([string]$id, [string]$parent, [string]$text, [string]$kids) {
    return '"' + $id + '":{"id":"' + $id + '","parentId":' + $parent + ',"children":[' + $kids + '],"text":"' + $text + '","collapsed":false,"modifiedAt":0}'
}

$tmp = Join-Path $env:TEMP ("mw-v5a-" + [Guid]::NewGuid().ToString("N"))
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
$manifest = '{"formatVersion":1,"app":"probe/1.0","docId":"probe-v5a","title":"probe","createdAt":0,"modifiedAt":0,"nodeCount":4}'
$dir = Join-Path $tmp "doc"
[void](New-Item -ItemType Directory -Force -Path $dir)
[System.IO.File]::WriteAllText((Join-Path $dir "content.json"), $content, $encNoU8)
[System.IO.File]::WriteAllText((Join-Path $dir "manifest.json"), $manifest, $encNoU8)
[System.IO.File]::WriteAllText((Join-Path $dir "mimetype"), "application/vnd.mindweaver.zmind", $encNoU8)
$fx = Join-Path $tmp "v5a.zmind"
# The recovery point the app writes every 3s while the document is dirty. Same container,
# same writer as Ctrl+S, and it needs no modifier chord. `slot_for` hangs it off the
# *document's own directory*, and the document sits in $tmp - not in the unpacked folder
# the zip was built from.
$slot = Join-Path (Join-Path $tmp ".autosave") "v5a.autosave.zmind"
[System.IO.Compression.ZipFile]::CreateFromDirectory($dir, $fx)

$env:MW_HOME = "0"
$env:MW_WINDOW_STATE = "off"
$env:MW_OPEN = $fx
# The app writes one line every time keyboard focus moves (and an `event` line for the F6
# handover). The probe needs it: Tab order depends on how many focusable widgets precede
# the accent row, and that number moves whenever the panel changes. Guessing it from
# pixels would re-invent the geometry this rig is supposed to measure.
$trace = Join-Path $tmp "focus.log"
$env:MW_TRACE = $trace

$proc = Start-Process -FilePath $Exe -PassThru
$hwnd = [IntPtr]::Zero
for ($i = 0; $i -lt 60) {
    Start-Sleep -Milliseconds 250
    $proc.Refresh()
    $h = $proc.MainWindowHandle
    if ($h -ne [IntPtr]::Zero) {
        $r = New-Object V5A+RECT
        [void][V5A]::GetClientRect($h, [ref]$r)
        if ($r.Right -ge 600 -and $r.Bottom -ge 400) { $hwnd = $h; break }
    }
}
if ($hwnd -eq [IntPtr]::Zero) { Write-Error "main window never appeared"; exit 1 }
[void][V5A]::ShowWindow($hwnd, 9)
[void][V5A]::BringWindowToTop($hwnd)
try { (New-Object -ComObject WScript.Shell).AppActivate($proc.Id) | Out-Null } catch { }
[void][V5A]::SetForegroundWindow($hwnd)
Start-Sleep -Milliseconds 1000

$rect = New-Object V5A+RECT
[void][V5A]::GetClientRect($hwnd, [ref]$rect)
$origin = New-Object V5A+P
[void][V5A]::ClientToScreen($hwnd, [ref]$origin)
$cw = $rect.Right
$ch = $rect.Bottom
$scale = [V5A]::GetDpiForWindow($hwnd) / 96.0
Write-Host ("client=" + $cw + "x" + $ch + " scale=" + $scale + " origin=" + $origin.X + "," + $origin.Y)
if ([math]::Abs($scale - 1.5) -gt 0.05) { Write-Warning "chip and label geometry is calibrated for 150% DPI" }

# --- can the rig actually see the app? ---------------------------------------------------
# HWND_TOP with NOACTIVATE: put our own window back above whatever dialog wandered in,
# without asking for the foreground - a background process is not allowed to take it, and
# pretending otherwise is what made the old runs measure a Settings window.
function Raise() {
    [void][V5A]::SetWindowPos($hwnd, [IntPtr]::Zero, 0, 0, 0, 0, (0x0001 -bor 0x0002 -bor 0x0010))
    Start-Sleep -Milliseconds 180
}
function CoveredPts() {
    $c = [V5A]::Covered($hwnd, $origin.X, $origin.Y, ($origin.X + $cw), ($origin.Y + $ch), 90)
    return @([int]$c[0], [int]$c[1], [V5A]::Blocker)
}

function Shot([string]$name) {
    # Raise first: CopyFromScreen grabs whatever is on the screen, and a dialog that
    # wandered over the app would be measured as if the app had painted it.
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
# --- keys ------------------------------------------------------------------------------
# Every step is a sent WM_KEYDOWN/WM_KEYUP pair (see V5A.Key). keybd_event is not used at
# all: it lands on whichever window the OS calls foreground, which on this machine is not
# ours, so half of these sleeps would just be waiting for a key that never arrived.
$SETTLE = 620
function Press([int]$vk) {
    [V5A]::Key($hwnd, $vk, $false)
    Start-Sleep -Milliseconds 60
    [V5A]::Key($hwnd, $vk, $true)
    Start-Sleep -Milliseconds $SETTLE
}
function Chord([int[]]$mods, [int]$vk) {
    foreach ($m in $mods) { [V5A]::Key($hwnd, $m, $false); Start-Sleep -Milliseconds 45 }
    [V5A]::Key($hwnd, $vk, $false)
    Start-Sleep -Milliseconds 55
    [V5A]::Key($hwnd, $vk, $true)
    foreach ($m in $mods) { [V5A]::Key($hwnd, $m, $true); Start-Sleep -Milliseconds 45 }
    Start-Sleep -Milliseconds $SETTLE
}
function Ctrl([int]$vk) { Chord @($VK_CONTROL) $vk }
function CtrlShift([int]$vk) { Chord @($VK_CONTROL, $VK_SHIFT) $vk }

# --- focus walk -------------------------------------------------------------------------
# MW_TRACE gives one line per focus change plus the `event` lines the app cannot paint, so
# the probe reads the app's own opinion about where the keyboard is instead of
# reverse-engineering a ring from pixels.
function TraceText() {
    if (-not (Test-Path -LiteralPath $trace)) { return "" }
    try {
        # FileShare ReadWrite: the app holds this file open for writing, and the default
        # share mode of Get-Content would refuse to coexist with it.
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

# The last line carrying this prefix. Focus is state and an F6 handover is an event, and
# they interleave - reading only "the last thing written" would report whichever of the
# two landed second.
function TraceLast([string]$prefix) {
    $found = ""
    foreach ($l in (TraceLines)) { if ($l.StartsWith($prefix)) { $found = $l } }
    return $found
}

function TabCount() { return (TraceLines).Count }

function TabTo([string]$want) {
    $prev = TraceTail
    $before = TabCount
    $stale = 0
    for ($i = 0; $i -lt 240; $i++) {
        Press $VK_TAB
        $now = TraceTail
        if ($now.StartsWith("focus " + $want)) {
            Write-Host ("    focus reached '" + $want + "' after " + ($i + 1) + " tabs")
            return $true
        }
        if ($now -eq "" -or $now -ne $prev) { $stale = 0; $prev = $now; continue }
        # A TextEdit swallows Tab (its event filter claims it), so a walk that really stops
        # reporting movement means focus is parked in a field. Escape hands it back.
        # The threshold has to be larger than the whole ring: pressing Escape resets the
        # walk to its start, so firing it on a quiet stretch would send the probe back
        # round and round the first few widgets - which is exactly what a threshold of 10
        # did before the trace covered every widget.
        $stale++
        if ($stale -ge 45) {
            Write-Host "    focus stuck (a field is eating Tab); Escape and keep walking"
            Press $VK_ESCAPE
            $stale = 0
        }
    }
    Write-Host ("    walk over: trace lines " + $before + " -> " + (TabCount) + ", tail='" + (TraceTail) + "'")
    return $false
}

# --- the recovery point on disk ---------------------------------------------------------
# One read attempt. Returns "" when the slot is missing or half-written (the background
# thread is not synchronized with this rig, and a torn zip is a retry, not a failure).
function ReadSlot() {
    if (-not (Test-Path -LiteralPath $slot)) { return "" }
    try {
        $zip = [System.IO.Compression.ZipFile]::OpenRead($slot)
        $t = ""
        $e = $zip.GetEntry("content.json")
        if ($null -ne $e) {
            $r = New-Object System.IO.StreamReader($e.Open(), [System.Text.Encoding]::UTF8)
            $t = $r.ReadToEnd()
            $r.Dispose()
        }
        $zip.Dispose()
        return $t
    } catch { return "" }
}

# Wait for a recovery point written after `$since`, so an older one on disk cannot be
# mistaken for the one this step produced.
function WaitSlot([DateTime]$since) {
    for ($try = 0; $try -lt 20; $try++) {
        if ((Test-Path -LiteralPath $slot) -and
            ([System.IO.File]::GetLastWriteTime($slot) -gt $since)) {
            $t = ReadSlot
            if ($t.Length -gt 0) { return $t }
        }
        Start-Sleep -Milliseconds 400
    }
    return ""
}

# --- mouse, if this desktop allows it ----------------------------------------------------
function ClickPt([single]$lx, [single]$ly) {
    if (-not $canInject) { return }
    $x = $origin.X + [int]($lx * $scale)
    $y = $origin.Y + [int]($ly * $scale)
    [void][V5A]::SetCursorPos($x, $y)
    Start-Sleep -Milliseconds 140
    [V5A]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [V5A]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 420
}
function MovePt([single]$lx, [single]$ly) {
    if (-not $canInject) { return }
    [void][V5A]::SetCursorPos($origin.X + [int]($lx * $scale), $origin.Y + [int]($ly * $scale))
    Start-Sleep -Milliseconds 260
}
# Park on bare canvas: egui re-tests hover every frame, so a cursor left on a chip would
# paint a hover ring into the very frame we are about to measure. When injection is dead
# the cursor cannot move at all, which is even better - it stays outside the window.
$PARK_LX = [single](($cw * 0.30) / $scale)
$PARK_LY = [single](($ch * 0.88) / $scale)
function Park() { MovePt $PARK_LX $PARK_LY }

# --- preflight: what does this desktop let the rig do? -----------------------------------
# Naming the blocker matters: a zero-pixel diff here would otherwise read like the app
# ignoring its own shortcuts.
function Is-Foreground() { return ([V5A]::GetForegroundWindow()) -eq $hwnd }
$canInject = -not [V5A]::CursorFrozen()
$fgw = [V5A]::GetForegroundWindow()
Write-Host ("preflight: injected mouse " + $(if ($canInject) { "usable" } else { "BLOCKED" }) + "; foreground window = '" + [V5A]::Title($fgw) + "'")
Write-Host ("preflight: our window is " + $(if (Is-Foreground) { "the foreground one" } else { "NOT the foreground one - egui's has_focus() also requires that, so a bare canvas key is correctly refused here" }))
if (-not $canInject) {
    Write-Host "  SetCursorPos returns FALSE, so clicks cannot be synthesized. Every step below"
    Write-Host "  is sent keys plus the app's Tab order - the same path a keyboard-only user takes."
}
$cur = New-Object V5A+P
[void][V5A]::GetCursorPos([ref]$cur)
$inClient = ($cur.X -ge $origin.X -and $cur.X -lt ($origin.X + $cw) -and $cur.Y -ge $origin.Y -and $cur.Y -lt ($origin.Y + $ch))
Write-Host ("  cursor at " + $cur.X + "," + $cur.Y + " lies " + $(if ($inClient) { "INSIDE" } else { "outside" }) + " the client rect" + $(if ($inClient) { " - one widget will be hovered in every frame" } else { "" }))

$yBar = [int](58 * $scale)
$STATUS_Y1 = $ch - [int](40 * $scale)
$PANEL_X0 = $cw - [int](260 * $scale)
# The status strip's right hand side: zoom and theme name are right-aligned there, and
# the F6 mode indicator is inserted ahead of them. The left hand side says "unsaved",
# which changes for reasons that have nothing to do with the keyboard, so claim 1 reads
# only this half.
$SX0 = [int]($cw * 0.55)
$SY0 = $ch - [int](30 * $scale)
# Canvas band: clear of the title bar, the toolbar pill, the minimap and both panels.
$bx0 = [int]($cw * 0.15); $bx1 = [int]($cw * 0.65)
$by0 = [int]($ch * 0.20); $by1 = [int]($ch * 0.72)
# The same band with the left hand side cut back to where a 220pt outline panel cannot
# reach even at 2x scale. The last claim opens the outline, and the panel's own pixels
# must not be mistaken for the document moving.
$nx0 = [int]($cw * 0.35); $nx1 = $bx1

$fail = 0
# Claims the rig could not reach because this desktop blocks part of the input path.
# Counted separately: a blocked claim must never be read as a passing one.
$blocked = 0
# Frames captured while another window was on top of the app. Any number above zero makes
# the pixel verdicts above untrustworthy, so the run ends INCONCLUSIVE rather than OK.
$blind = 0
$VK_F6 = 0x75
$VK_F8 = 0x77; $VK_F9 = 0x78; $VK_Z = 0x5A; $VK_S = 0x53
$VK_TAB = 0x09; $VK_ESCAPE = 0x1B; $VK_CONTROL = 0x11; $VK_SHIFT = 0x10
$VK_HOME = 0x24; $VK_K = 0x4B; $VK_SPACE = 0x20

function Near([int[]]$a, [int[]]$b, [int]$tol) {
    return ([math]::Abs($a[0] - $b[0]) -le $tol) -and ([math]::Abs($a[1] - $b[1]) -le $tol) -and ([math]::Abs($a[2] - $b[2]) -le $tol)
}

# --- rig self-test: does a modifier chord reach the app at all? -------------------------
# Sent WM_KEYDOWN does not run through the system key-state bookkeeping, so winit can read
# the letter as unmodified. Undo, redo and Ctrl+S all need a chord, so find that out here
# instead of reporting a product failure over a rig limitation.
$chords = $false
$b = Shot "v5a-00-pre"; $pre = [V5A]::Grab($b); $b.Dispose()
Ctrl $VK_K
$b = Shot "v5a-00-palette"; $pal = [V5A]::Grab($b); $b.Dispose()
$d = [V5A]::Diff($pre, $pal, 0, $yBar, $cw, $STATUS_Y1, 0)
Write-Host ("preflight: Ctrl+K repainted " + $d + " pixels -> chords " + $(if ($d -gt 0) { "arrive" } else { "DO NOT arrive" }))
if ($d -gt 0) {
    $chords = $true
    Press $VK_ESCAPE
    $b = Shot "v5a-00-closed"; $back = [V5A]::Grab($b); $b.Dispose()
    $d2 = [V5A]::Diff($pre, $back, 0, $yBar, $cw, $STATUS_Y1, 0)
    if ($d2 -ne 0) { Write-Host ("  note Escape did not restore the pre-test frame (" + $d2 + " px differ)") }
    $back = $null
}
$pre = $null; $pal = $null

# --- settle the shell: inspector open, outline closed, nothing selected --------------
# The panels' state is read back from pixels after every keypress rather than trusted:
# a key that never arrives must not silently turn into "the app ignored it".
function PanelState($name) {
    $bmp = Shot $name
    $p = [V5A]::Grab($bmp)
    $c = [int[]][V5A]::Modal($p, $bx0, $by0, $bx1, $by1, 0, 2)
    $l = [int[]][V5A]::Modal($p, [int](100 * $scale), $by0, [int](200 * $scale), $by1, 0, 2)
    $r = [int[]][V5A]::Modal($p, $cw - [int](120 * $scale), [int]($ch * 0.20), $cw - [int](40 * $scale), [int]($ch * 0.45), 0, 2)
    $bmp.Dispose()
    # A strip that is not the canvas colour belongs to an open panel.
    $o = -not (Near $l $c 2)
    $i = -not (Near $r $c 2)
    return @($o, $i)
}

$settled = $false
for ($round = 0; $round -lt 4; $round++) {
    $st = PanelState ("v5a-settle" + $round)
    $outlineOpen = $st[0]; $inspectorOpen = $st[1]
    Write-Host ("settle " + $round + " outline=" + $outlineOpen + " inspector=" + $inspectorOpen)
    if (-not $outlineOpen -and $inspectorOpen) { $settled = $true; break }
    if ($outlineOpen) { Press $VK_F8 }
    if (-not $inspectorOpen) { Press $VK_F9 }
}
if (-not $settled) { Write-Host "  FAIL the panels never reached outline closed / inspector open"; $fail++ }

Press $VK_HOME                 # fit, so the tree is where the band expects it
# The document was just opened, so nothing is selected and the inspector shows the
# document page - which is where the accent row lives. No click needed to get there.
Start-Sleep -Milliseconds 500

# --- the reference frames -------------------------------------------------------------
# Not "Measure": that is a built-in alias for Measure-Object, and aliases outrank
# functions in command resolution, so the call would silently return a Count=0 record.
function MeasureFrame($name) {
    $bmp = Shot $name
    $px = [V5A]::Grab($bmp)
    $bg = [int[]][V5A]::Modal($px, $bx0, $by0, $bx1, $by1, 0, 2)
    $fill = [int[]][V5A]::Modal($px, $bx0, $by0, $bx1, $by1, 60, 2)
    $rect = [int[]][V5A]::CardRect($px, $bx0, $by0, $bx1, $by1, $fill[0], $fill[1], $fill[2], 24)
    $ink = -1.0
    if ($rect[0] -ge 0) { $ink = [V5A]::CardInk($px, $rect[0], $rect[1], $rect[2], $rect[3], $fill[0], $fill[1], $fill[2]) }
    $box = [int[]][V5A]::InkBox($px, $bx0, $by0, $bx1, $by1, $bg[0], $bg[1], $bg[2], 30)
    Write-Host ("  " + $name + " bg=" + ($bg[0], $bg[1], $bg[2] -join ",") + " card=" + ($fill[0], $fill[1], $fill[2] -join ",") + " rect=" + ($rect -join "x") + " ink=" + [math]::Round($ink, 2) + ":1 inkbox=" + ($box -join ","))
    $bmp.Dispose()
    return [pscustomobject]@{ Px = $px; Bg = $bg; Fill = $fill; Rect = $rect; Ink = $ink; Box = $box }
}

$base = MeasureFrame "v5a-01-base"
if ($base.Rect[0] -lt 0) { Write-Host "  FAIL no first-level card found in the canvas band"; $fail++ }
if ($base.Ink -lt 3.0) { Write-Host "  FAIL the card label is unreadable before anything was touched"; $fail++ }

# --- find the hue chips ---------------------------------------------------------------
$px = $base.Px
$crow = [int[]][V5A]::BestChipRow($px, $PANEL_X0 + 4, $yBar, $cw - 4, $STATUS_Y1, 70, 16, 40)
$yBest = $crow[0]
$runs = [V5A]::Runs($px, $yBest, $PANEL_X0 + 4, $cw - 4, 70, 12)
$n = [int]($runs.Length / 2)
Write-Host ("chip row y=" + $yBest + " score=" + $crow[1] + " runs=" + $n)
for ($i = 0; $i -lt $n; $i++) {
    Write-Host ("  chip " + $i + " x=" + [math]::Round($runs[2 * $i], 1) + " w=" + $runs[2 * $i + 1])
}
if ($n -lt 6) { Write-Host "  FAIL the accent chip row was not found"; $fail++ }

$target = -1
if ($n -ge 6) {
    # The chip already nearest the live accent wears a 2pt ring at r=10pt. 9.5pt above
    # the row centre the 7.5pt disc and the 8.6pt hairline ring cannot reach, so any
    # saturated pixel up there belongs to that selection ring - and that is the one chip
    # whose override would barely move anything. Aim at the far end from it instead.
    $ringY = $yBest - [int](9.5 * $scale)
    $ring = [V5A]::Runs($px, $ringY, $PANEL_X0 + 4, $cw - 4, 70, 6)
    $ringX = -1.0
    if ($ring.Length -ge 2) {
        $sum = 0.0
        for ($i = 0; $i -lt [int]($ring.Length / 2); $i++) { $sum += $ring[2 * $i] }
        $ringX = $sum / ($ring.Length / 2)
    }
    Write-Host ("selection ring y=" + $ringY + " x=" + [math]::Round($ringX, 1))
    if ($ringX -gt 0) {
        $far = 0
        for ($i = 1; $i -lt $n; $i++) {
            if ([math]::Abs($runs[2 * $i] - $ringX) -gt [math]::Abs($runs[2 * $far] - $ringX)) { $far = $i }
        }
    } else {
        # No ring found: the leftmost chip is 0 degrees, the furthest any theme accent can
        # sit from a sampled hue without being the nearest one.
        $far = 0
    }
    $target = $far
    Write-Host ("  target chip " + $far + " x=" + [math]::Round($runs[2 * $far], 1))
}

if ($n -lt 6 -or $target -lt 0) {
    Write-Host "  FAIL the accent chip row was not found"; $fail++
} else {
    # --- F6: hand the keyboard to the panels -------------------------------------------
    # Gated on evidence, because the alternative is worse than a failed claim: while the
    # canvas holds the keyboard, Tab is "add a child topic". If F6 did not arrive and the
    # walk kept pressing Tab, the rig would edit the document it is about to measure and
    # then compare frames of its own damage.
    Press $VK_F6
    $tr = TraceLast "event "
    $b = Shot "v5a-02-f6"
    $f6 = [V5A]::Grab($b)
    $b.Dispose()
    $hint = [V5A]::Diff($f6, $base.Px, $SX0, $SY0, $cw, $ch, 0)
    $moved = [V5A]::Diff($f6, $base.Px, 0, $yBar, $cw, $STATUS_Y1, 0)
    $fbox = [int[]][V5A]::DiffBox($f6, $base.Px, 0, $yBar, $cw, $STATUS_Y1, 0)
    Write-Host ("F6: '" + $tr + "' status strip repainted " + $hint + " px; document body " + $moved + " px")
    $walk = $false
    if ($hint -lt 30) {
        Write-Host "  FAIL F6 never reached the app - the keyboard indicator was never painted"
        Write-Host "  Stopping before the Tab walk (see above: Tab would edit the document)"
        $fail++
    } elseif ($moved -ne 0) {
        Write-Host ("  FAIL handing over the keyboard repainted the document, box=" + ($fbox -join ","))
        $fail++
    } else {
        Write-Host "  F6 OK: the keyboard is in the panels and the document did not move"
        $walk = $true
    }

    if ($walk) {
        # --- apply a hue the way a keyboard-only user does -----------------------------
        if (-not (TabTo ("accent-chip " + $target + " "))) {
            Write-Host ("  FAIL Tab never landed on chip " + $target); $fail++
        } else {
            $markSpin = [DateTime]::Now
            Press $VK_SPACE
            Start-Sleep -Milliseconds 400
            $spun = MeasureFrame "v5a-03-spun"

            # Applying a panel control goes through the same path that hands the keyboard
            # back to the canvas after an edit - which is right for a slider and wrong in
            # the middle of a colour row. The indicator says which one happened.
            # Not a pixel diff though: that label is painted in the *accent* colour, so
            # applying a hue repaints every glyph of it without the keyboard moving at
            # all. Coverage of the strip is what answers "is the label still there".
            $sbg = [int[]][V5A]::Modal($f6, $SX0, $SY0, $cw, $ch, 0, 2)
            $cov1 = [int[]][V5A]::InkBox($f6, $SX0, $SY0, $cw, $ch, $sbg[0], $sbg[1], $sbg[2], 40)
            $cov2 = [int[]][V5A]::InkBox($spun.Px, $SX0, $SY0, $cw, $ch, $sbg[0], $sbg[1], $sbg[2], 40)
            $lost = $cov1[4] - $cov2[4]
            Write-Host ("after the pick, strip ink " + $cov1[4] + " -> " + $cov2[4] + " px; mode line '" + (TraceLast "event ") + "'")
            if ($lost -gt 200) { Write-Host "  FAIL applying a chip yanked the keyboard back to the canvas; the row is one-press-then-lose"; $fail++ }

            $d = [math]::Abs($spun.Fill[0] - $base.Fill[0]) + [math]::Abs($spun.Fill[1] - $base.Fill[1]) + [math]::Abs($spun.Fill[2] - $base.Fill[2])
            Write-Host ("card colour moved by " + $d)
            if ($d -lt 30) { Write-Host "  FAIL applying the chip did not repaint the cards"; $fail++ }
            if (-not (Near $spun.Bg $base.Bg 2)) { Write-Host "  FAIL the canvas background moved: an accent override must not touch neutrals"; $fail++ }
            $dC = [math]::Abs($spun.Ink - $base.Ink)
            Write-Host ("card ink contrast " + [math]::Round($base.Ink, 2) + ":1 -> " + [math]::Round($spun.Ink, 2) + ":1 (delta " + [math]::Round($dC, 2) + ")")
            if ($spun.Ink -lt 3.0) { Write-Host "  FAIL the card label fell below 3:1 after the override"; $fail++ }
            if ($dC -gt 1.5) { Write-Host "  FAIL the same card slot lost more than 1.5 points of contrast"; $fail++ }
            # Only the box, never the pixel count: recolouring moves antialiased edge pixels
            # across the "is this ink" threshold, and that is exactly what must stay allowed.
            if (($spun.Box[0..3] -join ",") -ne ($base.Box[0..3] -join ",")) { Write-Host "  FAIL the camera moved: ink bounding box changed"; $fail++ }
            else { Write-Host ("ink box held at " + ($base.Box[0..3] -join ",")) }

            # --- persistence, dirty half: the recovery point is the container -----------
            # No Ctrl+S: the interval writer uses the same mw_format::save, so what it
            # puts in .autosave/ is what a manual save would put in the document.
            $tSpin = WaitSlot $markSpin
            if ($tSpin -eq "") { Write-Host "  FAIL no recovery point appeared within 8s of the override"; $fail++ }
            elseif (-not $tSpin.Contains('"themeAccent"')) {
                Write-Host "  FAIL the recovery point has no themeAccent - the override never reached the document"
                $fail++
            } else { Write-Host "recovery point carries themeAccent" }

            # --- undo / redo: one step each, landing on the two known frames --------------
            # Two routes to the same claim. Ctrl+Z is the one a user takes when the chord
            # actually reaches the app; on this desktop it does not (see preflight), so the
            # rig walks the Tab ring to the toolbar's own undo/redo buttons and presses
            # Space - the same command through the same widget, only without the modifier.
            # Scoped to the canvas band, not the whole body: the widget under the keyboard
            # wears a focus ring here and nothing in the base frame, and that ring is not
            # what this claim is about.
            $du = -1
            $dr = -1
            $route = ""
            if ($chords) {
                $route = "chord"
                Ctrl $VK_Z
                $b = Shot "v5a-04-undo"
                $upx = [V5A]::Grab($b)
                $du = [V5A]::Diff($upx, $base.Px, $bx0, $by0, $bx1, $by1, 0)
                $b.Dispose()
                CtrlShift $VK_Z
                $b = Shot "v5a-05-redo"
                $rpx = [V5A]::Grab($b)
                $dr = [V5A]::Diff($rpx, $spun.Px, $bx0, $by0, $bx1, $by1, 0)
                $b.Dispose()
            } else {
                $route = "toolbar"
                if (TabTo "tool-undo") {
                    Press $VK_SPACE
                    Start-Sleep -Milliseconds 450
                    $b = Shot "v5a-04-undo"
                    $upx = [V5A]::Grab($b)
                    $b.Dispose()
                    $du = [V5A]::Diff($upx, $base.Px, $bx0, $by0, $bx1, $by1, 0)
                    if ($du -ne 0) {
                        Write-Host ("  undo frame still differs, box=" + (([int[]][V5A]::DiffBox($upx, $base.Px, $bx0, $by0, $bx1, $by1, 0)) -join ","))
                    }
                    $upx = $null
                    $markRedo = [DateTime]::Now
                    if (TabTo "tool-redo") {
                        Press $VK_SPACE
                        Start-Sleep -Milliseconds 450
                        $b = Shot "v5a-05-redo"
                        $rpx = [V5A]::Grab($b)
                        $b.Dispose()
                        $dr = [V5A]::Diff($rpx, $spun.Px, $bx0, $by0, $bx1, $by1, 0)
                        if ($dr -ne 0) {
                            Write-Host ("  redo frame still differs, box=" + (([int[]][V5A]::DiffBox($rpx, $spun.Px, $bx0, $by0, $bx1, $by1, 0)) -join ","))
                        }
                        $rpx = $null
                        # Drain the interval writer before the next step reads the slot:
                        # the redo bumps the revision, so a write from *this* revision could
                        # otherwise land after the reset mark and be mistaken for the reset.
                        $tRedo = WaitSlot $markRedo
                        if ($tRedo -eq "") { Write-Host "  note no recovery point landed after the redo (the reset read below is still unambiguous)" }
                        elseif (-not $tRedo.Contains('"themeAccent"')) {
                            Write-Host "  FAIL the recovery point after the redo lost themeAccent - redo did not restore the override"
                            $fail++
                        } else { Write-Host "redo reaches the container (recovery point carries themeAccent again)" }
                    } else { Write-Host "  BLOCKED the redo half: Tab never reached the toolbar redo button"; $blocked++ }
                } else { Write-Host "  BLOCKED undo/redo: Tab never reached the toolbar undo button"; $blocked++ }
            }
            if ($du -ge 0) {
                Write-Host ("undo vs base canvas band diff=" + $du + " (route: " + $route + ")")
                if ($du -ne 0) { Write-Host "  FAIL undo did not land back on the pre-override frame"; $fail++ }
            }
            if ($dr -ge 0) {
                Write-Host ("redo vs spun canvas band diff=" + $dr + " (route: " + $route + ")")
                if ($dr -ne 0) { Write-Host "  FAIL redo did not restore the override frame exactly"; $fail++ }
            }

            # --- the reset button -------------------------------------------------------
            # Its label is still located by pixels: the trace says the app considers the
            # button focused, and this proves the thing under that focus is a drawn CJK
            # label in the band below the discs (28pt pitch + 11pt half disc + 4pt gap +
            # half a 24pt row).
            $bandY0 = $yBest + [int](36 * $scale)
            $bandY1 = $yBest + [int](74 * $scale)
            $labels = [V5A]::Label($spun.Px, $PANEL_X0 + 4, $bandY0, $cw - 4, $bandY1, 0.35, 40, 30)
            $ln = [int]($labels.Length / 3)
            Write-Host ("reset band y=" + $bandY0 + ".." + $bandY1 + " labels=" + $ln)
            if ($ln -lt 1) { Write-Host "  note no dark CJK label found in the expected band (panel geometry moved?)" }
            for ($i = 0; $i -lt $ln; $i++) {
                Write-Host ("  label " + $i + " x=" + [math]::Round($labels[3 * $i], 1) + " yc=" + [math]::Round($labels[3 * $i + 1], 1) + " w=" + $labels[3 * $i + 2])
            }
            if (-not (TabTo "accent-reset")) {
                Write-Host "  FAIL Tab never landed on the theme-default button"; $fail++
            } else {
                $markReset = [DateTime]::Now
                Press $VK_SPACE
                # F6 again: take the keyboard back. Two things get proved with one frame -
                # the indicator goes away, and the panel's focus ring does too, so the
                # comparison against the base frame is not contaminated by either.
                Press $VK_F6
                $b = Shot "v5a-06-reset"
                $rpx2 = [V5A]::Grab($b)
                $b.Dispose()
                $dres = [V5A]::Diff($rpx2, $base.Px, 0, $yBar, $cw, $STATUS_Y1, 0)
                $bres = [int[]][V5A]::DiffBox($rpx2, $base.Px, 0, $yBar, $cw, $STATUS_Y1, 0)
                Write-Host ("reset vs base body diff=" + $dres + " box=" + ($bres -join ","))
                if ($dres -ne 0) { Write-Host "  FAIL clearing the override did not land back on the original frame"; $fail++ }
                $gone = [V5A]::Diff($rpx2, $base.Px, $SX0, $SY0, $cw, $ch, 0)
                Write-Host ("second F6: status strip moved " + $gone + " px")
                if ($gone -ne 0) { Write-Host "  FAIL F6 did not hand the keyboard back to the canvas"; $fail++ }

                # --- persistence, cleared half ------------------------------------------
                $tReset = WaitSlot $markReset
                if ($tReset -eq "") { Write-Host "  FAIL no recovery point was written after the reset"; $fail++ }
                elseif ($tReset.Contains('"themeAccent"')) {
                    Write-Host "  FAIL the recovery point still carries themeAccent after 'theme default'"
                    $fail++
                } else { Write-Host "recovery point drops themeAccent - clearing the override reaches the container too" }

                # --- a panel toggle must not silence the canvas ----------------------------
                # This is the reason F6 works at all. The canvas region used to be registered
                # under an id the layout handed out: opening the outline moves the central Ui,
                # the id changes, egui's dead-man switch clears the focus, and every bare
                # canvas key (Tab = add a child topic) silently stops working until the user
                # clicks the canvas again. Two halves of one claim, and this rig can only see
                # the first of them.
                Press $VK_F8
                Start-Sleep -Milliseconds 800
                $who = TraceLast "focus "
                Write-Host ("after F8 the keyboard sits on: '" + $who + "'")
                if ($who -ne "focus canvas") {
                    Write-Host "  FAIL the panel toggle took the keyboard away from the canvas"
                    $fail++
                } else { Write-Host "  OK: the canvas kept the keyboard across the panel toggle" }

                # The other half - the document actually reacting - needs the window to be the
                # foreground one: egui's `Response::has_focus` (which gates the canvas
                # shortcuts) asks `input.focused` as well as the memory layer, and here the
                # foreground belongs to another process. Sent keys still arrive, but the app
                # is right to refuse them; that is not a defect, so it must not be counted as
                # one either.
                if ((TraceLast "focus ") -ne "focus canvas") {
                    Write-Host "  BLOCKED the Tab half too: without the canvas keyboard there is nothing to test."
                    $blocked++
                } elseif (-not ((Is-Foreground))) {
                    Write-Host "  BLOCKED Tab reaching the document: this window is not the foreground one, and egui only lets a widget act on keys when its window is. Rig limitation, not a product defect."
                    $blocked++
                } else {
                    $b = Shot "v5a-07-settling"; $ping = [V5A]::Grab($b); $b.Dispose()
                    $b = Shot "v5a-08-before-tab"; $p8 = [V5A]::Grab($b); $b.Dispose()
                    $drift = [V5A]::Diff($p8, $ping, $nx0, $by0, $nx1, $by1, 0)
                    Write-Host ("outline open, canvas band drift between two frames = " + $drift + " px")
                    if ($drift -ne 0) {
                        Write-Host "  BLOCKED this claim: the view is still animating, so a diff here proves nothing."
                        $blocked++
                    } else {
                        Press $VK_TAB
                        Start-Sleep -Milliseconds 700
                        $b = Shot "v5a-09-after-tab"; $pt = [V5A]::Grab($b); $b.Dispose()
                        $grew = [V5A]::Diff($pt, $p8, $nx0, $by0, $nx1, $by1, 0)
                        Write-Host ("Tab after the toggle repainted the document over " + $grew + " px")
                        if ($grew -lt 30) {
                            Write-Host "  FAIL Tab never reached the document, although the trail says the canvas held the keyboard"
                            $fail++
                        } else { Write-Host "  Tab OK: the keystroke landed on the document after the panel toggle" }
                    }
                    $ping = $null; $p8 = $null; $pt = $null
                }
            }
        }
    }
    $f6 = $null; $spun = $null
}

Start-Sleep -Milliseconds 300
[void][V5A]::SetWindowPos($hwnd, [IntPtr](-2), 0, 0, 0, 0, (0x0002 -bor 0x0001))
Stop-Process -Id $proc.Id -Force
Start-Sleep -Milliseconds 400
Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
if ($fail -gt 0) { Write-Host ("ACCENT FAIL (" + $fail + " failed, " + $blocked + " blocked)"); exit 1 }
if ($blind -gt 0) {
    Write-Host ("ACCENT INCONCLUSIVE: " + $blind + " frame(s) were captured through another window - the desktop, not the app, moved in front. Re-run, or close what popped up.")
    exit 3
}
if ($blocked -gt 0) {
    Write-Host ("ACCENT PARTLY VERIFIED: " + $blocked + " claim(s) blocked by this desktop's input lock, 0 failed")
    exit 2
}
Write-Host "ACCENT OK"
