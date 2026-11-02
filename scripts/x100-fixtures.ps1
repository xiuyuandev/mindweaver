param(
    [string]$OutDir = 'tests/fixtures/x100',
    [string]$PerfPath = 'target/x100-fixtures/perf-20k.zmind',
    [int]$PerfNodes = 20000
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

& cargo run -q -p mw-format --example x100_fixtures -- --out $OutDir --perf $PerfPath --perf-nodes $PerfNodes
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$files = @(
    (Join-Path $OutDir 'tiny-cn.zmind'),
    (Join-Path $OutDir 'complex-style.zmind'),
    $PerfPath
)

$rows = foreach ($rel in $files) {
    if (-not (Test-Path -LiteralPath $rel)) { throw "fixture missing: $rel" }
    $item = Get-Item -LiteralPath $rel
    [ordered]@{
        path = $rel.Replace('\', '/')
        bytes = $item.Length
        sha256 = (Get-FileHash -LiteralPath $rel -Algorithm SHA256).Hash
    }
}

$manifest = [ordered]@{
    generatedAt = (Get-Date).ToString('s')
    generator = 'crates/mw-format/examples/x100_fixtures.rs'
    files = $rows
}
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $OutDir 'manifest.json') -Encoding UTF8
Write-Host "fixture manifest -> $(Join-Path $OutDir 'manifest.json')"

