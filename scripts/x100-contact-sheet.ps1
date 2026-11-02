param(
    [Parameter(Mandatory = $true)][string]$Left,
    [Parameter(Mandatory = $true)][string]$Right,
    [Parameter(Mandatory = $true)][string]$OutFile,
    [string]$LeftLabel = 'Xmind reference',
    [string]$RightLabel = 'MindWeaver actual',
    [int]$Width = 1800,
    [int]$Height = 1000
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Add-Type -AssemblyName System.Drawing

function Resolve-RepoPath([string]$path) {
    if ([IO.Path]::IsPathRooted($path)) { return $path }
    return Join-Path $root $path
}

function Draw-Fit($graphics, $image, [int]$x, [int]$y, [int]$w, [int]$h) {
    $scale = [Math]::Min($w / $image.Width, $h / $image.Height)
    $dw = [int]($image.Width * $scale)
    $dh = [int]($image.Height * $scale)
    $dx = $x + [int](($w - $dw) / 2)
    $dy = $y + [int](($h - $dh) / 2)
    $graphics.DrawImage($image, $dx, $dy, $dw, $dh)
}

$leftPath = Resolve-RepoPath $Left
$rightPath = Resolve-RepoPath $Right
$outPath = Resolve-RepoPath $OutFile
$leftImage = [System.Drawing.Image]::FromFile($leftPath)
$rightImage = [System.Drawing.Image]::FromFile($rightPath)
try {
    $bitmap = New-Object System.Drawing.Bitmap $Width, $Height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.Clear([System.Drawing.Color]::FromArgb(23, 26, 30))
        $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $font = New-Object System.Drawing.Font 'Segoe UI', 14, ([System.Drawing.FontStyle]::Bold)
        $small = New-Object System.Drawing.Font 'Segoe UI', 10
        $white = [System.Drawing.Brushes]::White
        $dim = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(185, 192, 200))
        try {
            $half = [int]($Width / 2)
            $top = 54
            $panelH = $Height - $top - 18
            Draw-Fit $graphics $leftImage 12 $top ($half - 24) $panelH
            Draw-Fit $graphics $rightImage ($half + 12) $top ($half - 24) $panelH
            $graphics.DrawString($LeftLabel, $font, $white, 18, 14)
            $graphics.DrawString("$($leftImage.Width)x$($leftImage.Height)", $small, $dim, 18, 34)
            $graphics.DrawString($RightLabel, $font, $white, ($half + 18), 14)
            $graphics.DrawString("$($rightImage.Width)x$($rightImage.Height)", $small, $dim, ($half + 18), 34)
            $graphics.DrawLine((New-Object System.Drawing.Pen ([System.Drawing.Color]::FromArgb(62, 68, 76), 1)), $half, 0, $half, $Height)
        } finally {
            $font.Dispose(); $small.Dispose(); $dim.Dispose()
        }
        $dir = Split-Path -Parent $outPath
        if ($dir) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
        $bitmap.Save($outPath, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $graphics.Dispose(); $bitmap.Dispose()
    }
} finally {
    $leftImage.Dispose(); $rightImage.Dispose()
}

Write-Host "contact-sheet=$OutFile"

