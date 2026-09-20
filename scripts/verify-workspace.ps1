#requires -Version 5.1
[CmdletBinding()]
param(
    [switch]$SkipTests,
    [switch]$SkipDependencyAudit
)

$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path

function Invoke-CargoChecked {
    param([Parameter(Mandatory = $true)][string[]]$Arguments)

    & cargo @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo verification failed: cargo $($Arguments -join ' ')"
    }
}

. (Join-Path $PSScriptRoot 'build-storage.ps1')
$storage = Enter-SupervisorBuild $projectRoot
Push-Location $projectRoot
try {
    if (-not (Get-Command node -ErrorAction SilentlyContinue)) {
        throw 'Node.js is required to build the Svelte frontend and parse-check the embedded WebView scripts.'
    }
    & node --test (Join-Path $PSScriptRoot 'test-publication.mjs') (Join-Path $PSScriptRoot 'test-license-policy.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Publication boundary or license policy verification failed.' }
    & node (Join-Path $PSScriptRoot 'verify-memory-removal.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Retired Memory runtime verification failed.' }
    & node --test (Join-Path $PSScriptRoot 'test-computer-use-timing.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Computer Use timing probe verification failed.' }
    & (Join-Path $PSScriptRoot 'build-frontend.ps1')
    & (Join-Path $PSScriptRoot 'update-licenses.ps1') -Check
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/chat-tree.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Project chat ancestry verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/project-board.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Project board isolation and file navigation verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/native-delegation.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Native delegation and event diagnostics verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/file-editor.test.mjs')
    if ($LASTEXITCODE -ne 0) {
        throw 'Source editor state verification failed.'
    }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/diff-search.test.mjs')
    if ($LASTEXITCODE -ne 0) {
        throw 'Diff search verification failed.'
    }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/agent-timeline.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Agent timeline verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/graph-contexts.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Graph tab/shell context verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/composer-receipt.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Composer receipt ownership verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/conversation-drafts.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Conversation draft navigation verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/project-diff.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Project Git diff verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/file-attachments.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Conversation file-attachment presentation verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/graph-history.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Graph conversation timeline verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/timeline-dom.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Shared timeline DOM identity verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/native-media.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Native media presentation verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/mcp-options.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Native MCP option editor verification failed.' }
    & node (Join-Path $PSScriptRoot 'verify-provider-reset.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Provider reset boundary verification failed.' }
    & node (Join-Path $PSScriptRoot 'verify-app-server-contract.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Official App Server contract verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/app-server-account.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Official App Server account presentation verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/native-p2.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Optional App Server P2 workflow verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/native-requests.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Native App Server request presentation verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/native-access.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Native access and summary selection verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/conversation-events.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Conversation event isolation verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/native-steering.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Native steering composer verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/native-commands.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Native command shortcut verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/native-usage.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Native token usage tests failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/native-lifecycle.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Native conversation lifecycle verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/native-mcp.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Native MCP settings verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/native-apps.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Native Apps selection verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/native-hooks.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Native hooks presentation verification failed.' }
    & node --test (Join-Path $projectRoot 'ui/tests/native-mcp-progress-fixture.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Native MCP progress fixture verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/native-mcp-configuration.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Native MCP configuration confirmation verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/request-id.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Internal WebView correlation ID verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/native-history.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Native history browser verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/native-skills.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Native skill component verification failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/native-preferences.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Native preferences tests failed.' }
    & node --experimental-strip-types --test (Join-Path $projectRoot 'ui/tests/native-goals.test.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Native goal controls verification failed.' }
    & node (Join-Path $PSScriptRoot 'verify-inline-js.mjs')
    if ($LASTEXITCODE -ne 0) {
        throw 'Embedded WebView JavaScript verification failed.'
    }
    & node (Join-Path $PSScriptRoot 'verify-ui-theme.mjs')
    if ($LASTEXITCODE -ne 0) {
        throw 'Static UI theme verification failed.'
    }
    & (Join-Path $PSScriptRoot 'test-release-pipeline.ps1')
    & (Join-Path $PSScriptRoot 'test-build-storage.ps1')
    & (Join-Path $PSScriptRoot 'test-update-supervisor.ps1')
    Invoke-CargoChecked -Arguments @('fmt', '--all', '--', '--check')
    Invoke-CargoChecked -Arguments @('check', '--locked', '--workspace', '--all-targets')
    if (-not $SkipTests) {
        Invoke-CargoChecked -Arguments @('test', '--locked', '--workspace')
        # Unit-test native probes' acceptance oracles without starting
        # Codex or consuming account inference during normal verification.
        Invoke-CargoChecked -Arguments @('test', '--locked', '-p', 'central-agent-codex-runtime', '--example', 'review_scope_probe')
        Invoke-CargoChecked -Arguments @('test', '--locked', '-p', 'central-agent-codex-runtime', '--example', 'permission_scope_probe')
        Invoke-CargoChecked -Arguments @('test', '--locked', '-p', 'central-agent-codex-runtime', '--example', 'computer_use_speed_probe')
    }
    Invoke-CargoChecked -Arguments @('clippy', '--locked', '--workspace', '--all-targets', '--', '-D', 'warnings')
    if (-not $SkipDependencyAudit) {
        if (-not (Get-Command cargo-audit -ErrorAction SilentlyContinue)) {
            throw 'cargo-audit is required. Install it with: cargo install cargo-audit --locked'
        }
        # RUSTSEC-2023-0071 is confined to IronRDP's client-side certificate/SSPI graph.
        # Supervisor never supplies an RSA private key to that dependency. See
        # docs/DEPENDENCY_SECURITY.md for the scoped risk acceptance and removal condition.
        Invoke-CargoChecked -Arguments @('audit', '--ignore', 'RUSTSEC-2023-0071')
    }
} finally {
    Pop-Location
    Exit-SupervisorBuild $storage
}

Write-Output 'Workspace verification passed.'
