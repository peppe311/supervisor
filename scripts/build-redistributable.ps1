#requires -Version 5.1
[CmdletBinding()]
param(
    [string]$CertificateThumbprint,
    [string]$TimestampUrl = 'http://timestamp.digicert.com',
    [switch]$AllowUnsignedDevelopment
)

$ErrorActionPreference = 'Stop'

if ($CertificateThumbprint -and $AllowUnsignedDevelopment) {
    throw '-CertificateThumbprint and -AllowUnsignedDevelopment are mutually exclusive.'
}
if (-not $CertificateThumbprint -and -not $AllowUnsignedDevelopment) {
    throw 'A production bundle requires -CertificateThumbprint. Use -AllowUnsignedDevelopment only for a local unsigned bundle.'
}

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$outputParent = Join-Path $projectRoot 'outputs'
$bundleName = 'Supervisor'
$bundleRoot = Join-Path $outputParent $bundleName
$operationId = [Guid]::NewGuid().ToString('N')
$stagingRoot = Join-Path $outputParent ".$bundleName.$operationId.staging"
$backupRoot = Join-Path $outputParent ".$bundleName.$operationId.rollback"
$previousBuildTarget = $env:CENTRAL_AGENT_BUILD_TARGET_DIR
$previousRustFlags = $env:RUSTFLAGS
$redistributableTarget = if ($env:CENTRAL_AGENT_REDISTRIBUTABLE_TARGET_DIR) {
    [System.IO.Path]::GetFullPath($env:CENTRAL_AGENT_REDISTRIBUTABLE_TARGET_DIR)
} else {
    Join-Path $projectRoot 'target\redistributable'
}
$staticCrtFlag = '-C target-feature=+crt-static'

New-Item -ItemType Directory -Force -Path $outputParent | Out-Null
. (Join-Path $PSScriptRoot 'build-storage.ps1')
$storage = Enter-SupervisorBuild $projectRoot $redistributableTarget
try {
    $env:CENTRAL_AGENT_BUILD_TARGET_DIR = $redistributableTarget
    if (-not $env:RUSTFLAGS) {
        $env:RUSTFLAGS = $staticCrtFlag
    } elseif ($env:RUSTFLAGS -notmatch '(?:^|\s)target-feature=\+crt-static(?:\s|$)') {
        $env:RUSTFLAGS = "$($env:RUSTFLAGS) $staticCrtFlag"
    }

    $buildArguments = @{
        OutputDirectory = $stagingRoot
        TimestampUrl = $TimestampUrl
        AllowUnsignedDevelopment = $AllowUnsignedDevelopment
    }
    if ($CertificateThumbprint) {
        $buildArguments.CertificateThumbprint = $CertificateThumbprint
    }
    & (Join-Path $PSScriptRoot 'build-release.ps1') @buildArguments

    $manifestPath = Join-Path $stagingRoot 'Supervisor.release.json'
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    if ($manifest.schema -ne 'central-agent.release-manifest' -or $manifest.schemaVersion -ne 2) {
        throw 'The canonical release emitted an unsupported manifest schema.'
    }

    if (Test-Path -LiteralPath $bundleRoot) {
        [System.IO.Directory]::Move($bundleRoot, $backupRoot)
    }
    try {
        [System.IO.Directory]::Move($stagingRoot, $bundleRoot)
        & (Join-Path $PSScriptRoot 'verify-release-manifest.ps1') -ManifestPath (Join-Path $bundleRoot 'Supervisor.release.json') | Out-Null
    } catch {
        if (Test-Path -LiteralPath $backupRoot -PathType Container) {
            if (Test-Path -LiteralPath $bundleRoot -PathType Container) {
                Remove-Item -LiteralPath $bundleRoot -Recurse -Force
            }
            [System.IO.Directory]::Move($backupRoot, $bundleRoot)
        } elseif (Test-Path -LiteralPath $bundleRoot -PathType Container) {
            Remove-Item -LiteralPath $bundleRoot -Recurse -Force
        }
        throw
    }

    if (Test-Path -LiteralPath $backupRoot -PathType Container) {
        Remove-Item -LiteralPath $backupRoot -Recurse -Force
    }

} finally {
    try {
        if ($null -eq $previousBuildTarget) {
            Remove-Item Env:CENTRAL_AGENT_BUILD_TARGET_DIR -ErrorAction SilentlyContinue
        } else {
            $env:CENTRAL_AGENT_BUILD_TARGET_DIR = $previousBuildTarget
        }
        if ($null -eq $previousRustFlags) {
            Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue
        } else {
            $env:RUSTFLAGS = $previousRustFlags
        }
        if (Test-Path -LiteralPath $stagingRoot -PathType Container) {
            Remove-Item -LiteralPath $stagingRoot -Recurse -Force
        }
    } finally { Exit-SupervisorBuild $storage }
}

Write-Output "Redistributable bundle: $bundleRoot"
Write-Output "Release manifest: $(Join-Path $bundleRoot 'Supervisor.release.json')"
