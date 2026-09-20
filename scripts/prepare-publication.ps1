#requires -Version 5.1
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
. (Join-Path $PSScriptRoot 'build-storage.ps1')
$storage = Enter-SupervisorBuild $projectRoot
try {
    & node (Join-Path $PSScriptRoot 'prepare-publication.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Publication snapshot preparation failed.' }
} finally { Exit-SupervisorBuild $storage }
