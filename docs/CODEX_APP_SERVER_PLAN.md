# Official Codex App Server integration

Active implementation objective, started 2026-09-06 and status-refreshed
2026-09-09. The previous Codex client was removed. Do not restore it, its custom
action loop, protocol emulation or prompt/history reconstruction. Preserve all
other providers and native UI.

This file is the chronological implementation ledger, not the shortest statement
of current truth. Use `CODEX_APP_SERVER_GAP_ANALYSIS.md` for delivery priority,
`CODEX_APP_SERVER_ACCEPTANCE.md` for verified evidence, and
`CODEX_APP_SERVER_USER_VALIDATION.md` for checks that still require a person.
Older "current" and "latest" labels below are retained only as dated evidence.

## Supervisor rebrand — 2026-09-10

Renamed the product to Supervisor. The native icon, graph launcher,
configuration launcher and active-project indicator share one monochrome
eye-and-S geometry. Old logo assets and tetrahedron animation/piece transfers
were removed; legacy hooks and all configuration/native actions are preserved.
Product titles/copy, Windows icon/version resources, current documentation and
packaging filenames were updated without changing personal stores or client IDs.

Workspace verification passed: 738 ordinary Rust tests (23 ignored), three scope
probes, 181 frontend tests (one existing skip), zero Svelte errors/warnings and
38 scenarios. Final targeted cleanup checks, locked build and packaged hidden
WebViews passed. Browser inspected the affected themes/layouts and interactions;
Windows file metadata and extracted icon agree with the new identity.

Handoff: `outputs/supervisor-preview-2026-09-10/Supervisor.exe`, 34,437,632 bytes,
SHA-256 `cb6a40c5ab16c65ce22e3b9d75bf98749cce6d35f9d0db6197b07bdd634487d3`.
The renamed MCP companion is included. Unsigned local preview, not a signed
release; no new Codex feature, account mutation or F2 acceptance is implied.
Evidence: `outputs/supervisor-brand-audit-2026-09-10/AUDIT.md`.

## Ordered Agent configuration — 2026-09-10

Reorganized the existing main and graph controls into Model & response,
Permissions & summaries, Codex conversation, Tools & integrations and Saved
defaults, using one accessible Svelte section component. Only the profile starts
open; native refreshes preserve each disclosure's state. Conversation actions
are grouped by purpose; Archive/Restore, Reset and Delete live under Manage or
remove history. Native readback, current-chat overrides and shared defaults are
not conflated. All existing IPC messages, owner checks and confirmations remain.

Collapsed groups exempt direct native dialogs and selected-skill evidence.
Actionable lifecycle/access errors remain visible outside their disclosure.
Main configuration receives a temporary readable column minimum; graph profile
controls use shared theme tokens. No runtime feature or backend method was added.

Complete workspace verification passes: 737 ordinary Rust tests (23 ignored),
three scope-probe tests and 181 frontend tests (one existing skip). The rebuilt
release and packaged hidden main/graph WebViews pass, including new group-state,
guard, evidence-visibility and six slash-dialog regressions. Browser checked both
themes/hosts at Full HD, 2K, 4K and narrow layout with synthetic lifecycle states.

Handoff: `outputs/agent-configuration-preview-2026-09-10/`; executable size
34,336,256 bytes, SHA-256
`d55da193e281caa5a716e31ac55f1d812a62a90d5af5365fbc874e927c4f4ab7`.
The MCP companion is unchanged. This is an unsigned, uncommitted local preview;
prior previews and personal app instances were preserved. Evidence and limits:
`outputs/agent-configuration-audit-2026-09-10/AUDIT.md`. No new account action,
external side effect or independent-fork F2 validation was performed.

## Remembered ChatGPT connection — 2026-09-10

Implemented automatic restoration of an already connected ChatGPT account on
app launch. Rust persists only a versioned boolean in
`app-server-connection.json`, under the existing profile lock. The first ready
panel consumes one connection attempt; the official runtime and authoritative
account/catalog/requirements reads establish readiness. No OAuth is opened,
native thread resumed, model prompt sent, permission restored or work replayed.
Known older native bindings/model profiles receive a safe startup account check.

Sign-out persists false before its RPC and blocks late replies from rearming
it. Definitive missing/unsupported authentication disables reconnection;
temporary failures preserve intent without loops. Corrupt preferences and
backups are not silently recovered; persistence failures remain visible.

Verification: 13 new Rust regression tests, account presentation regression,
complete `scripts/verify-workspace.ps1`, locked release build and packaged
hidden-WebView startup all pass. Browser checks cover all five account states
in Light/Dark at Full HD and connected-state layout at 2K/4K. Computer Use
confirmed a real account restored in two separate app processes with a normal
close between them, without Connect/Login clicks, model prompts or logout.
Only a separate Central Agent profile was used; personal app files were not
changed. The live native account was read, not signed out or replaced.

Handoff: `outputs/codex-connection-preview-2026-09-10/`; executable size
34,317,312 bytes, SHA-256
`dbc93fa7bd2d3b825b611ca3b9d1f2b27721ef74cb6b6e0fcc3199a9713009d8`.
MCP companion remains unchanged. Unsigned/uncommitted local preview, not a
release; previous preview preserved. Evidence and limits are recorded in
`outputs/codex-connection-audit-2026-09-10/AUDIT.md`. Existing manual OAuth,
external-service, OS and independent-fork continuation gates are unchanged.

## Chat identity and fork tree — 2026-09-10

The user clarified that "tree" means the project chat tree. The flat chat list
is now a Svelte-owned forest with source-first ordering, nested fork indentation,
expand/collapse and explicit relationship captions. Pinned shortcuts retain a
flat layout but include the relationship in their label/tooltip. Search keeps a
matching fork visible if its parent is filtered out. Archived, missing or
cross-project parents do not hide a child. The current composer identifies its
chat and source and offers navigation only when that source is available.

Rust retains a display-only native child -> parent ID map after the normal
generation, response and ownership checks succeed. This is separate from
pending `forkOrigins`/recovery receipts and never grants mutation/replay
authority. Successful fork acknowledgements record their frozen source; valid
read/resume/import responses can backfill `forkedFromId`. Missing optional
metadata does not erase previously confirmed ancestry. Names, histories, drafts,
provider settings, permissions and existing IPC ownership remain unchanged.

Verification: 724 ordinary Rust tests passed (23 ignored), 180 frontend tests
passed (one existing skip), Svelte check/build with zero errors/warnings,
format/diff checks, locked release build and packaged hidden-WebView startup.
The hidden component probe covers duplicate names, exact local identity,
collapse across updates, opening/menu callbacks and 170/240 px layouts.
Browser inspected the complete production HTML/bundle using synthetic state in
Light/Dark at Full HD, 2K and 4K, plus narrow long names, collapsed branches,
search, archived source and pinned duplicate titles. The draft survived source
navigation and background refresh. Evidence: `outputs/chat-tree-audit-2026-09-10/`.

Local unsigned handoff: `outputs/codex-chat-tree-preview-2026-09-10/`.
No personal app was stopped or restarted and no native model turn/fork was
performed for these checks. F1's earlier real-history acceptance remains valid;
F2's user-operated independent continuation remains pending. Older forks may
need one explicit Refresh history in the new preview; do not infer their source
from their title. See the upgrade note in `CODEX_APP_SERVER_USER_VALIDATION.md`.
P3-A Git/sections/revert remain backend-only; experimental item listing stays off.

## P3-A backend increment — 2026-09-09

The newly authorized scope adds bound/revalidated Git metadata updates, native
backend section CRUD and membership/order, and protected paginated-history
revert with write-ahead receipts, exact-prefix recovery and stale-page barriers.
`thread/items/list` has a constructor/decoder but is **off at the wire boundary**.
No experimental capability or new UI route was enabled. The isolated official
0.153.4 test passes Git/sections, a 3-to-1-turn revert, explicit resume and
receipt recovery from disk without replay; local files stay unchanged.

Final P3-A gate: 719 ordinary Rust tests pass (23 ignored), three scope probes,
171 frontend tests (one skipped), 121 request samples and 10 P3-A incoming
fixtures; strict Clippy and dependency audit pass under the existing advisory
policy. The additional isolated native P3-A probe passes explicitly.

See [P3-A implementation and limits](CODEX_APP_SERVER_P3A.md) and the newest
entry in [acceptance](CODEX_APP_SERVER_ACCEPTANCE.md). The P2 preview below is
unchanged and does not contain this backend increment. Earlier Astra/visual
acceptance is historical P0/P1/P2 evidence, not an audit of new P3-A controls.

## Autonomous closure ledger — 2026-09-09

All work in the previously selected P0/P1/P2 scope that does not require personal audit, account mutation, OS elevation or
an external side effect is complete:

- the four original P2 findings from the final Astra review are fixed: import
  lifetime, terminate recovery, terminal account/feedback copy and settings ACK
  races;
- Windows display-path identity is consistent for preferences, skills and Goal;
- paginated history reconciles an uncertain turn only through its exact native
  `clientId` and persists the result before the success event; graceful shutdown
  permits bounded App Server rollout flush;
- all no-inference compiled hosts pass for settings, MCP config/options,
  drafts, MCP decisions, command policy and Goal in main/graph as applicable;
- 15 of 19 explicit native probes pass. The remaining four are retained as
  honest diagnostics for stable 0.153.4 behavior that is absent or experimental,
  never emulated by Central Agent;
- the complete gate passes 693 ordinary Rust tests, three scope-probe tests and
  171 frontend tests, with 22 Rust tests ignored by the ordinary workspace gate
  (including 19 runtime probes) and one retained-image capture skipped;
- optimized workspace build, hidden-WebView smoke test, unsigned verification,
  690-package inventory and five-record release manifest pass;
- `outputs/codex-p2-source-preview-2026-09-09` is the current local test
  handoff. It is `NotSigned`, built from uncommitted source and explicitly not
  distributable.
- six exact project-local `.codex-*` audit profiles were classified (about
  11.7 MiB total). Their deletion was denied by execution policy and was not
  bypassed through a different shell or API.

Only the checklist items in `CODEX_APP_SERVER_USER_VALIDATION.md`, human license/
source review, a clean reproducible build, Authenticode signing and enterprise
client registration remain release gates.

Security-audit hardening (2026-09-08): initial session authorization is now a
single gesture and rechecks every queued request, leaving destructive and
ungranted-scope actions pending for an individual decision. Arbitrary local,
workspace, provider and SSH shell execution is classified destructive unless it
matches a narrow read-only allowlist; interpreter, alias and PowerShell
subexpression bypass fixtures are covered. App Server JSONL frames and legacy
provider frames/diagnostics are bounded, while managed commands now enforce
runtime, combined-output and lifetime-stdin ceilings. The complete integration
is committed as one source set, and release packaging refuses a dirty or
incomplete Git HEAD through `scripts/verify-release-source.ps1`.

Post-visual-audit hardening (2026-09-09): the source now discovers the tested
official Windows desktop CLI after packaged/PATH candidates; follows the active
project for P2 automatically; seeds reported settings from stable
start/resume/fork responses; coalesces one Apps refresh race; filters large
Apps/MCP/feature inventories; bounds/redacts history fallback previews; normalizes
display-only Windows paths; wraps model descriptions; connects checkbox labels;
and removes visible opaque IDs. Compound Settings focus and native modal ownership
are corrected, including the light close control, disabled context control and
refresh rerenders. Terminal IPC uses the shared `ui_owner` parser.

Final remediation adds metadata-only history linking, `excludeTurns` resume/read/
fork requests and bounded atomic `thread/turns/list` hydration, closing the real
greater-than-64-MiB history disconnect. The JSONL reader now enforces that limit
before unbounded growth; active-writer resume rejection releases pending UI; and
WebView2 shutdown is marshalled to its owning UI thread. Apps failures are
sanitized before presentation, and long MCP confirmation titles fit without
horizontal scroll. The final end-to-end retest additionally makes explicit Apps
refresh bypass cached catalog/runtime snapshots, converts HTTP access denial into
a terminal account/workspace-unavailable state, and clears superseded main/graph
connection-required alerts as soon as reconnect succeeds.

The final gate passes the complete workspace: 693 ordinary Rust tests (22
ignored by the ordinary workspace gate, including 19 runtime probes), three
scope probes, 171 ordinary frontend tests (one retained
image capture skipped), 111 generated 0.153.4 calls, Svelte/TypeScript, 35 design
scenarios, generated bundle, formatting, locked all-target checks, strict Clippy,
theme/inline/release probes and dependency audit. The final controllable desktop
pass covers both themes and all remaining App Server surfaces, including clean
close/relaunch with no WebView2 error. Apps inventory returned an explicit
account/workspace-unavailable state after fresh retrieval, so external App
selection remains unexercised. The apparent protocol fragment in an old response
was traced to the authoritative persisted native agent message, not a Central
Agent item merge; the source text is intentionally preserved. A verified
unsigned source preview is available, but no new signed distributable is claimed.

## Scope update — native App Server only

User decision on 2026-09-06: this phase implements only functionality exposed
natively by Codex App Server and the Central Agent UI needed to use it. Native
MCP support remains in scope; connecting Central Agent's own browser,
SSH, graph coordination or desktop-control tools is deferred. Time Machine
checkpoint/restore integration is deferred too. Preserve these existing features
for other providers; do not delete or rewrite them as part of the Codex client.

Use native `fileChange` and `turn/diff/updated` for Codex change review. Do not
take a parallel Time Machine snapshot, inject a custom context/tool loop, or
advertise file restore from native conversation rewind. The installed protocol
states that `thread/revert` and deprecated `thread/rollback` change conversation
history, not local files. Main/graph conversations can host the same native Codex
UI, without adding custom graph tools to model input.

## Source of truth and architecture

### Current local source-preview handoff — 2026-09-09

The current local handoff is
`outputs/codex-p2-source-preview-2026-09-09/CentralAgent.exe`, 34,142,720 bytes,
SHA-256 `8eb9dce7449eabace427d4c30a1cc2626fd0003a7fb09ed6b7eb474adda521b2`.
`CentralAgentMcp.exe` is 5,645,824 bytes, SHA-256
`c355cc9757226c92bd1aa1a5a7c5f69951f852b27ea6813ac895a167b070edce`.
The preview contains the P2 remediation, passes hidden startup and manifest
verification, and requires external official CLI 0.153.4. It is an unsigned,
dirty-source development artifact, not a production release.

### Historical packaged handoff — 2026-09-08

The latest packaged handoff remains `outputs/codex-p1-final/CentralAgent.exe`,
built on 2026-09-08 with the
external official CLI **0.153.4**. It is 33,646,080 bytes and its SHA-256 is
`0ebb246ff6a1a05b9e1907dfc9780fea38a6b4f38f11216e0523b312d80cc6a7`.
The five release-manifest payloads were regenerated and verified against disk.
The build closes the stable P1 delivery but predates the current source-only P2
increment; it is unsigned development software, not a signed or
complete-public-parity release.

The dated increments below are historical records; their references to a
"current" or "latest" build apply only to their recorded date, not this handoff.
The current acceptance evidence is in `CODEX_APP_SERVER_ACCEPTANCE.md` and
the exact user-operated steps are in `CODEX_APP_SERVER_USER_VALIDATION.md`. The
official-version decision and upgrade procedure are in
`CODEX_APP_SERVER_IMPLEMENTATION_GUIDE.md`; method-level P0/P1/P2 status is in
`CODEX_APP_SERVER_GAP_ANALYSIS.md`.
P1 is closed, and the selected P2 scope is closed in source and deterministic
verification. Explicit Windows setup, OS picker/browser handoffs, external
App/MCP/P2 side effects and enterprise client registration remain separate
opt-in or external gates; never emulate missing callbacks, enable experimental
APIs or change system permissions to make a check pass.

P0 source hardening completed after this published handoff: typed bounded runtime
and security notices; atomic loaded-thread reconciliation; local-surface
unsubscribe; read-only permission-profile inventory; authoritative rate-limit
invalidation; and explicit non-ChatGPT subscription policy. One constant now owns
the `central_agent` handshake/service identity. OpenAI registration remains an
external enterprise release gate. This source increment has automated contract,
Rust and compiled-Svelte coverage but is not retroactively present in the
historical P1 package identified above; it is present in the current P2 preview.

The source-only P0 baseline is committed as `857e2e8`; the release-source gate
reports it clean and reproducible. Its full verification passed 648 Rust tests,
148 frontend tests, the 68-call stable contract inventory and 17 dedicated P0
fixtures. Component-level visual inspection covered both themes and all three
supported desktop profiles. No new executable was published.

P1 builds on that baseline and remains stable-only:
Apps discovery/selection and `$app-id` mentions; provider capabilities,
personality/modalities and upgrade metadata; account usage/workspace messages;
direct MCP resources and confirmed tools; bounded extended forms; precise
completed-turn forks; legacy-compatible detached-review ownership; hooks;
process-scoped extra skill roots; and validated audio input are wired through the
production Rust/Svelte paths. The contract inventory is now 94 serialized samples
across 47 methods including initialization, with 11 additional P1 incoming
fixtures. Global experimental API enablement, attestation and dynamic host tools
remain off.

The selected 0.153.4 schemas do not contain the rolling guide's `isPinned`
fields, so no pinning control was invented. Paginated histories still reject
detached review. Advanced arbitrary instructions/config, ephemeral forks and
structured host outputs remain constructor-only until an owned product workflow
exists. Closure verification passes 659 ordinary Rust tests, three additional
scope probes, 158 frontend tests, the schema/design/build gates, Clippy and the
dependency audit. Read-only live acceptance connected 0.153.4 and loaded the
actual MCP inventory after correcting bounded nested-schema handling and the
camel-case resource/schema projection.

P2 source implementation now adds 12 directly wrapped stable methods: four
standalone sandbox-command calls, feature list/set, external detect/import/
history, reset-credit consumption, workspace email nudge and feedback upload.
The total is 111 constructor samples across 60 methods including initialization,
plus 15 P2 incoming fixtures. All side effects require confirmation. Rust owns
project scope, raw migration items, native process IDs and reset idempotency;
feedback is text-only, and `experimentalApi` remains false. `fs/*`, generic
history injection/thread shell, single-value configuration and immature plugin/
marketplace APIs are deliberate holds. No real P2 side effect was performed and
the historical P1 executable does not contain this increment; the current source
preview does. The clean pre-hardening
P2 source gate passed 668 ordinary Rust tests with 22 opt-in probes ignored, three
additional scope-probe tests and 160 frontend tests with one retained-image
capture skipped; 108 outbound samples, 15 P2 fixtures, 35 design scenarios,
frontend generation, release probes, strict Clippy and the dependency audit also
pass. The four transitive audit advisories remain explicitly allowed and
non-blocking.

The later 2026-09-09 hardening increment is present in the source preview. Its
final workspace gate passes 693 ordinary Rust tests with 22 tests ignored by
that ordinary workspace gate (including 19 runtime probes), three scope probes
and 171 ordinary frontend tests with
one capture probe skipped, together with the full deterministic pipeline. All 19
opt-in probes were also audited separately, with 15 passes and four explicit
upstream-boundary diagnostics. The autonomous desktop audit is complete; only a
clean signed distributable and explicitly opt-in or external acceptance remain.

### Historical implementation records

User-operated acceptance handoff (2026-09-07, no production/build change):
`CODEX_APP_SERVER_USER_VALIDATION.md` records exact current labels and expected
states for account readback, explicitly confirmed Windows setup, actual OS
file-picker handoff in main/graph, and optional login/MCP browser steps when
needed. Each starts as not performed; setup may change Windows and request UAC,
and logout can affect the shared CLI account. No such operation is authorized by
a status check. User choice/results are needed before certifying those gates.
Do not repeat unrelated unit suites as a substitute, enable experimental APIs
or infer full completion from this handoff. The 23:01:03 preview remains current.

Current verified build (2026-09-07 **23:01:03 Europe/Rome**): corrected pending
approval-card ordering in the shared native client. Cards now follow their
owner's numeric arrival sequence instead of lexical ticket order, without
changing native IDs, decision contents, authority or execution scheduling.
The regression reproduced the defect before the fix and now covers 120
interleaved main/graph callbacks, sending/removal and survivor identity; the
frontend preserves that host order. DESIGN.md and the existing decision scenario
record the invariant. No new interface surface, model inference or visual test.

`%TEMP%/central-native-request-order-release.log` records the complete passing
pipeline: 622 Rust tests, 144 frontend tests, 21 explicit Rust probes and one
optional frontend capture test skipped, formatting/contracts/check/clippy,
provider-reset verification, release probes and hidden WebView startup. The
same 13 allowed dependency warnings remain. All five published manifest records
were independently verified at `2026-09-07T21:01:19.4315823Z`.
Unsigned preview `outputs/codex-app-server-preview/CentralAgent.exe`:
32,956,928 bytes, SHA-256
`23feca8787bb40491971097f7654180207bc4b133ef8b4430f2eb04fd4fd3ae6`.
This supersedes older current-build labels below, not their historical evidence.
Outstanding native/OS acceptance gates remain open; no complete-parity claim.

Named-permission boundary verified against actual 0.153.4 (2026-09-07):
`permissionProfile/list` is present in the generated stable request union and
works without experimental opt-in. An isolated profile inventory returned all
four profiles through one-item pages, including the owned named read-only
profile and its native `allowed` flag/description. Direct `thread/start.permissions`
is rejected with JSON-RPC -32600, `requires experimentalApi capability`, with no
thread creation or model request. The dedicated ignored native boundary test
passes (`%TEMP%/central-native-permission-boundary.log`). This corrects the older
source-audit wording below; inventory availability is not selection support.

A separate diagnostic using `config.default_permissions` completed one fixed
loopback READY turn but received no `thread/settings/updated` event or active
profile provenance. It correctly fails its stronger inheritance oracle
(`%TEMP%/central-native-permission-profile.log`); this is not proof that named
permissions were ignored or that the sandbox is broken. Do not use that route
as a production substitute for the documented beta field, derive a profile from
the compatibility sandbox, or enable experimental APIs silently. Both probes
use only disposable owned config/workspace/history and no account inference,
tools, user credentials or visual checks. No production/UI/build payload changed;
the verified 22:42:28 preview below remains current. Named-profile selection
remains outside the current stable client; the full objective remains open.

