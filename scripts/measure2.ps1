param([string]$path)
Add-Type -AssemblyName System.Drawing
$bmp = [System.Drawing.Bitmap]::new((Resolve-Path $path).Path)
$w = $bmp.Width; $h = $bmp.Height
"size=${w}x${h}"
$bg = $bmp.GetPixel(2, 2)
function Ink($y) { $n = 0; for ($x = 0; $x -lt $w; $x += 2) { $c = $bmp.GetPixel($x, $y); if ($c.R -ne $bg.R -or $c.G -ne $bg.G -or $c.B -ne $bg.B) { $n++ } }; $n }
# 顶栏：逐行数墨迹（只数前 200 行），找最后一行有字的 y
$lastInk = 0
for ($y = 0; $y -lt 200; $y++) { if ((Ink $y) -gt 0) { $lastInk = $y } }
"topbar_last_ink_y=$lastInk"
# 正文包围盒：y>150 的所有非背景像素
$minX = $w; $maxX = -1; $minY = $h; $maxY = -1
for ($y = 150; $y -lt $h - 1; $y += 1) {
  for ($x = 0; $x -lt $w; $x += 2) {
    $c = $bmp.GetPixel($x, $y)
    if ($c.R -ne $bg.R -or $c.G -ne $bg.G -or $c.B -ne $bg.B) {
      if ($x -lt $minX) { $minX = $x }; if ($x -gt $maxX) { $maxX = $x }
      if ($y -lt $minY) { $minY = $y }; if ($y -gt $maxY) { $maxY = $y }
    }
  }
}
"body_bbox: x=$minX..$maxX (w=$($maxX-$minX)) y=$minY..$maxY"
"logical: left=$([math]::Round($minX/1.5,1)) width=$([math]::Round(($maxX-$minX)/1.5,1))"
$bmp.Dispose()
exit 0