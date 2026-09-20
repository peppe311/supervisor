# Central Agent Local OS Runtime Plan

Status update, 2026-09-08: the local OS runtime and its manual surfaces remain,
but the Codex dynamic-tool bridge described in the historical target below is
**not part of the current Codex integration**. The active scope uses the official
native App Server and defers Central Agent's custom browser, terminal, SSH,
desktop-control and Time Machine bridges. Do not restore the retired
custom tool loop from this plan; see `CODEX_APP_SERVER_IMPLEMENTATION_GUIDE.md`.

## Outcome

Central Agent provides local coding and computer-use surfaces without routing
operating-system access through a VPS. The installed Rust application remains
the trusted host for Central Agent-owned actions. Connecting those actions to
native Codex is a deferred product and security decision, not current behavior.

The interactive terminal remains available for direct user input. Agent-issued
commands are a separate path and always pass through the local policy engine.

## Current baseline

The application already provides:

- browser tabs and typed browser actions;
- multiple visible interactive PowerShell PTYs;
- agent-issued terminal commands with live output and cancellation;
- approve-once, approve-each, and full-access modes;
- a project and agent graph without a separate knowledge store;
- native Codex App Server conversations with server-owned tools and approvals;
- local application-state storage.

The present terminal tool runs with the current Windows user's authority. A
working directory is context, not a security sandbox: an arbitrary child
process can still access any resource available to that Windows account.

## Deferred historical target architecture

The diagram below records the earlier custom-tool proposal. It is retained for
security analysis only and is not an implementation instruction for the current
native App Server client.

```text
Codex App Server
       |
       | typed dynamic tool call
       v
Central Agent trusted host
       |-- schema validation
       |-- capability and risk classification
       |-- approval policy
       |-- result filtering and audit trail
       v
Local OS Runtime (Rust, local only)
       |-- workspace and filesystem adapter
       |-- process and PTY adapter
       |-- Windows window adapter
       |-- Windows UI Automation adapter
       `-- preview and capture adapter
