Add-Type -AssemblyName System.Drawing
$bmp = [System.Drawing.Bitmap]::FromFile((Join-Path (Get-Location) 'shots/final/s15-live/before.png'))
for ($y = 552; $y -le 585; $y++) {
    $hits = @()
    for ($x = 1400; $x -le 1510; $x++) {
        $c = $bmp.GetPixel($x, $y)
        $max = [Math]::Max($c.R, [Math]::Max($c.G, $c.B))
        $min = [Math]::Min($c.R, [Math]::Min($c.G, $c.B))
        if (($max - $min) -gt 25) {
            $hits += "$x=$($c.R),$($c.G),$($c.B)"
        }
    }
    if ($hits.Count -gt 0) { Write-Output "y=$y $($hits -join ' ')" }
}
$bmp.Dispose()
