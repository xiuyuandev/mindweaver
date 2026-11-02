param(
    [string]$ProcessName = 'mw-shell',
    [string]$ShotDir = 'shots/m2a'
)

# M2a inline-editing probe: drives REAL keyboard input against the running shell,
# so focus hand-off, IME-capable TextEdit, commit/cancel and undo/redo are tested
# end to end. Screenshots are screen copies of the window rect (PrintWindow cannot
# see wgpu surfaces).
#
# Every keystroke is preceded by a foreground-window assertion. The first run of
# this script silently lost the whole second half of the sequence: another app
# took the foreground, SendKeys delivered to it, and the screenshots looked like
# "Tab does nothing" instead of "keys never arrived". A refocus is logged so a
# flat screenshot series can be told apart from a real keyboard bug.
#
# ASCII only on purpose: Windows PowerShell 5.1 decodes BOM-less UTF-8 scripts as
# the ANSI code page. Chinese payloads are built from [char] codes instead.

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

if (-not ('M' -as [type])) {
    Add-Type -Language CSharp @'
using System;
using System.Runtime.InteropServices;
public static class M {
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out Rect r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref P p);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
    [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h, int x, int y, int w, int ht, bool repaint);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct P { public int X, Y; }
}
'@
}
[void][M]::SetProcessDPIAware()

$proc = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowHandle -ne [IntPtr]::Zero } | Select-Object -First 1
if (-not $proc) { Write-Error "process not running (or no window yet): $ProcessName"; exit 1 }
$hwnd = $proc.MainWindowHandle
$refocuses = 0

function Get-Client([IntPtr]$h) {
    $r = New-Object M+Rect
    [void][M]::GetClientRect($h, [ref]$r)
    $o = New-Object M+P
    [void][M]::ClientToScreen($h, [ref]$o)
    [pscustomobject]@{ x = $o.X; y = $o.Y; w = $r.Right; h = $r.Bottom; s = ([M]::GetDpiForWindow($h) / 96.0) }
}

function Test-Foreground([IntPtr]$h) {
    $fg = [M]::GetForegroundWindow()
    if ($fg -eq $h) { return $true }
    $pid2 = 0
    [void][M]::GetWindowThreadProcessId($fg, [ref]$pid2)
    $name = if ($pid2) { (Get-Process -Id $pid2 -ErrorAction SilentlyContinue).ProcessName } else { 'none' }
    Write-Host ("  ! foreground is '{0}' (pid {1})" -f $name, $pid2)
    return $false
}

function Focus-Window([IntPtr]$h) {
    [void][M]::ShowWindow($h, 9)          # SW_RESTORE
    [void][M]::SetForegroundWindow($h)
    Start-Sleep -Milliseconds 200
    $wsh = New-Object -ComObject WScript.Shell
    [void]$wsh.AppActivate($proc.Id)
    Start-Sleep -Milliseconds 200
    # Park the pointer inside our own titlebar area, far from the taskbar: a
    # taskbar hover pops a thumbnail preview over the window and pollutes shots.
    [void][M]::SetCursorPos(20, 20)
}

# Both conditions matter: the OS must have us in front AND egui must have handed
# the keyboard to the canvas widget. Only the first one can be asserted here.
function Ensure-Foreground([IntPtr]$h) {
    if (Test-Foreground $h) { return }
    $script:refocuses++
    for ($i = 0; $i -lt 3 -and -not (Test-Foreground $h); $i++) {
        Focus-Window $h
        Start-Sleep -Milliseconds 250
    }
    if (-not (Test-Foreground $h)) { Write-Error "cannot reclaim foreground"; exit 1 }
    Write-Host "  refocused"
}

function Save-Shot([string]$name, $c) {
    if (-not (Test-Path -LiteralPath $ShotDir)) { New-Item -ItemType Directory -Path $ShotDir | Out-Null }
    $bmp = New-Object System.Drawing.Bitmap $c.w, $c.h, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($c.x, $c.y, 0, 0, (New-Object System.Drawing.Size $c.w, $c.h))
    $out = Join-Path $ShotDir "$name.png"
    if (Test-Path -LiteralPath $out) { Remove-Item -LiteralPath $out -Force }
    $bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
    Write-Host "  shot -> $out"
}

function Send-Keys([string]$keys) {
    Ensure-Foreground $hwnd
    [System.Windows.Forms.SendKeys]::SendWait($keys)
    Start-Sleep -Milliseconds 450
}

# Payloads: "M2a 行内编辑", "子主题", "拼音: zhi si".
$cn = -join ([char[]](0x884C, 0x5185, 0x7F16, 0x8F91))
$child = -join ([char[]](0x5B50, 0x4E3B, 0x9898))
$pinyin = -join ([char[]](0x62FC, 0x97F3))

Focus-Window $hwnd
$c = Get-Client $hwnd
[void][M]::MoveWindow($hwnd, 80, 40, [int](1000 * $c.s), [int](700 * $c.s), $true)
Start-Sleep -Milliseconds 400
Focus-Window $hwnd
$c = Get-Client $hwnd
Write-Host ("client={0},{1} {2}x{3} physical, scale={4}" -f $c.x, $c.y, $c.w, $c.h, $c.s)
Save-Shot '01-start' $c

# F2 -> inline editor on the root (nothing selected yet).
Send-Keys '{F2}'
Save-Shot '02-edit-root' $c

# Clipboard paste exercises the CJK path plus TextEdit's own paste handling.
Set-Clipboard -Value ("M2a " + $cn)
Send-Keys '^v'
Save-Shot '03-edit-pasted' $c

# Enter commits (Shift+Enter would insert a newline); canvas regains focus.
Send-Keys '{ENTER}'
Save-Shot '04-committed' $c

# Document history: one SetText entry per commit.
Send-Keys '^z'
Save-Shot '05-undo' $c
Send-Keys '^y'
Save-Shot '06-redo' $c

# Tab adds a child and drops straight into its editor.
Send-Keys '{TAB}'
Save-Shot '07-child-editor' $c
Set-Clipboard -Value $child
Send-Keys '^v'
Send-Keys '{ENTER}'
Save-Shot '08-child-committed' $c

# Enter adds a sibling of the child.
Send-Keys '{ENTER}'
Set-Clipboard -Value ($pinyin + ': zhi si')
Send-Keys '^v'
Send-Keys '{ENTER}'
Save-Shot '09-sibling-committed' $c

# Esc cancels an in-progress edit: F2, type, Esc, and the text must be unchanged.
Send-Keys '{F2}'
Set-Clipboard -Value 'SHOULD-NOT-STICK'
Send-Keys '^v'
Send-Keys '{ESC}'
Save-Shot '10-cancelled' $c

# Delete removes the selected sibling; root itself is protected.
Send-Keys '{DELETE}'
Save-Shot '11-deleted' $c

Write-Host ("done (refocuses={0})" -f $refocuses)
