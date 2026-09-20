#requires -Version 5.1
# Shared by build entry points. Dot-sourcing this file does not create files.

function Assert-SupervisorStoragePath {
    param([string]$ProjectRoot, [string]$Path)
    $root = [IO.Path]::GetFullPath((Join-Path $ProjectRoot 'target')).TrimEnd('\')
    $full = [IO.Path]::GetFullPath($Path).TrimEnd('\')
    if ($full -ne $root -and -not $full.StartsWith($root + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw "Build storage must stay inside the repository target directory: $full"
    }
    $cursor = $full
    while ($cursor.Length -ge $ProjectRoot.Length) {
        if (Test-Path -LiteralPath $cursor) {
            if (((Get-Item -LiteralPath $cursor -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
                throw "Build storage must not traverse links: $cursor"
            }
        }
        if ($cursor -eq $ProjectRoot) { break }
        $cursor = Split-Path $cursor -Parent
    }
    return $full
}

function Get-SupervisorStorageFiles {
    param([string]$ProjectRoot, [string]$Path)
    $full = Assert-SupervisorStoragePath $ProjectRoot $Path
    if (-not (Test-Path -LiteralPath $full)) { return }
    # Walk explicitly: never follow a junction while measuring or deleting.
    $pending = New-Object 'System.Collections.Generic.Stack[string]'
    $pending.Push($full)
    while ($pending.Count -gt 0) {
        foreach ($entry in Get-ChildItem -LiteralPath $pending.Pop() -Force) {
            if (($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
                throw "Preserving build storage containing a link: $($entry.FullName)"
            }
            if ($entry.PSIsContainer) { $pending.Push($entry.FullName) } else { $entry }
        }
    }
}

function Remove-SupervisorBuildSession {
    param([string]$ProjectRoot, [string]$Path)
    $full = Assert-SupervisorStoragePath $ProjectRoot $Path
    $sessions = Join-Path $ProjectRoot 'target\.supervisor-build\sessions'
    if ((Split-Path $full -Parent) -ne $sessions -or (Split-Path $full -Leaf) -notmatch '^[a-f0-9]{32}$') {
        throw "Refusing to remove an unowned build session: $full"
    }
    if (Test-Path -LiteralPath $full) {
        $null = @(Get-SupervisorStorageFiles $ProjectRoot $full)
        Remove-Item -LiteralPath $full -Recurse -Force
    }
}

function Clear-SupervisorAbandonedSessions {
    param([string]$ProjectRoot)
    $sessions = Assert-SupervisorStoragePath $ProjectRoot (Join-Path $ProjectRoot 'target\.supervisor-build\sessions')
    if (-not (Test-Path -LiteralPath $sessions)) { return }
    foreach ($entry in Get-ChildItem -LiteralPath $sessions -Directory -Force) {
        if ($entry.Name -notmatch '^[a-f0-9]{32}$' -or $entry.LastWriteTimeUtc -gt [DateTime]::UtcNow.AddDays(-7)) { continue }
        try {
            $null = Assert-SupervisorStoragePath $ProjectRoot $entry.FullName
            $owner = Get-Content -LiteralPath (Join-Path $entry.FullName 'owner.json') -Raw | ConvertFrom-Json
            if ($owner.schema -ne 'supervisor.build-session') { continue }
            # Conservatively preserve a session if its owning PID still exists.
            if (Get-Process -Id $owner.pid -ErrorAction SilentlyContinue) { continue }
            Remove-SupervisorBuildSession $ProjectRoot $entry.FullName
        } catch { Write-Warning "Preserved an abandoned build session: $($_.Exception.Message)" }
    }
}

function Invoke-SupervisorCacheBudget {
    param([string]$ProjectRoot, [long]$LimitBytes = 4GB)
    $target = Assert-SupervisorStoragePath $ProjectRoot (Join-Path $ProjectRoot 'target')
    $profiles = @(foreach ($relative in @('debug', 'release', 'redistributable\debug', 'redistributable\release')) {
        $path = Join-Path $target $relative
        if (-not (Test-Path -LiteralPath $path)) { continue }
        $files = @(Get-SupervisorStorageFiles $ProjectRoot $path)
        $bytes = [long](($files | Measure-Object -Property Length -Sum).Sum)
        $latest = ($files | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1).LastWriteTimeUtc
        [pscustomobject]@{ Path = $path; Bytes = $bytes; LastWrite = $latest }
    })
    $total = [long](($profiles | Measure-Object -Property Bytes -Sum).Sum)
    if ($total -le $LimitBytes) { return }
    # Build scripts share a project lock. Also defer for direct Cargo commands
    # and executables running from a cache, instead of interrupting their work.
    if (Get-Process -Name cargo, rustc, rustdoc -ErrorAction SilentlyContinue) {
        Write-Warning 'Cache cleanup deferred while a Rust build is running.'
        return
    }
    $runningPaths = @(Get-CimInstance Win32_Process | ForEach-Object { $_.ExecutablePath } | Where-Object { $_ })
    foreach ($profile in $profiles | Sort-Object LastWrite) {
        if ($total -le $LimitBytes) { break }
        if ($runningPaths | Where-Object { $_.StartsWith($profile.Path + '\', [StringComparison]::OrdinalIgnoreCase) }) {
            Write-Warning "Cache cleanup deferred for a running application: $($profile.Path)"
            continue
        }
        # Revalidate the exact tree immediately before native Cargo cleanup.
        $null = @(Get-SupervisorStorageFiles $ProjectRoot $profile.Path)
        $cargoProfile = if ((Split-Path $profile.Path -Leaf) -eq 'debug') { 'dev' } else { 'release' }
        & cargo clean --offline --manifest-path (Join-Path $ProjectRoot 'Cargo.toml') --target-dir (Split-Path $profile.Path -Parent) --profile $cargoProfile
        if ($LASTEXITCODE -ne 0) { throw "Cargo could not clean $($profile.Path)." }
        $total -= $profile.Bytes
        Write-Host ('Reclaimed {0:N2} GiB from build cache: {1}' -f ($profile.Bytes / 1GB), $profile.Path)
    }
}

function Enter-SupervisorBuild {
    param([string]$ProjectRoot, [string]$TargetDirectory)
    $root = (Resolve-Path -LiteralPath $ProjectRoot).Path.TrimEnd('\')
    $scopes = Get-Variable -Name SupervisorBuildScopes -Scope Global -ErrorAction SilentlyContinue
    if (-not $scopes) { $global:SupervisorBuildScopes = @{} }
    if ($global:SupervisorBuildScopes.ContainsKey($root)) {
        $context = $global:SupervisorBuildScopes[$root]
        if ($TargetDirectory -and [IO.Path]::GetFullPath($TargetDirectory).TrimEnd('\') -ne $context.TargetDirectory) {
            throw 'Nested builds must use the same Cargo cache as their caller.'
        }
        $context.Depth++
        return $context
    }
    if (-not $TargetDirectory) {
        $TargetDirectory = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $root 'target' }
    }
    $metadata = Assert-SupervisorStoragePath $root (Join-Path $root 'target\.supervisor-build')
    $lockPath = Assert-SupervisorStoragePath $root (Join-Path $metadata 'build.lock')
    $sessions = Assert-SupervisorStoragePath $root (Join-Path $metadata 'sessions')
    $session = Assert-SupervisorStoragePath $root (Join-Path $sessions ([Guid]::NewGuid().ToString('N')))
    $limit = 4GB
    if ($env:SUPERVISOR_BUILD_CACHE_GB) {
        $configured = 0
        if (-not [int]::TryParse($env:SUPERVISOR_BUILD_CACHE_GB, [ref]$configured) -or $configured -lt 1 -or $configured -gt 1024) {
            throw 'SUPERVISOR_BUILD_CACHE_GB must be an integer between 1 and 1024.'
        }
        $limit = [long]$configured * 1GB
    }
    New-Item -ItemType Directory -Path $metadata -Force | Out-Null
    try {
        $lock = [IO.File]::Open($lockPath, [IO.FileMode]::OpenOrCreate, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
    } catch { throw 'Another Supervisor build is using this project. Wait for it to finish before retrying.' }
    try {
        Clear-SupervisorAbandonedSessions $root
        New-Item -ItemType Directory -Path $session -Force | Out-Null
        @{ schema = 'supervisor.build-session'; pid = $PID } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $session 'owner.json') -Encoding utf8
        # Stop parent Git discovery, including in code that correctly clears
        # inherited GIT_* variables. Repositories initialized below this marker
        # still work; empty temporary directories cannot see the real .git.
        [IO.File]::WriteAllText((Join-Path $session '.git'), "gitdir: .supervisor-no-parent-repository`n")
        $temporary = Join-Path $session 'tmp'
        New-Item -ItemType Directory -Path $temporary | Out-Null
        $context = [pscustomobject]@{
            ProjectRoot = $root; TargetDirectory = [IO.Path]::GetFullPath($TargetDirectory).TrimEnd('\')
            Metadata = $metadata; Session = $session; Lock = $lock; Depth = 1; LimitBytes = $limit
            PreviousTarget = $env:CARGO_TARGET_DIR; PreviousTemp = $env:TEMP; PreviousTmp = $env:TMP
        }
        $env:CARGO_TARGET_DIR = $context.TargetDirectory
        $env:TEMP = $temporary
        $env:TMP = $temporary
        $global:SupervisorBuildScopes[$root] = $context
        return $context
    } catch {
        try { Remove-SupervisorBuildSession $root $session }
        finally { $lock.Dispose() }
        throw
    }
}

function Exit-SupervisorBuild {
    param([Parameter(Mandatory = $true)]$Context)
    $Context.Depth--
    if ($Context.Depth -gt 0) { return }
    if ($Context.Depth -lt 0) { throw 'Build scope was already closed.' }
    # Restore before callers restart the real application; it must not inherit
    # the disposable temporary directory used by build and test processes.
    $env:CARGO_TARGET_DIR = $Context.PreviousTarget
    $env:TEMP = $Context.PreviousTemp
    $env:TMP = $Context.PreviousTmp
    try {
        try { Remove-SupervisorBuildSession $Context.ProjectRoot $Context.Session }
        catch { Write-Warning "Build temporary files retained for a later cleanup: $($_.Exception.Message)" }
        try { Invoke-SupervisorCacheBudget $Context.ProjectRoot $Context.LimitBytes }
        catch { Write-Warning "Build cache cleanup deferred: $($_.Exception.Message)" }
    } finally {
        $global:SupervisorBuildScopes.Remove($Context.ProjectRoot)
        $Context.Lock.Dispose()
    }
}
