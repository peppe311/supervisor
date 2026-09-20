[CmdletBinding()]
param(
    [string]$ProjectRoot = (Join-Path $PSScriptRoot '..')
)

$ErrorActionPreference = 'Stop'
$resolvedProjectRoot = [System.IO.Path]::GetFullPath($ProjectRoot)
$uiRoot = Join-Path $resolvedProjectRoot 'ui'

if (-not (Test-Path -LiteralPath (Join-Path $resolvedProjectRoot 'Cargo.toml') -PathType Leaf)) {
    throw "ProjectRoot does not select the Supervisor repository: $resolvedProjectRoot"
}

$node = Get-Command node.exe -ErrorAction Stop
$cargo = Get-Command cargo.exe -ErrorAction Stop
$developmentTargetRoot = if ($env:CENTRAL_AGENT_DEV_TARGET_DIR) {
    [System.IO.Path]::GetFullPath($env:CENTRAL_AGENT_DEV_TARGET_DIR)
} else {
    if ($env:CARGO_TARGET_DIR) { [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR) } else { Join-Path $resolvedProjectRoot 'target' }
}
$watcher = $null
$previousDevelopmentRoot = $env:CENTRAL_AGENT_UI_DEV_ROOT
. (Join-Path $PSScriptRoot 'build-storage.ps1')
$storage = Enter-SupervisorBuild $resolvedProjectRoot $developmentTargetRoot
$logRoot = $env:TEMP
$watcherOutput = Join-Path $logRoot 'ui-watch.stdout.log'
$watcherError = Join-Path $logRoot 'ui-watch.stderr.log'

Push-Location $resolvedProjectRoot
try {
    & (Join-Path $PSScriptRoot 'build-frontend.ps1')

    $watcher = Start-Process `
        -FilePath $node.Source `
        -ArgumentList @('node_modules/vite/bin/vite.js', 'build', '--watch', '--clearScreen', 'false', '--logLevel', 'error') `
        -WorkingDirectory $uiRoot `
        -WindowStyle Hidden `
        -RedirectStandardOutput $watcherOutput `
        -RedirectStandardError $watcherError `
        -PassThru

    Start-Sleep -Milliseconds 500
    $watcher.Refresh()
    if ($watcher.HasExited) {
        $watcherFailure = if (Test-Path -LiteralPath $watcherError) {
            (Get-Content -LiteralPath $watcherError -Raw).Trim()
        }
        else {
            'No watcher error output was captured.'
        }
        throw "The frontend watcher stopped during startup. $watcherFailure"
    }

    $env:CENTRAL_AGENT_UI_DEV_ROOT = $resolvedProjectRoot
    Write-Host 'Supervisor live UI development is active.'
    Write-Host 'Edit ui/src or assets; affected internal surfaces reload automatically.'
    Write-Host "Frontend watcher logs: $logRoot"
    Write-Host "Development build cache: $developmentTargetRoot"
    Write-Host 'Rust/native changes still require stopping and starting this command again.'

    & $cargo.Source run
    if ($LASTEXITCODE -ne 0) {
        throw "Supervisor exited with code $LASTEXITCODE."
    }
}
finally {
    try {
        if ($null -ne $watcher -and -not $watcher.HasExited) {
            Stop-Process -Id $watcher.Id -Force -ErrorAction SilentlyContinue
            $watcher.WaitForExit()
        }
        if ($null -eq $previousDevelopmentRoot) {
            Remove-Item Env:CENTRAL_AGENT_UI_DEV_ROOT -ErrorAction SilentlyContinue
        }
        else {
            $env:CENTRAL_AGENT_UI_DEV_ROOT = $previousDevelopmentRoot
        }
        Pop-Location
    } finally { Exit-SupervisorBuild $storage }
}