Current build and MCP progress increment (2026-09-07 **22:42:28 Europe/Rome**):
the client now projects native MCP progress as ordered plain text in the same
main/graph tool row, with no native item/result/history mutation. Schema-backed
tests pass. The isolated actual Code Mode/MCP diagnostic receives a progress
token but no App Server progress callbacks; it correctly fails and remains
opt-in. Existing six native elicitation cases still pass. No synthetic callbacks,
account inference or visual tests. The complete pipeline in
`central-native-mcp-progress-release.log` passes 621 Rust and 143 frontend tests;
19 opt-in Rust tests and one frontend capture test skipped, same 13 allowed
dependency warnings. Published unsigned preview is 32,964,096 bytes, SHA-256
`c03c22720336310f35b766a1df2403ec8227336f6919f7a89299ef7269b12fca`.
All five manifest entries verified independently at `2026-09-07T20:42:46.8187685Z`.
This supersedes older current-build labels below; outstanding native/OS gates
are unchanged and the objective remains active.

Latest correction and verified build (2026-09-07 **22:32:39 Europe/Rome**): the
display-only mirror now handles native `item/fileChange/patchUpdated` replacement
  snapshots. Runtime and shared main/graph tests cover identity, isolation, stale
history reads and authoritative completion; the three shared fixtures validate
against the generated 0.153.4 schema. No local patch execution was added.
`central-native-patch-stream-release.log` records 618 Rust tests and 142 frontend
tests passing (18 Rust opt-in tests and one frontend capture check skipped),
format/check/clippy, full verification and hidden WebView startup. The same 13
existing allowed dependency warnings remain. Current unsigned executable:
`outputs/codex-app-server-preview/CentralAgent.exe`, 32,963,584 bytes, SHA-256
`e4055f202dc6ff4e89441ba51d5b32fa53d84f9d1b4c9dcc5fa95c79f41d51f4`.
All five manifest records independently verified at `2026-09-07T20:33:02.6512065Z`.
This supersedes older current-build labels below, not their test scope. No visual
test or new inference was performed; the full objective remains open.

Actual native generated-image verification (2026-09-07): after permitting native
preparation in the existing requested sandbox, the runtime produced a completed
image with 1,019,232 embedded characters, retained identically in native history.
The probe no longer assumes Codex's savedPath is under cwd; it never follows or
deletes that path and preserves actual embedded wire evidence before cleanup.
The production shared main/graph projection fully decodes the real **1254×1254**
image and preserves its live/history row; the compiled Svelte component passes
with those actual bytes. Native generation/history, shared projection and markup
acceptance are verified, not merely synthetic fixtures. No visual checks or
complete live-composer image-generation journey are claimed. Exact retained
artifact directory, commands, logs and account usage are in the completion audit.
Only examples/tests/docs changed; the 22:02 preview remains current.

Native image verification (2026-09-07, after the 22:02 preview): no-inference
`handshake --media-capabilities` confirms native imageGeneration=true and stable,
enabled `image_generation`, using versioned empty capability params and paginated
feature metadata. One opted-in `image_generation_probe --allow-test-inference`
request on server-selected GPT-5.6-Sol emitted a command before any image, so the
strict image-only diagnostic interrupted and deleted its exact owned thread.
This is not a passed generation/host acceptance gate or proof of unavailability.
No alternate image API, automatic retry, permission grant or visual test. The
example refuses execution without its explicit inference flag; all-target runtime
Clippy and format pass. Logs and remaining scope are in the completion audit.
Only diagnostic examples/docs changed; the 22:02 build remains current.

Full current workspace and handoff verification (2026-09-07 **22:02:50 Europe/Rome**):
`central-native-final-audit-release.log` passed 614 Rust tests (17 opt-in ignored),
142 frontend tests, Svelte/design/scenario checks, generated 0.153.4 protocol
contracts, provider-boundary checks, format/check/clippy and hidden WebView startup.
The dependency audit retains 13 existing allowed warnings. Published unsigned
`outputs/codex-app-server-preview/CentralAgent.exe`: 32,963,072 bytes, SHA-256
`4f927ec630060b23d7e58d0ca61ba0bddb4d1f9b2bc7c70ac0f1e9bd70a0cfb5`.
All five manifest records independently verified at `2026-09-07T20:03:10.5047864Z`.
This closes the workspace/build/documentation handoff item below, not native
network/review approvals, Windows setup, OS picker/OAuth launching or actual
generated-image acceptance. No visual tests, new engine logic or user-data cleanup.

Native review-policy evidence (2026-09-07): a deterministic loopback-model probe
using production read-only thread and inline-review constructors confirms native
`blocked by policy` for the fixed print-only command. No approval callback or
command-execution item is sent, while review exit and persisted history arrive.
The approval diagnostic remains failing/open, not converted to a pass based on
review completion. This directly observed attempt is not a hidden UI approval.
No escalation, custom review logic, account inference or production changes.
Logs: `central-native-review-policy.log` and
`central-native-review-policy-runtime-tests.log` (112 ordinary runtime tests
passed, 15 opt-in ignored); Clippy/format passed. Published preview unchanged.

Reserved-host network follow-up (2026-09-07): the fixed `.invalid` destination
also emitted no native network approval callback. Unlike the preceding loopback
probe, its command failed with a transport connection-aborted error. The denial
gate remains open: absence of connectivity is not evidence of a native approval
decision. Tightened only the diagnostic's working-directory and exact network
request/thread resolution checks; prerequisite command resolution cannot satisfy
the network gate. Runtime unit suite: 111 passed, 14 opt-in ignored. Logs:
`central-native-network-invalid.log` (expected acceptance failure) and
`central-native-network-invalid-runtime-tests.log` (ordinary suite passed).
No production behavior, authority, model account, or executable changes.

Native network-policy diagnostic (2026-09-07, after the 21:29 preview): actual
0.153.4 proxy startup accepts a positive local port in an isolated named profile;
port zero failed Windows shared-ingress reservation. After Allow once for the
exact fixed proxy-only command (full native argv/action validated), the local
target received one HTTP request and returned 200. No native network approval
callback was emitted, so the new ignored diagnostic intentionally fails its
network-denial oracle. Do not mark network amendment acceptance complete, infer
universal enforcement failure, or add a fake callback/full-access fallback.
Two new ordinary safety tests cover exact command/port scope and injection
rejection. Runtime verification passed 110 tests with 14 opt-in ignored, plus
all-target Clippy and formatting. Logs: `central-native-network-denial.log` and
`central-native-network-runtime-tests.log`. Test-only change, no account-backed
model call, Windows setup or visual testing; published executable unchanged.

Published native Goal host preview (2026-09-07 **21:29:59 Europe/Rome**):
`outputs/codex-app-server-preview/CentralAgent.exe`, **32,963,072 bytes**,
SHA-256 `26557090fe3dd1bf1ba1d1c1c7fb96612da2e4a5627ec209c14ce11d0e76ea43`.
The full pipeline passed 610 Rust and 142 frontend tests (15 opt-in ignored),
static checks/clippy/format, generated protocol validation, provider reset,
dependency audit (13 existing allowed warnings) and hidden WebView startup.
All five manifest records were independently verified at
`2026-09-07T19:30:33.8179958Z`. Actual main and graph Goal acceptance also passed
on this exact executable: `central-native-goal-release-main.log` and
`central-native-goal-release-graph.log`; full build log:
`central-native-goal-host-release.log`. No visual tests or account-backed model
calls; external official CLI 0.153.4 still required. Other open native/OS gates
in the completion audit remain; the objective is not yet complete.

Native Goal desktop-host increment (2026-09-07): added the opt-in
`--check-native-settings --native-goal [--graph]` acceptance mode. Debug main
and graph both passed through the compiled create/confirm/activate/pause/remove
controls, showing the native autonomous turn as active with Stop available and
its final text in chat. Native readback retains the exact completed turn IDs;
owner/sibling drafts and empty fixture workspaces remain unchanged. Both tests
use owned temporary profiles and a loopback text-only Responses endpoint, no
personal account or custom scheduler. The graph harness's missing state observer
was corrected, not the production UI. Logs: `central-native-goal-host-main.log`
and `central-native-goal-host-graph.log`. This closes the main/graph native Goal
activation gate for this local fixed-response scope; real model/tool behavior
and the other outstanding native/OS gates are not implied by it.

Native Goal acceptance increment (2026-09-07): the real 0.153.4 runtime, isolated
from personal state and backed only by a local fixed-response model fixture,
starts a turn when its goal is activated without any client `turn/start`.
The new opt-in `native_goal_activation_and_paused_restart_use_server_execution`
test verifies pause without implicit interruption, matching goal notifications,
persisted native turn IDs, blocked/complete/clear, sibling isolation while loaded,
and exact paused-goal state across restart. The initial metadata-only assumption
was disproved by the real server and removed from the test. No scheduler or
continuation prompt was added. Main/graph Goal button acceptance was subsequently
completed by the separate desktop-host increment above.
Runtime tests (108 passed, 13 ignored), all-target Clippy and formatting passed;
logs: `central-native-goal-activation.log`, `central-native-goal-runtime-tests.log`.
This test/documentation-only increment does not replace the published executable.

- OpenAI's [App Server guide](https://learn.chatgpt.com/docs/app-server) recommends
  App Server for rich interactive products, and SDK for job automation.
- Ship a thin **Rust client**, not another Codex implementation. A persistent
  official `codex app-server --listen stdio://` child owns inference, tools,
  sandboxing, authentication, history, compaction and turn lifecycle.
- Rust owns the process, bidirectional JSONL connection, request correlation,
  immutable UI ownership and permission decisions. Svelte renders typed events
  and emits user intent; untrusted browser WebViews never reach this channel.
- Use generated schemas/types from the selected runtime. Initial contract:
  `codex-cli 0.153.4`, generated without `--experimental`. Version mismatches
  must be explicit, not papered over with guessed fields or fallback commands.
- No required Node.js runtime, HTTP listener, daemon shared with another app,
  private cookies, credential parsing, Responses API key or manual tool loop.
- Keep experimental capability off by default. Do not call the under-development
  plugin install/list APIs, deprecated rollback, or unsandboxed process APIs to
  simulate missing functionality. Record unsupported capabilities honestly.

## Milestones and acceptance

Unprojected-notification diagnostics increment (2026-09-07): the native host now
records metadata when none of its conversation/account/request/MCP/goal handlers
projects a notification. This observes unsupported and out-of-scope events; it
does not classify them as errors or create work. The existing AI Settings account
card exposes a collapsed Protocol diagnostics disclosure, never a chat message.
Only names in the generated stable JSON notification union are displayed; other
names appear as SHA-256 fingerprints. Payloads and native conversation IDs never
enter the collector. It holds 32 recently observed identities and saturating
counts in memory, retains the last connection after disconnect and resets on
explicit reconnect. This storage bound does not limit agent actions or output.

Three Rust tests cover stable-schema name fidelity, unknown-name privacy,
recency/count bounds and independence from conversation state. A real-pipe
fixture verifies an unknown notification reaches the consumer, changes no native
binding or active work, and leaves subsequent requests usable. A compiled Svelte
server-render test covers empty/populated and connected/disconnected disclosures,
no auto-opening, no action controls and exclusion of raw metadata/payload extras.
The schema test initially caught rawResponse methods present in generated TS but
absent from stable JSON; the allowlist now comes directly from stable JSON.

Published unsigned development preview: built **2026-09-07 21:05:11 Europe/Rome**,
`outputs/codex-app-server-preview/CentralAgent.exe`, **32,916,480 bytes**,
SHA-256 **f28102b7e4b890176f8ae11b992be4f2d76387d8e528b417e79357e24cd7a7de**.
Full pipeline passed **610 Rust tests and 142 frontend tests** (14 opt-in tests
ignored by default), generated protocol checks, Svelte/design/inline-JS checks,
format/check/clippy, scoped dependency audit with 13 existing allowed warnings,
and hidden WebView startup checks. Log: `central-native-diagnostics-release.log`.
All five published manifest records were independently reverified at
`2026-09-07T19:05:28.0913337Z`. No visual tests or model inference were used in this
increment. Official Codex CLI 0.153.4 remains an external runtime dependency.
Other native/OS acceptance gates remain open; this is not a parity release.

Completion audit (2026-09-07, after the stdio-loss increment):
the current requirement/evidence map is in
[CODEX_APP_SERVER_ACCEPTANCE.md](CODEX_APP_SERVER_ACCEPTANCE.md). Dated progress
paragraphs below are historical records, not an authoritative list of today's
remaining work. In particular, their older claims that restart, provider switching
or Queue/Steer are still pending are superseded by the later verified increments.
The goal remains open: passing the core conversation checks is not proof that
every advanced native control and OS integration has been accepted.

Whole-application stdio-loss restart increment (2026-09-07, main/graph passed):
`--restart --restart-crash --restart-wire-loss=before|after [--graph]` extends
the existing explicit account-backed desktop harness. A first no-tool seed
materializes native history. An acceptance-only executable delegates --version
to the actual CLI and runs the existing native stdio relay; normal transport,
runtime discovery, thread engine and UI are unchanged. It requires the checkout,
rustc and Node only for this opt-in developer test. The relay and its root/token
are parent-owned temporary artifacts, not shipped runtime dependencies.

Before mode drops the second turn/start before forwarding. After mode forwards
it but loses every native response/notification through its completion, so the
real client never receives an ACK or reconstructed event. The parent crashes
its exact app child after receipt and newer main/sibling drafts are durably saved;
Windows production job ownership must terminate both the relay and actual native
server. A second app loads the same binding and pending receipt without history,
uses the actual history inspection/dismiss/resume controls, preserves drafts and
continues without replay. Readback requires two native inputs for before mode,
three for after mode, original thread/directory and no legacy transcript copy.

Debug main passed both cases in `central-native-restart-wire-before-debug-main.log`
and `central-native-restart-wire-after-debug-main.log`. Two and three model turns
respectively used account allowance. Only each newly owned native thread was
deleted. Unit coverage rejects malformed/repeated mode choices and incorrect
loss-marker counts, acknowledgement state, phase or PID.

All four cases passed on the published executable:
`central-native-restart-wire-before-release-main.log`,
`central-native-restart-wire-after-release-main.log`,
`central-native-restart-wire-before-release-graph.log`, and
`central-native-restart-wire-after-release-graph.log`. The parent verified the
actual official server's exit as well as the relay's. Production restart restored
the saved receipt and newer drafts; explicit native inspection/dismiss/resume
preceded continuation. Final native inputs matched exactly the selected before/
after case, with no unexpected tool items, workspace changes or legacy transcript
copy. Graph sibling drafts stayed saved and inactive. Only the four newly owned
native threads and their temporary test profiles were removed. Four invalid
argument combinations were rejected before application initialization.

The existing isolated runtime wire-loss regression also passed all four owner/
boundary combinations after the relay metadata change, without account inference:
`central-native-restart-wire-relay-regression.log`. This supplements, rather than
replaces, the actual account-backed two-process desktop tests above.

Previous verified preview (before diagnostics): `outputs/codex-app-server-preview/CentralAgent.exe`,
2026-09-07 **20:40:13** local, **32,735,232 bytes**, SHA-256
`b8fa9a1183bfc269439c2433d5808cb388868a0ca016bea98d3940d41d7fbd5f`.
Release log: `central-native-restart-wire-release.log`. Full verification passed
**606 Rust tests**, **141 frontend tests**, generated contract checks, lint/clippy,
inline JavaScript, design checks and hidden-WebView nonvisual startup. Fourteen
opt-in Rust tests are ignored by default; the wire-loss one was separately run
above. Dependency audit retains the same 13 allowed warnings. All five manifest
records were independently verified at `2026-09-07T18:41:08.0818700Z`.
MCP executable unchanged; this unsigned development preview still requires the
external official Codex CLI 0.153.4. No production engine or UI changed. This
closes the whole-app wire-loss gate, not the remaining native sandbox, review
approval, media/picker or other explicitly listed acceptance gates.

Native running-review Stop acceptance (2026-09-07, passed on main and graph):
the explicit `--review --review-command-stop [--graph]` mode waits for a real
print-only command's running marker and clicks the existing Stop control.
The fixture has a 20-second sleep, not a production deadline. No production
agent engine, cancellation mechanism or UI change was added. It verifies native
interrupt acknowledgement, interrupted persisted review/worker IDs, inactive UI,
unchanged workspace/seed/drafts and cleanup of only the newly owned native thread.

Earlier actual attempts exposed incorrect test assumptions: Stop may target a
different ID from the review acknowledgement/terminal notification; a streamed
command and temporary native user items can be absent from persisted history.
The latter was reproduced in `central-native-review-command-stop-debug-main5.log`:
three persisted review items plus three genuinely live items in the display.
The test must preserve native live evidence, not demand its deletion or fabricate
persistence. A dedicated oracle now checks ordered inclusion of persisted items
and received-event/response evidence for extra scoped identities. A unit test
rejects unknown or cross-turn extras, changed types, omitted/reordered persisted
items and duplicates. Displayed command activity must be stopped, not running.

The separate `--review-approval-stop` diagnostic has not passed. Actual logs
`central-native-review-approval-stop-debug-main.log` and
`central-native-review-approval-explicit-debug-main.log` show no native approval
callback; no approval/elevation was granted. All owned test threads were deleted.
Do not classify this as a production UI defect or claim review approval support
verified. The already verified ordinary-turn approval coverage remains separate.
The corrected Debug main test passed in
`central-native-review-command-stop-debug-main6.log`. The published executable
then passed both `central-native-review-command-stop-release-main.log` and
`central-native-review-command-stop-release-graph.log`: real command output,
actual scoped Stop, interrupted native history, no displayed running activity,
unchanged seed/workspace/drafts, graph sibling isolation and deletion of each
owned native thread. Each case used one seed and one review from the account;
no personal conversation was inspected. Four invalid/mixed option combinations
were rejected by the published executable before application initialization.

Previous verified preview: `outputs/codex-app-server-preview/CentralAgent.exe`,
2026-09-07 **20:27:37** local, **32,769,024 bytes**, SHA-256
`2211bfd38c9052eda25e446065254d7938594126561d3302bea457f5fc4b8617`.
Release log: `central-native-review-command-release.log`. Full verification passed
**605 Rust tests**, **141 frontend tests**, generated native contracts,
lint/clippy, inline JavaScript, design checks and hidden-WebView startup. Fourteen
opt-in Rust tests remain ignored; dependency audit retains 13 existing allowed
warnings. All five manifest records were independently verified at
`2026-09-07T18:28:06.3973072Z`. The MCP executable is unchanged. This is an unsigned
development preview requiring official Codex CLI 0.153.4, not a final parity claim.
Only acceptance code and documentation changed in this increment. Native review
approval, whole-app wire-loss and the remaining listed gates stay open.

Native preparation-Stop acceptance increment (2026-09-07): added explicit
`--review --review-prepare-stop` to the existing compiled conversation harness.
Only its real successful preparation reply is held at the worker-to-host boundary.
The actual main/graph Stop must mark the unsent review cancelled before the
unchanged reply is released. The oracle requires no review/start or turn/interrupt,
no native review items, exactly unchanged seed history, released pending state,
hidden Stop, preserved drafts/sibling state and an untouched temporary workspace.
One seed prompt uses account allowance; no review inference should start. The
seed-history oracle unit test rejects extra turns, wrong threads and changed seed
status. Production code and UI remain unchanged. Both actual published-executable
tests passed: `central-native-review-preparation-main.log` and
`central-native-review-preparation-graph.log`. Each sent exactly one no-tool seed,
cancelled through the actual Stop button, released the original preparation reply,
and verified unchanged native history/workspace/drafts with no review or interrupt
request. Both newly owned native threads were deleted. Invalid/mixed test flags
were also rejected before application initialization.

Previous verified preview: `outputs/codex-app-server-preview/CentralAgent.exe`,
2026-09-07 **19:54:33** local, **32,739,840 bytes**, SHA-256
`4e1a14e1e9b8561f713058e673ee6e794401bec653db13fc065310194012b4a3`.
Full release verification passed **604 Rust tests**, **141 frontend tests**,
generated protocol checks, lint/clippy, hidden-WebView nonvisual startup and scoped
dependency audit (13 existing allowed warnings). Fourteen opt-in Rust tests remain
ignored by default. Log: `central-native-review-preparation-release.log`. All five
manifest records were independently verified at `2026-09-07T17:55:15.2366334Z`.
MCP executable unchanged; this unsigned development preview requires the external
official Codex CLI 0.153.4. The full objective is still active. This closes only
preparation cancellation, not Stop during review tools/approvals or late workers.

Rechecked native Plan availability in this increment: current official docs still
mark collaborationMode/list experimental; selected 0.153.4 stable TurnStartParams
still omits collaborationMode. The previous unsupported-mode conclusion stands.
No custom planning prompt or experimental capability was introduced.

Native Git-target review acceptance (2026-09-07): the optional
`--review-git=uncommitted|branch|commit` mode extends the existing explicit
`--check-native-conversation --allow-test-inference --review [--graph]` host test.
It initializes only its new empty temporary workspace, commits a tiny documented
Rust function, then introduces a reversed comparison as a worktree change or
commit. The actual review dialog selects the native target; the client does not
substitute a custom prompt. Native review preparation remains read-only/untrusted.
The probe grants no approval requests and never broadens the sandbox. It requires
successful native command output showing the changed source, a source-file finding,
one rendered result and authoritative native history. Fixture file contents, HEAD,
index, tracked paths and worktree status must remain unchanged; main/graph drafts
and sibling state remain isolated. Git setup has no remotes, ignores global/system
config, disables hooks/signing and supplies author identity only per command.
Three-target unit verification passed; mutation of a fixture is detected, and
nonempty/non-temporary targets are rejected. Actual Debug main uncommitted review
passed with two successful native commands and owned-history cleanup
(`central-native-review-git-uncommitted-main.log`). The published executable then
passed uncommitted **graph**, branch **main and graph**, and commit **main and
graph**. All five logs use
`central-native-review-git-release-{uncommitted|branch|commit}-{main|graph}.log`
(uncommitted main is the Debug log above, not an additional Release run).
Every case reported two successful native commands showing the actual changed
source, preserved files/HEAD/index/drafts, returned one rendered finding and
matched authoritative native history. Each newly created native thread was
deleted successfully. No permission was granted by these probes; review approval
choices remain a separate gate. This increment changes test coverage and
documentation, not the production review engine or UI.

