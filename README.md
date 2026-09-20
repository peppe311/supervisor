# Supervisor

A Windows desktop workspace for project agents and the agents that supervise them.

Supervisor brings project conversations, supervisor conversations, project files,
a browser and terminals into one local interface. Supervisors can observe linked
agents and steer their work while you retain control over access and execution.

**Status: experimental source release.** The app is under active development;
there is no production-certified or signed public executable yet.

## What is included

- Project onboarding from a local folder, Git clone or configured SSH directory.
- Project chats and supervisor chats, activity timelines, file attachments,
  streaming diffs, context usage and stop/resume controls.
- Official Codex App Server integration with native approvals, steering, forks,
  history and provider/account features exposed by supported runtime versions.
- Plugin selection and integration with separately installed provider tools.
- Embedded browser, local/remote terminals and background work in the Windows tray.

Provider capabilities vary. Codex cloud tasks, plugins and other account features
require an eligible account and compatible runtime; other providers do not
inherit the Codex protocol. Supervisor does not provide free API usage or bundle
provider subscriptions, official desktop plugins or proprietary runtimes.

## Build on Windows

Install Git, Node.js 24, Rust MSVC (the pinned toolchain is selected automatically),
Visual Studio 2022 C++ Build Tools with Windows SDK, and WebView2 Runtime.

```powershell
git clone https://github.com/peppe311/supervisor.git
cd supervisor
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/update-supervisor.ps1
```

Open `outputs/Supervisor/Supervisor.exe`. The updater reuses this path, replaces
only the local Supervisor package, and preserves its profile. Builds use the
shared `target/` cache with a 4 GiB default post-build Cargo budget.

For full verification, install `cargo-audit` and run:

```powershell
cargo install cargo-audit --locked
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/verify-workspace.ps1
```

See [development](docs/DEVELOPMENT.md) for toolchain details and focused checks,
[architecture](docs/ARCHITECTURE.md), and [the design contract](DESIGN.md).
Live provider and desktop tests are opt-in; routine CI does not use your account.

## Accounts, data and permissions

Connect your provider in **Settings → AI accounts**. Install the provider's
supported runtime separately. Do not import or share another app's credential
store. Supervisor retains some internal `CentralAgent` names for profile and
protocol compatibility.

Prompts and selected context are sent to the configured provider. Local files,
chat state and settings remain outside the source repository. Supervisor's host
tools can act with your Windows/SSH account permissions; approvals are not an
OS sandbox. Codex's native tools retain their own sandbox/approval controls.
Closing the window may leave agents running in the tray; use Stop or tray Exit
to interrupt work. Read [privacy](docs/PRIVACY.md) and [security](SECURITY.md).

## Contribute

Read [CONTRIBUTING.md](CONTRIBUTING.md) and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
Use the issue templates for bugs and proposals; report vulnerabilities privately.
[CHANGELOG.md](CHANGELOG.md) describes the initial experimental release.
The [publication guide](docs/PUBLISHING.md) separates source releases from the
existing signed-binary release gate.

## License and independence

Original Supervisor code is **MPL-2.0**. See [LICENSE](LICENSE),
[LICENSES.md](LICENSES.md), [NOTICE](NOTICE) and
[third-party notices](THIRD_PARTY_NOTICES.md).

Supervisor is an independent project, not an official OpenAI product or an
OpenAI-endorsed application. Provider names and logos identify integrations;
their trademarks, services and separately installed components retain their
owners' terms.
