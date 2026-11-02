param([string]$a,[string]$b)
Add-Type -AssemblyName System.Drawing
$ia=[System.Drawing.Bitmap]::new((Resolve-Path $a).Path); $ib=[System.Drawing.Bitmap]::new((Resolve-Path $b).Path)
$w=$ia.Width; $h=$ia.Height
for ($y0=0; $y0 -lt $h; $y0+=40) {
  $y1=[Math]::Min($y0+39,$h-1); $n=0; $minx=99999; $maxx=-1
  for ($y=$y0; $y -le $y1; $y++) { for ($x=0; $x -lt $w; $x+=1) {
    if ($ia.GetPixel($x,$y).ToArgb() -ne $ib.GetPixel($x,$y).ToArgb()) { $n++; if($x -lt $minx){$minx=$x}; if($x -gt $maxx){$maxx=$x} } } }
  if ($n -gt 0) { "y$y0-$y1 n=$n x$minx..$maxx" }
}
$ia.Dispose(); $ib.Dispose()
exit 0