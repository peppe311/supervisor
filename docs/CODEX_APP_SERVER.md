# Codex App Server integration

**Runtime compatibility update, 2026-09-20:** Supervisor now selects and accepts
the installed official stable runtime **0.155.1**. Its 312 JSON schemas and 721
TypeScript files were generated directly from that executable without
experimental methods. The stable contract is byte-for-byte identical to the
previously verified `0.155.0-alpha.2.6` contract, so no request, notification or
server callback changed. The exact-version manifest, runtime tests and release
source gate now share the `0.155.1` baseline; older verified versions remain
available for rollback and unknown versions still fail closed.

**Codex Cloud subscription update, 2026-09-17:** the native history dialog now
loads Codex Cloud chats through the official `codex cloud list --json` command
using Supervisor's signed-in ChatGPT profile. Cloud tasks remain distinct from
local App Server threads. Supervisor presents bounded public task metadata and a
read-only official diff; validated first-party links open the full chat or new
task page in a real browser tab. No token is read, no private endpoint or Agents
API is used, and no cloud transcript is synthesized into local history.

**Runtime compatibility update, 2026-09-17:** Supervisor accepts the verified
official desktop runtime **0.155.0-alpha.2.6** alongside
**0.154.0-alpha.6.2** and the **0.153.4** baseline.
The desktop update had removed the only installed 0.153.4 executable, so the
previous single-version check rejected Connect before the account handshake.
Runtime discovery and schema verification now share an exact-version manifest.
All 123 serialized calls, 19 approval decisions and existing incoming fixtures
pass against all independently generated contracts. The 0.155 stable schema is
additive over 0.154 and adds thread attachments without removing a method; that
new surface is not implicitly enabled. The native isolation,
profile-transition and summary/history probes pass on the installed runtime
using owned loopback fixtures without account inference. No account migration,
experimental API opt-in or change to persisted permissions is required. See
[the generated contracts](../protocol/app-server/README.md) and
[profile/restart evidence](CODEX_PROFILE_ISOLATION.md).

**Chat-tree UI update, 2026-09-10:** the Projects list now groups local forks
beneath their source with expandable nested branches, explicit Original/Fork/
Fork unconfirmed labels and direct fork counts. The composer identifies the
active chat, its source and the fact that a fork does not copy project files.
**Open source chat** navigates through the existing local-chat action only.
Confirmed ancestry is persisted separately from uncertain fork recovery, as
IDs only. Unknown ancestry is not guessed from titles or directories; older
forks can be backfilled by an explicit **Refresh history**. Full evidence and
upgrade limits are in [user validation](CODEX_APP_SERVER_USER_VALIDATION.md).

**P3-A source update, 2026-09-09:** Git metadata synchronization with repository
revalidation, native backend sections and protected paginated-history revert are
implemented. Item-list construction/decoding is prepared but `thread/items/list`
is blocked before transport writes. No new UI, experimental opt-in or executable
handoff was introduced. See [P3-A scope, tests and limits](CODEX_APP_SERVER_P3A.md).
Earlier visual-audit and package statements below refer to the P0/P1/P2 baseline.

Scope decision, 2026-09-06: implement native Codex App Server functionality first.
Custom Central Agent MCP tools and SSH/browser/desktop-control bridges
and Time Machine integration are deferred. Preserve those existing features for
other providers. Native MCP configuration and native file-change/diff events stay
in scope; conversation rewind is not a promise of filesystem restore. This scope
update supersedes earlier references below to required platform/checkpoint wiring.

Historical baseline audited 2026-09-09: the old integration is removed and the new official
client is connected to account Settings and the main/graph conversation UI.
Central Agent targets the newest official Codex CLI release, **0.153.4**; its
checked-in stable contract matches fresh JSON and TypeScript generation exactly.
The stable P0/P1 milestones and selected P2 workflows are closed in current
source, deterministic verification and the autonomous desktop visual audit. All
four original P2 findings from the independent Astra review and its follow-up
receipt-persistence finding are fixed and regression tested, but complete
public-method parity and every opt-in environment journey are not claimed. The P0 source pass
implements actionable runtime/security notices, loaded-thread subscription
cleanup, stable read-only permission-profile inventory, authoritative live
rate-limit invalidation and an explicit non-ChatGPT account policy. The P1 source
surface described below is implemented, verified, packaged and read-only live
tested; the P2 source increment is separately described below. Enterprise
distribution still requires external registration of `central_agent` with
OpenAI, and several live Windows/account/approval checks remain open. Verified
unsigned previews are recorded in `CODEX_APP_SERVER_PLAN.md`; they are not final
parity releases.

Supervisor now uses a dedicated native Codex profile, including separate history,
configuration and sign-in. See [profile isolation and migration](CODEX_PROFILE_ISOLATION.md)
for the current storage boundary and first-use transfer; this supersedes older
notes below about the official app/CLI sharing Supervisor's account profile.

