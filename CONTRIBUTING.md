# Contributing to Supervisor

Supervisor is an experimental Windows desktop agent workspace. Small, focused
changes with a clear reproduction or use case are easiest to review.

1. Read [the design contract](DESIGN.md), [architecture](docs/ARCHITECTURE.md)
   and [development guide](docs/DEVELOPMENT.md).
2. Fork the repository, create a branch and keep unrelated edits separate.
3. Make the change and run `scripts/verify-workspace.ps1`. For a focused UI
   iteration run `npm run check` in `ui`; run the full check before submitting.
4. Run `scripts/build-frontend.ps1` for frontend changes and commit `ui/dist`
   together with its source. Do not edit generated bundles or schemas by hand.
5. Update relevant tests and documentation. New UI surfaces or material states
   need an entry in `ui/evals/scenarios.json` and visual evidence for affected
   themes/layouts. Never run desktop automation on someone else's session
   without their explicit request.
6. Open a pull request explaining the problem, changed behavior, validation and
   remaining limits. Include redacted screenshots only when useful.

Never commit credentials, account stores, chat histories, browser profiles,
SSH keys, captured terminal output or private project files. Run
`scripts/scan-secrets.ps1` before pushing. A detected credential must be revoked;
deleting the latest file alone does not remove it from Git history.

Dependency changes must update the lockfile, notices and inventory with
`scripts/update-licenses.ps1`. Explain new privileges, network destinations or
runtime components. Keep the Rust host as the owner of permissions and IPC.
Do not add a second frontend framework or redistribute proprietary plugins.

By submitting a contribution you agree to license your original contribution
under MPL-2.0 and confirm you have the right to submit it. Existing third-party
files keep their own licenses. No copyright assignment is required.

Report vulnerabilities privately using [SECURITY.md](SECURITY.md). Ordinary
bugs and feature proposals belong in GitHub Issues. Follow our
[code of conduct](CODE_OF_CONDUCT.md).
