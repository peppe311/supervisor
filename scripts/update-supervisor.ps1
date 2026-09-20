#requires -Version 5.1
[CmdletBinding(SupportsShouldProcess = $true)]
param()

$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$manifestVerifier = Join-Path $PSScriptRoot 'verify-release-manifest.ps1'
$outputParent = Join-Path $projectRoot 'outputs'
$outputRoot = Join-Path $outputParent 'Supervisor'
$targetRoot = Join-Path $projectRoot 'target'
$operationId = [Guid]::NewGuid().ToString('N')
$stagingRoot = Join-Path $outputParent ".Supervisor.$operationId.staging"
$backupRoot = Join-Path $outputParent ".Supervisor.$operationId.rollback"
$payloadNames = @('Supervisor.exe', 'Supervisor.dependencies.json',
    'LICENSE.txt', 'THIRD_PARTY_NOTICES.md', 'Supervisor.release.json')

if (-not ('SupervisorUpdateNative' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

public static class SupervisorUpdateNative {
    private delegate bool EnumWindowsCallback(IntPtr window, IntPtr parameter);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern uint RegisterWindowMessage(string name);

    [DllImport("user32.dll")]
    private static extern bool EnumWindows(EnumWindowsCallback callback, IntPtr parameter);

    [DllImport("user32.dll")]
    private static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);

    [DllImport("user32.dll")]
    private static extern bool PostMessage(IntPtr window, uint message, IntPtr wParam, IntPtr lParam);

    public static bool RequestStopAllAndQuit(uint targetProcessId) {
        uint message = RegisterWindowMessage("Supervisor.StopAllWorkAndQuit");
        bool posted = false;
        EnumWindows(delegate(IntPtr window, IntPtr parameter) {
            uint processId;
            GetWindowThreadProcessId(window, out processId);
            if (processId == targetProcessId) {
                posted = PostMessage(window, message, IntPtr.Zero, IntPtr.Zero) || posted;
            }
            return true;
        }, IntPtr.Zero);
        return posted;
    }
}
'@
}

