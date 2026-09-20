#requires -Version 5.1
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
. (Join-Path $PSScriptRoot 'build-storage.ps1')
$storage = Enter-SupervisorBuild $projectRoot
Push-Location $projectRoot
try {
    # Pinned upstream binary, verified before extraction/execution. Temporary
    # binary, archive and redacted reports are removed by the build scope.
    $archive = Join-Path $storage.Session 'gitleaks.zip'
    $expected = 'd29144deff3a68aa93ced33dddf84b7fdc26070add4aa0f4513094c8332afc4e'
    Invoke-WebRequest -UseBasicParsing -Uri 'https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_windows_x64.zip' -OutFile $archive
    if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $expected) { throw 'Gitleaks download checksum mismatch.' }
    $bin = Join-Path $storage.Session 'gitleaks'
    Expand-Archive -LiteralPath $archive -DestinationPath $bin
    $tool = Join-Path $bin 'gitleaks.exe'
    $snapshot = Join-Path $storage.Session 'scan-inputs'
    & node (Join-Path $PSScriptRoot 'stage-secret-scan.mjs') $snapshot
    if ($LASTEXITCODE -ne 0) { throw 'Cannot stage source files for scanning.' }
    foreach ($scan in @('git','dir')) {
        $report = Join-Path $storage.Session "$scan.json"
        $arguments = @($scan, '--redact', '--no-banner', '--no-color', '--ignore-gitleaks-allow', '--config', (Join-Path $projectRoot '.gitleaks.toml'), '--gitleaks-ignore-path', (Join-Path $projectRoot '.gitleaksignore'), '--report-format', 'json', '--report-path', $report)
        if ($scan -eq 'git') { $arguments += @('--log-opts=--all', $projectRoot) } else { $arguments += $snapshot }
        & $tool @arguments
        if ($LASTEXITCODE -ne 0) {
            if (Test-Path -LiteralPath $report) {
                Get-Content -LiteralPath $report -Raw | ConvertFrom-Json | Select-Object RuleID, File, StartLine, Fingerprint | ConvertTo-Json
            }
            throw "Secret scan failed ($scan). Review findings without publishing secret values."
        }
    }
    Write-Output 'Secret scan passed: complete Git history and current publication inputs.'
} finally {
    Pop-Location
    Exit-SupervisorBuild $storage
}
