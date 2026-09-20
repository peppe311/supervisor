#requires -Version 5.1
[CmdletBinding()]
param(
    [string]$OutputPath
)

$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if (-not $OutputPath) {
    $OutputPath = Join-Path $projectRoot 'artifacts\Supervisor.dependencies.json'
}
$OutputPath = [System.IO.Path]::GetFullPath($OutputPath)
$outputDirectory = Split-Path $OutputPath -Parent
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null

function Get-RelativePathCompatible {
    param(
        [Parameter(Mandatory = $true)][string]$BasePath,
        [Parameter(Mandatory = $true)][string]$Path
    )

    $separator = [System.IO.Path]::DirectorySeparatorChar
    $baseFullPath = [System.IO.Path]::GetFullPath($BasePath).TrimEnd('\', '/') + $separator
    $pathFullPath = [System.IO.Path]::GetFullPath($Path)
    $baseUri = New-Object System.Uri($baseFullPath)
    $pathUri = New-Object System.Uri($pathFullPath)
    [System.Uri]::UnescapeDataString($baseUri.MakeRelativeUri($pathUri).ToString()).Replace('/', $separator)
}

function Get-ProjectRelativeManifestPath {
    param([Parameter(Mandatory = $true)][string]$ManifestPath)

    $separator = [System.IO.Path]::DirectorySeparatorChar
    $relativePath = Get-RelativePathCompatible -BasePath $projectRoot -Path $ManifestPath
    if ($relativePath -eq '..' -or $relativePath.StartsWith("..$separator")) {
        return $null
    }
    $relativePath
}

Push-Location $projectRoot
try {
    $metadataText = & cargo metadata --locked --format-version 1
    if ($LASTEXITCODE -ne 0) {
        throw 'Could not read the locked Cargo dependency graph.'
    }
} finally {
    Pop-Location
}

$metadata = $metadataText | ConvertFrom-Json
$workspaceIds = @{}
foreach ($workspaceId in $metadata.workspace_members) {
    $workspaceIds[[string]$workspaceId] = $true
}
$packages = @($metadata.packages | ForEach-Object {
    $workspacePackage = $workspaceIds.ContainsKey([string]$_.id)
    $relativeManifestPath = Get-ProjectRelativeManifestPath -ManifestPath ([string]$_.manifest_path)
    $origin = if ($workspacePackage) {
        'workspace'
    } elseif ($relativeManifestPath) {
        'vendored'
    } elseif ([string]$_.source -like 'registry+*') {
        'registry'
    } elseif ([string]$_.source -like 'git+*') {
        'git'
    } else {
        'external-or-unknown'
    }
    [ordered]@{
        name = $_.name
        version = $_.version
        declaredLicense = $_.license
        source = $_.source
        origin = $origin
        manifestPath = $relativeManifestPath
        workspacePackage = $workspacePackage
    }
} | Sort-Object name, version)
$application = $metadata.packages | Where-Object { $_.name -eq 'central-agent' -and $workspaceIds.ContainsKey([string]$_.id) } | Select-Object -First 1
$noticeInventory = Get-Content -LiteralPath (Join-Path $projectRoot 'licenses/dependencies.json') -Raw | ConvertFrom-Json
if ($noticeInventory.schema -ne 'supervisor.license-inventory' -or $noticeInventory.schemaVersion -ne 1) {
    throw 'The checked dependency notice inventory is missing or unsupported.'
}
$missingRequiredNotices = @($noticeInventory.packages | Where-Object { $_.required -and @($_.notices).Count -eq 0 })
if ($missingRequiredNotices.Count -gt 0) { throw 'Required Windows dependency notices are missing.' }
$noticesPath = Join-Path $projectRoot 'THIRD_PARTY_NOTICES.md'
$payload = [ordered]@{
    schema = 'central-agent.cargo-dependency-inventory'
    schemaVersion = 2
    application = 'Supervisor'
    applicationVersion = if ($application) { $application.version } else { $null }
    generatedAtUtc = [DateTimeOffset]::UtcNow.ToString('o')
    lockfile = 'Cargo.lock'
    reviewStatus = 'notice-coverage-checked-redistribution-review-required'
    bundledNoticeTextsComplete = @($noticeInventory.packages | Where-Object { @($_.notices).Count -eq 0 }).Count -eq 0
    windowsNoticeTextsComplete = $true
    noticeTarget = $noticeInventory.target
    noticeDocumentSha256 = (Get-FileHash -LiteralPath $noticesPath -Algorithm SHA256).Hash.ToLowerInvariant()
    sourceRepository = 'https://github.com/peppe311/supervisor'
    limitation = 'Notice coverage is verified for the Windows target and frontend. Other-platform metadata may lack texts; collection does not certify redistribution compliance.'
    packageCount = $packages.Count
    packages = $packages
    thirdPartyComponents = $noticeInventory.packages
}

$temporaryPath = Join-Path $outputDirectory ".$([System.IO.Path]::GetFileName($OutputPath)).$([Guid]::NewGuid().ToString('N')).tmp"
$backupPath = Join-Path $outputDirectory ".$([System.IO.Path]::GetFileName($OutputPath)).$([Guid]::NewGuid().ToString('N')).rollback"
try {
    $payload | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $temporaryPath -Encoding utf8
    if (Test-Path -LiteralPath $OutputPath -PathType Leaf) {
        [System.IO.File]::Replace($temporaryPath, $OutputPath, $backupPath, $true)
    } else {
        [System.IO.File]::Move($temporaryPath, $OutputPath)
    }
} finally {
    foreach ($cleanupPath in @($temporaryPath, $backupPath)) {
        if (Test-Path -LiteralPath $cleanupPath -PathType Leaf) {
            Remove-Item -LiteralPath $cleanupPath -Force
        }
    }
}

Write-Output "Dependency inventory: $OutputPath"
