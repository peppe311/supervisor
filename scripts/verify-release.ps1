#requires -Version 5.1
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string[]]$Path,
    [switch]$AllowUnsignedDevelopment
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
