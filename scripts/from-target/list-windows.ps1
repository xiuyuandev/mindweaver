param([int]$ProcessId)
Add-Type -Language CSharp @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class W {
    public delegate bool EnumProc(IntPtr h, IntPtr p);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr p);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    public static List<string> List(uint pid) {
        var res = new List<string>();
        EnumWindows(delegate(IntPtr h, IntPtr p) {
            uint wp; GetWindowThreadProcessId(h, out wp);
            if (wp != pid) return true;
            RECT r; GetWindowRect(h, out r);
            var t = new StringBuilder(256); GetWindowText(h, t, 256);
            var c = new StringBuilder(256); GetClassName(h, c, 256);
            res.Add(string.Format("hwnd={0} vis={1} rect={2},{3} {4}x{5} class={6} title={7}",
                h, IsWindowVisible(h), r.Left, r.Top, r.Right-r.Left, r.Bottom-r.Top, c, t));
            return true;
        }, IntPtr.Zero);
        return res;
    }
}
'@
$p = Get-Process -Id $ProcessId -ErrorAction Stop
[W]::List([uint32]$ProcessId) | ForEach-Object { $_ }
