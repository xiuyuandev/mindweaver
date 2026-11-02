# CER-12 · 大纲 9 场景 / 10 组证据脚本（G-B2-1 机器门）
#   powershell -NoProfile -File scripts/x100-cer12-outline-gate.ps1
# 产出：
#   target/x100/cer12-outline-gate.log        全程日志（双跑逐场景结果）
#   shots/x100/CER-12/hashes.txt              9 场景 10 组两轮哈希 + 一致判定
#   shots/x100/CER-12/matrix/<theme>-<ppp>/   outline-default 四主题 × 三 DPI
#   shots/x100/CER-12/narrow/<size>/          窄窗 800x600 / 640x460
# 约定：只读证据，不改产品代码；--update 只写本包目录，不回写 shots/golden/。
param(
    [string]$Root = (Split-Path -Parent $PSScriptRoot)
)
$ErrorActionPreference = 'Continue'
Set-Location $Root

$logDir = Join-Path $Root 'target/x100'
New-Item -ItemType Directory -Force -Path $logDir | Out-Null
$evidence = Join-Path $Root 'shots/x100/CER-12'
New-Item -ItemType Directory -Force -Path (Join-Path $evidence 'matrix') | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $evidence 'narrow') | Out-Null

$log = New-Object System.Collections.Generic.List[string]
function Say([string]$text) {
    Write-Host $text
    $log.Add($text)
}

function Png-Name([string]$ppp) {
    $v = [double]::Parse($ppp, [Globalization.CultureInfo]::InvariantCulture)
    return 'shell-ppp' + $v.ToString('0.00', [Globalization.CultureInfo]::InvariantCulture) + '.png'
}

function Invoke-Golden([string]$ppp, [int]$warmup, [string]$dir, [hashtable]$env, [switch]$Update) {
    $saved = @{}
    $all = @{}
    foreach ($k in $env.Keys) { $all[$k] = [string]$env[$k] }
    foreach ($k in $all.Keys) {
        $saved[$k] = [Environment]::GetEnvironmentVariable($k, 'Process')
        [Environment]::SetEnvironmentVariable($k, $all[$k], 'Process')
    }
    try {
        $argv = @('golden', '--ppp', $ppp, '--tol', '0', '--warmup', "$warmup", '--dir', $dir)
        if ($Update) { $argv += '--update' }
        $out = & cargo @argv 2>&1
        $code = [int]$LASTEXITCODE
        foreach ($line in $out) { Say ([string]$line) }
        return $code
    } finally {
        foreach ($k in $saved.Keys) {
            [Environment]::SetEnvironmentVariable($k, $saved[$k], 'Process')
        }
    }
}

function Isolate([string]$name) {
    $app = Join-Path $Root "target/x100/cer12-appdata/$name"
    $local = "$app-local"
    # 每次都从干净 profile 起跑：上一轮落盘的 index.json 会让布局差 1px，
    # 两遍就不是同一张图了（CER-12 实测，见 report.md §7）。
    Remove-Item -Recurse -Force $app, $local -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force -Path $app | Out-Null
    New-Item -ItemType Directory -Force -Path $local | Out-Null
    return @{ APPDATA = $app; LOCALAPPDATA = $local }
}

Say "CER-12 outline evidence gate"
Say "root=$Root"
Say "started=$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss zzz')"

