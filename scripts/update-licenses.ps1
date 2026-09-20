#requires -Version 5.1
[CmdletBinding()]
param([switch]$Check)
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
. (Join-Path $PSScriptRoot 'build-storage.ps1')
$storage = Enter-SupervisorBuild $projectRoot
Push-Location $projectRoot
try {
    if ($env:OS -ne 'Windows_NT' -or [IntPtr]::Size -ne 8) { throw 'Notice generation requires Windows x64.' }
    $all = Join-Path $storage.Session 'license-all.json'
    $windows = Join-Path $storage.Session 'license-windows.json'
    & cargo metadata --locked --format-version 1 | Set-Content -LiteralPath $all -Encoding utf8
    if ($LASTEXITCODE -ne 0) { throw 'Cannot read locked Cargo graph.' }
    & cargo metadata --locked --format-version 1 --filter-platform x86_64-pc-windows-msvc | Set-Content -LiteralPath $windows -Encoding utf8
    if ($LASTEXITCODE -ne 0) { throw 'Cannot read Windows Cargo graph.' }
    $mode = if ($Check) { 'check' } else { 'write' }
    & node (Join-Path $PSScriptRoot 'generate-license-notices.mjs') $all $windows $mode
    if ($LASTEXITCODE -ne 0) { throw 'Dependency notice verification failed.' }
} finally {
    Pop-Location
    Exit-SupervisorBuild $storage
}
