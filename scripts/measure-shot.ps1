param([string]$path)
Add-Type -AssemblyName System.Drawing
$path = [string]$path
$bmp = [System.Drawing.Bitmap]::new((Resolve-Path $path).Path)
"size=$($bmp.Width)x$($bmp.Height)"
# 1) 顶栏底边：在 x=60 处沿 y 找第一种与背景不同的颜色块
$x = 60
$bg = $bmp.GetPixel($x, 2)
$runs = @()
$prev = $null
for ($y = 0; $y -lt [Math]::Min(160, $bmp.Height); $y++) {
  $c = $bmp.GetPixel($x, $y)
  $key = "$($c.R),$($c.G),$($c.B)"
  if ($key -ne $prev) { $runs += "$y=$key"; $prev = $key }
}
"col60: " + ($runs -join ' ')
# 2) 正文左右边界：在正文中部横扫，找最左/最右非背景像素
$bg2 = $bmp.GetPixel(4, [int]($bmp.Height/2))
$midY = [int]($bmp.Height * 0.45)
$leftMost = -1; $rightMost = -1
for ($x2 = 0; $x2 -lt $bmp.Width; $x2++) {
  $c = $bmp.GetPixel($x2, $midY)
  if ($c.R -ne $bg2.R -or $c.G -ne $bg2.G -or $c.B -ne $bg2.B) { if ($leftMost -lt 0) { $leftMost = $x2 }; $rightMost = $x2 }
}
"row$midY ink: left=$leftMost right=$rightMost span=$($rightMost-$leftMost)"
$bmp.Dispose()
exit 0