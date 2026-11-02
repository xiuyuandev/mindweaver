# 织思本地 CI：fmt -> clippy（基线守卫）-> test -> golden。
#
# 用法：
#   pwsh -File scripts/ci.ps1
#   pwsh -File scripts/ci.ps1 -SkipGolden     # 无 GPU/CI 机器
#   pwsh -File scripts/ci.ps1 -SkipClippy
#
# 退出码：0 = 全通过；非 0 = 第一个失败步骤的退出码。
param(
    [switch]$SkipGolden,
    [switch]$SkipClippy,
    # 按 CI 口径把所有 golden 基线重拍一遍（每个场景的环境变量/隔离 APPDATA 都走本文，避免手拍走样）。
    [switch]$Update,
    # 存量告警基线在 `scripts/clippy-baseline.txt`（一条告警一行指纹，形如
    # 「标题 @ 文件:行」）。新增指纹即失败；改代码时若确实新增了合理告警，
    # 明确把那一行加进基线文件，而不是把 `-D warnings` 关掉。
    #
    # 为什么不用「数字上限」：同一条告警会在 lib 与 lib test 两个 target 各打印一次，
    # 而 cargo 对没有重新检查的 crate 不重印告警——纯计数既被重复放大、又随缓存少报
    # （同一份代码实测报过 33 / 37 / 38）。指纹集合去重后不随缓存漂移。
    # 重新生成基线：powershell -File scripts/ci.ps1 -WriteClippyBaseline
    [switch]$WriteClippyBaseline
)

$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $PSScriptRoot

# 本机用户 Temp（%LOCALAPPDATA%\Temp）对测试进程**偶发拒写**（os error 5 拒绝访问），
# 会让所有写盘用例假失败（mw-shell 56 条 / mw-format 8 条）。测试与夹具一律落在仓库内，
# 这里显式把 TEMP/TMP 指过去——CI 的结论不该受本机 Temp 权限影响。
$scratch = Join-Path $root "target/tmp/ci-temp"
New-Item -ItemType Directory -Force -Path $scratch | Out-Null
$env:TEMP = $scratch
$env:TMP = $scratch
$upd = @()
if ($Update) { $upd = @('--update') }
Set-Location $root

# 测试会往 $env:TEMP 写临时目录（`.zmind` 往返用例）。在受限令牌下系统 Temp 可能不可写
# （本会话实测 `os error 5 拒绝访问`），测试就会「假失败」。统一把 TEMP/TMP 指到仓库内
# `target\tmp`：测试不再依赖用户 Temp 的权限，临时垃圾也留在 target 里。
$repoTemp = Join-Path $root 'target\tmp'
New-Item -ItemType Directory -Force -Path $repoTemp | Out-Null
$env:TEMP = (Resolve-Path $repoTemp).Path
$env:TMP = $env:TEMP

function Step([string]$name) {
    Write-Host ""
    Write-Host "==> $name" -ForegroundColor Cyan
}

function Need([string]$name) {
    if ($LASTEXITCODE -ne 0) {
        Write-Host "FAILED: $name (exit $LASTEXITCODE)" -ForegroundColor Red
        exit $LASTEXITCODE
    }
}

Step "cargo fmt --check"
cargo fmt --all -- --check
Need "fmt"

# 从 clippy 输出里抽「告警指纹」：标题 + 首个 `--> 文件:行`。
# 一条告警可能在多个 target 里重复打印，用指纹去重。
function Get-ClippyFingerprints([string]$text) {
    $lines = $text -split "`n"
    $out = New-Object System.Collections.Generic.List[string]
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -notmatch '^warning: ' -or $lines[$i] -match 'generated \d+ warning') { continue }
        # 增量编译锁文件删除失败（os error 5，见文件头注释）是环境噪声、不是 lint 告警，
        # 且它自身没有 `-->` 位置、会借用下一条真实告警的位置造出假指纹——跳过不计。
        if ($lines[$i] -match 'error deleting lock file') { continue }
        $title = ($lines[$i] -replace '^warning: ', '').Trim()
        $loc = ''
        for ($j = $i + 1; $j -lt [Math]::Min($i + 6, $lines.Count); $j++) {
            if ($lines[$j] -match '^\s*-->\s+(\S+):(\d+):') {
                $loc = ($Matches[1] -replace '\\', '/') + ':' + $Matches[2]
                break
            }
        }
        $out.Add("$title @ $loc")
    }
    return @($out | Sort-Object -Unique)
}

