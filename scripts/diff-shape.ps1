param([string]$a,[string]$b,[int]$x0,[int]$y0,[int]$x1,[int]$y1)
Add-Type -AssemblyName System.Drawing
$ia=[System.Drawing.Bitmap]::new((Resolve-Path $a).Path); $ib=[System.Drawing.Bitmap]::new((Resolve-Path $b).Path)
$edge=0; $inner=0; $rows=@{}
for ($y=$y0; $y -le $y1; $y++) { for ($x=$x0; $x -le $x1; $x++) {
  if ($ia.GetPixel($x,$y).ToArgb() -ne $ib.GetPixel($x,$y).ToArgb()) {
    $dx=[Math]::Min($x-$x0, $x1-$x); $dy=[Math]::Min($y-$y0, $y1-$y)
    if ($dx -le 14 -and $dy -le 14) { $edge++ } else { $inner++ }
    $rows[$y] = 1 + $rows[$y] } } }
"corner_zone=$edge inner=$inner rows_with_diff=$($rows.Count) span_y=$($rows.Keys | Measure-Object -Minimum -Maximum | ForEach-Object { "$($_.Minimum)..$($_.Maximum)" })"
$ia.Dispose(); $ib.Dispose(); exit 0