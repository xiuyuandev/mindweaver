param(
    [string]$ShotDir = "shots/qa"
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $root

Add-Type -Language CSharp @'
using System;
using System.Runtime.InteropServices;
public static class Inp {
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool attach);
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, int data, IntPtr extra);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint f, IntPtr extra);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    public static void Key(byte vk) {
        keybd_event(vk, 0, 0, IntPtr.Zero);
        System.Threading.Thread.Sleep(60);
        keybd_event(vk, 0, 2, IntPtr.Zero);
        System.Threading.Thread.Sleep(180);
    }
    public static void Click(int x, int y) {
        SetCursorPos(x, y);
        System.Threading.Thread.Sleep(80);
        mouse_event(0x0002, 0, 0, 0, IntPtr.Zero);
        System.Threading.Thread.Sleep(60);
        mouse_event(0x0004, 0, 0, 0, IntPtr.Zero);
        System.Threading.Thread.Sleep(180);
    }
    public static bool Focus(IntPtr h) {
        uint pid; uint tid = GetWindowThreadProcessId(h, out pid);
        uint mine = GetCurrentThreadId();
        AttachThreadInput(mine, tid, true);
        ShowWindow(h, 9);
        BringWindowToTop(h);
        bool ok = SetForegroundWindow(h);
        AttachThreadInput(mine, tid, false);
        System.Threading.Thread.Sleep(350);
        return ok && GetForegroundWindow() == h;
    }
}
'@

$smoke = Join-Path $root 'target\inspector-shot'
New-Item -ItemType Directory -Force -Path $smoke, (Join-Path $root $ShotDir) | Out-Null
$doc = Join-Path $smoke 'fixture.zmind'
Copy-Item -Force (Join-Path $root 'shots\qa\e5-timeline.zmind') $doc
$env:APPDATA = Join-Path $smoke 'appdata'
$env:LOCALAPPDATA = Join-Path $smoke 'localappdata'
$env:MW_HOME = '0'
$env:MW_OPEN = $doc
$env:MW_INSPECTOR = '1'
$state = Join-Path $smoke 'window.json'
[IO.File]::WriteAllText($state, '{"onboarded":true,"show_inspector":true,"inspector_w":300}', (New-Object System.Text.UTF8Encoding($false)))
$env:MW_WINDOW_STATE = $state
$exe = Join-Path $root 'target\release\mw-shell.exe'
$p = Start-Process -FilePath $exe -PassThru
Start-Sleep -Seconds 6
$p.Refresh()
if ($p.HasExited) { throw "exited $($p.ExitCode)" }
$h = $p.MainWindowHandle
if ($h -eq [IntPtr]::Zero) { throw 'no window' }
$r0 = New-Object Inp+RECT
[Inp]::GetWindowRect($h, [ref]$r0) | Out-Null
"Inspector shot pid=$($p.Id) hwnd=$h rect=$($r0.Left),$($r0.Top) $($r0.Right-$r0.Left)x$($r0.Bottom-$r0.Top)"
# Click the fixture root: this focuses the canvas, selects the node, and the
# inspector switches from the document page to the node style page.
& (Join-Path $root 'scripts\win-capture.ps1') -ProcessId $p.Id -OutFile (Join-Path $ShotDir 'current-inspector-style.png') -ClickX 340 -ClickY 584
Start-Sleep -Milliseconds 500
& (Join-Path $root 'scripts\win-capture.ps1') -ProcessId $p.Id -OutFile (Join-Path $ShotDir 'current-inspector-content.png') -ClickX 1583 -ClickY 364
Start-Sleep -Milliseconds 500
& (Join-Path $root 'scripts\win-capture.ps1') -ProcessId $p.Id -OutFile (Join-Path $ShotDir 'current-inspector-mark.png') -ClickX 1716 -ClickY 364
$p.Refresh()
if (-not $p.HasExited) { Stop-Process -Id $p.Id -ErrorAction SilentlyContinue; $p.WaitForExit(5000) | Out-Null }
'DONE'
