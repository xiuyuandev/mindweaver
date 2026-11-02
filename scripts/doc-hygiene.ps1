# 文档体检：断链、过期断言、索引覆盖。
#
# 为什么要机器查：这个仓库换过 5 任开发/AI 接手，文档靠人手维护的结果是
# 「README 说门是红的」「手册写着已被删除的右侧目录」「02 里挂着两周前的状态表」。
# 这三类腐烂都能被规则挡住，不需要 judgement：
#
#   1. 相对链接必须指向存在的文件（排除 http/#/纯语法示例）；
#   2. 已知过期说法不许出现在现行文档（只允许留在 STATUS、archive、x100/log）；
#   3. 每一篇现行文档都必须在 docs/00 索引里出现，
#      每一篇归档文档都必须在 docs/archive/README.md 里出现（否则等于没有入口）。
#
# 用法：
#   powershell -File scripts/doc-hygiene.ps1
#   powershell -File scripts/doc-hygiene.ps1 -Verbose
#
# 退出码：0 = 干净；1 = 有问题（输出逐条列出）。
param(
    [switch]$Verbose
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$utf8 = New-Object System.Text.UTF8Encoding($false)
$problems = New-Object System.Collections.Generic.List[string]

function Read-Utf8([string]$path) {
    return [IO.File]::ReadAllText($path, $utf8)
}

# 「历史区」：这些文件可以保留旧说法/旧链接，因为它们记录的是当时的事实。
$historyPrefixes = @(
    'docs\archive\',
    'docs\x100\log\'
)

# 「冻结」的现行文件：仍留在现行目录、仍被大量代码注释按小节引用，但内容本身就是历史，
# 允许保留旧说法（2026-10-30：docs/13 就地冻结，理由见其头部与 docs/00 §6）。
$frozenFiles = @(
    'docs\13-纯Rust原生重构方案.md'
)

# 这些文件的「整顿记录」小节要引用旧说法才能记下「修过什么」，不参与过期断言扫描。
$truncateAt = @{
    'docs\00-文档索引.md' = '## 6.'
}

function Is-History([string]$rel) {
    foreach ($p in $historyPrefixes) {
        if ($rel.StartsWith($p, [StringComparison]::OrdinalIgnoreCase)) { return $true }
    }
    return $false
}

$live = @()
$live += Get-ChildItem (Join-Path $root 'docs') -File -Filter *.md
$live += Get-ChildItem (Join-Path $root 'docs\x100') -File -Filter *.md
$live += Get-ChildItem $root -File -Filter *.md
$all = $live + (Get-ChildItem (Join-Path $root 'docs\archive') -File -Filter *.md)

# ---- 1. 断链 ----
$linkCount = 0
foreach ($f in $all) {
    $rel = $f.FullName.Substring($root.Length + 1)
    $text = Read-Utf8 $f.FullName
    foreach ($m in [regex]::Matches($text, '\]\(([^)#\s]+)(#[^)]*)?\)')) {
        $target = $m.Groups[1].Value
        if ($target -match '^(https?:|mailto:|#)') { continue }
        # 纯语法示例：`[文字](链接)`、`](url)` 这类不是路径
        if ($target -notmatch '[./\\]') { continue }
        $linkCount++
        $decoded = [uri]::UnescapeDataString($target)
        $resolved = Join-Path $f.DirectoryName $decoded
        if (-not (Test-Path -LiteralPath $resolved)) {
            if (Is-History $rel) {
                if ($Verbose) { Write-Host "  [历史区，忽略] $rel -> $target" -ForegroundColor DarkGray }
            } else {
                $problems.Add("断链：$rel -> $target")
            }
        }
    }
}

# ---- 2. 已知过期说法 ----
# 每条：正则 + 为什么过期。新增规则时同步把「为什么」写在这——半年后才知道能不能删。
$staleRules = @(
    @{ Pattern = 'stage-b 门当前为红'; Why = 'stage-b 门 2026-10-28 已回绿；「红」会让人不敢开始工作' },
    @{ Pattern = '尚无 LICENSE'; Why = '仓库根已有 LICENSE / LICENSE-MIT / LICENSE-APACHE' },
    @{ Pattern = '三种自动布局'; Why = '现在是五种（放射/逻辑/组织/鱼骨/时间轴）' },
    @{ Pattern = '144MB'; Why = '仓库体积实测已到 600MB 量级，写死体积只会过期' },
    @{ Pattern = '900 passed'; Why = '测试数唯一真相源是 docs/STATUS.md §1，不要复制快照' },
    @{ Pattern = '760\s*px\s*栏宽|栏宽\s*760|右侧粘性目录'; Why = 'B2 的 CER-13 删掉了常驻右侧目录，版心 720（窄窗 640）' },
    @{ Pattern = 'F9[^\n]{0,12}检查器'; Why = '检查器合同键已改 Ctrl+Shift+I，F9 只是备用动作' },
    @{ Pattern = '正式发布前补齐'; Why = 'LICENSE / LICENSE-MIT / LICENSE-APACHE 已在仓库根，不要再说「待补」' },
    @{ Pattern = 'PLAN\s*§'; Why = 'PLAN.md 已是归档存根，逐轮记录只进 docs/x100/log/' }
)
foreach ($f in $live) {
    $rel = $f.FullName.Substring($root.Length + 1)
    # STATUS 允许写历史对照；历史区与冻结文件同理；其它现行文档不许出现
    if ($rel -ieq 'docs\STATUS.md') { continue }
    if (Is-History $rel) { continue }
    if ($frozenFiles -contains $rel) { continue }
    $lines = (Read-Utf8 $f.FullName) -split "`n"
    if ($truncateAt.ContainsKey($rel)) {
        $marker = $truncateAt[$rel]
        for ($j = 0; $j -lt $lines.Count; $j++) {
            if ($lines[$j].StartsWith($marker)) {
                if ($j -gt 0) { $lines = $lines[0..($j - 1)] }
                break
            }
        }
    }
    for ($i = 0; $i -lt $lines.Count; $i++) {
        foreach ($rule in $staleRules) {
            if ($lines[$i] -match $rule.Pattern) {
                $problems.Add("过期断言：$rel`:$($i + 1) 命中 /$($rule.Pattern)/ —— $($rule.Why)")
            }
        }
    }
}

# ---- 3. 索引覆盖 ----
$indexPath = Join-Path $root 'docs\00-文档索引.md'
$indexText = Read-Utf8 $indexPath
foreach ($f in Get-ChildItem (Join-Path $root 'docs') -File -Filter *.md) {
    $name = $f.Name
    if ($name -like '00-*') { continue }
    $stem = ($name -replace '\.md$', '')
    # 索引里可以写 `01`、`01-项目目标` 或带目录的文件名
    $short = ($name -split '-')[0]
    if ($indexText -notmatch [regex]::Escape($stem) -and $indexText -notmatch "``$short``") {
        $problems.Add("未进索引：docs/$name 没有出现在 docs/00-文档索引.md")
    }
}
$archiveIndex = Read-Utf8 (Join-Path $root 'docs\archive\README.md')
foreach ($f in Get-ChildItem (Join-Path $root 'docs\archive') -File -Filter *.md) {
    if ($f.Name -ieq 'README.md') { continue }
    if ($archiveIndex -notmatch [regex]::Escape($f.Name)) {
        $problems.Add("未进归档台账：docs/archive/$($f.Name) 没有出现在 archive/README.md")
    }
}

# ---- 结果 ----
if ($problems.Count -eq 0) {
    Write-Host "文档体检通过：$($all.Count) 篇 / $linkCount 条相对链接 / $($staleRules.Count) 条过期规则" -ForegroundColor Green
    exit 0
}
Write-Host "文档体检发现 $($problems.Count) 个问题：" -ForegroundColor Red
foreach ($p in ($problems | Sort-Object -Unique)) { Write-Host "  - $p" -ForegroundColor Yellow }
exit 1
