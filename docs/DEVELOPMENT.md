# Development on Windows

## Requirements

- Windows 10/11 x64 with Microsoft Edge WebView2 Runtime.
- Git; Node.js 24 with npm.
- Rust MSVC toolchain from `rust-toolchain.toml` (currently 1.95.0).
- Visual Studio 2022 Build Tools with Desktop development with C++, MSVC and
  the Windows SDK/resource compiler. Run builds from a Developer PowerShell
  if these tools are not on PATH.
- `cargo-audit` for the full check: `cargo install cargo-audit --locked`.
- Provider runtimes/accounts only for live provider testing. They are not needed
  for unit tests and are not installed or authenticated by CI.

## Build and check

From the repository root:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify-workspace.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/update-supervisor.ps1
```

The updater installs frontend dependencies from the lockfile, checks/rebuilds the
frontend, compiles Rust and updates `outputs/Supervisor/Supervisor.exe` in place.
It closes only that canonical Supervisor copy and restores it if previously
running, preserving the Windows execution context and profile. It never closes
the official Codex app. An unsuccessful build retains the previous package.
The local executable is unsigned and is not a production release.

For UI iteration, run `npm run check` in `ui`, then
`scripts/build-frontend.ps1`. Commit generated `ui/dist` changes with the source.
Do not edit generated bundles. See [DESIGN.md](../DESIGN.md).

Build/test entry points share `scripts/build-storage.ps1`, a lock, temporary
directory ownership and the `target/` cache. The default post-build Cargo cache
budget is 4 GiB. No extra dated executable copies are needed. See
[WORKSPACE_STORAGE.md](WORKSPACE_STORAGE.md) for diagnostic overrides.

## Verification levels

`verify-workspace.ps1` runs frontend type/design checks, UI logic and protocol
tests, packaging/storage regressions, Rust formatting/check/tests/Clippy and
dependency auditing. Desktop and authenticated model probes are separate opt-in
tests; do not run them automatically or claim CI covers them.

After a lockfile change, run `scripts/update-licenses.ps1`, review the resulting
inventory and commit it with `THIRD_PARTY_NOTICES.md`. CI checks for drift.
Run `scripts/scan-secrets.ps1` for a redacted full-history and tracked-file scan.

## Providers

Install and sign in to the desired supported provider through its official
distribution, then connect it in Supervisor's AI accounts settings. Supported
Codex versions and the protocol update procedure are recorded in
[protocol/app-server](../protocol/app-server/README.md). Availability of plugins,
cloud tasks and account features depends on the installed runtime and account.
An account subscription does not grant arbitrary API access or redistribution
rights to official desktop components.

Original implementation notes remain in `docs/`. Dated audits record historical
tests; they are not evidence that the current build passed the same live tests.
