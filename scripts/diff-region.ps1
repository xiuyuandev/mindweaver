param([string]$a,[string]$b,[int]$x0,[int]$y0,[int]$x1,[int]$y1)
Add-Type -AssemblyName System.Drawing
$ia = [System.Drawing.Bitmap]::new((Resolve-Path $a).Path)
$ib = [System.Drawing.Bitmap]::new((Resolve-Path $b).Path)
$n = 0
for ($y = $y0; $y -le $y1; $y++) { for ($x = $x0; $x -le $x1; $x++) {
  if ($ia.GetPixel($x,$y).ToArgb() -ne $ib.GetPixel($x,$y).ToArgb()) { $n++ } } }
$area = ($x1-$x0+1)*($y1-$y0+1)
"region_diff=$n of $area (x$x0..$x1 y$y0..$y1)"
$ia.Dispose(); $ib.Dispose()
exit 0