# Codex App Server integration guide

Protocol version checked: **2026-09-20**. Implementation guidance updated:
**2026-09-20**. This guide is specific to Central Agent. It
combines the official App Server semantics with the versioned generated contract
and the behavior already implemented in this repository.

## Current version decision

Codex App Server is shipped as part of Codex CLI; it does not have an independent
release number. Supervisor's selected runtime is the official desktop
**Codex CLI 0.155.1** installed on the development machine:

- `crates/central-agent-codex-runtime/src/runtime.rs` uses `0.155.1` as the
  tested baseline and fails closed on versions outside the exact manifest;
- `codex --version` returned `codex-cli 0.155.1`;
- stable generation from that executable produced 312 JSON schemas and 721
  TypeScript files in `protocol/app-server/0.155.1`;
- those generated files are byte-for-byte identical to the previously verified
  `0.155.0-alpha.2.6` stable contract, with no request, notification or server
  callback added or removed.

The previous verified runtimes remain in the exact-version allowlist for safe
rollback. An unknown future build is still rejected until its generated contract
and native startup behavior have been checked.

## P3-A backend integration

The P3-A backend follows the versioned semantics and native probes documented in
[P3-A](CODEX_APP_SERVER_P3A.md). Revert is an exclusive-cutoff conversation-history
change, requires a durable pre-send receipt, and can release the loaded session
before its ACK. Keep explicit resume distinct from history reconciliation.
Sections are server-owned metadata, not a local replacement store. Item listing
stays disabled at `Client::request` until a separate product phase enables it.

## Runtime discovery on Windows

Use one deterministic precedence order:

1. `CENTRAL_AGENT_CODEX_BIN`, when present, is an explicit absolute override and
   therefore authoritative. A missing, non-executable or wrong-version override
   must fail; it must not silently fall through.
2. Prefer `resources/codex/codex.exe` beside the Central Agent executable when a
   future package intentionally ships the tested runtime.
3. Probe every absolute `codex.exe` candidate found through PATH rather than
   stopping at the first wrong-version installation.
4. On Windows, inspect only the bounded official desktop layout
   `%LOCALAPPDATA%\OpenAI\Codex\bin\<build>\codex.exe`, newest build directories
   first. Canonical containment prevents a link from escaping that root.

Every automatic candidate must identify as one of the exact versions in
`protocol/app-server/supported-versions.json`; skip an incompatible automatic
candidate and continue. `0.155.1` is the current tested baseline. Do not recursively scan AppData,
download a runtime, inspect `.codex`, read credential stores or infer the binary
from a running process. The desktop fallback is executable discovery only. A
no-inference smoke run should remove `CENTRAL_AGENT_CODEX_BIN`, use a disposable
`CODEX_HOME`, and verify initialize/account/requirements/model-list without
printing account details.

Normalize Windows `\\?\` and `\\?\UNC\` prefixes only in bounded display
projections. Keep the canonical/raw path for Rust authority checks and App Server
requests.

## Documentation and contract sources

Use these sources in this order:

1. The [official rolling App Server documentation](https://learn.chatgpt.com/docs/app-server)
   defines public behavior and maturity labels.
2. Optional local reference snapshots can be downloaded with
   `scripts/sync-codex-app-server-docs.ps1` into ignored `target/reference-docs/`.
   Full official website snapshots are not redistributed in the public source.
3. `protocol/app-server/<accepted-version>` defines the exact wire shapes for
   each allowlisted executable; `0.155.1` is current. Generated schemas are
   version-specific and must never be edited manually.
4. `docs/CODEX_APP_SERVER_ACCEPTANCE.md` records behavior actually observed in
   Central Agent and distinguishes deterministic fixtures from live account or
   operating-system acceptance.
5. `docs/CODEX_APP_SERVER_GAP_ANALYSIS.md` is the maintained method inventory and
   delivery priority. It is not allowed to override the generated contract or
   promote an experimental method to production.

The official page is rolling documentation. A method appearing in a generated
schema but not in the public page is not enough evidence to expose it in the
product. Conversely, a method documented as experimental must remain behind an
explicit, independently reviewed capability gate even if a generated type is
present.

Refresh the local documentation snapshot with:

```powershell
./scripts/sync-codex-app-server-docs.ps1
```

## Recommended architecture for Central Agent

```text
Svelte UI
  -> semantic, owner-scoped IPC commands
  -> Rust App Server host/controller
  -> typed request constructors + versioned schema checks
  -> one supervised Codex App Server process over stdio JSONL

Codex notifications/server requests
  -> validated transport correlation
  -> native thread/turn/item mirror
  -> owner-scoped presentation and approval cards
