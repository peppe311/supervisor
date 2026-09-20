#requires -Version 5.1
[CmdletBinding()]
param(
    [switch]$SkipTypeCheck,
    [switch]$Force
)

$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$uiRoot = Join-Path $projectRoot 'ui'
$lockPath = Join-Path $uiRoot 'package-lock.json'
$installMarker = Join-Path $uiRoot 'node_modules\.supervisor-lock.sha256'
. (Join-Path $PSScriptRoot 'build-storage.ps1')
. (Join-Path $PSScriptRoot 'frontend-cache.ps1')

if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
    throw 'Node.js and npm are required to build the Svelte frontend.'
}
if (-not (Test-Path -LiteralPath $lockPath -PathType Leaf)) {
    throw 'ui\package-lock.json is missing. Restore the committed lockfile before building.'
}

$storage = Enter-SupervisorBuild $projectRoot
Push-Location $uiRoot
try {
    $lockHash = (Get-FileHash -LiteralPath $lockPath -Algorithm SHA256).Hash
    $installedHash = if (Test-Path -LiteralPath $installMarker -PathType Leaf) { (Get-Content -LiteralPath $installMarker -Raw).Trim() } else { '' }
    if ($installedHash -ne $lockHash -or -not (Test-Path -LiteralPath (Join-Path $uiRoot 'node_modules\vite\bin\vite.js'))) {
        & npm ci --no-audit --no-fund
        if ($LASTEXITCODE -ne 0) { throw 'Installing locked frontend dependencies failed.' }
        Set-Content -LiteralPath $installMarker -Value $lockHash -Encoding ascii
    }
    if (-not $SkipTypeCheck) {
        & npm run check
        if ($LASTEXITCODE -ne 0) { throw 'Svelte and TypeScript verification failed.' }
    }
    $fingerprint = Get-SupervisorFrontendFingerprint $projectRoot
    $stampPath = Join-Path $storage.Metadata 'frontend.json'
    if (-not $Force -and (Test-SupervisorFrontendCache $projectRoot $stampPath $fingerprint)) {
        Write-Output 'Embedded Svelte frontend unchanged; reusing the verified bundle.'
        return
    }
    & npm run build
    if ($LASTEXITCODE -ne 0) { throw 'Building the embedded Svelte frontend failed.' }

    $utf8WithoutBom = [System.Text.UTF8Encoding]::new($false)
    Get-ChildItem -LiteralPath (Join-Path $uiRoot 'dist') -File |
        Where-Object { $_.Extension -in @('.css', '.js') } |
        ForEach-Object {
            $content = [System.IO.File]::ReadAllText($_.FullName)
            $normalized = [System.Text.RegularExpressions.Regex]::Replace(
                $content,
                '[\t ]+(?=\r?$)',
                '',
                [System.Text.RegularExpressions.RegexOptions]::Multiline
            )
            if ($content -cne $normalized) {
                [System.IO.File]::WriteAllText($_.FullName, $normalized, $utf8WithoutBom)
            }
        }
    Save-SupervisorFrontendCache $projectRoot $stampPath $fingerprint
} finally {
    Pop-Location
    Exit-SupervisorBuild $storage
}

Write-Output 'Embedded Svelte frontend built.'
