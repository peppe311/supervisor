param(
    [string]$Destination = "target/reference-docs/CODEX_APP_SERVER_OFFICIAL.md"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$sourceUri = "https://learn.chatgpt.com/docs/app-server.md"
$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$destinationPath = if ([System.IO.Path]::IsPathRooted($Destination)) {
    [System.IO.Path]::GetFullPath($Destination)
} else {
    [System.IO.Path]::GetFullPath((Join-Path $repositoryRoot $Destination))
}
$repositoryPrefix = $repositoryRoot.TrimEnd(
    [System.IO.Path]::DirectorySeparatorChar,
    [System.IO.Path]::AltDirectorySeparatorChar
) + [System.IO.Path]::DirectorySeparatorChar

if (-not $destinationPath.StartsWith($repositoryPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "The documentation destination must stay inside the repository."
}

$destinationDirectory = Split-Path -Parent $destinationPath
New-Item -ItemType Directory -Force -Path $destinationDirectory | Out-Null
$temporaryPath = Join-Path $destinationDirectory (".codex-app-server-" + [guid]::NewGuid().ToString("N") + ".tmp")

try {
    Invoke-WebRequest -UseBasicParsing -Uri $sourceUri -OutFile $temporaryPath
    $content = Get-Content -Raw -LiteralPath $temporaryPath
    if (
        $content.Length -lt 50000 -or
        -not $content.StartsWith("# Codex App Server") -or
        -not $content.Contains('`thread/start`') -or
        -not $content.Contains('`account/read`') -or
        -not $content.Contains('`mcpServerStatus/list`')
    ) {
        throw "The downloaded document does not look like the complete Codex App Server Markdown page."
    }

    $downloadHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $temporaryPath).Hash
    $changed = $true
    if (Test-Path -LiteralPath $destinationPath) {
        $currentHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $destinationPath).Hash
        $changed = $currentHash -ne $downloadHash
    }

    if ($changed) {
        Move-Item -Force -LiteralPath $temporaryPath -Destination $destinationPath
    }

    $item = Get-Item -LiteralPath $destinationPath
    [pscustomobject]@{
        Source = $sourceUri
        Destination = $item.FullName
        Bytes = $item.Length
        Sha256 = $downloadHash
        Changed = $changed
        RetrievedAtUtc = [DateTimeOffset]::UtcNow.ToString("O")
    } | Format-List
} finally {
    if (Test-Path -LiteralPath $temporaryPath) {
        Remove-Item -Force -LiteralPath $temporaryPath
    }
}