```

This keeps authority in the correct places: App Server owns agent execution,
native history and approval state; Rust owns process supervision, local UI
bindings and safe IPC; Svelte owns presentation and user intent. The frontend
must never send raw RPC method names, request IDs, paths, policies or arbitrary
approval payloads.

### 1. Keep stdio as the production transport

For the current Windows-first, local-first desktop product, keep one supervised
local process using `--listen stdio://` and newline-delimited JSON. It has the
smallest attack surface and matches the existing process lifecycle.

Treat each newline-delimited message as an untrusted frame, not an unbounded
line. Central Agent accepts at most 64 MiB for one App Server JSONL frame,
rejects the next byte before the buffer can grow further, validates UTF-8, and
fails the connection if either check fails. The legacy
provider bridges use an 8 MiB frame ceiling and retain only the final 64 KiB of
stderr diagnostics. Managed host commands have separate 30-minute, 64 MiB
combined stdout/stderr, and 8 MiB lifetime-stdin ceilings.

Do not replace it with WebSocket or remote Code Mode solely for feature parity.
The official documentation labels the WebSocket path and remote Code Mode host
experimental and unsupported for production. If remote App Server becomes a
product requirement, add it as a separate transport with TLS, mandatory
authentication, bounded-queue retry with exponential backoff and explicit host
trust; do not silently fall back from local stdio.

### 2. Negotiate capabilities narrowly

Send exactly one `initialize`, wait for its response, send `initialized`, and
only then expose the connection as ready. Keep `experimentalApi: false` in the
production client.

Add capabilities only when the corresponding host implementation exists. The
current production initialization keeps `experimentalApi` and attestation off,
and declares `openai/form` through `capabilities.extensions` plus the compatible
legacy boolean because both request rendering and result validation now exist:

- `mcpServerOpenaiFormElicitation` or its extension declaration requires a
  schema-bounded extended-form renderer and response validator. Central Agent
  accepts bounded object schemas, uses typed controls for flat primitives and a
  sanitized JSON editor for nested arrays/objects, and rejects unsupported or
  invalid schema/results without rendering HTML;
- `requestAttestation` requires a real host attestation provider and must not
  return a fabricated token;
- `optOutNotificationMethods` should be used only to reduce traffic for events
  the product intentionally never consumes, not to hide unimplemented behavior.

Keep `clientInfo.name = "central_agent"` and `serviceName = "central_agent"`
stable. Before enterprise distribution, contact OpenAI to register the client
name for compliance-log attribution as requested by the official guide.

### 3. Treat generated schemas as the wire boundary

Continue using small semantic constructors in
`central-agent-codex-runtime::api`, and validate every serialized request and
server-response decision against the generated stable contract. Do not expose a
generic `call(method, params)` path to WebView IPC.

On every CLI update:

1. create a new `protocol/app-server/<version>` directory;
2. generate stable TypeScript and JSON schemas from the exact executable;
3. generate an experimental schema into a temporary directory only for change
   discovery;
4. diff methods, fields, enum values, notifications and server requests;
5. update typed adapters and fixtures before changing `TESTED_VERSION`;
6. run contract, Rust, frontend, hidden-WebView and manual acceptance gates;
7. retain the previous directory until migration and rollback decisions are
   complete.

Never regenerate in place and never accept a version range without compatibility
tests. A hotfix may change behavior even when its protocol schema is unchanged.

### 4. Model native identity, not reconstructed chat

Persist only Central Agent owner-to-native identifiers and mutation receipts.
Use native `threadId`, `sessionId`, `turnId` and `itemId` as identities. Resume or
read native history; do not rebuild it from rendered messages, inject a second
conversation store, or infer delivery from UI state.

Linking an existing conversation must persist metadata only; it must not fetch a
possibly unbounded transcript as part of the link acknowledgement. For resume,
read and fork, request `excludeTurns: true` and hydrate display state through
ascending `thread/turns/list` pages. In this client each request asks for 16
turns, each accepted page is bounded to 64 turns, and one hydration is capped at
4,096 turns/512 pages. Reject repeated cursors and repeated turn IDs, publish
only after the terminal page, and merge against newer live events by native
identity. A failure clears the exact pending owner and never publishes a partial
history. The legacy inline-turn response is compatibility fallback only.

Project the stable top-level model/provider/service/cwd/approval/sandbox fields
from successful thread start, resume and fork responses into the initial
read-only reported-settings view. A later `thread/settings/updated` event may
replace or extend that allowlisted view, but the absence of that optional event
must not leave fields already returned by the successful response unavailable.
Do not use `config/read`, local composer choices or raw permission/config objects
to fill missing values.

History lists are navigation, not transcript viewers. Redact and collapse any
fallback preview, cap it independently of the native response, disclose
truncation and keep opaque thread IDs out of presentation. Preserve the exact ID
only in Rust-owned selection and mutation authority.

For mutations, preserve the current acknowledgement discipline:

