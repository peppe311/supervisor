# Architecture

Supervisor is a Windows desktop application with a Rust host and trusted local
Svelte/TypeScript interfaces rendered by WebView2.

| Area | Responsibility |
| --- | --- |
| `src/` | Native application, windows, project state, process ownership, browser, terminal, SSH, permissions and persistence. |
| `src/browser/` | WebView ownership and typed UI/host routing; external pages must not receive trusted IPC. |
| `crates/central-agent-codex-runtime/` | Official App Server transport, version checks, requests, events and cancellation. |
| `ui/src/components/` | Modular UI; Rust remains authoritative for capabilities and operations. |
| `assets/` | Shared visual tokens, remaining legacy surfaces, artwork and licensed fonts. |
| `protocol/app-server/` | Versioned generated upstream JSON and TypeScript contracts. |
| `vendor/` | Two small patched upstream crates, retaining upstream licenses. |
| `scripts/` | Builds, deterministic checks, dependency notices and release tooling. |

A prompt is owned by a conversation. The host validates the provider and context,
sends typed requests to that provider's runtime, and routes streaming events back
to the same conversation. Codex native operations stay in App Server; other
providers expose only the capabilities their adapter implements. Do not assume
all providers share Codex's wire protocol or feature set.

Supervisor conversations observe linked project conversations and can steer
them. Observation, delegation, forks and independent conversations retain their
separate ownership. Tests exercise ordering, late events, retries and recovery.
The current project surface is described in [PROJECT_BOARD.md](PROJECT_BOARD.md).

Local state lives outside the repository. The app retains the internal
`CentralAgent` directory and protocol names for compatibility. See
[privacy](PRIVACY.md) and [Codex profile isolation](CODEX_PROFILE_ISOLATION.md).
Account stores and production chats are never test fixtures.

The trusted host is not an OS sandbox. Native providers and plugins keep their
own trust boundaries. See [SECURITY.md](../SECURITY.md) before adding a tool.
