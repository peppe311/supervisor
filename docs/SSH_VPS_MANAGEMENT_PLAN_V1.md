# SSH VPS Management Plan V1

Status update, 2026-09-08: manual SSH management is implemented for local
validation; real VPS connectivity and usability remain user-tested. The former
Codex `ssh_action` bridge and **Enable Codex** controls described below are
historical and unavailable in the current native App Server phase. They must not
be restored without a new explicit scope and security review. See
`CODEX_APP_SERVER.md`.

Status update, 2026-09-14: remote project mapping was removed together with the
retired local knowledge store. SSH profiles, visible terminals, remote desktop
and explicit live-session agent access remain.

## Objective

Add direct VPS administration to Central Agent without introducing a hosted
bridge or storing credentials. The installed desktop application starts the
operating system's OpenSSH client inside an existing visible PTY, so network
traffic flows directly from the user's computer to the selected VPS.

```text
Trusted Settings UI
        │ profile metadata
        ▼
Central Agent Rust host
        │ validated argv, no shell interpolation
        ▼
Local OpenSSH client ───────────────► VPS sshd
        │
        └── visible interactive PTY (host-key check, login, output)
```

## V1 scope

- Detect the local OpenSSH client, with `CENTRAL_AGENT_SSH_BIN` as an explicit
  override.
- Persist up to 24 named profiles: host, port, username, optional absolute
  private-key path, and whether the profile may ever be used by Codex.
- Open multiple interactive SSH sessions in normal Central Agent terminal
  tabs; they can be reordered, docked, detached, and attached to chat.
- Keep host-key verification, passwords, key passphrases, and other
  authentication interaction manual and visible.
- Expose a separate `ssh_action` to Codex only for a profile with both its
  persistent opt-in and a process-local, user-armed authenticated session.
- Apply the existing permission mode and audit journal to every agent-issued
  remote command under a distinct `ssh` capability scope.
- Return bounded, locally filtered output and exit status to the active Codex
  turn.

## Explicit non-goals

- No passwords, passphrases, private-key contents, or session tokens stored by
  Central Agent.
- No automatic host-key acceptance and no manipulation of `known_hosts`.
- No embedded SSH implementation, hosted relay, reverse tunnel, or Central
  Agent account service.
- No SFTP/file browser, deployment recipes, port forwarding, SSH-agent
  forwarding, proxy commands, jump hosts, or local SSH commands in V1.
- No automatic privilege elevation, `sudo` authentication, or credential UI
  automation.
- No file-content import, file-name inventory, package inventory, process
  command lines, environment-variable capture, privilege escalation,
  vulnerability scanning, unbounded whole-filesystem crawl, or continuous
  background discovery. Hidden directories, dependency/build directories, and
  other filesystems are skipped.
- No claim that a remote shell is sandboxed. It has the authority of the
  configured VPS account.

## Security invariants

1. Profile fields are normalized by Rust and never concatenated into a shell
   command. OpenSSH receives a program path and a validated argument vector.
2. Hosts that resemble options, URLs, paths, or whitespace-separated command
   fragments are rejected.
3. OpenSSH starts with strict host-key prompting and forwarding, proxy, and
   local-command features disabled. User SSH config is ignored so a saved
   profile cannot inherit executable or proxy behavior.
4. Codex cannot create profiles, open connections, answer authentication
   prompts, or arm sessions. Those messages exist only on the trusted local UI
   bridge.
5. A profile opt-in is insufficient by itself. After manual login, the user
   must press **Enable Codex** on that exact live terminal session. The arm is
   never persisted across app restarts or reconnects.
6. The provider receives only the IDs and names of profiles that are eligible
   and armed at the start of a run. `ssh_action` rejects any invented or stale
   profile ID again in Rust.
7. Local `terminal_action` selects only a local terminal. `ssh_action` selects
   only a matching armed SSH terminal, preventing cross-routing between local
   PowerShell and remote shells.
8. Remote output and attached SSH snapshots are untrusted data and pass through
   the same bounded redaction path as local terminal content.

## Implemented components

### Profile and client runtime

`src/ssh_runtime.rs` owns client detection, profile normalization,
persistence-facing models, secure OpenSSH arguments, and launch specifications.
An unavailable key file does not erase a saved profile at startup; it is
revalidated before connection.

### Interactive transport

`src/terminal.rs` now distinguishes local and SSH sessions. Both use the same
PTY renderer and process cleanup. Local Windows commands use the existing
PowerShell completion protocol; SSH V1 uses a POSIX completion marker that
returns the remote exit code and base64-encoded working directory.

### Authorization and Codex tools

`src/commands.rs` defines the trusted profile/session controls and the typed
`SshCommand`. `src/permissions.rs` defines the independent `ssh` scope.
`src/codex_provider.rs` publishes `ssh_action` only when at least one eligible
live session was armed, constrains `profileId` to an exact enum, and maps the
trusted profile name into the local command object.

### User interface

Settings → Servers reports OpenSSH availability, edits local profiles, and
opens visible connections. SSH terminal tabs are labelled, display the remote
target, and expose the process-local **Enable Codex** control. No credential
field exists. Remote terminal snapshots retain their SSH identity when dragged
into chat.

## Milestones and result

1. **Profile model and OpenSSH detection — complete.**
2. **SSH PTY sessions and local/remote routing — complete.**
3. **Servers settings and terminal session arm — complete.**
4. **Typed `ssh_action`, independent scope, approvals, and audit — complete.**
5. **Automated non-visual verification and documentation — complete.**
6. **Manual real-VPS acceptance test — pending user validation.**

## MVP acceptance criteria

- A valid profile survives restart without storing any credential material.
- Connect opens a visible SSH terminal and leaves the host fingerprint and
  authentication decision to the user.
- Multiple local and remote terminals can coexist without commands crossing
  session types.
- Codex cannot see `ssh_action` until a profile is opted in and a live session
  is manually armed.
- Every Codex remote command is separately represented as `ssh_run`, uses the
  `ssh` scope, follows the selected permission mode, appears live in the PTY,
  and returns complete redacted output without an automatic timeout.
- Closing/disarming the last eligible SSH session revokes approve-once access
  to the SSH scope.
- Invalid hosts, invented profile IDs, missing key paths at launch, and unknown
  fields are rejected locally.
- A successful explicit map creates one selected server root, a bounded remote
  directory hierarchy, and typed project nodes; a later map archives paths that
  disappeared and does not expose its raw scan to Codex.

## Go / no-go

Go for local manual use when OpenSSH is detected, the target fingerprint can be
verified independently, and the account follows least privilege.

No-go for public autonomous VPS administration until code signing, a completed
threat model, destructive remote-command classification, credential-handling
review, real-server disconnect/reconnect tests, and an explicit recovery story
are complete. Full Access is not a substitute for those release gates.

## Immediate next roadmap

1. Manually test password, key-file, and `ssh-agent` authentication against a
   disposable VPS, including first-connect fingerprints and disconnects.
2. Add native key-file selection so users do not type an absolute path.
3. Add reconnect state that always requires a fresh manual session arm.
4. Add a remote command risk classifier before considering production Full
   Access.
5. Add trusted per-profile scan-root selection for projects stored outside the
   current home and standard deployment paths.
6. Evaluate bounded SFTP separately; do not overload shell commands with file
   transfer behavior.
