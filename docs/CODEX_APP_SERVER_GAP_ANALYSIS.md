# Codex App Server feature and gap analysis

Status: **2026-09-09, P0/P1/P2 remain closed in their selected scope; P3-A adds
Git metadata, backend sections and protected revert in source**. P3-A has no new
UI controls or visual-audit claim. The earlier full-workspace/desktop audit is
P0/P1/P2 evidence. The unsigned package remains the historical P1 build, while a
verified unsigned P2 source preview now exists in
`outputs/codex-p2-source-preview-2026-09-09`. It is non-distributable because it
was built from uncommitted source; implementation invoked no account, email,
feedback, external-import or command side effect.

- Selected and latest official runtime at this audit: **Codex CLI 0.153.4**.
- Production capability negotiation: stable protocol, `experimentalApi: false`.
- Versioned wire source: `protocol/app-server/0.153.4`.
- Directly serialized methods: **67 including `initialize`**, represented by
  **121 constructor samples**. One, `thread/items/list`, is prepared but always
  blocked by the transport; wrapper count is not enabled-feature count.
- Official-page request inventory used by this audit: **82 methods**.
- Remaining methods from that page inventory without a direct wrapper: **21**, all
  deliberate duplicate-authority, maturity or deprecation holds.
- Additional methods generated only with `--experimental`: **56**, intentionally
  unavailable in production.

Counts describe protocol surface, not percentage completion. A single method
such as `turn/start` owns more product behavior than several metadata reads.

Sources:

