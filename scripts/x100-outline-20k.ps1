# CER-11 · 大纲 20k 证据脚本（性能日志 + 三 DPI × 四主题矩阵）
#   powershell -NoProfile -File scripts/x100-outline-20k.ps1
# 产出：
#   target/x100/outline-20k-stats.log        运行时 stats（60 帧一行：UI p50/p95、wall fps）
#   shots/x100/CER-11/matrix/<theme>-<ppp>/  12 张 20k 大纲截图 + hashes.txt
# 预算（docs/28 §5.8）：首屏 ≤100ms、滚动 ≥55fps（UI p95 ≤16ms）、折叠/展开 ≤16ms。
param(
    [string]$Root = (Split-Path -Parent $PSScriptRoot)
)
$ErrorActionPreference = 'Stop'
Set-Location $Root

New-Item -ItemType Directory -Force -Path target/x100 | Out-Null
New-Item -ItemType Directory -Force -Path shots/x100/CER-11/matrix | Out-Null

function Set-Env([hashtable]$vars) {
    foreach ($k in $vars.Keys) {
        [Environment]::SetEnvironmentVariable($k, [string]$vars[$k], 'Process')
    }
}
function Clear-Env([string[]]$names) {
    foreach ($k in $names) {
        [Environment]::SetEnvironmentVariable($k, $null, 'Process')
    }
}

$isolation = @{
    APPDATA = (Join-Path $Root 'target/x100/20k-appdata')
    LOCALAPPDATA = (Join-Path $Root 'target/x100/20k-local')
}
New-Item -ItemType Directory -Force -Path $isolation.APPDATA, $isolation.LOCALAPPDATA | Out-Null

$statsLog = 'target/x100/outline-20k-stats.log'
Remove-Item $statsLog -ErrorAction SilentlyContinue
Set-Env ($isolation + @{
    MW_OUTLINE = '1'
    MW_OUTLINE_20K = '1'
    MW_WINDOW_SIZE = '1280x800'
    MW_STATS = '1'
    MW_STATS_LOG = (Join-Path $Root $statsLog)
    MW_GOLDEN_INPUT = 'shots/golden/inputs/outline-20k-scroll.txt'
})
Write-Host '[20k] 运行时 stats：滚动 200 帧'
cargo golden --ppp 1.0 --tol 0 --warmup 200 --dir target/x100/20k-perf --update | Out-Null
if ($LASTEXITCODE -ne 0) { Clear-Env @('MW_STATS','MW_STATS_LOG','MW_GOLDEN_INPUT'); Write-Host 'FAIL: stats run'; exit 1 }
Clear-Env @('MW_STATS','MW_STATS_LOG','MW_GOLDEN_INPUT')

if (Test-Path $statsLog) {
    Write-Host ('[20k] stats 末行：' + ((Get-Content $statsLog | Select-Object -Last 1)))
} else {
    Write-Host 'FAIL: stats log 未生成'; exit 1
}

$matrix = Join-Path $Root 'shots/x100/CER-11/matrix'
$rows = @()
foreach ($theme in @('snow', 'ink', 'mist', 'sand')) {
    foreach ($ppp in @('1.0', '1.5', '2.0')) {
        $dir = Join-Path $matrix "$theme-$ppp"
        New-Item -ItemType Directory -Force -Path $dir | Out-Null
        Set-Env ($isolation + @{
            MW_OUTLINE = '1'
            MW_OUTLINE_20K = '1'
            MW_WINDOW_SIZE = '1280x800'
            MW_THEME = $theme
        })
        cargo golden --ppp $ppp --tol 0 --warmup 6 --dir $dir --update | Out-Null
        $code = $LASTEXITCODE
        Clear-Env @('MW_THEME')
        if ($code -ne 0) { Write-Host "FAIL: matrix $theme @ $ppp"; exit 1 }
        $pppName = [double]::Parse($ppp, [Globalization.CultureInfo]::InvariantCulture).ToString('0.00', [Globalization.CultureInfo]::InvariantCulture)
        $png = Join-Path $dir "shell-ppp$pppName.png"
        $hash = (Get-FileHash $png -Algorithm SHA256).Hash.ToLower()
        $rows += "$theme`t$ppp`t$hash`t$png"
        Write-Host "[20k] matrix $theme @ $ppp -> $hash"
    }
}
Clear-Env @('MW_OUTLINE','MW_OUTLINE_20K','MW_WINDOW_SIZE')
$rows | Set-Content -Path (Join-Path $matrix 'hashes.txt') -Encoding UTF8
Write-Host '[20k] OK（12 张矩阵 + stats 日志）'
