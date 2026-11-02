# Acceptance probe: input methods, display topology, and a DPI capture matrix.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts/acceptance.ps1
#   powershell -ExecutionPolicy Bypass -File scripts/acceptance.ps1 -SkipCapture
#   powershell -ExecutionPolicy Bypass -File scripts/acceptance.ps1 -Ppp 1.25,1.5,2.0
#
# What is automated:
#   - installed keyboard layouts / IME tips (Get-WinUserLanguageList + HKL)
#   - monitor topology + per-monitor DPI
#   - four surfaces x three DPI scales, each rendered twice, SHA256 compared
#   - every capture must be 1620x1080; the library page must be fully opaque
# What is NOT automated (single-display or missing IMEs): see docs/19 manual checklist.
param(
    [switch]$SkipCapture,
    [string[]]$Ppp = @('1.25', '1.5', '2.0'),
    [string]$OutDir = 'shots/final/acceptance',
    [int]$Warmup = 6
)

$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$exe = Join-Path $root 'target/release/mw-shell.exe'
$report = New-Object System.Collections.Generic.List[string]
$failures = 0

function Say([string]$text) {
    Write-Host $text
    $report.Add($text)
}

# --- environment: displays -------------------------------------------------

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

public static class MwDisplay {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    [StructLayout(LayoutKind.Sequential)] public struct MONITORINFO {
        public int cbSize; public RECT rcMonitor; public RECT rcWork; public uint dwFlags;
    }
    public delegate bool MonitorEnumProc(IntPtr hMonitor, IntPtr hdc, ref RECT rect, IntPtr data);

    [DllImport("user32.dll")] public static extern bool EnumDisplayMonitors(IntPtr hdc, IntPtr clip, MonitorEnumProc proc, IntPtr data);
    [DllImport("user32.dll")] public static extern bool GetMonitorInfo(IntPtr hmon, ref MONITORINFO info);
    [DllImport("shcore.dll")] public static extern int GetDpiForMonitor(IntPtr hmon, int dpiType, out uint dpiX, out uint dpiY);
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);

    public static List<string> Scan() {
        var rows = new List<string>();
        EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero, delegate(IntPtr hmon, IntPtr hdc, ref RECT rect, IntPtr data) {
            var info = new MONITORINFO();
            info.cbSize = Marshal.SizeOf(typeof(MONITORINFO));
            GetMonitorInfo(hmon, ref info);
            uint dx = 0, dy = 0;
            int hr = GetDpiForMonitor(hmon, 0, out dx, out dy);
            int w = rect.Right - rect.Left;
            int h = rect.Bottom - rect.Top;
            string primary = (info.dwFlags & 1) != 0 ? "primary" : "secondary";
            rows.Add(string.Format("{0}x{1} at {2},{3} dpi={4}x{5} ({6:P0}) {7}",
                w, h, rect.Left, rect.Top, dx, dy, dx / 96.0, primary));
            return true;
        }, IntPtr.Zero);
        return rows;
    }
}
'@ -ErrorAction SilentlyContinue

[void][MwDisplay]::SetProcessDpiAwarenessContext([IntPtr](-4))
$monitors = [MwDisplay]::Scan()
$stamp = Get-Date -Format 'yyyy-MM-dd HH:mm:ss'
Say "# 织思验收报告（自动部分）"
Say ""
Say "- 生成时间：$stamp"
Say "- 可执行文件：$exe"
if (Test-Path $exe) {
    $fi = Get-Item $exe
    Say ("- release：{0:N0} B，SHA256 {1}" -f $fi.Length, (Get-FileHash $exe -Algorithm SHA256).Hash.Substring(0, 16))
} else {
    Say "- release：缺失（先运行 cargo build --release -p mw-shell）"
    $failures++
}
Say ""
Say "## 显示器"
Say ""
if ($monitors.Count -eq 0) {
    Say "- 未枚举到显示器（脚本无桌面会话？）"
} else {
    foreach ($m in $monitors) { Say "- $m" }
}
if ($monitors.Count -lt 2) {
    Say "- 双屏边界：**SKIP**（本机只有一个显示器；按 docs/19 人工清单在双屏机器上执行）"
} else {
    Say "- 双屏边界：检测到 $($monitors.Count) 台显示器；仍建议按 docs/19 走人工清单确认跨屏拖拽与候选窗位置"
}