Previous Git-review preview: `outputs/codex-app-server-preview/CentralAgent.exe`,
2026-09-07 **19:40:54** local, **32,683,520 bytes**, SHA-256
`fb50656f48dd633335978fdc9b90c5444371d7a36bc6838f7ef82d22af583740`.
The full release pipeline passed **603 Rust tests**, **141 frontend tests**, the
generated contract, lint/clippy, hidden-WebView nonvisual startup verification and
scoped audit (13 existing allowed warnings). Fourteen opt-in Rust checks remain
ignored in the default suite. Log: `central-native-review-git-release.log`.
All five manifest records were independently verified at
`2026-09-07T17:41:57.2280863Z`. The unsigned preview uses the external official
0.153.4 CLI; MCP executable is unchanged. Orphan, conflicting and Stop-combined
Git target flags were also rejected before initialization. The objective remains
active; other native feature/acceptance gates below are not certified by this work.

Native review duplicate-output correction (2026-09-07): the strengthened actual
graph review test reproduced two rendered results (`central-native-review-single-before.log`).
Native history contains an exitedReviewMode followed immediately by a completed
phase-less agentMessage with identical result text. The display adapter previously
rendered both as answers. It now projects that exact echo only through the stable
exit row, without changing native history, IDs, turn states or model input. It
does not remove different/explicit-phase/nonadjacent/unfinished messages or
deduplicate across turns. Stream/readback and negative fixtures cover those
boundaries. The compiled review oracle now requires exactly one matching result,
not merely presence. The new oracle failed against the old projection and now
passes on the published executable in both **main and graph**, using actual
native review/start and complete thread/read. Logs:
`central-native-review-single-release-main.log` and
`central-native-review-single-release-graph.log`. Both native histories still
contain the exit and echo plus the remaining worker/review records; each probe
preserved drafts and its empty workspace and deleted only its new native thread.
This does not certify every native review target, tool approval or timing case.

Latest verified preview: `outputs/codex-app-server-preview/CentralAgent.exe`,
2026-09-07 **19:31:18** local, **32,628,224 bytes**, SHA-256
`3d11d56a75ae3ada4c445c4e5ee1dc2194a7e7909b72c5d14d7a3283ce2218f8`.
Full release verification passed **602 Rust tests**, **141 frontend tests**, the
generated native contract, lint/clippy, hidden-WebView nonvisual startup checks
and scoped audit (13 existing allowed warnings); 14 opt-in Rust checks remain
ignored by the default suite. Log: `central-native-review-single-release.log`.
All five manifest records were independently verified at
`2026-09-07T17:31:56.8074845Z`. The preview is unsigned, uses the external official
0.153.4 CLI and leaves the MCP executable unchanged. DESIGN.md and the native
review scenario record this display-only rule. The full objective remains active.

Native review Stop acceptance (2026-09-07): the opt-in `--review-stop` modifier
requires `--check-native-conversation --allow-test-inference --review`, with
optional `--graph`. It clicks the actual Stop control only after native review
entry and acknowledgement, captures the displayed active turn ID, verifies its
exact `turn/interrupt` acknowledgement and interrupted terminal notification,
then compares the complete projected ID/status set with fresh native history.
Both the acknowledged review and targeted turn must persist as interrupted;
completed/failed/missing turns cannot satisfy the new negative-tested oracle.
The original seed, owner/sibling drafts and empty temporary workspace must remain
unchanged. This is client acceptance, not a local cancellation engine.
The initial compiled Debug main-host run passed (`central-native-review-stop-main.log`)
and deleted only its new native test thread. The first graph attempt exposed a
missing graph-composer event listener in the test harness; this prevented the test
from observing the active turn. The listener is now installed before seed input,
without changes to production UI or native lifecycle. The failed attempt reached
the acceptance deadline and deleted its new native test thread; it is not a Stop
pass (`central-native-review-stop-graph.log`).

The final published executable passed actual account-backed **main and graph**
Stop checks: `central-native-review-stop-release-main.log` and
`central-native-review-stop-release-graph.log`. Each persisted exactly the native
seed and interrupted review, preserved drafts/workspace, received both the native
interrupt ACK and terminal notification, and deleted its owned native thread.
The active turn equalled the review ACK in these early-Stop runs. They do not
certify Stop during preparation, a late distinct worker, or tool approval. Missing
consent and orphan `--review-stop` are rejected before startup/inference
(`central-native-review-stop-flags.log`, `central-native-review-stop-orphan-flag.log`).

Latest verified preview: `outputs/codex-app-server-preview/CentralAgent.exe`,
2026-09-07 **19:21:52** local, **32,659,968 bytes**, SHA-256
`f427a91ce1de5c949a64f7d5dfbeddf99883010c47846ee08e6693b0693bd83b`.
The full release pipeline passed **600 Rust tests**, **141 frontend tests**, the
generated native protocol contract, lint/clippy, nonvisual hidden-WebView startup
checks and scoped audit (13 existing allowed warnings); 14 opt-in Rust checks
remain ignored in the default suite. Log: `central-native-review-stop-release.log`.
All five manifest records were independently verified at
`2026-09-07T17:22:51.0990858Z`. This unsigned development preview still uses the
external official 0.153.4 CLI; MCP binary is unchanged. The objective remains
active with the broader native feature/acceptance gates below still open.

Native review host acceptance (2026-09-07): the explicit
`--check-native-conversation --allow-test-inference --review [--graph]` mode
reuses the compiled composer, native conversation dialog, owner-scoped IPC and
existing owned-history cleanup. It submits one tiny seed prompt, cancels all four
review target dialogs without dispatching work, then confirms one inline custom
review of an in-prompt Rust snippet. It checks the original native thread, native
entry/exit events, authoritative completion/readback, actual rendered review text,
unchanged draft and graph sibling, and an unchanged empty workspace. Unexpected
tools or permission requests fail the no-tool probe; no permission is granted by
the harness. Mixed test modes are rejected before any app/profile initialization.
The new negative completion oracle passed unit verification. Actual compiled-host
results and published artifact are recorded below; no claim of
review target execution for branch/commit/uncommitted changes is implied by their
dialog cancellation checks. Native review remains wholly owned by App Server.

The first real main-host check exposed a production Windows path-comparison bug:
the selected workspace was canonical `\\?\C:\...`, while native `thread/resume`
confirmed the same directory as `C:\...`. All requested sandbox/network/approval
fields matched, but lexical Path equality rejected the scope before `review/start`.
Two diagnostic runs each used one seed prompt, sent no review, and deleted their
own native test thread. Logs: `%TEMP%/central-native-review-host-main.log` and
`central-native-review-host-scope.log`. The client now compares existing absolute
directories through filesystem canonicalization; missing/relative/file/foreign
paths still fail, with no string-prefix fallback. Sandbox type, network denial,
approval policy, reviewer and native thread/lifecycle checks remain unchanged.
Ten targeted review tests passed, including the exact Windows prefix regression
and rejection of broader network access even for the equivalent directory.

The subsequent main-host test confirmed that this runtime persists an additional
interrupted intermediate worker record during inline review. The acceptance oracle
was corrected from an assumed two-record count to exact native ID/status projection
equality, unchanged full native seed record, one actual UI confirmation, one
`review/start` and the completed turn acknowledged by native Codex. Main then passed
actual dialog cancellation/confirmation, read-only preparation, streamed entry/exit,
persisted result and exact rendered Markdown text, retaining its draft and deleting
only its owned native test thread (`central-native-review-history-main.log`).
Graph uses the existing trusted surface's `AgentPanel` IPC, whereas main uses
`ScopedAgentPanel`; the acceptance observer now recognizes both with the same
exact owner/thread/target checks. No production routing or model behavior changed.

Debug main and graph hosts now both pass the self-contained inline review flow,
with real account inference, native entry/exit and complete history readback,
exact displayed Markdown text, preserved drafts/sibling isolation and successful
deletion of only the newly created native test conversation. Both native histories
contained three records: original completed seed, interrupted intermediate worker
and completed acknowledged review. The client display's ID/status set matched
native history; no synthetic cleanup/completion was used. Logs:
`%TEMP%/central-native-review-history-main.log`, `central-native-review-final-graph.log`.
`central-native-review-flags.log` also confirms incompatible modes are rejected
before startup, without inference. Branch/commit/uncommitted execution, tool
approvals during review and interrupted-review host interaction are not claimed
by this no-tool custom-target probe and remain distinct acceptance cases.

Published corrected preview: `outputs/codex-app-server-preview/CentralAgent.exe`,
2026-09-07 **19:07:14** local, **32,619,008 bytes**, SHA-256
`7dcc58fcfb811f3d0b79d6883c8b51763f0d710c128ad38bac83a3487b09ce60`.
All five manifest records were independently verified at
`2026-09-07T17:07:51.4276767Z`. Complete workspace verification passed:
**599 Rust tests**, 14 explicitly opt-in tests ignored by the ordinary suite,
**141 frontend tests**, generated schemas, design/inline-JavaScript checks,
all-target Clippy with warnings denied and audit with 13 existing allowed warnings.
The staged actual EXE passed hidden-WebView startup/DOM checks. The published EXE
also passed graph command-policy regression and exact owned-profile cleanup,
without inference (`central-native-review-release-policy-regression.log`).
Release log: `%TEMP%/central-native-review-final-release.log`.
Review inference acceptance above was run on Debug hosts; it was not repeated on
the published Release. No visual tests were performed. This unsigned preview
requires official CLI 0.153.4. The overall objective remains active.

Graph fixture startup ordering (2026-09-07): the 18:19 published command-policy
probe passed in main but failed in graph before any turn, because its test actor
opened cards as soon as transport connected, before native model/effort selection
was available. The production graph binding validator correctly rejected that
empty fixture profile. The acceptance-only guard now waits for completed startup
refresh, managed requirements and a selection validated against the actual native
catalog. It neither fabricates a selection nor requires account authentication
for the owned loopback fixture. The MCP elicitation graph probe shares this guard.
A regression test covers disconnected, empty, not-yet-selected, refreshing and
unsupported-effort states, including a ready unauthenticated fixture. Debug actual
graph policy acceptance then passed both turns and exact owned-profile cleanup.

The failed previous coordinator retained its partially removed test profile at
`C:/Users/<user>/AppData/Local/Temp/central-native-elicitation-O0NRsL` after Windows
error 145. No cleanup retry, permission change or process killing was performed on
that retained directory. Logs: `%TEMP%/central-native-command-policy-published-graph.log`
(previous failure), `central-native-catalog-ready-graph-debug.log` (fixed probe).
Final published-binary verification is recorded below.

The 18:32 Release subsequently passed main policy execution but its coordinator
hit the same Windows directory-not-empty error while deleting WebView cache files.
Its owned root `C:/Users/<user>/AppData/Local/Temp/central-native-elicitation-qGcZwd`
was retained, not retried later. This is teardown failure, not failed native work.
For newly created request-fixture profiles only, the existing bounded cleanup loop
now also retries Windows 145 alongside sharing errors 32/33. It does not retry
access denied, alter permissions, kill processes or revisit retained directories.
Unit coverage checks the exact error allowlist and actual file-handle release.

Verified current preview: `outputs/codex-app-server-preview/CentralAgent.exe`,
2026-09-07 **18:37:24** local, **32,561,152 bytes**, SHA-256
`955784dc05bffd665722f6e621cb4326b03818ed823132a025bf1d6f78e5da76`.
All five manifest records were independently verified at
`2026-09-07T16:38:34.1410936Z`. This supersedes the 18:19 and 18:32 previews.
Complete verification passed: **596 Rust tests**, 14 explicitly opt-in probes
ignored by the ordinary suite, **141 frontend tests**, generated contract,
design/inline-JavaScript checks, all-target Clippy with warnings denied and the
dependency audit (13 existing allowed warnings). Staged actual-EXE hidden WebView
startup/DOM checks passed without visual inspection.

The actual published EXE passed main command-policy acceptance and two consecutive
fresh-profile graph policy checks, plus six in-turn MCP form/URL decision cases
on each host. Every new coordinator verified removal of its own exact temporary
profile. No account inference, personal rules/config changes or screenshots.
Logs: `%TEMP%/central-native-catalog-ready-final-release.log`,
`central-native-catalog-ready-final-main.log`,
`central-native-catalog-ready-final-graph-1.log`,
`central-native-catalog-ready-final-graph-2.log`,
`central-native-catalog-ready-final-mcp-main.log` and
`central-native-catalog-ready-final-mcp-graph.log`.
This verifies the corrected acceptance startup/teardown, not native network-policy
amendments, OS sandbox setup, whole-app wire loss or the other pending gates below.
The goal remains active. Unsigned development preview; official CLI 0.153.4 required.

Compiled command-policy acceptance (2026-09-07): the new explicit
`--check-native-command-policy [--graph]` coordinator uses an isolated native home
and the same exact print-only model fixture as the runtime restart probe. Main and
graph actual desktop hosts both passed two native turns: real disclosure click,
exact three offered buttons, real policy-button IPC, owner/request/turn identity,
native answer and resolution, completed command output and persisted READY reply.
The second identical native command needs no new request or client decision. The
request card disappears, the unsent draft survives and graph sibling/main owners
receive no request/history. The parent verifies four local model responses and
removes only its exact owned profile after child exit. No account inference,
personal configuration/rules, workspace changes, screenshot or visual inspection.

The native print fixture was extracted into `native_exec_policy_fixture.rs`, shared
by the runtime and desktop acceptance only. The existing loopback Responses fixture
and owned-profile coordinator are reused; ordinary startup has no listener or
policy test behavior. Mixing an account-inference flag into this mode is rejected
before creating a profile or launching the native child. Two negative unit oracles
cover extra/missing/disabled buttons and mismatched ACK/event turn identity. The
runtime restart probe passed again after extraction. MCP in-turn main/graph
regression results and the final Release are recorded in the increment above.

Logs: `%TEMP%/central-native-command-policy-main.log`,
`central-native-command-policy-graph.log`, `central-native-command-policy-runtime.log`
and `central-native-command-policy-flags.log`. This closes the compiled-host
**command** policy gate. Native network-policy amendment behavior remains a
separate gate; no network grant is implied by the print-only test.

Native command-rule acceptance and offered-decision correction (2026-09-07):
the new opt-in `native_execpolicy_amendment_is_saved_and_reused_after_restart`
probe drives actual 0.153.4 App Server turns through an unauthenticated loopback
model fixture. The only executable input is one exact literal PowerShell print
expression. The test accepts only the complete proposed argument vector for the
known system/bundled shell, never a broad shell prefix or unvalidated model command.
Production Requests verifies the owner, sends `acceptWithExecpolicyAmendment`,
observes exact native resolution and reads persisted command/output/continuation.
After shutdown, a fresh App Server process on the same owned temporary profile
executes it without another approval. Four local model responses, no account
inference, personal rules/config, workspace changes or visual tests.

The actual request exposed a client defect: `availableDecisions` excluded Session
and Decline, but the card always displayed them. Rust now projects supported button
availability and revalidates exact native decision equality before sending.
Svelte renders those choices in main/graph; missing supported choices produce an
explicit no-fallback state. Omitted/null lists preserve selected-schema defaults;
empty/malformed/future-only lists cannot grant authority. This documented field is
present on the selected wire but absent from its generated request type: no generated
files or outgoing protocol types were changed, and no experimental API was enabled.
Unit tests cover exact exec/network proposals, wrong-owner/stale intent, unsupported
choices and default behavior. The compiled Svelte server-render check covers actual
button/disclosure markup; it is not a visual test or whole-desktop click acceptance.
Full compiled-host command/network policy acceptance remains open.

Evidence: `%TEMP%/central-native-execpolicy.log`, runtime request tests and
`ui/tests/native-requests.test.mjs`. Complete release verification passed: 592 Rust
tests (14 explicit opt-in probes ignored), 141 frontend tests, generated protocol,
design/inline JavaScript checks, all-target Clippy with warnings denied, and the
existing dependency audit with 13 explicitly allowed warnings. The staged actual
EXE passed hidden-WebView startup/DOM interaction checks, not visual inspection.

Published preview: `outputs/codex-app-server-preview/CentralAgent.exe`,
2026-09-07 **18:00:19** local, **32,527,872 bytes**, SHA-256
`224ff6a98e26da4e9274a95d423714112dc3b3f631fde233ebf5219c56eaf273`.
All five manifest records were independently verified at
`2026-09-07T16:00:43.3722164Z`. The actual published Release also passed six in-turn
MCP card cases on each of main and graph; coordinator-owned profile removal was
verified. Those regressions do not substitute for the still-open policy-button
host gate. Logs: `%TEMP%/central-native-policy-release.log`,
`central-native-policy-release-main.log`, `central-native-policy-release-graph.log`.
Unsigned development preview, external official CLI 0.153.4 required; goal active.

Compiled in-turn MCP host acceptance (2026-09-07): `--check-native-mcp-requests
--in-turn` extends the existing isolated request-card probe. Main and graph hosts
both passed six actual native turns (form/URL Accept, Decline, Cancel) and all
native pre-call permission forms. Real compiled form submit/button handlers send
decisions through production owner-scoped IPC; native ACK and resolution remove
the matching cards, preserve the unsent draft, keep sibling/main conversations
free of requests, and continue to exact persisted MCP tool/assistant items.
Each native history has six turns; each isolated model fixture sees twelve local
requests. No account inference, OS URL launch, workspace write, personal config,
UI screenshot or visual inspection. Both exact coordinator-owned profiles were
removed after child exit. This specifically tests in-turn request cards, not
composer submission (covered by the separate account-backed composer checks).

The response fixture is shared with runtime tests, not duplicated: no local
production tool engine, account bypass or protocol implementation was introduced.
Native permission and downstream elicitation IDs are tracked independently;
the harness accepts native turn ACK/started notification in either order, with
exact ID validation. A stale sampled permission card cannot receive the next
form's answer. Production behavior is unchanged.

Logs: `%TEMP%/central-native-in-turn-host-main.log` and
`central-native-in-turn-host-graph.log`. Existing standalone main/graph MCP card
checks passed again (`central-native-in-turn-host-standalone-main.log`,
`central-native-in-turn-host-standalone-graph.log`). Add `--graph` to the command
above for the graph variant. The native in-turn MCP card gate is now closed;
policy-amendment approvals and other outstanding gates remain separate.

Preview published after this increment: `outputs/codex-app-server-preview/CentralAgent.exe`,
2026-09-07 17:40:22 local, 32,574,464 bytes, SHA256
`ad50f92fa4ae83537319837595d1cbfdc96296698c7af0f3c555b5024024026c`.
Full pipeline passed 587 Rust tests (13 explicit opt-in tests excluded), 140
frontend tests, generated-contract/design/inline-JS checks, all-target clippy
and the existing scoped dependency audit (13 previously allowed warnings).
The staged executable passed hidden-WebView startup, toolbar, agent/editor,
graph and diff-search functional checks. Five manifest records were independently
verified at 2026-09-07T15:40:51.9898680Z. Both in-turn main/graph MCP probes then
passed **against the published Release executable**, including owned temporary
profile cleanup (`central-native-in-turn-release-main.log`,
`central-native-in-turn-release-graph.log`). Pipeline log:
`%TEMP%/central-native-in-turn-host-release.log`. Unsigned development preview,
external official Codex 0.153.4 required; active objective is not complete.

Native in-turn MCP acceptance (2026-09-07): a new opt-in runtime probe now runs
six actual App Server turns across two owned native histories, covering form/URL
Accept, Decline and Cancel. Unlike the prior standalone `mcpServer/tool/call`
checks, each starts with production `turn/start`, runs through native tool
execution, resolves both the native MCP call permission and the downstream
elicitation, then resumes to a persisted assistant response. Exact thread/turn,
request ID, wrong-owner rejection, removal on `serverRequest/resolved`, tool
identity, decision content, history length and saved tool/assistant items are
asserted. The synthetic workspace remains empty.

The native model connection uses an unauthenticated loopback Responses fixture;
no account inference or personal config is used. 0.153.4 supplies its code-mode
`functions.exec` declaration via an `additional_tools` input item, not a top-level
MCP function list. The test response uses that native executor to call only the
owned effect-free MCP fixture; no client-side tool executor, dynamic tool or
production protocol adaptation was added. Native pre-call consent is a separate
empty form, answered explicitly before the fixture's own form/URL request. Test
setup does not auto-answer any production request or enable experimental APIs.
The shared fixed READY fixture remains the default for earlier tests.

Command: `cargo test --locked -p central-agent-codex-runtime
native_in_turn_mcp_elicitation_is_owned_resolved_and_persisted -- --ignored --nocapture`.
Log: `%TEMP%/central-native-in-turn-mcp.log`. Six turns use twelve local responses;
no external URL is opened. Runtime suite: 100 passed, 11 explicit probes excluded;
the configured-summary and profile-transition probes passed again, and runtime
all-target clippy passed with warnings denied. Logs use the same prefix with
`-unit`, `-summary`, `-profiles` and `-clippy`. All four existing runtime wire-loss
recovery cases also passed again using the shared default fixture (`-wire.log`).
Initial fixture failures reflected
wrong assumptions about native tool declarations and the extra permission form;
the corrected probe passes without production changes. Compiled desktop in-turn
MCP request-card acceptance is still a separate open gate; this runtime probe
does not claim to have exercised that UI. Published executable is unchanged.

Compiled profile-picker acceptance (2026-09-07): the existing two-turn provider
round-trip check now has an explicit `--profiles` extension. Both main and graph
hosts passed actual model/effort/speed selection: Luna/low/Fast first, then
Sol/high/Standard on the same native thread. Choices are validated against the
real native catalog, selected using actual main menu buttons or graph selects
and Update, and compared with production Rust selection plus the actual native
`turn/start` payload after successful acknowledgement. No catalog or UI state is
fabricated. The preceding loopback runtime gate verifies native settings and
model-request normalization separately.

Permission selection uses the actual scoped native select and full-access dialog:
no grant before confirmation, Cancel preserves read-only, explicit Allow grants
only this owner, then the test restores read-only **before any model request**.
The second prompt uses Project access. Unsent draft, sibling graph assignment,
permissions and draft, native identity/history, and separately stored Claude
fixture history remain unchanged. No Claude inference. The actual four short
Codex prompts were announced beforehand; both exact owned native histories were
deleted. An initial harness run stopped before inference because it checked a
stale DOM sample; the probe now waits for the real draft acknowledgement before
driving the first selector. No production workaround was required.

Commands: `target/debug/central-agent.exe --check-native-conversation --providers
--profiles --allow-test-inference`, with `--graph` for the card variant. Logs:
`%TEMP%/central-native-profile-host-main.log`, `central-native-profile-host-graph.log`.
This closes compiled profile/preset-selection acceptance, not Windows sandbox
installation/enforcement or experimental managed named-profile support. Only
acceptance code changed; published preview payload is unchanged.
Verification after the host checks: 582 workspace tests passed (12 explicit
opt-in tests excluded), including a new negative oracle for wrong model, effort,
speed, approval policy and sandbox in the frozen submission. All-target workspace
clippy passed with warnings denied. Logs: `central-native-profile-host-tests.log`
and `central-native-profile-host-clippy.log`. No additional inference was used
for those checks.

