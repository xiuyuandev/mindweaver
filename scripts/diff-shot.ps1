param([string]$a,[string]$b)
Add-Type -AssemblyName System.Drawing
$ia = [System.Drawing.Bitmap]::new((Resolve-Path $a).Path)
$ib = [System.Drawing.Bitmap]::new((Resolve-Path $b).Path)
$n = 0; $minX = 99999; $maxX = -1; $minY = 99999; $maxY = -1
for ($y = 0; $y -lt $ia.Height; $y++) {
  for ($x = 0; $x -lt $ia.Width; $x++) {
    $pa = $ia.GetPixel($x,$y); $pb = $ib.GetPixel($x,$y)
    if ($pa.ToArgb() -ne $pb.ToArgb()) { $n++
      if ($x -lt $minX) { $minX = $x }; if ($x -gt $maxX) { $maxX = $x }
      if ($y -lt $minY) { $minY = $y }; if ($y -gt $maxY) { $maxY = $y } }
  }
}
"diff_pixels=$n bbox=x$minX..$maxX y$minY..$maxY"
$ia.Dispose(); $ib.Dispose()
exit 0