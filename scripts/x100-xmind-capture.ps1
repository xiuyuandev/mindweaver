param(
    [ValidateSet('check', 'launch', 'capture')]
    [string]$Mode = 'check',
    [string]$OutFile = 'shots/reference/xmind-26.5.1107/capture.png',
    [int]$ProcessId = 0,
    [switch]$KeepOpen
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$installed = Join-Path $env:LOCALAPPDATA 'Programs/Xmind/Xmind.exe'
$expectedVersion = '26.5.1107.202609082009'
$existing = @(Get-Process -Name Xmind -ErrorAction SilentlyContinue)

if ($Mode -eq 'check') {
    Write-Host "installed=$installed"
    Write-Host "exists=$(Test-Path -LiteralPath $installed)"
    if (Test-Path -LiteralPath $installed) {
        $item = Get-Item -LiteralPath $installed
        Write-Host "version=$($item.VersionInfo.FileVersion)"
        Write-Host "sha256=$((Get-FileHash -LiteralPath $installed -Algorithm SHA256).Hash)"
    }
    Write-Host "runningXmindProcesses=$($existing.Count)"
    if ($existing.Count -gt 0) {
        Write-Host 'launchBlocked=true reason=existing-user-instance'
    }
    exit 0
}

if ($Mode -eq 'launch') {
    if (-not (Test-Path -LiteralPath $installed)) { throw 'Xmind executable not found' }
    if ($existing.Count -gt 0) {
        throw "Refusing to launch while $($existing.Count) Xmind processes are running. Close the user instance or use another isolated desktop."
    }
    $profile = Join-Path $root 'target/x100/xmind-isolated-profile'
    $appData = Join-Path $root 'target/x100/xmind-appdata'
    $localData = Join-Path $root 'target/x100/xmind-localappdata'
    New-Item -ItemType Directory -Force -Path $profile, $appData, $localData | Out-Null
    $oldAppData = $env:APPDATA
    $oldLocalData = $env:LOCALAPPDATA
    try {
        $env:APPDATA = $appData
        $env:LOCALAPPDATA = $localData
        $proc = Start-Process -FilePath $installed -ArgumentList @("--user-data-dir=$profile") -PassThru
    } finally {
        $env:APPDATA = $oldAppData
        $env:LOCALAPPDATA = $oldLocalData
    }
    $deadline = (Get-Date).AddSeconds(45)
    while ($proc.MainWindowHandle -eq [IntPtr]::Zero -and (Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 500
        $proc.Refresh()
    }
    if ($proc.MainWindowHandle -eq [IntPtr]::Zero) { throw 'Isolated Xmind did not expose a main window within 45s' }
    $meta = [ordered]@{
        processId = $proc.Id
        launchedAt = (Get-Date).ToString('s')
        profile = $profile
        appData = $appData
        localAppData = $localData
        instruction = 'Use a new blank document only. Do not open personal files.'
    }
    New-Item -ItemType Directory -Force -Path (Join-Path $root 'target/x100') | Out-Null
    $meta | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $root 'target/x100/xmind-isolated.json') -Encoding UTF8
    Write-Host "launched isolated Xmind pid=$($proc.Id)"
    if (-not $KeepOpen) {
        Write-Host "Use: powershell -File scripts/x100-xmind-capture.ps1 -Mode capture -ProcessId $($proc.Id)"
    }
    exit 0
}

if ($Mode -eq 'capture') {
    if ($ProcessId -le 0) {
        $metaPath = Join-Path $root 'target/x100/xmind-isolated.json'
        if (-not (Test-Path -LiteralPath $metaPath)) { throw 'No -ProcessId and no isolated launch metadata found' }
        $ProcessId = (Get-Content -Raw -Encoding UTF8 -LiteralPath $metaPath | ConvertFrom-Json).processId
    }
    $proc = Get-Process -Id $ProcessId -ErrorAction Stop
    if ($proc.ProcessName -ne 'Xmind') { throw "Process $ProcessId is not Xmind" }
    $capture = Join-Path $root $OutFile
    & (Join-Path $PSScriptRoot 'win-capture.ps1') -ProcessId $ProcessId -OutFile $OutFile
    $item = Get-Item -LiteralPath $installed
    $meta = [ordered]@{
        referenceVersion = $expectedVersion
        executableVersion = $item.VersionInfo.FileVersion
        executableSha256 = (Get-FileHash -LiteralPath $installed -Algorithm SHA256).Hash
        processId = $ProcessId
        capturedAt = (Get-Date).ToString('s')
        capture = $OutFile
        imageSha256 = (Get-FileHash -LiteralPath $capture -Algorithm SHA256).Hash
        note = 'Capture must depict an isolated blank/test document only.'
    }
    $sidecar = [IO.Path]::ChangeExtension($capture, '.json')
    $meta | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $sidecar -Encoding UTF8
    Write-Host "capture=$OutFile"
    Write-Host "metadata=$sidecar"
    if (-not $KeepOpen) {
        Stop-Process -Id $ProcessId -ErrorAction SilentlyContinue
        Write-Host "stopped isolated Xmind pid=$ProcessId"
    }
    exit 0
}

