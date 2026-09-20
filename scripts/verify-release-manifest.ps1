#requires -Version 5.1
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$ManifestPath
)

$ErrorActionPreference = 'Stop'
$resolvedManifest = (Resolve-Path -LiteralPath $ManifestPath).Path
$manifestItem = Get-Item -LiteralPath $resolvedManifest
if ($manifestItem.PSIsContainer) {
    throw "Release manifest verification expects a file: $resolvedManifest"
}

try {
    $manifest = Get-Content -LiteralPath $resolvedManifest -Raw | ConvertFrom-Json
} catch {
    throw "Release manifest is not valid JSON: $resolvedManifest. $($_.Exception.Message)"
}

if ($manifest.schema -ne 'central-agent.release-manifest' -or [int]$manifest.schemaVersion -ne 2) {
    throw 'Release manifest must use central-agent.release-manifest schema version 2.'
}
if ([string]$manifest.launcher -cne 'Supervisor.exe') {
    throw 'Release manifest launcher must be Supervisor.exe.'
}

$expectedArtifacts = @('Supervisor.exe')
$expectedSupportingFiles = @('Supervisor.dependencies.json', 'LICENSE.txt', 'THIRD_PARTY_NOTICES.md')
$artifactRecords = @($manifest.artifacts)
$supportingRecords = @($manifest.supportingFiles)
$allEntries = @()
foreach ($record in $artifactRecords) {
    $allEntries += [pscustomobject]@{ section = 'artifacts'; record = $record }
}
foreach ($record in $supportingRecords) {
    $allEntries += [pscustomobject]@{ section = 'supportingFiles'; record = $record }
}

$seenNames = @{}
foreach ($entry in $allEntries) {
    $record = $entry.record
    $name = if ($null -ne $record) { [string]$record.file } else { '' }
    if ([string]::IsNullOrWhiteSpace($name) -or
        $name -in @('.', '..') -or
        [System.IO.Path]::IsPathRooted($name) -or
        $name.IndexOfAny([System.IO.Path]::GetInvalidFileNameChars()) -ge 0 -or
        [System.IO.Path]::GetFileName($name) -cne $name) {
        throw "Release manifest contains an unsafe file name in $($entry.section): '$name'."
    }
    if ($seenNames.ContainsKey($name)) {
        throw "Release manifest contains a duplicate file record: $name."
    }
    $seenNames[$name] = $true
}

function Assert-ExactRecordSet {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][object[]]$Records,
        [Parameter(Mandatory = $true)][string[]]$Expected,
        [Parameter(Mandatory = $true)][string]$Section
    )

    $actual = @()
    foreach ($record in $Records) {
        $name = [string]$record.file
        if ($Expected -cnotcontains $name) {
            throw "Release manifest contains an unexpected $Section record: $name."
        }
        $actual += $name
    }
    foreach ($name in $Expected) {
        if ($actual -cnotcontains $name) {
            throw "Release manifest is missing the required $Section record: $name."
        }
    }
}

Assert-ExactRecordSet -Records $artifactRecords -Expected $expectedArtifacts -Section 'artifact'
Assert-ExactRecordSet -Records $supportingRecords -Expected $expectedSupportingFiles -Section 'supporting file'

$releaseRoot = Split-Path $resolvedManifest -Parent
$releaseRootPrefix = [System.IO.Path]::GetFullPath($releaseRoot).TrimEnd('\', '/') + [System.IO.Path]::DirectorySeparatorChar
foreach ($entry in $allEntries) {
    $record = $entry.record
    $name = [string]$record.file
    $candidate = [System.IO.Path]::GetFullPath((Join-Path $releaseRoot $name))
    if (-not $candidate.StartsWith($releaseRootPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Release manifest record escapes the release directory: $name."
    }
    if (-not (Test-Path -LiteralPath $candidate -PathType Leaf)) {
        throw "Release manifest references a missing file: $name."
    }

    $expectedBytes = 0L
    if ($null -eq $record.bytes -or
        -not [Int64]::TryParse([string]$record.bytes, [ref]$expectedBytes) -or
        $expectedBytes -lt 0) {
        throw "Release manifest contains an invalid byte count for $name."
    }
    $expectedHash = [string]$record.sha256
    if ($expectedHash -notmatch '^[0-9a-fA-F]{64}$') {
        throw "Release manifest contains an invalid SHA-256 value for $name."
    }

    $item = Get-Item -LiteralPath $candidate
    if ($item.Length -ne $expectedBytes) {
        throw "Release manifest byte count mismatch for ${name}: expected $expectedBytes, found $($item.Length)."
    }
    $actualHash = (Get-FileHash -LiteralPath $candidate -Algorithm SHA256).Hash
    if ($actualHash -cne $expectedHash.ToUpperInvariant()) {
        throw "Release manifest SHA-256 mismatch for $name."
    }
}

[pscustomobject]@{
    manifest = $manifestItem.Name
    schemaVersion = 2
    verifiedRecords = $allEntries.Count
    verifiedAtUtc = [DateTime]::UtcNow.ToString('o')
}
