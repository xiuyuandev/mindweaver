# Zhisi (MindWeaver) per-user installer / uninstaller.
#
#   powershell -ExecutionPolicy Bypass -File install.ps1            # install from this folder
#   powershell -ExecutionPolicy Bypass -File install.ps1 -Uninstall # remove everything
#   powershell -ExecutionPolicy Bypass -File install.ps1 -DryRun    # print what would happen
#
# No admin needed: everything lives under HKCU and %LOCALAPPDATA%.
# ASCII-only on purpose: PowerShell 5.1 mis-decodes UTF-8 .ps1 files without a BOM.
param(
    [string]$Source = '',
    [switch]$Uninstall,
    [switch]$DryRun,
    [switch]$NoDesktopShortcut
)

$ErrorActionPreference = 'Stop'
$appName = 'MindWeaver'
$displayName = 'Zhisi (MindWeaver)'
$version = '0.1.0'
$installDir = Join-Path $env:LOCALAPPDATA "Programs\$appName"
$progId = 'MindWeaver.zmind'
$uninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$appName"
$classes = 'HKCU:\Software\Classes'
$startMenu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs'
$desktop = [Environment]::GetFolderPath('Desktop')

function Say([string]$text) { Write-Host $text }

function Remove-Tree([string]$path) {
    if (-not (Test-Path $path)) { return }
    if ($DryRun) { Say "would remove $path"; return }
    Remove-Item $path -Recurse -Force -ErrorAction SilentlyContinue
    Say "removed $path"
}

function New-Shortcut([string]$linkPath, [string]$target, [string]$workDir, [string]$icon) {
    if ($DryRun) { Say "would create shortcut $linkPath"; return }
    $shell = New-Object -ComObject WScript.Shell
    $lnk = $shell.CreateShortcut($linkPath)
    $lnk.TargetPath = $target
    $lnk.WorkingDirectory = $workDir
    $lnk.IconLocation = "$icon,0"
    $lnk.Description = $displayName
    $lnk.Save()
    Say "shortcut $linkPath"
}

# ---------------------------------------------------------------- uninstall
if ($Uninstall) {
    # Only drop our own association; never steal .zmind from another app.
    $current = (Get-ItemProperty "$classes\.zmind" -ErrorAction SilentlyContinue).'(default)'
    if ($current -eq $progId) { Remove-Tree "$classes\.zmind" }
    Remove-Tree "$classes\$progId"
    Remove-Tree $uninstallKey
    Remove-Tree (Join-Path $startMenu "$appName.lnk")
    Remove-Tree (Join-Path $desktop "$appName.lnk")
    if (Test-Path $installDir) {
        if (-not $DryRun) {
            Get-Process $appName -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
            Start-Sleep -Milliseconds 300
        }
        Remove-Tree $installDir
    }
    Say 'UNINSTALL OK'
    exit 0
}

# ------------------------------------------------------------------ install
if (-not $Source) { $Source = $PSScriptRoot }
$sourceExe = Join-Path $Source "$appName.exe"
if (-not (Test-Path $sourceExe)) {
    Say "package not found: $sourceExe"
    Say 'Run this script from the unzipped package folder, or pass -Source <folder>.'
    exit 1
}

Say "installing $displayName $version"
Say "  from $Source"
Say "  to   $installDir"

if (-not $DryRun) {
    # Close a running copy first: Windows keeps a locked exe from being replaced.
    Get-Process $appName -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 300
    New-Item -ItemType Directory -Force $installDir | Out-Null
    Copy-Item (Join-Path $Source '*') $installDir -Recurse -Force
}

$exe = Join-Path $installDir "$appName.exe"

if (-not $DryRun) {
    # Keep a copy of this installer inside the install dir so the uninstall
    # registry entry never points at a folder the user has already deleted.
    Copy-Item $PSCommandPath (Join-Path $installDir 'install.ps1') -Force

    # Start menu + desktop shortcuts (per-user, no admin).
    New-Item -ItemType Directory -Force $startMenu | Out-Null
    New-Shortcut (Join-Path $startMenu "$appName.lnk") $exe $installDir $exe
    if (-not $NoDesktopShortcut) {
        New-Shortcut (Join-Path $desktop "$appName.lnk") $exe $installDir $exe
    }

    # .zmind association (per-user).
    New-Item -Path "$classes\.zmind" -Force | Out-Null
    Set-ItemProperty -Path "$classes\.zmind" -Name '(default)' -Value $progId
    New-Item -Path "$classes\$progId" -Force | Out-Null
    Set-ItemProperty -Path "$classes\$progId" -Name '(default)' -Value 'Zhisi concept map'
    New-Item -Path "$classes\$progId\DefaultIcon" -Force | Out-Null
    Set-ItemProperty -Path "$classes\$progId\DefaultIcon" -Name '(default)' -Value "$exe,0"
    New-Item -Path "$classes\$progId\shell\open\command" -Force | Out-Null
    Set-ItemProperty -Path "$classes\$progId\shell\open\command" -Name '(default)' -Value "`"$exe`" `"%1`""

    # Apps & Features entry.
    New-Item -Path $uninstallKey -Force | Out-Null
    Set-ItemProperty -Path $uninstallKey -Name 'DisplayName' -Value $displayName
    Set-ItemProperty -Path $uninstallKey -Name 'DisplayVersion' -Value $version
    Set-ItemProperty -Path $uninstallKey -Name 'Publisher' -Value 'Zhisi contributors'
    Set-ItemProperty -Path $uninstallKey -Name 'InstallLocation' -Value $installDir
    Set-ItemProperty -Path $uninstallKey -Name 'DisplayIcon' -Value "$exe,0"
    Set-ItemProperty -Path $uninstallKey -Name 'EstimatedSize' -Value ([int]((Get-Item $exe).Length / 1KB)) -Type DWord
    $uninstallCmd = "powershell -NoProfile -ExecutionPolicy Bypass -File `"$installDir\install.ps1`" -Uninstall"
    Set-ItemProperty -Path $uninstallKey -Name 'UninstallString' -Value $uninstallCmd
    Set-ItemProperty -Path $uninstallKey -Name 'QuietUninstallString' -Value $uninstallCmd
    Set-ItemProperty -Path $uninstallKey -Name 'NoModify' -Value 1 -Type DWord
    Set-ItemProperty -Path $uninstallKey -Name 'NoRepair' -Value 1 -Type DWord
}

Say ''
Say 'INSTALL OK'
Say "  exe        $exe"
Say "  start menu $startMenu\$appName.lnk"
Say "  uninstall  Settings > Apps, or: powershell -File `"$installDir\install.ps1`" -Uninstall"
exit 0
