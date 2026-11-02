# 用**已构建的** release 二进制跑 golden 全矩阵（不触发 cargo 重建）。
#
# 为什么需要它：`cargo golden` 每次调用都会检查源码新鲜度。仓库有并行的另一个会话在改
# `crates/mw-core`，一改就触发 4 分钟 release 重建（LTO），178 场 × 2 遍根本跑不完，
# 而且会把我验证的那一版代码换成对方未完成的半成品。这个脚本只跑二进制，跑的是
# **我交付的那一版**（`target/release/mw-shell.exe` 的 mtime 就是证据）。
#
# 场景表从正式门脚本里抽（只有一份环境表，不许复制），所以口径与门完全一致。
#
# 用法：
#   powershell -NoProfile -File scripts/golden-matrix.ps1                      # 只验证（两遍）
#   powershell -NoProfile -File scripts/golden-matrix.ps1 -Names a,b           # 只跑点名的场景
#   powershell -NoProfile -File scripts/golden-matrix.ps1 -Update -Names a,b   # 重拍 + 立刻复核
#   powershell -NoProfile -File scripts/golden-matrix.ps1 -Gate stage-cm
param(
    [ValidateSet('stage-b', 'stage-cm')][string]$Gate = 'stage-b',
    [string[]]$Names,
    [switch]$Update,
    [int]$Passes = 2,
    # 单场超时（毫秒）。harness 自己有 10s 超时，这里只防 cargo 之外的整场挂死。
    [int]$SceneTimeoutMs = 60000
)

$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$exe = Join-Path $root 'target/release/mw-shell.exe'
if (-not (Test-Path $exe)) { throw "没有 release 二进制：先 cargo build --release -p mw-shell" }
$exeTime = (Get-Item $exe).LastWriteTime.ToString('yyyy-MM-dd HH:mm:ss')
Write-Host "binary: $exe ($exeTime)"

# TEMP/TMP 与门脚本一致（受限令牌下系统 %TEMP% 拒写，漏了就是 56 条假红）。
$repoTemp = Join-Path $root 'target/tmp'
New-Item -ItemType Directory -Force -Path $repoTemp | Out-Null
$env:TEMP = (Resolve-Path $repoTemp).Path
$env:TMP = $env:TEMP

$gateFile = Join-Path $PSScriptRoot "x100-$Gate-gate.ps1"
$raw = Get-Content -Raw -Encoding UTF8 $gateFile
$m = [regex]::Match($raw, '(?s)\$scenes\s*=\s*(@\(.*?\r?\n\s*\))')
if (-not $m.Success) { throw "在 $gateFile 里找不到场景表" }
$scenes = Invoke-Expression $m.Groups[1].Value
# `-File` 传参时 `-Names a,b` 会当成**一个字符串**进来（PowerShell 的已知行为），
# 所以这里再按逗号/分号切一次；直接 `-Names a,b`（点源调用）时本来就已经是数组。
if ($Names -and $Names.Count -eq 1 -and $Names[0] -match '[,;]') {
    $Names = $Names[0] -split '[,;]'
}
if ($Names) {
    $picked = @()
    foreach ($n in $Names) {
        $hit = $scenes | Where-Object { $_.Name -eq $n }
        if (-not $hit) { throw "场景表里没有 $n" }
        $picked += $hit
    }
    $scenes = $picked
}
Write-Host "scenes: $($scenes.Count)（$Gate）  passes=$Passes  update=$Update"

function Invoke-Scene($scene, [bool]$update) {
    $app = Join-Path $root "target/ci-appdata/matrix-$($scene.Name)"
    $localApp = "$app-local"
    New-Item -ItemType Directory -Force -Path $app | Out-Null
    New-Item -ItemType Directory -Force -Path $localApp | Out-Null
    $saved = @{}
    $all = @{ APPDATA = $app; LOCALAPPDATA = $localApp }
    foreach ($k in $scene.Env.Keys) { $all[$k] = $scene.Env[$k] }
    foreach ($k in $all.Keys) {
        $saved[$k] = [Environment]::GetEnvironmentVariable($k, 'Process')
        [Environment]::SetEnvironmentVariable($k, [string]$all[$k], 'Process')
    }
    try {
        $argv = @('--golden', '--ppp', [string]$scene.Ppp, '--tol', '0', '--warmup', [string]$scene.Warmup,
            '--dir', [string]$scene.Dir)
        if ($update) { $argv += '--update' }
        $tmpOut = Join-Path $root 'target/tmp/matrix-scene.out'
        $p = Start-Process -FilePath $exe -ArgumentList $argv -NoNewWindow -Wait -PassThru `
            -RedirectStandardOutput $tmpOut -RedirectStandardError "$tmpOut.err"
        $text = ''
        if (Test-Path $tmpOut) { $text = Get-Content -Raw -Encoding UTF8 $tmpOut }
        if (Test-Path "$tmpOut.err") { $text += Get-Content -Raw -Encoding UTF8 "$tmpOut.err" }
        return [pscustomobject]@{ Code = $p.ExitCode; Out = $text }
    } finally {
        foreach ($k in $saved.Keys) {
            [Environment]::SetEnvironmentVariable($k, $saved[$k], 'Process')
        }
    }
}

$results = @()
$fail = @()
if ($Update) {
    Write-Host ""
    Write-Host "==> reshoot（--update，只写本次点名的场景）"
    foreach ($scene in $scenes) {
        $r = Invoke-Scene $scene $true
        $verdict = if ($r.Code -eq 0) { 'updated' } else { 'FAIL-update' }
        if ($r.Code -ne 0) { $fail += $scene.Name }
        Write-Host ("  {0,-28} {1}" -f $scene.Name, $verdict)
    }
}
foreach ($pass in 1..$Passes) {
    Write-Host ""
    Write-Host "==> pass $pass / $Passes"
    foreach ($scene in $scenes) {
        $sw = [System.Diagnostics.Stopwatch]::StartNew()
        $r = Invoke-Scene $scene $false
        $sw.Stop()
        $verdict = if ($r.Code -eq 0) { 'ok' } else { 'FAIL' }
        if ($r.Code -ne 0) {
            $fail += $scene.Name
            Write-Host ("  {0,-28} {1}  {2:N1}s" -f $scene.Name, $verdict, $sw.Elapsed.TotalSeconds)
            foreach ($line in ($r.Out -split "`r?`n")) {
                if ($line -match '像素超容差|FAIL|missing|缺失') { Write-Host "      $line" }
            }
        } else {
            Write-Host ("  {0,-28} {1}  {2:N1}s" -f $scene.Name, $verdict, $sw.Elapsed.TotalSeconds)
        }
        $results += [pscustomobject]@{ Pass = $pass; Scene = $scene.Name; Code = $r.Code }
    }
}

$logPath = Join-Path $root "target/x100/golden-matrix-$Gate.log"
$results | Format-Table -AutoSize | Out-String | Set-Content -LiteralPath $logPath -Encoding UTF8
Write-Host ""
Write-Host "binary=$exeTime scenes=$($scenes.Count) passes=$Passes"
if ($fail.Count -gt 0) {
    Write-Host ("FAIL {0} 次：{1}" -f $fail.Count, (($fail | Select-Object -Unique) -join ', '))
    exit 1
}
Write-Host "MATRIX PASS: $($scenes.Count) 场 × $Passes 遍零差异（$($scenes.Count * $Passes) 次比较）"
