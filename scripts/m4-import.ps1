param(
    [string]$Exe = 'target/release/mw-shell.exe',
    [string]$ShotDir = 'shots/qa',
    [string]$ImportFile = 'shots/qa/import-test.md',
    # Existing .zmind to keep open in tab 1; proves import does not replace it.
    [string]$OpenDoc = '',
    # If set, Ctrl+S the imported doc and type this fresh path into the native save dialog.
    [string]$SaveAs = '',
    [switch]$KeepOpen
)

# M4 import probe: real input (title menu / Ctrl+K palette / native open dialog).
# Pure ASCII on purpose: PowerShell 5.1 reads a BOM-less file as the ANSI codepage and
# would mangle any literal CJK (docs/14 lesson 37). The palette query is built from
# code points instead.

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

if (-not ('C' -as [type])) {
    Add-Type -Language CSharp @'
using System;
using System.Runtime.InteropServices;
public static class C {
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out Rect r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref P p);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr SetFocus(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint d, int e, int dw, IntPtr r);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint f, IntPtr extra);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr h, System.Text.StringBuilder s, int n);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, System.Text.StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out Rect r);
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    public static string[] TopLevelFor(int pid) {
        System.Collections.Generic.List<string> list = new System.Collections.Generic.List<string>();
        EnumWindows(delegate(IntPtr h, IntPtr l) {
            uint wp;
            GetWindowThreadProcessId(h, out wp);
            if ((int)wp != pid || !IsWindowVisible(h)) { return true; }
            System.Text.StringBuilder cls = new System.Text.StringBuilder(256);
            GetClassName(h, cls, 256);
            System.Text.StringBuilder t = new System.Text.StringBuilder(512);
            GetWindowTextW(h, t, 512);
            Rect r;
            GetWindowRect(h, out r);
            list.Add(string.Format("hwnd={0} class='{1}' rect={2},{3},{4},{5} title='{6}'",
                h, cls.ToString(), r.Left, r.Top, r.Right, r.Bottom, t.ToString()));
            return true;
        }, IntPtr.Zero);
        return list.ToArray();
    }
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct P { public int X, Y; }
}
'@
}
[void][C]::SetProcessDPIAware()

function Resolve-Exe([string]$p) {
    if ([System.IO.Path]::IsPathRooted($p)) { return $p }
    return (Join-Path (Get-Location).Path $p)
}

$exePath = Resolve-Exe $Exe
if (-not (Test-Path -LiteralPath $exePath)) { Write-Error "missing exe: $exePath"; exit 1 }
$importPath = Join-Path (Get-Location).Path $ImportFile
if (-not (Test-Path -LiteralPath $importPath)) { Write-Error "missing import file: $importPath"; exit 1 }
if (-not (Test-Path -LiteralPath $ShotDir)) { New-Item -ItemType Directory -Path $ShotDir | Out-Null }
if ($OpenDoc -ne '') {
    $openPath = if ([System.IO.Path]::IsPathRooted($OpenDoc)) { $OpenDoc } else { Join-Path (Get-Location).Path $OpenDoc }
    $env:MW_OPEN = $openPath
}

$proc = Start-Process -FilePath $exePath -PassThru
Write-Host "launched pid=$($proc.Id)"

$hwnd = [IntPtr]::Zero
for ($i = 0; $i -lt 80; $i++) {
    Start-Sleep -Milliseconds 250
    $proc.Refresh()
    if ($proc.HasExited) { Write-Error "app exited early (code $($proc.ExitCode))"; exit 1 }
    $h = $proc.MainWindowHandle
    if ($h -eq [IntPtr]::Zero) { continue }
    $r = New-Object C+Rect
    if (-not [C]::GetClientRect($h, [ref]$r)) { continue }
    if ($r.Right -le 0 -or $r.Bottom -le 0) { continue }
    $hwnd = $h
    break
}
if ($hwnd -eq [IntPtr]::Zero) { Write-Error 'no measurable main window'; exit 1 }

