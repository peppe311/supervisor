#requires -Version 5.1
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$repository = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
. (Join-Path $PSScriptRoot 'build-storage.ps1')
. (Join-Path $PSScriptRoot 'frontend-cache.ps1')

function Assert-StorageProbe {
    param([bool]$Condition, [string]$Message)
    if (-not $Condition) { throw $Message }
}
function Assert-StorageRejection {
    param([scriptblock]$Action, [string]$Pattern)
    $failure = $null
    try { & $Action | Out-Null } catch { $failure = $_ }
    if (-not $failure -or $failure.Exception.Message -notlike $Pattern) { throw "Expected rejection '$Pattern'; received '$failure'." }
}
function Write-StorageProbeFile {
    param([string]$Path, [string]$Content)
    New-Item -ItemType Directory -Path (Split-Path $Path -Parent) -Force | Out-Null
    [IO.File]::WriteAllText($Path, $Content)
}

$outer = Enter-SupervisorBuild $repository
$fixture = Join-Path $outer.Session 'storage-probe'
$target = Join-Path $fixture 'target'
$context = $null
$junction = $null
try {
    Write-StorageProbeFile (Join-Path $fixture 'Cargo.toml') "[package]`nname = 'supervisor-storage-probe'`nversion = '0.0.0'`nedition = '2024'`n[workspace]`n[lib]`npath = 'lib.rs'`n"
    Write-StorageProbeFile (Join-Path $fixture 'lib.rs') ''
    $personal = Join-Path $fixture 'outputs\Supervisor\personal.txt'
    Write-StorageProbeFile $personal 'preserve this package'
    Assert-StorageRejection { Assert-SupervisorStoragePath $fixture (Join-Path $target '..\outputs') } '*inside the repository target*'
    Assert-StorageRejection { Remove-SupervisorBuildSession $fixture (Join-Path $target 'debug') } '*unowned build session*'

    $previousTemp = $env:TEMP
    $previousTmp = $env:TMP
    $previousTarget = $env:CARGO_TARGET_DIR
    $context = Enter-SupervisorBuild $fixture $target
    $sessionPath = $context.Session
    $temporaryFile = Join-Path $env:TEMP 'temporary.txt'
    Write-StorageProbeFile $temporaryFile 'test output'
    $gitProcess = Start-Process -FilePath (Get-Command git.exe).Source -ArgumentList @('-C', ('"' + $env:TEMP + '"'), 'rev-parse', '--is-inside-work-tree') -WindowStyle Hidden -Wait -PassThru
    Assert-StorageProbe ($gitProcess.ExitCode -ne 0) 'An empty temporary directory inherited the real repository Git state.'
    Assert-StorageRejection {
        $duplicateLock = [IO.File]::Open((Join-Path $context.Metadata 'build.lock'), [IO.FileMode]::Open, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
        $duplicateLock.Dispose()
    } '*'
    $nested = Enter-SupervisorBuild $fixture
    Exit-SupervisorBuild $nested
    Assert-StorageProbe (Test-Path -LiteralPath $temporaryFile) 'A nested build removed its caller temporary files.'
    Assert-StorageRejection {
        try { throw 'simulated build failure' } finally { Exit-SupervisorBuild $context }
    } '*simulated build failure*'
    $context = $null
    Assert-StorageProbe (-not (Test-Path -LiteralPath $sessionPath)) 'A failed build retained its disposable session.'
    Assert-StorageProbe ($env:TEMP -eq $previousTemp -and $env:TMP -eq $previousTmp -and $env:CARGO_TARGET_DIR -eq $previousTarget) 'The original process environment was not restored.'

    $deadSession = Join-Path $target ('.supervisor-build\sessions\' + [Guid]::NewGuid().ToString('N'))
    $liveSession = Join-Path $target ('.supervisor-build\sessions\' + [Guid]::NewGuid().ToString('N'))
    foreach ($entry in @(@($deadSession, [int]::MaxValue), @($liveSession, $PID))) {
        Write-StorageProbeFile (Join-Path $entry[0] 'owner.json') (@{ schema = 'supervisor.build-session'; pid = $entry[1] } | ConvertTo-Json)
        (Get-Item -LiteralPath $entry[0]).LastWriteTimeUtc = [DateTime]::UtcNow.AddDays(-8)
    }
    Clear-SupervisorAbandonedSessions $fixture
    Assert-StorageProbe (-not (Test-Path -LiteralPath $deadSession)) 'An expired abandoned session was retained.'
    Assert-StorageProbe (Test-Path -LiteralPath $liveSession) 'A live session was removed.'

    $external = Join-Path $fixture 'external-cache'
    Write-StorageProbeFile (Join-Path $external 'sentinel.txt') 'explicit override'
    $context = Enter-SupervisorBuild $fixture $external
    Assert-StorageProbe ($env:CARGO_TARGET_DIR -eq $external) 'An explicit cache override was ignored.'
    Exit-SupervisorBuild $context
    $context = $null
    Assert-StorageProbe (Test-Path -LiteralPath (Join-Path $external 'sentinel.txt')) 'An external cache was cleaned automatically.'

    $debugFile = Join-Path $target 'debug\generated.bin'
    $releaseFile = Join-Path $target 'release\generated.bin'
    Write-StorageProbeFile $debugFile ('x' * 65536)
    Write-StorageProbeFile $releaseFile ('y' * 8192)
    (Get-Item -LiteralPath $debugFile).LastWriteTimeUtc = [DateTime]::UtcNow.AddDays(-2)
    Invoke-SupervisorCacheBudget $fixture 1MB
    Assert-StorageProbe (Test-Path -LiteralPath $debugFile) 'A cache below budget was cleaned unnecessarily.'

    $junction = Join-Path $target 'debug\foreign-link'
    New-Item -ItemType Junction -Path $junction -Target $external | Out-Null
    Assert-StorageRejection { Invoke-SupervisorCacheBudget $fixture 16384 } '*link*'
    Assert-StorageProbe (Test-Path -LiteralPath (Join-Path $external 'sentinel.txt')) 'A linked directory was modified.'
    # Remove the link itself, never its target; both are within this exact fixture.
    [IO.Directory]::Delete($junction, $false)
    $junction = $null

    if (-not (Get-Process -Name cargo, rustc, rustdoc -ErrorAction SilentlyContinue)) {
        Invoke-SupervisorCacheBudget $fixture 16384
        Assert-StorageProbe (-not (Test-Path -LiteralPath $debugFile)) 'The oldest cache was not evicted above budget.'
        Assert-StorageProbe (Test-Path -LiteralPath $releaseFile) 'A recent cache was unnecessarily evicted.'
    } else { throw 'Run storage probes without another Rust build to verify real Cargo cache eviction.' }
    Assert-StorageProbe ((Get-Content -LiteralPath $personal -Raw) -eq 'preserve this package') 'Cache cleanup changed a package or user file.'
    Assert-StorageProbe (Test-Path -LiteralPath (Join-Path $target '.supervisor-build\build.lock')) 'Cache cleanup removed the build coordination lock.'

    foreach ($relative in @('ui\src\entry.ts', 'ui\package-lock.json', 'assets\mark.svg', 'scripts\build-frontend.ps1', 'scripts\frontend-cache.ps1')) {
        Write-StorageProbeFile (Join-Path $fixture $relative) 'initial source'
    }
    Write-StorageProbeFile (Join-Path $fixture 'ui\dist\central-agent-ui.js') 'compiled javascript'
    Write-StorageProbeFile (Join-Path $fixture 'ui\dist\central-agent-ui.css') 'compiled stylesheet'
    $stamp = Join-Path $target '.supervisor-build\frontend.json'
    $fingerprint = Get-SupervisorFrontendFingerprint $fixture
    Save-SupervisorFrontendCache $fixture $stamp $fingerprint
    Assert-StorageProbe (Test-SupervisorFrontendCache $fixture $stamp $fingerprint) 'Unchanged frontend cannot reuse its verified bundle.'
    $source = Join-Path $fixture 'ui\src\entry.ts'
    $originalTime = (Get-Item -LiteralPath $source).LastWriteTimeUtc
    Write-StorageProbeFile $source 'changed source'
    (Get-Item -LiteralPath $source).LastWriteTimeUtc = $originalTime
    $changedFingerprint = Get-SupervisorFrontendFingerprint $fixture
    Assert-StorageProbe (-not (Test-SupervisorFrontendCache $fixture $stamp $changedFingerprint)) 'A source change with the same timestamp reused a stale bundle.'
    Write-StorageProbeFile (Join-Path $fixture 'ui\dist\central-agent-ui.js') 'tampered output'
    Assert-StorageProbe (-not (Test-SupervisorFrontendCache $fixture $stamp $fingerprint)) 'Tampered output reused a stale bundle.'
    Write-StorageProbeFile $stamp '{invalid JSON'
    Assert-StorageProbe (-not (Test-SupervisorFrontendCache $fixture $stamp $fingerprint)) 'An invalid cache stamp was accepted.'
    Write-Output 'Build storage probes passed: nested/failing builds, lock, path/link boundaries, abandoned/live sessions, external cache preservation, real Cargo eviction and frontend invalidation.'
} finally {
    if ($junction -and (Test-Path -LiteralPath $junction)) { [IO.Directory]::Delete($junction, $false) }
    if ($context -and $context.Depth -gt 0) { Exit-SupervisorBuild $context }
    Exit-SupervisorBuild $outer
}
