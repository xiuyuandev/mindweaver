param(
    [string]$Exe = "target/release/mw-shell.exe",
    [string]$ShotDir = "shots/qa"
)

# v5 theme-entry probe (real input, release build).
#
# Two claims:
#   1. Ctrl+K -> paste a theme name -> Enter repaints the whole UI in that theme, and
#      switching back lands on a pixel-identical frame (the round trip loses nothing).
#   2. The hamburger menu carries a theme section whose tick follows the current theme,
#      and clicking a row there switches the document exactly like the palette does.
#   3. The palette's selected row - the only keyboard-navigation cue - stays legible on
#      its own fill in both the light and the dark appearance.
#
# The tick is the only accent-coloured ink in the menu icon column, so it is found by
# saturation instead of measured geometry; "two rows below the tick" is then the dark
# theme row (rows are 30pt + 1pt spacing).
#
# ASCII only (PowerShell 5.1 reads BOM-less files as ANSI); CJK strings are built from
# char codes.

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.IO.Compression.FileSystem
Add-Type -ReferencedAssemblies System.Drawing -Language CSharp @'
using System;
using System.Drawing;
using System.Runtime.InteropServices;
public static class V5 {
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

    public static int Diff(Bitmap a, Bitmap b, int x0, int y0, int x1, int y1, int tol) {
        int n = 0;
        for (int y = y0; y < y1; y++) {
            for (int x = x0; x < x1; x++) {
                Color pa = a.GetPixel(x, y), pb = b.GetPixel(x, y);
                if (Math.Abs(pa.R - pb.R) > tol || Math.Abs(pa.G - pb.G) > tol || Math.Abs(pa.B - pb.B) > tol) n++;
            }
        }
        return n;
    }

    /// Modal colour of a band: the canvas background is by far the most common pixel.
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

    static int Sat(Color c) {
        int hi = c.R > c.G ? c.R : c.G; if (c.B > hi) hi = c.B;
        int lo = c.R < c.G ? c.R : c.G; if (c.B < lo) lo = c.B;
        return hi - lo;
    }

    /// Vertical centroids of saturated-ink clusters inside a column band. In the menu
    /// that is the accent tick; every icon and label around it is greyscale.
    public static double[] Ticks(Bitmap bmp, int x0, int y0, int x1, int y1, int minSat, int minPx) {
        int[] cnt = new int[y1 - y0];
        for (int y = y0; y < y1; y++) {
            int c = 0;
            for (int x = x0; x < x1; x++) {
                if (Sat(bmp.GetPixel(x, y)) >= minSat) c++;
            }
            cnt[y - y0] = c;
        }
        var outp = new System.Collections.Generic.List<double>();
        int start = -1, prev = -1; double sum = 0.0; int px = 0;
        for (int i = 0; i < cnt.Length; i++) {
            if (cnt[i] > 0) {
                if (px == 0) start = i;
                else if (i - prev > 2) { Push(outp, y0, start, prev, sum, px, minPx); start = i; sum = 0.0; px = 0; }
                sum += (double)(y0 + i) * cnt[i]; px += cnt[i]; prev = i;
            }
        }
        if (px > 0) Push(outp, y0, start, prev, sum, px, minPx);
        return outp.ToArray();
    }

