# Workspace storage

The standard update command is `scripts/update-supervisor.ps1`. It publishes
one current package at `outputs/Supervisor/`, closes only that running copy of
Supervisor, and reopens it after the update if necessary. The previous package
survives a failed build or publication. Staging files are temporary; rollback
material is retained only when recovery requires it.

## Preventing accumulation

- Development and test profiles disable full debug symbols and incremental
  compilation. Debug assertions and overflow checks remain enabled. Release
  artifacts strip debug information. Cargo still reuses unchanged dependencies;
  disabling incremental compilation does not force a full rebuild every time.
- Local updates, workspace verification, MCP builds, UI development and regular
  releases share `target/`. Static-CRT distribution builds use
  `target/redistributable/` because their compiler flags differ. No default build
  creates a second cache in AppData or a sibling Documents folder.
- The MCP build publishes the complete current Supervisor package, including its
  verified companion and manifest, instead of another standalone executable.
  Production release commands retain their clean-source and signing gates.
- Frontend dependencies stay in `ui/node_modules/`. The locked dependencies are
  installed when missing or when the lockfile changes. Keeping this directory
  avoids repeated installation and downloads during normal work.
- Frontend builds hash source files, assets, configuration, build scripts and
  the Node.js version. They also verify every generated bundle file's hash.
  Unchanged input and output reuse `ui/dist/` without touching its timestamps.
  Type and design checks still run. Missing, edited or corrupt outputs trigger
  rebuilding. `scripts/build-frontend.ps1 -Force` explicitly rebuilds a bundle.

Cargo's [profile documentation](https://doc.rust-lang.org/cargo/reference/profiles.html)
describes the debug and incremental settings; its
[build cache documentation](https://doc.rust-lang.org/cargo/reference/build-cache.html)
explains the reusable target directory.

## Temporary files and cache budget

Every build entry point uses `scripts/build-storage.ps1`. The outermost command
holds a shared project lock and gives its child processes a disposable TEMP/TMP
directory under `target/.supervisor-build/sessions/`. Nested build commands reuse
the same session. Cleanup happens only after the entire outer command finishes,
including on failure. Build checks cannot evict artifacts before packaging has
copied them. A Git discovery boundary prevents empty temporary directories from
inheriting the real repository's Git state. The real application is restarted
with the original environment.

UI watcher logs stay in that session and are removed after development exits.
Successful watcher rebuild chatter is suppressed. If an interrupted process
leaves a session behind, a later build reclaims it after seven days when its
recorded owner PID is no longer running. Locked files are left for a later
attempt. Unknown sessions and live owners are preserved.

At the end of a managed command, the helper measures the combined contents of
`target/debug`, `target/release`, and the two corresponding redistributable
profiles. Above **4 GiB**, it asks Cargo to remove the least recently written
profile first, stopping once the remaining profiles fit. Below the threshold
the cache is kept. Cleanup is deferred while another Rust build is running or
an executable is running from the selected profile. A shared lock prevents
concurrent managed builds and cleanup in this repository.

This is a **post-build cleanup threshold**, not a hard filesystem quota:
compilation may temporarily exceed it. A busy or linked cache is preserved.
Custom Cargo target triples/profiles and explicit external cache overrides are
outside automatic eviction. Direct `cargo` commands use the compact profiles,
but automatic temporary-file management and eviction require the build scripts.

Set `SUPERVISOR_BUILD_CACHE_GB` to an integer from 1 to 1024 in the current shell
to override the threshold for managed commands. A smaller value can cause more
recompilation; 4 GiB is the default balance for this workspace. Existing explicit
`CARGO_TARGET_DIR`, `CENTRAL_AGENT_BUILD_TARGET_DIR`,
`CENTRAL_AGENT_DEV_TARGET_DIR` and `CENTRAL_AGENT_REDISTRIBUTABLE_TARGET_DIR`
overrides remain available where applicable, but automatic cleanup never deletes
an external cache. The local updater always uses the canonical `target/`.

## Boundaries and diagnostics

Cleanup only acts on known generated paths inside this repository's `target/`.
Absolute path and junction checks precede recursive operations. It does not
delete source files, `ui/dist/`, `vendor/`, `docs/brand/`, the current application,
actual chat/account profiles, user artifacts, or Time Machine checkpoints.
Existing runtime retention policies remain in effect. Global Cargo/npm package
download caches are shared with other projects and are outside this budget.

For a debugging session requiring full Rust symbols, temporarily set
`CARGO_PROFILE_DEV_DEBUG=2` (and `CARGO_PROFILE_TEST_DEBUG=2` for tests).
Remove these overrides afterward; symbols consume additional space and can
cause the next managed cleanup to evict that profile. Do not permanently
reenable incremental output just for an ordinary feature update.

`scripts/test-build-storage.ps1` checks actual path and junction boundaries,
nested and failed build cleanup, environment restoration, exclusive locking,
expired/live sessions, external-cache preservation, real Cargo cache eviction,
and frontend invalidation. `scripts/test-update-supervisor.ps1` checks package
recovery and replacement. Both are part of `scripts/verify-workspace.ps1`.

## Verification on 2026-09-11

The complete workspace gate passed: 767 Rust tests (including the three scope
oracle tests), 190 frontend tests, formatting, locked checks, Clippy, release
probes and dependency audit. The opt-in/live cases remained skipped as intended
(24 Rust tests and one frontend test). The audit retained its four allowed
dependency warnings and existing scoped exception; this storage change does not
claim to resolve those dependency advisories.

Storage and update probes also passed in Windows PowerShell 5.1. The actual
local update passed its hidden startup check, MCP doctor and package manifest
verification. Frontend hashes and timestamps were identical before and after
that update. No Computer Use checks were performed.

After verification and rebuilding the current package:

| Location | Logical file size |
| --- | ---: |
| Development/test cache | 2.368 GiB |
| Release cache | 1.521 GiB |
| Combined `target/` | 3.889 GiB |
| Frontend dependencies | 75.46 MiB |
| Current six-file application package | 39.28 MiB |

There were no remaining build sessions, no incremental compilation files,
and `outputs/` contained only `Supervisor/`. The combined cache was below the
4 GiB threshold and was retained for reuse. Logical file sizes include any
hard-linked artifacts; physical allocation can differ.