# --- environment: input methods -------------------------------------------

Say ""
Say "## 输入法"
Say ""
$tips = @()
try {
    foreach ($lang in (Get-WinUserLanguageList)) {
        foreach ($tip in $lang.InputMethodTips) {
            $tips += [pscustomobject]@{ Lang = $lang.LanguageTag; Tip = $tip }
        }
    }
} catch {
    Say "- 读取 Get-WinUserLanguageList 失败：$($_.Exception.Message)"
}
$hkls = @()
try {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class MwKbd {
    [DllImport("user32.dll")] public static extern int GetKeyboardLayoutList(int n, IntPtr[] list);
}
'@ -ErrorAction SilentlyContinue
    $n = [MwKbd]::GetKeyboardLayoutList(0, $null)
    $arr = New-Object IntPtr[] $n
    [void][MwKbd]::GetKeyboardLayoutList($n, $arr)
    foreach ($h in $arr) { $hkls += ('0x{0:X8}' -f $h.ToInt64()) }
} catch {
    Say "- 读取键盘布局失败：$($_.Exception.Message)"
}
Say "- 语言清单（$($tips.Count) 个输入法条目）："
foreach ($t in $tips) { Say ("  - {0} · {1}" -f $t.Lang, $t.Tip) }
Say "- HKL：$($hkls -join ', ')"
if ($tips.Count -lt 5) {
    Say "- 五款输入法矩阵：**SKIP**（本机只注册 $($tips.Count) 个输入法；需要测试机安装五款后按 docs/19 人工清单签字）"
} else {
    Say "- 五款输入法矩阵：环境就绪（仍需人工逐款输入并签字）"
}

