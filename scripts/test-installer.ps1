#requires -Version 5.1
[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$PackageDirectory)
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$package = (Resolve-Path -LiteralPath $PackageDirectory).Path
& (Join-Path $PSScriptRoot 'verify-release-manifest.ps1') -ManifestPath (Join-Path $package 'Supervisor.release.json') | Out-Null
. (Join-Path $PSScriptRoot 'build-storage.ps1')
$storage = Enter-SupervisorBuild $projectRoot
$probeMutex = $null
try {
    $compiler = & (Join-Path $PSScriptRoot 'get-installer-compiler.ps1')
    $probeId = 'Supervisor.InstallerSmoke.' + [Guid]::NewGuid().ToString('N')
    $mutexName = 'Local\' + $probeId
    $installRoot = Join-Path $storage.Session 'installed'
    $args = @('/Qp', "/DPackageDirectory=$package", "/DOutputDirectory=$($storage.Session)", '/DReleaseVersion=0.1.0-test', '/DNumericVersion=0.1.0', '/DOutputBaseName=installer-test', "/DInstallerAppId=$probeId", "/DApplicationMutex=$mutexName")
    & $compiler @args (Join-Path $PSScriptRoot 'installer\Supervisor.iss')
    if ($LASTEXITCODE -ne 0) { throw 'Installer test compilation failed.' }
    $setup = Join-Path $storage.Session 'installer-test.exe'
    $installArgs = @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART','/SP-','/NOICONS','/TASKS=""',('/DIR="' + $installRoot + '"'))

    function Invoke-TestInstaller([string]$Executable, [string[]]$Arguments) {
        $resolved = (Resolve-Path -LiteralPath $Executable).Path
        if (-not $resolved.StartsWith($storage.Session + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Test executable escaped its owned build session.' }
        $process = Start-Process -FilePath $resolved -ArgumentList $Arguments -WindowStyle Hidden -PassThru
        if (-not $process.WaitForExit(60000)) { throw "Installer test process $($process.Id) timed out; inspect before cleanup." }
        return $process.ExitCode
    }

    $probeMutex = [Threading.Mutex]::new($false, $mutexName)
    if ((Invoke-TestInstaller $setup $installArgs) -eq 0 -or (Test-Path (Join-Path $installRoot 'Supervisor.exe'))) { throw 'Installer failed to block an active app.' }
    $probeMutex.Dispose()
    $probeMutex = $null
    if ((Invoke-TestInstaller $setup $installArgs) -ne 0) { throw 'Fresh per-user installation failed.' }

    $manifest = Get-Content -LiteralPath (Join-Path $package 'Supervisor.release.json') -Raw | ConvertFrom-Json
    foreach ($record in @($manifest.artifacts) + @($manifest.supportingFiles)) {
        if ((Get-FileHash -LiteralPath (Join-Path $installRoot $record.file) -Algorithm SHA256).Hash.ToLowerInvariant() -ne $record.sha256) { throw "Installed file differs: $($record.file)" }
    }
    # This synthetic file proves unknown content is not removed on update/uninstall.
    $sentinel = Join-Path $installRoot 'preserve-test-content.txt'
    [IO.File]::WriteAllText($sentinel, 'owned synthetic test content')
    if ((Invoke-TestInstaller $setup $installArgs) -ne 0) { throw 'In-place installer update failed.' }
    if ([IO.File]::ReadAllText($sentinel) -ne 'owned synthetic test content') { throw 'Update changed unrelated content.' }

    $uninstaller = Join-Path $installRoot 'unins000.exe'
    $uninstallArgs = @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART')
    $probeMutex = [Threading.Mutex]::new($false, $mutexName)
    if ((Invoke-TestInstaller $uninstaller $uninstallArgs) -eq 0 -or -not (Test-Path (Join-Path $installRoot 'Supervisor.exe'))) { throw 'Uninstaller failed to block an active app.' }
    $probeMutex.Dispose()
    $probeMutex = $null
    if ((Invoke-TestInstaller $uninstaller $uninstallArgs) -ne 0) { throw 'Uninstall failed.' }
    if (Test-Path (Join-Path $installRoot 'Supervisor.exe')) { throw 'Uninstall left the application installed.' }
    if ([IO.File]::ReadAllText($sentinel) -ne 'owned synthetic test content') { throw 'Uninstall removed unrelated content.' }
    if (Test-Path ('HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\' + $probeId + '_is1')) { throw 'Uninstall left its test registration.' }
    Write-Output 'Installer smoke passed: active-app guards, fresh install, exact payload, upgrade, uninstall, and preservation of unrelated content.'
} finally {
    if ($probeMutex) { $probeMutex.Dispose() }
    Exit-SupervisorBuild $storage
}
