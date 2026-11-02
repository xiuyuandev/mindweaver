param([string]$a,[string]$b,[int]$x0,[int]$y0,[int]$x1,[int]$y1)
Add-Type -AssemblyName System.Drawing
$ia=[System.Drawing.Bitmap]::new((Resolve-Path $a).Path); $ib=[System.Drawing.Bitmap]::new((Resolve-Path $b).Path)
$pairs=@{}
for ($y=$y0; $y -le $y1; $y++) { for ($x=$x0; $x -le $x1; $x++) {
  $p=$ia.GetPixel($x,$y).ToArgb(); $q=$ib.GetPixel($x,$y).ToArgb()
  if ($p -ne $q) { $k=("{0:X6} -> {1:X6}" -f ($p -band 0xFFFFFF), ($q -band 0xFFFFFF)); $pairs[$k] = 1 + $pairs[$k] } } }
$pairs.GetEnumerator() | Sort-Object -Property Value -Descending | Select-Object -First 6 | ForEach-Object { ($_.Key + "  x" + $_.Value) }
$ia.Dispose(); $ib.Dispose(); exit 0