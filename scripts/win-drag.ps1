# Drag with SendInput absolute moves (works where legacy mouse_event does not).
# Usage: powershell -File win-drag.ps1 -X1 100 -Y1 200 -X2 400 -Y2 500 [-Steps 16]
param(
  [Parameter(Mandatory=$true)][int]$X1,
  [Parameter(Mandatory=$true)][int]$Y1,
  [Parameter(Mandatory=$true)][int]$X2,
  [Parameter(Mandatory=$true)][int]$Y2,
  [int]$Steps = 16,
  [int]$StepMs = 40
)
Add-Type @"
using System;
using System.Runtime.InteropServices;
[StructLayout(LayoutKind.Sequential)]
public struct MOUSEINPUT {
  public int dx; public int dy; public uint mouseData; public uint dwFlags;
  public uint time; public UIntPtr dwExtraInfo;
}
[StructLayout(LayoutKind.Sequential)]
public struct INPUT { public uint type; public MOUSEINPUT mi; }
public class Drag {
  [DllImport("user32.dll", SetLastError=true)] public static extern uint SendInput(uint n, INPUT[] p, int cb);
  [DllImport("user32.dll")] public static extern int GetSystemMetrics(int i);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  const uint MOVE = 0x0001, ABS = 0x8000, DOWN = 0x0002, UP = 0x0004, VDESK = 0x4000;
  static void Send(uint flags, int x = -1, int y = -1) {
    INPUT inp = new INPUT();
    inp.type = 0;
    inp.mi.dwFlags = flags;
    if (x >= 0) {
      int sw = GetSystemMetrics(0), sh = GetSystemMetrics(1);
      inp.mi.dx = (int)Math.Round((double)x * 65535 / (sw - 1));
      inp.mi.dy = (int)Math.Round((double)y * 65535 / (sh - 1));
      inp.mi.dwFlags |= ABS | VDESK;
    }
    INPUT[] arr = new INPUT[] { inp };
    if (SendInput(1, arr, Marshal.SizeOf(typeof(INPUT))) != 1)
      throw new Exception("SendInput failed: " + Marshal.GetLastWin32Error());
  }
  public static void Run(int x1, int y1, int x2, int y2, int steps, int stepMs) {
    SetProcessDPIAware();
    Send(MOVE, x1, y1);
    System.Threading.Thread.Sleep(350);
    Send(DOWN);
    System.Threading.Thread.Sleep(150);
    for (int i = 1; i <= steps; i++) {
      int x = x1 + (x2 - x1) * i / steps;
      int y = y1 + (y2 - y1) * i / steps;
      Send(MOVE, x, y);
      System.Threading.Thread.Sleep(stepMs);
    }
    System.Threading.Thread.Sleep(300);
    Send(UP);
  }
}
"@
[Drag]::Run($X1, $Y1, $X2, $Y2, $Steps, $StepMs)
"dragged $X1,$Y1 -> $X2,$Y2 in $Steps steps"