if (-not $SkipClippy) {
    Step "cargo clippy (fingerprint baseline: scripts/clippy-baseline.txt)"
    $clippy = (cargo clippy --workspace --all-targets 2>&1 | Out-String)
    $code = $LASTEXITCODE
    $current = Get-ClippyFingerprints $clippy
    $baselinePath = Join-Path $PSScriptRoot "clippy-baseline.txt"
    if ($code -ne 0) {
        Write-Host $clippy
        Write-Host "FAILED: clippy" -ForegroundColor Red
        exit $code
    }
    if ($WriteClippyBaseline) {
        $current | Out-File -Encoding utf8 $baselinePath
        Write-Host "已写入基线：$baselinePath（$($current.Count) 条）" -ForegroundColor Yellow
        exit 0
    }
    $baseline = @(Get-Content $baselinePath | Where-Object { $_.Trim() -ne "" } | Sort-Object -Unique)
    $added = @($current | Where-Object { $baseline -notcontains $_ })
    $gone = @($baseline | Where-Object { $current -notcontains $_ })
    Write-Host "clippy 告警：当前 $($current.Count) 条 / 基线 $($baseline.Count) 条 / 新增 $($added.Count)"
    if ($added.Count -gt 0) {
        $added | ForEach-Object { Write-Host "    新增：$_" -ForegroundColor Red }
        Write-Host "FAILED: clippy 出现基线里没有的告警（见上；确认合理后跑 -WriteClippyBaseline）" -ForegroundColor Red
        exit 1
    }
    if ($gone.Count -gt 0) {
        # 缓存只少报不多报；少报的消息不算问题，但值得提一句，免得基线越积越虚。
        Write-Host "    有 $($gone.Count) 条基线告警这次没重报（cargo 缓存），不算新增"
    }
}

Step "cargo test --workspace (serial, mirrors the X100 stage gate)"
cargo test --workspace -- --test-threads=1
Need "test"

