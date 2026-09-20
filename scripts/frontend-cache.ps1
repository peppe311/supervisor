#requires -Version 5.1

function Get-SupervisorFrontendFingerprint {
    param([string]$ProjectRoot)
    $ui = Join-Path $ProjectRoot 'ui'
    $files = @(
        foreach ($directory in @('ui\src', 'ui\public', 'assets')) {
            $path = Join-Path $ProjectRoot $directory
            if (Test-Path -LiteralPath $path) { Get-ChildItem -LiteralPath $path -File -Recurse }
        }
        Get-ChildItem -LiteralPath $ui -File
        Get-Item -LiteralPath (Join-Path $ProjectRoot 'scripts\build-frontend.ps1')
        Get-Item -LiteralPath (Join-Path $ProjectRoot 'scripts\frontend-cache.ps1')
    )
    $records = @(foreach ($file in $files | Sort-Object FullName -Unique) {
        $relative = $file.FullName.Substring($ProjectRoot.Length)
        $hash = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash
        "$relative=$hash"
    })
    $nodeVersion = & node --version
    if ($LASTEXITCODE -ne 0) { throw 'Could not identify the frontend Node.js runtime.' }
    $bytes = [Text.Encoding]::UTF8.GetBytes(($records -join "`n") + "`nnode=$nodeVersion")
    $sha = [Security.Cryptography.SHA256]::Create()
    try { return ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-', '').ToLowerInvariant() }
    finally { $sha.Dispose() }
}

function Test-SupervisorFrontendCache {
    param([string]$ProjectRoot, [string]$StampPath, [string]$Fingerprint)
    if (-not (Test-Path -LiteralPath $StampPath -PathType Leaf)) { return $false }
    try {
        $stamp = Get-Content -LiteralPath $StampPath -Raw | ConvertFrom-Json
        if ($stamp.schemaVersion -ne 1 -or $stamp.fingerprint -ne $Fingerprint) { return $false }
        $dist = Join-Path $ProjectRoot 'ui\dist'
        $files = @(Get-ChildItem -LiteralPath $dist -File -Recurse)
        $names = @($files | ForEach-Object { $_.FullName.Substring($dist.Length + 1) })
        if ('central-agent-ui.js' -notin $names -or 'central-agent-ui.css' -notin $names -or $files.Count -ne @($stamp.outputs).Count) { return $false }
        foreach ($file in $files) {
            $name = $file.FullName.Substring($dist.Length + 1)
            $expected = @($stamp.outputs | Where-Object { $_.name -eq $name })
            if ($expected.Count -ne 1 -or $expected[0].sha256 -ne (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash) { return $false }
        }
        return $true
    } catch { return $false }
}

function Save-SupervisorFrontendCache {
    param([string]$ProjectRoot, [string]$StampPath, [string]$Fingerprint)
    $dist = Join-Path $ProjectRoot 'ui\dist'
    $outputs = @(foreach ($file in Get-ChildItem -LiteralPath $dist -File -Recurse) {
        @{ name = $file.FullName.Substring($dist.Length + 1); sha256 = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash }
    })
    @{ schemaVersion = 1; fingerprint = $Fingerprint; outputs = $outputs } |
        ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $StampPath -Encoding utf8
}
