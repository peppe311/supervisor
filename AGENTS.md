# Supervisor repository instructions

## Interface work

Read `DESIGN.md` before changing any user-facing surface. It is the product's
visual and interaction contract, not a mood board.

- Keep `assets/themes.css` as the only source of shared visual tokens.
- Build new interface ownership in `ui/src/components` with Svelte and
  TypeScript. Do not add a second frontend framework.
- Preserve existing DOM IDs, `data-*` hooks, IPC messages, and Rust ownership
  while a legacy surface is migrated.
- Do not edit `ui/dist` by hand. Run `scripts/build-frontend.ps1` and commit the
  generated bundle together with its source change.
- Update `ui/evals/scenarios.json` when a new primary surface or material state
  is introduced.
- Run `npm run check` from `ui` for a focused UI check, or
  `scripts/verify-workspace.ps1` for the complete workspace verification.

The deterministic checks are a floor. A user-facing visual change still needs
inspection in every affected theme, relevant state, and supported layout
profile.

## Local builds and cleanup

The user wants one local executable updated in place. Use
`scripts/update-supervisor.ps1` and deliver `outputs/Supervisor/Supervisor.exe`.
Do not create dated preview folders, extra executable copies, or ZIP bundles
for ordinary feature updates. The updater closes only Supervisor instances
launched from this canonical path, rebuilds the frontend and Rust workspace,
checks the staged application, and replaces the existing local package. It
keeps the previous package if building or publication fails and reopens the
application if it was running. Never terminate the official Codex app.

All build entry points must enter/exit `scripts/build-storage.ps1` in `try/finally`.
Use `target/` as the shared local compilation cache; the static-CRT distribution
variant lives in `target/redistributable/`. Do not introduce caches in AppData or
sibling folders. The helper owns build/test temporary directories, a shared
build lock, and a 4 GiB post-build Cargo cache budget. Keep useful cache entries
below that threshold rather than cleaning and recompiling on every update.
Explicit external cache overrides are never cleaned automatically. See
`docs/WORKSPACE_STORAGE.md` for limits and diagnostic overrides. Use
`build-frontend.ps1` to reuse unchanged bundles while still checking types.

Remove obsolete build
copies, test profiles and temporary caches only after checking their resolved
paths and confirming they are generated artifacts. The user has requested a
fully cleaned output directory: keep only the current package in `outputs/`.
Keep lasting brand deliverables in `docs/brand/` and canonical artwork in
`assets/`. Do not retain dated output folders, source snapshots, screenshots,
raw audit logs or test profiles after the relevant results have been recorded
in project documentation. Use temporary directories for probes and clean them
afterward. The updater lock belongs in `target/`, not the delivery directory.
Preserve source changes and actual application/account data. The signed-release
scripts retain their separate release requirements.

Do not perform visual checks through Computer Use unless the user explicitly
requests them. Hidden deterministic startup checks are allowed.

## Windows application data

Codex-launched maintenance processes can inherit Windows file redirection:
reads and writes under `%LOCALAPPDATA%\CentralAgent\CentralAgent\data` may
actually reach Codex's package `LocalCache`. Package identity checks alone do
not detect this. Supervisor opened normally from Explorer uses the actual
LocalAppData store. Verify physical file paths in an independently launched
Windows process before changing production chat data. Do not relax path guards
to accept the package overlay as equivalent to the actual application store.

Preserve the running app's Windows execution context when restarting it.
A direct child launch from Codex can reopen the other profile. Use a hidden
independent maintenance launcher when restoring a normally launched Supervisor
instance, then verify its own native-history read responses in the actual data
directory. Keep existing accounts, databases, unrelated chats and originals;
never solve a profile mismatch by copying credentials or whole native databases.