Native profile-transition acceptance (2026-09-07): production `start_thread`,
`start_turn`, `resume_thread` and `read_thread` constructors now have an explicit
isolated native-runtime probe covering eight turns across two independent native
threads. It alternates Luna/Sol, low/high/medium effort, Fast/Standard and all
three access presets, including the return from full access to read-only. Each
step checks the actual native settings readback, model-bound request, unchanged
cwd, exact history length and the sibling's previous profile. Configured concise
reasoning summaries remain inherited. The fixture replies only with fixed READY
text over loopback; no account inference, credentials, personal configuration,
model-generated command, file write or permission approval is involved.

Observed 0.153.4 normalization is asserted explicitly: `fast` becomes `priority`
in native settings and the model request; explicit null resets a previous fast
tier to native `default` and omits the model-request tier. Workspace-write
readback has no additional writable roots for the current cwd, while retaining
network-disabled and both temporary-directory exclusions. This verifies native
settings transitions, **not OS sandbox enforcement or the compiled UI picker**.
Those separate host/Windows acceptance gates remain open. The shared completed-
turn helper still rejects tool items/authority requests and checks persisted READY.

Command: `cargo test --locked -p central-agent-codex-runtime
native_profile_transitions_apply_model_effort_speed_and_access -- --ignored --nocapture`.
Log: `%TEMP%/central-native-profiles.log`. All 99 ordinary runtime tests passed
(10 opt-in probes excluded), as did the explicit configured-summary probe and
all-target runtime clippy with warnings denied. Logs: `central-native-profiles-unit.log`,
`central-native-profiles-summary.log`, `central-native-profiles-clippy.log`.
No production code/UI or release payload changed in this increment.

Permission-profile source audit (corrected by the native boundary probe above):
the official App Server guide makes named `permissions` selection beta with
`experimentalApi` opt-in; it is absent from the selected stable ThreadStartParams.
The separate `permissionProfile/list` method is present in the generated stable
ClientRequest union and actually works without opt-in. Do not invent a stable
named-selection adapter or silently replace
managed named profiles with preset sandbox modes. The existing fail-closed
managed-profile behavior remains; this is not a claim of named-profile support.

Native token-usage host acceptance (2026-09-07): extended the existing two-turn
main and concurrent three-turn graph acceptance with an opt-in `--usage` check.
It observes actual thread/tokenUsage/updated events for owned native thread IDs,
checks each turn reported usage, then compares the compiled main/card DOM with
the native report: all six latest/thread-total counter rows, reported capacity,
percentage and progress meter. The independent oracle uses the latest total for
capacity, never lifetime totals or re-added cached/reasoning counters. It handles
missing values, zero capacity and over-capacity display separately in unit cases.

Both actual desktop hosts passed. After successful continuation and native history
readback, the check closes only its owned App Server connection. Actual EOF handling
must retain the exact displayed counters and mark them stale/disconnected in every
owner. No counter event or UI usage update is fabricated. The graph case also
retains the existing concurrent A/B checks, independent A continuation and B draft.
Five short model prompts were announced beforehand (two main, three graph); no
additional inference is submitted by the usage check. Exact disposable native
histories were deleted by existing scoped cleanup. Logs:
`%TEMP%/central-native-usage-main.log`, `central-native-usage-graph.log`.
Command against the newly compiled development executable:
`target/debug/central-agent.exe --check-native-conversation --usage --allow-test-inference`
(add `--graph` for the card pair). This is hidden DOM functional acceptance, not
visual inspection. Production counter projection/UI were unchanged.
The independent oracle test passed. Clippy requested an explicit string
comparison for HTML data-stale rather than an allocated boolean string; this
test-only comparison was corrected without changing the production component.
Final verification passed: 581 workspace Rust tests plus three review/permission
probe-oracle tests (584 total), 11 opt-in ignored, workspace all-targets clippy
with warnings denied and cargo format. Logs: `central-native-usage-unit.log`,
`central-native-usage-workspace-tests.log`, `central-native-usage-clippy.log`,
`central-native-usage-review-oracle.log`, `central-native-usage-permission-oracle.log`
under `%TEMP%`. No frontend source changed, and no additional inference was used
by those regression checks. The published 16:39:35 preview is unchanged: this
increment adds opt-in acceptance and corrects the plan, not end-user behavior.

Checklist reconciliation: the earlier basic native history/import gate was already
closed by the four recorded main/graph Link/Fork host runs. Their six history and
branch log files were re-read along with current import guards; stale milestone
wording is corrected below. No duplicate history inference was run for this audit.

Compiled desktop uncertain-delivery restart (2026-09-07): the new opt-in
`--check-native-conversation --restart --restart-crash --restart-lost-ack --allow-test-inference`
mode now passes on both main and `--graph` hosts. The first actual native turn
completes, but its real successful worker reply is held before the production
host handles it. The native process did accept the turn; this is host-boundary
ACK loss, not a claim of whole-app wire fault injection. The host retains its
write-ahead receipt while newer text drafts are saved through ordinary UI/IPC.
The parent forcibly terminates only its owned application process after checking
the durable receipt and saved main/sibling drafts. Native child cleanup is checked
through Windows process ownership, without test-side native termination.

A second independent application process uses production loaders: native binding,
the exact unresolved receipt and unsent draft must exist before any history read;
the history mirror is initially empty, permissions are read-only and no prompt is
replayed. The actual scoped Check native history control then reads native state.
Only exact native clientId correlation or explicit review can clear the receipt;
the actual Resume connection and composer continue the same thread. Final native
readback has exactly two original user inputs; no legacy transcript copy. The
graph sibling's saved draft remains unchanged and it receives no run or request.

Logs: `%TEMP%/central-native-restart-uncertain-main.log` and
`central-native-restart-uncertain-graph.log`. Four short real model submissions
were announced beforehand (two per host), with no tools or file changes requested.
The parent removed only its two exact disposable native histories and owned test
profiles. No visual test. The old fresh-store acceptance check was narrowed to
allow exactly one pending receipt only for this validated second-stage fixture;
production loading/receipt logic was unchanged. Whole-app wire-level ACK loss
remains distinct from these passed whole-app host-boundary checks and the passed
runtime/book wire-loss checks below.

Full verification and publication for this increment passed: 583 Rust tests,
11 opt-in tests ignored in the ordinary suite, 140 frontend tests, generated
contracts, format/check/clippy and scoped audit (the same 13 allowed warnings).
The focused uncertain-restart oracle test also passed. Full pipeline log:
`%TEMP%/central-native-restart-uncertain-release.log`. The actual optimized staged
EXE passed hidden-WebView startup on toolbar, agent/editor, graph and diff search.
Published preview: `outputs/codex-app-server-preview/CentralAgent.exe`,
2026-09-07 16:39:35 local, 32,365,568 bytes, SHA-256
`e47226b882d025dfb77afaa8c7821a99e8084775b72572a80615c606046c2dda`.
All five manifest records independently verified at 2026-09-07T14:40:01Z.
Unsigned development build, external official CLI 0.153.4 still required.
This publishes the new acceptance mode, not a claim of a completed objective.

Native wire-level pre-ACK recovery acceptance (2026-09-07): added an opt-in test
against actual App Server 0.153.4, with a test-only Node stdio relay and loopback
Responses fixture. The relay forwards the native protocol unchanged until the
second turn/start, then either drops that request before forwarding, or forwards
it while dropping its whole return stream, including the real wire ACK. This is
not the earlier delayed worker-to-host callback case. The production Ticket must
remain pending until disconnect and then report delivery_unknown:true.

All four cases passed: main/graph conversation owners, request lost before native
acceptance or ACK lost after acceptance. A real seed turn first establishes native
persistence. The test writes the actual Saved receipt metadata, discards the
conversation book, stops the owned native process tree, restores the metadata and
starts a fresh official process. It refuses resume/dismissal before native history
inspection. Native readback has exactly the expected prompt/reply sequence and
directory; the accepted lost input correlates by clientId exactly once and clears
its receipt, while the unaccepted input requires explicit review of its exact
receipt. Both continue the same thread without replay. Actual model request counts
are two or three as appropriate, all to the owned loopback fixture. No account
inference, tool execution, personal config/history or UI test is involved.

Command: `cargo test --locked -p central-agent-codex-runtime native_wire_loss_before_ack_restores_exact_receipts_without_replay -- --ignored --nocapture`.
Evidence: `%TEMP%/central-native-wire-loss.log`. Ordinary runtime regressions also
pass: 99 tests, nine opt-in ignored (`central-native-wire-loss-unit.log`). The new
relay exists only under tests and does not introduce a Node production dependency
or modify production transport. This closes the runtime/book wire-loss check,
not the compiled desktop whole-app pre-ACK restart, UI draft recovery or graph
sibling-isolation acceptance. Those remain open; no narrow test is treated as a
full host gate. This test/documentation increment does not change the published EXE.
The final relay test was rerun after moving the shared Responses fixture to one
test-only parent module (clippy rejected loading the same file twice). Both native
wire-loss acceptance and the existing two-turn summary-inheritance regression
pass with that shared module. `cargo fmt --all -- --check`, Node syntax check and
runtime all-target clippy with warnings denied also pass. Logs:
`central-native-wire-loss-clippy.log` and
`central-native-wire-loss-summary-regression.log` under `%TEMP%`.

Native summary inheritance and reload evidence (2026-09-07): investigation found
that the production turn constructor unconditionally sent `summary:"auto"`.
TurnStartParams defines this as an override for this and subsequent turns, so it
defeated the selected native saved summary. The constructor now omits summary,
letting Codex inherit its session configuration. Explicit sandbox, approval,
model/effort/service selections remain unchanged. A regression checks omission
across all three access presets without weakening their policy fields.

A test-only loopback Responses fixture now lets the actual 0.153.4 process create
and persist native turns in an unauthenticated owned profile. It serves a fixed
READY reply; only the model backend is simulated, not App Server, configuration,
thread persistence or outgoing model requests. It stores only model/reasoning/
service metadata, rejects authorization headers and non-loopback requests, and
is compiled only for tests. No account inference, personal config/credentials,
native personal history or tools are used. The initial fixture had an inherited
nonblocking Windows socket; accepted connections now explicitly use blocking IO.

The separate inheritance acceptance passed two native turns through the actual
production constructor, observing concise summaries and verifying READY in stored
native history. Log: `%TEMP%/central-native-summary-inheritance.log`.
The full reload acceptance remains FAILING, with stronger evidence than the prior
empty-thread test: after two initial native turns, config/batchWrite with
reloadUserConfig:true saved detailed summary. Both persisted sessions resumed
with their original model/effort, no settings notification arrived in the 15s
observation, and subsequent native model requests still carried concise summary.
Log: `%TEMP%/central-native-reload-loopback.log`. No automatic retry or substitute
runtime mutation was added. This proves that this tested reload path did not
update summaries in 0.153.4, not that every possible dynamic setting is unsupported.
Keep the full gate open and the production UI's no-live-reload semantics honest.
Full workspace verification passed for this increment: 582 Rust tests, ten
opt-in tests ignored in the ordinary suite, 140 frontend tests, generated schema
contracts, Svelte/design checks, clippy and scoped audit (13 existing allowed
warnings). The ignored reload test is still a failing acceptance gate; it is not
counted as passed. The separately opted-in summary-inheritance test above passed.
Full pipeline log: `%TEMP%/central-native-summary-release.log`.
The optimized unsigned preview was then published successfully, including the
prior thread-settings report UI. `outputs/codex-app-server-preview/CentralAgent.exe`
is 32,357,376 bytes, SHA-256
`543c7dea121b2117dc1fc019d87a37d5e7ecc3aa226cd43b1404503d0da2dfa3`.
Release verification completed at 2026-09-07T14:12:39Z. The exact staged executable
passed hidden-WebView toolbar/agent/editor/graph startup and diff-search checks.
No visible UI was opened. CLI 0.153.4 remains external; this is a development
preview, not a claim that the full objective or native hot reload is complete.

Native thread-settings notification increment (2026-09-07): the previously
unhandled stable `thread/settings/updated` event now updates an allowlisted,
memory-only report for its exact owned loaded thread. Main and graph render it
in Codex conversation > Reported thread settings, separate from saved defaults
and composer selections. The report contains public profile/directory metadata
and policy-kind labels, never collaboration instructions or raw config/permission
objects. It does not grant permissions, change consent or start a turn. Late
events cannot revalidate unloaded/archived/deleted/foreign threads; disconnect,
unload and malformed updates mark old data stale. Native history hydration does
not refresh this report. Missing notifications remain explicitly not reported.

Current correction (2026-09-09): this historical notification-only behavior is
superseded for successful start/resume/fork operations. Their stable top-level
response fields now seed the allowlisted report immediately; an ordinary
`thread/read` still does not, and omitted fields remain not reported until an
actual settings event supplies them.

Three new Rust tests cover private-field exclusion, malformed payloads, main/graph
ownership, lifecycle freshness and unchanged durable state; 98 runtime tests
passed (seven opt-in tests ignored). Nineteen UI lifecycle tests passed, including
the new owner/freshness/no-intent case. Svelte/design check and frontend build
passed. A new complete ThreadSettingsUpdatedNotification fixture validates
against the selected runtime's generated schema; 63 outbound contract samples
and existing incoming fixtures also passed. Logs in `%TEMP%`:
`central-native-thread-settings-unit.log`, `central-native-thread-settings-contract.log`,
`central-native-thread-settings-frontend.log`. An initial test variable shadowed
its fixture helper and failed compilation; the helper call was corrected and
the full runtime suite passed. No native inference, personal config mutation or
visual tests. This closes notification handling in source, NOT real hot-reload
acceptance. The published 15:48:58 preview is unchanged by this increment.
All 140 frontend tests and all-targets workspace clippy with warnings denied
also passed (`central-native-thread-settings-all-ui.log` and
`central-native-thread-settings-clippy.log` in `%TEMP%`). The production host
emits a refreshed owner view on a malformed settings event as well, so invalidated
freshness is not left visible as current while only account diagnostics change.

Native compaction counting increment (2026-09-07): the shared main/graph Codex
defaults dialog now exposes the tenth public preference,
`model_auto_compact_token_limit_scope`, restricted to the generated native enum
`total` / `body_after_prefix`. Its explanatory copy distinguishes full active
context from growth after the carried compaction-window prefix. Native Codex
still owns compaction and accounting; no context-monitor reinterpretation,
capacity increase, automatic prompt or live reload was added. Existing frozen
save/clear confirmations, base-user target/version and owner guards are reused.

Actual isolated 0.153.4 acceptance passed: both values saved/read back, override
cleared, stale revision rejected without mutation, all unrelated configuration
preserved and synthetic private values excluded from the public projection.
Log: `%TEMP%/central-native-scope-runtime.log`. Zero model turns, native threads,
tools, personal config writes or visual tests. Fourteen focused Rust config/MCP
tests and eleven main/graph preference handler tests passed. Svelte check/build
passed with zero errors/warnings; DESIGN and the existing scenario were updated.
The embedded bundle is regenerated. Full verification and preview publication
then passed: 578 Rust tests, nine opt-in tests ignored in the ordinary suite,
139 frontend tests, Svelte/design checks, 63 serialized outbound protocol
contracts, clippy and scoped dependency audit with existing allowed warnings.
The ignored set includes the open loaded-settings probe, not nine verified
acceptance gates. Build log: `%TEMP%/central-native-scope-release.log`.

Published unsigned preview `outputs/codex-app-server-preview/CentralAgent.exe`:
32,325,632 bytes, SHA-256
`54b0175759c997737a05b56fcae1b720207b343828554d7a4fc5093dd310ceb8`.
All five manifest records were independently verified at 2026-09-07T13:49:22Z.
The staged executable passed hidden-WebView startup/editor/graph/diff-search
checks. Both ordinary 28-phase main/graph settings regressions passed again
against the published EXE, preserving drafts and testing native save/clear,
skills, confirmation/cancellation and privacy in isolated profiles. Logs:
`%TEMP%/central-native-scope-settings-main.log` and
`%TEMP%/central-native-scope-settings-graph.log`. Those host regressions exercise
the existing verbosity editor; both new scope values are independently verified
by the native protocol probe and shared component-handler test above. No visual
test, inference or personal configuration mutation occurred. CLI 0.153.4 remains
an external dependency. Other full-objective gates remain open.

Live config reload is explicitly NOT closed: an earlier isolated probe accepted
`reloadUserConfig:true` and verified saved settings, but observed no matching
settings notifications in its 15-second observation window. Both ephemeral and
persistent newly started threads returned `no rollout found` on resume before
their first persisted turn. This cannot certify active-session reload or static
default preservation; the opt-in loaded-settings probe remains failing/pending.
The fixture also established that saved `approval_policy="untrusted"` is rejected
at startup by 0.153.4; its setup uses `on-request`. This finding does not change
the separately supported thread/start read-only request contract. Do not expose
or claim verified live reload from this configuration-only acceptance.

Typed MCP option host acceptance passed (2026-09-07): the opt-in
`--check-native-settings --mcp-options` exercises the existing compiled option
editor and frozen confirmation against an isolated native profile. It covers
all 12 exposed keys across disabled STDIO/HTTP entries, including false, empty
lists, numeric values and environment-name maps. Every set and restore/clear
is first cancelled, then confirmed; native saved-user-layer readback must
preserve all other fields, including synthetic private fixture values. The test
rejects reloads, model turns, native thread creation and private-value exposure.
The actual host passed all 12 options and 24 confirmed writes, retaining the
draft and ending with the original saved configuration. HTTP environment-header
replacement removes the old mapped entry and restores it afterward; static
private headers and STDIO environment values remain unchanged and unexposed.
The native base-user file path is checked against the owned profile, and both
entries remain effectively disabled after each native read. No production option
behavior or UI changed. The 15:06 preview is unchanged; this increment adds an
opt-in acceptance mode to the locally compiled test executable.

Three test assumptions were corrected from actual failures: a stale open form is
disabled even when its Reopen button is ready; effective config can include null
defaults which are not saved overrides; native timeout numbers may be projected
as integral floats. Read the actual user layer to decide Restore versus Clear.
The comparison permits only an equivalent numeric representation for timeout
seconds; every unrelated field, false value and empty list is still checked.
Two regression tests prove those comparison boundaries. Actual final host log:
`%TEMP%/central-native-options-host-layer.log`. Workspace all-targets clippy passed
with warnings denied; the two oracle tests passed. Existing compiled-host MCP
CRUD/reload (22 phases) and ordinary preferences/skills (28 phases) both passed:
`central-native-options-crud-regression.log` and
`central-native-options-settings-regression.log` in `%TEMP%`.
All checks used owned isolated profiles, cleaned by their coordinators, with zero
inference and no visual tests. This closes typed-option host coverage, not OS
browser OAuth launch, advanced configuration or the other full-objective gates.

Unsent text persistence (2026-09-07): main drafts previously depended on
sessionStorage/memory; graph drafts were memory-only. The shared UI now saves
text through a Rust-owned, hashed-owner store with atomic replacement, an
exclusive nonblocking file lock and version checks. Corrupt/conflicting records
are preserved with an explicit error. No old-text backup is retained. Late
hydration and acknowledgements preserve newer typing; versioned empty records
prevent resurrection after clearing. Local deletion/ejection clears drafts,
while archive preserves them. This is plaintext local UI state, never native
history, reconstructed model input, queued work or restored permission consent.
Attachments, approval answers and OAuth credentials are excluded.

Actual hidden-host checks passed for main and graph using three fresh processes:
save multiline Unicode text, reopen and verify it, clear it, reopen and verify
the empty draft. Graph also retains an independent sibling draft throughout.
Both checks use isolated owned profiles and zero prompts; no screenshots or
personal configuration. Logs: `%TEMP%/central-native-draft-restart-main.log` and
`%TEMP%/central-native-draft-restart-graph.log`. This closes recovery of saved
unsent text across app exits, not attachment persistence or unacknowledged last
keystrokes during an abrupt kill. Four frontend regressions and four Rust store
tests cover hydration, correlation, conflicts, tombstones, corruption and locks.
Full verification and preview publication passed. The first full run caught a
source-contract assertion still requiring the retired in-component session cache;
it now checks the shared persistence bridge, keeping the existing editing/delivery
assertions. The full rerun passed 575 Rust tests (seven intentionally ignored),
138 frontend tests, Svelte/design checks, generated protocol contracts, clippy,
and the scoped dependency audit (13 existing allowed warnings). Hidden-WebView
startup/editor/graph/diff-search checks also passed. Build log:
`%TEMP%/central-native-drafts-release-recheck.log`.

Published unsigned preview `outputs/codex-app-server-preview/CentralAgent.exe`:
2026-09-07 15:06:24 local, 32,292,352 bytes, SHA-256
`6fd93bd223b0281785922a91405ecca8a1bb5c595b39f4b7a849ccf499fc2a28`.
All five manifest records were independently verified at 13:06:48Z. Both
three-process draft checks passed again against this published executable:
`%TEMP%/central-native-drafts-published-main.log` and
`%TEMP%/central-native-drafts-published-graph.log`. Each coordinator cleaned up
only its own temporary profile. Official CLI 0.153.4 remains external. These
results close the saved-text increment, not the remaining full-objective gates.

Bound-fork lifecycle correction (2026-09-07): the main/graph New conversation
fork path previously allocated its local destination before rejecting an archived
native source. The shared conversation layer now exposes a read-only source
preflight (bound, not deleted/archived, observed and not busy). The host calls it
before any chat/card allocation or persistence; native Action::Fork rechecks it
before creating a request or pending origin. The UI disables Fork for archived
sources, rejects /fork, and closes an already open fork/review confirmation when
an archive event arrives. Confirmation revalidates the same source; restoration
is never implicit. Other providers, original drafts and native history remain
unchanged. Existing import handling retains its separate archived-source guard.

Two new runtime tests cover main/graph archived preflight/direct dispatch and
missing, busy, disconnected and deleted sources, with no new binding/receipt.
Two UI tests cover archived button/shortcut intent and archive-during-confirmation
on both hosts. All 36 conversation tests and 18 lifecycle UI tests passed; the
full frontend suite passed 134 tests. Svelte check/build passed with no errors or
warnings (61 modular files, 32 scenarios). DESIGN and the history scenario were
updated. No model prompts or visual tests were used.

