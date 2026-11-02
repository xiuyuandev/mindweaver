param([string]$path)
Add-Type -AssemblyName System.Drawing
$bmp = [System.Drawing.Bitmap]::new((Resolve-Path $path).Path)
$w = $bmp.Width; $h = $bmp.Height
# 只认「暗墨迹」：亮度 < 150 的像素，排除窗口衬边与浅灰分隔线
function Dark($c) { return (($c.R + $c.G + $c.B) / 3) -lt 150 }
$minX = $w; $maxX = -1; $minY = $h; $maxY = -1
for ($y = 0; $y -lt $h; $y++) {
  for ($x = 0; $x -lt $w; $x += 1) {
    $c = $bmp.GetPixel($x, $y)
    if (Dark $c) {
      if ($x -lt $minX) { $minX = $x }; if ($x -gt $maxX) { $maxX = $x }
      if ($y -lt $minY) { $minY = $y }; if ($y -gt $maxY) { $maxY = $y }
    }
  }
}
"dark_bbox: x=$minX..$maxX w=$($maxX-$minX+1) y=$minY..$maxY"
"logical: left=$([math]::Round($minX/1.5,1)) width=$([math]::Round(($maxX-$minX+1)/1.5,1)) top=$([math]::Round($minY/1.5,1))"
$bmp.Dispose()
exit 0