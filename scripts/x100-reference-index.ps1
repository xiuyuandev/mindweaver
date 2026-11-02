param(
    [string]$OutFile = 'shots/reference/xmind-26.5.1107/corpus.json'
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Add-Type -AssemblyName System.Drawing

$baseUri = [Uri]((Join-Path $root '') + [IO.Path]::DirectorySeparatorChar)
$sourceFiles = @(
    Get-ChildItem -Path (Join-Path $root 'shots/reference') -Recurse -File -Filter *.png -ErrorAction SilentlyContinue
    Get-ChildItem -Path (Join-Path $root 'shots/qa/xmind*.png') -File -ErrorAction SilentlyContinue
)
$items = foreach ($file in $sourceFiles) {
    $image = [System.Drawing.Image]::FromFile($file.FullName)
        try {
            $relative = [Uri]::UnescapeDataString($baseUri.MakeRelativeUri([Uri]$file.FullName).ToString()).Replace('\', '/')
            [PSCustomObject]([ordered]@{
                path = $relative
                bytes = $file.Length
                width = $image.Width
                height = $image.Height
                sha256 = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash
                modifiedAt = $file.LastWriteTime.ToString('s')
            })
        } finally {
            $image.Dispose()
        }
}

$unique = @($items | Sort-Object path -Unique)
$index = [ordered]@{
    product = 'Xmind'
    version = '26.5.1107.202609082009'
    generatedAt = (Get-Date).ToString('s')
    count = $unique.Count
    files = $unique
}
$out = Join-Path $root $OutFile
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $out) | Out-Null
$index | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $out -Encoding UTF8
Write-Host "reference index -> $OutFile ($($unique.Count) images)"

