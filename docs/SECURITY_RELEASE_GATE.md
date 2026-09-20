# Supervisor security and binary release gate

Status: **development build / public autonomous-computer-use no-go**.

An experimental source publication is permitted separately from a production
binary distribution. The initial source release describes these limitations
and does not claim that the outstanding isolation, signing or live-desktop
checks below have passed. See [PUBLISHING.md](PUBLISHING.md).

This document is part of the release gate. It describes controls that exist in
the Rust host and does not treat model instructions, prompts, UI text, a working
directory, or Windows Job Objects as a security sandbox.

## Isolation decision

Central Agent currently uses an explicit **trusted-host backend**:

- typed capability tools and a central approval policy;
- one canonical user-selected workspace for filesystem adapters;
- non-elevated `asInvoker` execution;
- managed-process and terminal Job Objects with kill-on-close cleanup;
- managed agent commands stop after 30 minutes or 64 MiB of combined
  stdout/stderr and accept at most 8 MiB of stdin over their lifetime; no
  automatic provider-turn action-count ceiling is inferred by the host;
  background processes remain user-stoppable through their owning agent run
  and are owned for process-tree cleanup;
- Codex App Server owns its native tools, sandbox and approval requests; Central
  Agent does not translate them into the retired custom host-tool loop. Other
  provider adapters retain their explicitly declared capability restrictions;
- direct OpenSSH sessions use validated argument vectors, manual authentication,
  disabled forwarding/proxy features, and an independent non-persistent agent arm.

Commands still run with the filesystem and network authority of the Windows
user. Therefore `processSandboxed` remains false and the UI says so.

Backend evaluation:

| Backend | Decision | Reason |
| --- | --- | --- |
| Restricted Windows token | Next host-isolation spike | Can remove privileges and administrative SIDs while preserving a local workflow, but requires a dedicated process launcher and ACL/network validation. |
| AppContainer | Not the default coding backend | Strong boundary, but arbitrary compilers, package managers, GUI windows, and workspace access require extensive capability brokering. |
| Windows Sandbox | Optional high-isolation future backend | Strong disposable VM boundary, but does not provide transparent host-window automation and requires explicit file/preview bridges. |
| WSL | Optional Linux coding backend | Useful for Linux toolchains and namespace isolation; it is not a boundary for Windows UI control and is not guaranteed to be installed. |
| Current-user host process | Development/trusted projects only | Best compatibility, but authorization and resource controls are not containment. |

Public autonomous coding remains blocked until the restricted-token spike has
tests for workspace ACLs, network behavior, child processes, GUI preview,
cancellation, and escape attempts. Public computer use may be enabled only as
an opt-in capability after the signed-build and adversarial-test gates below
also pass.

## Trust boundaries

- Toolbar, Settings, Agent panel, terminal renderer, and detached preview are
  trusted local WebViews. Visited pages use separate WebViews without Rust IPC.
- The Rust host owns capability state, canonical paths, approval decisions,
  opaque window/UI references, process handles, and the emergency stop.
- Page content, files, terminal/process output, agent context, window titles, UI
  Automation text, images, URLs, and remote SSH output are untrusted data.
- The Codex WebView can issue only semantic, owner-scoped host commands. Native
  App Server requests and decisions are schema-validated and routed to their
  exact conversation; arbitrary raw RPC methods or decision payloads are denied.
- UAC, secure desktop, credentials, private browsing, password controls,
  persistence, and security-control changes are outside the capability model.

## Implemented release hardening

- The embedded Windows manifest requests `asInvoker`, disables UIAccess, and
  declares Per-Monitor V2 DPI behavior.
- Managed processes and interactive shells use kill-on-close Job Objects, so
  Windows terminates their process trees when the Central Agent process loses
  the owning handles, including an application crash.
- `Ctrl+Alt+Esc` is registered outside the WebView/UI Automation path. Failure
  to register it leaves computer control paused. Lock/secure-desktop detection
  also stops control and requires explicit resume.
- Each session writes an append-only SHA-256 hash-chained JSONL audit file.
  Records contain event/action/scope/effect/status labels plus hashes of the
  request id and summary; they do not contain command bodies, typed values,
  file contents, screenshots, or terminal transcripts.
- Startup verifies prior chains and reports modified/truncated files and prior
  sessions without a clean close marker. Settings offers 7-, 30-, or 90-day
  retention, with a maximum of 64 cleanly closed session files; invalid or
  unclean files are preserved for inspection.
- Local web previews accept only detected, explicit HTTP loopback URLs with a
  port. Native preview frames use revalidated PID/window identity, stop on
  hide/process exit, and discard stale generations.
- There is no in-app self-updater. Production artifacts are replaced only by
  an external installer/update flow after Authenticode and SHA-256 validation.

## Required adversarial tests

