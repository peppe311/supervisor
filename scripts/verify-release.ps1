#requires -Version 5.1
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string[]]$Path,
    [switch]$AllowUnsignedDevelopment,
    [switch]$CheckBuildPrivacy
)

$ErrorActionPreference = 'Stop'

$results = foreach ($candidate in $Path) {
    $resolved = (Resolve-Path -LiteralPath $candidate).Path
    $item = Get-Item -LiteralPath $resolved
    if ($item.PSIsContainer) {
        throw "Release verification expects a file: $resolved"
    }

    $signature = Get-AuthenticodeSignature -LiteralPath $resolved
    if ($AllowUnsignedDevelopment) {
        if ($signature.Status -ne 'NotSigned') {
            throw "Unsigned development verification accepts only NotSigned files: $resolved is $($signature.Status)."
        }
    } elseif ($signature.Status -ne 'Valid') {
        throw "Authenticode verification failed for $resolved ($($signature.Status))."
    }

    if ($CheckBuildPrivacy -or -not $AllowUnsignedDevelopment) {
        # Reject personal build directories even when source and signatures are valid.
        # Do not print matched paths: failure output can itself become a release log.
        $bytes = [IO.File]::ReadAllBytes($resolved)
        $utf8 = [Text.Encoding]::UTF8.GetString($bytes)
        $utf16 = [Text.Encoding]::Unicode.GetString($bytes)
        $privateRoots = @([Environment]::GetFolderPath('UserProfile'), (Resolve-Path (Join-Path $PSScriptRoot '..')).Path)
        foreach ($privateRoot in $privateRoots) {
            if (-not $privateRoot -or $privateRoot.Length -lt 6) { continue }
            foreach ($needle in @($privateRoot, $privateRoot.Replace('\','/'), $privateRoot.Replace('\','\\'))) {
                if ($utf8.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -ge 0 -or $utf16.IndexOf($needle, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
                    throw 'Release privacy verification failed: embedded personal build path.'
                }
            }
        }
        }
    $hash = Get-FileHash -LiteralPath $resolved -Algorithm SHA256
    [pscustomobject]@{
        file = $item.Name
        bytes = $item.Length
        sha256 = $hash.Hash.ToLowerInvariant()
        signatureStatus = [string]$signature.Status
        signer = if ($signature.SignerCertificate) { $signature.SignerCertificate.Subject } else { $null }
        verifiedAtUtc = [DateTime]::UtcNow.ToString('o')
    }
}

$results