- clear a draft only after the matching native acknowledgement;
- never replay an uncertain `turn/start`, `turn/steer`, fork, delete or approval;
- after transport loss, read native state and ask for explicit reconciliation;
- keep pending server requests connection-scoped and discard them on disconnect.

The P0 implementation uses `thread/loaded/list` on connect and explicit refresh,
publishes all pages atomically, and reconciles only IDs already bound to a local
owner. It uses `thread/unsubscribe` after a local surface is intentionally
unlinked and only when that connection had loaded the thread. Local unlink is
preflighted before draft clearing and persisted before dispatch; unsubscribe
never substitutes for native deletion and does not remove Codex history.

### 5. Make completed objects authoritative

Build incremental UI from `item/*` and `turn/*` deltas, but replace provisional
state with the final object from `item/completed` and `turn/completed`. Preserve
arrival ordering and reject late deltas for completed items.

Continue these rules:

- `turn/diff/updated` is the aggregate diff, while item events remain the source
  of truth for item identity;
- reasoning UI may display public summaries, never hidden reasoning content;
- `contextCompaction` is current; `thread/compacted` and
  `item/fileChange/outputDelta` are deprecated compatibility signals;
- unknown events stay diagnosable without recording payloads, but documented
  warnings and security notices use a bounded typed user-facing projection rather
  than only a method-name counter. Keep automatic-review action bodies/rationales,
  terminal stdin/process IDs and opaque moderation metadata outside that projection.

### 6. Preserve server-owned permissions and approvals

Read managed requirements before enabling execution. Start read-only and require
an explicit user choice for workspace write or full access. Never broaden access
to make a test pass and never translate a named managed profile into a locally
invented sandbox policy.

For server-initiated requests:

- bind the request to its exact connection generation, native thread and owner;
- expose only decisions offered by the server;
- grant only the subset of filesystem/network permissions actually requested;
- distinguish network destination approval from a shell-command approval;
- consider `serverRequest/resolved`, turn completion and disconnect terminal
  states for the card;
- keep form answers and credentials in memory only.

`thread/shellCommand` explicitly runs outside the thread sandbox. If adopted, it
must be a separate user-initiated terminal action with a strong disclosure; it
must never be a hidden implementation of a chat command.

### 7. Keep authentication owned by Codex

Prefer the managed ChatGPT browser or device-code flows already used by Central
Agent. Open only allowlisted HTTPS OpenAI URLs returned by the exact active login
attempt. Do not read Codex credential files or receive tokens in WebView IPC.