if ($SkipCapture) {
    Say ""
    Say "## DPI 矩阵"
    Say ""
    Say "- SKIP（-SkipCapture）"
} else {
    Say ""
    Say "## DPI 矩阵"
    Say ""
    Say "每个界面每档 DPI 渲染两次，逐文件 SHA256 比对；尺寸必须 1620x1080；卡片库整页必须不透明（alpha=255）。"
    Say ""
    Say "| 界面 | DPI | 尺寸 | 最小 alpha | 双跑 SHA256 | 结果 |"
    Say "| --- | --- | --- | --- | --- | --- |"

    if (-not (Test-Path $exe)) {
        Say ""
        Say "**FAIL**：没有 release 可执行文件。"
        exit 1
    }

    Add-Type -AssemblyName System.Drawing
    $surfaces = @(
        @{ Name = 'canvas'; Env = @{} },
        @{ Name = 'outline'; Env = @{ MW_OUTLINE = '1' } },
        @{ Name = 'library'; Env = @{ MW_LIBRARY_DEMO = '1' } },
        @{ Name = 'inspector'; Env = @{ MW_INSPECTOR = '1' } },
        @{ Name = 'card'; Env = @{ MW_CARD_DEMO = '1' } },
        @{ Name = 'card-slash'; Env = @{ MW_CARD_DEMO = '1'; MW_CARD_SLASH = '1' } },
        @{ Name = 'menu'; Env = @{ MW_CONTEXT_DEMO = '1' } },
        @{ Name = 'dialog'; Env = @{ MW_DIALOG_DEMO = '1' } }
    )
    $known = @('MW_OUTLINE', 'MW_OUTLINE_DENSITY', 'MW_LIBRARY_DEMO', 'MW_INSPECTOR', 'MW_CARD_DEMO', 'MW_CARD_SLASH', 'MW_SELECT_DEMO', 'MW_HOME', 'MW_THEME', 'MW_EMPTY_DEMO', 'MW_THEME_STUDIO', 'MW_CONTEXT_DEMO', 'MW_DIALOG_DEMO')
    $oldAppData = $env:APPDATA
    New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

    foreach ($surface in $surfaces) {
        foreach ($p in $Ppp) {
            # The harness names files with two decimals (1.5 -> shell-ppp1.50.png).
            $tag = ([double]$p).ToString('0.00', [Globalization.CultureInfo]::InvariantCulture)
            $dir = Join-Path $OutDir ("$($surface.Name)-p$tag")
            $hashes = @()
            $sizes = @()
            $alphas = @()
            $ok = $true
            foreach ($run in @(1, 2)) {
                $runDir = Join-Path $dir "run$run"
                New-Item -ItemType Directory -Force -Path $runDir | Out-Null
                $appData = Join-Path $root "target/acceptance-appdata/$($surface.Name)-$p-run$run"
                New-Item -ItemType Directory -Force -Path $appData | Out-Null
                foreach ($k in $known) { Remove-Item ("Env:" + $k) -ErrorAction SilentlyContinue }
                foreach ($k in $surface.Env.Keys) { Set-Item ("Env:" + $k) $surface.Env[$k] }
                $env:APPDATA = $appData
                $log = & $exe --golden --ppp $p --tol 0 --warmup $Warmup --dir $runDir --update 2>&1
                if ($LASTEXITCODE -ne 0) {
                    Say ""
                    Say "**FAIL** $($surface.Name) ppp $p run $run exit=$LASTEXITCODE"
                    $log | ForEach-Object { Say "    $_" }
                    $ok = $false
                    $failures++
                    continue
                }
                $png = Join-Path $runDir ("shell-ppp$tag.png")
                if (-not (Test-Path $png)) {
                    Say ""
                    Say "**FAIL** $($surface.Name) ppp $p run $run 没有输出 PNG"
                    $ok = $false
                    $failures++
                    continue
                }
                $bmp = [System.Drawing.Bitmap]::FromFile($png)
                $minA = 255
                for ($y = 0; $y -lt $bmp.Height; $y += 4) {
                    for ($x = 0; $x -lt $bmp.Width; $x += 4) {
                        $a = $bmp.GetPixel($x, $y).A
                        if ($a -lt $minA) { $minA = $a }
                    }
                }
                $sizes += "$($bmp.Width)x$($bmp.Height)"
                $bmp.Dispose()
                $alphas += $minA
                $hashes += (Get-FileHash $png -Algorithm SHA256).Hash
                if ($run -eq 1) {
                    Copy-Item $png (Join-Path $OutDir ("$($surface.Name)-p$tag.png")) -Force
                }
            }
            $env:APPDATA = $oldAppData
            $same = ($hashes.Count -eq 2 -and $hashes[0] -eq $hashes[1])
            $sizeOk = ($sizes.Count -eq 2 -and $sizes[0] -eq '1620x1080' -and $sizes[1] -eq '1620x1080')
            $alphaOk = ($surface.Name -ne 'library') -or ($alphas.Count -eq 2 -and $alphas[0] -eq 255 -and $alphas[1] -eq 255)
            if (-not ($same -and $sizeOk -and $alphaOk)) { $ok = $false; $failures++ }
            $hashText = if ($same) { '一致 ' + $hashes[0].Substring(0, 12) } else { '不一致' }
            $verdict = if ($ok) { 'PASS' } else { 'FAIL' }
            Say ("| {0} | {1} | {2} | {3} | {4} | {5} |" -f $surface.Name, $p, ($sizes -join '/'), ($alphas -join '/'), $hashText, $verdict)
        }
    }
    foreach ($k in $known) { Remove-Item ("Env:" + $k) -ErrorAction SilentlyContinue }

    Say ""
    Say "## 结论"
    Say ""
    if ($failures -eq 0) {
        Say "- DPI 矩阵全部 PASS；双跑 0 差异，尺寸与不透明度符合预期。"
    } else {
        Say "- **有 $failures 项失败**，见上表。"
    }
    Say "- 输入法逐款输入、跨屏候选窗位置等无法在本机自动化的项目：见 docs/19 人工验收清单。"
}

$reportPath = Join-Path $OutDir 'report.md'
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
Set-Content -Path $reportPath -Value ($report -join "`r`n") -Encoding UTF8
Write-Host ""
Write-Host "report -> $reportPath"
if ($failures -gt 0) { exit 1 }
exit 0