function Assert-LocalBuildPath {
    param([Parameter(Mandatory = $true)][string]$Path)
    $fullPath = [IO.Path]::GetFullPath($Path)
    if (-not $fullPath.StartsWith($projectRoot + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw "Local build path is outside the repository: $fullPath"
    }
    $cursor = $fullPath
    while ($cursor.Length -ge $projectRoot.Length) {
        if (Test-Path -LiteralPath $cursor) {
            $entry = Get-Item -LiteralPath $cursor -Force
            if (($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
                throw "Local build paths must not traverse links: $cursor"
            }
        }
        if ($cursor -eq $projectRoot) { break }
        $cursor = Split-Path $cursor -Parent
    }
}

function Assert-LocalPackage {
    param([Parameter(Mandatory = $true)][string]$Path)
    Assert-LocalBuildPath $Path
    if (-not (Test-Path -LiteralPath $Path)) { return }
    if (-not (Test-Path -LiteralPath $Path -PathType Container)) {
        throw "Expected a local package directory: $Path"
    }
    foreach ($entry in Get-ChildItem -LiteralPath $Path -Force) {
        if ($entry.PSIsContainer -or $entry.Name -notin $payloadNames -or
            ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "Unexpected content in the local package; preserve it before updating: $($entry.FullName)"
        }
    }
}

function Remove-LocalBuildScratch {
    param([Parameter(Mandatory = $true)][string]$Path)
    if ($Path -notin @($stagingRoot, $backupRoot)) {
        throw "Refusing to remove a directory outside this update's scratch paths: $Path"
    }
    Assert-LocalPackage $Path
    if (Test-Path -LiteralPath $Path -PathType Container) {
        Remove-Item -LiteralPath (Resolve-Path -LiteralPath $Path).Path -Recurse -Force
    }
}

function Stop-CanonicalSupervisor {
    param(
        [Parameter(Mandatory = $true)][Diagnostics.Process]$Process,
        [Parameter(Mandatory = $true)][string]$ApplicationPath
    )

    if ($Process.HasExited) { return }
    $closed = $Process.CloseMainWindow() -and $Process.WaitForExit(2000)
    if (-not $closed) {
        $requested = [SupervisorUpdateNative]::RequestStopAllAndQuit([uint32]$Process.Id)
        $closed = $requested -and $Process.WaitForExit(30000)
    }
    if ($closed) { return }

    # Closing the main window intentionally moves Supervisor into the tray. If
    # that process cannot consume the registered quit request, force only the
    # exact canonical executable selected above. Re-read the physical path
    # immediately before termination so a reused PID or another Supervisor
    # build can never be affected.
    $live = Get-CimInstance Win32_Process -Filter "ProcessId = $($Process.Id)" -ErrorAction SilentlyContinue
    if ($null -eq $live -or -not $live.ExecutablePath -or
        -not [string]::Equals($live.ExecutablePath, $ApplicationPath, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Supervisor did not close normally and its exact executable path could not be revalidated. The package is unchanged.'
    }
    $verified = Get-Process -Id $Process.Id -ErrorAction SilentlyContinue
    if ($null -eq $verified -or $verified.HasExited) { return }
    Stop-Process -InputObject $verified -Force -ErrorAction Stop
    if (-not $verified.WaitForExit(10000)) {
        throw 'The exact canonical Supervisor process could not be stopped. Its executable has not been replaced.'
    }
}

function Start-CanonicalSupervisorIndependent {
    param(
        [Parameter(Mandatory = $true)][string]$ApplicationPath,
        [Parameter(Mandatory = $true)][string]$WorkingDirectory
    )

    if (-not (Test-Path -LiteralPath $ApplicationPath -PathType Leaf)) {
        throw "The updated Supervisor executable is missing: $ApplicationPath"
    }

    # A Codex child process can inherit package file redirection. Task
    # Scheduler starts Supervisor in the signed-in user's ordinary desktop
    # context instead, without opening a console window. The temporary task is
    # removed as soon as the canonical process is observed.
    $taskName = "Supervisor Restart $([Guid]::NewGuid().ToString('N'))"
    $registered = $false
    try {
        $action = New-ScheduledTaskAction -Execute $ApplicationPath -WorkingDirectory $WorkingDirectory
        $trigger = New-ScheduledTaskTrigger -Once -At (Get-Date).AddMinutes(5)
        $principal = New-ScheduledTaskPrincipal `
            -UserId ([Security.Principal.WindowsIdentity]::GetCurrent().Name) `
            -LogonType Interactive -RunLevel Limited
        Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger `
            -Principal $principal -Description 'One-time Supervisor desktop restart' | Out-Null
        $registered = $true
        Start-ScheduledTask -TaskName $taskName

        $deadline = [DateTime]::UtcNow.AddSeconds(20)
        do {
            Start-Sleep -Milliseconds 200
            $launched = @(Get-CimInstance Win32_Process -Filter "Name = 'Supervisor.exe'" -ErrorAction SilentlyContinue |
                Where-Object { $_.ExecutablePath -and [string]::Equals(
                    $_.ExecutablePath, $ApplicationPath, [StringComparison]::OrdinalIgnoreCase) })
        } while ($launched.Count -eq 0 -and [DateTime]::UtcNow -lt $deadline)

        if ($launched.Count -eq 0) {
            throw 'Windows did not start the updated Supervisor executable in the desktop context.'
        }
    } finally {
        if ($registered) {
            Unregister-ScheduledTask -TaskName $taskName -Confirm:$false -ErrorAction SilentlyContinue
        }
    }
}

function Publish-LocalPackage {
    Assert-LocalPackage $outputRoot
    Assert-LocalPackage $stagingRoot
    $hadPrevious = Test-Path -LiteralPath $outputRoot -PathType Container
    if ($hadPrevious) { [IO.Directory]::Move($outputRoot, $backupRoot) }
    try {
        [IO.Directory]::Move($stagingRoot, $outputRoot)
        & $manifestVerifier -ManifestPath (Join-Path $outputRoot 'Supervisor.release.json') | Out-Null
    } catch {
        if (Test-Path -LiteralPath $outputRoot -PathType Container) {
            # Move the failed package back to our exact scratch path, then restore.
            Assert-LocalPackage $outputRoot
            [IO.Directory]::Move($outputRoot, $stagingRoot)
        }
        if ($hadPrevious) { [IO.Directory]::Move($backupRoot, $outputRoot) }
        throw
    }
    Remove-LocalBuildScratch $backupRoot
}

foreach ($path in @($outputRoot, $targetRoot, $stagingRoot, $backupRoot)) {
    Assert-LocalBuildPath $path
}
Assert-LocalPackage $outputRoot
if (-not $PSCmdlet.ShouldProcess($outputRoot, 'Close Supervisor, build, verify and replace the local application')) {
    return
}

New-Item -ItemType Directory -Path $outputParent -Force | Out-Null
. (Join-Path $PSScriptRoot 'build-storage.ps1')
$storage = Enter-SupervisorBuild $projectRoot $targetRoot
$restartApplication = $false
try {
    $applicationPath = Join-Path $outputRoot 'Supervisor.exe'
    $running = @(Get-CimInstance Win32_Process -Filter "Name = 'Supervisor.exe'" |
        Where-Object { $_.ExecutablePath -and [string]::Equals($_.ExecutablePath, $applicationPath, [StringComparison]::OrdinalIgnoreCase) })
    foreach ($instance in $running) {
        $process = Get-Process -Id $instance.ProcessId -ErrorAction SilentlyContinue
        if ($null -eq $process -or $process.HasExited) { continue }
        Stop-CanonicalSupervisor -Process $process -ApplicationPath $applicationPath
        $restartApplication = $true
    }

    & node (Join-Path $PSScriptRoot 'verify-memory-removal.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Retired Memory runtime verification failed; the previous package is unchanged.' }
    & (Join-Path $PSScriptRoot 'build-frontend.ps1')
    & (Join-Path $PSScriptRoot 'update-licenses.ps1') -Check
    Push-Location $projectRoot
    try {
        & cargo build --locked --release --workspace --target-dir $targetRoot
        if ($LASTEXITCODE -ne 0) { throw 'The local Rust build failed; the previous package is unchanged.' }
    } finally { Pop-Location }

    New-Item -ItemType Directory -Path $stagingRoot | Out-Null
    [IO.File]::Copy(
        (Join-Path $targetRoot 'release\central-agent.exe'),
        (Join-Path $stagingRoot 'Supervisor.exe'),
        $false)
    $startup = Start-Process -FilePath (Join-Path $stagingRoot 'Supervisor.exe') -ArgumentList '--check-ui-startup' -WindowStyle Hidden -Wait -PassThru
    if ($startup.ExitCode -ne 0) { throw 'The staged application failed its hidden startup check.' }
    & (Join-Path $PSScriptRoot 'generate-dependency-inventory.ps1') -OutputPath (Join-Path $stagingRoot 'Supervisor.dependencies.json') | Out-Null
    [IO.File]::Copy((Join-Path $projectRoot 'LICENSE'), (Join-Path $stagingRoot 'LICENSE.txt'), $false)
    [IO.File]::Copy((Join-Path $projectRoot 'THIRD_PARTY_NOTICES.md'), (Join-Path $stagingRoot 'THIRD_PARTY_NOTICES.md'), $false)
    $artifacts = @(& (Join-Path $PSScriptRoot 'verify-release.ps1') -Path @(
        (Join-Path $stagingRoot 'Supervisor.exe')
    ) -AllowUnsignedDevelopment)
    $supporting = @(foreach ($name in @('Supervisor.dependencies.json', 'LICENSE.txt', 'THIRD_PARTY_NOTICES.md')) {
        $file = Get-Item -LiteralPath (Join-Path $stagingRoot $name)
        [ordered]@{ file = $name; bytes = $file.Length; sha256 = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant() }
    })
    $head = (& git -C $projectRoot rev-parse --verify HEAD).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Could not record the local source commit.' }
    $changes = @(& git -C $projectRoot status --porcelain=v1 --untracked-files=normal)
    if ($LASTEXITCODE -ne 0) { throw 'Could not record the local working-tree state.' }
    $inventory = Get-Content -LiteralPath (Join-Path $stagingRoot 'Supervisor.dependencies.json') -Raw | ConvertFrom-Json
    [ordered]@{
        schema = 'central-agent.release-manifest'; schemaVersion = 2
        application = 'Supervisor'; applicationVersion = $inventory.applicationVersion
        target = 'windows'; profile = 'release'; releaseMode = 'unsigned-development'
        launcher = 'Supervisor.exe'; generatedAtUtc = [DateTimeOffset]::UtcNow.ToString('o')
        sourceCommit = $head; sourceHasLocalChanges = ($changes.Count -gt 0)
        artifacts = $artifacts; supportingFiles = $supporting
    } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $stagingRoot 'Supervisor.release.json') -Encoding utf8
    & $manifestVerifier -ManifestPath (Join-Path $stagingRoot 'Supervisor.release.json') | Out-Null
    Publish-LocalPackage
    Write-Output "Updated local application: $applicationPath"
} finally {
    try {
        Remove-LocalBuildScratch $stagingRoot
        if (Test-Path -LiteralPath $backupRoot) {
            Write-Warning "Previous package retained for recovery: $backupRoot"
        }
    } finally {
        Exit-SupervisorBuild $storage
        if ($restartApplication -and (Test-Path -LiteralPath (Join-Path $outputRoot 'Supervisor.exe') -PathType Leaf)) {
            Start-CanonicalSupervisorIndependent `
                -ApplicationPath (Join-Path $outputRoot 'Supervisor.exe') `
                -WorkingDirectory $projectRoot
        }
    }
}
