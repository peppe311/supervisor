#requires -Version 5.1
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$probeRoot = Join-Path ([System.IO.Path]::GetTempPath()) "central-agent-release-probe-$([Guid]::NewGuid().ToString('N'))"

try {
    New-Item -ItemType Directory -Path $probeRoot | Out-Null

    # Regression probe for the prior File.Replace(..., $null, ...) failure.
    $destination = Join-Path $probeRoot 'destination.bin'
    $staged = Join-Path $probeRoot 'staged.bin'
    $backup = Join-Path $probeRoot 'rollback.bin'
    [System.IO.File]::WriteAllText($destination, 'old')
    [System.IO.File]::WriteAllText($staged, 'new')
    [System.IO.File]::Replace($staged, $destination, $backup, $true)
    if ([System.IO.File]::ReadAllText($destination) -ne 'new' -or
        [System.IO.File]::ReadAllText($backup) -ne 'old') {
        throw 'Atomic replacement did not preserve the expected destination and rollback contents.'
    }

    foreach ($buildScript in @('build-release.ps1', 'build-redistributable.ps1')) {
        try {
            & (Join-Path $PSScriptRoot $buildScript) -CertificateThumbprint 'probe' -AllowUnsignedDevelopment
            throw "$buildScript unexpectedly accepted mutually exclusive signing modes."
        } catch {
            if ($_.Exception.Message -notlike '*mutually exclusive*') {
                throw
            }
        }
    }

    $unsignedResult = @(& (Join-Path $PSScriptRoot 'verify-release.ps1') -Path $PSCommandPath -AllowUnsignedDevelopment)
    if ($unsignedResult.Count -ne 1 -or $unsignedResult[0].signatureStatus -ne 'NotSigned') {
        throw 'The unsigned-development gate did not accept exactly one NotSigned probe file.'
    }
    try {
        & (Join-Path $PSScriptRoot 'verify-release.ps1') -Path $PSCommandPath | Out-Null
        throw 'Production verification unexpectedly accepted a NotSigned probe file.'
    } catch {
        if ($_.Exception.Message -notlike 'Authenticode verification failed*') {
            throw
        }
    }

    foreach ($encoding in @([Text.Encoding]::UTF8, [Text.Encoding]::Unicode)) {
        $privatePathFixture = Join-Path $probeRoot 'private-path.ps1'
        [IO.File]::WriteAllText($privatePathFixture, [Environment]::GetFolderPath('UserProfile'), $encoding)
        try {
            & (Join-Path $PSScriptRoot 'verify-release.ps1') -Path $privatePathFixture -AllowUnsignedDevelopment -CheckBuildPrivacy | Out-Null
            throw 'Release privacy check accepted an embedded personal path.'
        } catch {
            if ($_.Exception.Message -ne 'Release privacy verification failed: embedded personal build path.') { throw }
        }
    }

    $packageRoot = Join-Path $probeRoot 'package'
    New-Item -ItemType Directory -Path $packageRoot | Out-Null
    $artifactNames = @('Supervisor.exe')
    $supportingNames = @('Supervisor.dependencies.json', 'LICENSE.txt', 'THIRD_PARTY_NOTICES.md')
    $contents = @('alpha', 'bravo', 'charlie', 'delta')
    $allNames = @($artifactNames + $supportingNames)
    for ($index = 0; $index -lt $allNames.Count; $index++) {
        [System.IO.File]::WriteAllText((Join-Path $packageRoot $allNames[$index]), $contents[$index])
    }

    function New-ProbeRecord {
        param([Parameter(Mandatory = $true)][string]$Name)
        $path = Join-Path $packageRoot $Name
        $item = Get-Item -LiteralPath $path
        [ordered]@{
            file = $Name
            bytes = $item.Length
            sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
        }
    }

    $manifestPath = Join-Path $packageRoot 'Supervisor.release.json'
    $baselineManifest = [ordered]@{
        schema = 'central-agent.release-manifest'
        schemaVersion = 2
        launcher = 'Supervisor.exe'
        artifacts = @($artifactNames | ForEach-Object { New-ProbeRecord -Name $_ })
        supportingFiles = @($supportingNames | ForEach-Object { New-ProbeRecord -Name $_ })
    }
    function Write-ProbeManifest {
        param([Parameter(Mandatory = $true)]$Manifest)
        $Manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $manifestPath -Encoding utf8
    }
    function Copy-ProbeManifest {
        $baselineManifest | ConvertTo-Json -Depth 8 | ConvertFrom-Json
    }
    function Assert-ManifestRejected {
        param(
            [Parameter(Mandatory = $true)]$Manifest,
            [Parameter(Mandatory = $true)][string]$ExpectedMessage
        )
        Write-ProbeManifest -Manifest $Manifest
        try {
            & (Join-Path $PSScriptRoot 'verify-release-manifest.ps1') -ManifestPath $manifestPath | Out-Null
            throw "Manifest verifier unexpectedly accepted probe: $ExpectedMessage"
        } catch {
            if ($_.Exception.Message -notlike $ExpectedMessage) {
                throw
            }
        }
    }

    Write-ProbeManifest -Manifest $baselineManifest
    $manifestResult = & (Join-Path $PSScriptRoot 'verify-release-manifest.ps1') -ManifestPath $manifestPath
    if ($manifestResult.verifiedRecords -ne 4) {
        throw 'Manifest verifier did not validate all four canonical release records.'
    }

    $probeManifest = Copy-ProbeManifest
    $probeManifest.artifacts[0].file = '..\Supervisor.exe'
    Assert-ManifestRejected -Manifest $probeManifest -ExpectedMessage '*unsafe file name*'

    $probeManifest = Copy-ProbeManifest
    $probeManifest.supportingFiles[0].file = 'Supervisor.exe'
    Assert-ManifestRejected -Manifest $probeManifest -ExpectedMessage '*duplicate file record*'

    $probeManifest = Copy-ProbeManifest
    $probeManifest.artifacts = @()
    Assert-ManifestRejected -Manifest $probeManifest -ExpectedMessage '*missing the required artifact record*'

    $probeManifest = Copy-ProbeManifest
    $probeManifest.supportingFiles = @($probeManifest.supportingFiles) + [pscustomobject]@{
        file = 'unexpected.txt'
        bytes = 0
        sha256 = ('0' * 64)
    }
    Assert-ManifestRejected -Manifest $probeManifest -ExpectedMessage '*unexpected supporting file record*'

    $probeManifest = Copy-ProbeManifest
    $probeManifest.artifacts[0].bytes = [Int64]$probeManifest.artifacts[0].bytes + 1
    Assert-ManifestRejected -Manifest $probeManifest -ExpectedMessage '*byte count mismatch*'

    [System.IO.File]::WriteAllText((Join-Path $packageRoot 'LICENSE.txt'), 'CHARLIE')
    Write-ProbeManifest -Manifest $baselineManifest
    try {
        & (Join-Path $PSScriptRoot 'verify-release-manifest.ps1') -ManifestPath $manifestPath | Out-Null
        throw 'Manifest verifier unexpectedly accepted a hash-mismatched supporting file.'
    } catch {
        if ($_.Exception.Message -notlike '*SHA-256 mismatch*') {
            throw
        }
    }
    [System.IO.File]::WriteAllText((Join-Path $packageRoot 'LICENSE.txt'), 'charlie')

    $missingPath = Join-Path $packageRoot 'THIRD_PARTY_NOTICES.md'
    $heldPath = "$missingPath.held"
    [System.IO.File]::Move($missingPath, $heldPath)
    Write-ProbeManifest -Manifest $baselineManifest
    try {
        & (Join-Path $PSScriptRoot 'verify-release-manifest.ps1') -ManifestPath $manifestPath | Out-Null
        throw 'Manifest verifier unexpectedly accepted a missing canonical file.'
    } catch {
        if ($_.Exception.Message -notlike '*references a missing file*') {
            throw
        }
    }
    [System.IO.File]::Move($heldPath, $missingPath)
} finally {
    if (Test-Path -LiteralPath $probeRoot -PathType Container) {
        $resolvedProbeRoot = [System.IO.Path]::GetFullPath($probeRoot)
        $resolvedTempRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath()).TrimEnd('\') + '\'
        if (-not $resolvedProbeRoot.StartsWith($resolvedTempRoot, [System.StringComparison]::OrdinalIgnoreCase) -or
            (Split-Path $resolvedProbeRoot -Leaf) -notlike 'central-agent-release-probe-*') {
            throw "Refusing to clean an unexpected release probe path: $resolvedProbeRoot"
        }
        Remove-Item -LiteralPath $resolvedProbeRoot -Recurse -Force
    }
}

Write-Output 'Release pipeline probes passed.'