if (-not $SkipGolden) {
    Step "cargo build --release -p mw-shell"
    cargo build --release -p mw-shell
    Need "release build"

    Step "cargo golden --tol 0 (ppp 1.0 / 1.5 / 2.0)"
    foreach ($ppp in @('1.0', '1.5', '2.0')) {
        cargo golden @upd --ppp $ppp --tol 0
        Need "golden ppp $ppp"
    }

    # 卡片库是整页视图：用临时 APPDATA + 固定示例卡片，绝不读用户真实笔记库。
    Step "cargo golden · library (ppp 1.0 / 1.5 / 2.0)"
    $oldAppData = $env:APPDATA
    $ciAppData = Join-Path $root 'target/ci-appdata/library'
    New-Item -ItemType Directory -Force -Path $ciAppData | Out-Null
    $env:APPDATA = $ciAppData
    $env:MW_LIBRARY_DEMO = '1'
    try {
        foreach ($ppp in @('1.0', '1.5', '2.0')) {
            cargo golden @upd --ppp $ppp --tol 0 --dir shots/golden/library
            Need "library golden ppp $ppp"
        }
    } finally {
        Remove-Item Env:MW_LIBRARY_DEMO -ErrorAction SilentlyContinue
        $env:APPDATA = $oldAppData
    }

    # 四主题矩阵：壳层基线只覆盖 snow，深色/雾青/暖砂各留一张 1.5 基线。
    # 任何按主题写死、只在一个主题下才对的颜色都会在这里被逐像素抓出来。
    Step "cargo golden - theme matrix (ink / mist / sand @ 1.5)"
    $themeAppData = Join-Path $root 'target/ci-appdata/theme'
    New-Item -ItemType Directory -Force -Path $themeAppData | Out-Null
    $env:APPDATA = $themeAppData
    try {
        foreach ($theme in @('ink', 'mist', 'sand')) {
            $env:MW_THEME = $theme
            cargo golden @upd --ppp 1.5 --tol 0 --dir "shots/golden/theme-$theme"
            Need "theme $theme golden"
        }
    } finally {
        Remove-Item Env:MW_THEME -ErrorAction SilentlyContinue
        $env:APPDATA = $oldAppData
    }

    # 大纲是整页视图：行尾角标（备注/卡片/标签/任务）也要有逐像素守护。
    Step "cargo golden - outline (ppp 1.5)"
    $outlineAppData = Join-Path $root 'target/ci-appdata/outline'
    New-Item -ItemType Directory -Force -Path $outlineAppData | Out-Null
    $env:APPDATA = $outlineAppData
    $env:MW_OUTLINE = '1'
    try {
        cargo golden @upd --ppp 1.5 --tol 0 --dir shots/golden/outline
        Need "outline golden"
    } finally {
        Remove-Item Env:MW_OUTLINE -ErrorAction SilentlyContinue
        $env:APPDATA = $oldAppData
    }

    # 聚焦子树与面包屑（S09）：表头层级、聚焦计数、行缩进都要逐像素守住。
    Step "cargo golden - outline focus (ppp 1.5)"
    $focusAppData = Join-Path $root 'target/ci-appdata/outline-focus'
    New-Item -ItemType Directory -Force -Path $focusAppData | Out-Null
    $env:APPDATA = $focusAppData
    $env:MW_OUTLINE = '1'
    $env:MW_OUTLINE_FOCUS = '1'
    try {
        cargo golden @upd --ppp 1.5 --tol 0 --dir shots/golden/outline-focus
        Need "outline focus golden"
    } finally {
        Remove-Item Env:MW_OUTLINE_FOCUS -ErrorAction SilentlyContinue
        Remove-Item Env:MW_OUTLINE -ErrorAction SilentlyContinue
        $env:APPDATA = $oldAppData
    }

    # 卡片斜杠菜单（S11）：斜杠行、菜单底板、选中行、八条命令逐像素守护。
    Step "cargo golden - card slash (ppp 1.5)"
    $slashAppData = Join-Path $root 'target/ci-appdata/card-slash'
    New-Item -ItemType Directory -Force -Path $slashAppData | Out-Null
    $env:APPDATA = $slashAppData
    $env:MW_CARD_DEMO = '1'
    $env:MW_CARD_SLASH = '1'
    try {
        cargo golden @upd --ppp 1.5 --tol 0 --dir shots/golden/card-slash
        Need "card slash golden"
    } finally {
        Remove-Item Env:MW_CARD_SLASH -ErrorAction SilentlyContinue
        Remove-Item Env:MW_CARD_DEMO -ErrorAction SilentlyContinue
        $env:APPDATA = $oldAppData
    }

    # 紧凑大纲（S10）：行高/角标/分组间距的密度档也要守住。
    Step "cargo golden - outline compact (ppp 1.5)"
    $compactAppData = Join-Path $root 'target/ci-appdata/outline-compact'
    New-Item -ItemType Directory -Force -Path $compactAppData | Out-Null
    $env:APPDATA = $compactAppData
    $env:MW_OUTLINE = '1'
    $env:MW_OUTLINE_DENSITY = 'compact'
    try {
        cargo golden @upd --ppp 1.5 --tol 0 --dir shots/golden/outline-compact
        Need "outline compact golden"
    } finally {
        Remove-Item Env:MW_OUTLINE_DENSITY -ErrorAction SilentlyContinue
        Remove-Item Env:MW_OUTLINE -ErrorAction SilentlyContinue
        $env:APPDATA = $oldAppData
    }

    # 选中态浮层（X15）：上下文工具条、缩放条与状态栏选中摘要。
    Step "cargo golden - selected canvas chrome (ppp 1.5)"
    $selectAppData = Join-Path $root 'target/ci-appdata/selected'
    New-Item -ItemType Directory -Force -Path $selectAppData | Out-Null
    $env:APPDATA = $selectAppData
    $env:MW_SELECT_DEMO = '1'
    try {
        cargo golden @upd --ppp 1.5 --tol 0 --dir shots/golden/selected
        Need "selected golden"
    } finally {
        Remove-Item Env:MW_SELECT_DEMO -ErrorAction SilentlyContinue
        $env:APPDATA = $oldAppData
    }

    # 菜单引擎（X16）：右键菜单 + 展开的「样式」子菜单，锁住行/快捷键列/子菜单几何。
    # warmup 8：菜单首帧到第 6 帧仍在做入场上色，3 帧会截到半透明中间态。
    Step "cargo golden - context menu (ppp 1.5)"
    $menuAppData = Join-Path $root 'target/ci-appdata/menu'
    New-Item -ItemType Directory -Force -Path $menuAppData | Out-Null
    $env:APPDATA = $menuAppData
    $env:MW_CONTEXT_DEMO = '1'
    try {
        cargo golden @upd --ppp 1.5 --tol 0 --warmup 8 --dir shots/golden/menu
        Need "context menu golden"
    } finally {
        Remove-Item Env:MW_CONTEXT_DEMO -ErrorAction SilentlyContinue
        $env:APPDATA = $oldAppData
    }

    # 窄窗右键菜单（X20 门后维护）：640×460 下节点菜单比视口高，锁住卡片夹高、
    # 内部滚动与 4px 可视滑块；「样式」子菜单同时验证窄窗左翻。
    Step "cargo golden - narrow context menu (ppp 1.5)"
    $menuNarrowAppData = Join-Path $root 'target/ci-appdata/menu-narrow'
    New-Item -ItemType Directory -Force -Path $menuNarrowAppData | Out-Null
    $oldWindowSize = $env:MW_WINDOW_SIZE
    $env:APPDATA = $menuNarrowAppData
    $env:MW_CONTEXT_DEMO = '1'
    $env:MW_WINDOW_SIZE = '640x460'
    try {
        cargo golden @upd --ppp 1.5 --tol 0 --warmup 8 --dir shots/golden/menu-narrow
        Need "narrow context menu golden"
    } finally {
        Remove-Item Env:MW_CONTEXT_DEMO -ErrorAction SilentlyContinue
        if ($null -eq $oldWindowSize) {
            Remove-Item Env:MW_WINDOW_SIZE -ErrorAction SilentlyContinue
        } else {
            $env:MW_WINDOW_SIZE = $oldWindowSize
        }
        $env:APPDATA = $oldAppData
    }
    # 统一弹窗（X17）：标题/内容/按钮区、暗幕与焦点环，锁住 dialog 壳的完整形态。
    # warmup 8：应用入场着色到第 6 帧才停，暗幕会把背景的逐帧变化放大成全局浅差。
    Step "cargo golden - unified dialog (ppp 1.5)"
    $dialogAppData = Join-Path $root 'target/ci-appdata/dialog'
    New-Item -ItemType Directory -Force -Path $dialogAppData | Out-Null
    $env:APPDATA = $dialogAppData
    $env:MW_DIALOG_DEMO = '1'
    try {
        cargo golden @upd --ppp 1.5 --tol 0 --warmup 8 --dir shots/golden/dialog
        Need "dialog golden"
    } finally {
        Remove-Item Env:MW_DIALOG_DEMO -ErrorAction SilentlyContinue
        $env:APPDATA = $oldAppData
    }

    # 开始页（X18）：层级、三条最近文件、模板网格与底部提示条的整页基线。
    Step "cargo golden - start page (ppp 1.0, 1440x1000)"
    $startAppData = Join-Path $root 'target/ci-appdata/start'
    New-Item -ItemType Directory -Force -Path $startAppData | Out-Null
    $env:APPDATA = $startAppData
    $env:MW_HOME = '1'
    $env:MW_START_DEMO = '1'
    $env:MW_WINDOW_SIZE = '1440x1000'
    try {
        cargo golden @upd --ppp 1.0 --tol 0 --dir shots/golden/start
        Need "start golden"
    } finally {
        Remove-Item Env:MW_START_DEMO -ErrorAction SilentlyContinue
        Remove-Item Env:MW_HOME -ErrorAction SilentlyContinue
        Remove-Item Env:MW_WINDOW_SIZE -ErrorAction SilentlyContinue
        $env:APPDATA = $oldAppData
    }

    # 开始页窄窗（X18）：紧凑空状态、两列模板与首屏不横向裁剪。
    Step "cargo golden - start page narrow (ppp 1.5, 720x620)"
    $startNarrowAppData = Join-Path $root 'target/ci-appdata/start-narrow'
    New-Item -ItemType Directory -Force -Path $startNarrowAppData | Out-Null
    $env:APPDATA = $startNarrowAppData
    $env:MW_HOME = '1'
    $env:MW_WINDOW_SIZE = '720x620'
    try {
        cargo golden @upd --ppp 1.5 --tol 0 --dir shots/golden/start-narrow
        Need "start narrow golden"
    } finally {
        Remove-Item Env:MW_HOME -ErrorAction SilentlyContinue
        Remove-Item Env:MW_WINDOW_SIZE -ErrorAction SilentlyContinue
        $env:APPDATA = $oldAppData
    }

    # 开始页滚动（X20 门后维护）：640x460 下把自绘滑块拖到底，锁住顶部渐隐与滑块贴底。
    Step "cargo golden - start page scrolled (ppp 1.5, 640x460)"
    $startScrollAppData = Join-Path $root 'target/ci-appdata/start-scroll'
    New-Item -ItemType Directory -Force -Path $startScrollAppData | Out-Null
    $env:APPDATA = $startScrollAppData
    $env:MW_HOME = '1'
    $env:MW_START_DEMO = '1'
    $env:MW_WINDOW_SIZE = '640x460'
    $env:MW_GOLDEN_INPUT = 'shots/golden/inputs/start-scroll.txt'
    try {
        cargo golden @upd --ppp 1.5 --tol 0 --warmup 16 --dir shots/golden/start-scroll
        Need "start scroll golden"
    } finally {
        Remove-Item Env:MW_START_DEMO -ErrorAction SilentlyContinue
        Remove-Item Env:MW_HOME -ErrorAction SilentlyContinue
        Remove-Item Env:MW_WINDOW_SIZE -ErrorAction SilentlyContinue
        Remove-Item Env:MW_GOLDEN_INPUT -ErrorAction SilentlyContinue
        $env:APPDATA = $oldAppData
    }

    # 开始页键盘焦点（X20 门后维护）：7 次 Tab 后焦点落在模板卡，页面必须
    # 自动滚到焦点卡完整可见，而不是把焦点环留在首屏之外。
    Step "cargo golden - start page tab focus (ppp 1.5, 640x460)"
    $startTabFocusAppData = Join-Path $root 'target/ci-appdata/start-tab-focus'
    New-Item -ItemType Directory -Force -Path $startTabFocusAppData | Out-Null
    $env:APPDATA = $startTabFocusAppData
    $env:MW_HOME = '1'
    $env:MW_START_DEMO = '1'
    $env:MW_WINDOW_SIZE = '640x460'
    $env:MW_GOLDEN_INPUT = 'shots/golden/inputs/start-tab-focus.txt'
    try {
        cargo golden @upd --ppp 1.5 --tol 0 --warmup 16 --dir shots/golden/start-tab-focus
        Need "start tab focus golden"
    } finally {
        Remove-Item Env:MW_START_DEMO -ErrorAction SilentlyContinue
        Remove-Item Env:MW_HOME -ErrorAction SilentlyContinue
        Remove-Item Env:MW_WINDOW_SIZE -ErrorAction SilentlyContinue
        Remove-Item Env:MW_GOLDEN_INPUT -ErrorAction SilentlyContinue
        $env:APPDATA = $oldAppData
    }

    # 标签条（X19）：三个内存标签、活动下划线、脏点、关闭叉与固定加号。
    Step "cargo golden - tab bar (ppp 1.5, 1080x720)"
    $tabsAppData = Join-Path $root 'target/ci-appdata/tabs'
    New-Item -ItemType Directory -Force -Path $tabsAppData | Out-Null
    $env:APPDATA = $tabsAppData
    $env:MW_TABS_DEMO = '1'
    $env:MW_WINDOW_SIZE = '1080x720'
    try {
        cargo golden @upd --ppp 1.5 --tol 0 --warmup 8 --dir shots/golden/tabs
        Need "tabs golden"
    } finally {
        Remove-Item Env:MW_TABS_DEMO -ErrorAction SilentlyContinue
        Remove-Item Env:MW_WINDOW_SIZE -ErrorAction SilentlyContinue
        $env:APPDATA = $oldAppData
    }

    # 标签条横向溢出（X20 门后维护）：9 个内存标签、640x460，Ctrl+Tab 五次后
    # 活动标签滚进视口中央，左右渐隐同时可见、加号不动。
    Step "cargo golden - tab bar overflow (ppp 1.5, 640x460)"
    $tabsOverflowAppData = Join-Path $root 'target/ci-appdata/tabs-overflow'
    New-Item -ItemType Directory -Force -Path $tabsOverflowAppData | Out-Null
    $env:APPDATA = $tabsOverflowAppData
    $env:MW_TABS_DEMO = '9'
    $env:MW_WINDOW_SIZE = '640x460'
    $env:MW_GOLDEN_INPUT = 'shots/golden/inputs/tabs-overflow.txt'
    try {
        cargo golden @upd --ppp 1.5 --tol 0 --warmup 30 --dir shots/golden/tabs-overflow
        Need "tabs overflow golden"
    } finally {
        Remove-Item Env:MW_TABS_DEMO -ErrorAction SilentlyContinue
        Remove-Item Env:MW_WINDOW_SIZE -ErrorAction SilentlyContinue
        Remove-Item Env:MW_GOLDEN_INPUT -ErrorAction SilentlyContinue
        $env:APPDATA = $oldAppData
    }

    # 标签条滚轮（X20 门后维护）：9 个内存标签、640x460，滚轮一格只滚 100px，
    # 不按标签数放大；横向溢出时左右渐隐可见、加号不动。
    Step "cargo golden - tab bar wheel (ppp 1.5, 640x460)"
    $tabsWheelAppData = Join-Path $root 'target/ci-appdata/tabs-wheel'
    New-Item -ItemType Directory -Force -Path $tabsWheelAppData | Out-Null
    $env:APPDATA = $tabsWheelAppData
    $env:MW_TABS_DEMO = '9'
    $env:MW_WINDOW_SIZE = '640x460'
    $env:MW_GOLDEN_INPUT = 'shots/golden/inputs/tabs-wheel.txt'
    try {
        cargo golden @upd --ppp 1.5 --tol 0 --warmup 16 --dir shots/golden/tabs-wheel
        Need "tabs wheel golden"
    } finally {
        Remove-Item Env:MW_TABS_DEMO -ErrorAction SilentlyContinue
        Remove-Item Env:MW_WINDOW_SIZE -ErrorAction SilentlyContinue
        Remove-Item Env:MW_GOLDEN_INPUT -ErrorAction SilentlyContinue
        $env:APPDATA = $oldAppData
    }

    # 标签条滚轮单位（X20 门后维护）：同一脚本改用 Line 单位模拟普通鼠标，
    # 三格 = 3 × 40 = 120px；Point（触控板）与 Line（滚轮）两条硬件路径都有基线。
    Step "cargo golden - tab bar wheel line units (ppp 1.5, 640x460)"
    $tabsWheelLineAppData = Join-Path $root 'target/ci-appdata/tabs-wheel-line'
    New-Item -ItemType Directory -Force -Path $tabsWheelLineAppData | Out-Null
    $env:APPDATA = $tabsWheelLineAppData
    $env:MW_TABS_DEMO = '9'
    $env:MW_WINDOW_SIZE = '640x460'
    $env:MW_GOLDEN_INPUT = 'shots/golden/inputs/tabs-wheel-lines.txt'
    try {
        cargo golden @upd --ppp 1.5 --tol 0 --warmup 16 --dir shots/golden/tabs-wheel-line
        Need "tabs wheel line golden"
    } finally {
        Remove-Item Env:MW_TABS_DEMO -ErrorAction SilentlyContinue
        Remove-Item Env:MW_WINDOW_SIZE -ErrorAction SilentlyContinue
        Remove-Item Env:MW_GOLDEN_INPUT -ErrorAction SilentlyContinue
        $env:APPDATA = $oldAppData
    }

    # 标签条输入隔离（X20 门后维护）：模态弹窗开着时，暗幕下的标签条不吃滚轮、
    # 也起不了重排；脚本同时做这两件事，基线应停在原位（第 1 个标签在左缘）。
    Step "cargo golden - tab bar input isolation behind modal (ppp 1.5, 640x460)"
    $tabsModalAppData = Join-Path $root 'target/ci-appdata/tabs-modal-input'
    New-Item -ItemType Directory -Force -Path $tabsModalAppData | Out-Null
    $env:APPDATA = $tabsModalAppData
    $env:MW_TABS_DEMO = '9'
    $env:MW_WINDOW_SIZE = '640x460'
    $env:MW_DIALOG_DEMO = '1'
    $env:MW_GOLDEN_INPUT = 'shots/golden/inputs/tabs-modal-input.txt'
    try {
        cargo golden @upd --ppp 1.5 --tol 0 --warmup 24 --dir shots/golden/tabs-modal-input
        Need "tabs modal input golden"
    } finally {
        Remove-Item Env:MW_TABS_DEMO -ErrorAction SilentlyContinue
        Remove-Item Env:MW_WINDOW_SIZE -ErrorAction SilentlyContinue
        Remove-Item Env:MW_DIALOG_DEMO -ErrorAction SilentlyContinue
        Remove-Item Env:MW_GOLDEN_INPUT -ErrorAction SilentlyContinue
        $env:APPDATA = $oldAppData
    }

    # 标题栏菜单下的画布点击归属（X20 门后维护）：开「文件」菜单后点画布节点，
    # 第一下只收起菜单，不能顺手选中；指针随后移开，基线应无选中环/工具条。
    Step "cargo golden - canvas click ownership under titlebar menu (ppp 1.5, 1080x720)"
    $menuCanvasAppData = Join-Path $root 'target/ci-appdata/menu-canvas-click'
    New-Item -ItemType Directory -Force -Path $menuCanvasAppData | Out-Null
    $oldWindowSize = $env:MW_WINDOW_SIZE
    $env:APPDATA = $menuCanvasAppData
    $env:MW_WINDOW_SIZE = '1080x720'
    $env:MW_GOLDEN_INPUT = 'shots/golden/inputs/menu-canvas-click.txt'
    try {
        cargo golden @upd --ppp 1.5 --tol 0 --warmup 24 --dir shots/golden/menu-canvas-click
        Need "menu canvas click golden"
    } finally {
        Remove-Item Env:MW_GOLDEN_INPUT -ErrorAction SilentlyContinue
        if ($null -eq $oldWindowSize) {
            Remove-Item Env:MW_WINDOW_SIZE -ErrorAction SilentlyContinue
        } else {
            $env:MW_WINDOW_SIZE = $oldWindowSize
        }
        $env:APPDATA = $oldAppData
    }

    # 正在打开（X19）：无按钮、不可 Esc 的加载模态，必须先完整画一帧。
    Step "cargo golden - opening (ppp 1.5, 960x680)"
    $openingAppData = Join-Path $root 'target/ci-appdata/opening'
    New-Item -ItemType Directory -Force -Path $openingAppData | Out-Null
    $env:APPDATA = $openingAppData
    $env:MW_OPENING_DEMO = '1'
    $env:MW_WINDOW_SIZE = '960x680'
    try {
        cargo golden @upd --ppp 1.5 --tol 0 --warmup 8 --dir shots/golden/opening
        Need "opening golden"
    } finally {
        Remove-Item Env:MW_OPENING_DEMO -ErrorAction SilentlyContinue
        Remove-Item Env:MW_WINDOW_SIZE -ErrorAction SilentlyContinue
        $env:APPDATA = $oldAppData
    }

    # 打开失败（X19）：文件名、完整路径、原因与四个动作（含最近列表移除）。
    Step "cargo golden - open failed (ppp 1.5, 960x680)"
    $openErrorAppData = Join-Path $root 'target/ci-appdata/open-error'
    New-Item -ItemType Directory -Force -Path $openErrorAppData | Out-Null
    $env:APPDATA = $openErrorAppData
    $env:MW_OPEN_FAILED = '1'
    $env:MW_WINDOW_SIZE = '960x680'
    try {
        cargo golden @upd --ppp 1.5 --tol 0 --warmup 8 --dir shots/golden/open-error
        Need "open failed golden"
    } finally {
        Remove-Item Env:MW_OPEN_FAILED -ErrorAction SilentlyContinue
        Remove-Item Env:MW_WINDOW_SIZE -ErrorAction SilentlyContinue
        $env:APPDATA = $oldAppData
    }

    # 窄窗外壳（X20）：640x460 下菜单折成「更多」、标题与状态栏不横向裁剪。
    # Home 先把画布适应到视口，几何漂移不会被缩放差异掩盖。
    Step "cargo golden - narrow shell (ppp 1.5, 640x460)"
    $shellNarrowAppData = Join-Path $root 'target/ci-appdata/shell-narrow'
    New-Item -ItemType Directory -Force -Path $shellNarrowAppData | Out-Null
    $env:APPDATA = $shellNarrowAppData
    $env:MW_WINDOW_SIZE = '640x460'
    $env:MW_GOLDEN_INPUT = 'shots/golden/inputs/home.txt'
    try {
        cargo golden @upd --ppp 1.5 --tol 0 --warmup 16 --dir shots/golden/shell-narrow
        Need "narrow shell golden"
    } finally {
        Remove-Item Env:MW_GOLDEN_INPUT -ErrorAction SilentlyContinue
        Remove-Item Env:MW_WINDOW_SIZE -ErrorAction SilentlyContinue
        $env:APPDATA = $oldAppData
    }

    # 深色紧凑外壳（X20）：墨夜主题 + 1080x720，锁住主题令牌在外壳各区的组合。
    Step "cargo golden - ink compact shell (ppp 1.5, 1080x720)"
    $shellInkAppData = Join-Path $root 'target/ci-appdata/shell-ink-compact'
    New-Item -ItemType Directory -Force -Path $shellInkAppData | Out-Null
    $env:APPDATA = $shellInkAppData
    $env:MW_THEME = 'ink'
    $env:MW_WINDOW_SIZE = '1080x720'
    $env:MW_GOLDEN_INPUT = 'shots/golden/inputs/home.txt'
    try {
        cargo golden @upd --ppp 1.5 --tol 0 --warmup 16 --dir shots/golden/shell-ink-compact
        Need "ink compact shell golden"
    } finally {
        Remove-Item Env:MW_GOLDEN_INPUT -ErrorAction SilentlyContinue
        Remove-Item Env:MW_WINDOW_SIZE -ErrorAction SilentlyContinue
        Remove-Item Env:MW_THEME -ErrorAction SilentlyContinue
        $env:APPDATA = $oldAppData
    }
}

Write-Host ""
Write-Host "CI OK" -ForegroundColor Green
exit 0