```

If a future phase reintroduces a Central Agent OS bridge, it must use explicit
startup scopes and a separately reviewed authority model. It must not disable or
impersonate native App Server features, open an implicit network listener, or
silently register itself in other applications.

## Authorization model

Every agent action carries two independent labels:

1. a capability scope: browser, terminal, workspace, process, window,
   UI automation, or capture;
2. an effect class: read-only, reversible write, process execution, external
   interaction, destructive, or privileged.

The modes mean:

- **Approve each action**: every side-effecting action waits for approval.
- **Authorize once**: the user grants the connected capability scopes for the
  current app session. A newly selected workspace revokes the old workspace
  grant.
- **Full access**: actions run autonomously inside the explicitly connected
  scopes. Destructive and privileged operations remain separately gated.

Full access never implies UAC elevation, credential extraction, persistence,
security-control changes, or unrestricted administrator access.

## Milestone 0 — Capability-aware policy

- Replace the global yes/no decision with an action descriptor.
- Add scope and effect metadata to every command family.
- Require explicit approval for destructive actions in every mode.
- Hard-deny privileged actions until an isolated elevation design exists.
- Show scope and effect on approval cards and in the local log.
- Add policy tests for every mode and effect class.

## Milestone 1 — Workspace Session V1

The user explicitly selects one local folder. Agent paths are always relative
to that canonical root.

Initial tools:

| Tool action | Effect | Limits |
| --- | --- | --- |
| `get_state` | Read-only | Returns capability state, not secrets |
| `list_files` | Read-only | Bounded entries and recursion depth |
| `read_file` | Read-only | UTF-8 text, bounded bytes and lines |
| `search_text` | Read-only | Bounded files, bytes, and matches |
| `apply_patch` | Reversible write | Optimistic hash check and exact replacements |
| `run_command` | Process execution | Visible PTY, workspace as starting directory |

Path rules:

- reject absolute paths, parent traversal, NULs, alternate data streams, and
  any canonical target outside the workspace;
- never follow directory symlinks or Windows reparse points during traversal;
- require an existing canonical parent for new files;
- reject binary, oversized, and non-UTF-8 content;
- return bounded and filtered results to Codex;
- use optimistic content hashes to prevent overwriting a file that changed
  after the agent read it.

UI requirements:

- a dedicated Workspace settings section;
- native folder selection and an explicit Disconnect action;
- visible root path, connection state, and capability summary;
- new terminals start in the selected workspace;
- the model receives only relative paths and a bounded workspace name.

## Milestone 2 — Managed processes

- Separate non-interactive task processes from human PTY sessions.
- Track only processes launched by Central Agent.
- Stream and retain stdout and stderr up to a 64 MiB combined per-process cap.
- Support status, stdin, explicit cancellation, and process-tree termination.
- Use Windows Job Objects for lifetime and cleanup, while documenting that Job
  Objects do not provide filesystem or network isolation.
- Stop managed commands after 30 minutes or 64 MiB of combined output and reject
  stdin after an 8 MiB lifetime cap. CPU, memory, action-count, and process-count
  controls remain future isolation-backend work; the user retains explicit Stop
  and application-exit cleanup.
- Show running processes and their owning agent run in the UI.

## Milestone 3 — Window awareness

- List top-level windows with process identity and visibility state.
- Focus, move, resize, minimize, restore, and close a selected window.
- Prefer windows owned by processes started by Central Agent.
- Require a fresh window identifier; never let the model invent raw handles.
- Capture one selected window using Windows Graphics Capture.
- Redact or block protected, credential, private-browsing, and secure-desktop
  surfaces.

## Milestone 4 — Semantic computer use

- Inspect a bounded Windows UI Automation tree.
- Return stable, short-lived element references.
- Implement focus, invoke, set-value, select, expand, scroll, and text input.
- Use raw mouse coordinates and keystrokes only as an explicit fallback.
- Display a persistent control indicator and current target application.
- Add a global emergency-stop shortcut and stop on session lock or secure
  desktop transitions.

## Milestone 5 — Application previews

- Detect local HTTP preview servers started by the agent.
- Associate native GUI/game windows with managed child processes.
- Embed, dock, or detach a bounded live preview surface.
- Keep browser, native-window, and game previews as distinct source types.
- Stop capture when the user hides the preview or the owning process exits.

## Milestone 6 — Isolation and release hardening

- Decide whether untrusted commands run with a restricted token, AppContainer,
  Windows Sandbox, WSL, or another explicit isolation backend.
- Sign the application and runtime executables.
- Add tamper-evident local audit metadata without storing secrets or complete
  terminal transcripts by default.
- Add crash recovery, process cleanup, retention controls, and safe update
  behavior.
- Perform threat modelling and adversarial prompt-injection tests before any
  public release of autonomous computer use.

Implementation status:

- the current backend decision is documented as trusted-host/development only;
  restricted-token execution is the next isolation spike, while AppContainer,
  Windows Sandbox, and WSL remain explicit alternative backends;
- the application manifest requests `asInvoker`, disables UIAccess, and all
  Central Agent-owned terminal/task trees use kill-on-close Job Objects;
- every session writes a metadata-only SHA-256 hash-chained audit journal;
  startup reports modified, truncated, or unclean sessions and Settings offers
  7-, 30-, and 90-day retention;
- release scripts build, hash, Authenticode-sign, timestamp, and verify both
  executables; an unsigned artifact requires the explicit
  `-AllowUnsignedDevelopment` switch;
- the public autonomous-computer-use gate remains **no-go** until restricted
  token (or stronger) isolation, a production signing identity, clean-VM tests,
  and the adversarial test matrix in `SECURITY_RELEASE_GATE.md` are complete.

## Security and privacy invariants

- No hidden background control and no automatic privilege elevation.
- No cookie, browser-session, credential, or secret extraction tools.
- Page content, files, process output, UI text, and agent context are untrusted data,
  never authorization.
- The capability token and canonical root stay inside the trusted Rust host.
- Structured browser, workspace, and UI contexts remain typed and
  purpose-specific. Terminal, SSH, and managed-process command results are sent
  completely after local secret redaction, without an app-imposed character
  tail.
- Manual shell input is never reclassified as an agent action.
- Deletes use a recoverable mechanism where possible and are not part of
  Workspace V1.

## Go / no-go gates

Workspace write/process tools are a go only when:

- absolute-path, traversal, symlink/reparse, race, and stale-hash tests pass;
- an approval cannot be bypassed by changing tool arguments after display;
- cancellation ends the owned command and the agent receives a final result;
- every mutation is visible in the action log;
- restarting the app clears session authorization;
- no operation requests elevation.

Computer-use tools are available only as an off-by-default local-development
capability. Public autonomous computer use is a no-go until strong process
isolation, signed distribution, clean-VM validation, and the documented
adversarial test matrix all pass. The global emergency stop must remain
independent of the model, target application, and UI Automation thread.

## Delivery sequence

- [x] Capability-aware permission engine
- [x] Workspace selection and persisted root
- [x] Bounded read-only workspace tools
- [x] Hash-checked text patch tool
- [x] Workspace-scoped visible command tool
- [x] Workspace settings and approval UI
- [x] Managed process runtime
- [x] Window awareness
- [x] Semantic UI Automation
- [x] Preview manager
- [x] Non-elevated manifest and crash-safe owned-process cleanup
- [x] Tamper-evident metadata audit and retention controls
- [x] Signed-build verification pipeline and explicit public no-go gate
- [ ] Restricted-token or stronger untrusted-command isolation
- [ ] Production certificate, clean-VM, updater, and adversarial release proof