Full Release verification passed: 571 Rust tests, seven intentionally ignored,
134 frontend tests, Svelte/design checks, generated protocol contracts, clippy
and scoped dependency audit (13 existing allowed warnings). The staged executable
passed hidden-WebView toolbar/agent/editor/graph startup and diff-search checks.
Published unsigned preview `outputs/codex-app-server-preview/CentralAgent.exe`:
2026-09-07 14:39:27 local, 32,240,128 bytes, SHA-256
`79e49230786ba6d8eba286f39a2c2e18f1027cd42f7169525f2f63b5aa609194`.
All five manifest records were independently verified at 12:39:41Z. Build log:
`%TEMP%/central-native-fork-guard-release.log`. Official CLI 0.153.4 is still an
external dependency. This release includes the prior scoped OAuth acceptance
increment, but does not close the other goal gates or claim full parity.

Native conversation OAuth host acceptance (2026-09-07):
`--check-native-settings --mcp-oauth --conversation-oauth [--graph]` now drives
the compiled main/graph Conversation MCP controls. Each coordinator owns an
unauthenticated temporary native profile, loopback service and one ephemeral
native thread. It observes actual inventory, explicit login and URL-open intent,
PKCE exchange, exact thread-scoped completion and authorized/connected readback.
The production owner/thread/attempt URL validator runs before the test substitutes
only OS browser launch with fixture consent. The target draft remains unchanged;
graph siblings keep their drafts and gain no native binding. No model turn,
tool invocation, resource read, personal credential or persistent native history
is used. The temporary profile and fixture child are cleaned up by their owners.

The first main-host attempt exposed a test timing assumption: successful OAuth
precedes the native MCP connection restart. Reading during restart can correctly
be invalidated by startup events, leaving an explicitly stale inventory. A trace
confirmed this ordering; the fixture now waits for the actual scoped `ready`
notification before clicking Refresh. No production auto-refresh, retry, second
login or fabricated authorized state was added. Both final checks passed:
`%TEMP%/central-native-mcp-oauth-main-ready.log` and
`%TEMP%/central-native-mcp-oauth-graph.log`. The global six-phase OAuth/reconnect
regression also passed (`central-native-mcp-oauth-global-regression.log`).
All-targets workspace clippy passed with warnings denied; all 16 focused MCP
state/ownership regression tests passed. These are hidden DOM
functional checks, not visual tests. Changes are opt-in acceptance code and
validator visibility only; the published 14:08 preview is unchanged. Actual OS
browser launching, typed-option host coverage and the other milestone gates
remain open. Ephemeral scoped sessions are not claimed resumable after restart.

Native global OAuth host acceptance (2026-09-07):
`--check-native-settings --mcp-oauth` drives the compiled account Settings
inventory, Authorize OAuth, explicit Open authorization page intent and Reconnect.
The same production attempt/URL validator runs; only the external OS browser
launch is substituted by consent at the owned loopback OAuth fixture. Actual
App Server discovery, dynamic registration, PKCE exchange, completion notification
with global threadId:null and auth inventory readback are verified. No URL query
values or synthetic credentials enter the DOM. The draft stays unchanged.
After clicking the actual Reconnect control, the new native connection reports
the persisted authorization without a second login, browser intent or token
exchange. Native credentials use file storage in the newly owned profile, never
personal files or keyring. No model prompt, tool invocation, resource read or
native/local conversation creation occurs. Fixture child and temporary profile
are cleaned up by their respective owners.

The final six-phase check passed, log
`%TEMP%/central-native-mcp-oauth-reconnect.log`; the earlier four-phase initial
flow passed in `%TEMP%/central-native-mcp-oauth-host.log`. All-targets workspace
clippy passed with warnings denied; Release compilation and the 28-phase ordinary
settings regression passed too. Zero inference prompts and no visual tests.
Only opt-in acceptance code/docs changed; the published 14:08 preview is unchanged.
This does not prove the OS default-browser launcher or main/graph thread-scoped
OAuth host flow. Those remain separate gates; no experimental fallback is added.

Native MCP Settings host acceptance (2026-09-07): the actual compiled account
Settings controls now pass disabled STDIO creation, cancelled/confirmed save,
enable, explicit cancelled/confirmed reload, actual fixture tool/resource
inventory, disable and cancelled/confirmed removal. Native readback verifies
every mutation, exact unrelated private fixture preservation and final absence
of the removed entry. Exactly four configuration writes and one reload occur;
the draft and local/native conversation stores remain unchanged. The trace
proves inventory reads without tools/call or resources/read. No model prompt,
external service authorization or personal configuration was used. The profile
coordinator removes only its own temporary fixture after child exit.

This exposed two test issues (Node rejecting canonical Windows verbatim script
paths, and stale sampled UI phases) and a production confirmation race: a queued
dialog close could erase a newly opened MCP configuration/reload intent. Those
two components now discard closed intent only if their dialog remains closed;
native inventory/revision validation and explicit confirmation are unchanged.
Two new regression tests and all 132 frontend tests passed, Svelte/design checks
passed, and the real host reached all 22 phases after the fix. The final success
log is `%TEMP%/central-native-mcp-settings-host-closefix.log`.
Run with `--check-native-settings --mcp-settings`; this is the shared Settings
surface, so --graph is rejected, not claimed as graph-scoped coverage.
The full host check passed twice after the fix (repeat log:
`%TEMP%/central-native-mcp-settings-host-repeat.log`). Full Release verification
passed: 569 Rust tests, 7 intentionally ignored, 132 frontend tests separately,
zero Svelte errors/warnings, schema checks, clippy and scoped dependency audit
(13 existing allowed warnings). Hidden-WebView startup/diff-search checks passed
without visual inspection. Published unsigned preview:
`outputs/codex-app-server-preview/CentralAgent.exe`, 32,123,392 bytes, SHA-256
`7f2532c443c62d06c1a1e11136901ff5afbf745ba8a786b9301d118f10f846cb`,
manifest verified 2026-09-07T12:08:51Z. Build log:
`%TEMP%/central-native-mcp-settings-release.log`.
OAuth host acceptance, typed-option host coverage and the other gates remain
open; the earlier native OAuth/option probes do not substitute for them.

Native preferences/skills host acceptance (2026-09-07): the opt-in
`--check-native-settings [--graph]` test now drives the actual compiled Svelte
dialogs and Rust dispatch against official 0.153.4 in an owned, unauthenticated
temporary native profile. Both main and graph passed all 28 phases: read,
cancelled and confirmed preference save, native readback and refresh, cancelled
and confirmed clear of that override, skill disable/re-enable, and explicit
next-prompt selection/removal. Exactly two preference writes and two skill writes
occur per successful full invocation. Native config readback verifies the user
override is absent after Clear and unrelated synthetic private configuration is
preserved. Private fixture values and skill instructions never enter the UI;
the unsent draft stays unchanged and no native thread/model turn or local
transcript is created. Zero inference prompts were used in this increment.

The previous attempt timed out because the test sampled document.textContent
(null in the main host); sampling now uses document.body and reports JavaScript
errors explicitly instead of waiting without diagnostics. This was a test-only
fault. Full successful logs: `%TEMP%/central-native-settings-main-complete.log`
and `%TEMP%/central-native-settings-graph-complete.log`. Each coordinator removed
only its own temporary profile after its child exited. Release compilation and
all-targets workspace clippy with warnings denied passed. No production UI
change or visual test was made, and the published 13:34 preview is unchanged;
this increment adds acceptance coverage, not a new user executable. Full MCP
configuration/OAuth host acceptance and the other remaining gates are still open.

Internal-host correlation fix (2026-09-07): the actual history dialog exposed
`crypto.randomUUID` as undefined in WebView2 NavigateToString (`isSecureContext`
false, `getRandomValues` available). Replace all four UI callers in History,
Skills, Preferences and ProjectDiff with one cryptographic UUID-v4 helper.
No origin/IPC change, insecure random fallback or extra provider engine. The
regression suite runs with a crypto object that has no randomUUID; all 130
frontend tests passed, including existing history/skills/preferences handlers.
Svelte/design checks and 34 focused Rust conversation tests passed. Full release
verification is being completed below.

The initial zero-inference history fixture was unsuitable: shell-only/injected
items could be read from the loaded runtime but did not provide discoverable,
archivable persisted conversations in 0.153.4. Those preparation paths were
removed from the test. `--check-native-history --allow-test-inference` now uses
two explicitly opted-in one-word turns in owned native histories and an isolated
desktop workspace. Every list/search query is restricted to the test UUID name
prefix, and cleanup deletes only the exact returned test IDs. No private auth
files or personal conversation contents are inspected; production configuration
is unchanged.
Import itself must send no new turn. Main archived Link passed including search,
empty results, archive filtering, cancelled selection, explicit confirmation,
exact native identity/readback and preserved draft. The runtime explicitly
refused archived Fork with RPC -32600 and a requirement to unarchive first.
The UI now disables archived Fork with guidance; the Rust conversation layer
rejects it before creating a request or pending fork receipt. No automatic
restore, alternate command or retry was added. The pre-existing fork unit test
incorrectly combined an active fixture with archived=true; it now tests active
forks and has a separate archived-refusal regression.

Main and graph both passed Link of archived history and Fork of active history,
including title search, no-match results, exclusive archive filtering, disabled
archived Fork, cancelled selection, explicit confirmation, exact resulting native
ID/history and preserved draft. No import sends a model turn. Twelve short
prompts were used in total: eight for the four successful cases and four for the
two archived-fork refusal discoveries. All exact owned histories were deleted.
The initial main-link log's old footer incorrectly says no inference; its two
seed-prompt lines and this accounting are authoritative, and the footer is fixed.
Logs under `%TEMP%`: `central-native-history-main-real.log`,
`central-native-history-main-fork-fixed.log`, `central-native-history-graph-link.log`,
`central-native-history-graph-fork-fixed.log`. This closes the previously open
basic browse/import host gate, not every other native feature or the objective.

Full Release verification passed after these corrections: 569 Rust tests,
7 intentionally ignored, 130 separately run frontend tests, Svelte zero errors/
warnings, generated-schema contracts, clippy and the scoped dependency audit
(13 existing allowed warnings). Hidden-WebView startup and diff-search checks
passed without visual inspection. Published unsigned development preview:
`outputs/codex-app-server-preview/CentralAgent.exe`, 32,048,128 bytes, SHA-256
`592E2459DD9B792BF79C79E38606F545CB0700B6E9D2F77F7176FA30A92393B6`,
manifest verified 2026-09-07T11:34:20Z. Other native acceptance gates remain open;
the overall goal is still active.

Native fork navigation/continuation acceptance (2026-09-07): extended the
existing `--lifecycle` host test, not the production agent implementation.
Actual Open branch now opens the new project chat or same-node graph card,
renders copied native history and accepts a second prompt through that branch's
actual composer. Native readback proves the source still has exactly one turn
and the fork has exactly two, with the exact independently submitted inputs and
unchanged cwd. New branch access starts read only. Returning through the actual
Projects chat button restores the main source's unsent draft; the graph source
card remains open with its draft throughout. The original lifecycle/dependency
rejection checks then run to completion and remove both owned native histories.

Both hosts passed. Five announced short prompts were used: two in the successful
graph run, one in the first main attempt and two in the corrected main run.
The first main check sampled the old DOM after the Rust owner had already
changed; the test now correlates the rendered owner too, without changing the
production boundary or hiding an actual draft mismatch. Logs:
`%TEMP%/central-native-branch-main-2.log` and
`%TEMP%/central-native-branch-graph.log`. No visual tests or production UI changes.
The existing published 12:44 preview remains unchanged; only opt-in acceptance
code and documentation changed in this increment. Native history browse/import
remains an open gate; Open branch and independent continuation are now verified.
Workspace compilation and all-targets clippy passed with warnings denied. The
native acceptance executable was built in Release mode. No new published release
was needed because this increment changes only opt-in tests and documentation.

Native lifecycle host acceptance (2026-09-07): the actual shared main/graph
buttons and dialogs passed Rename, Archive, Restore, Fork into a new local
conversation and Delete against 0.153.4. Native fork readback preserves exact
seed input and cwd; source identity, draft and other-provider storage stay
unchanged. The real runtime refused deleting a source still referenced by its
fork. The shared confirmation now explains this dependency; an added component
test verifies rejection preserves the binding and requires a new explicit
confirmation. There is no automatic dependent deletion or retry.

The corrected acceptance runs passed on both hosts. Four short, announced
prompts total were used including the first two exploratory runs: one discovered
the native deletion dependency, and the other exposed a harness attempt to
address an unopened main chat. The latter was fixed in the test, preserving the
production selected-owner boundary. The test independently reads its exact new
fork via the official API, explicitly removes that owned fork, then retries
source deletion through the actual source dialog. All owned histories were
deleted. No visual test, personal configuration change or real workspace write.
Browse/import and actual Open branch navigation remain separate gates. Full
workspace/Release verification passed: 568 Rust tests (7 intentionally ignored),
16 lifecycle component checks, zero Svelte errors/warnings, 62 outbound native
schema cases, clippy and scoped audit (13 existing allowed warnings). Staged
hidden-WebView startup also passed, with no visual tests. Published preview:
`outputs/codex-app-server-preview/CentralAgent.exe`, 31,975,424 bytes, SHA-256
`495EB6D382D70CFA8BFCBFA5CE2C04CE2923B327D3FAFE9296D76F7FDF196F2A`,
manifest verified 2026-09-07T10:44:52Z. The objective remains active.

Native MCP option editing (2026-09-07): Settings now
offers one-key editing for existing base-user STDIO/HTTP entries. The observed
saved transport governs eligible keys even when another layer changes the
effective transport. Executable/arguments/directory, endpoint/environment-name
references, header environment-name maps, startup/tool timeouts, required startup
and tool allow/deny lists use typed values and an exact versioned native leaf
write. Optional overrides have a separate Clear confirmation. No timeout, filter,
reload or model turn starts implicitly. Old raw values/credentials never enter
the UI; confirmations explain that unedited credentials/options remain and can
be reused after a command or endpoint change. Unknown keys and raw credential
maps are rejected, not passed through as an arbitrary configuration editor.

The isolated real 0.153.4 App Server test passed all 12 setters, optional clears,
whole-map replacement, stale revision rejection, unrelated private-value
preservation and exact initial configuration restoration. Both temporary fixture
roots were removed; no personal profile, MCP process or model was used. Added
Rust validation/transport-scope tests, UI parser/view/confirmation tests and three
outbound generated-schema cases. Full workspace/Release verification passed:
568 Rust tests, 7 intentionally ignored, zero Svelte errors/warnings, 62 outbound
native schema cases, UI tests, clippy and the scoped dependency audit (13 existing
allowed warnings). The isolated native configuration test also passed separately.
The staged hidden-WebView startup passed; no visual tests were performed.

Updated unsigned preview: `outputs/codex-app-server-preview/CentralAgent.exe`,
2026-09-07 12:25:33 local, 31,983,616 bytes, SHA256
65622EAB9A0420CBEC690DC1718FEDBA82AE9F562F5FF0E18E58CF1D572B5316.
All five release-manifest records were independently verified after publication.
Transport-kind conversion, arbitrary secret editing and broader native gates
remain distinct from this change; the objective stays active.

Native generated-image presentation (2026-09-07): the selected
0.153.4 ThreadItem/ImageGenerationItem contract now projects completed encoded
PNG/JPEG results into separate stable main/graph media rows. They stay outside
the work disclosure and do not replace the native final answer. Shared Svelte
presentation supports explicit enlargement, preserves the open preview across
same-result updates and reports decode failures. Rust validates format, encoded
size and dimensions without reading savedPath or fetching result URLs; native
history is not modified. Tests cover projection/live-history identity, rejected
sources, completed/failure distinction and shared timeline ownership. Hidden
startup component checks exercise decoding, enlargement, refresh, closing and
fallback in both hosts without inference. Full verification passed: 566 Rust
tests, 7 intentionally ignored, zero Svelte errors/warnings, 59 outbound native
schema samples, component tests, clippy and dependency audit (13 previously
allowed warnings). The first startup attempt correctly caught the test's own
synthetic decode-error event in its global error listener. The harness now
excludes only that exact injected Event identity; all other startup errors still
fail, and both hidden host checks passed on the complete second pipeline.
No inference, personal data reads, visual tests or provider configuration writes
were needed. This does not claim live account image-generation acceptance or
support for every MCP/audio/artifact output format.

Updated unsigned preview: `outputs/codex-app-server-preview/CentralAgent.exe`,
2026-09-07 12:13:19 local, 31,931,904 bytes, SHA256
A4064E9F2F8C9644827D7389E19F313694A10ECFB6304A57D355955BA2BB570D.
The five release-manifest records were independently verified after publication.
The full objective remains active; other native configuration, lifecycle and
acceptance gates below are not closed by this increment.

Native abrupt app-crash acceptance (2026-09-07): `--restart-crash` requires
`--restart` and implies active-stream mode. The parent validates its spawned
child against a token-bound readiness proof and uses only that owned Child
handle to force termination. A synchronize-only native-process handle, acquired
before host termination, proves that production kill-on-close ownership also
ends the native server. No normal CloseRequested handler, late app-state flush,
test-side native interrupt or fabricated transport event is used. Fixture
project/chat metadata is saved before inference, matching an already saved
project, while normal production writes own native binding persistence.

Main and graph passed in separate two-process tests using four announced prompts
total. The second process acquires storage normally, loads the original binding,
uses native read/resume through actual UI controls and continues without replay
or graph crossover. Parent cleanup deleted only native histories
`<disposable-test-thread>` and
`<disposable-test-thread>`; explicit TempDir close also verified removal
of both owned profiles. A new proof regression rejects mismatched PID, native
PID, readiness and identity fields. No production recovery change, personal
config write or visual test was needed. Release verification passed: 564 Rust
tests, 7 intentionally ignored, frontend checks and staged hidden WebViews.
The 11:58 preview was published with SHA256
4B3A0ECF7C76F9D63857CA4F0BBC8BA29E5A43EC0E155F3D415343F07E418E0E.
Unacknowledged app-crash input,
physical pre-ACK wire loss, cross-exit draft persistence and other objective
gates remain open, not implied by this accepted-stream crash acceptance.

Native active-close restart acceptance (2026-09-07): `--restart-active` extends
the existing opt-in two-process restart test and requires `--restart`. The first
actual desktop host waits for accepted, streamed native output, verifies the
on-disk native binding and invokes production normal window-close handling.
No synthetic completion or test-side interrupt precedes close. The second
process uses the production loader and actual Load history/Resume controls,
requires the same thread/first turn/cwd, keeps its new unsent draft while reading,
and sends one explicit continuation. Exact original and new user inputs must
appear once each in final native history, without legacy transcript copies or
another graph card becoming active. The proof ties both processes to the same
test owner, native ID and active-close mode, but distinct PIDs/application sessions.

Both main and graph passed with four announced native prompts in total. Parent
cleanup deleted only `<disposable-test-thread>` and
`<disposable-test-thread>` through native APIs after both application
processes exited. No production recovery fix, personal config write or visual
test was needed. Full release verification passed: 563 Rust tests, 7 intentionally
ignored, zero Svelte errors/warnings, 59 outbound schema samples, component tests,
clippy and scoped audit (13 existing allowed dependency warnings). A nested-if
lint in the new test observer was simplified before the complete successful
rerun. Staged hidden-WebView startup/interaction also passed.

Updated unsigned preview: `outputs/codex-app-server-preview/CentralAgent.exe`,
2026-09-07 11:49:57 local, 31,892,480 bytes, SHA256
`BABA9643D4D9FFBCED5D2EA4C1438FE09ABA37405AD21E77F21E56023EB05627`.
All five release records reverified at 2026-09-07T09:50:09Z. External native
Codex CLI remains 0.153.4. This is not whole-goal completion.
Abrupt app-process crash, uncertain
whole-app restart, physical pre-ACK wire loss and cross-exit unsent draft
persistence are not implied by this normal active-close test.

Native PNG/concurrent-media acceptance (2026-09-07): the real desktop-host
file test now supports `--files-png` and mutually exclusive `--files-steer` /
`--files-queue`, all requiring `--files`. It drives actual composer Enter and
delivery buttons, validates the exact image data URL and selected text snapshot,
requires model recognition and native history/cwd, and retains the later draft
and file. Steering must use/acknowledge the original turn; Queue must be observed
held during the warmup and create exactly one following turn. The graph fixture
also keeps its sibling draft/files and main native history untouched.

Six announced tests consumed ten user prompts in total and passed on actual
Codex 0.153.4: PNG start main+graph, PNG steer main+graph, PNG queue main+graph.
Only these exact owned histories were deleted through native `thread/delete`:
`<disposable-test-thread>`, `<disposable-test-thread>`,
`<disposable-test-thread>`, `<disposable-test-thread>`,
`<disposable-test-thread>`, `<disposable-test-thread>`.
No model tools, personal config changes or visual tests were involved. The image
crate now explicitly enables PNG for the fixture encoder/decoder (it previously
enabled only JPEG); original attachment byte transport is unchanged. Unit tests
cover both formats and invalid mode combinations. Full release verification
passed: 563 Rust tests, 7 intentionally ignored, zero Svelte errors/warnings,
59 outbound native schema samples, component tests, clippy and scoped dependency
audit (13 existing allowed warnings). Staged hidden-WebView startup/interaction
passed; this is not a visual test or whole-goal completion.

Published unsigned preview: `outputs/codex-app-server-preview/CentralAgent.exe`,
2026-09-07 11:41:32 local, 31,890,432 bytes, SHA256
`BC22C08EFD98C33DECCF4A863A03CAF24503439F4E0918D1F47C681FA7D839A8`.
All five release records reverified at 2026-09-07T09:41:44Z.
Official Codex CLI 0.153.4 remains an external dependency.
OS picker interaction, generated media, advanced controls and the other
remaining objective gates are not implied by these passing cases.

Native MCP configuration increment (2026-09-07):
Settings now exposes explicit versioned inspection and confirmed disabled
STDIO/Streamable HTTP creation, enable/disable and exact base-user entry removal.
Commands/URLs/credential values from native configuration are not projected;
creation accepts environment variable names, not inline secrets. Native
`config/batchWrite` owns persistence, preserves the observed file/version and
does not reload running threads. Shared writes and active/queued Codex work are
guarded; disconnect and invalidated reads cannot revive an old confirmation or
replay a write. A pending write ACK is retained across shared invalidations.

