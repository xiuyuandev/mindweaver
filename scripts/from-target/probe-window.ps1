$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
Set-Location $root
$smoke = Join-Path $root 'target\inspector-shot'
New-Item -ItemType Directory -Force -Path $smoke | Out-Null
$doc = Join-Path $smoke 'fixture.zmind'
Copy-Item -Force (Join-Path $root 'shots\qa\e5-timeline.zmind') $doc
$env:APPDATA = Join-Path $smoke 'appdata'
$env:LOCALAPPDATA = Join-Path $smoke 'localappdata'
$env:MW_HOME = '0'
$env:MW_OPEN = $doc
$env:MW_INSPECTOR = '1'
$env:MW_WINDOW_STATE = 'off'
$p = Start-Process -FilePath (Join-Path $root 'target\release\mw-shell.exe') -PassThru
Start-Sleep -Seconds 6
"pid=$($p.Id) exited=$($p.HasExited)"
if (-not $p.HasExited) {
  & (Join-Path $PSScriptRoot 'list-windows.ps1') -ProcessId $p.Id
  Stop-Process -Id $p.Id -ErrorAction SilentlyContinue
  $p.WaitForExit(5000) | Out-Null
}
