#requires -Version 5.1
[CmdletBinding()]
param(
    [string]$CertificateThumbprint,
    [string]$TimestampUrl = 'http://timestamp.digicert.com',
    [switch]$AllowUnsignedDevelopment,
    [string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'

if ($CertificateThumbprint -and $AllowUnsignedDevelopment) {
    throw '-CertificateThumbprint and -AllowUnsignedDevelopment are mutually exclusive.'
}
if (-not $CertificateThumbprint -and -not $AllowUnsignedDevelopment) {
    throw 'A production release requires -CertificateThumbprint. Use -AllowUnsignedDevelopment only for a local unsigned build.'
}

$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
& (Join-Path $PSScriptRoot 'verify-release-source.ps1')
if (-not $OutputDirectory) {
    $OutputDirectory = Join-Path $projectRoot 'outputs\Supervisor'
}
$outputRoot = [System.IO.Path]::GetFullPath($OutputDirectory)
$outputParent = Split-Path $outputRoot -Parent
$outputLeaf = Split-Path $outputRoot -Leaf
$buildTargetRoot = if ($env:CENTRAL_AGENT_BUILD_TARGET_DIR) {
    [System.IO.Path]::GetFullPath($env:CENTRAL_AGENT_BUILD_TARGET_DIR)
} else {
    if ($env:CARGO_TARGET_DIR) { [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR) } else { Join-Path $projectRoot 'target' }
}
$releaseRoot = Join-Path $buildTargetRoot 'release'
$releaseId = [Guid]::NewGuid().ToString('N')
$stagingRoot = Join-Path $outputParent ".$outputLeaf.release.$releaseId.staging"
$rollbackRoot = Join-Path $outputParent ".$outputLeaf.release.$releaseId.rollback"
$manifestName = 'Supervisor.release.json'
$inventoryName = 'Supervisor.dependencies.json'
$licenseName = 'LICENSE.txt'
$noticesName = 'THIRD_PARTY_NOTICES.md'
$releasePublished = $false
$payloadNames = @(
    'Supervisor.exe',
    $inventoryName,
    $licenseName,
    $noticesName,
    $manifestName
)

function Remove-ReleaseScratchDirectory {
    param([Parameter(Mandatory = $true)][string]$Path)

    $resolved = (Resolve-Path -LiteralPath $Path).Path
    $resolvedParent = (Resolve-Path -LiteralPath $outputParent).Path
    $item = Get-Item -LiteralPath $resolved
    if (-not $item.PSIsContainer -or
        ($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0 -or
        -not [string]::Equals((Split-Path $resolved -Parent), $resolvedParent, [StringComparison]::OrdinalIgnoreCase) -or
        (Split-Path $resolved -Leaf) -notin @(".$outputLeaf.release.$releaseId.staging", ".$outputLeaf.release.$releaseId.rollback")) {
        throw "Refusing to clean a path outside this release's exact scratch directories: $resolved"
    }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}

function Assert-ReleaseDestinationAvailable {
    param([Parameter(Mandatory = $true)][string]$Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        return
    }

    $stream = $null
    try {
        $stream = [System.IO.File]::Open(
            $Path,
            [System.IO.FileMode]::Open,
            [System.IO.FileAccess]::ReadWrite,
            [System.IO.FileShare]::None
        )
    } catch {
        throw "Release output is in use or cannot be replaced: $Path. Close Supervisor and retry; no build was started."
    } finally {
        if ($null -ne $stream) {
            $stream.Dispose()
        }
    }
}

function Get-ReleaseFileRecord {
    param([Parameter(Mandatory = $true)][string]$Path)

    $item = Get-Item -LiteralPath $Path
    $hash = Get-FileHash -LiteralPath $Path -Algorithm SHA256
    [ordered]@{
        file = $item.Name
        bytes = $item.Length
        sha256 = $hash.Hash.ToLowerInvariant()
    }
}

function Publish-ReleaseSet {
    param(
        [Parameter(Mandatory = $true)][string]$SourceDirectory,
        [Parameter(Mandatory = $true)][string]$DestinationDirectory,
        [Parameter(Mandatory = $true)][string]$BackupDirectory,
        [Parameter(Mandatory = $true)][string[]]$FileNames
    )

    New-Item -ItemType Directory -Force -Path $DestinationDirectory | Out-Null
    New-Item -ItemType Directory -Force -Path $BackupDirectory | Out-Null
    $published = New-Object System.Collections.ArrayList

    try {
        foreach ($fileName in $FileNames) {
            $source = Join-Path $SourceDirectory $fileName
            $destination = Join-Path $DestinationDirectory $fileName
            $existed = Test-Path -LiteralPath $destination -PathType Leaf
            $backup = if ($existed) { Join-Path $BackupDirectory $fileName } else { $null }

            if ($existed) {
                # File.Replace requires a non-empty backup path on both Windows
                # PowerShell 5.1 and modern PowerShell/.NET runtimes.
                [System.IO.File]::Replace($source, $destination, $backup, $true)
            } else {
                [System.IO.File]::Move($source, $destination)
            }

            [void]$published.Add([pscustomobject]@{
                destination = $destination
                backup = $backup
                existed = $existed
            })
        }
    } catch {
        for ($index = $published.Count - 1; $index -ge 0; $index--) {
            $entry = $published[$index]
            try {
                if ($entry.existed -and (Test-Path -LiteralPath $entry.backup -PathType Leaf)) {
                    [System.IO.File]::Copy($entry.backup, $entry.destination, $true)
                } elseif (-not $entry.existed -and (Test-Path -LiteralPath $entry.destination -PathType Leaf)) {
                    Remove-Item -LiteralPath $entry.destination -Force
                }
            } catch {
                Write-Warning "Could not roll back $($entry.destination): $($_.Exception.Message)"
            }
        }
        throw
    }
}

New-Item -ItemType Directory -Force -Path $outputParent | Out-Null
New-Item -ItemType Directory -Force -Path $outputRoot | Out-Null

# Fail before the expensive build if any file in the canonical payload is held
# open. The manifest is published last and is the completion marker.
foreach ($fileName in $payloadNames) {
    Assert-ReleaseDestinationAvailable -Path (Join-Path $outputRoot $fileName)
}

. (Join-Path $PSScriptRoot 'build-storage.ps1')
$storage = Enter-SupervisorBuild $projectRoot $buildTargetRoot
try {
    Push-Location $projectRoot
    try {
        & (Join-Path $PSScriptRoot 'verify-workspace.ps1')
        # Frontend generation must not silently move the release away from HEAD.
        & (Join-Path $PSScriptRoot 'verify-release-source.ps1')
        & cargo build --locked --release --workspace
        if ($LASTEXITCODE -ne 0) { throw 'Locked release build failed.' }
    } finally {
        Pop-Location
    }

    $artifactSources = [ordered]@{
        'Supervisor.exe' = (Join-Path $releaseRoot 'central-agent.exe')
    }
    foreach ($source in $artifactSources.Values) {
        if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
            throw "Expected release artifact is missing: $source"
        }
    }

    try {
        New-Item -ItemType Directory -Force -Path $stagingRoot | Out-Null
        foreach ($fileName in $artifactSources.Keys) {
            [System.IO.File]::Copy($artifactSources[$fileName], (Join-Path $stagingRoot $fileName), $false)
        }

        # Exercise the exact staged executable. This creates only hidden WebViews and
        # temporary profiles, never user projects, providers, or a visible test window.
        & (Join-Path $stagingRoot 'Supervisor.exe') --check-ui-startup
        if ($LASTEXITCODE -ne 0) { throw 'The staged executable failed the UI startup check.' }

        if ($CertificateThumbprint) {
            $signTool = (Get-Command signtool.exe -ErrorAction Stop).Source
            foreach ($fileName in $artifactSources.Keys) {
                $stagedArtifact = Join-Path $stagingRoot $fileName
                & $signTool sign /sha1 $CertificateThumbprint /fd SHA256 /tr $TimestampUrl /td SHA256 $stagedArtifact
                if ($LASTEXITCODE -ne 0) { throw "Signing failed for $fileName." }
            }
        }

        $stagedArtifactPaths = @($artifactSources.Keys | ForEach-Object { Join-Path $stagingRoot $_ })
        $verification = @(& (Join-Path $PSScriptRoot 'verify-release.ps1') -Path $stagedArtifactPaths -AllowUnsignedDevelopment:$AllowUnsignedDevelopment)

        & (Join-Path $PSScriptRoot 'generate-dependency-inventory.ps1') -OutputPath (Join-Path $stagingRoot $inventoryName) | Out-Null
        [System.IO.File]::Copy((Join-Path $projectRoot 'LICENSE'), (Join-Path $stagingRoot $licenseName), $false)
        [System.IO.File]::Copy((Join-Path $projectRoot 'THIRD_PARTY_NOTICES.md'), (Join-Path $stagingRoot $noticesName), $false)

        $inventory = Get-Content -LiteralPath (Join-Path $stagingRoot $inventoryName) -Raw | ConvertFrom-Json
        $supportingFiles = @(
            Get-ReleaseFileRecord -Path (Join-Path $stagingRoot $inventoryName)
            Get-ReleaseFileRecord -Path (Join-Path $stagingRoot $licenseName)
            Get-ReleaseFileRecord -Path (Join-Path $stagingRoot $noticesName)
        )
        $manifest = [ordered]@{
            schema = 'central-agent.release-manifest'
            schemaVersion = 2
            application = 'Supervisor'
            applicationVersion = $inventory.applicationVersion
            sourceCommit = (& git -C $projectRoot rev-parse --verify HEAD).Trim()
            sourceDirty = $false
            target = 'windows'
            profile = 'release'
            releaseMode = if ($AllowUnsignedDevelopment) { 'unsigned-development' } else { 'signed-production' }
            launcher = 'Supervisor.exe'
            generatedAtUtc = [DateTimeOffset]::UtcNow.ToString('o')
            artifacts = $verification
            supportingFiles = $supportingFiles
        }
        $manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $stagingRoot $manifestName) -Encoding utf8
        & (Join-Path $PSScriptRoot 'verify-release-manifest.ps1') -ManifestPath (Join-Path $stagingRoot $manifestName) | Out-Null

        # Recheck the complete destination set immediately before publishing.
        foreach ($fileName in $payloadNames) {
            Assert-ReleaseDestinationAvailable -Path (Join-Path $outputRoot $fileName)
        }
        Publish-ReleaseSet -SourceDirectory $stagingRoot -DestinationDirectory $outputRoot -BackupDirectory $rollbackRoot -FileNames $payloadNames
        $releasePublished = $true
    } finally {
        if (Test-Path -LiteralPath $stagingRoot -PathType Container) {
            Remove-ReleaseScratchDirectory -Path $stagingRoot
        }
        if ($releasePublished -and (Test-Path -LiteralPath $rollbackRoot -PathType Container)) {
            Remove-ReleaseScratchDirectory -Path $rollbackRoot
        } elseif (Test-Path -LiteralPath $rollbackRoot -PathType Container) {
            Write-Warning "Release publication did not complete. Rollback material was retained at $rollbackRoot"
        }
    }

    $artifactPaths = @($artifactSources.Keys | ForEach-Object { Join-Path $outputRoot $_ })
    $publishedVerification = @(& (Join-Path $PSScriptRoot 'verify-release.ps1') -Path $artifactPaths -AllowUnsignedDevelopment:$AllowUnsignedDevelopment)
    & (Join-Path $PSScriptRoot 'verify-release-manifest.ps1') -ManifestPath (Join-Path $outputRoot $manifestName) | Out-Null
    $publishedVerification
    Write-Output "Release directory: $outputRoot"
    Write-Output "Release manifest: $(Join-Path $outputRoot $manifestName)"
} finally {
    Exit-SupervisorBuild $storage
}
