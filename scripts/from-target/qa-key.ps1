param(
    [int]$ProcessId,
    [string]$Keys
)
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class KW {
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
}
"@
$p = Get-Process -Id $ProcessId -ErrorAction Stop
[void][KW]::SetForegroundWindow($p.MainWindowHandle)
Start-Sleep -Milliseconds 400
[System.Windows.Forms.SendKeys]::SendWait($Keys)
