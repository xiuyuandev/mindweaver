# Build a distributable Windows package for Zhisi (MindWeaver).
#
#   powershell -File scripts/package-exe.ps1                 # build + stage + zip
#   powershell -File scripts/package-exe.ps1 -SkipBuild      # reuse target\release
#   powershell -File scripts/package-exe.ps1 -Install        # ...and install it for the current user
#   powershell -File scripts/package-exe.ps1 -SelfExtract    # also try a single-file setup .exe
#
# Output:
#   dist\zhisi-v<ver>-win64\            portable folder (MindWeaver.exe + install.ps1)
#   dist\zhisi-v<ver>-win64.zip         same folder zipped
#   dist\zhisi-v<ver>-win64.zip.sha256
#   dist\zhisi-v<ver>-setup.exe         only with -SelfExtract (IExpress self-extracting)
param(
    [string]$Version = '',
    [switch]$SkipBuild,
    [switch]$Install,
    [switch]$SelfExtract,
    [switch]$NoDesktopShortcut
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

if (-not $Version) {
    $line = Select-String -Path (Join-Path $root 'Cargo.toml') -Pattern '^version\s*=\s*"([^"]+)"' |
        Select-Object -First 1
    if (-not $line) { throw 'workspace version not found' }
    $Version = $line.Matches[0].Groups[1].Value
}

if (-not $SkipBuild) {
    Write-Host '==> cargo build --release -p mw-shell'
    & cargo build --release -p mw-shell
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }
}

$exe = Join-Path $root 'target\release\mw-shell.exe'
if (-not (Test-Path $exe)) { throw "release exe missing: $exe" }

$dist = Join-Path $root 'dist'
$stage = Join-Path $dist "zhisi-v$Version-win64"
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Force $stage | Out-Null

# The installed name is MindWeaver.exe (install.ps1 and the uninstall entry key off it).
Copy-Item $exe (Join-Path $stage 'MindWeaver.exe')
foreach ($extra in @('README.md', 'LICENSE')) {
    $p = Join-Path $root $extra
    if (Test-Path $p) { Copy-Item $p $stage }
}
Copy-Item (Join-Path $root 'scripts\install.ps1') (Join-Path $stage 'install.ps1')

# Double-click installer: users should not have to type a command or fight the
# execution policy. ASCII only - a .cmd with non-ASCII gets decoded as ANSI.
$setupCmd = @'
@echo off
setlocal
echo Installing Zhisi (MindWeaver) for the current user...
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0install.ps1" %*
echo.
pause
'@
Set-Content -Path (Join-Path $stage 'setup.cmd') -Value $setupCmd -Encoding ASCII

# The Chinese quick-start lives in its own UTF-8 file: a non-ASCII .ps1 gets
# decoded as ANSI here, which eats the here-string terminator (hit this once).
$readmePath = Join-Path $root 'scripts\package-readme.txt'
$readme = Get-Content -LiteralPath $readmePath -Encoding UTF8 -Raw
# UTF-8 with BOM: Notepad on Windows 10+ and PowerShell 5.1 both read it correctly.
$utf8Bom = New-Object System.Text.UTF8Encoding($true)
[System.IO.File]::WriteAllText((Join-Path $stage 'README-zh.txt'), $readme, $utf8Bom)

$zip = Join-Path $dist "zhisi-v$Version-win64.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -CompressionLevel Optimal
$hash = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
"$hash  zhisi-v$Version-win64.zip" | Set-Content -Encoding ASCII "$zip.sha256"

$exeBytes = (Get-Item $exe).Length
$zipBytes = (Get-Item $zip).Length
"PACKAGE version=$Version exe_bytes=$exeBytes zip_bytes=$zipBytes zip_mb=$([Math]::Round($zipBytes / 1MB, 2))"
"PACKAGE folder=$stage"
"PACKAGE zip=$zip"
"PACKAGE sha256=$hash"

if ($SelfExtract) {
    # Single-file setup exe via IExpress (ships with Windows, no extra tooling).
    # IExpress is picky about non-ASCII paths, so stage a copy under %TEMP%.
    $sfxRoot = Join-Path $env:TEMP 'zhisi-sfx'
    if (Test-Path $sfxRoot) { Remove-Item $sfxRoot -Recurse -Force }
    New-Item -ItemType Directory -Force $sfxRoot | Out-Null
    Copy-Item (Join-Path $stage '*') $sfxRoot -Recurse -Force
    $setupCmd = "@echo off`r`npowershell -NoProfile -ExecutionPolicy Bypass -File `"%~dp0install.ps1`"`r`necho.`r`necho Done. Press any key to close.`r`npause >nul`r`n"
    Set-Content -Path (Join-Path $sfxRoot 'setup.cmd') -Value $setupCmd -Encoding ASCII -NoNewline

    $target = Join-Path $dist "zhisi-v$Version-setup.exe"
    if (Test-Path $target) { Remove-Item $target -Force }

    $files = Get-ChildItem $sfxRoot -File | Sort-Object Name
    $sed = New-Object System.Collections.Generic.List[string]
    $sed.Add('[Version]')
    $sed.Add('Class=IEXPRESS')
    $sed.Add('SEDVersion=3')
    $sed.Add('[Options]')
    $sed.Add('PackagePurpose=InstallApp')
    $sed.Add('ShowInstallProgramWindow=1')
    $sed.Add('HideExtractAnimation=0')
    $sed.Add('UseLongFileName=1')
    $sed.Add('InsideCompressed=0')
    $sed.Add('CAB_FixedSize=0')
    $sed.Add('CAB_ResvCodeSigning=0')
    $sed.Add('RebootMode=N')
    $sed.Add('InstallPrompt=')
    $sed.Add('DisplayLicense=')
    $sed.Add('FinishMessage=')
    $sed.Add("TargetName=$target")
    $sed.Add("FriendlyName=Zhisi MindWeaver $Version")
    $sed.Add('AppLaunched=cmd /c setup.cmd')
    $sed.Add('PostInstallCmd=<None>')
    $sed.Add('AdminQuietInstCmd=')
    $sed.Add('UserQuietInstCmd=')
    $sed.Add('SourceFiles=SourceFiles')
    $sed.Add('[Strings]')
    for ($i = 0; $i -lt $files.Count; $i++) {
        $sed.Add("FILE$i=$($files[$i].Name)")
    }
    $sed.Add('[SourceFiles]')
    $sed.Add("SourceFiles0=$sfxRoot\")
    $sed.Add('[SourceFiles0]')
    for ($i = 0; $i -lt $files.Count; $i++) {
        $sed.Add("%FILE$i%=")
    }
    $sedPath = Join-Path $sfxRoot 'zhisi.sed'
    Set-Content -Path $sedPath -Value $sed -Encoding ASCII

    Write-Host '==> iexpress /N zhisi.sed'
    & "$env:SystemRoot\System32\iexpress.exe" /N $sedPath | Out-Null
    Start-Sleep -Seconds 2
    if (Test-Path $target) {
        $sfxHash = (Get-FileHash $target -Algorithm SHA256).Hash.ToLower()
        "PACKAGE setup=$target"
        "PACKAGE setup_sha256=$sfxHash"
    } else {
        Write-Host 'WARN IExpress did not produce a setup exe; the portable folder + install.ps1 still work.'
    }
}

if ($Install) {
    Write-Host '==> installing for the current user'
    & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $stage 'install.ps1') -Source $stage
    if ($LASTEXITCODE -ne 0) { throw 'install failed' }
}
