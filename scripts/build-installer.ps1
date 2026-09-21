#requires -Version 5.1
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$PackageDirectory,
    [Parameter(Mandatory = $true)][ValidatePattern('^v[0-9]+\.[0-9]+\.[0-9]+(?:-alpha\.[0-9]+)?$')][string]$ReleaseTag,
    [string]$CertificateThumbprint,
    [string]$TimestampUrl = 'http://timestamp.digicert.com',
    [switch]$AllowUnsignedDevelopment,
    [switch]$PublishUnsignedAlpha
)
$ErrorActionPreference = 'Stop'
if (([int][bool]$CertificateThumbprint + [int][bool]$AllowUnsignedDevelopment + [int][bool]$PublishUnsignedAlpha) -ne 1) { throw 'Choose exactly one signing mode; signing modes are mutually exclusive.' }
if ($PublishUnsignedAlpha -and $ReleaseTag -notmatch '-alpha\.[0-9]+$') { throw 'Unsigned public distribution is limited to explicitly selected alpha tags.' }
if ($CertificateThumbprint -and $CertificateThumbprint -notmatch '^[0-9a-fA-F]{40}$') { throw 'Invalid certificate thumbprint.' }
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if (-not $AllowUnsignedDevelopment) { & (Join-Path $PSScriptRoot 'verify-release-source.ps1') | Out-Null }
$unsigned = $AllowUnsignedDevelopment -or $PublishUnsignedAlpha
$package = (Resolve-Path -LiteralPath $PackageDirectory).Path
$manifestPath = Join-Path $package 'Supervisor.release.json'
& (Join-Path $PSScriptRoot 'verify-release-manifest.ps1') -ManifestPath $manifestPath | Out-Null
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
if ($manifest.sourceDirty -ne $false -or $manifest.sourceCommit -notmatch '^[0-9a-f]{40}$') { throw 'Installer input must come from clean, committed release source.' }
if (-not $AllowUnsignedDevelopment) {
    $tagCommit = & git -C $projectRoot rev-parse --verify "$ReleaseTag^{commit}" 2>$null
    if ($LASTEXITCODE -ne 0 -or $tagCommit.Trim() -ne $manifest.sourceCommit) { throw 'Installer input must match the exact application source tag.' }
}
$expectedMode = if ($unsigned) { 'unsigned-development' } else { 'signed-production' }
if ($manifest.releaseMode -ne $expectedMode) { throw 'Package signing mode differs from installer signing mode.' }
$version = $ReleaseTag.Substring(1)
$numericVersion = ($version -split '-')[0]
if ($manifest.applicationVersion -ne $numericVersion) { throw 'Release tag and packaged application version differ.' }
& (Join-Path $PSScriptRoot 'verify-release.ps1') -Path (Join-Path $package 'Supervisor.exe') -AllowUnsignedDevelopment:$unsigned | Out-Null
if ($CertificateThumbprint) {
    $packageSigner = (Get-AuthenticodeSignature -LiteralPath (Join-Path $package 'Supervisor.exe')).SignerCertificate.Thumbprint
    if ($packageSigner -ne $CertificateThumbprint) { throw 'Application and installer must use the same publisher certificate.' }
}
. (Join-Path $PSScriptRoot 'build-storage.ps1')
$storage = Enter-SupervisorBuild $projectRoot
try {
    $compiler = & (Join-Path $PSScriptRoot 'get-installer-compiler.ps1')
    $destination = Assert-SupervisorStoragePath $projectRoot (Join-Path $projectRoot 'target\installer')
    New-Item -ItemType Directory -Path $destination -Force | Out-Null
    $baseName = "Supervisor-Setup-$version-x64"
    if ($AllowUnsignedDevelopment) { $baseName += '-unsigned-local' }
    $compilerArgs = @('/Qp', "/DPackageDirectory=$package", "/DOutputDirectory=$($storage.Session)", "/DReleaseVersion=$version", "/DNumericVersion=$numericVersion", "/DOutputBaseName=$baseName")
    if ($CertificateThumbprint) {
        $signTool = (Get-Command signtool.exe -ErrorAction Stop).Source
        $compilerArgs += '/DSignedInstaller=1'
        $compilerArgs += ('/Ssupervisor=$q' + $signTool + '$q sign /sha1 ' + $CertificateThumbprint + ' /fd SHA256 /tr $q' + $TimestampUrl + '$q /td SHA256 $f')
    }
    & $compiler @compilerArgs (Join-Path $PSScriptRoot 'installer\Supervisor.iss')
    if ($LASTEXITCODE -ne 0) { throw 'Installer compilation failed.' }
    $staged = Join-Path $storage.Session ($baseName + '.exe')
    $verification = & (Join-Path $PSScriptRoot 'verify-release.ps1') -Path $staged -AllowUnsignedDevelopment:$unsigned
    $output = Join-Path $destination ($baseName + '.exe')
    [IO.File]::Copy($staged, $output, $true)
    $record = [ordered]@{
        schema = 'supervisor.installer'; schemaVersion = 1; applicationVersion = $version
        sourceCommit = $manifest.sourceCommit
        installerSourceCommit = (& git -C $projectRoot rev-parse --verify HEAD).Trim()
        releaseMode = if ($PublishUnsignedAlpha) { 'unsigned-alpha' } else { $expectedMode }
        file = $verification.file; bytes = $verification.bytes; sha256 = $verification.sha256
        signatureStatus = $verification.signatureStatus
    }
    $record | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $destination ($baseName + '.json')) -Encoding utf8
    [IO.File]::WriteAllText((Join-Path $destination ($baseName + '.sha256')), "$($verification.sha256)  $baseName.exe`n")
    Write-Output "Installer: $output"
} finally { Exit-SupervisorBuild $storage }