- [official App Server documentation](https://learn.chatgpt.com/docs/app-server)
- [official Codex changelog](https://learn.chatgpt.com/docs/changelog)
- [vendored rolling documentation](https://learn.chatgpt.com/docs/app-server)
- [selected generated contract](../protocol/app-server/0.153.4)
- [current acceptance evidence](CODEX_APP_SERVER_ACCEPTANCE.md)

## Executive assessment

### P3-A backend addition

The [P3-A increment](CODEX_APP_SERVER_P3A.md) implements native Git metadata
capture/revalidation and update, section inventory/CRUD/membership/order, and
protected paginated-history revert with durable ID-only receipts. Native tests
verify a three-to-one-turn revert, subsequent explicit resume and receipt
recovery from disk with no replay or local file changes. Item pagination has a
bounded decoder but stays off at the wire boundary. No new visual surface,
experimental capability, automatic migration or P3 executable was introduced.

P3-A presentation and appearance editing remain future work. They are not
silently included in the previous Astra/Computer Use acceptance.

The minimum rich-client loop and the planned stable P0/P1/P2 product surface now
exist in verified source: connection/authentication, discovery, native conversations,
streaming, decisions, history, lifecycle, precise forks, reviews, goals, skills,
Apps, hooks, MCP management and direct operations, account information, media
input, model/personality fidelity, bounded extended forms and recovery.

This is not a complete-public-parity or signed-production claim. Work remaining
outside the implemented stable milestones is:

1. external enterprise registration of the stable `central_agent` client name;
2. user-authorized OS, OAuth and external-side-effect checks when those workflows
   are actually needed;
3. converting the verified P2 source preview into a clean, reviewed and signed
   distributable when a public handoff is requested;
4. waiting for stable public semantics before enabling plugin or experimental
   APIs.

## Post-visual-audit defect closure

The 2026-09-09 source increment closes the defects found in the first full-app
inspection:

| Observed defect | Current source result |
| --- | --- |
| App Server required `CENTRAL_AGENT_CODEX_BIN` although the official desktop CLI was installed | Bounded official desktop-build discovery is the final automatic source after packaged and PATH candidates. Every automatic candidate is version-probed; the explicit override remains strict. A no-inference handshake passed without the variable. |
| P2 project scope required a manual synchronization | Connect and active-project changes now synchronize the Rust-owned scope automatically. Display paths omit Windows extended prefixes without changing API paths. |
| Reported thread settings stayed empty after ordinary starts/resumes/forks | Stable response fields now seed the allowlisted report immediately; later settings notifications remain authoritative for additional fields. |
| Apps refresh could close the host or loop on a racing update | The host survives loading rerenders, one race schedules at most one atomic follow-up read, and the explicit action requests fresh catalog and installed-runtime snapshots. |
| A connection-required alert survived a successful reconnect | Main and graph decision/lifecycle surfaces clear that superseded local alert on the disconnected-to-connected transition while retaining real native errors raised after connection. |
| Disabled context control, slash modals and advanced refreshes collapsed configuration | Compound-host focus/escape handling now respects disabled elements, loading focus loss and an open nested dialog; all native modals use the shared visible backdrop. |
| Model descriptions, import choices and large inventories were difficult to read/use | Descriptions wrap, checkbox text is part of the label, and Apps/MCP/features have local filters with visible counts and empty results. |
| History, paths and native IDs leaked implementation-oriented presentation | History fallback text is redacted/collapsed/bounded, local paths are normalized for display, and goal/MCP dialogs no longer present opaque thread IDs. |
| Terminal panel rejected `ui_owner` | Terminal IPC now uses the same validated owner-envelope parser as the main panel. |
| A large native history produced a response beyond the 64 MiB JSONL limit | Link is metadata-only; resume/read/fork suppress inline turns and hydrate bounded, ordered `thread/turns/list` pages atomically. Duplicate cursors/turns and excessive page/turn counts fail closed. The real oversized history resumed without disconnecting. |
| Explicit resume rejection could leave the local UI pending | Active-writer rejection now releases the exact pending owner without being misreported as a failed prompt submission. |
| Closing the app could destroy WebView2 from the wrong thread | WebView2 shutdown is marshalled back to its owning UI thread; close/relaunch completed without an Application Error. |
| Apps failures could render an upstream HTML error page and stale recovery copy | HTTP 401/403 is a distinct non-current account/workspace-unavailable state. It clears loading/race copy, contains no HTML or remote body, and never suggests Resume or a false empty catalog. |
| An old transcript appeared to contain a tool/protocol fragment | The exact fragment exists inside the persisted native agent message and task-complete record. Projection tests and source tracing found no cross-item merge; source history is preserved instead of pattern-stripped. |
| A long MCP server/tool name clipped its confirmation and created horizontal scroll | The dialog uses a bounded border box, zero-minimum grid children and forced safe wrapping; light/dark desktop retest shows the complete name with no horizontal scrollbar. |
| Import became idle after its acknowledgement while native work continued | The accepted import identity remains busy through progress/completion, blocks overlap and becomes explicitly uncertain on post-ACK disconnect. |
| Rejected terminate left a live command permanently in `stopping` | Definitive rejection restores controllable `running`; malformed/uncertain delivery fails safe and completion racing the response remains terminal. |
| Feedback/reset/email errors retained active request copy | Every terminal failure has a dedicated status; reset retains its idempotency identity only across an explicit uncertain outcome. |
| Late start/resume/fork response could replace newer settings | A settings revision orders response, notification, unload and disconnect so stale acknowledgements cannot regain authority. |
| Preference, skill and Goal directory controls disagreed with the conversation on Windows | All three views use the same display-path serializer while Rust keeps canonical extended paths internally. |
| Paginated recovery could not durably clear an exact uncertain normal-turn receipt | Completed bounded hydration reconciles only the exact native `clientId`, persists the binding before publishing success and fails closed on write error; four physical wire-loss orderings pass with no replay. |

The final clean gate passes 693 ordinary Rust tests (22 ignored by the ordinary
workspace gate, including 19 runtime probes), three scope probes and 171 ordinary frontend tests (one retained-image
capture skipped), plus generated contracts, Svelte/TypeScript, 35 design
scenarios, bundle, formatting, locked all-target checks, strict Clippy, release
probes and dependency audit. The visible 0.153.4 desktop audit covers both themes,
reconnect/resume, history, settings, Goal, lifecycle/review confirmations, Apps,
MCP, hooks, permissions, skills/defaults and clean close/relaunch. The official
Apps inventory returned HTTP 403, so an App could not be selected; this is an
external availability/account state, while its terminal unavailable presentation
is verified in both themes with no stale, resume or empty-inventory copy. No OAuth, direct MCP
call, destructive lifecycle action, Full access or
P2 account/external side effect was executed merely for testing.

All 19 runtime probes were also run explicitly after the gate. Fifteen pass. The
four remaining diagnostics demonstrate absent or experimental 0.153.4 behavior:
MCP progress and network-policy callbacks were not emitted, named permission
selection requires `experimentalApi`, and print-only review policy stopped the
command before an approval callback. These are not silently promoted to product
support and are not worked around with synthetic events.

## P0 status

P0 is implemented in commit `857e2e8` and retained by P1:

- bounded typed error, warning, guardian, strict/automatic-review,
  world-writable, terminal-interaction and moderation-presence projections;
- atomic paginated `thread/loaded/list` reconciliation and connection-local
  `thread/unsubscribe` after durable local unlink;
- stable read-only `permissionProfile/list`, with experimental named-profile
  selection deliberately disabled;
- authoritative rate-limit refetch/invalidation and explicit non-ChatGPT account
  policy;
- one stable `central_agent` client/service name, with external OpenAI enterprise
  registration still a release gate.

## P1 closure status

| Capability | Status | Implemented behavior and boundary |
| --- | --- | --- |
| Plugins | **Closed to the supported App Server boundary** | Bounded `plugin/installed` inventory, project-scoped marketplace discovery, exclusion of `INSTALLED_BY_DEFAULT` control-plane packages, and explicit owner/thread/inventory-scoped selection. Accepted input prepends the official `@plugin-name` token and matching `plugin://name@marketplace` mention. Raw connector services are never presented as plugins. Codex retains installation, authentication, execution and approvals. |
| Provider/model fidelity | **Closed in P1** | `modelProvider/capabilities/read`, personality gated by `supportsPersonality`, exact model/provider/profile freezing, known input modalities and sanitized upgrade metadata. Long model descriptions wrap; upgrades are never automatic. |
| Account information | **Closed in P1** | Read-only `account/usage/read` and `account/workspaceMessages/read` with exact safe 64-bit values, bounds, nullable/stale states and removal of unknown/private fields. |
| MCP direct access | **Closed in P1** | `mcpServer/resource/read` uses Rust-only URIs behind opaque UI handles. `mcpServer/tool/call` requires a fresh explicit confirmation bound to owner, thread, inventory, server, tool and JSON-object arguments. Large server/tool/resource inventories are locally filterable and do not present native thread IDs. Results are bounded/redacted; binary/media output is omitted; uncertain side effects are not retried. |
| Extended MCP forms | **Closed in P1** | Initialization declares both the `openai/form` extension and the compatible legacy boolean. Schemas/results are bounded and validated. Flat primitive fields use typed controls; nested arrays/objects use a sanitized JSON editor. No schema HTML or executable content is accepted. |
| Precise forks | **Closed in P1** | A durable read-only fork can stop at an explicitly selected completed `lastTurnId`; source, directory and destination profile are frozen and revalidated. |
| Detached reviews | **Closed to the 0.153.4 boundary** | Legacy histories may create a separate destination bound only to the returned `reviewThreadId`, under either response/notification ordering. The actual 0.153.4 runtime rejects detached delivery for paginated histories (`-32600`), so that UI state is truthfully disabled; fork + inline review remains two explicit actions. |
| Hooks | **Closed read-only in P1** | `hooks/list`, `hook/started` and `hook/completed` feed an exact-thread/directory inventory and recent activity. Commands, hashes, MCP targets and plugin internals are never projected; Central Agent never runs a second hook engine. |
| Extra skill roots | **Closed in P1** | `skills/extraRoots/set` accepts at most 16 existing canonical absolute non-secret directories, is process-scoped, invalidates discovery and clears locally on reconnect/restart. It is neither persisted nor inferred. |
| Audio input | **Closed in P1** | MP3/WAV snapshots require matching signatures, an 8 MiB per-file bound, a 12 MiB serialized-input aggregate, and an explicit model audio modality. Bytes are frozen into native data URLs; paths/bytes never enter projected history. |
| Existing partial constructors | **Closed where a product owner exists** | Plugin mentions, audio, personality, `lastTurnId`, detached-review ownership, direct MCP and extended forms now have host/UI workflows and tests. Advanced instruction/config/ephemeral/structured-output fields remain protocol-ready only for the reasons below. |

The focused schema verifier validates all 94 outgoing constructor samples plus
11 dedicated P1 response/notification fixtures for account, Apps, hooks and MCP
against the generated 0.153.4 contract.

The full deterministic gate passes 659 ordinary Rust tests (22 explicit probes
ignored), three additional scope-probe tests and 158 frontend tests (one retained
image capture skipped), plus Svelte/design validation, generated bundle, release
pipeline probes and Clippy with warnings denied. Dependency audit reports four
explicitly allowed transitive advisories and no blocking result. The unsigned
closure package is `outputs/codex-p1-final`.

## P2 source and deterministic closure status

| Capability | Status | Implemented behavior and boundary |
| --- | --- | --- |
| Sandboxed command utility | **Closed in P2 source** | `command/exec`, `/write`, `/resize` and `/terminate` have a separately labelled Settings surface. Input is an argv vector; Rust automatically follows the active local-project cwd, supplies an opaque native process ID, enforces read-only/project-write policy, no network/environment override, bounded PTY/output and no Full access. Disconnect terminates rather than replays. |
| External-agent import | **Closed in P2 source** | `detect` accepts only Rust-selected home/current-project scopes. Raw migration details stay Rust-side behind opaque handles; import requires a frozen preview and sends exact selected values. Progress/completion and `readHistories` expose counts without raw failure messages or source/target paths. The app does not fabricate `recordHistory`. |
| Runtime feature controls | **Closed in P2 source** | `experimentalFeature/list` is atomically paginated and locally filterable. A fresh confirmation can update one reported beta/stable flag process-wide; under-development/deprecated/removed entries are read-only. `experimentalApi` remains false and raw config is not written. |
| Account side effects | **Closed in P2 source; live effect opt-in** | Reset-credit consumption requires a ChatGPT account, current positive available count and explicit gesture. Only that count crosses the UI, never credit IDs. Rust owns the UUID idempotency key and reuses it only for explicit uncertain retry, then refreshes rate limits. Credit/usage-limit email nudge reports sent/cooldown. |
| Feedback | **Closed in P2 source; live effect opt-in** | `feedback/upload` is a bounded text-only preview/confirmation and respects managed disablement. Logs, paths, attachments, tags and conversation IDs/content are always omitted; uncertain delivery is neither replayed nor presented as success. |
| Native filesystem | **Deliberate hold** | Explorer/editor remain the single local filesystem authority. No `fs/*` bridge or hidden watcher was added. |
| Injection, shell and single-value config | **Deliberate hold** | No general `thread/inject_items` or `thread/shellCommand` UI. The separate argv utility owns the narrow command case, and revision-checked `config/batchWrite` remains safer than `config/value/write`. |

The focused verifier covers 111 outgoing samples plus 15 dedicated P2
response/notification fixtures. Focused Rust tests cover opaque migration
values, stale-project rejection, idempotent reset retry, no-replay disconnect
handling and argv/full-access boundaries; frontend tests cover escaped bounded
projection and absence of duplicate authorities.

The clean pre-hardening P2 baseline gate passed 668 ordinary Rust tests with 22 explicit
probes ignored, three additional scope-probe tests and 160 frontend tests with
one retained-image capture skipped, plus 35 design scenarios, generated bundle,
release-pipeline probes, Clippy with warnings denied and dependency audit with
four allowed transitive advisories and no blocking result. Live P2 side effects
and a signed replacement package remain separate acceptance/delivery work; the
verified source preview supersedes the P1 binary only for local testing.

## Deliberate protocol-ready fields, not product features

Codex Cloud chat discovery is separate from this App Server method inventory.
Supervisor uses the official subscription CLI surface (`codex cloud list` and
`codex cloud diff`) with the same isolated ChatGPT login. The stable App Server
schema has no Cloud thread source or transcript method, so Cloud tasks are never
misrepresented as resumable local threads. Their first-party pages own complete
history and follow-up input.

The API module serializes and schema-tests some stable fields without granting a
generic WebView/RPC route. They remain intentionally unexposed until a concrete
trusted workflow owns their lifecycle:

- `thread/start`, `thread/resume` and `thread/fork` arbitrary base/developer
  instructions, `config`, `threadSource`, `excludeTurns` and ephemeral variants;
- additional `thread/list` provider/source/cwd/ancestry/state-database filters;
- `turn/start.outputSchema`, `toolOutput` and arbitrary `turnTrigger`;
- path-based `localAudio` input (the product freezes validated bytes instead,
  avoiding time-of-check/time-of-use and path-disclosure issues);
- `thread/metadata/update.gitInfo`.

These are not incomplete UI toggles. Exposing arbitrary instructions/config or
host-produced tool output would create a new authority boundary. Ephemeral forks
conflict with the current durable, resumable branch workflow. Structured output
needs a specific consumer and schema/result lifecycle before it is useful.

### Selected-version metadata limitation

The rolling official page currently discusses thread pinning. The generated
**0.153.4** `ThreadMetadataUpdateParams` and `ThreadListParams` do **not** contain
`isPinned`; metadata update contains only optional `gitInfo`. Central Agent does
not invent pinning or derive Git metadata from unrelated local state. The
`thread/metadata/update` constructor is retained for exact future Git-metadata
work, but there is no P1 pin/unpin control for this selected runtime.

## Methods directly wrapped

| Area | Methods |
| --- | --- |
| Handshake | `initialize` followed by `initialized` |
| Account | `account/read`, `account/login/start`, `account/login/cancel`, `account/logout`, `account/rateLimits/read`, `account/usage/read`, `account/workspaceMessages/read` |
| Account side effects | `account/rateLimitResetCredit/consume`, `account/sendAddCreditsNudgeEmail`, `feedback/upload` |
| Apps | `app/list`, `app/installed`, `app/read` |
| Discovery/config | `model/list`, `modelProvider/capabilities/read`, `permissionProfile/list`, `configRequirements/read`, `config/read`, `config/batchWrite` |
| Feature controls | `experimentalFeature/list`, `experimentalFeature/enablement/set` |
| External import | `externalAgentConfig/detect`, `externalAgentConfig/import`, `externalAgentConfig/import/readHistories` |
| Sandboxed command | `command/exec`, `command/exec/write`, `command/exec/resize`, `command/exec/terminate` |
| Threads | `thread/start`, `thread/resume`, `thread/read`, `thread/turns/list`, `thread/list`, `thread/loaded/list`, `thread/unsubscribe`, `thread/fork`, `thread/metadata/update`, `thread/name/set`, `thread/archive`, `thread/unarchive`, `thread/delete`, `thread/compact/start` |
| Goals | `thread/goal/get`, `thread/goal/set`, `thread/goal/clear` |
| Turns/review | `turn/start`, `turn/steer`, `turn/interrupt`, `review/start` |
| Skills/hooks | `skills/list`, `skills/config/write`, `skills/extraRoots/set`, `hooks/list` |
| MCP | `mcpServerStatus/list`, `config/mcpServer/reload`, `mcpServer/oauth/login`, `mcpServer/resource/read`, `mcpServer/tool/call` |
| Windows | `windowsSandbox/setupStart` |

## P2 and duplicate-authority decisions

| Capability | Methods | Current decision |
| --- | --- | --- |
| Sandboxed command utility | `command/exec`, `command/exec/write`, `command/exec/resize`, `command/exec/terminate` | Implemented as an explicitly App-Server-sandboxed surface with separate identity. |
| Native filesystem | `fs/readFile`, `fs/writeFile`, `fs/createDirectory`, `fs/getMetadata`, `fs/readDirectory`, `fs/remove`, `fs/copy`, `fs/watch`, `fs/unwatch` | Existing Rust Explorer/editor owns local files. Reserve for a future native execution-environment view, not a second hidden filesystem authority. |
| External-agent import | `externalAgentConfig/detect`, `externalAgentConfig/import`, `externalAgentConfig/import/readHistories` | Implemented with frozen preview, opaque raw details, progress and sanitized history recovery. |
| Experimental-feature controls | `experimentalFeature/list`, `experimentalFeature/enablement/set` | Implemented with explicit named updates; never toggles `experimentalApi`. |
| Account side effects | `account/rateLimitResetCredit/consume`, `account/sendAddCreditsNudgeEmail` | Implemented behind user gesture, idempotency/uncertainty and cooldown handling. |
| Feedback upload | `feedback/upload` | Implemented as text-only explicit opt-in; logs and files are always absent. |
| History injection/full-access command | `thread/inject_items`, `thread/shellCommand` | No general UI. Require a narrowly trusted import or terminal workflow respectively. |
| Single-value config write | `config/value/write` | Existing revision-checked `config/batchWrite` is the safer common path. |

## Publicly documented methods not directly wrapped

Of the dated 82-method official-page inventory, **21 methods** still have no
direct wrapper. P3-A also adds six generated-only methods absent from that page;
subtracting the total wrapper count from the page count is therefore no longer
a valid coverage calculation. Item listing now has a wrapper but remains off.

| Area | Methods | Classification |
| --- | --- | --- |
| Configuration | `config/value/write` | Superseded by batch-write design |
| Filesystem | `fs/copy`, `fs/createDirectory`, `fs/getMetadata`, `fs/readDirectory`, `fs/readFile`, `fs/remove`, `fs/unwatch`, `fs/watch`, `fs/writeFile` | Deliberate duplicate-authority hold |
| Marketplaces | `marketplace/add`, `marketplace/remove`, `marketplace/upgrade` | Hold |
| Plugins | `plugin/install`, `plugin/list`, `plugin/read`, `plugin/skill/read`, `plugin/uninstall` | Hold: official docs say under development |
| Threads | `thread/inject_items`, `thread/rollback`, `thread/shellCommand` | Hold: injection/shell duplicate existing authority; rollback is deprecated |

## Generated stable methods absent from the public guide

The selected stable union also contains methods without sufficient public
product semantics, including external-history recording, fuzzy search, legacy
auth/summary helpers, Git diff-to-remote, plugin reconciliation/share, guardian
override and Windows readiness. Revert/sections are now a bounded P3-A backend
exception, verified against generated types and isolated native behavior; they
remain absent from the UI. Central Agent does not
expose a generated method solely because a type exists. Public maturity,
threat-model review and targeted acceptance are required first.

## Experimental-only 0.153.4 surface

The experimental generation adds 56 methods across Bedrock setup,
collaboration/environments, fuzzy-search sessions, MCP event streaming,
diagnostics, raw process control, projects, remote control, background
terminals, elicitation counters, queue operations, realtime audio/text,
search/settings/timeline and plugin search.

Production keeps `experimentalApi: false`, `requestAttestation: false`, and does
not expose any of these routes. If one becomes a real product requirement, use a
separate build/runtime capability, generated experimental contract, kill switch,
threat model and acceptance suite.

## Server-request coverage

Handled modern stable request classes:

- `item/commandExecution/requestApproval`;
- `item/fileChange/requestApproval`;
- `item/permissions/requestApproval`;
- `item/tool/requestUserInput`;
- `mcpServer/elicitation/request`, including basic and bounded `openai/form`.

Not handled by design:

- experimental `item/tool/call` dynamic host tools;
- experimental `account/chatgptAuthTokens/refresh`;
- `attestation/generate`, because no capability is negotiated;
- legacy `applyPatchApproval` and `execCommandApproval` shapes.

## Operational checks outside deterministic P2 closure

The closure run inspected the actual Full HD application in light and dark,
connected to the existing 0.153.4 account, read six model profiles and provider/
usage metadata, and loaded four configured MCP servers. One observed server
exposed 138 tools and 40 resources; all 40 resource controls had valid opaque
handles. No prompt, resource, tool, OAuth flow or system setup was invoked. The
deterministic layout gate continues to cover the 2K and 4K profiles.

These environment journeys remain opt-in checks rather than missing source code:

- real account-backed App selection/tool approval;
- a real thread-scoped MCP resource read and explicitly confirmed tool call,
  including a side-effecting approval and disconnect uncertainty;
- live `openai/form` flat and nested requests;
- OS picker selection of valid/invalid MP3/WAV and model audio acceptance;
- actual legacy-history detached review if such a history is available; the
  paginated 0.153.4 rejection is already an observed limitation;
- actual native hook inventory/activity where the user's configuration contains
  hooks;
- Windows sandbox elevation, OS-browser handoff and remaining native
  network/review-approval callbacks;
- real P2 feature update, external-agent import, reset-credit redemption,
  workspace email, feedback upload and sandboxed command execution;
- enterprise registration of `central_agent`.

The observed 0.153.4 MCP reload acknowledgement also did not update tools in an
already loaded thread. Keep that as a runtime limitation; do not add a custom
reload engine or silent retry.

## Next delivery order

1. Run remaining OS/external-service checks only when the user needs and
   authorizes the corresponding operation.
2. When a public handoff is requested, review/commit the source, repeat the
   reproducible build and sign the already verified P2 preview payload.
3. Re-evaluate plugin and experimental APIs only when their official maturity or
   selected-version contract changes.