    static void Push(System.Collections.Generic.List<double> outp, int y0, int start, int prev, double sum, int px, int minPx) {
        if (px < minPx) return;
        outp.Add(sum / px);
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

    static bool RowLike(Bitmap bmp, int y, int x0, int x1, int br, int bg, int bb) {
        int hit = 0, n = 0;
        for (int x = x0; x < x1; x += 2) {
            Color c = bmp.GetPixel(x, y);
            n++;
            if (Math.Abs(c.R - br) <= 2 && Math.Abs(c.G - bg) <= 2 && Math.Abs(c.B - bb) <= 2) hit++;
        }
        return hit * 100 >= n * 55;
    }

    /// WCAG contrast of the strongest ink inside the popup's chosen row. The row is the
    /// only band painted with the theme's accent_soft token, so it is found by colour
    /// instead of measured geometry. This is what caught the translucent selected-row ink:
    /// deriving it as accent * 0.45 scaled the alpha too, so the label blended into its
    /// own background and read 1.4:1 - the one row that must stand out stood out least.
    public static double RowInk(Bitmap bmp, int y0, int y1, int x0, int x1, int br, int bg, int bb) {
        int bestTop = -1, bestLen = 0, top = -1;
        for (int y = y0; y <= y1; y++) {
            bool on = y < y1 && RowLike(bmp, y, x0, x1, br, bg, bb);
            if (on) { if (top < 0) top = y; }
            else if (top >= 0) {
                if (y - top > bestLen) { bestLen = y - top; bestTop = top; }
                top = -1;
            }
        }
        if (bestLen < 8) return -1.0;
        double bandL = Lum(br, bg, bb), inkL = bandL;
        int dist = 0;
        for (int y = bestTop; y < bestTop + bestLen; y++) {
            for (int x = x0; x < x1; x++) {
                Color c = bmp.GetPixel(x, y);
                int d = Math.Abs(c.R - br) + Math.Abs(c.G - bg) + Math.Abs(c.B - bb);
                if (d > dist) { dist = d; inkL = Lum(c.R, c.G, c.B); }
            }
        }
        return dist == 0 ? -1.0 : Ratio(inkL, bandL);
    }
}
'@
[void][V5]::SetProcessDPIAware()

$tmp = Join-Path $env:TEMP ("mw-v5-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
$env:APPDATA = $tmp
if (-not (Test-Path -LiteralPath $ShotDir)) { New-Item -ItemType Directory -Path $ShotDir | Out-Null }
$shotFull = (Resolve-Path -LiteralPath $ShotDir).Path
$encNoU8 = New-Object System.Text.UTF8Encoding $false

function Cj([int[]]$codes) {
    $s = ""
    foreach ($c in $codes) { $s += [string]([char]$c) }
    return $s
}
# 织思 / 配色 / 交互 / 字体 - four nodes, so branch colours and card fills are both on
# screen while the theme changes.
$rootT = Cj @(0x7EC7, 0x601D)
$kidA = Cj @(0x914D, 0x8272)
$kidB = Cj @(0x4EA4, 0x4E92)
$kidC = Cj @(0x5B57, 0x4F53)
# 主题：墨夜 / 主题：素白 - fed to the palette through the clipboard, because SendKeys
# cannot produce CJK without an IME.
$qInk = (Cj @(0x4E3B, 0x9898, 0xFF1A, 0x58A8, 0x591C))
$qSnow = (Cj @(0x4E3B, 0x9898, 0xFF1A, 0x7D20, 0x767D))

function Node([string]$id, [string]$parent, [string]$text, [string]$kids) {
    return '"' + $id + '":{"id":"' + $id + '","parentId":' + $parent + ',"children":[' + $kids + '],"text":"' + $text + '","collapsed":false,"modifiedAt":0}'
}
$nodes = @(
    (Node "n0" "null" $rootT '"n1","n2","n3"'),
    (Node "n1" '"n0"' $kidA ""),
    (Node "n2" '"n0"' $kidB ""),
    (Node "n3" '"n0"' $kidC "")
)
$content = '{"schema":1,"rootId":"n0","nodes":{' + ($nodes -join ",") + '},"theme":"snow","layout":{"kind":"mindmap","siblingGap":24,"levelGap":80,"maxNodeWidth":300}}'
$manifest = '{"formatVersion":1,"app":"probe/1.0","docId":"probe-v5","title":"probe","createdAt":0,"modifiedAt":0,"nodeCount":4}'
$dir = Join-Path $tmp "doc"
[void](New-Item -ItemType Directory -Force -Path $dir)
[System.IO.File]::WriteAllText((Join-Path $dir "content.json"), $content, $encNoU8)
[System.IO.File]::WriteAllText((Join-Path $dir "manifest.json"), $manifest, $encNoU8)
[System.IO.File]::WriteAllText((Join-Path $dir "mimetype"), "application/vnd.mindweaver.zmind", $encNoU8)
$fx = Join-Path $tmp "v5.zmind"
[System.IO.Compression.ZipFile]::CreateFromDirectory($dir, $fx)

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
        $r = New-Object V5+RECT
        [void][V5]::GetClientRect($h, [ref]$r)
        if ($r.Right -ge 600 -and $r.Bottom -ge 400) { $hwnd = $h; break }
    }
}
if ($hwnd -eq [IntPtr]::Zero) { Write-Error "main window never appeared"; exit 1 }
[void][V5]::ShowWindow($hwnd, 9)
[void][V5]::BringWindowToTop($hwnd)
try { (New-Object -ComObject WScript.Shell).AppActivate($proc.Id) | Out-Null } catch { }
[void][V5]::SetForegroundWindow($hwnd)
Start-Sleep -Milliseconds 1000

