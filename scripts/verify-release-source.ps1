#requires -Version 5.1
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path

if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    throw 'Git is required to prove that a release is built from committed source.'
}

$insideWorktree = & git -C $projectRoot rev-parse --is-inside-work-tree 2>$null
if ($LASTEXITCODE -ne 0 -or $insideWorktree.Trim() -ne 'true') {
    throw 'Release source must be a Git worktree.'
}

$requiredHeadPaths = @(
    'Cargo.lock',
    'Cargo.toml',
    'crates/central-agent-codex-runtime/Cargo.toml',
    'crates/central-agent-codex-runtime/src/transport.rs',
    'docs/CODEX_APP_SERVER_IMPLEMENTATION_GUIDE.md',
    'protocol/app-server/0.155.1/json/ClientRequest.json',
    'protocol/app-server/0.155.1/typescript/ClientRequest.ts',
    'scripts/verify-app-server-contract.mjs',
    'scripts/verify-release-source.ps1',
    'src/browser/app_server.rs'
)
foreach ($path in $requiredHeadPaths) {
    & git -C $projectRoot cat-file -e "HEAD:$path" 2>$null
    if ($LASTEXITCODE -ne 0) {
        throw "Release source is incomplete at HEAD; required committed path is missing: $path"
    }
}

$worktreeChanges = @(& git -C $projectRoot status --porcelain=v1 --untracked-files=all)
if ($LASTEXITCODE -ne 0) {
    throw 'Could not inspect the release source worktree.'
}
if ($worktreeChanges.Count -gt 0) {
    $sample = ($worktreeChanges | Select-Object -First 12) -join [Environment]::NewLine
    throw "Release source is not reproducible from HEAD. Commit, remove, or ignore every local change before packaging:`n$sample"
}

$head = (& git -C $projectRoot rev-parse --verify HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $head -notmatch '^[0-9a-f]{40}$') {
    throw 'Could not resolve the release source commit.'
}

Write-Output "Release source is clean and reproducible at $head."
