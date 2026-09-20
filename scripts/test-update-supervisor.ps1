#requires -Version 5.1
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$manifestVerifier = Join-Path $PSScriptRoot 'verify-release-manifest.ps1'
$tokens = $null
$parseErrors = $null
$ast = [Management.Automation.Language.Parser]::ParseFile(
    (Join-Path $PSScriptRoot 'update-supervisor.ps1'), [ref]$tokens, [ref]$parseErrors)
if ($parseErrors.Count -gt 0) { throw 'The local updater has PowerShell syntax errors.' }
$helpers = $ast.FindAll({ param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] }, $false)
. ([scriptblock]::Create(($helpers | ForEach-Object { $_.Extent.Text }) -join "`n"))
$updaterSource = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'update-supervisor.ps1') -Raw
foreach ($required in @(
    'function Stop-CanonicalSupervisor',
    'Get-CimInstance Win32_Process -Filter "ProcessId = $($Process.Id)"',
    'Stop-Process -InputObject $verified -Force',
    'function Start-CanonicalSupervisorIndependent',
    'New-ScheduledTaskAction -Execute $ApplicationPath',
    'Unregister-ScheduledTask -TaskName $taskName'
)) {
    if (-not $updaterSource.Contains($required)) {
        throw "The updater is missing its guarded desktop lifecycle behavior: $required"
    }
}

. (Join-Path $PSScriptRoot 'build-storage.ps1')
$storage = Enter-SupervisorBuild $projectRoot
$probeRoot = Join-Path $storage.Session 'update-probe'
$outputRoot = Join-Path $probeRoot 'app'
$stagingRoot = Join-Path $probeRoot 'staging'
$backupRoot = Join-Path $probeRoot 'rollback'
$payloadNames = @('Supervisor.exe', 'Supervisor.dependencies.json',
    'LICENSE.txt', 'THIRD_PARTY_NOTICES.md', 'Supervisor.release.json')

function New-ProbePackage {
    param([string]$Path, [string]$Content)
    New-Item -ItemType Directory -Path $Path | Out-Null
    $records = @(foreach ($name in $payloadNames | Where-Object { $_ -ne 'Supervisor.release.json' }) {
        $filePath = Join-Path $Path $name
        [IO.File]::WriteAllText($filePath, "$Content $name")
        [ordered]@{ file = $name; bytes = (Get-Item -LiteralPath $filePath).Length;
            sha256 = (Get-FileHash -LiteralPath $filePath -Algorithm SHA256).Hash.ToLowerInvariant() }
    })
    [ordered]@{
        schema = 'central-agent.release-manifest'; schemaVersion = 2; launcher = 'Supervisor.exe'
        artifacts = @($records | Select-Object -First 1)
        supportingFiles = @($records | Select-Object -Skip 1)
    } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $Path 'Supervisor.release.json') -Encoding utf8
}

function Assert-ProbeFailure {
    param([scriptblock]$Action, [string]$Message)
    $failure = $null
    try { & $Action } catch { $failure = $_ }
    if ($null -eq $failure -or $failure.Exception.Message -notlike $Message) {
        throw "Expected failure '$Message', observed '$failure'."
    }
}

Assert-LocalBuildPath $probeRoot
New-Item -ItemType Directory -Path $probeRoot | Out-Null
try {
    Assert-ProbeFailure { Assert-LocalBuildPath $projectRoot } '*outside the repository*'
    Assert-ProbeFailure { Assert-LocalBuildPath (Join-Path $projectRoot '..\outside') } '*outside the repository*'
    Assert-ProbeFailure { Remove-LocalBuildScratch $probeRoot } '*outside this update*'

    New-ProbePackage $outputRoot 'previous'
    $oldHash = (Get-FileHash -LiteralPath (Join-Path $outputRoot 'Supervisor.exe') -Algorithm SHA256).Hash
    $unexpected = Join-Path $outputRoot 'personal.txt'
    [IO.File]::WriteAllText($unexpected, 'preserve me')
    Assert-ProbeFailure { Assert-LocalPackage $outputRoot } '*Unexpected content*'
    if ([IO.File]::ReadAllText($unexpected) -ne 'preserve me') { throw 'Unexpected user file was modified.' }
    Remove-Item -LiteralPath $unexpected

    # A missing staged directory must restore the package moved aside for publication.
    Assert-ProbeFailure { Publish-LocalPackage } '*'
    if ((Get-FileHash -LiteralPath (Join-Path $outputRoot 'Supervisor.exe')).Hash -ne $oldHash) {
        throw 'Missing-stage failure did not restore the previous executable.'
    }

    New-ProbePackage $stagingRoot 'invalid'
    [IO.File]::WriteAllText((Join-Path $stagingRoot 'Supervisor.exe'), 'corrupted after hashing')
    Assert-ProbeFailure { Publish-LocalPackage } '*mismatch*'
    if ((Get-FileHash -LiteralPath (Join-Path $outputRoot 'Supervisor.exe')).Hash -ne $oldHash) {
        throw 'Manifest verification failure did not restore the previous executable.'
    }
    & (Join-Path $PSScriptRoot 'verify-release-manifest.ps1') -ManifestPath (Join-Path $outputRoot 'Supervisor.release.json') | Out-Null
    Remove-LocalBuildScratch $stagingRoot

    New-ProbePackage $stagingRoot 'updated'
    $newHash = (Get-FileHash -LiteralPath (Join-Path $stagingRoot 'Supervisor.exe')).Hash
    Publish-LocalPackage
    if ((Get-FileHash -LiteralPath (Join-Path $outputRoot 'Supervisor.exe')).Hash -ne $newHash -or
        (Test-Path -LiteralPath $backupRoot) -or (Test-Path -LiteralPath $stagingRoot)) {
        throw 'Successful update did not publish exactly one complete package.'
    }
    Write-Output 'Local update probes passed: path boundaries, user-file preservation, missing-stage recovery, invalid-package recovery and successful replacement.'
} finally {
    try {
        Assert-LocalBuildPath $probeRoot
        $links = @(Get-ChildItem -LiteralPath $probeRoot -Force -Recurse | Where-Object {
            ($_.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0
        })
        if ($links.Count -gt 0) { throw 'Refusing to remove probe files containing an unexpected link.' }
        Remove-Item -LiteralPath (Resolve-Path -LiteralPath $probeRoot).Path -Recurse -Force
    } finally { Exit-SupervisorBuild $storage }
}