The isolated actual-runtime add/toggle/remove and stale-revision test passed on
0.153.4 with no inference, server startup, OAuth or personal configuration change;
its owned profile/workspace were removed. Focused host and UI tests pass.
Five new outbound contract samples use the production validated MCP edit
constructors. Full workspace verification passed: 562 Rust tests, 7 intentionally
ignored, zero Svelte errors/warnings, 55 modular files, 31 scenario contracts,
59 outbound schema samples, component tests, clippy and dependency audit with
13 existing allowed warnings. The first pipeline attempt stopped on a
nonminimal-boolean lint; after simplification, the complete pipeline passed.
The optimized staged executable passed hidden-WebView startup/interaction checks.
No visual inspection or model inference was performed.

Updated unsigned preview: `outputs/codex-app-server-preview/CentralAgent.exe`,
2026-09-07 11:28:49 local, 31,686,656 bytes, SHA256
`4E48F081919DFB2293F3C63BD19717BE706FBDBA7636AB4E091FC61424C27C8E`.
All five release manifest records were independently reverified at
2026-09-07T09:29:02Z. Official Codex CLI 0.153.4 remains an external dependency.
This release includes the configuration controls, not completion of the goal.
Existing transport-option editing, advanced native configuration, in-turn MCP
and the remaining broader acceptance gates remain open. No visual tests.

Native session-approval acceptance (2026-09-07): the existing real hidden desktop
approval harness now accepts the explicit, mutually exclusive `--session` choice.
It opens the real session disclosure and clicks **Allow for session**; the
original owner/ticket and actual typed UI decision must match. Answer delivery
and `serverRequest/resolved` now require the exact native callback key/request ID,
not merely a notification for the same thread. These stricter checks also apply
to the existing once/decline/cancel modes. The focused choice regression covers
invalid mode combinations and prevents treating once approval as session approval.

Four announced native model prompts passed on compiled Codex 0.153.4 desktop
hosts: command/session main+graph and single-file/session main+graph. Each
requires the native successful item and exact stdout or disk patch, resolved
and removed request card, retained unsent draft, correct native history/cwd,
no other-provider transcript and no sibling/main crossover. All four exact owned
test histories were deleted via `thread/delete`:
`<disposable-test-thread>`,
`<disposable-test-thread>`,
`<disposable-test-thread>`, and
`<disposable-test-thread>`.
No policy amendment or personal config write was performed. These checks prove
session-choice routing and actual action completion, not persistence of a grant
after reconnect or the approval behavior of every later command: those semantics
belong to the native server. Policy-amendment and in-turn MCP checks, advanced
native controls and the rest of the objective remain open. No visual tests.

Full release verification passed: 555 Rust tests, 6 intentionally ignored,
Svelte checks/build and component/contract tests, clippy, scoped dependency audit
(13 existing allowed warnings), and staged hidden-WebView startup/interaction.
The permission-scope oracle is now included in ordinary workspace verification;
it does not run the opt-in model probe. Updated unsigned preview published at
`outputs/codex-app-server-preview/CentralAgent.exe`, 2026-09-07 11:00:03 local,
31,534,592 bytes, SHA256
`938364616FD3C731AB8E522E4FC34CBE75AF6675028979E59288D0927E97A14A`.
All five release manifest records were reverified at 2026-09-07T09:00:18Z.
Official Codex CLI 0.153.4 remains an external dependency. This is a verified
incremental preview, not completion of the broader objective.

Native availability audit (2026-09-07): one explicitly announced read-only
permission-denial prompt returned `PERMISSION_TOOL_UNAVAILABLE`; no permission
callback was observed or grant sent. The exact owned native conversation
`<disposable-test-thread>` was deleted via `thread/delete`.
This is not successful permission-callback acceptance. Subsequent paginated
`experimentalFeature/list` on the actual stable 0.153.4 connection, without
experimental opt-in, established:

- `request_permissions_tool`: `underDevelopment`, disabled.
- `default_mode_request_user_input`: `underDevelopment`, disabled.
- `tool_call_mcp_elicitation`: `stable`, enabled.

The new opt-in `permission_scope_probe` inspects native availability before any
thread/prompt, refuses unavailable or non-stable permission inference and never
enables flags. Its `--check-availability` mode completed without inference,
thread creation or config writes. The denial-only test path preserves exact
owner/turn/request IDs and verifies callback resolution, history and unchanged
temporary directories when a stable runtime actually provides the tool.
An exact-scope regression guards that fixture; this does not assert that the
unexercised real callback path passes. Ordinary command/file approvals and MCP
elicitation remain supported and separately verified. Keep handlers for protocol
requests, but do not make forcing these disabled experimental model tools a
release requirement. Revisit their real-turn acceptance when the supported
runtime exposes them stably. In-turn MCP, session/policy choices and other
previously listed stable gates remain open. Runtime crate/all-target verification
passed: 88 tests, 4 intentionally ignored, and clippy with warnings denied.
No production UI or binary changed
in this increment; the verified 10:38 preview remains the current executable.

Native MCP elicitation increment (2026-09-07): actual isolated 0.153.4 transport
passed six standalone form/URL accept/decline/cancel round trips, using the same
typed request/answer store. Actual hidden desktop main and graph hosts then
passed all 12 real Svelte card decisions without inference or external URL opening.
Tests verify owner/key resolution, exact returned typed content, removed cards,
empty native turn history, retained form/composer values and graph isolation.
Each coordinator creates/validates its own temporary Codex profile, token and
ephemeral thread; shutdown removes only that owned test profile. Four preliminary
host runs stalled because the test waited for thread idle while a decision was
pending; fixed in the harness, not by weakening production busy state. Native
startup-before-open-reply is retained by exact thread ID. UI form schemas/defaults
are initialized once per immutable ticket to preserve entered values on refresh;
new tickets start fresh. A component initialization regression and temp-scope
guard regression were added. The final main/graph rechecks also prove profile
cleanup; a Windows file-sharing regression covers bounded retry after child exit.
Eight earlier development profiles remain under the system temp directory after
failed best-effort cleanup; manual removal was policy-blocked and not bypassed.
These synthetic residues are `central-native-elicitation-8YGlgK`, `-CdGnL9`,
`-FiLpmo`, `-nwvCW1`, `-zCnLR9`, `-u4akx9`, `-g9lcjd`, and `-FWFa5I`
(each suffix after the same full prefix). No personal configuration was used.
Full verification passed: 554 Rust tests, 6 ignored (the new real-runtime probe
was run explicitly and passed), frontend checks/build and component tests,
54 outbound schema calls, 19 decision fixtures, 31 scenarios, clippy and scoped
audit (13 existing allowed warnings). The optimized preview passed staged
hidden-WebView startup and both six-case main/graph MCP checks on the published
EXE, including verified coordinator cleanup. All five manifest records were
reverified at 2026-09-07T08:38:33Z. Published preview:
`outputs/codex-app-server-preview/CentralAgent.exe`, 31,521,792 bytes, SHA256
`6F63BE87EFDFB09D95460D7A43009207D4AE9C69907DD2F317EEA486E8D2503A`.
Unsigned development build; external official Codex CLI 0.153.4. No inference,
visual tests, personal configuration changes or commits in this increment.
This does not close in-turn permission/questions, session/policy decisions,
MCP configuration/editor/OAuth host acceptance or the broader objective.

File-change acceptance increment (2026-09-07): the actual desktop-host approval
fixture now supports `--approvals --file-change` on main/graph with Allow,
Decline and Cancel. Exact disposable path/diff validation precedes any answer;
rendered approval/timeline diffs, unchanged or exactly changed disk, native
resolution/readback, draft/sibling isolation and cleanup are required. Four
Allow/Decline probes passed. Two preliminary Cancel probes exposed an incorrect
declined-item test assumption; two corrected probes passed with authoritative
interrupted turn, unchanged file and actual Cancel answer. All eight histories
were deleted, including both failed probes. Native inProgress/failed file items
at cancellation are preserved, not rewritten as successful changes. A display
fix prevents unfinished activities remaining live after a terminal turn and
labels patches proposed until native completion. Already completed status-less
activities remain completed even when their parent turn later fails or stops.
Full verification passed: 552 Rust tests, 5 ignored, frontend checks/build,
54 outbound schema calls, 19 native decision fixtures, component/DOM tests,
31 scenarios, clippy and scoped audit (13 existing allowed warnings). The
optimized preview passed staged hidden-WebView startup and all five manifest
records, reverified at 2026-09-07T08:07:32Z. Published preview:
`outputs/codex-app-server-preview/CentralAgent.exe`, 31,492,608 bytes, SHA256
`B525644B901FE335D1B67D3488F4D5CD0EAFCE9A002CB36BC41372B03BDF1D12`.
This is an unsigned development preview using external official CLI 0.153.4,
not final objective completion. No visual tests or commits were performed.

### 1. Protocol and transport foundation

- [x] Read official guidance; identify installed runtime; generate versioned TS
  and JSON Schema from that runtime (not from the retired integration).
- [x] New standalone Rust client module/crate: initialize once, await response,
  send initialized, then accept calls; concurrent requests and notifications.
- [x] Server-initiated requests have independent IDs and connection generation;
  stale approval responses cannot target a reconnected process.
- [x] Drain stdout/stderr without blocking the UI; process exit fails pending
  RPCs. No automatic replay of mutating requests or user prompts.
- [x] Unit/fixture tests plus a real local handshake with **no inference**.

### 2. Connection, account and model profile

- [x] Resolve an explicitly configured/bundled/PATH executable; report version
  and compatibility. No automatic downloads or user-global config edits.
- [x] Account read, ChatGPT browser/device-code login, cancel and logout through
  official account APIs. Tokens remain owned by Codex; logout warns that the
  runtime's account may be shared with the CLI.
- [x] Read model/list with pagination and configuration requirements. Populate
  model, effort and speed from actual catalog; never invent model IDs or limits.
  Account Settings and main/graph profile pickers now use the complete native
  catalog and requirements. Native runtime profile transitions and compiled
  main/graph picker/preset selection now pass the probes above. Native Windows
  sandbox setup/enforcement remains a separate gate below.
- [ ] Show actionable connection/login/config errors and explicit retry. Windows
  sandbox setup requires a user action, never an automatic elevation.
  Native elevated/unelevated setup now has explicit Settings confirmation and an
  asynchronous ACK/completion state machine. No live setup/elevation was performed
  during development; real native runtime acceptance remains to verify.

### 3. Persistent conversations and streaming

- [x] Map main chat or graph conversation IDs to official thread IDs. Preserve
  existing other-provider history separately when switching provider mid-chat.
  The native binding/receipt store, async thread-control IPC and owner-addressed
  event bridge, composer/provider selection and native timeline projection are
  implemented. Completion is backed by provider round-trip, lifecycle, concurrent
  graph, native crash and whole-app restart acceptance, not only the initial
  read-only text workflow. The diagnostic and advanced request gates below remain
  independent and open where their evidence is incomplete.
  On 2026-09-07 the compiled desktop main-chat host passed real native acceptance:
  two prompts through the actual Svelte textarea/Enter and scoped IPC, streamed
  native replies, one continuing thread, ACK-driven draft clearing, assistant DOM
  rows, authoritative native readback and no other-provider transcript copy.
  The exact disposable native history was deleted successfully. Graph acceptance
  also passed on 2026-09-07: two concurrent card submissions on distinct native
  threads/directories, continuation of A with B's unsent draft preserved, assistant
  DOM rows and both native history/cwd readbacks. Both owned histories were deleted.
  Main Queue/Steer/Stop/reconnect and once/decline/cancel command decisions in
  main/graph have since passed actual host acceptance (below), including graph
  Queue/Steer/Stop/reconnect. Real server crash after acceptance also passed in
  main/graph, including explicit resume and same-thread continuation. Other
  request kinds have their separate gates below. Whole-app restart with
  host-boundary unacknowledged delivery and saved drafts now passes above.
  Runtime/book wire-level pre-ACK recovery passes separately; compiled whole-app
  before/after wire-loss now also passes all four main/graph cases above.
  Restart after a completed turn passed in
  main/graph hosts on 2026-09-07 (below).
  Main/graph provider-selector round-trip acceptance passed on 2026-09-07 (below).
  Normal application close while a turn is actively streaming also passed with
  same-thread continuation in a second main/graph process (increment above).
  Forced app-process termination during acknowledged streaming also passed in
  main/graph, including native child cleanup and production startup/read/resume.
- [x] Start/resume/read/list/name/archive/unarchive/delete/fork using supported
  official methods. Only Central Agent-owned or explicitly imported threads may
  be mutated; never bulk-touch personal Codex history.
  Main/graph controls now expose read, rename, archive, unarchive and delete for
  their existing native binding. Lifecycle notifications update local metadata;
  deletion retains only a terminal ID marker to prevent accidental recreation.
  Native listing/import and fork-into-unbound-destination UI are now connected.
  Automatic new local chat/graph branch creation and end-to-end acceptance pass.
  The native lifecycle host increment above verifies actual main/graph rename,
  archive/unarchive, branch creation and source deletion confirmations, including
  fork-dependency rejection. Independent branch readback/cleanup uses native RPC;
  history browse/import passed in four main/graph Link/Fork host checks above.
  Actual Open branch and
  independent fork continuation now passed on main/graph (increment above).
- [x] Turn start, interrupt, native steer with expectedTurnId, local Queue.
  Clear a draft only after acceptance; ambiguous delivery requires reconciliation,
  never silent resend. No app-imposed task action/time budget.
  Native start, Stop, local queue and owner-scoped steering UI are connected.
  Steering freezes the displayed expectedTurnId and never falls back to a new
  turn or queue entry. Actual main-host text acceptance passed on 2026-09-07:
  steering acknowledged the observed first turn; Queue waited and ran once,
  retaining a newer unsent draft; Stop interrupted a separate streaming turn.
  Native history retained all three turns. Graph delivery has now passed the
  same real acceptance, with sibling isolation and cwd readback. Attachment-bearing
  Queue/Steer also passed on both hosts with frozen UTF-8/PNG inputs (increment above).
  Open/acceptance are owner-bound, and write-ahead receipts are saved before send.
  Loaded idle sessions now skip redundant resume and submit the actual next turn
  with frozen profile/cwd/access. Before-first-turn cancellation is recoverable:
  reuse the loaded session, or explicitly unlink a known unused binding after
  disconnect. Native history, drafts and files are not deleted or replayed.
- [ ] Timeline keyed by threadId/turnId/itemId. Render commentary, exposed
  reasoning summaries, tool output, plans, file diffs and final answer live.
  The previously unprojected native fileChange/patchUpdated event now replaces
  the same item's intermediate diff. Generated-schema fixtures and runtime/shared
  main/graph tests cover replacement, empty updates, identity, history-read races
  and authoritative completion; no patch application or new tool loop is added.
  Completed items are authoritative; preserve disclosure and scroll state.
  Existing main/graph timelines now render native items and reported duration.
  Native entered/exited-review items and live plan-step details are rendered.
  Web actions, collaboration/child status, function output, sleep and image
  generation metadata now use the shared disclosure too. Opaque payloads, hook
  instructions and media bytes are excluded from text details. Real native
  generated-image bytes now pass history, full decoding, shared main/graph
  projection and compiled media markup checks above. Additional standalone
  notifications and complete desktop-host end-to-end acceptance remain open.
  MCP progress notifications are now rendered as ordered plain detail on the
  same main/graph activity, never a native result/status mutation. Client/schema
  tests pass. The actual isolated 0.153.4 Code Mode → MCP test receives a progress
  token but emits no App Server progress event; its separate opt-in diagnostic
  correctly fails. All six existing native elicitation cases still pass. Do not
  count missing native callbacks as live progress acceptance or simulate them.
- [x] Reconnect by querying official state, not parsing rollout files or
  replaying a transcript. Unknown events remain diagnosable without crashing.
  The audit's missing diagnostic evidence is now corrected by the memory-only
  Settings disclosure above. Transport still forwards unknown methods unchanged;
  the client records only schema-owned names/fingerprints and counters, never
  model input or a synthetic lifecycle event. Real-pipe and compiled UI tests
  complement the prior actual native recovery checks below.
  Actual explicit main-host reconnect passed: a new native connection, real
  Load history/Resume connection buttons, same thread and unchanged three-turn
  history, preserved draft and no extra start. Graph reconnect also passed with
  same-thread/cwd readback and sibling draft isolation. Real server termination
  during an acknowledged streaming turn passed on both hosts, preserving draft,
  exact history and continuation without replay. Whole-app restart after a
  completed turn passed in main/graph using two separate application processes.
  Normal app close during streaming subsequently passed on main/graph hosts.
  Forced app crash during acknowledged streaming also passed in both hosts.
  App restart with uncertain host-boundary delivery and saved drafts now passes
  at the compiled desktop level above. Whole-app wire-level loss now passes on
  the published main/graph hosts, both before native receipt and after native
  completion with no client ACK. Native process cleanup, persisted uncertainty,
  newer drafts and explicit same-thread recovery were verified. Runtime/book
  wire-level loss also passed again independently with a loopback model fixture.
  Delayed worker-to-host acceptance has now passed in both main and graph:
  real old-generation replies cannot clear the persisted receipt or new draft;
  actual native history inspection precedes reconciliation and continuation.
- [x] Token usage uses official thread/tokenUsage events. Distinguish last-turn,
  accumulated usage and model context capacity; absent values stay unavailable.
  Native event projection and shared main/graph monitor are implemented and tested:
  the generated protocol calls the snapshot `last`, so UI says latest report,
  not an inferred whole-turn sum. Thread totals, partial/zero values and stale
  connection/turn state remain separate. Actual main/graph end-to-end usage
  acceptance now passes, including real disconnect freshness (increment above).

### 4. Native approvals, tools and configuration

- [ ] Route command, file-change, permission, user-input and MCP elicitation
  requests into the originating main chat/graph card. Only explicit decisions
  answer requests; handle resolution, interruption and stale IDs.
  Native request routing, response validation and shared Svelte cards are now
  implemented. Actual main/graph Allow once, Decline and Cancel request passed with
  one exact print-only native command, server resolution, successful execution or
  native rejection, card removal, retained drafts and history readback. Extended
  single-file Allow/Decline/Cancel acceptance has also passed on both hosts,
  including actual diff presentation and disk verification (increment above).
  Session-choice command/file acceptance passed on main/graph (increment above).
  In-turn MCP request cards now pass both compiled hosts above. Native command-rule
  persistence/restart and compiled-host command-policy buttons now pass; network
  amendments still have a separate acceptance gate. Dedicated
  model permission requests and default-mode user questions are runtime-gated
  experimental features, not enabled by this stable-only client (audit above).
- [ ] Use native Codex tools and native sandbox/approval policies for ordinary
  local coding. Display native file changes/diffs without a parallel custom
  action loop or Time Machine checkpoint engine.
  Read-only, workspace-write/on-request and confirmed full-access presets are now
  selectable per main/graph conversation, captured with queued prompts and reset
  on application restart. Managed allowlists filter presets; named managed
  permission profiles require their own native config integration, not fallback.
- [ ] Expose native MCP status, configuration/reload, OAuth and elicitation
  through the official API. Do not automatically register Central Agent custom
  tools or inject local knowledge instructions. Those integrations are deferred.
  Global and owner-bound thread inventories are now connected, including actual
  isolated runtime validation of scoped tools/resources and unknown-thread rejection.
  Global reload/OAuth, thread-scoped OAuth and owner-scoped elicitation are connected;
  scoped OAuth also passed an actual isolated loopback authorization/PKCE/completion
  probe. Settings now provides versioned disabled-server creation, enable/disable
  and exact user-entry removal, with an isolated real-runtime persistence/conflict
  test. Existing typed transport-option editing also passed the isolated native
  writer test (increment above). Advanced configuration and full
  production host acceptance remain pending.
  All 12 typed options now also pass the compiled Settings host, including
  confirmed set/clear/restore, cancellation and private-field preservation above.
  Compiled main/graph thread-scoped OAuth and global Settings OAuth/reconnect
  host checks now pass with isolated native profiles and loopback consent, as
  recorded above; OS browser launching is explicitly not covered by that fixture.
- [ ] Attach supported text/images using official UserInput types. Local file
  references require explicit selection; no automatic upload of secret files.
  Existing main/graph file selection now maps explicit frozen UTF-8/PNG/JPEG
  snapshots into native text/image inputs for start, Queue and Send now. Native
  image previews and acceptance cleanup are connected. Actual main/graph start
  acceptance passed on 2026-09-07 with UTF-8/JPEG recognition, changed-on-disk
  snapshot fidelity, newer draft/file preservation, image history and cwd readback.
  PNG start and attachment-bearing Queue/Steer subsequently passed on both hosts
  with actual input, recognition, history and draft/file preservation (above).
  OS picker interaction remains a separate gate, not implied by those tests.
- [ ] Provide supported skill/MCP/config/review controls on demand with generated
  request types. Slash commands are UI intents mapped to real APIs, not a claimed
  generic RPC for every CLI/TUI command.
  Inline native review is connected for four review targets in bound main/graph
  conversations, with explicit scope/usage confirmation and uncertain-delivery
  recovery. Actual inline review inference, terminal worker reconciliation and
  native history readback passed on a disposable no-tool fixture. Detached review
  is verified runtime-unavailable on newly created paginated threads; legacy
  delivery remains unimplemented/unverified, without forcing experimental history
  overrides. Compiled main/graph custom no-tool review now passes (increment above);
  Early native review Stop also passes in the published main/graph hosts (above).
  Native uncommitted/branch/commit execution now also passes on main and graph
  disposable Git fixtures (increment above). Preparation cancellation passed in
  both compiled hosts with a delayed real reply; Stop during a running native
  review command now also passes on the published main/graph hosts. Review tool
  approvals remain unverified: two diagnostic attempts produced no callback,
  and no client-side substitute or permission escalation was added.
  Native composer shortcuts now open the
  existing history, skill, preference, review and lifecycle controls, preserving
  their confirmations and original owner; they are not a generic CLI interpreter.
  Native per-directory skill inventory and confirmed shared enable/disable controls
  are now connected. Nine native public preferences now have layered inspection
  and confirmed revision-checked writes in main/graph. Explicit native skill
  selection now supports Start, Queue and Send now with frozen references.
  Confirmed removal of observed base-user overrides is connected through the
  same revision-checked writer; null deletion was verified on isolated Codex 0.153.4.
  Actual compiled main/graph preference save/clear and skill disable/re-enable,
  selection/removal, confirmation/cancellation and draft/privacy checks passed
  the isolated no-inference settings host acceptance above.
  Advanced configuration, live reload and additional native controls remain pending;
  no local skill or configuration-precedence engine is added.

