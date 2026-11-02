# CER-33 · 三界面 × 四主题 × 三 DPI 矩阵
#   powershell -ExecutionPolicy Bypass -File scripts/x100-cer33-matrix.ps1 [-Only outline|card|library]
# 产出：
#   shots/x100/CER-33/matrix/<surface>/<theme>-<ppp>/shell-ppp<0.00>.png
#   shots/x100/CER-33/matrix/<surface>/hashes.txt      theme / ppp / sha256 / path
# 说明：这是证据矩阵，不参与 stage-b 门；不改任何 shots/golden 基线。
param(
    [string]$Root = (Split-Path -Parent $PSScriptRoot),
    [ValidateSet('outline', 'card', 'library', 'all')]
    [string]$Only = 'all'
)
$ErrorActionPreference = 'Stop'
Set-Location $Root

$matrix = Join-Path $Root 'shots/x100/CER-33/matrix'
New-Item -ItemType Directory -Force -Path $matrix | Out-Null

# 夹具会写设置 / 窗口状态；隔离到仓库内，绝不碰真实用户目录。
$isolation = @{
    APPDATA = (Join-Path $Root 'target/x100/cer33-appdata')
    LOCALAPPDATA = (Join-Path $Root 'target/x100/cer33-local')
}
New-Item -ItemType Directory -Force -Path $isolation.APPDATA, $isolation.LOCALAPPDATA | Out-Null

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

$surfaces = @{
    outline = @{
        Env = @{ MW_OUTLINE = '1'; MW_OUTLINE_20K = '1'; MW_WINDOW_SIZE = '1280x800' }
        Warmup = 6
        Clear = @('MW_OUTLINE', 'MW_OUTLINE_20K', 'MW_WINDOW_SIZE')
    }
    card = @{
        Env = @{ MW_CARD_20K = '1'; MW_WINDOW_SIZE = '1200x1000' }
        Warmup = 12
        Clear = @('MW_CARD_20K', 'MW_WINDOW_SIZE')
    }
    library = @{
        Env = @{ MW_LIBRARY_DEMO = 'long'; MW_WINDOW_SIZE = '1280x800' }
        Warmup = 3
        Clear = @('MW_LIBRARY_DEMO', 'MW_WINDOW_SIZE')
    }
}

$order = if ($Only -eq 'all') { @('outline', 'card', 'library') } else { @($Only) }
$ppi = [Globalization.CultureInfo]::InvariantCulture

foreach ($surface in $order) {
    $spec = $surfaces[$surface]
    $dirRoot = Join-Path $matrix $surface
    New-Item -ItemType Directory -Force -Path $dirRoot | Out-Null
    $rows = @()
    foreach ($theme in @('snow', 'ink', 'mist', 'sand')) {
        foreach ($ppp in @('1.0', '1.5', '2.0')) {
            $dir = Join-Path $dirRoot "$theme-$ppp"
            New-Item -ItemType Directory -Force -Path $dir | Out-Null
            Set-Env ($isolation + $spec.Env + @{ MW_THEME = $theme })
            cargo golden --ppp $ppp --tol 0 --warmup $spec.Warmup --dir $dir --update | Out-Null
            $code = $LASTEXITCODE
            Clear-Env (@('MW_THEME') + $spec.Clear)
            if ($code -ne 0) { Write-Host "FAIL: $surface $theme @ $ppp (exit $code)"; exit 1 }
            $pppName = [double]::Parse($ppp, $ppi).ToString('0.00', $ppi)
            $png = Join-Path $dir "shell-ppp$pppName.png"
            if (-not (Test-Path $png)) { Write-Host "FAIL: 缺图 $png"; exit 1 }
            $hash = (Get-FileHash $png -Algorithm SHA256).Hash.ToLower()
            $rows += "$surface`t$theme`t$ppp`t$hash`t$png"
            Write-Host "[$surface] $theme @ $ppp -> $hash"
        }
    }
    $rows | Set-Content -Path (Join-Path $dirRoot 'hashes.txt') -Encoding UTF8
}

Write-Host "[CER-33] OK（$($order -join ' / ') 矩阵完成）"