# docs/28 §5.10 的 8 个验收场景。env 与 scripts/x100-stage-b-gate.ps1 保持一致，
# 否则同一张基线会因为窗口尺寸/密度不同而误报漂移。
$base = @{ MW_OUTLINE = '1' }
$scenes = @(
    @{ Name = 'outline-default'; Dir = 'shots/golden/outline'; Ppp = '1.5'; Warmup = 3
       Env = ($base + @{}) },
    @{ Name = 'outline-hover-select'; Dir = 'shots/golden/outline-hover-select'; Ppp = '1.5'; Warmup = 3
       Env = ($base + @{ MW_WINDOW_SIZE = '1200x720'; MW_GOLDEN_INPUT = 'shots/golden/inputs/outline-hover-select.txt' }) },
    @{ Name = 'outline-edit-ime'; Dir = 'shots/golden/outline-edit-ime'; Ppp = '1.5'; Warmup = 3
       Env = ($base + @{ MW_OUTLINE_EDIT = '纯 Rust 原生'; MW_GOLDEN_INPUT = 'shots/golden/inputs/outline-edit-ime.txt' }) },
    @{ Name = 'outline-focus'; Dir = 'shots/golden/outline-focus'; Ppp = '1.5'; Warmup = 3
       Env = ($base + @{ MW_OUTLINE_FOCUS = '1' }) },
    @{ Name = 'outline-search'; Dir = 'shots/golden/outline-search'; Ppp = '1.5'; Warmup = 6
       Env = ($base + @{ MW_OUTLINE_SEARCH = '排版'; MW_OUTLINE_FILTER_LABEL = '验收' }) },
    @{ Name = 'outline-drag'; Dir = 'shots/golden/outline-drag'; Ppp = '1.5'; Warmup = 3
       Env = ($base + @{ MW_GOLDEN_INPUT = 'shots/golden/inputs/outline-drag.txt' }) },
    @{ Name = 'outline-narrow'; Dir = 'shots/golden/outline-narrow'; Ppp = '1.5'; Warmup = 3
       Env = ($base + @{ MW_WINDOW_SIZE = '800x600' }) },
    @{ Name = 'outline-20k'; Dir = 'shots/golden/outline-20k'; Ppp = '1.0'; Warmup = 6
       Env = ($base + @{ MW_OUTLINE_20K = '1'; MW_WINDOW_SIZE = '1280x800' }) },
    # CER-12 补拍：docs/28 §5.2 非编辑态长文换行（不截断、每行 +24、悬挂缩进）。
    # 同一份夹具在 1.5/2.0 两档各留一张，证明换行按逻辑像素算、缩放只影响光栅。
    @{ Name = 'outline-wrap'; Dir = 'shots/golden/outline-wrap'; Ppp = '1.5'; Warmup = 6
       Env = ($base + @{ MW_OUTLINE_WRAP = '1'; MW_WINDOW_SIZE = '1280x800' }) },
    @{ Name = 'outline-wrap-2x'; Dir = 'shots/golden/outline-wrap'; Ppp = '2.0'; Warmup = 6
       Env = ($base + @{ MW_OUTLINE_WRAP = '1'; MW_WINDOW_SIZE = '1280x800' }) }
)

$failures = @()
$rows = @()
foreach ($scene in $scenes) {
    $png = Join-Path $scene.Dir (Png-Name $scene.Ppp)
    if (-not (Test-Path $png)) {
        Say "FAIL baseline missing: $png"
        $failures += "missing $($scene.Name)"
        continue
    }
    $hashes = @()
    foreach ($pass in 1..2) {
        Say "==> double-run $($scene.Name) pass $pass"
        $env = Isolate "gates-$($scene.Name)"
        foreach ($k in $scene.Env.Keys) { $env[$k] = $scene.Env[$k] }
        $code = Invoke-Golden $scene.Ppp $scene.Warmup $scene.Dir $env
        if ($code -ne 0) {
            Say "FAIL double-run $($scene.Name) pass $pass (exit $code)"
            $failures += "$($scene.Name) pass $pass"
        } else {
            Say "ok double-run $($scene.Name) pass $pass"
        }
        $hashes += (Get-FileHash $png -Algorithm SHA256).Hash.ToLower()
    }
    $same = if ($hashes[0] -eq $hashes[1]) { 'same' } else { 'DIFF' }
    if ($same -ne 'same') { $failures += "$($scene.Name) hash differs between passes" }
    $rows += "$($scene.Name)`t$($scene.Ppp)`t$($hashes[0])`t$($hashes[1])`t$same`t$png"
    Say ("record $($scene.Name) $($scene.Ppp) $($hashes[0]) / $($hashes[1]) -> $same")
}
$rows | Set-Content -Path (Join-Path $evidence 'hashes.txt') -Encoding UTF8