Remember confirmed connection intent, not authentication material. The current
host saves `{version:1,reconnect:true}` in `app-server-connection.json` only
after a valid supported account/read. On launch, under the existing profile
lock, consume one startup attempt and use the normal official-process handshake
plus fresh read-only account/catalog/requirements inventory. Repeated panel-ready
events and transport failure must not create retries or replay native work.
Native cached sign-in and refresh stay with Codex; see the
[official auth guide](https://learn.chatgpt.com/docs/auth). Ephemeral native
credential configuration cannot be made durable by this preference and is never
overridden by Central Agent.

Durably save false before native logout and close the in-memory remember gate
so stale replies cannot undo the opt-out. Keep intent on transient failure;
disable it for authoritative missing/unsupported accounts. Preserve invalid
files, refuse backup resurrection, and block logout if opt-out persistence
fails. Legacy local bindings/model settings can justify a one-time account
check on upgrade, never assumed authentication or restored permission consent.

API-key and Amazon Bedrock responses are recognized but explicitly unavailable
under the current ChatGPT-subscription-only policy. Their credential data is not
projected. If either becomes a product requirement, introduce it as separate
provider-policy and secret-handling work; externally managed ChatGPT tokens remain
experimental and should not be enabled in this client.

Consume `account/updated`, `account/login/completed` and
`account/rateLimits/updated` as invalidation events, then refresh authoritative
account data. If a rate-limit read is already in flight, coalesce the invalidation
into one follow-up read so the older reply cannot remain current. Destructive or
externally visible account actions, such as logout,
rate-limit reset consumption and owner-notification email, always require direct
user intent.

The stable `permissionProfile/list` method is a read-only inventory in Settings.
Do not turn it into a selector until `thread/start.permissions` is stable and the
product has an explicit consent design. Rate-limit UI keeps only public bucket and
window fields from the authoritative read; account IDs, individual credit
records/IDs, spend controls and upsell payloads stay outside the WebView. The
available reset-credit count is the sole P2 exception and crosses as decimal
text, not as authority.

The P1 account readbacks follow the same rule. `account/usage/read` preserves
safe 64-bit counters and nullable values without exposing billing internals;
`account/workspaceMessages/read` retains only bounded message identity, type,
body and timestamps. `modelProvider/capabilities/read` controls only matching
affordances and never implies account access or permission.

### 8. Use plugins without building a second tool loop

The composer reads `plugin/installed`, including the active project directory
for repository marketplaces. It exposes installed user-facing plugins and omits
packages whose native policy is `INSTALLED_BY_DEFAULT`; those packages provide
Codex infrastructure rather than ordinary composer choices. Raw `app/installed`
connector rows must not be relabelled as plugins because that inventory includes
internal services such as document control, safety settings and hotline lookup.

Selection is frozen to the exact owner, loaded native thread and inventory. Only
an accepted prompt receives the official `@plugin-name` token and matching
`plugin://name@marketplace` mention returned by App Server. Codex remains
responsible for installation, authentication, tool discovery, execution and
approvals. A plugin/skill update racing a refresh receives at most one automatic
follow-up; a second race leaves an explicit stale state instead of looping.

Do not convert plugins or their connector services into Central Agent custom
tools. Side-effecting and destructive calls continue through the native
request/approval path.

Use the same identity discipline for direct MCP: keep resource URIs Rust-side
behind opaque handles; require an explicit confirmation for tool calls bound to
the exact thread/inventory/server/tool/arguments; bound and redact results, and
never retry an uncertain side effect. Hook support is read-only native inventory
and activity. Do not expose command bodies/hashes or create a local hook runner.
Filter large MCP server/tool/resource/template inventories locally and show
displayed-versus-total counts; never use a thread ID as a human label.

Extra skill roots are process-scoped explicit configuration only. Canonicalize
at most 16 existing absolute directories, reject known credential/secret paths,
invalidate discovery after writes and clear the local observation on reconnect.

Plugin RPCs are explicitly described as under development by OpenAI. The bounded
read-only composer inventory is pinned to the supported App Server schema. Do not
expose install/uninstall or marketplace mutation until OpenAI removes that
warning and those flows have their own security review.

### 9. Avoid duplicate filesystem and process authority

Central Agent already owns a local editor, Explorer and terminal. Do not replace
them wholesale with App Server `fs/*`, `command/exec` or experimental `process/*`
APIs. P2 adopts only `command/exec` and its three controls as a visibly separate
App Server sandbox utility: current local-project cwd is selected in Rust, input
is a bounded argv vector, the native process ID remains Rust-side, output is
bounded and escaped, and only read-only/project-write without network or
environment overrides is offered. Full access is intentionally unavailable.
Connection loss terminates the process and never replays it. Keep `fs/*` reserved
for a future native execution-environment view instead of adding a hidden second
filesystem authority.

The other adopted P2 workflows follow the same pattern:

- feature listing is paginated/atomic; enabling one named beta/stable entry is
  explicitly confirmed and process-wide, without setting `experimentalApi`;
- external migration detects only home and the Rust-selected current project,
  retains exact raw items behind opaque handles, freezes the preview before
  import, and exposes count-only progress/history without raw failure messages;
- reset redemption requires a current positive authoritative count, creates an
  idempotency UUID in Rust and reuses it only for an explicit retry after unknown
  delivery. Never expose or accept a credit ID;
- account email nudge handles `sent` and `cooldown_active` as distinct outcomes;
- feedback respects managed disablement and sends only a bounded, previewed
  category and reason with `includeLogs:false`, no extra files, no tags and no
  thread context. Never show unknown delivery as success.

Every mutation dialog freezes the exact view, scope and values it displays. In
particular, a project change while the command dialog is open must cause the
Rust scope check to reject the old confirmation rather than retarget it.
Synchronize the P2 project scope from the Rust-owned active project on connect
and every project switch; do not require a WebView button to make that authority
current.

Keep configuration-host focus ownership explicit. A disabled control must retain
the open host, a loading rerender with no `relatedTarget` must not be interpreted
as outside focus, and an open native `<dialog>` must own Escape and backdrop
interaction until it closes. Refreshing a disclosure must not collapse the
settings page.

Do not add generic WebView routes for `fs/*`, `thread/inject_items`,
`thread/shellCommand`, `config/value/write`, marketplace or plugin methods.
Existing Explorer/editor, native history, the separate sandbox utility and
revision-checked batch writes retain ownership.

### 10. Acceptance must cover four layers

A feature is complete only when all applicable layers pass:

1. schema: exact request/response/event shapes for the selected CLI;
2. runtime: real process transport, ordering, interruption and reconnection;
3. host/UI: owner isolation, semantic IPC, rendering and explicit consent;
4. environment: live account, OS dialog, sandbox, browser and model behavior.

Fixture success must not be reported as live-account or Windows acceptance. The
current open gates and the full method inventory are maintained in
[`CODEX_APP_SERVER_GAP_ANALYSIS.md`](CODEX_APP_SERVER_GAP_ANALYSIS.md).
