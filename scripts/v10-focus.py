#!/usr/bin/env python3
r"""H3 / H4 real-input probes (2026-10-19).

Why Python: the probe needs SendInput with absolute coordinates (mouse_event
relative moves get dropped while the button is held) plus a JSON read of the
saved .zmind. PowerShell can do SendInput (scripts/win-drag.ps1) but the rest is
much shorter here. Run it only on an interactive desktop; it needs the app
already running with MW_TRACE and (optionally) MW_OPEN pointing at a document.

Usage:
  set MW_TRACE=%TEMP%\trace.txt
  set MW_OPEN=%TEMP%\h4-doc.zmind
  start target\release\mw-shell.exe
  python scripts/v10-focus.py h3
  python scripts/v10-focus.py h4 --doc %TEMP%\h4-doc.zmind

H3 asserts: Tab reaches an inspector control; F9 close + F9 open emits
"event restore <same key>" and focus lands back on it.
H4 asserts: Tab reaches inspector/style/branch/swatch; Right moves; Space
applies a branch colour that reaches content.json; Ctrl+Z removes it again.

Exit codes: 0 PASS, 2 environment (no window / no focus / no trace), 3 product
regression (assertion failed).
"""
import argparse
import ctypes
import json
import os
import sys
import time
import zipfile
from ctypes import wintypes

import numpy as np
from PIL import ImageGrab

user32 = ctypes.windll.user32
kernel32 = ctypes.windll.kernel32
user32.SetProcessDPIAware()

VK = {"f6": 0x75, "f9": 0x78, "tab": 0x09, "space": 0x20,
      "right": 0x27, "left": 0x25, "ctrl": 0x11, "s": 0x53, "z": 0x5A}


def find_window():
    hits = []

    @ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    def cb(h, _l):
        n = user32.GetWindowTextLengthW(h)
        if n:
            buf = ctypes.create_unicode_buffer(n + 1)
            user32.GetWindowTextW(h, buf, n + 1)
            if "织思" in buf.value and user32.IsWindowVisible(h):
                hits.append(h)
        return True

    user32.EnumWindows(cb, 0)
    return hits[0] if hits else None


def focus_window(h):
    """Restore + attach-thread trick: keys only land on the foreground window."""
    user32.ShowWindow(h, 9)
    time.sleep(0.3)
    if user32.GetForegroundWindow() != h:
        tid_fg = user32.GetWindowThreadProcessId(user32.GetForegroundWindow(), 0)
        tid_me = kernel32.GetCurrentThreadId()
        user32.AttachThreadInput(tid_me, tid_fg, True)
        user32.SetForegroundWindow(h)
        user32.AttachThreadInput(tid_me, tid_fg, False)
        time.sleep(0.4)
    return user32.GetForegroundWindow() == h


class MOUSEINPUT(ctypes.Structure):
    _fields_ = [("dx", wintypes.LONG), ("dy", wintypes.LONG),
                ("mouseData", wintypes.DWORD), ("dwFlags", wintypes.DWORD),
                ("time", wintypes.DWORD), ("dwExtraInfo", ctypes.c_ulonglong)]


class _U(ctypes.Union):
    _fields_ = [("mi", MOUSEINPUT)]


class INPUT(ctypes.Structure):
    _fields_ = [("type", wintypes.DWORD), ("u", _U)]


MOVE, ABS, DOWN, UP, VDESK = 1, 0x8000, 2, 4, 0x4000
SW, SH = user32.GetSystemMetrics(0), user32.GetSystemMetrics(1)


def send_mouse(flags, x=None, y=None):
    inp = INPUT()
    inp.type = 0
    if x is not None:
        inp.u.mi.dx = int(round(x * 65535 / (SW - 1)))
        inp.u.mi.dy = int(round(y * 65535 / (SH - 1)))
        inp.u.mi.dwFlags = flags | ABS | VDESK
    else:
        inp.u.mi.dwFlags = flags
    user32.SendInput(1, ctypes.byref(inp), ctypes.sizeof(INPUT))


def click(x, y):
    send_mouse(MOVE, x, y)
    time.sleep(0.15)
    send_mouse(DOWN)
    time.sleep(0.05)
    send_mouse(UP)
    time.sleep(0.3)


def key(vk, hold=0.03, gap=0.08):
    user32.keybd_event(vk, 0, 0, 0)
    time.sleep(hold)
    user32.keybd_event(vk, 0, 2, 0)
    time.sleep(gap)


def chord(mod, vk):
    user32.keybd_event(mod, 0, 0, 0)
    time.sleep(0.03)
    user32.keybd_event(vk, 0, 0, 0)
    time.sleep(0.04)
    user32.keybd_event(vk, 0, 2, 0)
    time.sleep(0.03)
    user32.keybd_event(mod, 0, 2, 0)
    time.sleep(0.25)