$rect = New-Object V5+RECT
[void][V5]::GetClientRect($hwnd, [ref]$rect)
$origin = New-Object V5+P
[void][V5]::ClientToScreen($hwnd, [ref]$origin)
$cw = $rect.Right
$ch = $rect.Bottom
$scale = [V5]::GetDpiForWindow($hwnd) / 96.0
Write-Host ("client=" + $cw + "x" + $ch + " scale=" + $scale)
if ($scale -lt 1.2) { Write-Warning "menu rows are measured in logical points; 100% DPI makes them 1-2px thin" }

function Shot([string]$name) {
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
function Press([byte]$vk) {
    [V5]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 50
    [V5]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 420
}
function Ctrl([byte]$vk) {
    [V5]::keybd_event(0x11, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 40
    [V5]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 50
    [V5]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 40
    [V5]::keybd_event(0x11, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 420
}
function ClickPt([single]$lx, [single]$ly) {
    $x = $origin.X + [int]($lx * $scale)
    $y = $origin.Y + [int]($ly * $scale)
    [void][V5]::SetCursorPos($x, $y)
    Start-Sleep -Milliseconds 140
    [V5]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [V5]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 420
}
# Every full-frame diff parks the pointer first: egui re-tests hover each frame from the
# last pointer position, so an un-parked cursor is a diff waiting to happen.
$PARK_X = [int]($cw * 0.5)
$PARK_Y = [int]($ch * 0.75)
function Park() {
    [void][V5]::SetCursorPos($origin.X + $PARK_X, $origin.Y + $PARK_Y)
    Start-Sleep -Milliseconds 260
}

# The menu icon column, logical points: card left 10 + inner margin 6 + icon centre 17.
$TICK_X0 = [int](24 * $scale)
$TICK_X1 = [int](44 * $scale)
$MENU_Y0 = [int](60 * $scale)
$ROWS = [int](31.0 * $scale)
# The status bar is cut out of every measurement: switching the theme dirties the
# document, and "已保存" -> "● 未保存" (plus the hairline above it) is a legitimate
# change that has nothing to do with the palette.
$STATUS_Y1 = $ch - [int](40 * $scale)
# Canvas band, clear of panels, title bar, toolbar pill and minimap.
$bandX0 = [int]($cw * 0.40); $bandX1 = [int]($cw * 0.72)
$bandY0 = [int]($ch * 0.25); $bandY1 = [int]($ch * 0.70)
$yBar = [int](58 * $scale)

$fail = 0
$VK_HOME = 0x24; $VK_K = 0x4B; $VK_V = 0x56; $VK_ENTER = 0x0D; $VK_ESC = 0x1B
$VK_F8 = 0x77; $VK_F9 = 0x78
# canvas_bg tokens: snow #FBFBFC, inkNight #17191F
$SNOW_BG = @(251, 251, 252)
$INK_BG = @(23, 25, 31)
# accent_soft tokens (the chosen-row fill): snow #EAF2FF, inkNight #232A44
$SNOW_SOFT = @(234, 242, 255)
$INK_SOFT = @(35, 42, 68)
$PAL_X0 = [int]($cw * 0.25)
$PAL_X1 = [int]($cw * 0.75)

function RowInk($bmp, [int[]]$token) {
    return [V5]::RowInk($bmp, $yBar, $STATUS_Y1, $PAL_X0, $PAL_X1, $token[0], $token[1], $token[2])
}

function Near([int[]]$a, [int[]]$b, [int]$tol) {
    return ([math]::Abs($a[0] - $b[0]) -le $tol) -and ([math]::Abs($a[1] - $b[1]) -le $tol) -and ([math]::Abs($a[2] - $b[2]) -le $tol)
}

# Both panels closed: a checked outline/inspector row would add a second accent tick,
# and the side strips must read as bare canvas for the modal sampling. Calibrated
# against the canvas band of the same frame, not against the theme token, because the
# panel white and the canvas near-white differ by less than 5.
function Far([int[]]$a, [int[]]$b) {
    return ([math]::Abs($a[0] - $b[0]) -gt 2) -or ([math]::Abs($a[1] - $b[1]) -gt 2) -or ([math]::Abs($a[2] - $b[2]) -gt 2)
}
Park
$p0 = Shot "v5-start"
$cen0 = [int[]][V5]::Modal($p0, $bandX0, $bandY0, $bandX1, $bandY1)
$left = [int[]][V5]::Modal($p0, [int](100 * $scale), $bandY0, [int](200 * $scale), $bandY1)
$right = [int[]][V5]::Modal($p0, $cw - [int](120 * $scale), [int]($ch * 0.20), $cw - [int](40 * $scale), [int]($ch * 0.45))
$p0.Dispose()
Write-Host ("canvas=" + ($cen0 -join ",") + " left-strip=" + ($left -join ",") + " right-strip=" + ($right -join ","))
if (Far $left $cen0) { Press $VK_F8 }
if (Far $right $cen0) { Press $VK_F9 }
Start-Sleep -Milliseconds 700

Press $VK_HOME
ClickPt ([single]($PARK_X / $scale)) ([single]($PARK_Y / $scale))
Press $VK_HOME
Park
$snow = Shot "v5-snow"
$bg0 = [int[]][V5]::Modal($snow, $bandX0, $bandY0, $bandX1, $bandY1)
Write-Host ("snow canvas_bg=" + ($bg0 -join ","))
if (-not (Near $bg0 $SNOW_BG 8)) { Write-Host "  FAIL snow canvas background is not the snow token"; $fail++ }
$check = Shot "v5-ready"
$check.Dispose()

# --- palette path ---
Ctrl $VK_K
Start-Sleep -Milliseconds 300
Set-Clipboard -Value $qInk
Ctrl $VK_V
Park
Start-Sleep -Milliseconds 300
$pal = Shot "v5-palette"
$rc = RowInk $pal $SNOW_SOFT
Write-Host ("palette snow chosen-row contrast=" + [math]::Round($rc, 2) + ":1")
if ($rc -lt 4.5) { Write-Host "  FAIL the selected row's label is not legible on its own fill"; $fail++ }
$pal.Dispose()
Press $VK_ENTER
Start-Sleep -Milliseconds 900
Park
$ink1 = Shot "v5-palette-inknight"
$bg1 = [int[]][V5]::Modal($ink1, $bandX0, $bandY0, $bandX1, $bandY1)
Write-Host ("palette inkNight canvas_bg=" + ($bg1 -join ","))
if (-not (Near $bg1 $INK_BG 8)) { Write-Host "  FAIL the palette did not repaint the canvas in inkNight"; $fail++ }

# The same row, now on the dark fill: the ink is derived from the accent in both
# appearances, so one legible side and one washed-out side is exactly what a
# "brighten it in dark mode" shortcut produces.
Ctrl $VK_K
Start-Sleep -Milliseconds 300
Set-Clipboard -Value $qInk
Ctrl $VK_V
Park
$palDark = Shot "v5-palette-inknight-open"
$rcd = RowInk $palDark $INK_SOFT
Write-Host ("palette inkNight chosen-row contrast=" + [math]::Round($rcd, 2) + ":1")
if ($rcd -lt 4.5) { Write-Host "  FAIL the selected row's label is not legible on its own fill"; $fail++ }
$palDark.Dispose()
Press $VK_ESC
Start-Sleep -Milliseconds 300

Ctrl $VK_K
Start-Sleep -Milliseconds 300
Set-Clipboard -Value $qSnow
Ctrl $VK_V
Press $VK_ENTER
Start-Sleep -Milliseconds 900
Park
$snow2 = Shot "v5-snow-again"
# Below the title bar only: the first theme switch marks the document dirty, and the
# "saved / edited" label in the title bar is supposed to change.
$all = [V5]::Diff($snow, $snow2, 0, 0, $cw, $ch, 0)
$body = [V5]::Diff($snow, $snow2, 0, $yBar, $cw, $STATUS_Y1, 0)
Write-Host ("roundtrip_diff_body=" + $body + " whole=" + $all + "/" + ($cw * ($STATUS_Y1 - $yBar)))
if ($body -ne 0) { Write-Host "  FAIL the theme round trip is not pixel-identical outside the chrome strips"; $fail++ }

# --- menu path ---
ClickPt ([single]27.0) ([single]28.0)
$menuA = Shot "v5-menu-snow"
$tickA = [V5]::Ticks($menuA, $TICK_X0, $MENU_Y0, $TICK_X1, $STATUS_Y1, 60, 6)
Write-Host ("menu ticks=" + ($tickA -join ",") + " rowPitch=" + $ROWS)
if ($tickA.Length -ne 1) {
    Write-Host "  FAIL expected exactly one accent tick in the menu icon column"
    $fail++
    $menuA.Dispose()
} else {
    $menuA.Dispose()
    ClickPt ([single]120.0) ([single](($tickA[0] + 2 * $ROWS) / $scale))
    Start-Sleep -Milliseconds 900
    Park
    $ink2 = Shot "v5-menu-inknight"
    $bg2 = [int[]][V5]::Modal($ink2, $bandX0, $bandY0, $bandX1, $bandY1)
    Write-Host ("menu-click inkNight canvas_bg=" + ($bg2 -join ","))
    if (-not (Near $bg2 $INK_BG 8)) { Write-Host "  FAIL clicking the menu theme row did not switch the theme"; $fail++ }
    $same = [V5]::Diff($ink1, $ink2, 0, 0, $cw, $ch, 0)
    Write-Host ("palette_vs_menu_diff=" + $same)
    if ($same -ne 0) { Write-Host "  FAIL the two entry points disagree"; $fail++ }
    $ink1.Dispose(); $ink2.Dispose()

    ClickPt ([single]27.0) ([single]28.0)
    $menuB = Shot "v5-menu-inknight-tick"
    $tickB = [V5]::Ticks($menuB, $TICK_X0, $MENU_Y0, $TICK_X1, $STATUS_Y1, 60, 6)
    Write-Host ("menu ticks after switch=" + ($tickB -join ","))
    if ($tickB.Length -ne 1) {
        Write-Host "  FAIL the tick must follow the current theme, exactly once"
        $fail++
    } else {
        $moved = [math]::Abs($tickB[0] - $tickA[0] - (2 * $ROWS))
        Write-Host ("tick_move_err=" + [math]::Round($moved, 2) + "px")
        if ($moved -gt 4.0) { Write-Host "  FAIL the tick did not move down two rows"; $fail++ }
    }
    $menuB.Dispose()
    Press $VK_ESC
}
$snow.Dispose(); $snow2.Dispose()

[void][V5]::SetWindowPos($hwnd, [IntPtr](-2), 0, 0, 0, 0, (0x0002 -bor 0x0001))
Stop-Process -Id $proc.Id -Force
Start-Sleep -Milliseconds 400
Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
if ($fail -gt 0) { Write-Host ("THEME_ENTRY FAIL (" + $fail + ")"); exit 1 }
Write-Host "THEME_ENTRY OK"