For current completion evidence and remaining gaps, read
[the completion audit](CODEX_APP_SERVER_ACCEPTANCE.md). The dated implementation
notes below retain historical test scope; later successful increments supersede
their older statements about pending restart, provider switching and delivery.
The [current-version implementation guide](CODEX_APP_SERVER_IMPLEMENTATION_GUIDE.md)
records the recommended architecture, while the
[method-level gap analysis](CODEX_APP_SERVER_GAP_ANALYSIS.md) compares this client
with the complete 0.153.4 stable and experimental contracts. The
[official documentation](https://learn.chatgpt.com/docs/app-server) can be
downloaded for private local reference with `scripts/sync-codex-app-server-docs.ps1`;
the ignored download is not part of the public source distribution.
The [user-operated validation checklist](CODEX_APP_SERVER_USER_VALIDATION.md)
separates account/setup/OS-dialog checks requiring explicit user interaction from
automated coverage and runtime limitations. Unperformed checks remain unverified.

Current verified local preview (2026-09-10):
`outputs/codex-connection-preview-2026-09-10/CentralAgent.exe`, 34,317,312 bytes,
SHA-256 `dbc93fa7bd2d3b825b611ca3b9d1f2b27721ef74cb6b6e0fcc3199a9713009d8`.
Its MCP companion is 5,645,824 bytes, SHA-256
`c355cc9757226c92bd1aa1a5a7c5f69951f852b27ea6813ac895a167b070edce`.
The preview is `NotSigned`; its packaged hidden-WebView startup check passes.
The incremental build did not run the production provenance/manifest pipeline;
its artifact hashes and verification scope are in `DEVELOPMENT_PREVIEW.md`.
Because the source tree
contains uncommitted integration work, the preview is deliberately marked
non-distributable. A public release still requires a clean reviewed commit,
license review and signing.

Pending approval cards in main and graph now retain arrival order, including
double- and triple-digit tickets. New requests append without sorting before
existing cards; sending and resolution preserve survivor identity and routing.
This is a display correction only, not a new native queue, priority or permission.

### Post-visual-audit hardening

The 2026-09-09 source increment closes the defects observed in the first
full-application P2 inspection. It adds official desktop-runtime discovery,
automatic Rust-owned P2 project synchronization, initial thread-settings
projection from stable start/resume/fork responses, one bounded follow-up for a
racing Apps update, searchable Apps/MCP/feature inventories, compact redacted
history previews and presentation-safe Windows paths. Model descriptions wrap,
import checkboxes have connected labels, opaque thread IDs are not displayed,
and the terminal uses the shared validated `ui_owner` envelope.

Native-history links are metadata-only until an explicit resume. Resume/read/
fork request no inline turns and hydrate bounded ascending
`thread/turns/list` pages atomically, preventing a real large history from
exceeding the 64 MiB JSONL frame limit. Oversized frames fail before unbounded
growth, explicit active-writer resume rejection releases pending UI state, and
WebView2 destruction is returned to its UI thread. Apps errors expose only a
bounded sanitized status instead of an upstream HTML body; long MCP tool names
remain complete inside a no-horizontal-scroll confirmation.

The independent P2 follow-up also fixes four lifecycle races: an accepted import
remains busy until native completion; terminate rejection restores a controllable
process while uncertain delivery fails safe; feedback/reset/email errors have
explicit terminal presentation; and a settings revision prevents late
start/resume/fork responses from replacing newer native state. Paginated history
reconciliation now clears a persisted uncertain receipt only for the exact
native `clientId` and saves the updated binding before publishing success. Goal,
skill and preferences directory projections use the
same display path identity as their owning conversation.

The original end-to-end remediation makes a user-requested Apps refresh fetch fresh
catalog and installed-runtime snapshots. HTTP 401/403 becomes a distinct
account/workspace-unavailable state that clears superseded loading and race copy,
never recommends Resume or labels unavailable data as an empty catalog, and
cannot select, install or call an App. A successful reconnect also clears only the obsolete local
connection-required alert in main and graph decision/lifecycle surfaces; native
operation errors raised after connection remain visible.

### Installed plugin identity and connector separation (2026-09-16)

A read-only check using Supervisor's separate Codex profile showed why the first
selector inventory was misleading. `app/installed` returned GitHub and Sites
together with **Codex Document Control**, **Hotline**, **Plugin Management** and
**Safety Settings**. Those rows are connector/runtime services. The same native
profile's `plugin/installed` response reported the actual installed plugins and
their exact `name@marketplace` identities.

The composer now uses `plugin/installed` as its authority. It excludes packages
whose native install policy is `INSTALLED_BY_DEFAULT`, so control-plane packages
such as Plugin Management do not appear as ordinary choices. Project cwd is sent
for repository marketplace discovery; disabled or administrator-blocked plugins
remain visible in Settings but cannot be selected. A racing plugin/skill change
receives one bounded follow-up inventory and then becomes explicitly stale.

The same installed-plugin response is the icon authority. Supervisor accepts a
`composerIconUrl` only when it is a bounded, credential-free HTTPS URL on the
exact `files.openai.com` host, sends no referrer while loading it and falls back
deterministically if the image fails. GitHub, Cloudflare, Figma, Linear and
Notion also have bundled monochrome fallbacks, while every other installed
plugin receives a stable initial. The resulting icon is reused in the main and
graph composer rows, the selected-plugin card and the Settings inventory. This
supersedes the earlier bundled-only icon behavior without changing plugin
selection, installation or authentication ownership.

Use **Settings → AI → Codex tools → Plugins → Refresh Plugins** to inspect the
installed user-facing inventory. Selecting **GitHub** in the main or graph
composer leaves the compact icon-and-name card with its dedicated X. Only the
matching accepted prompt receives `@github` plus the exact
`plugin://github@openai-curated-remote` mention. Selection alone starts no turn
and calls no tool. Leaving the selector empty preserves Codex's own plugin/tool
choice. Installation, authentication, execution and approvals remain owned by
Codex; Supervisor stores no service credential and runs no duplicate plugin.
Figma, Linear and Notion enter the same selector automatically after Codex
reports them installed and enabled; any other official plugin appears with its
catalog icon as soon as `plugin/installed` reports it.

Validation uses inventory/selection regression tests and native metadata reads;
it does not publish repository changes or claim a completed model/tool execution.
The previous audit's catalog denial below is historical.

References: [official App Server API](https://learn.chatgpt.com/docs/app-server)
and [official plugins guide](https://learn.chatgpt.com/docs/plugins).

The compound configuration host now remains open across disabled-control clicks,
loading rerenders and advanced refresh actions. An open native dialog owns focus
and Escape, uses the shared tokenized backdrop and no longer appears as an
invisible blocker; the light-theme close control has explicit contrast.

The final complete gate passes 693 ordinary Rust tests (22 ignored by the
ordinary workspace gate, including 19 runtime probes), three scope probes and
171 ordinary frontend tests (one retained-image
capture skipped), plus the 111-call generated contract suite, Svelte/design,
bundle, formatting, locked all-target, strict Clippy, release and dependency
audit checks. All 19 opt-in probes were then executed separately: 15 pass and
four expose upstream stable-runtime boundaries without client emulation. A
no-inference isolated-home smoke check found official Codex
0.153.4 without `CENTRAL_AGENT_CODEX_BIN`.

The final desktop audit connected that runtime and covered light/dark Settings,
resume/history, Goal, lifecycle/review confirmations, Apps, conversation MCP,
hooks, permissions, skills/defaults and clean close/relaunch. The Apps catalog
returned a service/account HTTP 403, so no App was selectable; the explicit
unavailable state passed in both themes and contained no upstream HTML, stale
notice, misleading Resume instruction or false empty-inventory claim. Destructive
lifecycle actions, OAuth, direct tool calls, Full access and P2 external/account
side effects were inspected but cancelled, not executed. The source preview is a
test handoff, not a replacement signed distributable.

The protocol-looking fragment visible inside one older answer was also traced to
the native persisted `agentMessage`, its response item and task-complete record.
The Central Agent projection presents that authoritative message verbatim and did
not splice in a tool item. Pattern-based stripping would corrupt legitimate
history, so this remains documented as an upstream/source-data anomaly rather
than a missing client-side sanitizer.

### P0 App Server hardening

The stable production client now constructs 35 App Server methods plus
`initialize`. On each connection and explicit account refresh it atomically
reconciles all pages of `thread/loaded/list` against existing local bindings.
Deleting a local chat/graph surface or ejecting its workspace durably removes the
local binding and sends `thread/unsubscribe` for a loaded native session. This is
connection cleanup only: it never calls native thread deletion or removes Codex
history.

AI Settings shows the bounded, read-only `permissionProfile/list` inventory and
managed-profile requirement state without opting into experimental
`thread/start.permissions`. It also shows authoritative ChatGPT rate-limit
buckets; `account/rateLimits/updated` only invalidates and triggers a fresh read,
so sparse notification data is never treated as a complete snapshot. API-key and
Amazon Bedrock states are recognized but explicitly non-runnable under the
subscription-only policy, and their secret/credential-chain fields never cross
the host projection.

Turn errors, warnings, guardian/strict/automatic review state and moderation
presence use bounded service-notice rows. Global configuration, deprecation and
world-writable warnings use visible bounded Settings notices. Terminal input is
represented only by its UTF-8 byte count. stdin, process IDs, automatic-review
action bodies/rationales and opaque moderation metadata are not rendered or
persisted. The generated schema marks moderation metadata opaque and automatic
review payloads unstable, so Central Agent observes only a conservative public
projection and enables no experimental capability.

The completed P0 source baseline is commit `857e2e8`. At that checkpoint, the
ordinary workspace verification passed 648 Rust tests, 148 frontend tests, 68
serialized request constructors, 17 additional P0 schema fixtures, Clippy,
dependency audit and the design contract. The generated account component was
inspected in light and dark
themes for supported, unsupported and refresh-loading states; Full HD, 2K and 4K
measurements had no horizontal overflow and the browser console was clean. This
is source/component evidence only: it did not publish a new executable, query a
personal account, exercise Windows elevation or complete the external enterprise
client registration.

### P1 App Server integration

The stable-only P1 source increment is implemented. It adds:

- bounded `plugin/installed` discovery, explicit per-thread selection,
  `@plugin-name` text and matching `plugin://name@marketplace` mention input;
- `modelProvider/capabilities/read`, personality-aware profile selection, known
  input modalities and bounded upgrade metadata without automatic migration;
- read-only `account/usage/read` and `account/workspaceMessages/read` projections;
- Rust-owned opaque MCP resource handles, bounded resource reads and explicitly
  confirmed direct tool calls frozen to the exact thread/inventory/tool;
- bounded `openai/form` schema normalization/result validation, with typed flat
  fields and a sanitized JSON editor for nested arrays/objects;
- durable precise forks through an explicitly selected completed `lastTurnId`;
- detached review for compatible legacy histories, binding only the exact
  returned `reviewThreadId` under either response/notification ordering;
- read-only hook inventory/activity, process-scoped canonical extra skill roots
  and MP3/WAV native audio snapshots when the model explicitly advertises audio.

All new reads publish atomically, are owner/thread scoped, keep unknown/private
fields Rust-side and invalidate stale authority on disconnect or native changes.
App execution, MCP policy/approvals and hooks remain owned by Codex; Central Agent
does not introduce a second tool or hook engine. The focused verifier now covers
94 serialized request samples across 47 methods including initialization and 11
dedicated P1 response/notification fixtures against the generated 0.153.4 schema.

P1 closure used a disposable application profile for an actual read-only 0.153.4
connection. It confirmed six model profiles, account/provider metadata, both UI
themes and a four-server MCP inventory. The pre-fix inventory exposed four tool
schemas with legal depth beyond the generic call/result JSON bound. Schema
inspection now has a separate bounded depth budget; direct arguments/results keep
the stricter limit. The same closure fixes the projected `entryId` and
`inputSchema` casing. One real server then displayed 138 tools and 40 resources,
and all 40 resource controls had opaque handles. No resource, tool, App, prompt,
OAuth flow or system setup was invoked.

The rolling official documentation mentions `isPinned`, but the selected 0.153.4
`ThreadMetadataUpdateParams` and thread-list schema do not contain that field;
only optional `gitInfo` is available. No fake pinning control is exposed and Git
metadata is not derived from unrelated project state. The actual 0.153.4 runtime
also rejects detached review for paginated history, so that choice stays disabled
there. Arbitrary instruction/config overrides, ephemeral forks, structured output,
host `toolOutput` and `turnTrigger` remain schema-tested constructors only until a
concrete trusted product workflow owns their lifecycle.

The full deterministic P1 gate passes 659 ordinary Rust tests (22 opt-in probes
ignored), three additional scope probes and 158 frontend tests (one retained-image
capture skipped), together with Svelte/design checks, generated bundle, release
pipeline probes and Clippy with warnings denied. The dependency audit reports four
allowed transitive advisories and no blocking result; the verified unsigned P1
closure package is identified above.

Experimental dynamic tools, plugins, marketplace mutation and native filesystem
APIs remain outside the adopted product surface. Existing Central Agent Terminal,
Explorer and provider boundaries are not replaced. Optional external/OS
acceptance is tracked separately below and in the completion audit.

### P2 App Server workflows

The source now directly wraps 60 stable methods including initialization, with
111 schema-checked constructor samples and 15 dedicated P2 incoming fixtures.
AI Settings contains one labelled optional-workflows section:

- `command/exec`, `/write`, `/resize` and `/terminate` form a separate
  App-Server-sandboxed PTY utility. Rust fixes the active local-project path,
  accepts argv rather than interpolated shell text, exposes only read-only or
  project-write without network/environment overrides, retains the real process
  ID, and bounds output under an opaque WebView handle;
- `experimentalFeature/list` publishes an atomic paginated inventory. A fresh
  confirmation changes one beta/stable entry process-wide through
  `enablement/set`; it never changes `experimentalApi`, and immature/deprecated
  entries remain read-only;
- external-agent `detect` receives only Rust-selected home/current-project
  scopes. Raw migration details stay Rust-side behind opaque handles. Import
  sends exact selected items, consumes progress/completion notifications and
  offers count-only `readHistories` recovery without failure messages or
  source/target paths;
- reset-credit consumption requires a current positive available count. Rust owns and retains
  its idempotency UUID for the one explicit uncertain retry, never exposes a
  credit ID, and refreshes authoritative limits after reset/already-redeemed;
- add-credits/usage-limit email requests distinguish sent from cooldown, and
  feedback uploads only the confirmed bounded category/reason with logs, files,
  paths, tags and conversation context explicitly absent. Managed requirements
  can disable feedback, and an uncertain response is never rendered as success.

These operations are never run by startup, account refresh or a model turn.
Disconnect does not replay them; an uncertain import points to history, a feature
change requires inventory refresh, and the command process is reported closed.
The current local-project scope is synchronized automatically on connect and
project changes; a manual WebView synchronization button is not an authority
boundary.
The native filesystem methods remain excluded because Explorer/editor already
own local files. `thread/inject_items`, `thread/shellCommand` and
`config/value/write` likewise remain excluded in favor of native history,
the separate bounded argv utility and revision-checked batch writes.

The clean pre-hardening P2 baseline gate passed 668 ordinary Rust tests with 22 opt-in probes
ignored, three additional scope-probe tests and 160 frontend tests with one
retained-image capture skipped. It also passes the 108-call stable contract and
15 P2 fixtures, Svelte/design validation over 35 scenarios, generated frontend
bundle, release-pipeline probes, Clippy with warnings denied and the dependency
audit with four allowed transitive advisories and no blocking result. No live P2
side effect was used to obtain this deterministic closure.

### Protocol diagnostics

Permission-profile compatibility (verified against 0.153.4): the native
`permissionProfile/list` inventory is now displayed read-only without
experimental opt-in, but `thread/start.permissions` explicitly requires
`experimentalApi`. The stable client does not enable it implicitly. A
config-override diagnostic returned no active-profile confirmation and is not
used as a substitute. Named managed
profiles therefore still block incompatible presets rather than falling back
to broader access. See the completion audit for exact test scope and limitations.

AI Settings → Codex → **Protocol diagnostics** shows notifications that produced
no UI update. These include unsupported methods, intentionally unprojected native
events and notifications outside a bound conversation; they are not necessarily
failures. It never adds an error row or interrupts an agent.

The native host keeps only the 32 most recently observed method identities and
counts, in memory. A method name is shown only if present in the selected runtime's
generated stable JSON notification schema. An unknown name is represented by the
SHA-256 of its UTF-8 bytes, allowing a developer to compare a suspected protocol
method without exposing arbitrary incoming text. Fingerprints are identifiers,
not encrypted payloads. No event payload, command, thread ID, raw reasoning or
credential is collected, written to disk or sent to the model.

The total includes evicted entries; each row counts occurrences only while
retained. Counts saturate at 4,294,967,295 rather than wrapping. This diagnostic
storage bound is not a limit on agent work. Disconnect labels the last connection's
observations; explicit reconnect or application restart clears them. Viewing the
disclosure requires no native API request and grants no permission.

### Testing Stop while a native review command is running

The opt-in `--check-native-conversation --allow-test-inference --review
--review-command-stop [--graph]` test waits for actual command output, then clicks
the compiled Stop control. It uses one seed and one native review in a newly
owned empty workspace. The only requested command prints a marker, sleeps for
20 seconds and would print a final marker; the test must interrupt before that
final marker. This duration belongs only to the test fixture, not an application
timeout. No files, permissions or native configuration are changed.

On runtime 0.153.4 the review acknowledgement, displayed active turn and command
worker can have different IDs. The native terminal notification can identify the
review rather than the displayed Stop target. Acceptance requires native
interrupted history for both and an inactive UI, not assumed ID equality.
Native thread/read can omit review items previously sent in the live stream.
The display-only mirror retains those received items in memory; it never writes
them back to Codex history or reconstructs them on restart. The oracle verifies
all persisted items in order, native event/response evidence for every extra
display item and no activity still shown as running. Exact live/history item
equality was an invalid test assumption, not a reason to erase native live output.

`--review-approval-stop` remains a separate diagnostic, not certified acceptance:
two actual attempts did not produce a native approval callback (one print-only
command ran without one; the explicit escalation attempt ended without one).
No approval was granted. Do not simulate a reviewer approval, infer universal
unavailability or broaden access to make that diagnostic pass. This does not
invalidate the separately verified normal-turn command/file approval flows.
An additional isolated deterministic diagnostic is available without account
inference:

```powershell
cargo test --locked -p central-agent-codex-runtime native_review_print_only_observes_actual_approval_policy -- --ignored --nocapture
```

It uses the production read-only thread/review constructors, a temporary native
profile and a loopback model fixture emitting one fixed print-only command,
without escalation or permission changes. On 0.153.4, native tool output reports
`blocked by policy`; no command approval callback or command-execution item is
emitted. The native review exits and history contains two turns. This diagnostic
**fails its approval acceptance condition**; a completed review or blocked tool
does not certify interactive approval. The missing callback in this trace was
not dropped by the UI (the probe reads the native transport directly). This is
not a universal claim that all reviews can never ask for approval. See the audit
for the exact log and ordinary test evidence.

The separate review-command Stop test passed in Debug main and then in both main and graph on the
2026-09-07 20:27:37 published preview. Both retained drafts (including graph
siblings), verified unchanged files and deleted only their own native threads.
The release passed 605 Rust and 141 frontend tests, with no visual tests or
production engine changes. See the plan for artifact identity and exact logs.

### Testing Stop during native review preparation

The opt-in command `CentralAgent.exe --check-native-conversation --allow-test-inference
--review --review-prepare-stop [--graph]` tests cancellation before dispatching
`review/start`. The acceptance host delays only the real successful preparation
reply at the worker-to-host boundary, clicks the actual Stop button, verifies
pending cancellation, then delivers that exact reply unchanged. The oracle requires
no review/interrupt request, unchanged native seed history and workspace, released
pending state, hidden Stop and preserved owner/sibling drafts. This is not a native
wire-delay test, a mocked server response or an additional production cancellation
engine. Each case uses account allowance for one no-tool seed prompt and deletes
only the newly owned thread. It cannot combine with the other review test modes.
Main and graph both passed on the 2026-09-07 19:54:33 published preview; the original
native seed was unchanged and only each newly created thread was deleted.
The release pipeline passed 604 Rust and 141 frontend tests. See the plan for
artifact identity and logs; late-worker and tool-approval Stop are separate.

### Testing native Git review targets

Add `--review-git=uncommitted`, `--review-git=branch` or `--review-git=commit` to
`CentralAgent.exe --check-native-conversation --allow-test-inference --review`
to exercise the actual native target in a disposable Git repository. Optional
`--graph` selects the graph host. This is not compatible with `--review-stop`.
Each test uses one seed prompt and one native review from the connected account,
verifies native source inspection and unchanged fixture files/Git state, and
deletes only its new native thread. It does not grant approval requests or inspect
personal projects. See the plan for actual execution evidence and remaining gates.
Uncommitted, branch and commit targets have now passed actual main/graph checks:
uncommitted main in Debug and the other five cases in the 2026-09-07 19:40:54
published preview. Each used two successful native read commands and retained
its Git fixture and drafts. These runs do not certify review approval choices.

### Native review output

The final review is rendered once from its native `exitedReviewMode` item. When
the selected runtime immediately follows it with a completed, phase-less
`agentMessage` containing the identical result, the UI suppresses only that echo.
The original native items and IDs remain in Codex history. Different messages,
explicit phases and repeated results in other turns are not deduplicated.
Actual main and graph reviews passed the exactly-one-result check on the
2026-09-07 19:31:18 preview. The same check reproduced two copies before the fix.
Native history and drafts remained intact; see the plan for artifact and logs.

### Testing native review interruption

The explicit acceptance command
`CentralAgent.exe --check-native-conversation --allow-test-inference --review --review-stop`
exercises the real Stop button after native review entry, without visual tests.
Add `--graph` for the graph host and sibling-draft isolation. Each invocation
uses account allowance for one seed prompt and one self-contained no-tool review,
in a newly owned empty workspace; cleanup deletes only its new native thread.
The oracle requires the actual native interruption acknowledgement, terminal event
and persisted interrupted history, not a locally inferred completed state. Both
main and graph passed against the published 2026-09-07 19:21:52 preview, including
draft isolation and deletion of only the owned native test thread. These checks
stop shortly after native entry; preparation cancellation passed separately above.
Late worker interruption and Stop during tool approval remain separate cases.
See the plan for exact artifact,
test logs and remaining gates. No production cancellation engine was added.

### Native command approval choices

Command cards expose only the supported decisions offered for that request.
Some native callbacks offer Allow once, an exact proposed command rule and Cancel,
but not Decline or Allow for session. The client must not add those missing choices.
The Rust reply adapter checks the same offered list again before sending; the
frontend receives button availability, not authority to construct a rule. An absent
or null list uses the selected protocol's existing decisions. Empty, malformed or
unsupported-only lists grant no fallback; Stop remains a separate turn action.

The selected 0.153.4 runtime emits the documented `availableDecisions` field even
though its checked-in generated request type omits it. This optional incoming
restriction accepts only exact values from the existing generated response union;
it does not enable experimental capabilities or extend outgoing RPCs. Native
Codex saves and applies accepted policy amendments. A print-only isolated test
verified reuse by a fresh App Server process, without personal rules or account
inference. The explicit `--check-native-command-policy [--graph]` check now also
passes the real compiled request card in main/graph, with exact offered buttons,
policy-button IPC, native resolution, two persisted print-only turns, rule reuse
and draft/sibling isolation. It creates and removes its own temporary profile;
it never opens a personal account or adds a rule to the user's native home.
Network-policy amendment acceptance remains a separate gate.

The 2026-09-07 18:37 preview also verifies the compiled command-policy flow in
main and two consecutive fresh graph profiles, and the in-turn MCP request cards
on both hosts. Graph acceptance waits for the real startup catalog and validated
selection before opening fixture cards; it does not substitute a model or require
a personal account. Temporary request-fixture cleanup tolerates brief Windows
cache sharing/directory-not-empty races only within the newly owned profile.
Access-denied errors still retain that profile. This is test-harness behavior,
not a timeout or policy engine for user commands. See the plan for exact artifact
hash, test evidence, retained earlier test directories and remaining gates.

### Uncertain delivery after an application restart

When a prompt has no confirmed acceptance, Central Agent retains its delivery
receipt rather than resending it automatically. After reopening the app, use
**Check native history** in the original conversation. An exact native input ID
can resolve the warning; otherwise inspect the returned history before explicitly
dismissing that exact warning. Resume the connection before submitting new work.
Unsent text drafts are local UI state, not reconstructed native conversation history.

The compiled main/graph hosts have passed forced application-restart checks with
an actual native ACK held at the worker-to-host boundary: the pending receipt,
newer draft and graph sibling draft survive, and native history continues without
duplicate input. The runtime separately passes request/response wire-loss tests.
These are distinct scopes. Whole-app wire-loss testing is available with:

`CentralAgent.exe --check-native-conversation --allow-test-inference --restart
--restart-crash --restart-wire-loss=before|after [--graph]`

Choose `before` to drop the second prompt before it reaches Codex, or `after` to
let Codex complete it while suppressing its entire response stream before the
client receives an ACK. A seed first creates genuine native history. The parent
then terminates only its own test app and verifies exit of both its relay and the
actual official server through production Windows process ownership. A second
app process uses production loaders, the real Check native history/dismiss/Resume
controls and one explicit follow-up. No prompt is automatically replayed.

The relay is compiled inside an owned temporary directory with `rustc` and uses
the existing Node test relay. These are developer-test dependencies only, not
requirements for normal app use or a replacement Codex implementation. Messages
are forwarded unchanged until the selected loss point; version comes from the
actual selected CLI. The relay records only fixed boundary metadata and PID,
never credentials or conversation text. Both processes share only their owned
Central Agent test profile; personal Codex conversations are never listed.
Each `before` case uses two model turns; each `after` case uses three. Cleanup
deletes only the new native thread. Do not combine this with held-host-ACK or
active-close test modes. Debug main passed both cases, followed by all four
published main/graph cases on the 2026-09-07 20:40:13 preview. Drafts and native
history were preserved without replay; both the relay and actual native server
exited through production ownership. Only new test threads/profiles were removed.
The existing isolated runtime regression also passed independently without account
inference. The release passed 606 Rust and 141 frontend tests, with no visual tests.
See the plan for exact logs, artifact identity and remaining acceptance gates.

### Editing native MCP options

In AI Settings → Codex → Manage configuration, refresh the saved configuration
and choose **Edit connection options…** on a user entry. Select one option, enter
its replacement and review the frozen confirmation. Existing values are not
shown because commands, arguments and URLs can contain credentials. The saved
entry's transport, not an overriding effective transport, determines the options.
Optional saved keys also offer **Clear saved option…**; empty arrays and `false`
are values, not removal. The native versioned writer changes just that key.
Header environment-reference maps are replaced as a whole; other fields remain
unchanged. Changing executable/endpoint can reuse retained credentials, as the
confirmation explains. Saving does not reload conversations or start MCP tools.
Refresh configuration to inspect saved key presence, then explicitly reload if
desired. Transport-kind conversion and raw secret values are not part of this
editor. Timeout fields are optional native MCP configuration controls; Central
Agent does not set them on your behalf or impose a new run timeout.

### Native generated images

Completed `imageGeneration` items with an embedded PNG/JPEG `result` now appear
in a separate main/graph image row, outside the collapsed activity history. Click
the preview to enlarge it; Close preview or Escape returns to the conversation.
The native activity retains status, revised prompt and any reported saved path.
No result URL is fetched and no saved path is opened automatically. Missing or
unsupported embedded results have explicit fallback text; the original remains
in native history. Preview resource bounds (24 MiB encoded, maximum 16,384 pixels
per side and 64 million pixels) do not limit agent actions or generation itself.
The browser reports decode failure without erasing the native result. This is
presentation of the selected runtime's native image output, not a separate image
API, a custom tool, or general MCP/audio/artifact support.

Read-only native capability preflight (no inference):

```powershell
cargo run --locked -p central-agent-codex-runtime --example handshake -- --media-capabilities
```

This calls the generated stable `modelProvider/capabilities/read` with empty
params and paginates `experimentalFeature/list`, printing only capability bounds
and public image-feature metadata. It does not enable features or imply a
per-model output guarantee. On the recorded 0.153.4 runtime, `imageGeneration`
is true and `image_generation` is stable/enabled. Built-in generation consumes
Codex allowance, as described in the [official image guide](https://learn.chatgpt.com/docs/image-generation).

The opt-in `image_generation_probe` example requires `--allow-test-inference`
and requests one actual native image in a disposable workspace. Native preparation
commands use the requested workspace-write/network-disabled sandbox; the probe
does not approve requests, grant additional permissions or substitute a shell
renderer/image API. Only a native `imageGeneration` result can pass. It deletes
its own native thread and retains captured wire results in a temporary evidence
directory for nonvisual checks. Native `savedPath` is never followed or deleted.

Actual generation and identical native history now passed with server-selected
GPT-5.6-Sol. The captured **1254×1254** image was fully decoded through the
production shared main/graph projection and rendered by the compiled Svelte
component in automatic tests. Earlier attempts stopped on preparation commands
or on the incorrect assumption that savedPath must be inside cwd; those were
test limitations, not client fixes. The audit records exact artifacts and logs.
This verifies actual native output and shared rendering, not visual appearance
or a full live-generation journey through both desktop composers.

## Native MCP configuration management

The opt-in acceptance command
`CentralAgent.exe --check-native-settings --mcp-options` checks the existing
typed-option controls on an owned temporary unauthenticated profile. It must not
be combined with graph, OAuth, CRUD or draft acceptance modes. It cancels and
confirms each option set and restore/clear, comparing native configuration after
each action. Both fixture servers remain disabled: no reload or model prompt is
requested. Saved synthetic private fields must remain unchanged and absent from
the UI. This is a hidden DOM functional check, not a visual test.

This check passed on 2026-09-07 for all 12 exposed options, with 24 confirmed
writes and cancellation before each write. Verification reads the native saved
user layer, not effective default values; a missing saved key uses Clear rather
than attempting to save a default null. Equivalent integer/float representations
of timeout seconds are accepted, but changed values, false-versus-absence and
empty-list-versus-absence are not. It also verifies replacement (not merge) of
an existing HTTP header environment-name map and retention of private fields.
The final log is `%TEMP%/central-native-options-host-layer.log`. MCP CRUD/reload
and ordinary preference/skill host regressions also passed. No model was called.

In Settings, open **Codex MCP servers > Manage configuration**, then explicitly
refresh configuration. The UI shows native server names, transport, reported
enabled state and whether an entry belongs to the editable base user layer.
Stored commands, arguments, URLs and credential values are not copied into the
UI. Other layers are listed as non-editable rather than silently overwritten.

**Add a server** accepts STDIO (executable, JSON argument array, optional absolute
working directory and inherited environment variable names) or Streamable HTTP
(URL and optional bearer-token environment variable name). Review and confirm
the exact server and shared file. Creation always saves a disabled entry. Enable,
disable and removal are separately confirmed; removal deletes that exact user
entry and its options, not project/managed entries or OAuth credentials.

Writes use official `config/batchWrite`, the server-reported user file and
`expectedVersion`, with `reloadUserConfig: false`. They never rewrite TOML locally
or retry automatically. Conflicts, uncertain replies and disconnect require a
fresh inspection. Shared writes and active/queued Codex work prevent competing
configuration mutations. An invalidated in-flight read cannot revive an old
editable snapshot; pending write acknowledgements remain tracked.

Saving does not restart servers. Refresh inventory and explicitly choose the
existing Reload control when ready; it can start enabled servers and affects
loaded conversations. Other Codex clients also consume this shared file. Native
configuration precedence and startup outcomes remain authoritative.

Existing transport options now use the typed editor described above. Raw header
or credential values, transport-kind conversion and project/managed writes are
not exposed. Advanced native configuration and whole-host end-to-end acceptance
remain separate objective gates.

The opt-in isolated native check uses a fresh temporary Codex profile, not the
personal configuration, and performs no inference, OAuth or server startup:

```powershell
cargo test --locked -p central-agent-codex-runtime native_mcp_configuration_preserves -- --ignored --nocapture
```

It verifies disabled STDIO/HTTP creation, stale-version rejection, enable/disable
without reload, exact removal, preservation of unrelated settings and a synthetic
secret, then closes and removes only its owned temporary profile/workspace.

## Native availability is not the same as a protocol type

The installed 0.153.4 runtime reports feature state through the official,
paginated `experimentalFeature/list` API. On 2026-09-07 it reported
`request_permissions_tool` and `default_mode_request_user_input` as
`underDevelopment`, disabled; `tool_call_mcp_elicitation` was stable and enabled.
The corresponding generated request types and client cards do not mean those
first two model tools are currently available. Do not enable experimental flags
or substitute an MCP/custom tool to manufacture native behavior.

Developer-only read-only inspection, without model usage or thread creation:

```powershell
cargo run --locked -p central-agent-codex-runtime --example permission_scope_probe -- --check-availability
```

Its separate `--allow-test-inference` mode first requires the native permission
tool to be both stable and enabled. Only then may it create one owned native
conversation and submit one denial-only test prompt. No permission is granted,
and only that test's own history is deleted. The first exploratory prompt before
the preflight was added returned `PERMISSION_TOOL_UNAVAILABLE`; its history was
deleted. This is an observed availability limitation, not a passing approval test.
Command/file decisions and native MCP elicitation have independent successful
acceptance checks. The stable MCP tool can still request user input, independently
of the disabled default-mode model question tool.

## Verification of session approvals

```powershell
.\CentralAgent.exe --check-native-conversation --approvals --session --allow-test-inference
.\CentralAgent.exe --check-native-conversation --approvals --session --graph --allow-test-inference
.\CentralAgent.exe --check-native-conversation --approvals --file-change --session --allow-test-inference
.\CentralAgent.exe --check-native-conversation --approvals --file-change --session --graph --allow-test-inference
```

Each opt-in check consumes one native model prompt. The command is restricted to
an exact print-only expression; file changes are restricted to one exact line in
an owned temporary file. The actual hidden Svelte card opens its disclosure and
selects **Allow for session**. The harness verifies the typed session decision,
owner/ticket, exact native answer/resolution IDs, expected stdout or disk change,
preserved drafts and native history, then deletes only its owned test history.
All four checks passed on 2026-09-07. `--session`, `--decline` and `--cancel` are
mutually exclusive and require `--approvals`.

Session approval is transmitted as native `acceptForSession`, not saved as a
Central Agent permission rule. Codex defines the grant scope and lifetime. This
test does not assert approval reuse after reconnect or for arbitrary later
commands; it never writes an execpolicy or network amendment. No visual tests.

## Verification of an owned-server crash

```powershell
.\CentralAgent.exe --check-native-conversation --crash --allow-test-inference
.\CentralAgent.exe --check-native-conversation --crash --graph --allow-test-inference
```

Each explicit Windows development check consumes two native model prompts in a
disposable workspace. After the first prompt is acknowledged and actually streams,
the harness stores a new unsent draft and terminates only its own live native
server child by exact process identity/handle. It does not issue Stop, manufacture
an EOF event, scan process names or terminate any other Codex instance. Production
transport/host handling must observe the real EOF and clear loaded/active state.

The test explicitly reconnects, clicks the actual Load history and Resume
connection controls in a hidden WebView, and requires the original thread, turn,
input and directory before sending a second prompt through the real composer.
Final native readback must retain exactly two inputs and a successful continuation,
with the draft and graph sibling unchanged and no replay, queue or copied
other-provider transcript. Both checks passed on Codex 0.153.4 on 2026-09-07;
their exact owned histories were deleted. Cleanup can establish a fresh native
connection if a test fails while disconnected; it never lists personal history.

Add `--lost-ack` to either command to delay the real successful `turn/start`
worker reply before the production host applies it. The model still streams
through the actual server. The test kills that owned server, requires the
uncertain receipt on disk and a visible warning, reconnects, and delivers the
exact old-generation reply. It must not clear the receipt or the newer draft.
Only the real Check native history control may reconcile the receipt through
native `clientId` evidence; if that evidence is absent, explicit history-review
dismissal is required. That dismissal is also guarded in Rust: the current
connection must have successfully observed this exact owner's native history.
Another card's read or a rejected foreign response cannot unlock it.

Both delayed-host-ACK checks passed on 2026-09-07, with same-thread continuation,
no automatic input replay and deletion of their exact test histories. This
exercises the worker/UI delivery race, not lost bytes on the stdio wire. Full
desktop-process restart, provider-switching and other crash timings remain
separate gates. No screenshots or visual testing are performed.

## Verification of native selected files

The opt-in actual desktop-host test sends one UTF-8 note and one image through
the production composer, local snapshot loader and native `turn/start`:

```powershell
.\CentralAgent.exe --check-native-conversation --files --allow-test-inference
.\CentralAgent.exe --check-native-conversation --files --graph --allow-test-inference
.\CentralAgent.exe --check-native-conversation --files --files-png --allow-test-inference
.\CentralAgent.exe --check-native-conversation --files --files-png --graph --allow-test-inference
```

Each command consumes signed-in Codex usage for one multimodal prompt. Only
new disposable fixture paths are supplied at the Windows picker's result boundary;
the OS picker itself is not automated. Hidden WebView DOM checks require the
selected JPEG or PNG preview to decode. The note token and image color are generated independently
of the prompt, then both source files are changed on disk: actual native input,
model recognition and history readback must still contain the selected snapshots.
A newer file and unsent draft are added before the production acceptance handler;
only sent attachment IDs may be consumed. The graph test also preserves a sibling
draft and requires no sibling/main native conversation. Both real tests passed
on 2026-09-07 and deleted their exact owned histories via native APIs.

`--files-png` selects PNG; without it the fixture remains JPEG. Both formats'
initial sends have passed in main and graph. The harness now also offers:

```powershell
.\CentralAgent.exe --check-native-conversation --files --files-png --files-steer --allow-test-inference
.\CentralAgent.exe --check-native-conversation --files --files-png --files-queue --allow-test-inference
```

Add `--graph` to use an actual graph card with an untouched sibling. Queue and
Steer are mutually exclusive and require `--files`. Each uses two user prompts:
a text-only streaming warmup, then the selected note/image follow-up submitted
through Enter and the actual **Send now** or **Queue** button. Steering must send
the observed `expectedTurnId`, receive that same native turn ID, and never start
a second turn. Queue must be observed held behind the active turn before its
single dequeue/start. A newer unsent file/draft is installed while queued or
before native acceptance. Exact input bytes, model recognition, completed native
history, newer files/drafts and main/graph ownership must all pass, with no tools.
PNG steering and queue passed on both hosts on 2026-09-07; all six PNG test
histories (initial send, steer and queue on each host) were deleted natively.

These are not visual tests and do not establish OS picker interaction or
generated-artifact output acceptance. The test image codec now explicitly enables
PNG as well as JPEG; production selected images still use frozen original bytes.

## Verification of concurrent graph input

The real desktop-host acceptance harness can exercise native concurrent input
inside the graph as well as the main chat:

```powershell
.\CentralAgent.exe --check-native-conversation --delivery --graph --allow-test-inference
```

This is an explicit development test, not ordinary startup: it consumes signed-in
Codex usage for three text turns and a steering input, uses only disposable local
directories/history, and deletes its exact native test history afterward. Hidden
WebViews exercise real Svelte/IPC controls without screenshots or visual review.
Acceptance covers same-turn steering, a held queue delivered once, Stop, retained
drafts in both cards, original-directory history readback and explicit reconnect/
resume without replay. It does not imply crash, media or approval coverage.

## Network-policy diagnostic (not a passing acceptance gate)

This separate opt-in diagnostic belongs to native approval verification:

```powershell
cargo test --locked -p central-agent-codex-runtime native_network_policy_denial_uses_only_the_reserved_invalid_target -- --ignored --nocapture
```

It uses an isolated read-only-derived network profile, a local model endpoint,
a fixed `central-agent-network-denial.invalid` destination and a loopback sentinel
that must not receive any fallback request. It may allow once only the exact fixed proxy-request command after
checking its complete native argv and command action; it never grants a shell
rule or a network allow amendment. The denial must come from a real native
network callback, not a fabricated client request. On the recorded Windows
0.153.4 run, the command reached the local target with HTTP 200 without that
callback. A follow-up with the reserved `.invalid` hostname instead failed with a
transport connection-aborted error, also without a native network callback.
Both diagnostics **fail**, intentionally leaving network-policy acceptance open.
DNS/transport failure is not proof of a user-approved native denial. The test now
requires the resolution of the exact network request ID on the original thread,
not the resolution of its prerequisite command approval. Its ordinary safety-oracle tests pass; these do not certify
actual network enforcement. No Windows setup or full-access fallback is used.
See the completion audit for exact observations. Network access and proxy
filtering are distinct native settings in the [official permissions guide](https://learn.chatgpt.com/docs/permissions).

## Native conversation goals

Choose **Goal…** in the native conversation controls, or type `/goal` in the
main/graph composer. A local native thread must already be loaded; **Resume
connection** is explicit and no dummy prompt is sent. Opening the dialog reads
native state only. New objectives are confirmed and created **paused**, with no
token budget by default. Use **Set active…** separately. Status, optional budget,
objective replacement and removal each require an owner/thread/project/view-bound
confirmation. Live usage comes from Codex, not client estimation or a local loop.

The goal applies to the loaded native session. Pending composer model, directory
or access selections apply at the next accepted prompt, not when editing a goal.
Pausing, completing or removing the goal is not the same as interrupting a turn:
use **Stop** separately. Goal removal is neither history deletion nor file restore.
Codex account and runtime limits remain applicable even with no goal budget.

The client uses official `thread/goal/get`, `/set`, `/clear` and native updated/
cleared notifications. A fresh read precedes each write; this API has no atomic
compare-and-swap across clients. Stale confirmations and malformed responses are
rejected, newer notifications survive delayed replies, and uncertain writes are
never replayed. Closing the dialog does not cancel a sent edit. No custom goal
scheduler, history injection, configuration mutation or filesystem checkpoint is
introduced. The [official App Server guide](https://learn.chatgpt.com/docs/app-server)
describes native goal state and objective-replacement accounting.

Actual selected-runtime paused-state probe (no inference, opt-in disposable data):

```powershell
cargo run --locked -p central-agent-codex-runtime --example goal_scope_probe -- --allow-disposable-state
```

It uses the production Rust call constructors and decoders, checks create/get,
explicit budget removal via `null`, and both `cleared: true` and `cleared: false`
acknowledgements. It never activates a goal or starts a model turn, and deletes
only its exact newly created native thread.

Actual native activation probe (isolated profile, loopback fixed-response model):

```powershell
cargo test --locked -p central-agent-codex-runtime native_goal_activation_and_paused_restart_use_server_execution -- --ignored --nocapture
```

On 0.153.4, activating the goal started a native turn **without a client
`turn/start` call**. Pausing did not interrupt that already running turn; it
completed normally. The probe verifies goal update/clear notifications, native
turn history, blocked/paused/complete transitions, isolation from an unused
sibling and exact paused-goal persistence across a server restart. A bounded
500 ms post-resume observation found no new turn while paused; this is evidence
for the tested runtime, not a universal timing guarantee.

Only the local test Responses endpoint is used, with a temporary `CODEX_HOME`
and no account/API credentials. The client adds no continuation prompt or
scheduler. This is not a test of real model reasoning, tool execution or the
desktop Goal buttons.

The corresponding actual desktop-host acceptance is now available as:

```powershell
CentralAgent.exe --check-native-settings --native-goal
CentralAgent.exe --check-native-settings --native-goal --graph
```

These opt-in modes launch a hidden WebView host against an owned temporary
Codex profile and a loopback fixed-response endpoint. They click the compiled
Goal controls to create paused, confirm activation, observe active chat and Stop,
confirm pause, wait for native completion, remove the goal and explicitly read
native history. Original turn IDs, output, drafts, graph siblings and empty
workspace contents are checked. No client prompt, tool call, personal credential
or custom scheduler is involved. The endpoint's five-second response delay is
test-only, to observe the active state; it is not an agent timeout.
Both main and graph passed in Debug and in the 2026-09-07 21:29:59 published
preview; exact artifact identity and release logs are recorded in the plan.

## Conversation MCP inventory

Main/graph Codex controls expose **Conversation MCP tools**. Resume the native
conversation explicitly if needed, then choose Refresh servers. This calls the
official `mcpServerStatus/list` with its bound `threadId`, following every page.
The Settings inventory is a different scope; it is never used as a fallback.
Tools and resources are metadata only: listing sends no prompt, executes no tool
and reads no resource contents. Shared configuration reload stays in Settings;
service OAuth can also be started explicitly in its conversation scope.
The conversation view does not install custom Central
Agent tools or grant permissions. Disconnect, thread/server lifecycle changes,
reload or authentication changes mark the last observed list stale; refresh is
explicit and old responses cannot populate another owner/thread.

The [official App Server guide](https://learn.chatgpt.com/docs/app-server)
documents MCP status listing; the selected 0.153.4 generated parameters include
`threadId`, and its actual runtime passed this isolated, no-inference probe:

```powershell
cargo test --locked -p central-agent-codex-runtime native_thread_inventory_reports_only_fixture_tools_without_calling_them -- --ignored --nocapture --test-threads=1
```

It requires the compatible native Codex runtime and development Node.js, starts
only an inventory-only local fixture in a temporary child profile, and creates
an ephemeral native thread. It verifies actual scoped tool/resource metadata,
rejects an unknown thread, and checks that no tool or resource read occurred.
No login, personal configuration changes or model inference are performed.
This is not evidence of complete host-UI or production OAuth acceptance.

### Authorize an MCP service in its conversation

Choose **Authorize OAuth…** beside an observed service, then **Open authorization
page** for that exact attempt. The client sends native `mcpServer/oauth/login`
with the bound `threadId` and service name; it does not substitute global login.
Codex performs and stores authorization, which may also apply to other Codex
conversations/clients. This does not grant Central Agent system permissions.
Only the URL origin is displayed; tokens/query parameters stay out of UI and
local conversation storage. Reload/startup changes stale the inventory without
erasing pending authorization. Completion must match both thread and service;
disconnect does not imply cancellation, retry or credential revocation.

Actual 0.153.4 thread-scoped and global OAuth flows passed this explicit integration test:

```powershell
cargo test --locked -p central-agent-codex-runtime native_thread_oauth_completes_in_exact_scope_without_external_login -- --ignored --nocapture --test-threads=1
```

The owned loopback fixture verifies discovery, URL return, PKCE, native
completion scope and auth-status readback separately for thread and global login.
Synthetic credentials are isolated
using the native `mcp_oauth_credentials_store = "file"` setting only in a fresh
temporary child profile. No personal configuration/keyring change, external
account sign-in, model inference, tool execution or browser interaction is used.
The [official configuration reference](https://learn.chatgpt.com/docs/config-file/config-reference)
documents this storage setting. This proves the real native flow, not an external
service's availability or full desktop-host acceptance.

Removed: the provider process/JSON-RPC client, custom agent loop and dynamic tool
bridge, native thread/history/outbox state, model discovery, Codex controls and
slash commands, approval/question/recovery UI, protocol fixtures and generators.
The five non-Codex adapters remain alongside the distinct `codex_app_server` ID;
the retired `codex` ID cannot resume the old integration.

Preserved: Rust/Wry/WebView2 and Svelte UI, projects and local chat presentation,
Explorer/editor, graph, manual browser/SSH/RDP/VNC, Time Machine and
the existing other-provider authentication/streaming/permission adapters.
Provider-neutral catalog/input types were separated into provider_types.rs;
they contain no transport or account/thread lifecycle.

The user's Central Agent chat files, graph agent assignments and UI drafts were
removed from active local storage with a recoverable backup outside the app.
Project contents, SSH profiles, browser website sessions, legacy local data, checkpoints,
provider preferences and native provider credential/history stores were not
deleted. This was a local maintenance operation, not an automatic deletion
migration shipped to other users.

The active implementation plan is [CODEX_APP_SERVER_PLAN.md](CODEX_APP_SERVER_PLAN.md).
The new `central-agent-codex-runtime` crate owns only JSONL transport and a
display-only projection. Its outbound messages are checked against the generated
0.153.4 schema; it does not implement model inference, tools or conversation history.

Do not implement the next client by restoring removed code. Consult the
[official App Server README](https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md)
and use the newly generated versioned contract in `protocol/app-server/0.153.4`.
Experimental APIs remain disabled. Runtime upgrades require contract verification.

## Native tool streaming

### MCP progress notifications

`item/mcpToolCall/progress` is projected as ordered plain-text messages inside
the same main/graph tool activity. These memory-only observations never modify
the native item's `result`, history, status or permission. Completed items ignore
late progress; a new history load does not invent missing progress messages.
The schema-backed client tests and shared presentation test pass. A separate
actual 0.153.4 diagnostic with owned MCP/model fixtures received a progress token
and completed the tool, but emitted no App Server progress notifications. That
diagnostic remains failing/opt-in, not certified support for this runtime path.
Normal native elicitation, all six form/URL decisions and persisted continuation
still pass. No synthetic progress events or extra polling are sent to the model.

### File-change snapshots

The display-only mirror consumes `item/fileChange/patchUpdated` from the selected
0.153.4 contract. Each notification replaces the addressed item's complete
`changes` snapshot, including an empty snapshot; it is not appended to an older
patch and is never executed by Central Agent. Main and graph reuse the same
stable row and existing diff viewer. `item/completed` supplies the final outcome
and supersedes intermediate changes; later patch updates cannot change it.
An older in-flight history read cannot overwrite a more recent patch notification.
The deprecated `item/fileChange/outputDelta` is not used as a replacement: the
selected generated schema explicitly says the runtime no longer emits it.

The three shared patch fixtures are validated against the generated notification
schema and exercised by runtime replacement/isolation/late-event tests and the
shared main/graph presentation test. These deterministic checks do not claim
that a new account-backed native edit was performed.

## New account connection

In AI Settings, **Connect App Server** starts a private official stdio process.
Once a supported ChatGPT account is confirmed, Central Agent remembers this
connection and starts that same connection flow once on the next app launch.
No Connect click or new browser login is needed while native authentication
remains valid. The loading states remain explicit until fresh account, model
catalog and managed-policy reads complete; saved intent is not proof of readiness.

The versioned `app-server-connection.json` contains only `version` and `reconnect`.
Codex retains ownership of cached credentials and refresh, as specified in the
[official authentication guide](https://learn.chatgpt.com/docs/auth) and
[managed App Server account flow](https://learn.chatgpt.com/docs/app-server).
Central Agent does not inspect native auth files or persist tokens, account
metadata, device codes or permission grants. Restart does not resume a thread,
send a prompt, replay uncertain work or restore session access grants.

**Sign out** saves the local opt-out before dispatching native logout. A failed
save blocks that action with an error; a late account reply cannot re-enable
automatic connection. Invalid/unavailable replies and transport failures retain
the preference. An authoritative missing or unsupported account disables it.
Corrupt/oversized preferences and a second instance fail closed, and an older
backup is never used to undo a recorded sign-out. A manual Connect/sign-in can
remember a newly confirmed account again. There is no background retry loop.

For profiles created before this preference existed, validated native bindings
or a saved nonempty Codex model selection allow a one-time startup account check.
A recorded opt-out always wins. A fresh profile remains explicit; an old profile
that only connected without ever saving a native binding/model has no durable
evidence and may need Connect once after upgrade, not another OAuth login.

Resolution order is `CENTRAL_AGENT_CODEX_BIN` (an explicit absolute executable),
`resources/codex/codex.exe` beside CentralAgent.exe, every absolute PATH entry,
then bounded official desktop builds under
`%LOCALAPPDATA%\OpenAI\Codex\bin\<build>\codex.exe`. No executable is downloaded;
no bundled binary is currently distributed by this change. Automatic candidates
are version-probed until one identifies as `codex-cli 0.153.4`; a bad explicit
path or incompatible explicit version produces an actionable error instead of
fallback. Desktop discovery never reads `.codex`, credentials or private state.

After initialize/initialized, the client reads account, all model/list pages and
configRequirements/read. It displays only public account fields and model profile
choices; raw managed configuration remains Rust-side. Each refresh invalidates
earlier reads and publishes a complete model catalog only at the final page.

**Sign in with ChatGPT** and **Use device code** call account/login/start with
the corresponding managed type. **Open sign-in page** explicitly opens a validated
HTTPS OpenAI identity URL in the default browser. A code is displayed when needed.
Cancel and completion clear pending login state. **Sign out** requires confirmation
that only Supervisor's separate native account profile is signed out. It never
deletes project files or native conversations. No credentials or temporary device
codes are saved in Central Agent's session files.

Account readiness enables the text workflow and native access presets. Remaining
history/configuration controls and end-to-end acceptance still require the plan.
There is no hidden fallback to the retired Codex implementation or another provider.

## Native public configuration preferences

### Reported thread settings versus saved defaults

Main and graph **Codex conversation > Reported thread settings** consumes the
stable top-level fields returned by successful thread start/resume/fork calls and
later `thread/settings/updated` notifications. This is an allowlisted read-only
report (model/provider, effort, service tier, summary, personality, directory,
approval reviewer/policy kind and sandbox kind), not composer intent or config
readback. Fields omitted by the initial response remain Not reported until a
settings notification supplies them. Collaboration instructions and raw
permission/configuration objects are dropped before the WebView. These labels do
not grant access or enumerate all allowed paths; native request cards retain
their exact requested scope.

Only an owned loaded thread may update its report. Unknown, closed, archived or
deleted threads cannot create a binding or make stale settings current. After
disconnect/unload or malformed settings, the previous report is explicitly stale.
An ordinary history read does not refresh settings; start/resume/fork response
hydration does. Missing fields are displayed as not reported, not guessed from
saved defaults. Reports are memory-only and never change composer selections,
permissions, native history, drafts or model input.
Generated-schema fixtures and main/graph handler tests cover this path; actual
native hot-reload acceptance is still open. No reload control is implied.

Bound conversation forks reject an archived, deleted, busy or unobserved source
before Central Agent allocates a new project chat or graph card. Native fork
dispatch revalidates the source independently of the UI. Archiving while its
confirmation is open closes that confirmation; a stale submission cannot create
an empty destination. Restore is a separate explicit action, not an automatic
fork prerequisite performed on the user's behalf. The selected 0.153.4 runtime's
archived-fork refusal was previously observed in real history acceptance; the
additional preflight and UI regressions require no model inference.

Conversation OAuth host check (2026-09-07): run
`--check-native-settings --mcp-oauth --conversation-oauth`, adding `--graph` for
the graph card. This extends the global fixture below to one owned ephemeral
native thread and the actual Conversation MCP controls. Login/open/completion
must match its owner and native ID. Native auth readback reports OAuth authorized
and connected; target/sibling drafts are preserved, with no sibling bindings,
model turns, tool calls or resource reads. The harness waits for the service's
native post-login `ready` event before explicitly refreshing: a read crossing
startup can legitimately become stale. Production still requires explicit
refresh and does not replay a login. Only owned synthetic file credentials are
used, and the fixture/profile are removed after child exit. This is not an OS
browser-launch test or proof that an ephemeral thread survives restart. Main
and graph success logs: `%TEMP%/central-native-mcp-oauth-main-ready.log` and
`%TEMP%/central-native-mcp-oauth-graph.log`. No visual inspection or inference.

Global OAuth host check (2026-09-07): run the acceptance executable with
`--check-native-settings --mcp-oauth`. The compiled Settings buttons initiate
service login and explicitly request opening its authorization page. The opt-in
harness validates the actual production attempt/URL, then replaces only the OS
browser-launch side effect with consent against its owned loopback fixture.
The real App Server performs discovery, registration, PKCE exchange and emits
the correctly scoped completion. An explicit Settings Reconnect followed by
Refresh servers verifies native credential persistence without another login
or token exchange. The unsent draft survives; sensitive query parameters and
synthetic credentials never enter the DOM. The fixture uses only an isolated
file credential store, not personal login or Windows keyring, and is removed
after completion. No model, tool call, resource read or conversation is created.
This covers global Settings, not the OS default-browser launcher or per-thread
main/graph OAuth. Success log: `%TEMP%/central-native-mcp-oauth-reconnect.log`.

MCP Settings host verification (2026-09-07): the no-inference acceptance command
`--check-native-settings --mcp-settings` uses a freshly owned native profile and
an inventory-only local Node fixture. It drives the actual global Settings UI
through disabled creation, enable, separate reload, disable and removal,
including cancelled confirmations and readback of native persistence. The
fixture reports one tool and one resource; neither is invoked/read. Exactly
four writes and one reload are observed, with draft/private configuration
preservation and no conversation creation. The two MCP confirmation dialogs
now ignore a delayed close event while a newer confirmation is open. The
canonical success log is `%TEMP%/central-native-mcp-settings-host-closefix.log`.
This is shared Settings coverage, not thread-scoped OAuth or all option editors.
Node is required only for this test fixture, not the production app.

Actual desktop acceptance (2026-09-07): `--check-native-settings [--graph]` now
verifies compiled main and graph dialogs against the real selected App Server
using only an owned, unauthenticated temporary Codex profile. It covers native
preference save/clear and skill disable/re-enable, cancellation before writes,
confirmation, refresh, native readback, next-prompt skill selection/removal,
unchanged draft and absence of unintended conversations/inference. Synthetic
private environment values and skill instructions are absent from the rendered
DOM; unrelated private configuration survives preference writes. No screenshots
or visual tests are taken. The isolated profile is removed after child exit.

Run the acceptance executable with `--check-native-settings` for the main chat
or add `--graph` for the graph owner. No login or inference opt-in is required;
the coordinator supplies an isolated CODEX_HOME and removes API credential
environment variables for its child. Do not manually point this check at a
personal profile. Full successful logs are
`%TEMP%/central-native-settings-main-complete.log` and
`%TEMP%/central-native-settings-graph-complete.log`. Each full check verifies
exactly two confirmed preference writes and two confirmed skill writes, not
every configuration key or live reload. The published preview is unchanged by
this test-only increment; see CODEX_APP_SERVER_PLAN.md for the release record.

Verification (2026-09-07): full workspace passed with 489 Rust tests, 2 ignored,
zero Svelte errors/warnings, six preference component-handler tests, 43 outbound
schema calls and three config response fixtures. Clippy and scoped audit passed
(13 allowed dependency warnings). No visual inspection was performed.

**Codex defaults…** in the main/graph native controls uses `config/read` with
the explicit local project and `includeLayers: true`. Rust projects only ten
public preferences: model/review model, effort, reasoning summary, verbosity,
service tier, web search, context override, compaction threshold and its counting
scope (`model_auto_compact_token_limit_scope`: `total` or `body_after_prefix`).
The scope is native threshold accounting, not a client-side context counter:
`total` includes all active context; `body_after_prefix` includes growth after
the carried compaction-window prefix. Neither increases model capacity. It shows
effective values separately from base user-file values and native origin/layer
metadata. Private configuration, environment values, instructions and credentials
are not exposed to the WebView. Large integer preferences remain exact decimal
strings across IPC, not rounded JavaScript numbers.

An enabled base `user` layer with `profile: null`, an absolute file path and an
opaque native version is the only edit target. Missing/disabled/ambiguous layers
do not fall back to a guessed path or unversioned write. A confirmed immutable
owner/project/view/key/value uses native `config/batchWrite`, one `upsert` edit,
`filePath`, `expectedVersion` and `reloadUserConfig: false`. Codex owns TOML
mutation, revision conflicts and managed policy; Central Agent never reimplements
configuration precedence. The result distinguishes saved from `okOverridden`.
Refresh observes effective on-disk project values, not live thread settings.
Explicit composer/turn selections can override the newly saved defaults.
The production turn constructor does not force a reasoning-summary value:
it inherits the native session's summary. An unconditional `summary:"auto"`
override was removed after a native loopback acceptance test exposed the conflict.
This does not make saved defaults live: the selected runtime's tested
`reloadUserConfig:true` path retained the old summary on loaded sessions even
after the file write succeeded. Native summary reload remains unverified; see
the reproducible test and evidence in CODEX_APP_SERVER_PLAN.md. No custom engine
or implicit thread mutation is used to disguise that result.

Config and skill writes are mutually exclusive and blocked during active/queued
native work or sandbox setup. In-flight reads and confirmations are invalidated
by shared changes, owner/directory changes or disconnect. Closing the dialog
does not cancel, forget or replay an already-sent write. Unknown results and
conflicts require explicit refresh; no retry is sent automatically.

`cargo run --locked -p central-agent-codex-runtime --example config_probe`
passed against the selected real 0.153.4 runtime: nine public preferences, two
layer descriptors and a versioned user target. It prints counts only and makes
no inference call, config write or native conversation. Write cases are checked
with generated schemas and deterministic tests, not personal config mutation.
Advanced native configuration, live config reload and managed named-permission
profiles still require integration. Clearing overrides is implemented below.
See the latest preview build record in CODEX_APP_SERVER_PLAN.md for executable
coverage; source verification alone does not update the executable.

### Removing a saved native preference

Clear saved value in the preference's source disclosure confirms removal of
exactly one observed base-user override. It uses the same native
config/batchWrite target, expectedVersion and reloadUserConfig:false policy as
saving, with value:null and mergeStrategy:upsert. Empty editor text is rejected,
not interpreted as deletion. Missing user overrides, stale views, changed roots,
disabled/ambiguous targets and concurrent writes cannot authorize a clear.
The runtime owns resulting defaults and project/managed layering; the UI never
substitutes a guessed value or resets existing conversations. Unknown delivery
requires Refresh, not replay. Closing does not cancel a write already sent.

The guide documents config writes but not null deletion semantics. A real,
no-inference Codex 0.153.4 probe verified this behavior using only a newly created
temporary child profile: it removed model_verbosity, preserved
model_reasoning_effort, and verified the subsequent native config/read plus the
temporary file. No parent environment, personal config, account or native history
was modified. The probe exercises the production Snapshot.clear call and remains
explicitly ignored in ordinary tests because it requires a real selected CLI:

`cargo test --locked -p central-agent-codex-runtime native_config_null_removes_only_the_selected_fixture_override -- --ignored --nocapture --test-threads=1`

## Native skill inventory and enablement

Verification (2026-09-07): complete workspace passed with 478 Rust tests, 2 ignored,
zero Svelte errors/warnings, 41 outbound schema calls, three skill response/event
fixtures, five skill component-handler tests, clippy and scoped audit (13 allowed
warnings). These controls are in source; the preceding integration preview has
not been rebuilt with them. No visual tests or personal skill writes were performed.

**Codex skills…** in the main/graph native profile uses official `skills/list`
with the explicit local working directory and `forceReload: true`. It displays
name, description, scope, path, enabled state, owning plugin ID and discovery
issues. Central Agent does not read or expand SKILL.md, fetch icons or install a
parallel skill catalog. Missing metadata and malformed/foreign-directory results
are errors, not an empty successful catalog.

Enable/disable requires confirmation against the currently observed path,
inventory ID and original project. Official `skills/config/write` owns the actual
write; no user-global TOML is edited by this client. Its scope is shared Codex
configuration, potentially used by other chats and clients, not a chat-local
setting. The confirmation discloses this. Native `effectiveEnabled` remains
authoritative even when it differs from the request. There is no compare-and-swap
version on this particular API; changes from other clients are not locked out.

Active/queued Codex work and sandbox setup block the configuration change;
pending writes block new native submissions and preserve queued snapshots.
Closing the dialog does not cancel or replay a submitted write. Unknown delivery
requires reconnect/refresh, never automatic retry. `skills/changed` invalidates
all observed inventories and pending old reads. Refresh is explicit and all UI
choices are revalidated for owner, directory and inventory identity.

`cargo run --locked -p central-agent-codex-runtime --example skills_probe` passed
against real 0.153.4 on 2026-09-07 (18 entries, no discovery issues). It lists
metadata for the current directory without printing names/content/paths, creating
a conversation, performing inference or changing configuration. Native write
success/override/rejection/disconnect are exercised with deterministic fixtures,
not by changing personal Codex skills.

**Use with next prompt** selects an enabled native inventory entry without
running a model or changing shared configuration. Removable references remain
visible near native controls in main and graph conversations. The next explicit
prompt, Queue or Send now captures their exact IDs, names, paths and local
directory. The native input includes a `$name` text item and the recommended
`UserInput::skill {name,path}`. Codex loads instructions; Central Agent never
reads SKILL.md, expands dependencies or executes a local skill engine. A typed
`$name` without selection remains ordinary user text for native resolution.

Selections survive closing the inventory and remain isolated across owners.
Draft-to-chat promotion moves only the original draft selection. Queue admission
or native acceptance consumes exact selected IDs, so reselecting the same path
while a prompt is in flight is preserved. Rejected/unknown delivery retains the
selection. Changed skills/disconnect invalidate unsubmitted references until
explicit refresh or removal; refresh revalidates enabled name/path identities.
Frozen queued references do not follow subsequent UI selection changes; Codex
owns the actual skill content loaded when that queued input executes.
Selection currently accompanies a text/file prompt, not an empty implicit task.
No actual skill inference was performed during these automated checks.
Verification for explicit selection (2026-09-07): 493 Rust tests passed, 2 ignored,
eight skill component-handler tests, generated turn/start and turn/steer skill
input contracts, frontend build/check, clippy and scoped audit (13 allowed
warnings). The source and static bundle include selection; the older preview
Release does not yet include it.

## Native MCP inventory behavior

AI Settings → Codex → **Codex MCP servers** uses `mcpServerStatus/list` with
`detail: "full"`, follows native cursors and publishes only the complete result.
Loading is explicit and can initialize configured servers, but does not send a
model prompt, call tools or read resource contents. Names, descriptions, counts,
auth status and reported runtime state are displayed. A global configured-server
list is not a selected thread's actual tool set; null runtime state stays unknown.
Thread-specific startup notifications are not applied to the global inventory.

**Reload configuration…** confirms against the observed inventory and calls
`config/mcpServer/reload`. It rereads native disk configuration; Central Agent
does not edit it. The acknowledgement means refresh is queued for loaded threads,
not that every server is connected. Reload waits for active Codex work/setup to
finish. Listing afterward obtains a new observation; a failed/partial response
retains the previous list as stale, with mutations disabled until refresh.

**Authorize OAuth…** calls `mcpServer/oauth/login` for an observed configured
server. **Open authorization page** is a separate user action; only HTTPS or
loopback HTTP without embedded credentials is accepted. The URL remains in Rust
memory and only its origin crosses to the UI. Native
`mcpServer/oauthLogin/completed` is matched to the pending global server, including
completion before the response. Another thread's same-name notification cannot
complete this flow. There is no client timeout, automatic retry or invented OAuth
cancel operation. Disconnect discards pending UI requests and URLs; it does not
revoke authorization. Configured tokens, tool schemas, resource URIs/content and
arbitrary server metadata are not serialized into this settings view.

This native inventory does not reconnect Central Agent's custom
SSH/browser/desktop-control tool bridge or Time Machine. A native configuration
editor remains pending. Thread-specific inspection and OAuth are implemented
in the conversation controls described above.

Earlier foundation verification: 35 focused runtime tests, 11 Rust account/controller tests, 2 UI account tests,
3 UI native-request tests, 27 schema-validated call samples, 19 schema-validated
native request/decision samples, and a real read-only handshake/account/catalog/
requirements check. No login/logout or model inference was exercised on the user's
account, and no visual tests were performed.

## Native conversations: initial UI integration

### Browse, link and fork native history

Native lifecycle acceptance (2026-09-07) passed on the actual hidden main and
graph WebView hosts: one seed prompt per host, then the existing Rename,
Archive, Restore conversation, Fork into a new conversation and Delete buttons
and confirmation dialogs. Independent native readback verified exact fork input
and directory; the original unsent draft survived all operations. Runtime
0.153.4 refuses deleting a source when a fork still references its paginated
history. The confirmation now discloses this limit. Rejection keeps the source
available and never auto-deletes a fork or retries. Only the test driver removed
its own new fork explicitly, then used the source's Delete dialog again.

```powershell
.\CentralAgent.exe --check-native-conversation --lifecycle --allow-test-inference
.\CentralAgent.exe --check-native-conversation --lifecycle --graph --allow-test-inference
```

Each command now uses two Codex prompts and deletes only the native histories it
created. The extended check also clicks **Open branch**, waits for that owner's
rendered history, and submits its second prompt through the branch composer.
Readback proves one unchanged source turn versus two fork turns. Main navigation
returns using the actual Projects chat button and restores the source draft;
the original graph card/draft stays open beside the new branch.
Independent native readback/removal checks retain the selected-owner boundary:
main UI controls still reject a non-selected local chat. History browsing/import
remains a separate acceptance gate; no visual test is implied.

The native conversation controls now offer **Browse Codex history…**, including
before a Codex binding exists. Search uses the native case-sensitive title filter;
**Archived only** lists archived history exclusively. **Load more** follows the
server cursor across all source kinds. Browsing does not resume, mutate or submit
anything to a model. Summaries are memory-only; native rollout files are never
opened or parsed by Central Agent.

Create/select a local project chat or local graph agent without a Codex binding,
then choose a result. **Link to this chat…** explicitly reads and binds the same
native ID; history changes in either client affect that same conversation.
**Fork into this chat…** calls native `thread/fork` and binds its new ID. The
confirmation identifies the destination directory and explains that this is
conversation history, not a file/worktree copy or checkpoint. Future prompts use
the destination's selected directory/profile/permissions, without injecting its
other-provider history. Existing bindings cannot be replaced, including deleted
tombstones. Archived imports remain archived until explicitly restored.

No prompt follows import/fork. The destination is reserved while the native RPC
is pending; persistence must succeed before future input. Native callbacks retain
their original owner across navigation. Changed project/view, foreign IDs,
malformed history and observed lifecycle races cannot install a replacement
binding. Disconnect and uncertain fork results are not replayed; browse native
history because a fork may already exist. A pending-fork source reference is
persisted before dispatch and blocks creation of replacement native history after
restart. Recovery links an explicitly selected thread whose native `forkedFromId`
matches the original source; it never replays the request.

From a bound conversation, **Fork into a new conversation…** now creates and
saves a new local project chat or a second agent card on the same graph node,
then sends native `thread/fork`. The source thread and local directory are frozen
in the confirmation. No local messages, other-provider session or permission
consent is copied. **Open branch** switches only when requested, preserving the
source draft/card; new destinations use the shared saved Supervisor Codex preset.
Request-specific native grants are not copied. Saving the local
destination must succeed before the RPC. A failed RPC may leave an empty local
destination; an uncertain one retains its recovery reference, not an automatic
retry. No model prompt or file/worktree copy follows branching.

Automatic-branch verification (2026-09-07): complete workspace verification
passed with 462 Rust tests and 2 ignored, zero Svelte errors/warnings, generated
protocol checks, clippy and scoped audit (13 allowed warnings). Tests cover
restart/uncertain-result recovery, destination isolation and the graph's actual
window-event handler without launching a visual session or modifying personal
native history. End-to-end native acceptance and a new Release remain pending.

Verification of this increment: frontend check/build and deterministic UI tests,
27 native call + 19 native decision contract samples, reset-boundary checks,
and the full workspace script passed with `RUST_TEST_THREADS=1` (426 Rust tests,
2 ignored), including clippy and dependency audit. The parallel suite repeatedly
hit the pre-existing 10-second test-helper deadline in
`process_runtime::tests::captures_output_larger_than_the_old_volume_cutoff`;
serial execution passed without changing any application timeout or process code.
No visual test or real model inference was run for this increment. The focused
App Server host tests also cover sanitized summaries and native-only graph input.

Select Codex after connecting in AI Settings. Main chats and local-directory
graph agents open/resume a native thread, persist its ID, then dispatch only the
user's text and explicitly selected frozen snapshots with the frozen
model/effort/speed and local cwd. The sandbox is initially read-only; no old
permission mode is implicitly inherited. Unsupported remote nodes receive an
explicit error without clearing the draft.

Native messages, commentary, exposed reasoning summaries, command output,
file-change diffs and the final answer use the existing shared timeline. The
projection is never sent back as history. Native timestamps/duration are used
when reported. Switching visible chats does not redirect replies. Graph launch
does not inject a synthetic mission wrapper, linked agents or stored
other-provider history. Existing Codex configuration/AGENTS.md remains native.

Input is cleared only after acknowledgement. Stop cancels an unsent turn during
thread opening or calls turn/interrupt after submission. Local queues retain the
original owner and profile. A delivery receipt is saved before sending; after
disconnect the UI offers native history inspection and explicit warning dismissal,
never automatic resend. Correlation uses userMessage.clientId, not server item ID.
Native active work blocks removal of its project/chat without creating a Time
Machine checkpoint. Native chat lifecycle controls still need full UI integration.

The new client correlates requests to immutable `chat:…`, `graph:…` or `draft:…`
owners. Its binding file, `app-server-threads.json` in the application data
directory, contains official thread/session IDs and unresolved delivery receipts,
not a second model history. Successful native lifecycle responses are atomically
saved before the host announces them. Invalid binding files are preserved and
block native mutations; restore a valid backup and restart to reload them.
An OS-owned file lock protects this store from concurrent Central Agent instances.
The lock file can remain on disk; only an actual held lock prevents access, and
process exit or a crash releases it automatically. Other providers remain usable.

The internal main/graph command bridge accepts typed local conversation actions.
Rust resolves their real local directory and known native binding; the WebView
cannot supply an arbitrary native thread ID or command. Replies and streamed
items remain owner-scoped. Changing visible chats never redirects a reply.

The tested client supports start/resume/read and lifecycle calls, native turn
start/steer/interrupt and display projection. It does not start a model/tool loop.
After uncertain delivery, it queries native history instead of replaying input.
Only an exact native client message ID proves a start was accepted. Stable
`turn/steer` lacks this receipt field, so lost steering acknowledgements require
explicit user review. Existing other-provider transcript storage is untouched.

Named managed permission profiles,
the complete history/lifecycle UI and native MCP controls remain in the plan.
Do not treat passing backend/fixture tests as end-to-end UI readiness.

### Native Send now

Main and graph composers expose Send now after a second submission while a native
Codex turn is active and its prior input is acknowledged. The choice captures that
turn's ID. Rust resolves the original local owner/directory and sends `turn/steer`
with `expectedTurnId`; no model, cwd, sandbox or reconstructed transcript is sent.
The turn retains its native configuration. Queue remains explicit and separate.

Late/stale input is rejected instead of targeting a newer turn. Browser-tab,
terminal-output and supported file snapshots are frozen before steering and
remain owned by the original draft. Matching acknowledgement clears only the
accepted text and exact snapshots in that owner's draft. Rejection keeps the draft;
ambiguous delivery requires explicit history review because stable steering has
no native client-message receipt ID. There is no automatic retry, replacement
turn, or steering timeout. Stop continues to request native interruption.

Deterministic component-handler tests exercise captured turn IDs, no implicit
queue/start fallback, graph ownership, attachment preservation and newer drafts
surviving acknowledgement. They do not mount a browser or replace visual review.

Steering increment verification (2026-09-07): complete workspace verification
passed with `RUST_TEST_THREADS=1` (435 Rust tests, 2 ignored), including 39 native
runtime tests, 22 native host tests, three component-handler tests, generated
contract validation, frontend build, clippy and the existing scoped dependency
audit (13 allowed warnings). A real-pipe simulated server checks two independent
steered owners and item delivery before acknowledgement without extra turns.
No real inference, visual inspection or new Release was performed in this increment.

### Native token usage

`thread/tokenUsage/updated` supplies `turnId` and the native `last`, `total` and
`modelContextWindow` fields. Main and graph use one Svelte display, scoped to the
same native binding. The compact ratio compares `last.totalTokens` with reported
capacity; `total` is shown separately in Token details. Latest report does not
claim a per-token counter or a sum of all model requests in one turn. Native
cached-input, cache-write and reasoning counters are not added to totals again.

Decimal-string UI projection preserves native integer precision. Missing or invalid
counts stay Not reported, and zero remains zero. A null/zero capacity does not
produce a percentage. The bar clamps visually at 100% while text retains the real
ratio if it exceeds capacity. Lower subsequent snapshots, including after native
compaction, are accepted; an older known turn cannot overwrite a newer report.

Disconnect preserves last-known numbers but marks them stale. Reconnection/history
reads cannot make them fresh; another native event is required. Starting another
turn labels the prior report appropriately. Provider/owner changes cannot borrow
another conversation's counters. No new telemetry polling, local persistence,
token estimate, billing calculation, prompt limit or context override is introduced.

Usage increment verification (2026-09-07): the complete workspace script passed
with `RUST_TEST_THREADS=1` (437 Rust tests, 2 ignored), frontend checks/build,
three usage-presentation tests, 29 outbound/19 decision contract samples and three
incoming usage fixtures validated against the selected runtime's generated v2
schema. Clippy and the scoped dependency audit passed (13 allowed warnings).
No visual test, model inference or new Release was performed.

## Native permissions and Windows sandbox

Increment verification (2026-09-07): the complete workspace script passed with
`RUST_TEST_THREADS=1`: 431 Rust tests passed, 2 ignored; frontend checks/build,
contract validation (29 outbound calls and 19 native decisions), clippy and the
existing scoped dependency audit also passed. No visual inspection or real
sandbox elevation/model inference was performed. This does not complete the
remaining native UI/end-to-end milestones or publish a new Release.

Settings → Agent offers a shared Supervisor preset: Read only (`read-only` / `untrusted`),
Project access (`workspace-write` / `on-request`) and Full access
(`danger-full-access` / `never`). Full access requires an in-product confirmation
describing filesystem/network scope and the absence of command approval prompts.
The project preset keeps network access disabled unless native approvals grant it.
These are native Codex policies, not Central Agent's old permission engine.

Since the user direction on 2026-09-13, this preset applies to all Supervisor
Codex agents, including new main chats, graph agents and native fork destinations.
The full-access confirmation states that global scope and persistence. Supervisor
atomically saves `app-server-permissions.json` in its own data directory before
acknowledging the change and restores it on startup. It never writes the official
Codex app's settings or copies native request/session grants. Missing initial
preferences use read only; unreadable or unsupported files stay preserved with an
explicit read-only fallback, and broader backups are never restored automatically.
The preset is copied into accepted input before queueing; active or queued Codex
work blocks changing it. Public-summary preferences remain owner-scoped.
Saved queued input cannot change the shared preset. Fresh native requirements
are still necessary before execution. Managed sandbox/approval allowlists disable incompatible
presets. Named managed permission profiles are explicitly unsupported by these
presets until the native configuration UI lands; they are never bypassed.

AI Settings exposes `windowsSandbox/setupStart` for elevated and unelevated modes.
Both require an explicit dialog; elevated setup may show Windows UAC. Setup is
asynchronous, waits for both acknowledgement and completion in either order, and
does not grant full access or auto-retry on failure/disconnection. No sandbox
setup, administrator action or model inference was executed while implementing
this increment. App Server remains responsible for enforcing the sandbox and
reporting actual runtime readiness when a tool executes.

## Native conversation lifecycle

Increment verified 2026-09-07: full workspace script passed with
`RUST_TEST_THREADS=1` (442 Rust tests, 2 ignored), three UI handler tests, six
incoming lifecycle schema fixtures, frontend checks/build, clippy and scoped
audit (13 allowed warnings). No real conversation was modified/deleted, no
visual test or model inference was performed, and no new Release was produced.

Open the main/graph profile controls, then **Codex conversation** to load/refresh
the native history, rename it, archive/unarchive it or permanently delete it.
These controls affect the bound Codex thread only, not the local project/chat
metadata or another provider. Native archive/delete may also affect spawned
children as documented in the [official App Server guide](https://learn.chatgpt.com/docs/app-server).
Their confirmations disclose that scope; unarchive restores only one thread.
No operation promises file checkpoint/restore or deletes project files.

Rust validates the original local owner and expected native thread ID before
dispatch. Busy work blocks mutations; observed history is required after
reconnect for rename/archive/delete. Name and lifecycle notifications are
authoritative over older read/resume/operation replies. Closed, archived or
deleted threads expire their pending approval cards without granting anything.
Native deletion clears the in-memory transcript and delivery receipt; the local
binding keeps only a terminal ID marker (not history) across restarts. This
prevents queued input or a late response from recreating a deleted conversation.
Use a new local chat/graph conversation to start again. Existing binding stores
remain readable without a destructive migration. Native list/import and fork
controls are connected; full runtime acceptance remains pending.

## Composer command shortcuts

Unsent text is saved locally for each project chat, graph card and project draft.
It returns after restart without sending a Codex prompt. Saving is indicated;
on a save error, copy the text before closing. Another window's newer saved
revision is never silently overwritten. Deleting a local chat/card or ejecting
its project clears its draft; archiving preserves it. Clearing text persists an
empty revision, so an old browser cache cannot bring it back.

These are plaintext files under the app data directory's `composer-drafts`, not
Codex history, encrypted credential storage or cloud sync. Selected attachments
and approval answers are not persisted by this store. A keystroke not yet
acknowledged as saved is not guaranteed to survive an abrupt process termination.

The opt-in no-inference acceptance command is
`CentralAgent.exe --check-native-settings --draft-persistence` (add `--graph`
for graph cards). It uses an owned isolated profile and three fresh app processes
to verify save, recovery, clearing and recovery again through the real controls.
Graph additionally verifies an independent sibling draft. These are hidden DOM
functional tests, not visual tests or an invocation of the model.

Main and graph Codex composers now provide `/` suggestions with Arrow Up/Down,
Tab completion, Enter and Escape. These are client UI intents mapped to existing
native controls, not a generic App Server slash RPC or a recreated CLI engine.

| Shortcut | Existing native operation/control |
| --- | --- |
| `/resume` | Browse `thread/list`; linking a result still requires confirmation. |
| `/history` | Read the bound native history with `thread/read`. |
| `/compact` | Confirmed native `thread/compact/start`; completion follows native events. |
| `/skills` | Native skill inventory/selection and confirmed configuration controls. |
| `/config` | Native configuration inspection and confirmed versioned preferences. |
| `/rename` | Confirmed `thread/name/set`. |
| `/fork` | Confirmed new local conversation linked to `thread/fork`. |
| `/review` | Existing target/scope confirmation before native `review/start`. |
| `/archive`, `/unarchive` | Existing native lifecycle confirmations. |

No shortcut itself grants permission, copies conversation history as model input,
registers tools or writes configuration. Availability is checked against the
current native owner, connection, binding, directory and active work. Main dialogs
first reveal their existing configuration host. A command is consumed only when
its control acknowledges opening; attachments and skill selections are unchanged.
Submit, Queue and Send now inspect commands before model dispatch. Unsupported
commands and inline arguments are retained with guidance; prefix a space to send
literal slash text. Paths and multiline instructions remain ordinary input.
Other providers are unaffected. This catalog intentionally does not claim all
CLI/TUI commands, native modes or APIs whose UI has not been implemented yet.

### Native context compaction

`/compact` and **Compact context…** require a bound, loaded idle conversation with
native history. After reconnecting, **Resume connection** reopens the same native
ID without sending a prompt. Confirming compaction may consume account usage and
can summarize details available to later turns; it does not reset the chat, modify
project files, create a checkpoint or replay a locally assembled transcript.

The exact native call contains only `threadId`. A metadata-only delivery receipt
is written before dispatch. Its immediate `{}` acknowledgement means accepted,
not completed. The original owner remains busy until native turn/item events
report the result; old turns and other owners cannot resolve the operation.
Stop waits for a reported native turn ID and requests `turn/interrupt`. No local
time limit or automatic retry is introduced. On disconnect/unload or a malformed
acknowledgement, inspect native history before explicitly dismissing uncertainty.
The client does not match a compaction request by prompt text or synthesize a
success event. Definitive native rejection/failure/interruption clears its receipt.

The real lifecycle probe supports `--exercise-compaction` in addition to
`--allow-test-inference`. It passed on 0.153.4 on 2026-09-07: one new test
conversation compacted with its native `contextCompaction` item, while its sibling
and fork histories stayed unchanged. Only the probe's test histories were deleted;
no personal configuration or existing history was changed. This is core/transport
acceptance, not visual or full host-UI acceptance.

## Loaded-session reuse and unused connections

Real lifecycle acceptance (2026-09-07, Codex 0.153.4):

```powershell
cargo run --locked -p central-agent-codex-runtime --example conversation_lifecycle_probe -- --allow-test-inference
```

This opt-in probe uses the production `Client` and `Conversations`, two temporary
directories and synthetic main/graph owners. It creates two native conversations
and an independent branch, consuming account allowance for three tiny read-only
turns. It verifies both turn/start calls are dispatched before awaiting either
acknowledgement, live agent-message deltas, authoritative completed items, distinct
history, native rename/read, fork, archive/unarchive, and deletion/tombstone state.
It then restores only serialized bindings into a fresh private App Server
connection, reads/resumes the same IDs and asks for a marker supplied before the
reconnect. The follow-up contains neither that marker nor a replayed transcript;
the other conversation and branch keep their original histories unchanged.

The probe passed. It deletes only IDs returned by its own create/fork requests,
including on assertion failure, and reports exact test IDs if cleanup fails.
It never reads personal conversations or configuration files, grants tool
permissions, writes the application's binding store, or runs automatically in
the normal test suite. This proves the production core/transport lifecycle on
the real runtime, **not** WebView interaction, queued/steered/permission-gated work,
attachment inference, or concurrent execution timing within the service.

Normal prompts reuse an observed, loaded idle session rather than calling
thread/resume before every turn. The actual next turn/start still carries the
frozen directory, model, effort, service tier and native permission policy.
Active work, pending mutations and unresolved delivery prevent reuse. A native
notLoaded status invalidates it immediately, before thread/closed; an older
resume acknowledgement cannot resurrect that loaded state.

The 0.153.4 runtime can have no rollout to resume for an empty session. After
disconnect, **Reset unused connection…** offers a confirmed local unlink only
for bindings created here with no attempted turn/review or observed native work.
Legacy/imported/forked bindings default conservatively to ineligible. Read replies
with history revoke eligibility; pending reads block reset. No native delete,
replacement prompt, filesystem operation or other-provider history change occurs.
Any existing native history can still be found through the native history browser.

No-inference acceptance probe: `cargo run --locked -p central-agent-codex-runtime
--example loaded_session_probe`. It starts a private App Server process, opens an
ephemeral read-only session and verifies idle/reusable state plus its exact ID in
thread/loaded/list. Disconnect invalidates reuse and explicit local unlink is
tested in memory; the process is shut down. It passed on 0.153.4 on 2026-09-07.
No prompt, personal-history read/delete or persistent local binding was involved.
This does not substitute for full main/graph submitted-turn acceptance.

## Native code review

Windows scope correction and compiled-host verification (2026-09-07): review
preparation now recognizes the same existing absolute directory with or without
the Windows verbatim prefix. It resolves both paths through the filesystem,
rejecting missing/relative/file/foreign paths while keeping all native read-only,
network, approval and reviewer checks. This fixes a false scope mismatch before
`review/start`; it does not add a fallback permission mode.

The actual Debug main/graph hosts have passed
`--check-native-conversation --allow-test-inference --review [--graph]`:
cancel each target dialog without starting review, confirm one self-contained
custom review, verify native streaming/completion/readback and exact rendered
result, retain owner/sibling drafts, and delete only the new native test thread.
This opt-in check uses one seed prompt and one review from the signed-in account;
it rejects tool/permission requests rather than granting them. It performs no
visual tests. Native intermediate worker records are compared with `thread/read`,
not removed to force an assumed number of turns. It does not certify execution of
all review target types, review tool approvals, Stop or detached delivery.

Increment verification (2026-09-07): full workspace script passed with 468 Rust
tests and 2 ignored, zero Svelte errors/warnings, 39 outbound native schema samples,
five review fixtures, clippy and scoped audit (13 allowed warnings). No visual
tests or new Release were performed.

**Review code…** in a bound main/graph conversation starts official `review/start`
with inline delivery. Choose uncommitted changes, a base branch, a commit SHA or
custom review instructions. Native Codex validates the repository/ref; Central
Agent does not construct shell commands, inspect Git or synthesize a review prompt.
The composer draft, selected attachments and other-provider history stay intact.

Review has no per-call cwd/policy overrides. The client therefore first resumes
the native thread with the confirmed local cwd, read-only sandbox, untrusted
approval policy and user approval routing. It verifies the returned scope before
starting review. Managed restrictions remain authoritative; broader or unavailable
scope is an error, not a fallback. Codex's configured reviewer selects its model;
normal composer profile changes are not advertised as reviewer model overrides.
This prepares the native session; subsequent normal prompts reapply their selected
native access policy through turn/start. Review itself uses account allowance.

Stop during preparation cancels the unsent review. After dispatch it interrupts
the native review turn, including an acknowledgement race. A write-ahead receipt
records only owner/thread/request identity, never custom instructions. A lost or
invalid acknowledgement is not replayed; load native history and explicitly
resolve that delivery warning. Exposed entered/exited-review items render as
work activity and safe Markdown final output; turn/plan/updated renders reported
step states with stable disclosure identity.

Current limits: use an existing native conversation with persisted history.
The selected runtime rejects resume of an empty thread without a rollout, including
ephemeral threads. The client does not send a dummy prompt to work around that.
Detached review delivery is not wired: the actual selected runtime rejects it
for its newly created paginated threads (see the compatibility result below).
Explicit fork followed by inline review is available, but is not claimed to be
the same RPC. No custom Time Machine,
automatic restore or file-change attribution is attached to reviews.

Optional real-runtime probe: `cargo run --locked -p central-agent-codex-runtime
--example review_scope_probe -- --allow-test-inference`. It creates a temporary
native conversation, sends one minimal no-tool read-only prompt using the account,
verifies native review preparation, then deletes only its newly created thread.
It does not invoke review/start or inspect existing history. This probe passed
against Codex 0.153.4 on 2026-09-07; the test thread was deleted successfully.
Add `--exercise-review` to additionally invoke production `review/start` inline
on a self-contained Rust snippet, with explicit no-tool instructions. This uses
additional account allowance. Acceptance requires a successful native turn,
completed entered/exited-review items with nonempty text, unchanged earlier turns,
cleared submission receipt, and the final review still present after native
`thread/read`. It does not infer completion from the review ACK, impose an
inference deadline, or read any pre-existing personal history. A failed test
attempts to interrupt only its own observed active turn before deleting only its
newly created thread. Two deterministic acceptance-oracle tests run in normal
workspace verification without inference. Full reviewer/approval and host-UI
acceptance are not implied by those oracle tests.

Real review acceptance (2026-09-07, official 0.153.4): the extended probe passed
using the production reconciliation path. The runtime emitted a worker
`turn/started` ID different from the acknowledged review ID; stored native
history later marked that worker interrupted and the review completed. The
client now requests `thread/read` on completed review items and coalesces a
follow-up on terminal/idle notifications. Only authoritative history updates
the projected statuses; final review text and an idle notification alone never
invent turn completion. A read racing final persistence waits for another native
event, not a timer. Read failures are surfaced, not retried in a loop. Ambiguous
review submission receipts still require explicit reconciliation and never replay.
The real test verified both review items, preserved earlier turns, no remaining
busy state, readback of the final review, and deletion of its own root/descendant
test histories. No tool approval was granted, no personal history was inspected,
and no config file was changed. This does not prove detached delivery or full
host-UI/approval acceptance.

For an interrupted **test harness**, `inspect_review_probe <absolute-temp-cwd>
[native-thread-id]` reads only native metadata/history filtered to that explicitly
selected disposable `.tmp*` workspace. It refuses an ambiguous selection and
prints event types/statuses, not conversation content. The optional
`--delete-completed-probe` flag permanently deletes only the explicitly selected
test root and its native descendants after confirming terminal stored turns;
stop that owned test harness first. It never resumes work or submits inference.

### Detached review: verified runtime limitation

The official [App Server review guide](https://learn.chatgpt.com/docs/app-server)
describes `review/start` with `delivery: "detached"`. The generated 0.153.4
contract accepts that request shape, and the outbound adapter/contract fixture
now cover it. **This is not evidence that the current runtime can execute it.**

On 2026-09-07 the actual runtime created a thread with `historyMode: "paginated"`
and returned JSON-RPC `-32600`, `paginated threads do not support detached review`.
The opt-in compatibility probe verified the precise error, unchanged source
turns, no additional native histories in its exact disposable workspace, and
successful cleanup of the test conversation. Only the initial tiny prompt used
account inference; a separate reviewer was not started. This is a verified
unsupported outcome, **not successful detached-review acceptance**.

Reproduce only on disposable conversations with `cargo run --locked -p
central-agent-codex-runtime --example detached_review_probe --
--allow-test-inference --expect-paginated-rejection`. If the runtime later accepts
the request, the expectation fails and the full detached implementation must be
validated before enabling it. Omit `--expect-paginated-rejection` to require real
detached completion, independent thread identity, both review items, source
history preservation and fork provenance instead.

The nonexperimental generated `ThreadStartParams` does not expose a history-mode
override. Central Agent does not force a legacy store, enable experimental APIs,
edit native history, or disguise fork-plus-inline as detached review. Main/graph
review dialogs explain the limit when the runtime reports paginated history.
Unknown modes remain unknown. For an explicitly reported legacy history, the P1
flow now creates a separate local destination and binds only the exact
`reviewThreadId` from the response; it accepts either response-before-notification
or notification-before-response ordering and never guesses from `thread/started`.
That path is deterministic-test covered but still needs an available real legacy
history for live acceptance. History-mode metadata remains display-only and never
changes the actual thread format or model input.

## Native file inputs

Increment verified 2026-09-07: complete workspace verification passed with
`RUST_TEST_THREADS=1` (445 Rust tests, 2 ignored), frontend checks/build, native
schema checks, clippy and scoped audit (13 allowed warnings). Three new host
tests cover exact selected snapshots, native image serialization and safe history
previews. Contract samples now include inline images in turn/start and steer.
No real user files were sent, no inference or visual test was performed, and no
new Release was generated. Runtime image acceptance remains an end-to-end gate.

The existing + in ready local Codex main/graph conversations selects PNG/JPEG,
MP3/WAV or UTF-8 text/code. These are native `UserInput` text/image/audio entries,
not a generic file-upload endpoint. The client sends a selected text snapshot
with its filename and selected image/audio bytes as an inline data URL. It does
not automatically reference
or reread the original path, import other-provider history, or add custom tools.
Text uses the existing secret-value redaction; protected credential paths, links,
binary/unsupported types, mismatched MP3/WAV signatures and invalid encodings
remain rejected. Client limits are 12 files, 1 MiB per text file, 8 MiB per image
or audio file and 12 MiB for the complete serialized native input. These are
Central Agent safety bounds, not Codex protocol limits.

Snapshots remain fixed through Queue and Send now. Only exact accepted snapshot
IDs are consumed from their original draft. Failure/disconnect retains files;
uncertain delivery is never replayed. Start validates the selected model's image
or explicitly advertised audio capability; native steering keeps the active
profile and its runtime validates input compatibility. Native history previews
use bounded inline PNG/JPEG only,
never automatic URL fetches or localImage filesystem reads. Larger/native-path
images show a preview-unavailable label, not a missing-image claim. Image token
usage and absent sizes are not invented. Audio history exposes only safe metadata,
never its data URL, bytes or local path. Full native image/audio inference and OS
picker acceptance remain pending.

## Native activity detail projection

The display-only `presentation/activity.rs` projects additional fields from the
selected 0.153.4 `ThreadItem` contract into the shared main/graph disclosure:
web query/open-page/find actions; collaboration operation, addressed threads,
requested model/effort and child states/messages; subagent milestones; standalone
function text output; requested sleep duration; image generation status, revised
prompt, saved path and reported usage-limit failure. Completing a collaboration
call does not imply its children completed. All lifecycle updates keep native
thread/turn/item identity; item/completed replaces earlier detail authoritatively.

Only exposed fields become display text. Opaque search result bodies, encrypted
blocks, hook instructions and image/audio bytes or resource URLs are not forwarded
as raw detail. No URL fetch, image-path read or tool execution is caused by this
projection. Image generation/media output currently has an explicit unavailable
preview notice; full artifact presentation is a separate remaining gate. Historical
dynamic-tool output display does not enable experimental dynamic tool registration.
Current guide terminology is not used to rename the installed wire contract:
0.153.4 uses collabAgentToolCall with receiverThreadIds and agentsStates.

Ten item fixtures are validated as both ItemStartedNotification and
ItemCompletedNotification using generated JSON schemas, and host projection tests
cover authoritative replacement, text detail, separate child/call state and opaque
payload exclusion. This is automated contract coverage, not model-inference or
visual acceptance.

## Native model service notices

The client consumes `model/rerouted`, `model/verification` and
`model/safetyBuffering/updated` using the selected runtime's notification schemas.
Typed projections retain only the documented display fields; extra raw fields
are discarded. Notifications belong to their bound thread and original turn,
remain memory-only, and never create phantom active turns, change the composer
profile, grant permissions or trigger retries. Reroutes retain their sequence;
verification and buffering snapshots update one stable notice per turn.

The shared main/graph timeline leaves service notices outside the work disclosure
while keeping final model output distinct. Service text is escaped, without
Markdown links/images. `showBufferingUi: false` suppresses buffering presentation;
an optional faster model is a service suggestion, not an implicit selection.
No account verification URL is invented: this runtime reports verification names,
not navigation targets. Empty verification lists remove the notice without
claiming that an account-wide verification succeeded. Disconnect/unload marks
old observations stale; a history read cannot refresh notification-only state.
The normal connection-generation guard and conversation ownership boundary still
apply. The selected native schema currently exposes `trustedAccessForCyber` and
`highRiskCyberActivity`; these are service reports, not client classifications.

## Desktop-host conversation acceptance

Whole-application restart after a completed turn has its own opt-in check:
`CentralAgent.exe --check-native-conversation --restart --allow-test-inference`
(add `--graph` for two graph cards). A parent owns a fresh temporary profile and
runs two separate app processes sequentially, one minimal native prompt each.
The first invokes normal window-close handling; the second loads the same saved
workspace/chat/graph/native bindings via production startup, then uses real
Load history/Resume controls before continuing. Initial native state must not be
reconstructed locally; permission consent starts read only. Native readback must
retain the original thread/cwd and exactly two turns, without legacy transcript
copies or another card becoming active. Only the parent's exact test histories
are deleted after both processes exit. Child mode validates a temporary-directory
ownership token; it does not accept arbitrary personal data directories.
Both main and graph passed on 2026-09-07. This verifies normal restart after
completion, not process loss with active/uncertain work or cross-exit draft
persistence. New drafts entered after reopening are checked across read/resume.

Add `--restart-active` to either restart command to close the first application
while its acknowledged native response is actually streaming. The first process
checks that the binding is already persisted, then invokes the same normal
window-close handler, without a test-side interrupt or manufactured completion.
The new process must read the original turn/input, preserve a newly entered draft
across history load/resume, and submit exactly one new continuation on the same
native thread. The final readback checks both exact inputs and no duplicate turn.
Main and graph active-close checks passed on 2026-09-07, consuming four prompts
in total; both owned histories were deleted by the parent. This verifies normal
close during streaming, not abrupt application-process loss, uncertain delivery,
or persistence of an unsent draft across process exit.

`--restart-crash` (with `--restart`, optionally `--graph`) tests abrupt application
loss during an acknowledged native stream. It implies active mode. Fixture
project/chat metadata is saved before inference, not flushed before the crash.
The child reports readiness through an owned temporary token/proof; the parent
validates its exact spawned child ID and observes the native descendant through
a synchronize-only Windows handle before killing the application via its own
`std::process::Child` handle. No PID/name-based termination, normal close handler,
test-side native interrupt, transcript rewrite or manufactured ACK is used.
Production kill-on-close job ownership must terminate the native server too.

The second real app process must acquire the released storage lock, load the
original binding, read/resume natively, retain its new draft through inspection,
and complete one new prompt on the original thread. Main and graph passed on
2026-09-07 with four announced prompts total. Both exact owned histories and
temporary profiles were removed after native cleanup. This does not cover
unacknowledged input at crash time or unsent drafts persisted across process exit.

Provider changes have a separate opt-in check:
`CentralAgent.exe --check-native-conversation --providers --allow-test-inference`
(add `--graph` for the graph-card host). Each run consumes two minimal native
Codex prompts. It uses actual provider UI handlers for Codex → Claude → Codex,
preserves an unsent draft, verifies native thread/cwd continuity and exact prompt
input, and checks separate persisted legacy history. Claude catalog/history are
local fixtures, not a real Claude session or model request. Graph also checks a
second card remains untouched. Both checks passed on 2026-09-07; only their owned
native histories were deleted. They are functional hidden-WebView checks, not
visual tests, and do not establish whole-app restart or Claude runtime acceptance.

The optional `CentralAgent.exe --check-native-conversation --allow-test-inference`
command exercises the compiled main-chat composer, real scoped IPC, production
Rust dispatch, official App Server and rendered timeline. It is deliberately not
part of normal build/startup verification: it sends two tiny read-only prompts
using the signed-in Codex subscription. It creates a disposable application store,
WebView profile and empty workspace, without showing a window, starting OS control
hooks, browsing a website or opening personal projects. Native credentials remain
owned by the runtime; the test neither reads credential files nor changes config.

Acceptance requires native streaming, the same thread for both prompts, native
completion, matching assistant rows (not merely echoed user text), acknowledged
draft clearing, native history readback and no copied other-provider transcript.
Cleanup deletes only the native ID created by this test. A cleanup failure fails
the check. The harness deadline is not an agent command timeout. This check does
not certify graph submission, Queue/Steer, approvals, reconnection or media.

Adding `--graph` selects a separate two-card acceptance workflow, with three
tiny no-tool prompts: concurrent first turns in separate temporary directories,
then continuation only in A while B retains an unsent draft. It requires distinct
stable native IDs, observed overlapping turns, assistant DOM rows in the correct
card, both native history/cwd readbacks and cleanup of both owned histories.
This optional workflow does not certify Queue/Steer, approvals or reconnection.
Its fixture uses mapped directory entities, not generic knowledge nodes.

Real graph acceptance passed on 2026-09-07 against official Codex 0.153.4, including
all three prompts, observed concurrency, isolated history/directory readback and
successful deletion of both test threads. A preceding actual run exposed empty
other-provider session creation during graph assignment (not transcript copying).
Native assignment now preserves that separate store instead of creating or
relabeling its sessions. These inference probes consumed subscription usage;
only disposable test histories were deleted. No visual test was performed.

The graph host's timeline invalidation includes native messages as well as
legacy messages. A text delta or authoritative completion must update its stable
row even when runtime phase/status remain unchanged; unrelated decision metadata
does not invalidate the log. A deterministic test executes the actual host
signature to protect this contract. It is not visual or live-runtime evidence.

The pristine startup check must assert that `renderAgentPanelState` exists before
any fixture clicks. Testing after opening a profile picker previously masked an
incorrectly nested renderer declaration, leaving the initial real panel unbound.
The renderer is now registered during script evaluation, independently of menus.

`--check-native-conversation --delivery --allow-test-inference` selects the
main-host delivery check (mutually exclusive with `--graph`). It sends three
no-tool turns and one steering input through the actual composer, delivery
choices and Stop button. The first response is deliberately longer so the test
can exercise active-turn controls; it consumes subscription usage. Native
steering must acknowledge the observed turn ID without starting another turn.
Queue must remain held behind that turn and start once after completion, retaining
a newer unsent draft through dequeue. A separate streaming turn is interrupted;
native readback must retain its interrupted state and the unsent draft. No fake
notifications, manual queue draining, transcript replay or broader permissions
are used. The test then explicitly reconnects the private runtime, clicks the
real Load history and Resume connection controls, and requires the same native
thread, unchanged three-turn history, retained draft and no additional turn/start.
Only its disposable native thread is deleted during cleanup.

Real delivery acceptance passed on 2026-09-07 against official Codex 0.153.4:
native steering ACK for the first turn, held queue admitted once, a newer draft
preserved during dequeue, streamed third turn interrupted through Stop, native
readback, and explicit reconnect/read/resume without replay. Cleanup succeeded.
This proves the main-host read-only text workflow, not graph delivery controls,
approval decisions, file/image input or process crashes during uncertain delivery.
Those have separate acceptance gates. The test consumed subscription usage but
did not read personal history, change shared config, edit projects or run tools.

## Native decisions

`requests.rs` and the shared Svelte request cards handle official command/file
approvals, exact requested permission grants, questions and stable MCP elicitation.
The UI cannot send arbitrary RPC results, native IDs, policy strings or filesystem
grants. An opaque pending ticket identifies the original owner and connection.
Double clicks and resolved/expired tickets cannot approve a new request.
Completion/disconnect clears related cards without replay; a standalone MCP
request without a turn ID waits for its own resolution. No app timer auto-approves.

Cards show command and directory, proposed file diffs when supplied, permission
scope, requesting MCP server and explicit decisions. Extended session or policy
choices are disclosed separately. MCP HTTP(S) authorization links require a click
and cannot use local-executable schemes; opening one is not acceptance.
Managed-network callbacks show their native host and protocol with a network-specific
prompt, not a general shell-command approval. Native `kind: "writeStdin"` callbacks
identify input to an already running process. Any reported command remains in a
context disclosure in these two cases. Main and graph share the same presentation
and unchanged native decisions. These fields are present in the selected 0.153.4
schema; newer guide fields such as `availableDecisions` are not assumed supported
by this runtime. See the [official approval guidance](https://learn.chatgpt.com/docs/app-server#approvals).
Secret answers, MCP response content and raw request metadata are never persisted
in the local conversation store. Existing Codex/MCP tools can consume the answers
after the user's explicit send; Central Agent does not control their retention.

Stable primitive MCP forms support booleans, strings, numbers, integers, single
and multiple choices. Experimental openai/form is not enabled. Unsupported forms
offer decline/cancel instead of guessed fields. Native domain-specific string
format checks still apply. The complete user-submitted conversation flow has not
yet reached end-to-end acceptance; passing request tests is not that milestone.

### Actual desktop-host command approval

```powershell
.\outputs\codex-app-server-preview\CentralAgent.exe --check-native-conversation --approvals --allow-test-inference
```

This opt-in mode uses one subscription prompt, a fresh hidden WebView host and
disposable local workspace. It submits an exact core-string print command, checks
its complete preview and cwd before clicking the actual **Allow once** button,
and verifies native resolution, successful output, removed pending card, preserved
unsent draft and native history readback. It rejects extra tool work and deletes
only its newly owned native history. It does not change permissions/configuration,
read existing personal history or perform visual testing. Ordinary builds never
execute this inference test.

The real 0.153.4 flow passed on 2026-09-07. The final fixture uses
`[string]::Concat('NATIVE_APPROVAL_ALLOWED')`: an earlier Console.WriteLine probe
was correctly rejected by Windows ConstrainedLanguage after native approval.
That was a test-command incompatibility, not a reason to bypass the native sandbox.
Other decisions/request types, graph-specific approval and full end-to-end
acceptance are still separate gates.

The subsequent main/graph matrix passed all six ordinary command decisions:
use `--approvals` for Allow once, add `--decline` or `--cancel` for that exact
button, and add `--graph` to exercise the node-card host. For example:

```powershell
.\outputs\codex-app-server-preview\CentralAgent.exe --check-native-conversation --approvals --graph --decline --allow-test-inference
```

Each invocation consumes one subscription prompt. The graph case opens two
disposable cards but submits only one, retaining the sibling's unsent draft.
The native command must be reported as declined for Decline/Cancel; an ordinary
execution error is not a passing rejection. Both its outcome and original cwd
are verified after native history readback, with no request/binding created in
the sibling or main conversation. All six owned test histories were deleted
successfully. Session choices subsequently passed the four main/graph command/file
checks documented above. Policy amendments and in-turn MCP remain separate gates;
dedicated model permission/questions availability is recorded in the native
availability section. Standalone MCP form/URL acceptance is documented separately.
No visual tests were used.

### Actual desktop-host file-change approval

Add `--file-change` to the approval check to request exactly one native apply_patch
replacement in a fresh disposable `native-approval.txt`. Combine it with `--graph`,
`--decline` or `--cancel` as above. Each invocation uses one subscription prompt;
ordinary verification never runs it. Before answering the actual Svelte card,
the host validates the exact file, update kind, two changed lines, optional root
and unchanged disk. It verifies the real approval diff and timeline diff,
native answer/resolution, preserved drafts and sibling isolation. Allow additionally
requires the aggregate native turn diff and the exact changed disk contents.

Allow and Decline passed on both hosts on 2026-09-07. Initial Cancel checks
failed because the test assumed a declined file item; actual 0.153.4 returned
inProgress or failed when the turn was interrupted. The corrected Cancel oracle
requires a successful actual answer, resolved request, an interrupted turn,
unchanged exact file and no extra tool work, then verifies native history readback.
It never accepts a completed file item or treats an ordinary failed turn as cancel.
Both Cancel retries passed; all eight owned test histories were deleted through
the native API. No personal project files/configuration or visual tests were involved.

The shared display projection retains explicit item outcomes and native identities.
An unfinished item in a terminal turn cannot remain running, and a proposed,
declined or failed patch is not labelled as an applied file change. Diffs remain
reviewable without implying Time Machine snapshots or file restoration.

### Native history browser acceptance

`CentralAgent.exe --check-native-history --allow-test-inference` exercises the
actual shared history dialog in a hidden main host; add `--graph` for a graph
card, and `--fork-history` for a fork rather than a link. It requires an existing
native ChatGPT sign-in and uses **two short prompts per invocation** to seed two
owned native histories. The desktop profile/workspaces are disposable. Queries
use a unique UUID title prefix, never an unfiltered personal-history listing.
No credentials are copied/read by the test; only exact returned test IDs are
deleted through the official native API. It is opt-in, not part of normal build
verification. No visual test or inference is performed by the import action.

Shell-only and injected-item preparation did not yield usable persisted history
in this runtime and was removed, not used as a substitute for an authentic
conversation. Preliminary zero-inference attempts left owned temporary fixtures
`central-native-history-VBG59M` and `central-native-history-d6RbRQ` under `%TEMP%`;
they are not claimed removed. Their native test histories were deleted. New
coordinated cleanup reports failures and retries transient Windows sharing or
directory-not-empty errors only for its newly owned disposable desktop profile.

The real host exposed a WebView2 compatibility issue: NavigateToString does not
provide secure-context-only `crypto.randomUUID`. History, Skills, Defaults and
ProjectDiff now share a UUID-v4 helper using `crypto.getRandomValues`, without
changing the trusted origin, IPC authority or using weak randomness.

Codex 0.153.4 also explicitly rejects forking an archived source. Archived rows
keep Link available and explain the separate restore step, but disable Fork;
Rust rejects that unsupported request before writing a pending fork receipt.
There is no automatic unarchive or replay. The check verifies this restriction,
then selects its independently seeded active history to exercise Fork.

Main and graph Link/Fork checks passed on 2026-09-07 with the production Rust
dispatch and compiled Svelte handlers. Search, archive filtering, cancellation,
confirmation, exact native readback and retained drafts were verified. Twelve
short prompts total include the two preliminary archived-Fork failures; all
owned histories were deleted. Opening a fork and continuing it independently
remain covered by the separate completed `--lifecycle` checks.

### Isolated native MCP request acceptance

`CentralAgent.exe --check-native-mcp-requests` (optionally `--graph`) runs a
hidden desktop request-card check against an isolated official App Server profile
and disposable stdio MCP fixture. Node.js is needed for this development fixture
only; it is not a production runtime requirement. No ChatGPT login, model prompt,
external URL opening or personal configuration changes are used. The test's
coordinator owns its temporary directory and the child validates that directory,
ownership token and isolated native home before starting. Only ephemeral native
threads are created, then the owned server is shut down and the temporary profile
is removed. This diagnostic does not expose a generic tool executor to the UI.

The transport-level probe
`cargo test -p central-agent-codex-runtime native_mcp_elicitation_round_trips -- --ignored --nocapture`
passed all six form/URL accept/decline/cancel round trips on official 0.153.4.
It exercises native `mcpServer/tool/call`, the server-initiated request, the same
typed request store used by the desktop, exact owner/key resolution and the
fixture's actual received result. URL acceptance sends null content, which
0.153.4 normalizes to an empty object on the MCP side; decline/cancel remain null.
Boolean false and numeric form values retain their types. No model turn is
created. This transport probe alone does not certify desktop rendering; the
separate host check uses actual Svelte cards and awaits their actual answers.

Both desktop hosts passed all six cases on 2026-09-07 (12 actual card decisions).
Form fields and an unsent composer draft survive an unrelated production render;
the fixture receives the exact string, false boolean and integer, then the card
disappears on its native resolution. Graph requests remain outside sibling/main
bindings. Field defaults are snapshotted once per ticket instead of recomputed
from unrelated request snapshots. Preliminary host runs exposed a harness gate
that incorrectly waited for an idle thread while its MCP decision was pending;
that test gate is fixed without changing production busy/permission behavior.
These are standalone MCP calls, not proof of in-turn questions, permission grants,
MCP service login, or executing user-configured tools. Those gates remain open.
The final main/graph checks also verify coordinator cleanup. Windows sharing
violations receive a bounded retry only on the newly owned temporary test path;
other errors are reported with that retained path, not hidden by TempDir drop.
Earlier development attempts left eight isolated temporary fixture directories.
Manual removal was blocked by the execution policy, so those residues were not
deleted through a substitute mechanism. They contain synthetic test profiles,
not personal Codex credentials. No such residue is claimed removed by later tests.

### Native collaboration modes: current compatibility

The [official guide](https://learn.chatgpt.com/docs/app-server) labels
`collaborationMode/list` experimental. It is absent from the selected 0.153.4
stable ClientRequest union; its TurnStartParams likewise omits `collaborationMode`.
The client therefore does not offer a Plan selector or simulate the mode with
instructions. Enabling experimental API support is not implicit in a normal
connection or model selection. Native user-question cards remain available for
callbacks the runtime actually emits, without claiming that the corresponding
tool is available in every mode.

Run scripts/verify-provider-reset.mjs to check the removal boundary and UI routes.
Run scripts/verify-app-server-contract.mjs for the new client's wire contract.
# Summary selection follow-up (2026-09-08)

The main/graph native profile controls now select public reasoning summaries
per conversation. Automatic sends `turn/start.summary: "auto"`; Concise,
Detailed and Off send the corresponding stable native enum. Inherit native
setting omits the field, retaining the runtime's loaded setting. Selection is
frozen with queued input and reset to Automatic on restart; no global config
write, synthetic commentary or custom reasoning engine is added. Empty native
summary items remain invisible until actual public text arrives. The runtime
and model determine whether any summary is emitted.

For a content-free read-only diagnosis of explicitly linked histories:
`cargo run -p central-agent-codex-runtime --example reasoning_diagnostic -- <absolute Central Agent app-server-threads.json path>`.
The probe prints counts and two public config preferences, not transcripts,
credentials or raw configuration. It never resumes or starts a turn.

## Hidden Windows process tree (2026-09-14)

Supervisor starts its long-lived Codex App Server with a private hidden Windows
console and explicit inherited stdin, stdout and stderr handles. The server is
created suspended, assigned to Supervisor's kill-on-close job, and resumed only
after containment is active. PowerShell, compilers and other console tools then
inherit the same hidden console instead of asking Windows to create a visible
window for every tool invocation. Console-attached test and CLI hosts retain
their existing console.

The launcher builds the Windows command line and Unicode environment without a
shell, restricts inherited handles to the three protocol streams and preserves
the native JSONL transport. Deterministic tests verify hidden-window state in a
real PowerShell parent and child, environment propagation, quoted paths, full
duplex fake-server traffic and shutdown by owned process identity.

## Native browser-tab and terminal-output snapshots — 2026-09-16

**Attach browser tab snapshot** and **Attach terminal output** now work with the
native Codex provider for start, Queue and Send now. Selection is resolved
against the exact main-chat or graph-card owner. Follow-live terminal capture
ends before dispatch; queued requests serialize the frozen bytes and never read
the browser tab or terminal again when their turn eventually starts. A rejected
or uncertain request retains its draft. Acceptance consumes only the matching
capture, so a newer snapshot of the same tab or terminal survives an older
receipt.

The stable App Server input union has no dedicated browser-tab or terminal
snapshot variant. Supervisor therefore projects each already sanitized snapshot
to a versioned native text input marked as untrusted reference data. Page
credentials, URL query/fragment data and detected secrets remain redacted.
Browser screenshots and thumbnail data are excluded; the page's bounded text,
elements, links and image metadata are sent. Terminal output keeps its frozen
screen metadata and remains subject to the aggregate 12 MiB native-input wire
limit. This transport does not grant Codex live browser control, terminal command
access or a new filesystem permission.

The local optimistic row shows both attachments immediately. When native history
arrives in the same Supervisor session, the accepted user item is matched by its
native client message ID and receives the reviewed local page thumbnail. The
thumbnail remains display-only: it is absent from native input/history and the
bounded local overlay is never replayed. After a restart, Supervisor still
recognizes valid versioned payloads, hides those transport parts from visible
prompt prose and rebuilds the structured attachment card, but it does not invent
or reload an unpersisted screenshot. Malformed lookalikes stay visible as
ordinary user text. Native history remains provider-owned and is not rewritten.

Focused native-input, presentation, steering, queue and owner-isolation tests
passed together with the complete workspace verification and staged hidden
startup check. The canonical application was updated in place and reopened in
the physical LocalAppData profile. Six saved bindings completed accepted
automatic history reads at journal sequences 4214, 4216, 4218, 4220, 4222 and
4224. No Computer Use or model inference was used. Executable SHA-256:
`7cc502269349668c9dc9143abd7402441b66574e92bcddb86737a2bab56870fc`.

## Main composer Send, Stop and Resume — 2026-09-16

The main composer has one compact work control. It sends the current draft or
selected attachments, becomes a square Stop control while that conversation owns
active work, and offers Resume only after the latest response is authoritatively
interrupted. A nonempty draft always remains a new Send. Empty Enter never
resumes work; Resume requires its explicit button.

Resume is validated again in Rust against the current owner, provider, delivery
state and native history. It is rejected if work is pending, queued or active, if
the provider changed, if the latest Codex turn is not `interrupted`, if the native
connection is unavailable, or if draft attachments were added. A valid action
starts one continuation turn in the same conversation with a fixed continuation
instruction. Pending Send and Stop states suppress duplicate clicks and clear
only after an authoritative acknowledgement, state transition or scoped error.

Post-send tab thumbnails stay local and are overlaid onto the matching accepted
history row. At most 24 accepted thumbnail overlays are retained per conversation
owner; disconnect clears them. Structured tab and terminal context remains the
only snapshot content sent to Codex.

Focused preview reconciliation, command parsing and main-composer event tests
passed, followed by the complete workspace verification, both supported App
Server schema contracts, Clippy and dependency audit. The generated Svelte bundle
and staged application passed their deterministic checks; no Computer Use or
model inference was used. The canonical package was updated in place. Executable
SHA-256: `87a357d3c53012c9f382e2472a4a2d13c7ada42c0310eb4a00b7a25cb9419fdc`.
