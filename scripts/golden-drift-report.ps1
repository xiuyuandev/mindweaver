$ErrorActionPreference='Continue'
Add-Type -AssemblyName System.Drawing
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
$tmp = 'target/head-baseline'
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
$files = Get-ChildItem -Path 'shots/golden' -Recurse -Filter *.png | Where-Object { $_.FullName -notmatch '\\(actual|diff|inputs)\\' -and $_.Name -ne 'crop.png' }
foreach ($f in $files) {
  $rel = ($f.FullName.Substring($root.Length + 1) -replace '\\','/')
  $headPath = Join-Path $tmp ($rel -replace '/','_')
  if (Test-Path $headPath) { Remove-Item -Force $headPath }
  # `cmd /c` 重定向是字节安全的；PowerShell 管道会把二进制按字符串编码写坏。
  & cmd /c "git show HEAD:$rel > `"$headPath`" 2>nul"
  if (-not (Test-Path $headPath) -or (Get-Item $headPath).Length -lt 100) { "$rel NEW" ; continue }
  if ((Get-FileHash $headPath -Algorithm SHA256).Hash -eq (Get-FileHash $f.FullName -Algorithm SHA256).Hash) { "$rel SAME" ; continue }
  $ia=[System.Drawing.Bitmap]::new((Resolve-Path $headPath).Path); $ib=[System.Drawing.Bitmap]::new($f.FullName)
  if ($ia.Width -ne $ib.Width -or $ia.Height -ne $ib.Height) { "$rel SIZE $($ia.Width)x$($ia.Height)->$($ib.Width)x$($ib.Height)"; $ia.Dispose(); $ib.Dispose(); continue }
  $n=0; $mx=0; $x0=999999; $y0=999999; $x1=-1; $y1=-1
  for ($y=0; $y -lt $ia.Height; $y++) { for ($x=0; $x -lt $ia.Width; $x++) {
    $p=$ia.GetPixel($x,$y); $q=$ib.GetPixel($x,$y)
    $d=[Math]::Max([Math]::Abs($p.R-$q.R), [Math]::Max([Math]::Abs($p.G-$q.G), [Math]::Abs($p.B-$q.B)))
    if ($d -gt 0) { $n++; if ($d -gt $mx) { $mx=$d }; if($x -lt $x0){$x0=$x}; if($x -gt $x1){$x1=$x}; if($y -lt $y0){$y0=$y}; if($y -gt $y1){$y1=$y} } } }
  if ($n -eq 0) { "$rel SAME" } else { "$rel diff=$n max=$mx bbox=$x0..$x1,$y0..$y1" }
  $ia.Dispose(); $ib.Dispose()
}
exit 0