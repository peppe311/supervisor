#requires -Version 5.1
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
. (Join-Path $PSScriptRoot 'build-storage.ps1')
$storage = Enter-SupervisorBuild $projectRoot
try {
    $toolRoot = Assert-SupervisorStoragePath $projectRoot (Join-Path $projectRoot 'target\installer-tools\inno-6.7.3')
    $compiler = Join-Path $toolRoot 'ISCC.exe'
    if (-not (Test-Path -LiteralPath $compiler)) {
        $download = Join-Path $storage.Session 'innosetup-6.7.3.exe'
        Invoke-WebRequest -UseBasicParsing -Uri 'https://github.com/jrsoftware/issrc/releases/download/is-6_7_3/innosetup-6.7.3.exe' -OutFile $download
        if ((Get-FileHash -LiteralPath $download -Algorithm SHA256).Hash.ToLowerInvariant() -ne '9c73c3bae7ed48d44112a0f48e66742c00090bdb5bef71d9d3c056c66e97b732') {
            throw 'Inno Setup download checksum mismatch.'
        }
        $signature = Get-AuthenticodeSignature -LiteralPath $download
        if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'CN=Pyrsys B\.V\.,') {
            throw 'Inno Setup download does not have the expected valid publisher signature.'
        }
        $process = Start-Process -FilePath $download -ArgumentList @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART','/SP-','/CURRENTUSER','/PORTABLE=1',('/DIR="' + $toolRoot + '"')) -WindowStyle Hidden -PassThru -Wait
        if ($process.ExitCode -ne 0 -or -not (Test-Path -LiteralPath $compiler)) { throw 'Portable Inno Setup preparation failed.' }
    }
    $compilerSignature = Get-AuthenticodeSignature -LiteralPath $compiler
    if ($compilerSignature.Status -ne 'Valid' -or $compilerSignature.SignerCertificate.Subject -notmatch 'CN=Pyrsys B\.V\.,') {
        throw 'Inno Setup compiler publisher verification failed.'
    }
    Write-Output $compiler
} finally { Exit-SupervisorBuild $storage }
