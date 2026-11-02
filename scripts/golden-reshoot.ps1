<#
.SYNOPSIS
  重拍 golden 基线，并逐张量出「旧基线 -> 新基线」的漂移量。

.DESCRIPTION
  为什么要这个脚本：cargo golden 遇到第一处漂移就以 exit 1 退出，
  所以「CI 报了几处」永远只是冰山一角。直接 --update 重拍又会把旧图盖掉，
  事后没有参照物，只能盲拍（2026-10-27 那次就盲拍了 109 个场景）。

  本脚本的顺序是：先快照 -> 再按 CI 口径重拍（复用 ci.ps1 -Update，环境表只有一份）-> 最后逐张比。

.EXAMPLE
  powershell -NoProfile -File scripts/golden-reshoot.ps1            # 只量漂移，重拍前需要人工过目数字
  powershell -NoProfile -File scripts/golden-reshoot.ps1 -Keep      # 保留快照目录（默认也保留，便于复查）
#>
param(
    # 单张图漂移超过这个比例就高声报警：可能是真回归，不是基线陈旧。
    [double]$WarnPercent = 0.5,
    # 单通道最大差超过这个值也报警。
    [int]$WarnMax = 64,
    [switch]$SkipReshoot
)

$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
Add-Type -AssemblyName System.Drawing

$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$snap = Join-Path $root "target/golden-snapshot/$stamp"
New-Item -ItemType Directory -Force -Path $snap | Out-Null

function Get-Baselines {
    Get-ChildItem -Path 'shots/golden' -Recurse -Filter *.png |
        Where-Object { $_.FullName -notmatch '\\(actual|diff|inputs)\\' -and $_.Name -ne 'crop.png' }
}

Write-Host "==> 快照现有基线 -> $snap" -ForegroundColor Cyan
$before = @()
foreach ($f in Get-Baselines) {
    $rel = $f.FullName.Substring($root.Length + 1)
    $dst = Join-Path $snap ($rel -replace '\\', '_')
    Copy-Item $f.FullName $dst -Force
    $before += [pscustomobject]@{ Rel = $rel; Snap = $dst }
}
Write-Host "    快照 $($before.Count) 张"

if (-not $SkipReshoot) {
    Write-Host "==> 按 CI 口径重拍（复用 scripts/ci.ps1 -Update）" -ForegroundColor Cyan
    & powershell -NoProfile -File (Join-Path $PSScriptRoot 'ci.ps1') -Update -SkipClippy | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "    重拍本身失败（exit $LASTEXITCODE）：基线没改全，先修 CI" -ForegroundColor Red
        exit $LASTEXITCODE
    }
}

Write-Host "==> 对比" -ForegroundColor Cyan
$rows = @()
foreach ($b in $before) {
    $now = Join-Path $root $b.Rel
    if (-not (Test-Path $now)) { $rows += "$($b.Rel) MISSING"; continue }
    if ((Get-FileHash $b.Snap -Algorithm SHA256).Hash -eq (Get-FileHash $now -Algorithm SHA256).Hash) { continue }
    $ia = [System.Drawing.Bitmap]::new($b.Snap); $ib = [System.Drawing.Bitmap]::new($now)
    if ($ia.Width -ne $ib.Width -or $ia.Height -ne $ib.Height) {
        $rows += "$($b.Rel) SIZE $($ia.Width)x$($ia.Height)->$($ib.Width)x$($ib.Height)"
        $ia.Dispose(); $ib.Dispose(); continue
    }
    $n = 0; $mx = 0; $x0 = 999999; $y0 = 999999; $x1 = -1; $y1 = -1
    for ($y = 0; $y -lt $ia.Height; $y++) {
        for ($x = 0; $x -lt $ia.Width; $x++) {
            $p = $ia.GetPixel($x, $y); $q = $ib.GetPixel($x, $y)
            $d = [Math]::Max([Math]::Abs($p.R - $q.R), [Math]::Max([Math]::Abs($p.G - $q.G), [Math]::Abs($p.B - $q.B)))
            if ($d -gt 0) {
                $n++
                if ($d -gt $mx) { $mx = $d }
                if ($x -lt $x0) { $x0 = $x }; if ($x -gt $x1) { $x1 = $x }
                if ($y -lt $y0) { $y0 = $y }; if ($y -gt $y1) { $y1 = $y }
            }
        }
    }
    $w = $ia.Width; $h = $ia.Height; $total = $w * $h
    $ia.Dispose(); $ib.Dispose()
    $pct = [Math]::Round(100.0 * $n / $total, 4)
    $flag = ''
    if ($pct -gt $WarnPercent -or $mx -gt $WarnMax) { $flag = '  <== 超过阈值，先当回归查' }
    $rows += ("{0} diff={1} pct={2}% max={3} bbox={4}..{5},{6}..{7}{8}" -f $b.Rel, $n, $pct, $mx, $x0, $x1, $y0, $y1, $flag)
}

$outLog = 'target/x100/golden-reshoot-drift.txt'
New-Item -ItemType Directory -Force -Path 'target/x100' | Out-Null
if ($rows.Count -eq 0) {
    "$stamp：全部基线字节相同，重拍没有改动任何东西。" | Out-File -Encoding utf8 $outLog
    Write-Host '    0 张变化' -ForegroundColor Green
} else {
    $rows | Out-File -Encoding utf8 $outLog
    $rows | ForEach-Object { Write-Host "    $_" }
}
Write-Host "    快照保留在 $snap（复查用）"
Write-Host "    报告 $outLog"
exit 0