### 5. Verification and handoff

- [x] Contract tests against generated schemas; fake subprocess tests for full
  duplex, chunked output, out-of-order replies, failed initialize, EOF, stale
  approvals and ambiguous delivery; cross-chat/graph ownership tests.
- [x] Frontend tests for streaming, authoritative final items, queue/steer,
  approvals, reconnect, drafts, usage and provider switching.
- [x] Real no-inference smoke test; minimal sandboxed end-to-end inference only
  when needed, with explicit indication of account usage. No visual tests.
  Repeated the official 0.153.4 handshake/account/requirements/paginated model
  read during the completion audit: seven catalog entries, no model request,
  no printed account details. Actual account-backed acceptance is documented
  separately above; this smoke check does not claim login/logout or OS setup.
- [x] Complete workspace verification and unsigned development Release. Update
  DESIGN.md, scenarios and integration guide with implemented behavior and limits.
  Current full pipeline and independently checked 22:02:50 preview are recorded
  above. DESIGN.md and the 32 scenarios describe the native client surfaces;
  their static contract checks pass. No visual verification is claimed. Other
  unchecked native/OS acceptance requirements remain open separately.

## Guardrails

The API supports a broad surface but not every feature is production-ready.
Do not interpret “official” as a guarantee of identical desktop product features.
Keep managed requirements authoritative. Never select dangerFullAccess or
externalSandbox as a fallback when sandbox setup fails. Do not expose raw RPC
dispatch to website content. Process shutdown/reconnect is not a reason to replay
an action, silently drop an approval, or mark an interrupted turn completed.

## Current status

Desktop process-restart acceptance (2026-09-07): opt-in `--restart`, optionally
with `--graph`, runs two distinct compiled application processes on one freshly
created temporary profile. The first uses the normal main-window CloseRequested
handler after native completion/readback. The second uses BrowserApp's production
session/workspace/provider/graph/binding loaders; no fixture recreates its native
binding or transcript. A different process/session ID, released storage lock,
unloaded/unobserved initial native state and read-only initial consent are required.
Actual Load history and Resume connection controls restore the original thread
and cwd without starting a turn. A new unsent draft survives those operations;
the second explicit prompt adds exactly one turn. Both final native readback and
empty legacy transcript stores are checked. This does not claim persistence of
unsent drafts across application exit; drafts here are entered after reopening.

Main and graph passed against official Codex 0.153.4. Successful owned histories
`<disposable-test-thread>` and
`<disposable-test-thread>` were deleted by the parent through native APIs
only after child processes exited. Two preliminary graph checks exposed test
fixture/sampling defects: an empty unused-card mission and observing DOM before
its readback update. Fixtures now pass production persistence validation before
inference; readback requires the actual rendered first answer and retained draft.
Their exact histories `<disposable-test-thread>` and
`<disposable-test-thread>` were also deleted. Six minimal prompts total
consumed account usage; no Claude inference, personal config change or visual test.
The test-only child profile requires a canonical direct temporary directory, UUID
ownership token and valid stage, with a focused regression. It is not a general
data-directory override. App restart during active or ambiguously accepted work,
physical wire loss and remaining request/media/configuration acceptance stay open.

Graph concurrent-delivery acceptance (2026-09-07): the existing actual-host
delivery harness now supports `--delivery --graph`. It opens two real cards,
submits only A through its actual Svelte textarea/Enter handlers, uses the real
Send now/Queue/Stop buttons, then Load history and Resume connection after an
explicit native reconnect. Read-only text-only acceptance passed: steering ACK
targeted the observed first turn; one queued input waited, ran once, and retained
a newer draft; Stop interrupted the third streaming turn; the same native history
retained all three turns and the original directory after reconnect. B's unsent
draft stayed intact, with no activity, requests or native binding in B/main.
Only the exact disposable history `<disposable-test-thread>` was deleted.
This probe consumed subscription usage (three native turns and one steering input),
without tools, personal file reads/writes, configuration changes or visual tests.
Crash/uncertain delivery, provider switching, media and other native request kinds
remain separate gates. Two deterministic tests cover sampling scope and the strict
directory-readback oracle; ordinary builds never run inference acceptance.

Native provider-switch acceptance (2026-09-07): `--providers`, optionally with
`--graph`, passed on actual Codex 0.153.4 through compiled hidden-WebView hosts.
Main uses the actual tetrahedron/provider menu; graph changes the provider select
and clicks Update. Both run Codex, select Claude, return to Codex and continue
the original native thread/directory. Drafts and the legacy transcript survive;
graph B keeps its own draft without a native binding, activity or pending request.
The Claude catalog/history are explicit local fixtures: no Claude process,
authentication or inference was started. This is not Claude runtime acceptance.
Actual outgoing native calls exclude the fixture; each turn contains only the
new prompt, and native readback contains exactly two user inputs. Local persisted
history retains the Claude fixture without copying native messages. Four minimal
Codex prompts consumed account usage. Exact disposable histories
`<disposable-test-thread>` and
`<disposable-test-thread>` were deleted successfully via native APIs.
No visual tests or personal configuration changes occurred. Full app restart,
wire loss and remaining request/media/configuration gates remain open.

Delayed native worker-ACK acceptance (2026-09-07): `--crash --lost-ack`, with
optional `--graph`, delays an actual successful first turn/start reply before
production host delivery, kills only the owned streaming server, and checks
persisted uncertainty plus visible owner-scoped history controls. After explicit
reconnect it delivers that exact old-generation reply and requires no receipt/
draft mutation. Native history readback must correlate the client ID or precede
explicit review dismissal; a second actual prompt then continues the same thread.
Both real tests passed and deleted only their histories
`<disposable-test-thread>` and
`<disposable-test-thread>`. Four minimal prompts consumed account usage;
no model state or protocol payload was manufactured, and no visual test ran.
Rust now rejects receipt dismissal without a successful current-connection read
of that owner. A restored-receipt regression rejects missing/foreign/other-owner
reads; the previous disconnection test and scoped DOM sample check also cover it.
This verifies the worker/host race, not physical wire loss or complete app restart.

Native process-crash acceptance (2026-09-07): main and graph each passed the new
opt-in `--crash` check on actual Codex 0.153.4. The test killed only its private
native server during acknowledged streaming, observed real EOF through production
transport/host, preserved an unsent draft and unloaded native state, then used
explicit reconnect plus actual Load history/Resume controls. The second prompt
continued the exact original thread and directory; authoritative readback retained
exactly two user inputs, with no replay or cross-provider transcript. Graph B's
draft, requests and activity remained untouched. Both owned histories
`<disposable-test-thread>` and
`<disposable-test-thread>` were deleted via native APIs.
The client exposes read-only owned process identity for diagnostics; it is not
exposed to browser IPC. Only the opted-in test uses the exact OS handle to kill
that child. Process identity lifecycle and the exact-user-input oracle have
regressions. Failing disconnected checks can reconnect solely to delete their
own history. No personal config/history changes or visual tests were made.
Pre-ACK ambiguity, full desktop restart and provider-switch acceptance remain open.

Native selected-file acceptance (2026-09-07): main and graph passed actual host
`--files` runs on official Codex 0.153.4. Each used one multimodal model prompt
and disposable text/JPEG fixtures, supplied at the production file-picker result
boundary. Actual WebView preview decoding, immutable selected snapshots after
disk modification, exact wire input, both independent model answers, newer
draft/file preservation across ACK cleanup and native readback were verified.
Graph B remained independent with no extra native conversation. The two owned
histories `<disposable-test-thread>` and
`<disposable-test-thread>` were deleted through native APIs.
No screenshots, personal history/configuration edits or custom model tools were
used. OS picker interaction, queued/steered attachments and generated output
artifacts remain separate gates. The shared graph post-picker loader retains
existing file-policy checks; its partial-error, owner and count behavior now has
a direct regression. The fixture JPEG and answer validator are unit-tested too.

Current unsigned preview: built 2026-09-07 at 09:45 Europe/Rome, full verification
passed (547 Rust tests/5 ignored; frontend, generated native contracts, clippy and
scoped audit with 13 existing allowed warnings). The staged hidden-WebView startup
check passed, and all five published manifest records were reverified.
`outputs/codex-app-server-preview/CentralAgent.exe` is 31,476,224 bytes;
SHA256 `7ae620cef586d3e420c7e4e72372d734dc7fce3459a1ce22dc92a20abc30ec20`.
Requires official Codex CLI 0.153.4. Earlier preview records below are historical.

Native goal increment (2026-09-07): main/graph controls and `/goal` now expose
native get/set/clear with a shared Svelte dialog, live reported accounting,
paused creation, optional unlimited-by-default budget, status changes and removal.
Frozen confirmations and a native preflight read guard edits; no atomic CAS or
custom continuation engine is claimed. Goal writes briefly participate in owner
busy state, and disconnect/late-event handling preserves exact scope without
replay. Five host state tests, two core tests, six UI handler tests and six native
response/notification schema samples cover this increment. The actual 0.153.4
paused-goal probe passed using production constructors/decoders and removed its
exact disposable thread `<disposable-test-thread>`; no inference or
personal configuration changes occurred. Activation/execution and complete host
goal acceptance remain open.

This increment's unsigned preview was built on 2026-09-07 at 08:17 Europe/Rome.
Full workspace verification passed: 536 Rust tests/5 ignored, zero Svelte errors
or warnings, 54 outbound native schema samples and 19 decision samples, frontend
regressions, clippy and scoped dependency audit (13 previously allowed warnings).
The exact staged executable passed hidden-WebView startup and diff-search checks;
the published release manifest was verified again. No visual test or model
inference was performed in this increment. Current executable:
`outputs/codex-app-server-preview/CentralAgent.exe`, 30,947,840 bytes,
SHA256 `c863f9d69718b6b23e6262d5c14e35336d449f665e48169e994e130005eb7f57`.
Official Codex CLI 0.153.4 remains required. Older preview records below are
historical, not the current binary; overall objective acceptance remains open.

Native main/graph decision matrix (2026-09-07): the same opt-in approval harness
now supports `--graph`, plus optional `--decline` or `--cancel`. These decisions
are mutually exclusive and require `--approvals`; delivery mode cannot be mixed
with them. Six actual one-prompt runs passed on official Codex 0.153.4: Allow once,
Decline and Cancel request on both main and graph hosts. Every run used the real
composer/form, production IPC, native callback and actual Svelte decision button.
The assertion requires authoritative completed/zero-exit output only for Allow;
rejections must be native `declined`, not an execution failure mistaken for denial.
Outcome and original cwd are checked again after native history readback. Graph
tests open both fixture cards through production navigation, preserve the second
card's unsent draft, and verify no sibling/main request or native binding appears.
The six exact disposable histories were deleted successfully. The tests consumed
subscription usage; no visual testing, personal config/history mutation, project
file changes, privilege escalation or additional native permission was used.
Normal verification unit-tests the choice parser and outcome oracle without
inference. Other request kinds, extended policy decisions, graph delivery, crash
recovery, media and remaining supported native controls are not covered by this matrix.

This increment is packaged in the unsigned preview built on 2026-09-07 at 07:49
Europe/Rome. Full verification passed: 529 Rust tests/5 ignored, zero Svelte
errors/warnings, native generated contracts, frontend regressions, clippy, scoped
dependency audit (13 previously allowed warnings), hidden-WebView staged startup
and release-manifest verification. The six inference probes passed separately;
ordinary builds do not start them. No visual tests.
`outputs/codex-app-server-preview/CentralAgent.exe` is 30,827,008 bytes;
SHA256 `476fac81fa642bd660371e70f913ddaab1ff5387793a4cd50dcebe1e57ff5b80`.
Official Codex CLI 0.153.4 is still required. This is not a completed-objective release.

Mode investigation in this increment: the official guide labels collaboration
mode listing experimental. The selected generated stable ClientRequest union has
no `collaborationMode/list`, and TurnStartParams has no `collaborationMode` field.
Plan selection therefore remains unavailable under the current stable-capability
contract. No guessed field, custom planning prompt or experimental opt-in was
introduced. Existing native request presentation does not imply a guaranteed
way to trigger the user-input tool in every mode.

Native command-approval increment (2026-09-07): the opt-in desktop harness now
supports `--check-native-conversation --approvals --allow-test-inference`. Its
actual Svelte Enter/Allow once path passed against official 0.153.4 with one
`[string]::Concat('NATIVE_APPROVAL_ALLOWED')` command, unchanged read-only/untrusted
policy and exact disposable cwd. It verifies the pending card survives unrelated
rendering, native answer and server resolution, authoritative zero-exit output,
no additional tool work, card removal, an unsent draft, native history readback,
no other-provider transcript copy and no workspace file changes. Only its exact
owned native test thread is deleted on completion/failure.

Four one-prompt attempts consumed subscription usage during this increment.
The first rejected Codex's packaged PowerShell display path before approval;
the next two exposed Windows ConstrainedLanguage rejecting Console.WriteLine.
The final core-string operation passed under the same native permissions.
No sandbox setup, elevation, personal config/history changes, or visual test was
performed. The oracle accepts only the exact print operation with known Windows
or Codex-packaged PowerShell launchers, including native JSON-quoted paths; extra
commands, alternative executables and profile-enabled invocation are rejected.

The shared main/graph decision card also now distinguishes managed-network host/
protocol approvals and stdin callbacks from new commands, following the official
guide and selected generated schema. Two presentation regressions cover this
distinction; the exact-command oracle has its own Rust regression. This closes
one real command approval path, not file/permissions/questions/MCP decision gates,
graph-specific delivery/approval, crash recovery, media or advanced native controls.

The earlier approval increment was packaged in the unsigned preview built on 2026-09-07
at 07:35 Europe/Rome. Full verification passed: 528 Rust tests/5 ignored, zero
Svelte errors/warnings, generated native contracts, frontend regressions, clippy,
scoped dependency audit (13 previously allowed warnings), staged hidden-WebView
startup/interaction checks and release-manifest verification. No visual tests.
`outputs/codex-app-server-preview/CentralAgent.exe` is 30,817,792 bytes;
SHA256 `ccc42e034178c4e1246f923ca2061d9f57cd86d9dfbb3791b14139deed7d1199`.
It requires official Codex CLI 0.153.4 and is not the completed-objective release.

Main delivery/reconnect acceptance (2026-09-07): `--check-native-conversation
--delivery --allow-test-inference` drives the compiled composer and its actual
Send now, Queue, Stop, Load history and Resume connection controls against official
0.153.4. Three no-tool turns plus a steering input passed: exact first-turn ACK,
observed queue held behind active work, one dequeue, newer draft preservation,
streaming third-turn interruption, native readback and a new connection resuming
the same three-turn history without replay. Only the disposable native thread was
deleted; no visual testing, shared config write or project change occurred.
The probe consumed subscription usage. This increment adds acceptance coverage,
not a substitute agent loop or new permission behavior. Graph-specific delivery,
decisions, crashes/uncertain recovery, media and advanced native controls remain
open; the full objective is not complete.

The earlier delivery/reconnect preview was packaged on 2026-09-07 at 07:15 Europe/Rome.
Full verification passed: 527 Rust tests/5 ignored, zero Svelte errors/warnings,
native generated-contract checks, clippy, scoped audit (13 previously allowed
warnings) and the exact staged hidden-WebView startup check. The optional native
delivery probe passed separately; ordinary builds never run subscription prompts.
`outputs/codex-app-server-preview/CentralAgent.exe` is 30,691,840 bytes;
SHA256 `6be571fbee1488a470ecffff6e21e33dd9fbe68600050c81ec90e53330b41f6f`.
This unsigned development preview requires official Codex CLI 0.153.4. It does
not close the outstanding graph delivery, decisions, crash recovery or media gates.

Graph desktop-host acceptance (2026-09-07): the opt-in command also accepts
`--graph`, using actual graph navigation, Svelte forms, production dispatch and
the official runtime. Its mapped directory fixture includes real containment
relations. Three no-tool prompts passed: simultaneous A/B turns, continuation in
A, B's unsent draft preserved, distinct stable native threads and cwd readbacks,
matching assistant DOM rows, and cleanup of both exact owned native histories.
The real test found empty other-provider session creation during native graph
assignment; Codex now updates only its assignment without creating or relabeling
that separate session store. No native transcript was copied by that defect.

Separately, native messages now participate in graph timeline invalidation:
streaming no longer waits for a changed runtime phase. Six graph-history tests
include execution of the actual host signature for native-only deltas/completion
and stable unrelated metadata. This is deterministic evidence, not a claim of
visually measured streaming. DESIGN and the native conversation scenario are
updated. Queue/Steer, decisions, provider switching, reconnect, media and advanced
native controls still require their remaining gates; the objective remains active.

Updated unsigned preview Release passed on 2026-09-07 at 07:01 Europe/Rome:
527 Rust tests passed/5 ignored, six graph-history regressions, zero Svelte errors
or warnings, generated protocol checks, clippy and staged hidden-WebView startup
acceptance. The audit retains 13 previously allowed dependency warnings.
`outputs/codex-app-server-preview/CentralAgent.exe` is 30,684,160 bytes;
SHA256 `c1300ea0317fd432141a91be336c7c03e4fcae0bbf866c2a1f6a6fc3b60a4179`.
It requires official Codex CLI 0.153.4 and remains an integration preview, not a
claim that all native feature and end-to-end gates are complete.

Desktop-host conversation acceptance (2026-09-07): added the opt-in
`--check-native-conversation --allow-test-inference` command, which uses the
compiled main composer, production scoped IPC/Rust dispatch and real 0.153.4
runtime in a hidden window with disposable app data/workspace. The first run
exposed an actual startup defect: `renderAgentPanelState` was nested inside
`closeAllProfilePickers`, so an untouched panel had no state renderer and could
not bind its composer. The prior startup check clicked a picker before checking
the renderer, masking the defect. The renderer is now top-level and the startup
check asserts its existence before any interaction.

After the fix, actual host acceptance passed: two tiny read-only prompts,
matching assistant DOM rows, native deltas, ACK-driven draft clearing, one native
thread, two completed native turns, native history readback and no transcript
copy into the other-provider chat store. Only the owned native test history was
deleted; no visual tests, personal-project edits or shared config writes occurred.
This closes the basic main-chat host gate, not graph, Queue/Steer, approvals,
provider switching, reconnect or media acceptance.

Full verification and unsigned preview Release passed on 2026-09-07 at 06:30
Europe/Rome: 527 Rust tests passed/5 ignored, zero frontend errors/warnings,
generated-contract checks, clippy and staged hidden-WebView startup checks.
The dependency audit still reports 13 previously allowed warnings.
`outputs/codex-app-server-preview/CentralAgent.exe` is 30,664,192 bytes;
SHA256 `0c519b9c37f8f6a9e9419220a3288e2eb3ffdcc4e4ff687b573bb7395996b5c1`.
The complete objective remains active; this is not the final all-features release.

Thread-scoped OAuth increment (2026-09-07): native MCP service authorization is
now available directly in main/graph conversations. It sends the exact native
thread/server identity, retains the observed inventory and opens the returned URL
only on a second explicit user action. Credentials stay in Codex; UI shows only
the authorization origin. Inventory invalidation no longer discards a pending
login or its early completion. Global/foreign completions, stale inventories,
attempt IDs, thread replacement and disconnection cannot complete another attempt.
Main/graph component tests and Rust state tests passed. The actual selected
0.153.4 runtime passed OAuth discovery, URL return, PKCE exchange, exact-thread
completion and authenticated status against an owned loopback fixture, for both
thread-scoped and global authorization. Each case uses a fresh profile, ephemeral
thread and isolated file-only credential store. No external account,
personal credentials, keyring writes, model prompt, tool call or visual test was
used. This is an actual protocol/auth-flow acceptance result, not full production
host-UI acceptance. Shared MCP configuration editing remains open.

Earlier conversation MCP inventory increment (2026-09-07): main and graph exposed the
native `mcpServerStatus/list` with the exact bound `threadId`. Complete pages are
published atomically, separately from global Settings. Thread replacement,
unload/archive/delete, MCP startup/auth changes and disconnect invalidate pending
results and label old metadata stale. Inspection does not resume, send a prompt,
execute tools, read resource contents or register custom servers. Shared reload
and OAuth controls initially remained in Settings. Thread OAuth was completed by
the increment above; configuration editing and the full host approval gate remain open.
Five Rust state tests cover read-only scope, pagination and late-result isolation;
main/graph component tests cover targeting and unchanged pending chat actions.
An explicitly run real 0.153.4 test passed with an isolated child profile and a
local inventory-only MCP fixture: the ephemeral thread reported exactly its
fixture tool/resource, an unknown thread was rejected, and the fixture trace
proved no tools/call or resources/read. No account credentials, personal MCP
configuration or model inference were used. This proves the native inventory
path, not full production OAuth/host-UI end-to-end acceptance.

Detached-review compatibility increment (2026-09-07): the generated request
contract and official guide describe detached delivery, but the actual selected
0.153.4 runtime rejects it for the paginated threads it creates. The explicit
compatibility probe confirmed JSON-RPC -32600 (`paginated threads do not support
detached review`), unchanged source turns, no new detached histories in the exact
temporary cwd, and deletion of its own test thread. No native configuration,
personal history, experimental flag or model input format was changed. This
classifies a runtime limitation, not a passing detached feature. The ordinary
native fork and inline-review actions remain separately available.
Main/graph review dialogs now explain the observed limitation using native
history-mode metadata; missing/unknown mode does not imply legacy support.
A mirror regression test and component main/graph test cover metadata freshness,
owner isolation and unchanged inline-only submission. The new outbound detached
adapter is contract-checked but not exposed as an executable UI action.
Full detached UI/approval ownership remains contingent on a compatible native
history/runtime; the rest of the objective continues independently.

Native review reconciliation fix (2026-09-07): real-runtime review testing exposed
a worker-turn ID different from the parent review ID. Waiting only for the
projected live turn could leave the application apparently busy after native
completion. The main/graph host now drains event-triggered, coalesced native
thread/read requests after final review items and terminal/idle notifications.
Only native snapshots close projected worker turns. Reads do not submit prompts,
do not consume model inference, do not infer success from idle, do not clear an
ambiguous submission receipt, and do not retry on a timer. Lifecycle/disconnect
clears transient refresh requests. Three regression tests cover worker closure,
in-flight read races, owner isolation, failure/no retry and uncertain receipt
preservation. The actual 0.153.4 probe passed using this production path, including
final review/history readback, unchanged earlier history and no remaining busy
state. Its test root and native reviewer descendants were deleted. The initial
stalled test harness was stopped only after native readback proved completion;
its exact disposable histories were also deleted through the official API.
Detached review and full main/graph host-UI approval acceptance remain open.

