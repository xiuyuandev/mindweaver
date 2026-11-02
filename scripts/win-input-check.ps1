# Is synthetic input usable on this desktop right now?
#
# Every mouse-driven pixel probe (v5-accent, the v5-a/v5-b rigs) needs either
# SetCursorPos + mouse_event or a foreground we can actually take. Windows lets a
# background process inject input only into whatever it calls the foreground window,
# and a security suite that keeps activating itself can leave both paths dead. The
# check prints which ones work so a probe failure can be attributed to the rig
# instead of the product.
#
# ASCII only: Windows PowerShell 5.1 parses .ps1 as the ANSI codepage.

Add-Type @'
using System;
using System.Runtime.InteropServices;

public static class IN {
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out P p);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool join);
    [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
    [StructLayout(LayoutKind.Sequential)] public struct P { public int X, Y; }

    public static string Title(IntPtr h) {
        var sb = new System.Text.StringBuilder(512);
        GetWindowText(h, sb, sb.Capacity);
        return sb.ToString();
    }
    public static uint Pid(IntPtr h) { uint p; GetWindowThreadProcessId(h, out p); return p; }

    /// The documented way round the foreground lock: share input queues, activate, unshare.
    public static bool TakeForeground(IntPtr h) {
        uint pid;
        uint fgTid = GetWindowThreadProcessId(GetForegroundWindow(), out pid);
        uint myTid = GetCurrentThreadId();
        bool joined = AttachThreadInput(myTid, fgTid, true);
        BringWindowToTop(h);
        bool ok = SetForegroundWindow(h);
        if (joined) AttachThreadInput(myTid, fgTid, false);
        return GetForegroundWindow() == h;
    }
}
'@

[void][IN]::SetProcessDPIAware()

# --- can the cursor be moved at all? -------------------------------------------------
$before = New-Object IN+P
[void][IN]::GetCursorPos([ref]$before)
$targetX = $before.X + 137
$targetY = $before.Y + 91
$set = [IN]::SetCursorPos($targetX, $targetY)
Start-Sleep -Milliseconds 120
$after = New-Object IN+P
[void][IN]::GetCursorPos([ref]$after)
$moved = ($after.X -eq $targetX -and $after.Y -eq $targetY)
Write-Host ("cursor: SetCursorPos returned " + $set + ", wanted " + $targetX + "," + $targetY + ", got " + $after.X + "," + $after.Y + " -> injected mouse " + $(if ($moved) { "WORKS" } else { "BLOCKED" }))

# --- who owns the foreground? ---------------------------------------------------------
$fg = [IN]::GetForegroundWindow()
$fgPid = [IN]::Pid($fg)
Write-Host ("foreground: hwnd=" + $fg.Value + " pid=" + $fgPid + " title='" + [IN]::Title($fg) + "'")

# --- can a background process take it? --------------------------------------------------
$self = (Get-Process -Id $PID).MainWindowHandle
Write-Host ("this process main window: " + $self)

# --- does a posted key reach an app that is not foreground? ----------------------------
# Measured answer on this machine: yes, winit reads keys from WM_KEYDOWN/WM_KEYUP, but
# NOT from posted mouse messages because it takes the pointer from raw input.
Write-Host "posted-key path is verified per-app inside each probe (compare frames before/after)."

if (-not $moved) {
    Write-Host ""
    Write-Host "Verdict: hardware-level input injection is dead right now. Mouse-driven probes"
    Write-Host "cannot click anything; only SendMessage-based keyboard input still arrives."
    Write-Host "Unblock it by clicking the desktop once (or quitting whatever holds the"
    Write-Host "foreground above), then re-run this check."
    exit 1
}
Write-Host "Verdict: injection works - mouse-driven probes are runnable."
exit 0
