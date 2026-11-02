$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$env:APPDATA = Join-Path $root 'target\qa-appdata'
$env:MW_OUTLINE = '1'
$env:MW_TRACE = Join-Path $root 'target\qa-trace.txt'
$p = Start-Process -FilePath (Join-Path $root 'target\release\mw-shell.exe') -PassThru
Start-Sleep -Seconds 4
$p.Id | Out-File -Encoding ascii (Join-Path $root 'target\qa-pid.txt')
