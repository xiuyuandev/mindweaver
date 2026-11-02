$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.IO.Compression.FileSystem

$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$zipPath = Join-Path $root 'dist\zhisi-v0.1.0-win64.zip'
$smoke = Join-Path $root 'target\package-smoke-final'
$appDir = Join-Path $smoke 'app'
$shotDir = Join-Path $root 'shots\final\package-smoke'

$zipHash = (Get-FileHash -Algorithm SHA256 $zipPath).Hash
"ZIP SHA256: $zipHash"
if ($zipHash -ne '070C4B4CFBDF29B7D109F709C41BD47D104D6BB0B629F7C22A692BA271E024E2') { throw 'zip hash mismatch' }

New-Item -ItemType Directory -Force -Path $smoke, $shotDir | Out-Null
if (Test-Path $appDir) { Remove-Item -Recurse -Force $appDir }
[System.IO.Compression.ZipFile]::ExtractToDirectory($zipPath, $appDir)

$zip = [System.IO.Compression.ZipFile]::OpenRead($zipPath)
$entries = @($zip.Entries)
"ZIP entries: $($entries.Count)"
if ($entries.Count -ne 10) { $zip.Dispose(); throw 'entry count mismatch' }
$exeEntry = $entries | Where-Object { $_.FullName -eq 'MindWeaver.exe' }
$sha = [System.Security.Cryptography.SHA256]::Create()
$entryHash = [BitConverter]::ToString($sha.ComputeHash($exeEntry.Open())).Replace('-', '')
$zip.Dispose()
"ZIP exe SHA256: $entryHash"

$exe = Join-Path $appDir 'MindWeaver.exe'
$fileHash = (Get-FileHash -Algorithm SHA256 $exe).Hash
"Extracted exe: $((Get-Item $exe).Length) B  SHA256 $fileHash"
if ($fileHash -ne '9C7B0C99A88BEC0E221F2A81E5AC6C3222CE191D76A5E54A76C767B276AAE5D8') { throw 'exe hash mismatch' }
if ($entryHash -ne $fileHash) { throw 'zip entry hash mismatch' }

$installer = Join-Path $appDir 'scripts\install.ps1'
$errs = $null
[System.Management.Automation.Language.Parser]::ParseFile($installer, [ref]$null, [ref]$errs) | Out-Null
if ($errs.Count -ne 0) { $errs | ForEach-Object { $_.Message }; throw 'install.ps1 parse errors' }
'INSTALL PARSE OK'

$env:APPDATA = Join-Path $smoke 'appdata'
$env:LOCALAPPDATA = Join-Path $smoke 'localappdata'
$env:MW_HOME = '0'
$env:MW_WINDOW_STATE = 'off'
$p = Start-Process -FilePath $exe -PassThru
$deadline = (Get-Date).AddSeconds(20)
while ((Get-Date) -lt $deadline) {
    Start-Sleep -Milliseconds 400
    $p.Refresh()
    if ($p.HasExited) { throw "packaged exe exited with $($p.ExitCode)" }
    if ($p.MainWindowHandle -ne [IntPtr]::Zero) { break }
}
if ($p.MainWindowHandle -eq [IntPtr]::Zero) { throw 'no main window within 20s' }
Start-Sleep -Seconds 3
Push-Location $root
& (Join-Path $root 'scripts\win-capture.ps1') -ProcessId $p.Id -OutFile 'shots\final\package-smoke\packaged-start.png'
Pop-Location
"SMOKE OK pid=$($p.Id) window=$($p.MainWindowHandle) shot=shots/final/package-smoke/packaged-start.png"
Start-Sleep -Milliseconds 500
Stop-Process -Id $p.Id -ErrorAction SilentlyContinue
$p.WaitForExit(5000) | Out-Null
'exited cleanly (terminated by audit)'