# outline-default 四主题 × 三 DPI（docs/28 §5.10 / §5.8）。
Say "==> matrix: outline-default x 4 themes x 3 DPI"
$mat = @()
foreach ($theme in @('snow', 'ink', 'mist', 'sand')) {
    foreach ($ppp in @('1.0', '1.5', '2.0')) {
        $dir = Join-Path $evidence "matrix/$theme-$ppp"
        New-Item -ItemType Directory -Force -Path $dir | Out-Null
        $env = Isolate "matrix-$theme-$($ppp -replace '\.','')"
        $env.MW_OUTLINE = '1'
        $env.MW_THEME = $theme
        $env.MW_WINDOW_SIZE = '1280x800'
        $code = Invoke-Golden $ppp 6 $dir $env -Update
        if ($code -ne 0) {
            Say "FAIL matrix $theme @ $ppp"
            $failures += "matrix $theme-$ppp"
            continue
        }
        $png = Join-Path $dir (Png-Name $ppp)
        $hash = (Get-FileHash $png -Algorithm SHA256).Hash.ToLower()
        # 新基线必须双跑：第二遍不带 --update，任何差异都算失败。
        $verify = Invoke-Golden $ppp 6 $dir $env
        if ($verify -ne 0) {
            Say "FAIL verify matrix $theme @ $ppp"
            $failures += "verify matrix $theme-$ppp"
        }
        $mat += "$theme`t$ppp`t$hash`t$png"
        Say ("matrix $theme @ $ppp -> $hash (verify=$(if ($verify -eq 0) { 'ok' } else { 'fail' }))")
    }
}
$mat | Set-Content -Path (Join-Path $evidence 'matrix\hashes.txt') -Encoding UTF8

# 窄窗两档（docs/28 §5.8：800×600 已有基线；640×460 为更紧的一档）。
Say "==> narrow windows: 800x600 / 640x460"
$narrow = @()
foreach ($size in @('800x600', '640x460')) {
    $dir = Join-Path $evidence "narrow/$size"
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    $env = Isolate "narrow-$size"
    $env.MW_OUTLINE = '1'
    $env.MW_WINDOW_SIZE = $size
    $code = Invoke-Golden '1.5' 3 $dir $env -Update
    if ($code -ne 0) {
        Say "FAIL narrow $size"
        $failures += "narrow $size"
        continue
    }
    $png = Join-Path $dir (Png-Name '1.5')
    $hash = (Get-FileHash $png -Algorithm SHA256).Hash.ToLower()
    $verify = Invoke-Golden '1.5' 3 $dir $env
    if ($verify -ne 0) {
        Say "FAIL verify narrow $size"
        $failures += "verify narrow $size"
    }
    $narrow += "$size`t$hash`t$png"
    Say ("narrow $size -> $hash (verify=$(if ($verify -eq 0) { 'ok' } else { 'fail' }))")
}
$narrow | Set-Content -Path (Join-Path $evidence 'narrow\hashes.txt') -Encoding UTF8

Say ""
if ($failures.Count -gt 0) {
    Say "CER-12 EVIDENCE FAIL: $($failures -join '; ')"
    $code = 1
} else {
    Say 'CER-12 EVIDENCE PASS: 9 scenes / 10 groups x 2 runs, 12 matrix shots, 2 narrow shots (tol 0)'
    Say 'G-B2-1 仍需人工八判词。'
    $code = 0
}
Say "finished=$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss zzz')"
$log | Set-Content -LiteralPath (Join-Path $logDir 'cer12-outline-gate.log') -Encoding UTF8
Write-Host "log=$(Join-Path $logDir 'cer12-outline-gate.log')"
exit $code