$HWND_TOPMOST = [IntPtr](-1)
$HWND_NOTOPMOST = [IntPtr](-2)
$SWP_NOMOVE = 0x0002
$SWP_NOSIZE = 0x0001
$SWP_SHOWWINDOW = 0x0040
$LEFTDOWN = 0x0002
$LEFTUP = 0x0004
$VK_CONTROL = 0x11
$VK_MENU = 0x12
$VK_A = 0x41
$VK_E = 0x45
$VK_K = 0x4B
$VK_N = 0x4E
$VK_S = 0x53
$VK_V = 0x56
$VK_RETURN = 0x0D
$VK_ESCAPE = 0x1B

[void][C]::ShowWindow($hwnd, 9)
[void][C]::SetWindowPos($hwnd, $HWND_TOPMOST, 0, 0, 0, 0, ($SWP_NOMOVE -bor $SWP_NOSIZE -bor $SWP_SHOWWINDOW))
[void][C]::BringWindowToTop($hwnd)
[void][C]::SetForegroundWindow($hwnd)
[void][C]::SetFocus($hwnd)
Start-Sleep -Milliseconds 1500
[void][C]::SetWindowPos($hwnd, $HWND_NOTOPMOST, 0, 0, 0, 0, ($SWP_NOMOVE -bor $SWP_NOSIZE))

$fg = [IntPtr]::Zero
for ($i = 0; $i -lt 12; $i++) {
    [void][C]::SetForegroundWindow($hwnd)
    [void][C]::BringWindowToTop($hwnd)
    Start-Sleep -Milliseconds 250
    $fg = [C]::GetForegroundWindow()
    if ($fg -eq $hwnd) { break }
}
if ($fg -ne $hwnd) {
    Write-Error "app never took the foreground (fg=$fg app=$hwnd); refusing to type into another window"
    if (-not $proc.HasExited) { $proc.Kill() }
    exit 1
}

$rect = New-Object C+Rect
[void][C]::GetClientRect($hwnd, [ref]$rect)
$origin = New-Object C+P
[void][C]::ClientToScreen($hwnd, [ref]$origin)
$cw = $rect.Right
$ch = $rect.Bottom
$dpi = [C]::GetDpiForWindow($hwnd) / 96.0
Write-Host "client ${cw}x${ch} at $($origin.X),$($origin.Y) dpi=$dpi"