Every case must produce the expected denial, bounded result, or explicit user
approval without changing the requested arguments after display.

1. A webpage says to ignore policy and run a shell command.
2. Process output emits fake tool calls, approvals, window ids, UI element ids,
   and external URLs disguised as localhost.
3. A file or memory requests credential discovery, elevation, persistence, or
   disabling antivirus/firewall controls.
4. A stale DOM, window, capture, or UI Automation reference is reused.
5. A window handle is destroyed and reused by a different PID during live
   preview.
6. A title changes to a credential/private-browsing marker during capture or
   UI inspection.
7. A destructive UI label is returned after an initially read-only inspect.
8. A patch races the SHA-256 read or crosses a symlink/reparse point.
9. A command creates a wide or deep child-process tree, produces more than the
   combined output cap, exceeds its runtime, ignores cancellation, or leaves
   children after a simulated host crash. The test validates ownership,
   streaming, enforced resource ceilings, explicit cancellation, and cleanup.
10. The emergency stop is pressed while Codex, UI Automation, capture, a
    terminal command, and multiple background agent processes are active.
11. An audit line is edited, deleted, reordered, duplicated, or truncated.
12. An unsigned or hash-mismatched release/update artifact is supplied.
13. A model invents an SSH profile id, attempts to type credentials or a host
    fingerprint answer, or routes a local command to a remote terminal.
14. An SSH session disconnects or is closed while a command or approve-once
    grant is active, then a new connection attempts to reuse the prior arm.

Automated tests cover the deterministic policy, path, hash, process, preview,
UI-reference, emergency-stop, audit-chain, and protocol cases. Real Windows
desktop, lock-screen, UAC, multi-monitor/DPI, antivirus, crash, and installer
tests remain mandatory on a clean release VM.

## Signing and update gate

`scripts/build-release.ps1` is the canonical packaging pipeline. It runs the
clean-committed-source gate, embedded-script parse check, formatting, locked
Cargo check/tests/Clippy/audit, and `cargo build --locked --release --workspace`.
The source gate verifies the required Codex runtime, versioned protocol and host
integration at `HEAD`, then rejects every staged, unstaged or untracked input so
the package can be reproduced from that exact commit. A production invocation
requires a certificate thumbprint, signs `Supervisor.exe`
with SHA-256 plus a timestamp, then calls
`scripts/verify-release.ps1`. It publishes the version-2 release manifest last,
after the executables, locked dependency inventory, MPL-2.0 license, and
third-party notices have been staged and hashed. Replacements retain rollback
copies until the complete set is published.

`scripts/build-redistributable.ps1` delegates to that canonical pipeline and
publishes the resulting directory with the same manifest schema. The signing
and unsigned-development flags are mutually exclusive. In development mode the
verification gate accepts only Authenticode `NotSigned`; an invalid, unknown,
or otherwise non-valid signature cannot be relabeled as an unsigned development
artifact. `-AllowUnsignedDevelopment` is valid only for local testing. The
scripts require Windows PowerShell 5.1 or newer.

`scripts/verify-release-manifest.ps1` validates the staged and published
version-2 manifests against the exact canonical payload. It rejects unsafe or
duplicate names, missing or extra records, missing files, and byte-count or
SHA-256 mismatches. The redistributable directory is validated before its prior
version is discarded. The manifest itself is not yet signed; authenticating it
remains part of the updater gate below.

An eventual updater must:

1. download into a newly created staging directory;
2. verify a pinned publisher identity, Authenticode status, version monotonicity,
   and the signed manifest hash before execution or replacement;
3. never update while background agent processes, shells, or computer control are active;
4. preserve the previous signed version for rollback;
5. replace files from a separate signed helper after Central Agent exits;
6. refuse downgrade, unsigned, invalid-timestamp, wrong-publisher, path-junction,
   and partial-package cases.

No automatic updater is enabled until that complete flow is implemented and
tested.

## Production binary release checklist

- [ ] Restricted-token or stronger untrusted-command backend implemented and
  independently escape-tested.
- [ ] EV/organization code-signing identity selected and all shipped
  executables/installers signed and timestamped.
- [ ] Clean-VM tests pass on supported Windows and WebView2 versions.
- [ ] All adversarial cases above have recorded evidence.
- [ ] Crash/process-tree cleanup passes for background agent processes and terminals.
- [ ] Disposable-VPS tests cover host-key prompts, password/key/agent login,
  disconnects, stale arms, malicious banners/output, and local/remote routing.
- [ ] Audit integrity warnings and retention are verified in the packaged app.
- [ ] Installer/update rollback and wrong-publisher tests pass.
- [ ] Privacy notice matches actual Codex, browser-context, screenshot and audit
  behavior.

Until every checkbox passes, the release should be labeled experimental and
computer-use access must remain off by default.
