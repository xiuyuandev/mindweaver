param(
    [string]$Exe = 'target/release/mw-shell.exe',
    [string]$ShotDir = 'shots/qa',
    [string]$OutDir = '',
    [string]$Tag = 'export',
    # Logical client coords clicked (in order) after the dialog opens; ';'-separated "x,y".
    # Empty = keep the remembered options. Example: '261,289;357,317' = 1x chip + transparent.
    [string]$Clicks = '',
    # Absolute path to type into the native save dialog. Empty = dialog only.
    [string]$SaveTo = '',
    # ';'-separated keys pressed one at a time while the options dialog is open, each shot
    # afterwards ("<Tag>-key1-enter"). Names: enter / esc / tab / f2 / ctrl-e. Diagnostics:
    # whoever eats Enter shows up as a changed document or a vanished dialog.
    [string]$Keys = '',
    [switch]$KeepOpen
)

# M4 export probe: real input (Ctrl+E / click / native save dialog) end to end.
# Pure ASCII on purpose: PowerShell 5.1 reads a BOM-less file as the ANSI codepage and
# would mangle any literal CJK (docs/14 lesson 37).

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
    // Every visible top-level window owned by pid: the only way to tell "the save dialog
    // opened but sits behind the app" from "the save dialog never opened at all".
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
    // The "already exists, replace it?" confirm is a small #32770 owned by the save dialog
    // (itself a #32770, but a whole lot bigger). Returns the first visible #32770 smaller than
    // maxArea, or Zero. Needed because that confirm defaults to 否: Enter answers NO, the save
    // silently never happens, and only the file's mtime tells you.
    public static IntPtr SmallDialogFor(int pid, int maxArea) {
        IntPtr found = IntPtr.Zero;
        EnumWindows(delegate(IntPtr h, IntPtr l) {
            uint wp;
            GetWindowThreadProcessId(h, out wp);
            if ((int)wp != pid || !IsWindowVisible(h)) { return true; }
            System.Text.StringBuilder cls = new System.Text.StringBuilder(256);
            GetClassName(h, cls, 256);
            if (cls.ToString() != "#32770") { return true; }
            Rect r;
            GetWindowRect(h, out r);
            if ((r.Right - r.Left) * (r.Bottom - r.Top) < maxArea) { found = h; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
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
if ($OutDir -eq '') { $OutDir = (Join-Path (Get-Location).Path 'shots/qa') }
if (-not (Test-Path -LiteralPath $OutDir)) { New-Item -ItemType Directory -Path $OutDir | Out-Null }
# The common file dialog remembers the folder the last save went to, so a *relative* path
# typed into the name box lands inside whatever folder it happens to be showing
# ("shots\qa\out\shots\qa\out\x.png"). Always paste an absolute one.
if ($SaveTo -ne '' -and -not [System.IO.Path]::IsPathRooted($SaveTo)) {
    $SaveTo = Join-Path (Get-Location).Path $SaveTo
}

$proc = Start-Process -FilePath $exePath -PassThru
Write-Host "launched pid=$($proc.Id)"

# The winit window is created hidden and only shown after the first frame, so
# MainWindowHandle can be a placeholder for a while. Poll until the client rect is real.
$hwnd = [IntPtr]::Zero
$rect = New-Object C+Rect
$origin = New-Object C+P
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
    $rect = $r
    $o = New-Object C+P
    [void][C]::ClientToScreen($h, [ref]$o)
    $origin = $o
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
$VK_V = 0x56
$VK_Y = 0x59
$VK_RETURN = 0x0D
$VK_ESCAPE = 0x1B
$VK_TAB = 0x09
$VK_F2 = 0x71

[void][C]::ShowWindow($hwnd, 9)
[void][C]::SetWindowPos($hwnd, $HWND_TOPMOST, 0, 0, 0, 0, ($SWP_NOMOVE -bor $SWP_NOSIZE -bor $SWP_SHOWWINDOW))
[void][C]::BringWindowToTop($hwnd)
[void][C]::SetForegroundWindow($hwnd)
[void][C]::SetFocus($hwnd)
Start-Sleep -Milliseconds 1500

# TOPMOST was only there to make the app come up in front of the terminal. It has to go
# before the native dialog opens: a topmost window always outranks its own non-topmost
# owned dialog, so the dialog would be painted *behind* the app, never get the foreground,
# and every keystroke (Ctrl+A/Ctrl+V included) would land on the canvas instead.
[void][C]::SetWindowPos($hwnd, $HWND_NOTOPMOST, 0, 0, 0, 0, ($SWP_NOMOVE -bor $SWP_NOSIZE))
# Nothing may be typed until the app really owns the foreground. SetForegroundWindow can be
# refused outright (the receiving app forces itself back in front), and the keystrokes then
# go to whatever window the user left there - one run typed the save path and both Ctrl+E and
# the option clicks straight into Chrome. Retry, then refuse to run at all.
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
Write-Host "foreground ok: $hwnd"

$rect = New-Object C+Rect
[void][C]::GetClientRect($hwnd, [ref]$rect)
$origin = New-Object C+P
[void][C]::ClientToScreen($hwnd, [ref]$origin)
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
    if (-not (Test-Path -LiteralPath $ShotDir)) { New-Item -ItemType Directory -Path $ShotDir | Out-Null }
    $bmp = New-Object System.Drawing.Bitmap $cw, $ch, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($origin.X, $origin.Y, 0, 0, (New-Object System.Drawing.Size $cw, $ch))
    $out = Join-Path $ShotDir "$name.png"
    if (Test-Path -LiteralPath $out) { Remove-Item -LiteralPath $out -Force }
    $bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    Write-Host "  shot -> $out"
}
# Screenshot of the whole virtual screen: the native save dialog is a separate HWND and
# may sit outside our client rect.
function Save-Screen([string]$name) {
    if (-not (Test-Path -LiteralPath $ShotDir)) { New-Item -ItemType Directory -Path $ShotDir | Out-Null }
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

# Warm-up click on empty canvas: absorbs the activation click and focuses the canvas.
Click-At (Px 620 620)

Ctrl-Key $VK_E
Start-Sleep -Milliseconds 700
Save-Shot "$Tag-dialog"

if ($Clicks -ne '') {
    foreach ($c in $Clicks.Split(';')) {
        if ($c.Trim() -eq '') { continue }
        $xy = $c.Split(',')
        Click-At (Px ([double]$xy[0]) ([double]$xy[1]))
    }
    Save-Shot "$Tag-options"
}

if ($Keys -ne '') {
    $i = 0
    foreach ($k in $Keys.Split(';')) {
        $k = $k.Trim().ToLower()
        if ($k -eq '') { continue }
        $i++
        switch ($k) {
            'enter'  { Press-Key $VK_RETURN }
            'esc'    { Press-Key $VK_ESCAPE }
            'tab'    { Press-Key $VK_TAB }
            'f2'     { Press-Key $VK_F2 }
            'ctrl-e' { Ctrl-Key $VK_E }
            default  { Write-Warning "unknown key: $k" }
        }
        Start-Sleep -Milliseconds 400
        Save-Shot "$Tag-key$i-$k"
        $top = [C]::TopLevelFor($proc.Id) | Where-Object { $_ -like "*class='#32770'*" }
        Write-Host "  after $k : dialogs=$($top.Count)"
    }
}

if ($SaveTo -eq '') {
    Write-Host 'dialog-only stage; leaving the app open'
    exit 0
}

# Enter confirms the export dialog (the dialog owns Enter while it is open: the canvas is
# input-blocked, so the canvas does not eat it as "add sibling"). Keyboard-only on purpose:
# the egui window's on-screen position is not stable across launches, so clicking its
# buttons by absolute coordinate is flaky.
Press-Key $VK_RETURN
Start-Sleep -Milliseconds 1500
Save-Screen "$Tag-savedlg"
# Guard: if the app still owns the foreground, the dialog is behind it and the keystrokes
# below would edit the document instead of the dialog (that is exactly how the first run
# of this probe silently added a "C:\..." node and produced no file).
$fg = [C]::GetForegroundWindow()
$cls = New-Object System.Text.StringBuilder 256
[void][C]::GetClassName($fg, $cls, 256)
Write-Host "foreground: hwnd=$fg class=$($cls.ToString()) app=$hwnd"
$top = [C]::TopLevelFor($proc.Id)
Write-Host "top-level windows of pid $($proc.Id): $($top.Count)"
foreach ($w in $top) { Write-Host "  $w" }
if ($fg -eq $hwnd) { Write-Error 'save dialog did not take the foreground; aborting before typing'; exit 1 }
# The common file dialog focuses the file-name field with the default name selected.
# It needs BACKSLASHES: a pasted forward-slash path is rejected with "file name is invalid".
$paste = $SaveTo -replace '/', '\'
Set-Clipboard -Value $paste
Start-Sleep -Milliseconds 250
Ctrl-Key $VK_A
Ctrl-Key $VK_V
Start-Sleep -Milliseconds 300
Save-Screen "$Tag-path"
Press-Key $VK_RETURN
# A target that already exists pops the dialog's own "replace it?" confirm, which owns the
# screen until someone answers it - and it defaults to NO, so Enter answers "no", the save
# silently never happens, and the probe would report success while the file on disk is still
# the *old* one. That is exactly how a stale transparent PNG survived a "verified" run.
# Answer it with the 是(Y) accelerator; a fresh name never sees this path at all.
Start-Sleep -Milliseconds 900
for ($retry = 0; $retry -lt 2; $retry++) {
    $cf = [C]::SmallDialogFor($proc.Id, 300000)
    if ($cf -eq [IntPtr]::Zero) { break }
    Write-Host "  overwrite confirm up (hwnd=$cf); answering 是(Y)"
    Alt-Key $VK_Y
    Start-Sleep -Milliseconds 1200
}
# Whatever is left on screen after that is a real failure to answer.
$up = @([C]::TopLevelFor($proc.Id) | Where-Object { $_ -like "*class='#32770'*" })
if ($up.Count -gt 0) { Write-Host "  WARNING: dialog(s) still up: $($up -join ' | ')" }
# The receipt toast lives 2.4s (mw-shell/src/main.rs toast_bar), so shoot it while it is up
# and then again after it is gone: the second shot is what proves the app is still alive and
# the document is unchanged.
Save-Screen "$Tag-confirm"
Start-Sleep -Milliseconds 600
Save-Shot "$Tag-toast"
Start-Sleep -Milliseconds 2200
Save-Shot "$Tag-done"

Write-Host "exists=$(Test-Path -LiteralPath $SaveTo)"
if (Test-Path -LiteralPath $SaveTo) {
    $fi = Get-Item -LiteralPath $SaveTo
    Write-Host "bytes=$($fi.Length)"
    Write-Host "mtime=$($fi.LastWriteTime.ToString('HH:mm:ss')) (now $(Get-Date -Format HH:mm:ss))"
}
[void][C]::SetWindowPos($hwnd, $HWND_NOTOPMOST, 0, 0, 0, 0, ($SWP_NOMOVE -bor $SWP_NOSIZE))
if (-not $KeepOpen) {
    Write-Host 'closing app'
    $proc.CloseMainWindow() | Out-Null
    Start-Sleep -Milliseconds 800
    if (-not $proc.HasExited) { $proc.Kill() }
}
Write-Host 'done'