def trace_lines(path):
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            return f.read().splitlines()
    except FileNotFoundError:
        return []


def click_a_branch_card(h):
    r = wintypes.RECT()
    user32.GetWindowRect(h, ctypes.byref(r))
    img = ImageGrab.grab(bbox=(r.left, r.top, r.right, r.bottom), all_screens=True).convert("RGB")
    a = np.asarray(img).astype(np.int16)
    fill = (0x8f, 0xd0, 0xbc)  # snow 主题第一支分支卡的实色
    m = ((np.abs(a[:, :, 0] - fill[0]) <= 2) & (np.abs(a[:, :, 1] - fill[1]) <= 2)
         & (np.abs(a[:, :, 2] - fill[2]) <= 2))
    ys, xs = np.where(m)
    if len(xs) < 200:
        return False
    px, py = int(xs.min()) + 14, (int(ys.min()) + int(ys.max())) // 2
    click(r.left + px, r.top + py)
    return True


def run_h3(trace):
    key(VK["f9"])
    time.sleep(0.7)
    key(VK["f6"])
    time.sleep(0.4)
    target = None
    for _ in range(160):
        key(VK["tab"], 0.02, 0.04)
        ls = trace_lines(trace)
        last = ls[-1] if ls else ""
        if last.startswith("focus inspector/"):
            target = last[len("focus "):]
            break
    if not target:
        print("FAIL: Tab never reached an inspector control")
        return 3
    print("reached", target)
    before = len(trace_lines(trace))
    key(VK["f9"])
    time.sleep(0.8)
    key(VK["f9"])
    time.sleep(1.0)
    ls = trace_lines(trace)
    tail = ls[before:]
    ok_event = any(l == "event restore " + target for l in tail)
    ok_focus = ls and ls[-1] == "focus " + target
    print("restore event:", ok_event, "focus back:", ok_focus)
    print("trace tail:", tail)
    return 0 if (ok_event and ok_focus) else 3


def run_h4(trace, doc):
    if not click_a_branch_card(find_window()):
        print("SKIP: no 一级分支卡（需要夹具文档）")
        return 2
    key(VK["f9"])
    time.sleep(0.8)
    key(VK["f6"])
    time.sleep(0.4)
    swatch = None
    for _ in range(240):
        key(VK["tab"], 0.02, 0.04)
        ls = trace_lines(trace)
        last = ls[-1] if ls else ""
        if last.startswith("focus inspector/style/branch/swatch"):
            swatch = last[len("focus "):]
            break
    if not swatch:
        print("FAIL: Tab never reached the branch swatch row")
        return 3
    print("swatch", swatch)
    key(VK["right"])
    time.sleep(0.2)
    key(VK["space"])
    time.sleep(0.6)
    chord(VK["ctrl"], VK["s"])
    time.sleep(1.0)
    with zipfile.ZipFile(doc) as z:
        c = json.loads(z.read("content.json"))
    styled = {k: v.get("style") for k, v in c["nodes"].items() if v.get("style")}
    print("styled after apply:", json.dumps(styled, ensure_ascii=False)[:200])
    if not styled:
        print("FAIL: Space did not change the document")
        return 3
    chord(VK["ctrl"], VK["z"])
    time.sleep(0.6)
    chord(VK["ctrl"], VK["s"])
    time.sleep(1.0)
    with zipfile.ZipFile(doc) as z:
        c = json.loads(z.read("content.json"))
    left = {k: v.get("style") for k, v in c["nodes"].items() if v.get("style")}
    print("styled after undo:", json.dumps(left, ensure_ascii=False)[:200])
    return 0 if left == {} else 3


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("scene", choices=["h3", "h4"])
    ap.add_argument("--doc", default=os.environ.get("MW_OPEN", ""))
    ap.add_argument("--trace", default=os.environ.get("MW_TRACE", ""))
    args = ap.parse_args()
    if not args.trace or not os.path.exists(args.trace):
        print("INCONCLUSIVE: MW_TRACE 未开启（先在同一个 shell 里 set MW_TRACE=...）")
        return 2
    h = find_window()
    if not h:
        print("INCONCLUSIVE: 找不到 mw-shell 窗口")
        return 2
    if not focus_window(h):
        print("INCONCLUSIVE: 窗口拿不到前台，合成按键到不了应用")
        return 2
    if args.scene == "h3":
        return run_h3(args.trace)
    if not args.doc:
        print("INCONCLUSIVE: h4 需要 --doc（已保存到磁盘的 .zmind）")
        return 2
    return run_h4(args.trace, args.doc)


if __name__ == "__main__":
    sys.exit(main())