Review acceptance increment (2026-09-07): the opt-in `review_scope_probe` now
supports `--exercise-review` to test actual production inline review dispatch,
native entered/exited-review items, terminal success, earlier-history preservation
and native history readback on one disposable thread. This expands the existing
preparation-only probe; it does not replace review/start with a custom turn.
Two deterministic tests reject incomplete/failed/empty review results and foreign
thread/turn completion. They are included in the workspace verifier without
running model inference. Full workspace verification passed: 514 existing Rust
tests plus these two oracle tests, three ignored, frontend/contract checks,
clippy, and the scoped audit (13 allowed dependency warnings). No application
binary source or UI surface changed in this increment; the verified preview
below remains current. Real review inference evidence is recorded separately
from these deterministic checks; detached review and host-UI acceptance remain open.

Native compaction increment (2026-09-07): `/compact` and Compact context expose
official `thread/compact/start` with an original-thread confirmation and native
idle/loaded/history checks. Resume connection reopens the same thread without a
model submission. A metadata-only write-ahead receipt distinguishes sending,
accepted, running, stop-requested and uncertain outcomes. Native completion or
definitive failure releases the owner; disconnect/unload preserves explicit
history review, never an automatic retry. Stop retains intent until a native turn
ID exists. Main/graph drafts, attachments, skill selections and siblings remain
independent; no Time Machine or custom compaction engine is added.
Six new core regression tests cover ACK/event ordering, reconnect, rejection,
stale/foreign events, cancellation and receipt compatibility. One host projection
test checks active/completed labels, the target-validation test includes compact,
and one component test checks original-owner confirmation and accepted-not-done UI.
The actual 0.153.4 lifecycle probe with `--exercise-compaction` passed, consumed
account allowance only on newly created test histories, and deleted those histories
after verifying native contextCompaction completion and unchanged sibling/fork
content. Full host-UI acceptance and the other objective gates remain open.

Native command shortcut increment (2026-09-07): main and graph composers share
an on-demand, keyboard-operated suggestion menu for nine implemented native UI
intents. Existing dialogs retain all native confirmations and ownership checks;
unsupported commands remain drafts with an explicit literal-input escape.
Submit, Queue and Send now intercept commands before inference. Paths, multiline
input, attachments, selected skills and other-provider behavior are preserved.
Component event tests cover filtering, keyboard navigation, owner/provider
changes, unavailable controls, lifecycle confirmations, inventory opens and the
main configuration-host reveal. Complete workspace verification passed with
507 Rust tests/3 ignored, zero Svelte errors/warnings, 52 modular UI files,
30 scenarios, the native contract/decision suites, clippy and scoped audit
(13 allowed warnings). No model inference or visual tests in this increment.
This is not a claim that every CLI command is an available App Server method.

Latest preview build (2026-09-07 06:10:27 +02:00):
`outputs/codex-app-server-preview/CentralAgent.exe`, 30,544,896 bytes,
SHA-256 `40CCFE4B3BD592C4AE50CBD223E92836BB7C8BAD55930F07AB20BC7C4D977E3D`.
The release pipeline repeated complete verification, compiled optimized binaries,
passed the exact staged hidden-WebView startup check (toolbar, agent/editor,
graph, diff search) and verified release manifests. It includes native command
shortcuts, native context compaction, review completion reconciliation,
detached-review compatibility guidance, conversation-scoped MCP inventory and
native thread-scoped service OAuth.
Unsigned development preview; official
Codex 0.153.4 is still required.
No visible UI inspection was performed. Remaining full-objective gates stay open.

Latest release verification (2026-09-07): 527 Rust tests passed, five ignored,
including 81 focused tests in `crates/central-agent-codex-runtime` and two explicit
review-probe oracle tests. The ignored
real-runtime configuration, thread-MCP inventory and global/thread OAuth tests
were run explicitly and passed separately. The complete
frontend/build, clippy and scoped audit passed (13 allowed dependency warnings).
48 outbound calls and 19 native decisions validate against the generated contract;
incoming fixtures also cover lifecycle, history, review, usage, MCP, configuration,
skills, ten richer activity items and five model service notifications. These automated checks are not full native
end-to-end or visual acceptance. The real 0.153.4
smoke check completes initialize/initialized, account/read, configRequirements/read
and the full model/list pagination (7 entries), without inference or printing
account details. Explicit executable > packaged resources/codex > absolute PATH
resolution is tested. A bad explicit path/version does not silently fall back.

Connection errors invalidate pending responses; catalog publication is atomic
across pages and account refresh revisions reject old results. Native login URLs
are validated and opened only on user intent; logout has a shared-account warning.
There is no automatic login, download, configuration write, or elevation.
A display-only event mirror, native composer submission and main/graph streaming
are implemented. Codex is selectable after account/catalog/requirements readiness.
Native history/branching, inline review, MCP inventory/OAuth, public preferences
and explicit skill input are now connected. Advanced native configuration,
detached review, media presentation, additional notifications and end-to-end
acceptance remain required. Time Machine and custom platform tools are deferred by
the user's native-only scope decision.
An integration preview exists at `outputs/codex-app-server-preview/CentralAgent.exe`;
it is not the final accepted milestone-5 Release. See the latest build record below
for which increments are included. The previous reset Release is preserved.

### Native conversation backend progress

Real production-core lifecycle acceptance passed on Codex 0.153.4 (2026-09-07).
`conversation_lifecycle_probe --allow-test-inference` uses two temporary roots,
synthetic main/graph owners, two native conversations and a branch. Three minimal
read-only prompts verify concurrent dispatch, live deltas, completed-answer
isolation and continuation after restoring only bindings into a fresh private
App Server connection. Native read/rename/fork/archive/unarchive/delete also pass;
the branch and other owner's histories remain unchanged by the continued turn.
All test-created conversations were deleted through native methods; no existing
personal history was read or changed, no personal configuration was changed, and
the local Central Agent binding file was neither read nor modified. Repeated
development runs consumed a small amount of account inference allowance.
This closes the real **core/transport** lifecycle sub-gate, not the full main/graph
host/UI acceptance checkbox: queue/steer, permission decisions, attachments and
provider-switching end-to-end checks remain required. No production behavior or
visible UI changed, so the existing preview executable remains current.
The complete workspace verification was rerun successfully after adding this
probe: 507 Rust tests passed, three ignored; frontend checks/build and deterministic
UI tests, 44 outbound contract samples/19 decisions, clippy with warnings denied,
and the scoped dependency audit (13 previously allowed warnings) passed. The probe
is compiled by all-target checks but its inference requires explicit opt-in.

`conversations.rs` captures a stable main/graph owner for every native request.
Open/resume/read, rename/archive/restore/delete/fork, turn start/steer/interrupt
and compaction use the existing official API constructors. No transcript is
reconstructed or sent to resume; no local tool/model loop is introduced.
Only bindings and unresolved message receipts are durable. The host reads and
atomically writes `app-server-threads.json`; malformed data is retained and blocks
native mutations rather than being replaced with an empty store. An OS-owned
exclusive lock prevents another Central Agent instance from writing these bindings;
the lock releases on exit/crash and does not disable other provider adapters. Account reconnect
retains these bindings and invalidates old replies. Main and graph IPC use local
owners, not frontend-selected native IDs, commands or working directories.

Tests include concurrent main/graph owners with real fixture pipes, completion
before acceptance, authoritative item replacement, foreign-thread rejection,
native branch isolation, archived-state preservation and stale delivery recovery.
Start receipts reconcile only through the native client message ID. The stable
steer API has no equivalent receipt ID, so uncertain steering requires explicit
history review; neither path auto-replays a prompt. The private in-app event bridge
now connects composer submission, Stop, local queue and native history inspection
to the owning main chat or graph conversation. Thread list/import and explicit
forks into an unbound destination and automatic branch creation are connected;
the remaining native integrations are still part
of the active objective.

Lifecycle increment (2026-09-07): main/graph profile controls expose native
load/refresh, rename, archive/unarchive and delete with exact-thread confirmations.
Six lifecycle fixtures validate against generated v2 schemas; four additional
runtime tests cover notification/ACK races, terminal deletion and expired
decisions, plus one host target-validation test and three UI handler tests.
Full workspace verification passed with 442 Rust tests and 2 ignored, frontend
checks/build, contract validation, clippy and scoped audit (13 allowed warnings).
No real conversation was archived/deleted, no visual test or model inference was
performed, and no new Release was produced. End-to-end acceptance remains open.

### Native decision progress

Native clear-preference increment (2026-09-07): Clear saved value is available
only for observed base-user public overrides in main/graph native preferences.
Confirmation freezes owner, directory, view, file revision, key and observed value;
one config/batchWrite upsert with null removes that key without reload or guessed
defaults. Save/clear share the same write lock and stale/unknown-result handling.
Three new Rust regressions and three component tests cover removal authority,
missing values, changed scope, duplicate clicks, disconnect and no implicit reset.
An explicit ignored real-runtime probe, using only a newly created temporary
child profile, passed with Codex 0.153.4: production Snapshot.clear removed
model_verbosity while preserving model_reasoning_effort. No personal config,
history, account, inference or parent environment was changed. Full verification
passed: 507 Rust tests, three ignored (the real probe passed separately), nine
preferences UI tests, 44 outbound contract samples, frontend build/checks, clippy
and scoped audit (13 allowed warnings). No visual testing was performed.

Latest integration preview: rebuilt on 2026-09-07 at 04:08 Europe/Rome at
`outputs/codex-app-server-preview/CentralAgent.exe`, including model service
notices and clear-preference support. SHA256:
`be391307e60c3f4f77bf90b9b0777512f467cc27d9b2360da8bf09483d1e8a4d`.
The release pipeline repeated complete verification, compiled optimized binaries,
passed the staged hidden-WebView startup check, and verified five published
manifest records. This remains an unsigned development preview requiring the
official Codex CLI 0.153.4, not completion of the outstanding acceptance gates.

Native model-notice increment (2026-09-07): bound main/graph conversations now
show native model rerouting, account verification and opted-in service buffering
outside the completed-work disclosure. Typed memory-only state keeps native
thread/turn scope and stable notice IDs, without phantom turns, implicit profile
changes, retries, invented links or imported transcripts. Completion is not
inferred from notifications; disconnect/unload marks prior observations stale.
Service text cannot introduce HTML, Markdown images or executable links.
Six Rust regressions cover projection, ownership, deletion, malformed payloads,
native lifecycle independence, stale history and hidden buffering; shared timeline
tests cover live/completed visibility and final-answer distinction. Five incoming
schema samples passed. Full workspace verification passed: 504 Rust tests, two
ignored, frontend checks/build, native contracts, clippy and scoped audit (13 allowed
warnings), with no inference, personal config/history mutation or visual testing.
The frontend bundle is rebuilt. No new executable was produced in this increment;
the 03:46 preview below remains available and does not yet include model notices.

Native activity-detail increment (2026-09-07): web search/open/find fields,
collaboration call versus child state, subagent milestones, function/dynamic text
output, requested waits and image generation metadata use shared main/graph
activity disclosure. Hook instructions, opaque results, ciphertext and media
payloads are excluded; unavailable previews remain explicit. No new tools are
registered and experimental capability stays off. Five host regressions and ten
generated-schema item fixtures cover display, terminal replacement, interruption
and payload exclusion. Full workspace verification passed: 498 Rust tests, two
ignored, frontend build/checks, native contracts, clippy and scoped audit (13 allowed
warnings). No visual test, personal history/config mutation or inference was used.

The integration preview was rebuilt on 2026-09-07 at 03:46 Europe/Rome and now
includes skills inventory/toggle, native defaults, explicit skill inputs and the
activity detail increment. `outputs/codex-app-server-preview/CentralAgent.exe`
SHA256: `cfd39c8d2a5ef7806221aac50040e5b7fa74260c7890778526d25963ca8c7ff5`.
The pipeline repeated full verification, compiled optimized binaries, passed
the exact staged hidden-WebView startup check and verified five manifest records.
This unsigned development preview still requires official Codex CLI 0.153.4.
No visible UI test was performed; outstanding milestone gates above remain open.

Native explicit skill-input increment (2026-09-07): Use with next prompt attaches
enabled inventory-owned references in main and graph, without configuration
writes or inference. Start, Queue and Send now freeze native skill name/path
references with $name text; App Server owns instruction loading. Refresh/invalidation,
owner/directory isolation, draft promotion, queue snapshot serialization and exact
accepted-ID cleanup preserve later selections. Local selection replies no longer
broadcast every other graph inventory; shared invalidations are deduplicated.
Four host/input regressions and three additional UI handler tests cover this
increment. Full workspace verification passed: 493 Rust tests, 2 ignored, zero
Svelte errors/warnings, 29 scenarios, eight skill handler tests, 43 outbound schema
samples now including native skill inputs for turn/start and turn/steer, clippy
and scoped audit (13 allowed warnings). No actual skill inference, private skill
content read, personal config write, visual test or new Release was performed.
The frontend bundle is updated; the previous preview EXE is unchanged. Remaining
advanced configuration, override removal, detached review, native slash intents,
event variants and final runtime acceptance are still open.

Native preferences increment (2026-09-07): main/graph native controls inspect nine
public defaults, native origins and base user-file values through config/read.
Confirmed config/batchWrite uses the observed native target/version without live
thread reload. Conflicts, overrides, stale views, owner/root changes, pending
shared skill/config writes and disconnect retain honest outcomes without retry.
Six core projection/contract tests, five host lifecycle/conflict tests and six UI
handler tests were added. Full workspace verification passed: 489 Rust tests,
2 ignored, zero Svelte errors/warnings, 29 scenarios, 43 serialized outbound
schema calls, three configuration response fixtures, clippy and scoped audit
(13 allowed warnings). Real read-only config_probe passed on 0.153.4 with nine
public preferences, two layer descriptors and a versioned base user target.
No private values were printed, no personal config was changed, no inference or
visual test was performed. The frontend bundle is rebuilt; the preview EXE is
unchanged. Clearing overrides, advanced native config, explicit skill input,
detached review, remaining event variants and final runtime acceptance stay open.

Native skills increment (2026-09-07): main/graph profile controls list the current
local directory's native skill metadata and discovery errors, with confirmed
shared enable/disable through skills/config/write. Native effective state is
authoritative; active/queued work, changed inventories, cross-owner requests and
disconnect are handled without implicit prompts or replay. Five host state tests,
five component-handler tests, two outbound calls and three generated-schema
response/notification fixtures cover this increment. Real no-inference discovery
on 0.153.4 returned 18 skills and zero errors; no personal configuration was changed.
Full workspace verification passed: 478 Rust tests, 2 ignored, Svelte check/build
(48 modular files, 28 scenarios), 41 outbound native contract calls, three skill
response/notification fixtures, component tests, clippy and the scoped dependency
audit (13 allowed warnings). No visual testing or new Release in this increment.
The preview Release from the preceding increment does not yet contain these
controls. Explicit native skill input,
general configuration editing and final acceptance remain open.

Loaded-session increment (2026-09-07): direct reuse and explicit unused-link
recovery are connected to main/graph conversations. Five new core regressions
cover profile changes, unload/late-resume races, restart/local reset, attempted
and externally observed work, and legacy/imported/forked history exclusions.
Two component-handler tests cover explicit confirmation, owner changes and
revoked eligibility; host target validation also covers reset_unused.
The no-inference loaded_session_probe passed against actual Codex 0.153.4:
new ephemeral session idle/reusable and present in native thread/loaded/list.
Its private process was shut down; no prompt, native history mutation or local
binding was persisted. Full workspace verification passed: 473 Rust tests,
2 ignored, Svelte checks/build, 39 outbound schema samples, component tests,
clippy and scoped dependency audit (13 allowed warnings). No visual testing.
An unsigned integration preview was built successfully at
`outputs/codex-app-server-preview/CentralAgent.exe` on 2026-09-07. The release
pipeline reran all verification, compiled optimized binaries, passed the actual
staged hidden-WebView startup check and verified all five manifest file hashes.
The prior reset Release was not replaced. Codex 0.153.4 remains an external
official runtime dependency; advanced configuration, skills, detached review,
remaining event variants and final main/graph acceptance are still open.

Native inline-review increment (2026-09-07): Review code in main/graph binds
four typed native targets to the original conversation and confirmed directory.
Native resume prepares read-only/user-routed scope, validated before review/start;
the configured native reviewer owns model choice. Stop, completion-before-ACK,
uncertain delivery, unchanged drafts/attachments and cross-owner isolation are
covered. Entered/exited-review output and native plan-step details use the shared
timeline. Four core tests, two host tests and two UI handler tests were added.
Full workspace verification passed: 468 Rust tests, 2 ignored, frontend checks/
build (27 scenarios), 39 outbound schema samples, five review response/event
fixtures, clippy and scoped audit (13 allowed warnings). No visual test or new
Release was produced. Detached review and full reviewer inference acceptance,
native skills/configuration controls and remaining lifecycle gates are still open.

A real runtime probe completed one minimal read-only model turn and then verified
the exact review-preparation scope on Codex 0.153.4. This consumed account usage.
Only newly created test conversations were deleted through native thread/delete;
existing personal history was not read or changed. The empty-thread probes showed
that thread/resume has no rollout to load before a first persisted turn. Follow-up
M3 acceptance must cover cancellation before that first turn and later continuation:
reuse a still-loaded native session where supported, and handle an unmaterialized
binding after reconnect explicitly, never by sending a dummy model prompt or
silently replacing existing native history.

Native automatic-branch increment (2026-09-07): bound main/graph conversations
can create a new project chat or same-node agent card before native thread/fork.
Confirmation freezes the source ID and directory; explicit Open branch retains
the original draft/card. New permission consent starts read only. Pending-fork
source references are written before dispatch and survive uncertain delivery or
restart; only a user-selected matching native fork resolves them. No history or
model loop is reconstructed. Three new core tests cover receipts and recovery,
one host test covers independent local destinations, and four UI handler tests
cover confirmation, recovery and actual native graph-open event routing.
Full workspace verification passed: 462 Rust tests, 2 ignored, frontend build/
checks and deterministic tests, 34 outbound schema samples, clippy, and scoped
dependency audit (13 allowed warnings). No personal Codex conversation, model
inference, visual test or new Release was involved. Remaining native config,
skill/review controls, richer event variants and full runtime acceptance are open.

Native history increment (2026-09-07): on-demand paginated listing/search and
archived filtering, explicit link/read and native fork into an unbound main/graph
destination are connected. IDs, owner, view and local directory are validated;
native history is never replayed as input or persisted as a local transcript.
Four runtime tests, three host tests, four UI handler tests and generated-schema
list/read/fork fixtures cover this increment. Full workspace verification passed:
458 Rust tests, 2 ignored; frontend checks/build and deterministic tests; 34 native
outbound schema samples; clippy and scoped audit (13 allowed warnings).
No personal native conversation was read, imported, forked or mutated in tests;
no inference, visual testing or new Release was performed. This history increment
preceded automatic local branch creation; full end-to-end acceptance remains required.

Native MCP increment (2026-09-07): configured-server pagination, metadata/status
disclosure in AI Settings, inventory-bound reload confirmation and managed OAuth
are connected to official RPCs. OAuth URLs remain Rust-side, open only explicitly
and are discarded on completion/disconnect. Null runtime status is not inferred
as connected. No custom tools or model prompt are injected. Six host regressions,
three UI handler/presentation tests, five incoming schema samples and four new
outbound call samples cover this increment. Full workspace verification passed:
451 Rust tests, 2 ignored; frontend check/build and deterministic tests; 33 native
outbound schema samples, clippy and scoped dependency audit (13 allowed warnings).
No real MCP OAuth/configuration mutation or visual test was performed. Native
configuration editing, thread-specific status and end-to-end gates remain open.

File-input increment (2026-09-07): native main/graph attachment selection,
start/Queue/Send now snapshot delivery, exact-owner acceptance cleanup and
bounded native image-history previews are connected. The complete workspace
passed with 445 Rust tests, 2 ignored, frontend checks/build, schema validation,
clippy and scoped audit (13 allowed warnings). No visual test, real file upload,
model inference or new Release was performed. Remaining end-to-end gates stand.

The native request store scopes command/file approval, requested permissions,
user questions and stable MCP form/URL elicitation to the original thread owner
and connection generation. A typed UI intent selects only native proposed policies
or exactly the displayed permission profile. Expired and repeated decisions are
rejected; server resolution, turn completion and disconnect clear pending cards.
Standalone MCP requests with no turn remain active until their own resolution.
Requests and answers are not persisted. IPC Debug formatting redacts answers.

Main and graph share Svelte request cards, explicit once/session decisions,
diff previews, keyboard-accessible questions and primitive MCP forms. Session and
policy scope are explained; external URL opening is an independent user action.
Unsupported experimental form modes are rejected, not simulated. MCP field type,
choice and numeric/length constraints are checked locally; domain-specific string
format validation remains the native MCP server's responsibility.
19 serialized decisions and their matching native request samples are validated
against the selected runtime's generated schemas. Three UI tests cover request
isolation and typed form values. These cards are wired into the native composer
workflow; real submitted-turn end-to-end acceptance is still pending.
# 2026-09-08 resumed-goal increment

User validation advanced: native account readback and a completed main-chat
prompt confirmed. The reported missing reasoning was investigated through
read-only App Server reads of this application's bound histories: the completed
turn contained no public summary text. Added explicit per-conversation native
summary selection (Automatic initially), frozen input capture, empty-summary
row handling and main/graph regressions. Official runtime loopback tests proved
the summary parameter reaches model requests without shared config writes.
The verified unsigned build is `outputs/codex-summary-preview/CentralAgent.exe`;
see the acceptance audit for hash and test evidence. This is concrete progress,
not goal completion: account-backed summary emission and remaining user-operated
gates are still unverified. Preserve the full objective and native-only boundary.

The next increment completed a deterministic real-runtime summary-stream test:
two owned native threads received four public-summary fragments, two summary
parts and progressive commentary before final output; native history and the
production mirror agreed. Regression probes, ordinary runtime tests and clippy
passed. This follow-up changed tests/docs only and does not require a new EXE.
The account-backed summary check remains open; see the acceptance audit for
exact evidence and the distinction from main/graph presentation tests.