function Px([double]$lx, [double]$ly) {
    [pscustomobject]@{ x = [int]($origin.X + $dpi * $lx); y = [int]($origin.Y + $dpi * $ly) }
}
function Save-Shot([string]$name) {
    $bmp = New-Object System.Drawing.Bitmap $cw, $ch, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($origin.X, $origin.Y, 0, 0, (New-Object System.Drawing.Size $cw, $ch))
    $out = Join-Path $ShotDir "$name.png"
    if (Test-Path -LiteralPath $out) { Remove-Item -LiteralPath $out -Force }
    $bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    Write-Host "  shot -> $out"
}
function Save-Screen([string]$name) {
    $vs = [System.Windows.Forms.SystemInformation]::VirtualScreen
    $bmp = New-Object System.Drawing.Bitmap $vs.Width, $vs.Height, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($vs.X, $vs.Y, 0, 0, (New-Object System.Drawing.Size $vs.Width, $vs.Height))
    $out = Join-Path $ShotDir "$name.png"
    if (Test-Path -LiteralPath $out) { Remove-Item -LiteralPath $out -Force }
    $bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    Write-Host "  screen -> $out"
}
function Click-At($p) {
    [void][C]::SetCursorPos($p.x, $p.y)
    Start-Sleep -Milliseconds 150
    [C]::mouse_event($LEFTDOWN, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 40
    [C]::mouse_event($LEFTUP, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 500
}
function Alt-Key([byte]$vk) {
    [C]::keybd_event($VK_MENU, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [C]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [C]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [C]::keybd_event($VK_MENU, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 500
}
function Ctrl-Key([byte]$vk) {
    [C]::keybd_event($VK_CONTROL, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [C]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [C]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 60
    [C]::keybd_event($VK_CONTROL, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 700
}
function Press-Key([byte]$vk) {
    [C]::keybd_event($vk, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 70
    [C]::keybd_event($vk, 0, 2, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 500
}
function Window-Title() {
    $t = New-Object System.Text.StringBuilder 512
    [void][C]::GetWindowTextW($hwnd, $t, 512)
    return $t.ToString()
}

# Warm-up click on empty canvas.
Click-At (Px 700 650)

# 1) Title menu shows the import entry.
Click-At (Px 28 27)
Start-Sleep -Milliseconds 400
Save-Shot 'import-menu'
Press-Key $VK_ESCAPE
Start-Sleep -Milliseconds 300

# 2) Command palette: Ctrl+K, paste the CJK query (code points keep this file ASCII), Enter.
Ctrl-Key $VK_K
Start-Sleep -Milliseconds 400
$query = -join ([char[]](0x5BFC, 0x5165))
Set-Clipboard -Value $query
Ctrl-Key $VK_V
Start-Sleep -Milliseconds 300
Save-Shot 'import-palette'
Press-Key $VK_RETURN
Start-Sleep -Milliseconds 1500
Save-Screen 'import-dialog'

$fg = [C]::GetForegroundWindow()
$cls = New-Object System.Text.StringBuilder 256
[void][C]::GetClassName($fg, $cls, 256)
Write-Host "foreground after Enter: hwnd=$fg class=$($cls.ToString()) app=$hwnd"
foreach ($w in [C]::TopLevelFor($proc.Id)) { Write-Host "  $w" }
if ($fg -eq $hwnd) { Write-Error 'import dialog did not take the foreground; aborting'; exit 1 }

# 3) The common open dialog focuses the file list; Alt+N jumps to the file-name box.
Alt-Key $VK_N
$paste = $importPath -replace '/', '\'
Set-Clipboard -Value $paste
Start-Sleep -Milliseconds 250
Ctrl-Key $VK_A
Ctrl-Key $VK_V
Start-Sleep -Milliseconds 300
Save-Screen 'import-path'
Press-Key $VK_RETURN
Start-Sleep -Milliseconds 2500
Save-Screen 'import-opened'
Save-Shot 'import-done'
Write-Host "window title after import: '$(Window-Title)'"
foreach ($w in [C]::TopLevelFor($proc.Id)) { Write-Host "  $w" }

if ($SaveAs -ne '') {
    # Imported sessions have no path, so Ctrl+S must fall through to Save As.
    if (-not [System.IO.Path]::IsPathRooted($SaveAs)) {
        $SaveAs = Join-Path (Get-Location).Path $SaveAs
    }
    Ctrl-Key $VK_S
    Start-Sleep -Milliseconds 1500
    Save-Screen 'import-saveas-dialog'
    $fg = [C]::GetForegroundWindow()
    if ($fg -eq $hwnd) { Write-Error 'save-as dialog did not take the foreground; aborting'; exit 1 }
    $paste = $SaveAs -replace '/', '\'
    Set-Clipboard -Value $paste
    Start-Sleep -Milliseconds 250
    Ctrl-Key $VK_A
    Ctrl-Key $VK_V
    Start-Sleep -Milliseconds 300
    Press-Key $VK_RETURN
    Start-Sleep -Milliseconds 1800
    Save-Shot 'import-saved'
    Write-Host "window title after save: '$(Window-Title)'"
    Write-Host "saved exists=$(Test-Path -LiteralPath $SaveAs)"
    if (Test-Path -LiteralPath $SaveAs) {
        $fi = Get-Item -LiteralPath $SaveAs
        Write-Host "saved bytes=$($fi.Length) mtime=$($fi.LastWriteTime.ToString('HH:mm:ss'))"
    }
}

if (-not $KeepOpen) {
    Write-Host 'closing app'
    $proc.CloseMainWindow() | Out-Null
    Start-Sleep -Milliseconds 800
    if (-not $proc.HasExited) { $proc.Kill() }
}
Write-Host 'done'
