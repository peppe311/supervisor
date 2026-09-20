# Codex App Server completion audit

## 2026-09-11: automatic saved-chat loading and Settings reload

Saved chat and graph bindings now load native display history automatically on
startup/connection, including while the separate profile is signed out. A
sequential reader prioritizes the selected chat, waits for complete paginated
history, preserves missing links and skips active/rebound owners. AI Settings
offers Reload chats through the same reader without restarting a connected
runtime. No prompts, thread resumes or authentication pages are started.

The exact final Windows executable was launched against the migrated local
profile without clicking Connect or Reload. Accessibility inspection confirmed
the selected chat's saved answer was present while the account still required
sign-in. The native metadata journal recorded 9 accepted and 6 rejected history
reads, matching the previously recoverable/missing bindings; it recorded no
turn starts or thread resumes. The application was closed normally afterwards.

Full workspace verification passed: 764 ordinary Rust tests plus 3 scope-probe
tests, 190 frontend tests and 1 existing skip, with 24 opt-in Rust tests ignored
and 4 existing allowed dependency warnings. After an alignment-only markup
correction, the frontend and account tests passed again and the final executable
passed hidden packaged-UI startup. Forty state/theme/layout combinations passed
DOM checks; all five reload states were inspected in Light and Dark at 900x800.
Larger layouts are DOM geometry evidence, not full native 4K captures.

Windows input automation still could not click Settings reliably. The manual
button's rendered behavior and exact IPC intent were verified in the compiled
browser fixture; native automatic loading was observed separately above.
No account-backed inference was used.

Evidence and executable: chat reload report (historical report removed during workspace cleanup).

## 2026-09-11: Supervisor profile isolation and history transfer

Production App Server launches now pin the Supervisor-owned Codex home, SQLite
storage and file-backed native account store. The official app retains its own
profile and sign-in. Existing global credentials, configuration and databases
are not copied. See [profile isolation](CODEX_PROFILE_ISOLATION.md).

The authorized local migration recovered **9 conversations, 87 turns and 10,487
native items**. Full paginated history hashes, native IDs, fork/parent ancestry,
directories, titles and goal state match the source. Two native titles and one
completed, zero-usage goal were restored through public native metadata APIs.
All source rollout hashes and the Supervisor binding-file hash remain unchanged.
The **6 already-missing histories** remain missing in both profiles; their local
links were preserved. Original conversations remain in the official app.

Public account reads confirm the official profile is still signed in and the
new Supervisor profile is signed out. The user must sign in once from Supervisor;
this audit did not copy tokens or perform account-backed model inference.

Passed: full workspace verification, **762 ordinary Rust tests plus 3 scope-probe
tests**, **189 frontend tests with 1 existing skip**, and a separate pinned-native
isolation/restart/rehydration test using a loopback model fixture. The 24 ignored
Rust tests remain opt-in. Svelte reported zero errors/warnings; the dependency
audit retained 4 existing allowed warnings. The exact final local executable
passed hidden packaged-UI startup with exit 0 and has Windows GUI subsystem 2.

Compiled account, sign-out, preference and MCP copy was inspected in Light and
Dark with synthetic state. Account/sign-out geometry passed at 900x800,
1920x1080, 2560x1440 and 3840x2160. These include DOM layout checks rather than
full native 4K screenshots. Windows UI automation detected the app but could not
interact reliably; the migration therefore used an audit runner containing the
same production migration code and holding the same exclusive binding lock.
This does not claim a completed live Windows sign-in flow.

Evidence and final SHA-256: isolation report (historical report removed during workspace cleanup).
Local unsigned executable: `outputs/supervisor-isolation-2026-09-11/app/Supervisor.exe`.
Older executable copies retain their previous behavior; use this build for the
separated profile. No existing shortcuts were redirected.

## 2026-09-11: native delegation, plan outcomes and event diagnostics

The conversation-protocol audit findings A1, A2, G1 and G2 are implemented.
Plans preserve actual step outcomes and appear with the turn's work before its
final response. The main host keeps the native plan title/status visible even
when its detail block is present. Native spawned children receive separate,
durable local ownership in main chats and graph cards; delegated ancestry is
distinct from independent forks. Related-agent navigation remains available
while work is active.

Real-account acceptance passed separately through the compiled main and graph
composers with **gpt-5.6-luna, low effort, Standard speed**. Each parent delegated
one fixture-only read, received the child's result and verified it. The host
saved child ancestry, opened its actual controls and history, rendered an
approval only on that child, stopped it without granting the approval, and
continued the same native child thread. The fixture remained unchanged and the
test histories were retained. These are hidden-host native acceptance checks;
the screenshot inspection uses separate synthetic data. Neither real parent
emitted a native plan item, so plan outcomes/order are covered by deterministic
regressions and synthetic rendering rather than a new live-plan assertion.

The opt-in acceptance entry point is `--check-native-conversation
--allow-test-inference --retain-test-history --delegation`, with `--graph` for
the graph host. `SUPERVISOR_DELEGATION_REPORT_DIR` selects the evidence folder.
It requires the connected account and consumes inference; it is excluded from
ordinary workspace verification.

Event diagnostics retain only bounded protocol metadata, local receive order
and correlation identifiers. The two JSONL segments retain at most 2048 recent
events across the app; the UI reads the latest 200 for the exact local owner
and native thread. Prompt acknowledgement, uncertain delivery and actual turn
completion have distinct meanings. The journal contains no prompts, tool
outputs, approval answers or private reasoning, and does not drive replay.

Passed: complete workspace verification with dependency audit skipped,
**753 ordinary Rust tests plus 3 scope-probe tests**, **189 frontend tests with
1 existing skip**, type/design checks, generated contract checks, formatting and
Clippy. The 23 ignored Rust tests remain explicitly opt-in. Final plan CSS was
then checked visually and against the design contract; the locked release
binary was rebuilt and its hidden packaged-UI startup passed with exit 0.

Evidence and visual-capture limits:
implementation report (historical report removed during workspace cleanup).
Local unsigned executable:
`outputs/supervisor-delegation-implementation-2026-09-11/app/Supervisor.exe`.
This does not claim acceptance for every unrelated App Server capability.

## 2026-09-10: real chat/fork/tree chain verified

Computer Use on `supervisor-manrope-complete-preview-2026-09-10/Supervisor.exe`
completed **F1 and F2**, using the already connected ChatGPT account and five
short text-only test turns. The first fork retained only the chosen completed
turn; original and fork continued independently with BETA and ALFA. A nested
fork inherited its immediate parent's history and continued with GAMMA without
changing that parent. Source renaming, source navigation, subtree collapse and
reopening, durable native-ID ancestry and all three histories after restart
passed. No implementation change was required.

Verification: **25 frontend + 16 Rust focused tests passed**, plus the packaged
Light/Dark hidden-WebView suite. Live evidence is from the Dark Windows app;
hidden-WebView scenarios are synthetic. This does not pass unrelated graph-card,
OAuth, OS elevation, destructive-action or external-service checks.

Evidence: live chain audit (historical report removed during workspace cleanup).
The older entries below retain their historical scope; their references to F2
being pending are superseded by this live acceptance result. A remembered account
connection removes the login prerequisite for ordinary conversation testing;
manual user presence is not a blanket requirement for every live visual test.

## 2026-09-10: Supervisor product identity

The harness is now named **Supervisor**. The main configuration launcher and
Knowledge Graph launcher share one monochrome eye-and-S SVG with the native
application icon and active-project indicator. The former tetrahedron/rotating
pieces are removed; configuration actions, menu sections, busy guards, accessible
action names, native ownership and draft persistence remain unchanged.
Native titles, product copy, Windows file resources and release package names
are updated. Existing storage paths, account state, client IDs, IPC/DOM hooks,
internal package names and personal project/chat titles are deliberately retained.

Complete workspace verification passes: **738 ordinary Rust tests, 23 ignored**,
three scope probes, **181 frontend tests, one existing skip**, Svelte check/build
with zero errors/warnings, 74 modular UI files and 38 visual scenarios. Final
text cleanup was followed by MCP, inline-script/theme/design checks, fresh locked
release build and packaged hidden-WebView startup. Four existing allowed
dependency warnings remain; no dependency was added. The embedded main/graph
probe also verifies shared logo geometry and absence of tetrahedron pieces.

Browser inspected synthetic production UI in Light/Dark, Full HD, 2K, 4K and
720x900, plus native graph-logo sizes 84/104/132. Menu/graph operation, Settings,
start page, busy indicators, keyboard/modal behavior and draft retention were
checked. Windows product metadata and the executable-extracted icon were checked.
This is not a new real-account, live-taskbar, OS-picker or external-service audit.

Evidence: `outputs/supervisor-brand-audit-2026-09-10/AUDIT.md`.
Unsigned local preview: `outputs/supervisor-preview-2026-09-10/Supervisor.exe`.
Existing manual gates and F2 remain unchanged. See `SUPERVISOR_BRAND.md` for
compatibility details; historical names below refer to earlier builds.

## 2026-09-10: ordered Agent configuration

Main configuration and graph cards now share five collapsible sections, in this
order: Model & response, Permissions & summaries, Codex conversation, Tools &
integrations, Saved defaults. Only the model profile starts expanded. History
management has a separate, initially closed disclosure. Shared defaults are
explicitly distinguished from current-chat selections and read-only native
reports. No native method, ownership boundary or confirmation was changed.

The shared Svelte section keeps open state across refreshes, accessible keyboard
toggles and unique control targets. Selected skills and actionable native errors
remain visible when the corresponding section is closed. Slash-command dialogs
remain renderable from collapsed sections without hiding their host or drafts.
The main configuration column temporarily uses a readable minimum width without
overlapping the chat or overwriting the saved Explorer width. Graph profile
controls now use theme-backed text and background for Light/Dark legibility.

Passed: **737 ordinary Rust tests, 23 ignored**, three additional scope-probe
tests, **181 frontend tests, one existing skip**, Svelte check/build with zero
errors/warnings, 72 modular UI files and 37 scenarios. Complete workspace
verification, locked release build and the packaged hidden-WebView startup pass.
The new embedded DOM regression covers both hosts, group order/state, native
guards, visible errors/selected skills and all six collapsed shortcut paths:
goal, fork, review, resume, skills and config. Existing four allowed dependency
warnings remain unchanged; no new dependency was added.

Browser inspected production main/graph components with synthetic state and inert
IPC in Light/Dark at Full HD, 2K, 4K and 720x900. Busy, disconnected, archived,
deleted and unbound presentations were checked in both themes/hosts. No section
horizontal overflow or main-chat overlap was observed. Keyboard toggle, draft
retention, refresh and modal Escape checks passed. These are UI/embedded-host
checks, not new real-account or external-service end-to-end acceptance.

Evidence: `outputs/agent-configuration-audit-2026-09-10/AUDIT.md`.
Local unsigned preview: `outputs/agent-configuration-preview-2026-09-10/`.
Existing manual gates, including F2 independent continuation, remain unchanged.

## 2026-09-10: remembered account connection on restart

Implemented and verified automatic reconnection of a confirmed ChatGPT account.
The native runtime retains credentials; Central Agent saves only a non-secret
intent. Startup performs one handshake/account validation sequence. Sign-out
disables automatic connection before its RPC, including against late replies.
No conversation or session permission is automatically resumed/restored.

Passed: 13 Rust connection regressions, account presentation regression, complete
workspace verification, release build and packaged hidden-WebView startup.
The existing four allowed dependency warnings are unchanged; no new dependency.
Browser inspected the production account component in Light/Dark: connecting,
refreshing, connected, unavailable and signed-out at Full HD, plus connected
layout at 2K/4K. No console warning/error was observed in the fixture.

Computer Use also inspected the **real packaged app** with a separate local data
profile in two successive processes, closing the first normally. Both restored
the native ChatGPT account and six model profiles without pressing Connect or
Sign in. No OAuth/device flow, native logout, model prompt or tool action was
performed. Expired/revoked credentials and actual shared-account logout were
not induced; their host state/persistence branches are deterministic tests.
The empty-profile draft-owner alert observed outside Settings is unrelated to
this change and is recorded separately in the audit evidence.

Evidence: `outputs/codex-connection-audit-2026-09-10/AUDIT.md`.
Build: `outputs/codex-connection-preview-2026-09-10/`. Earlier user validation
and fork-continuation gates below remain unchanged.

## 2026-09-10: original/fork chat-tree clarity

Implemented local source/fork/nested-fork navigation and current-chat identity.
Ancestry comes from successful validated native responses or a confirmed frozen
fork request, not matching titles. Display metadata survives store restoration,
stays separate from unresolved fork receipts and is removed for an unlinked
child. The existing activation, rename, pin, archive and delete action ownership
is preserved. No new native API, permission or model invocation was added.

Passed verification for this increment:

- Rust workspace: **724 passed, 23 ignored**; no opt-in account probe was run.
- Frontend: **180 passed, one existing skip**; seven new pure tree tests.
- Svelte check/build: zero errors/warnings, 71 modular UI files, 36 scenarios.
- Rust formatting, diff check, locked release build and packaged hidden-WebView
  startup, including the new actual embedded tree regression, passed.
- Browser: production host/bundle, isolated synthetic state and IPC, Light/Dark
  at **1920x1080, 2560x1440 and 3840x2160**. Original, child and nested child are
  distinct; current-chat/source captions and no-file-copy note remain readable.
- Both themes also cover 170 px long-name wrapping, collapsed branch retained
  through a background update, filtered parent, archived source, and pinned
  duplicate titles. No row/page horizontal overflow or console warning/error
  was observed. Source navigation and return preserved `DRAFT_TREE_CHECK`.

Evidence and fixture: `outputs/chat-tree-audit-2026-09-10/`. These are rendering
and navigation checks with synthetic data, not a new real-account fork or native
multi-resolution OS-window audit. The personal app/profile was left unchanged.
The previous F1 history cutoff/source-preservation result is unchanged; guided
F2 independent continuation remains unperformed. See
[the current preview and upgrade instructions](CODEX_APP_SERVER_USER_VALIDATION.md).

## 2026-09-09: P3-A backend acceptance

**P3-A is complete in source within its backend-only scope.** It adds repository-
bound Git metadata update, native section CRUD/membership/order and protected
paginated-history revert. `thread/items/list` is prepared but is rejected at the
transport boundary. No new WebView/IPC controls, experimental capability or
replacement executable were introduced. Earlier P2 package/visual evidence
below is unchanged and is not P3-A visual acceptance.

The final `scripts/verify-workspace.ps1` run passes:

- **719 ordinary Rust tests**, zero failures, **23 ignored** (20 opt-in runtime
  probes plus three existing host tests);
- **three** additional scope-probe oracle tests;
- **171 frontend tests**, with one existing retained-image capture skipped;
- Svelte/TypeScript with zero errors/warnings, generated bundle, 35 design
  scenarios, inline/theme/release probes, locked all-target checks, formatting
  and strict Clippy;
- **121 constructor samples across 67 methods**, including one deliberately
  disabled item-list method, and **10 P3-A response/notification fixtures**;
- dependency audit of 690 crates, with the same four allowed transitive warnings
  and existing scoped advisory exception; no new dependency was added.

The additional `native_p3_git_sections_and_protected_revert` probe was explicitly
run and passes against the real official 0.153.4 server. It uses temporary
`CODEX_HOME`/`CODEX_SQLITE_HOME` and a temporary Git worktree; its three model
responses come exclusively from an inert loopback fixture. Verified outcomes:

- Git branch metadata update and authoritative read-back;
- section list/create/rename/move/remove/delete, preserving the native default
  Pinned section instead of assuming a fresh inventory is empty;
- three persisted turns reduced to the exact one-turn prefix, with native
  closure/revert notifications delivered before the ACK;
- receipt retained until full history read, explicit resume after native session
  release, and recovery of the pre-ACK saved receipt after a simulated client
  restart **without replay**;
- local sentinel file and Git branch unchanged by revert; no tools, account
  inference, personal configuration, personal history or remote service changes;
- item-list attempts rejected before any transport request.

Local evidence: `outputs/p3-a-verification-2026-09-09.log` and
`outputs/p3-a-native-2026-09-09.log`. Implementation boundaries and remaining UI
work are in [P3-A](CODEX_APP_SERVER_P3A.md). Revert has no server-side
expected-revision/CAS parameter and is not a file restore or hidden backup.
No new Computer Use or independent Astra review was performed for P3-A.

## Earlier P0/P1/P2 completion evidence

Date: 2026-09-09. Status: **stable 0.153.4 P0/P1 and the selected P2 product
scope are closed in current source; the complete workspace gate and final
desktop visual audit pass**. This remains an
evidence map for the larger App Server objective; it is not a claim of complete
public-method parity, enterprise registration or a signed production release.
No new Codex engine, tool loop, scheduler or conversation store is proposed.

The version and P0/P1 implementation gates are complete. The P2 product
baseline, its hardening increment and the current deterministic/visual source
gate are complete. The four original independent P2 findings and the follow-up
receipt-persistence finding are closed. Live side
effects and remaining opt-in operational
checks are maintained in
`CODEX_APP_SERVER_GAP_ANALYSIS.md`, while user-operated checks stay in
`CODEX_APP_SERVER_USER_VALIDATION.md`.

The current executable is the **2026-09-09 P2 source preview**:
`outputs/codex-p2-source-preview-2026-09-09/CentralAgent.exe`, 34,142,720 bytes,
SHA-256 `8eb9dce7449eabace427d4c30a1cc2626fd0003a7fb09ed6b7eb474adda521b2`.
Its companion `CentralAgentMcp.exe` has SHA-256
`c355cc9757226c92bd1aa1a5a7c5f69951f852b27ea6813ac895a167b070edce`.
The five-record manifest verifies. It is unsigned and built from uncommitted
source, therefore it is a local test handoff rather than a distributable release.
Later sections retain dated historical build records, not alternative
recommendations.

## 2026-09-09: autonomous closure after the Astra audit

All work in the then-selected P0/P1/P2 scope that can be performed without account changes, OS elevation or external
side effects is complete:

- the four original P2 races/state defects are fixed and covered by Rust plus
  compiled Svelte regressions;
- Windows extended-path presentation is aligned for preferences, skills and
  Goal, including main/graph host acceptance;
- exact-client receipt recovery works after paginated history hydration, is
  persisted before the success event, and all four physical wire-loss orderings
  pass without replay;
- loaded settings remain intentionally static across `config/reload`, while new
  threads adopt the new configured values; compaction-scope write/clear/conflict
  and reasoning-summary preservation pass against isolated App Server hosts;
- main and graph hosts pass native settings, MCP configuration/options,
  draft persistence, form/URL decisions, in-turn MCP decisions, print-only
  command policy and Goal lifecycle checks without account inference;
- all 19 opt-in runtime probes were run. Fifteen pass; four capture actual
  0.153.4 boundaries (missing MCP progress and network-policy callbacks, named
  profile selection requiring experimental API, and review policy stopping the
  command before an approval callback). Central Agent does not fabricate them;
- the complete workspace gate, optimized workspace build, hidden-WebView startup,
  unsigned-binary verification, dependency inventory and release manifest pass.

## Version and contract baseline

- App Server is versioned with Codex CLI, not released separately. OpenAI's
  changelog lists **0.153.4** as the selected latest release for this audit.
- The installed executable and Central Agent's fail-closed runtime pin both
  report `codex-cli 0.153.4`.
- Fresh stable generation matched the repository exactly: 304 JSON files and
  706 TypeScript files, with zero path or content differences.
- The stable request union contains 102 methods. Central Agent constructs 59 of
  them plus `initialize`, for 60 directly serialized request methods. This is a surface
  inventory, not a percentage-complete score.
- The focused contract verifier passes all 111 serialized Rust call samples and the
  native decision/goal/patch/MCP/request/token/lifecycle/history/review/skills/
  settings/activity/model-notice fixtures it recognizes.
- A dedicated generated-schema set now validates 17 P0 notification, inventory,
  rate-limit and unsubscribe samples in addition to the existing fixtures.
- A dedicated generated-schema set validates 11 P1 account, Apps, hooks and
  direct-MCP response/notification samples in addition to focused Rust/UI tests.
- A dedicated generated-schema set validates 15 P2 command, feature, import,
  account-side-effect and feedback response/notification samples.

The live documentation is rolling, so generated stable schemas remain the exact
wire boundary for this executable. Experimental generation adds 56 request
methods; production keeps `experimentalApi: false`.

## 2026-09-09: post-visual-audit remediation

The first full-application inspection found several host and presentation
defects after the P2 source closure. The current source fixes them without adding
protocol authority:

- automatic runtime discovery now tries the explicit override, packaged binary,
  every absolute PATH candidate and then bounded official Windows desktop-app
  build directories. Automatic candidates are version-probed until the tested
  0.153.4 executable is found; an invalid explicit override still fails closed;
- the P2 project scope follows connect and active-project changes automatically,
  while Windows extended-path prefixes stay internal;
- stable thread start/resume/fork response fields hydrate the initial reported
  settings snapshot. Later `thread/settings/updated` events can refine it; config
  reads and composer state still cannot manufacture a report;
- one Apps update racing a list read produces at most one atomic follow-up. Large
  Apps, MCP and feature inventories now have local text filters;
- history fallback previews are redacted, whitespace-collapsed and bounded to
  240 characters with truncation disclosed. Opaque native IDs and raw transcript
  bodies are no longer used as presentation labels;
- model descriptions wrap, P2 import checkboxes use one labelled hit target, and
  local paths are normalized only at the UI boundary;
- native dialogs use the shared visible modal backdrop. Disabled controls,
  loading rerenders, refreshes and open nested dialogs no longer collapse the
  configuration host; the light-theme close control has an explicit surface and
  text color;
- the terminal WebView now passes its IPC through the same owner-envelope parser
  as the main panel, eliminating rejection of the Rust-owned `ui_owner` field.
- native-history linking stores metadata only. Resume/read/fork request no inline
  turns and hydrate bounded ascending `thread/turns/list` pages (16 turns per
  request, 64 per accepted page, 4,096 turns/512 pages maximum), with duplicate
  cursor/turn rejection and atomic publication. A real history whose unbounded
  response exceeded the 64 MiB JSONL cap now loads without a disconnect;
- the JSONL reader rejects an oversized frame before it can grow past the 64 MiB
  limit, explicit active-writer resume rejection releases the pending UI state,
  and WebView2 shutdown returns to its owning UI thread before destruction;
- Explicit Apps refreshes request fresh catalog and installed-runtime snapshots.
  HTTP 401/403 responses become a first-class, non-current account/workspace
  unavailable state; they clear loading/race copy and expose neither upstream
  HTML nor account identifiers. Direct-MCP confirmations wrap complete long
  server/tool names without horizontal scroll.
- A successful disconnected-to-connected transition clears only the superseded
  local connection-required alert in main and graph decision/lifecycle surfaces;
  native operation failures received while already connected remain visible.
- accepted external-agent import remains an active mutation until completion;
  command termination, feedback, reset and email expose explicit rejected,
  failed or uncertain terminal states without unsafe retries;
- `settings_revision` prevents late start/resume/fork responses from replacing
  newer native settings or reviving an invalidated snapshot;
- complete paginated hydration reconciles only the exact native `clientId`,
  persists the binding before success, and owned App Server shutdown first
  closes stdin and allows bounded rollout flush.

The final complete gate passed 693 ordinary Rust tests with 22 tests ignored by
the ordinary workspace gate (including 19 runtime probes), three additional
scope-probe tests, and 171 ordinary frontend
tests with one retained-image capture skipped. It also passed the 111-call
generated 0.153.4 contract verifier, all P0/P1/P2 fixtures, zero-error/
zero-warning Svelte checks, 35 design scenarios, bundle generation, formatting,
locked all-target checks, strict Clippy, provider/reset, inline-script, theme and
release probes. The dependency audit reports the same four explicitly allowed
transitive advisories and no blocking result.

The final visible desktop audit connected the official 0.153.4 runtime and
confirmed the active project, metadata-only link/resume, bounded history
hydration, reported settings, Goal, lifecycle/review confirmations, native Apps,
conversation MCP, hooks, permissions, skills and defaults. Light theme covered
the complete configuration/lifecycle pass; dark theme rechecked the affected
Apps, MCP, Goal and settings states. The Apps service returned HTTP 403 in this
account/runtime state, so selection could not be exercised; the final UI displayed
one explicit account/workspace-unavailable status, no upstream HTML, no stale
refresh notice, no misleading Resume instruction and no false empty-inventory
claim. The same state remained
legible in both themes. The connected MCP inventory truthfully
showed connected, authentication-required, disabled and failed servers, and a
139-tool disclosure; its longest direct-call heading wrapped without clipping or
horizontal scroll. Rename, fork, review, compact, archive/delete, OAuth, direct
tool execution, goal creation and Full access were inspected through their
confirmation surfaces and then cancelled. No destructive operation, external
tool call, permission grant or account-side effect was performed. Clean close,
relaunch and reconnect produced no WebView2 or Application Error dialog.

One protocol-looking fragment seen in an older transcript was traced to the
authoritative persisted native `agentMessage`/`last_agent_message` in Codex's own
session JSONL. The Central Agent projection did not combine a tool payload with
the message. The client therefore preserves the source history instead of applying
an unsafe text-pattern filter; this is recorded as an upstream/source-data anomaly,
not an unresolved presentation-routing defect.

## 2026-09-08: P2 source implementation and verification

The adopted stable P2 scope is implemented without a generic RPC bridge:

- a separately identified App Server sandbox PTY owns `command/exec`, stdin,
  resize and terminate. It accepts bounded argv, fixes cwd to the active local
  project, exposes only read-only/project-write without network/environment
  overrides or Full access, and keeps native process identity Rust-side;
- feature inventory is atomic/paginated, and one fresh beta/stable entry can be
  changed process-wide after confirmation without enabling `experimentalApi`;
- external-agent detection accepts only Rust-selected home/current-project
  scopes. Raw details stay behind opaque handles, import uses an exact frozen
  selection, and progress/history expose counts without raw failure messages;
- reset-credit redemption requires a current positive available count and uses a Rust-owned
  idempotency UUID, retained for the same explicit retry after unknown delivery.
  Workspace email reports sent/cooldown;
- feedback is bounded text-only, explicitly previewed, and always omits logs,
  files, paths, tags and conversation content. Managed requirements can disable
  it, and unknown delivery is never presented as success.

Disconnect never replays a side effect. Feature changes require refresh, imports
require history reconciliation, account actions report uncertainty and sandboxed
processes are reported terminated. Seven focused Rust tests cover authority,
staleness, idempotency and no-replay behavior; two frontend tests cover safe
projection and excluded duplicate authorities.

The complete workspace gate passes 668 ordinary Rust tests with 22 explicit
probes ignored, three additional scope-probe tests and 160 frontend tests with
one retained-image capture skipped. It also passes all 108 serialized call
samples, 15 P2 fixtures, zero-error/zero-warning Svelte checks, the 35-scenario
design contract, generated bundle, release-pipeline probes, Clippy with warnings
denied and dependency audit with four allowed transitive advisories and no
blocking result. No real command, import, account credit, email or feedback
operation was invoked by this source pass.

Native `fs/*`, `thread/inject_items`, `thread/shellCommand`,
`config/value/write`, plugins and marketplaces remain explicit design/maturity
holds. This is P2 product closure, not 102-method parity, a signed release or live
external-side-effect acceptance.

## 2026-09-08: P0 source implementation verification

The P0 source pass completed at 15:32 Europe/Rome. The ordinary workspace gate
passed 648 Rust tests with 22 explicit native/OS probes ignored, 148 frontend
tests with one retained-image probe ignored, Svelte and design checks, all 68
serialized request constructors, the 17 added generated-schema P0 samples,
Clippy with warnings denied, release-pipeline probes and dependency audit. The
audit reported four allowed transitive advisories and no blocking result.

The rate-limit controller also coalesces an invalidation received during an
authoritative read into one follow-up read, so the older response cannot become
the final current snapshot. Local-surface removal preflights active work and
uncertain forks before clearing its draft, then persists the unlink before a
best-effort connection-local unsubscribe.

This verifies source and deterministic fixtures only. No executable was
packaged, no personal account was queried, and no Windows elevation, live
approval or enterprise-registration gate was performed in this pass. The actual
generated account component was also inspected through a disposable local
fixture in light and dark themes for supported, unsupported and refresh-loading
states. Full HD, 2K and 4K layout measurements reported no horizontal overflow,
and the browser console reported no warnings or errors. This is component-level
visual evidence, not a full-application or live-account visual gate.

## 2026-09-08: P1 source implementation verification

The P1 production paths now cover native Apps and exact `$app-id`/`mention`
input, provider capabilities/personality/upgrade metadata, account usage and
workspace messages, direct MCP resources and confirmed tools, bounded extended
forms, completed-turn forks, compatible detached-review ownership, read-only
hooks, process-scoped extra skill roots and MP3/WAV input. All new reads are
bounded, atomically published and invalidated when their owner/thread/directory or
connection changes. Resource URIs, app URLs, hook command/hash data, account
extras and audio paths/bytes are excluded from WebView state.

Focused and workspace evidence passes:

- 94 outgoing constructor samples across 47 methods, validated against the
  generated stable 0.153.4 request union;
- 11 new generated-schema response/notification fixtures for account, Apps,
  hooks and direct MCP;
- 130 runtime tests with 19 opt-in tests ignored, including detached-review
  response/notification ordering and uncertain recovery;
- focused Rust tests for attachments, catalog sanitization, direct MCP, Apps,
  hooks and extra roots;
- 12 frontend tests covering Apps selection/reference rendering, hook escaping,
  opaque resource dispatch and inventory-bound direct-tool confirmation.
- the complete workspace gate: 659 ordinary Rust tests with 22 explicit probes
  ignored, three additional scope-probe tests, 158 frontend tests with one
  retained-image capture skipped, Svelte/design validation, generated bundle,
  release-pipeline probes, Clippy with warnings denied and dependency audit with
  four allowed transitive advisories and no blocking result.

The selected schema contains no `isPinned`, despite that field appearing in the
rolling guide, so pinning was not implemented or simulated. The selected runtime
rejects detached review for paginated histories; only explicitly reported legacy
histories expose the separate destination. Arbitrary instruction/config fields,
ephemeral forks, structured output, host tool output and turn triggers remain
schema-tested constructors rather than unowned WebView controls.

## 2026-09-08: P1 closure and read-only live acceptance

The P1 source set was first committed as `ee0d5bc`. Final acceptance then found
a real configured-server interoperability defect: four official MCP tool schemas
were deeper than the generic JSON argument/result limit, so refreshing the whole
inventory failed. Schema inspection now has its own bounded depth budget while
direct arguments and returned structured content retain the stricter limit. The
same pass corrected the serialized Rust/WebView contract from `entry_id` and
`input_schema` to the UI's `entryId` and `inputSchema`; regression tests prove
that resource handles and sanitized schema previews are usable without publishing
raw URIs or raw schemas.

The corrected release build was launched with a disposable Central Agent data
directory and the existing official Codex 0.153.4 executable. Read-only live
acceptance confirmed:

- `ChatGPT connected`, six model profiles, three managed permission profiles,
  two rate-limit buckets and all three provider capability flags;
- a successful global MCP refresh with four configured servers after the failing
  pre-fix case was reproduced;
- one server reporting 138 tools and 40 resources, with all 40 **Read resource**
  controls enabled by opaque handles;
- stable Full HD Settings rendering in light and dark, with no observed clipping
  or stale error after refresh. The deterministic UI gate covers 2K and 4K layout
  profiles; no unsupported claim of physical 2K/4K monitor inspection is made.

No model prompt, App action, resource read, direct tool call, OAuth flow, account
change, Windows sandbox setup or OS picker was performed. Those potentially
quota-consuming, external or system-changing journeys remain explicit user-run
checks and do not represent missing P1 implementation. Enterprise registration
and the later P2 methods were outside that P1 closure.

## 2026-09-08: user-run summary diagnosis and correction

The user confirmed account readback and a completed main-chat prompt in the
23:01 preview after supplying CENTRAL_AGENT_CODEX_BIN. Full access also worked
after choosing native Codex permissions rather than the unrelated host setting.
They observed commands and final output, but no reasoning summaries.

The read-only `reasoning_diagnostic` example queried only this application's
four linked native conversations through thread/read and scoped config/read.
It did not resume threads, send prompts, change config, read credential files,
or print transcript content. The completed turn had three commands, one
commentary message, one final answer, and three reasoning items with **zero
summary characters**. On-disk summary preference was unspecified. History
does not prove which transient delta events were emitted, nor the loaded
session's effective summary setting; this is not proof of a renderer failure.

The client now explicitly requests Automatic public summaries by default via
native turn/start.summary. Main and graph expose Automatic/Concise/Detailed/Off
and Inherit native setting. Inherit omits the override and does not reset an
already-loaded session. Selection is frozen with queued input, owner-scoped,
session-only and separate from permissions. No shared configuration is written.
Empty reasoning items do not create blank rows; later summary deltas reveal
the same native identity. Commentary remains an ordinary progress message.

Evidence: native_profile_transitions_apply_model_effort_speed_and_access passed
eight real 0.153.4 turns against the owned loopback fixed-response model, checking
inherited concise, explicit auto/detailed and off at the actual model request.
No account inference or tools were used. Rust summary projection and structural
DOM tests cover incremental text, stable identity, scroll and final collapse;
the shared component handler test covers main/graph ownership and busy state.
Actual model summary emission in the user's next prompt remains to be verified;
no private reasoning is requested or fabricated. Other acceptance gates below
remain open. No visual tests were performed.

Release verification completed 2026-09-08 at 02:02:55 Europe/Rome:
`outputs/codex-summary-preview`, 33,005,056-byte CentralAgent.exe, SHA-256
`0667b2a37f64a15ae34645cf229c16ef911ea76350f4490275d6237208daadbf`.
All five payload hashes and lengths were independently checked against the
release manifest. The pipeline passed 626 Rust tests (22 optional ignored), 145
frontend tests (one optional capture test ignored), stable schema verification,
clippy, provider-reset checks and hidden-WebView functional startup. Thirteen
allowed dependency warnings remain. The earlier summary diagnostic and native
no-inference records remain in `%TEMP%/central-summary-release.log` and
`%TEMP%/central-summary-native.log`; the current artifact identity is the release
manifest above.

### Native summary stream acceptance (test-only follow-up)

`native_public_summaries_stream_before_final_and_survive_history_read` now runs
the official 0.153.4 process against a disposable loopback Responses SSE fixture.
The fixture sends documented summary-part/text deltas and commentary; App Server
itself translates them into JSON-RPC item events and persists their history.
For each of two independent native threads, the production transport and mirror
observed four summary deltas across two sections and two commentary deltas while
the turn was active, before any final-answer item. Stable reasoning identity,
completed text, exposed-summary-only projection and exact native thread/read
rehydration were asserted. A sibling stayed completed while the next ran.
The actual model request contained summary=auto and the temporary configuration
was unchanged. Unexpected tool or authority requests fail the test; no approval,
account inference, personal config access, UI launch or visual check occurred.

Log: `%TEMP%/central-summary-stream.log` (PASS). Ordinary runtime tests plus the
existing eight-turn profile and two-turn inherited-summary probes were rerun
successfully; `%TEMP%/central-summary-stream-regression.log`. Runtime all-targets
clippy with warnings denied and formatting also passed. The public native
events, mirror and history are directly verified here, not full main/graph UI
delivery; the previous Rust projection and structural DOM tests cover those
separate presentation boundaries. This is deterministic protocol evidence, not
a claim that a real model always emits summaries. Only test code and documents
changed in this follow-up; the previously verified preview hash is unchanged.

The official [App Server guide](https://learn.chatgpt.com/docs/app-server) places
authentication, native history, approvals and streamed agent events behind its
protocol. The current client launches official CLI **0.153.4** over stdio and
validates outgoing calls against that version's generated stable schemas.
Newer documentation does not authorize guessing fields or enabling experimental
features absent from the selected stable contract.

## Requirements and inspected evidence

| Requirement | Evidence inspected in this audit | Result |
| --- | --- | --- |
| Native transport, lifecycle and schema fidelity | `runtime.rs`, `transport.rs`, `wire.rs`; `transport/tests.rs` uses real subprocess pipes for chunked output, failed initialization, opposite-direction requests, out-of-order replies and EOF; `verify-app-server-contract.mjs` validates serialized Rust calls/decisions, not hand-written request examples alone | Implemented and covered by the ordinary verification suite |
| Native main/graph identity and other-provider preservation | `conversations.rs` persists native IDs/receipts, not model history; `acceptance/providers.rs` verifies exact new input, same thread/directory and separate persisted Claude fixture; main/graph provider and profile-host logs end in PASS | Verified for the Codex boundary; not a new claim of Claude runtime acceptance |
| Start, Queue, native Steer, Stop and ACK-scoped drafts | `turns.rs`, `acceptance/delivery.rs`, `native-steering.test.mjs`, `conversation-drafts.test.mjs`; actual main/graph delivery logs verify three native turns, explicit steer, queue once, interruption, retained drafts and native readback; PNG Steer logs verify frozen selected input | Verified; no automatic resend or model-work budget introduced |
| Recovery after process loss or uncertain submission | Main/graph physical relay loss before native acceptance and after acceptance/before client ACK; persisted receipt, bounded paginated read, exact `clientId` reconciliation and same-thread continuation | Verified in all four orderings with no automatic replay; unrelated upstream callback gaps remain explicit |
| UI ownership, streaming, disclosures, usage and request controls | Actual component-handler tests in `native-steering`, `native-lifecycle` and `native-requests`; `conversation-events`, `timeline-dom`, `native-usage` and draft tests check owner isolation, retained row/disclosure/scroll state, incomplete usage and newer drafts. Actual native generated-image bytes now also pass the shared Rust projection and compiled Svelte component | Automated coverage plus actual generated-media capture verified below; OS-dialog interaction remains separate |
| Unknown notifications remain diagnosable | The audit first found discarded notifications. The subsequent `diagnostics.rs` collector is called after unprojected native notifications; the account disclosure exposes stable-schema names, fingerprints and counts only. Real-pipe, Rust privacy/state and compiled Svelte tests cover it | Corrected; no payload logging or synthetic native action |
| Native auth, models and requirements | Runtime version check and official no-inference handshake example; account/catalog/config controllers and component tests; user confirmed account readback and a completed prompt on 2026-09-08 | Account readback and one main-chat execution confirmed; fresh login/logout and Windows setup remain separate unverified checks |
| P0 warning and security presentation | Typed bounded projections cover immediate turn errors, thread/global/config/deprecation/guardian/strict/automatic-review/world-writable notices, terminal interaction and opaque moderation presence; Rust/UI tests assert escaping, stale state and absence of private action/rationale/stdin/process/metadata fields | Implemented and fixture-tested; automatic-review payload remains marked unstable and no experimental capability is enabled |
| P0 loaded subscription lifecycle | Production startup/refresh calls paginated `thread/loaded/list`; local chat/graph/workspace removal durably unlinks and dispatches `thread/unsubscribe` only for loaded bindings; runtime and host tests verify atomic pages, owner isolation and non-deletion semantics | Implemented automatically; no claim that this deletes native history |
| ChatGPT subscription Cloud chats | The history dialog launches the official hidden `codex cloud list --json` and `codex cloud diff` commands with the same isolated signed-in profile; typed bounded parsing, exact task/URL validation, pagination, automatic visible-task refresh and owner/view invalidation are covered in Rust and Svelte tests | Implemented as a distinct Cloud source; full transcript/follow-up stays on the validated first-party page because App Server does not expose Cloud tasks as local threads |
| P0 permission/account invalidation policy | Settings renders read-only paginated permission profiles and authoritative bounded rate-limit buckets; sparse updates cause a refetch and coalesce one follow-up when a read is already in flight; API-key/Bedrock states show explicit subscription-only policy and remain non-runnable | Implemented and contract/component-tested; experimental profile selection remains disabled |
| P1 Apps/model/account | Atomic list/installed/read Apps controller, exact native mention input, capability/personality/catalog sanitization and bounded usage/workspace-message projections; Rust and compiled component tests cover stale/foreign state and private-field exclusion | Closed; live account/model/capability state verified read-only, while an external App invocation remains opt-in |
| P1 direct MCP and extended forms | Resource URIs remain behind opaque handles; direct tools require exact inventory-bound confirmation and bounded JSON; initialization advertises `openai/form` only with normalization/result validation | Closed; real global inventory and opaque resource handles verified, while invoking a resource/tool/form remains opt-in |
| P1 forks/review/hooks/skills/audio | Completed-turn `lastTurnId`, exact detached `reviewThreadId` ownership, read-only hooks, canonical non-secret extra roots and signed MP3/WAV snapshots with explicit audio modality | Closed to the stable 0.153.4 boundary; paginated detached review is a verified runtime rejection and OS/external journeys remain opt-in |
| Enterprise client identity | One `CLIENT_NAME` constant drives both `clientInfo.name` and `serviceName`, with handshake/constructor tests | Code side implemented; OpenAI registration remains an external enterprise release gate |
| Native approvals, MCP, tools and local policy | Existing command/file/session, MCP form/OAuth, config and review acceptance increments | Partially accepted; retain the specific outstanding gates below |
| Preserve other providers and remove the retired client | `verify-provider-reset.mjs` checks retired transport paths/routes are absent, remaining provider IDs exist and embedded UI references resolve; provider round-trip test excludes legacy transcript from native requests | Verified boundary, not a full end-to-end test of every other provider |
| Usable local build and documentation | Full workspace pipeline passed; the current P2 preview manifest independently verifies all five records. Latest local executable is `outputs/codex-p2-source-preview-2026-09-09`, identified above; integration guide, DESIGN.md and scenarios retain native behavior and limitations | P0/P1/P2 source/build verification closed; clean signed distribution and opt-in native/OS journeys remain separate |

Paths in this table are relative to `src/browser/app_server`,
`crates/central-agent-codex-runtime/src`, `scripts` or `ui/tests` as applicable.
The test implementations were inspected as well as their result logs. A PASS
from an isolated fixture is never promoted to account-backed or OS acceptance.

## Outstanding work and limitations

User-operated gates are now separated into
[the remaining user validation checklist](CODEX_APP_SERVER_USER_VALIDATION.md).
The account readback, setup confirmation and OS picker labels were checked
against current Svelte/Rust code; that inspection is not a successful OS test.
No setup, elevation, login/logout, external browser launch or personal-file
selection was performed by the agent. Await the user's choice/results for these
gates rather than repeating unrelated tests or reporting a passing substitute.
Native runtime limitations below remain independent; their isolated probes have
now been run, but the absent callbacks cannot be promoted to supported behavior.

- The autonomous P1/P2 visual inspection is complete. Run personal/live Apps,
  direct MCP, extended-form, hooks and audio-picker journeys only with explicit
  user intent. Focused schema/unit/component evidence is not promoted to
  environment acceptance.

- Unknown-event diagnostics was the next implementation gap from the audit and
  is now corrected. Its memory-only Settings disclosure remains separate from
  native history, approval state and composer delivery; no event content is saved.
- Verify the explicit Windows sandbox setup flow only with the user's action;
  no automatic setup, elevation or full-access fallback. Existing preset tests
  are not proof of a successful OS setup.
- Retain actual network-policy amendment acceptance as an open gate. Command
  policy and ordinary command/file decisions have separate successful evidence.
  The new isolated Windows native proxy diagnostic reached its owned loopback
  target after exact command approval, but emitted no separate network callback.
  It intentionally fails the network-denial oracle; do not count it as approval
  coverage or infer that domain controls are broken on every runtime/profile.
  The follow-up fixed `.invalid` destination also produced no network callback,
  but failed with a transport connection-aborted error. Neither outcome proves
  native network-denial acceptance; do not broaden permissions to force a pass.
- Retain OS file-picker interaction and OS-browser OAuth launch as separate
  gates; post-picker snapshots and loopback OAuth have already been tested.
  Actual generated-image output is now verified at the native transport/history,
  full pixel-decoding, shared main/graph Rust projection and compiled Svelte markup
  boundaries below. The old image-only probe's interruption was a test restriction,
  not a client/runtime failure. This does not claim visual review or a complete
  live image-generation journey initiated through both desktop composers.
- Native review with actual approval callbacks remains unverified. Two review
  diagnostics produced no callback; do not simulate one.
  The later isolated fixed-model diagnostic uses production read-only thread and
  inline-review constructors. It records a native `blocked by policy` tool result
  without an approval callback or command-execution item. This trace rules out
  a UI-hidden approval for that attempt, not all possible review configurations.
  The isolated real-runtime Goal probe now confirms that activation itself
  starts a native turn without client `turn/start`; pause permits the current
  turn to finish. It also verifies native notifications/history, state changes,
  sibling isolation and paused-goal persistence across restart. This removes
  uncertainty about native activation. Subsequent hidden-host main/graph tests
  now pass the actual compiled Goal control flow, active chat/Stop presentation,
  pause/clear, original native history and draft/sibling isolation. These use a
  loopback fixed-response model, not account-backed reasoning or tool execution.
- The native MCP reload ACK did not update tools in an already loaded thread in
  the recorded 0.153.4 probe. Document the observed native limitation; do not
  replace it with a custom reload engine or silently retry inference.
- Stable-contract/selected-runtime restrictions (including experimental
  permission/question modes and the observed detached-review rejection) remain
  explicit. They are not license to emulate those features in Central Agent.
  The named-permission inventory itself is stable and now verified on 0.153.4:
  one-item pagination returns the owned named read-only profile with native
  description and `allowed: true`. Direct selection via `thread/start.permissions`
  returns -32600 and explicitly requires `experimentalApi`; the rejected request
  creates no thread or model work. A config-override diagnostic completed READY
  but obtained no active-profile provenance. It failed, not an accepted fallback.
  Managed-profile selection is therefore still unavailable in the stable client;
  do not translate named profiles into locally reconstructed sandbox policies.

## Verification records

Pending-request ordering correction (2026-09-07): the new regression first
reproduced a real display defect: textual BTreeMap keys placed `request:7:10`
before `request:7:2`, moving existing approval cards when later requests arrived.
The private pending record now retains its numeric arrival sequence; `views`
orders each owner's cards by that sequence before cloning the public projection.
Opaque native IDs, tickets, decision contents and server-owned lifecycle remain
unchanged; this is not an execution queue or a request count limit.

The regression inserts 120 interleaved callbacks into main/graph owners, including
multiple callbacks for one item and native text/integer IDs in reverse order.
It verifies each intermediate snapshot, digit boundaries, sending state, exact
native response routing, removal, late duplicate completion and sibling isolation.
It failed before the change (`%TEMP%/central-native-request-order-before.log`)
and passes afterward with all 13 request tests
(`%TEMP%/central-native-request-order-after.log`). All eight frontend request tests
also pass, including snapshot ordering and existing compiled-card/typed-form
checks. No account inference, tool execution or visual inspection was performed;
these tests do not certify a new live native approval scenario or pixel/scroll QA.

Published build after that correction: **2026-09-07 23:01:03 Europe/Rome**,
`outputs/codex-app-server-preview/CentralAgent.exe`, 32,956,928 bytes, SHA-256
`23feca8787bb40491971097f7654180207bc4b133ef8b4430f2eb04fd4fd3ae6`.
Full log `%TEMP%/central-native-request-order-release.log`: 622 Rust tests and
144 frontend tests passed; 21 explicit Rust probes and one optional frontend
capture test skipped. Formatting, generated contract, Svelte/design checks,
clippy, provider reset, release-pipeline probes and hidden WebView startup passed.
The same 13 allowed dependency warnings remain. Independently checked all five
manifest artifact/support records at `2026-09-07T21:01:19.4315823Z`.
This is the current unsigned 0.153.4-compatible preview, superseding earlier
build labels below; it does not close the remaining native/OS acceptance gates.

Native permission-profile probe (test/docs only; the 22:42 executable unchanged):
`cargo test --locked -p central-agent-codex-runtime
native_permission_profiles_respect_stable_api_boundary -- --ignored --nocapture`
passes in `%TEMP%/central-native-permission-boundary.log`. It checks four native
inventory entries through one-item pages, repeated-cursor protection, exact
beta-selection refusal, no thread creation, no model requests, unchanged owned
config and empty owned workspace. It does not certify managed-deny enforcement,
profile switching or OS sandbox enforcement. No experimental capability enabled.

`native_named_permission_profile_is_reported_and_inherited` is a separate opt-in
diagnostic and currently **fails**: `%TEMP%/central-native-permission-profile.log`
records one completed local fixed-response turn and zero native settings events.
The required active profile is not available; no name is inferred from a readOnly
compatibility sandbox. It cannot certify inheritance and must not be counted as
a passing gate. Both use disposable owned native config/history, no personal
account, credentials, external model, tools, OS setup or visual checks.

Post-change verification: `cargo fmt --all -- --check`, 117 ordinary runtime
tests (18 explicit native probes ignored), and all-target runtime clippy with
warnings denied passed. Logs: `%TEMP%/central-native-permission-unit.log` and
`%TEMP%/central-native-permission-clippy.log`. These are focused test/docs checks,
not a new full-workspace release run. The previously verified 22:42:28 executable
remains unchanged; no rebuild was needed for test-only source and documentation.

Native MCP progress integration (after the 22:32 preview): the display mirror
now consumes `item/mcpToolCall/progress` and the shared main/graph presentation
shows ordered plain messages in the existing tool disclosure. Item completion
blocks late progress; messages remain memory-only and never become native result,
history or permission. Two generated-schema fixtures, two runtime tests and the
shared main/graph presentation test pass. A subprocess test verifies the disposable
MCP fixture emits correct ordered `notifications/progress`, including token zero.

Actual runtime diagnostic `central-native-mcp-progress-token.log` **fails**:
the owned MCP tool received a native progress token, but no matching App Server
progress notifications arrived before native completion. No client opt-out is
configured. This is an observed Code Mode/MCP route limitation in the tested
0.153.4 setup, not proof that all native MCP routes omit progress. The diagnostic
is separate from the existing six-case elicitation acceptance, which still
**passes** in `central-native-mcp-progress-regression.log`: form/URL accept,
decline and cancel, exact ownership/resolution, tool results and saved continuation.
Both probes use an owned native profile and local fixed model/MCP fixtures; no
account inference, personal config, credential access or external URL opening.
No fake App Server notification or custom MCP execution path was introduced.

`central-native-mcp-progress-release.log` records the full passing pipeline:
**621 Rust tests**, **143 frontend tests**, 19 opt-in Rust tests and one frontend
capture test skipped; generated schemas, format/check/clippy, design/scenarios,
release pipeline and hidden WebView startup verified. The same 13 allowed
dependency warnings remain. These totals exclude the failed progress diagnostic.
Latest executable: **2026-09-07 22:42:28 Europe/Rome**,
`outputs/codex-app-server-preview/CentralAgent.exe`, **32,964,096 bytes**, SHA-256
`c03c22720336310f35b766a1df2403ec8227336f6919f7a89299ef7269b12fca`.
All five manifest entries independently verified at `2026-09-07T20:42:46.8187685Z`.
Unsigned local development build, external official CLI 0.153.4 required; no
visual test, production-signing claim or completed-goal claim.

Native intermediate file-change correction (after the 22:02 preview): source
inspection found `item/fileChange/patchUpdated` was absent from the display
mirror's supported notifications, so revised proposed diffs were not reflected
until a subsequent full item. The client now consumes the official replacement
snapshot, preserving thread/turn/item identity, native completion authority and
newer events across an in-flight history read. Three schema-backed runtime tests
cover replacement/empty updates, sibling isolation, final outcomes, malformed
payloads and mismatched item kinds. The host projection test checks both main and
graph identities, changed paths/diffs and rejection without claiming file writes.
This is a presentation fix, not native patch execution or a simulated runtime.

`central-native-patch-stream-release.log` passes the full workspace pipeline:
**618 Rust tests**, **142 frontend tests**, 18 opt-in Rust tests and one optional
frontend capture test skipped. Generated protocol validation includes the three
patch snapshots; format/check/clippy, design/scenario checks and hidden WebView
startup pass. The same 13 previously allowed dependency warnings remain.
Published executable at **2026-09-07 22:32:39 Europe/Rome**:
`outputs/codex-app-server-preview/CentralAgent.exe`, **32,963,584 bytes**, SHA-256
`e4055f202dc6ff4e89441ba51d5b32fa53d84f9d1b4c9dcc5fa95c79f41d51f4`.
All five manifest entries independently verified at `2026-09-07T20:33:02.6512065Z`.
This unsigned local build still requires external official CLI 0.153.4 and
supersedes the 22:02 preview. No visual tests or account-backed inference occurred
in this correction. Existing native/OS acceptance gaps remain open.

Actual generated-image acceptance (2026-09-07, after the 22:02 preview):
the probe now permits native preparation commands under the same explicitly
requested workspace-write/network-disabled policy. It sends no approval or new
permission, does not enable features, and still requires a real completed native
`imageGeneration` item and identical embedded result in `thread/read` history.
The first preparation-enabled attempt produced a native image but failed an
incorrect assumption that `savedPath` must be inside cwd. That assertion was
removed: savedPath is metadata, not read/delete authority. Native saved images
were left untouched. The probe now preserves actual wire evidence before later
checks/cleanup, including on a subsequent readback failure.

`central-native-image-embedded.log` records the next explicitly opted-in request
on server-selected GPT-5.6-Sol: completed image, 1,019,232 embedded characters,
no failure and identical persisted result. Only its owned native thread was
deleted. Retained evidence directory:
`C:\Users\<user>\AppData\Local\Temp\central-native-image-oRF8mP`.
`central-native-image-projection.log` records full decoding to **1254 × 1254**,
identical live/history image rows using the production shared main/graph
projection, and no read of savedPath. `central-native-image-component.log`
records three passing frontend checks, including the actual captured row through
the compiled `NativeMedia.svelte`: embedded preview, dimensions, enlarge/close
controls, closed dialog and no remote/file URL. This is nonvisual functional
acceptance of actual native media, not a synthetic generation event.

Reproduce the capture checks without more inference by setting
`CENTRAL_AGENT_TEST_NATIVE_IMAGE_DIR` to that exact retained directory, running
`cargo test --locked -p central-agent captured_native_image_decodes_and_projects_from_live_and_history -- --ignored --nocapture`,
then `node --experimental-strip-types --test ui/tests/native-media.test.mjs`.
Without the variable the frontend capture check is skipped; ordinary fixtures
are not promoted to live-media coverage. The Rust capture check is opt-in/ignored.
Only examples, tests and documentation changed; the published 22:02 executable
is unchanged. Both account-backed image attempts consumed Codex allowance;
there was no API key, alternate image service, automatic permission grant or
visual test.

Native image capability/generation checks (2026-09-07, after the 22:02 preview):
`central-native-image-capabilities.log` records actual 0.153.4 handshake, seven
catalog entries, true native provider image-generation bound, and stable/enabled
`image_generation` from fully paginated feature metadata. This preflight sends
no prompt and enables nothing. Schema evidence: `ModelProviderCapabilitiesReadParams`
is empty in the selected version (not invented per-model fields), and
`ExperimentalFeatureListParams` supports cursor/limit.

`central-native-image-generation.log` records one explicitly opted-in actual
account-backed prompt on server-selected GPT-5.6-Sol, in an owned temporary
workspace with workspace-write/network-disabled policy. The probe observed
`commandExecution` before any `imageGeneration` result, requested interruption
and successfully deleted only its own native thread. The diagnostic **failed**;
there is no generated image or host projection to verify. This test's deliberately
image-only boundary is not a claim that native preparation commands are a bug.
`central-native-image-no-consent.log` confirms the example exits before runtime
startup without `--allow-test-inference`. All-target runtime Clippy and formatting
pass. Changes are diagnostic examples/documentation only; no visual tests,
configuration changes or new release were made. The 22:02 preview remains current.

Full workspace/build audit (2026-09-07 **22:02:50 Europe/Rome**):
`central-native-final-audit-release.log` records **614 passing Rust tests**,
**17 opt-in ignored**, **142 frontend tests**, zero Svelte errors/warnings,
format/check/clippy, design verification (62 modular UI files, 32 scenarios),
63 serialized Rust calls and 19 decisions against the generated 0.153.4 protocol,
provider-boundary checks, release-pipeline tests and hidden WebView startup.
The dependency audit retains **13 existing allowed warnings**; this is not a
claim of an advisory-free dependency graph. No ignored live diagnostic is
counted as passing. No visual tests were run.

Latest executable: `outputs/codex-app-server-preview/CentralAgent.exe`,
**32,963,072 bytes**, SHA-256
`4f927ec630060b23d7e58d0ca61ba0bddb4d1f9b2bc7c70ac0f1e9bd70a0cfb5`.
All five manifest records were independently verified at
`2026-09-07T20:03:10.5047864Z`. It remains an unsigned local development build
requiring external official Codex CLI 0.153.4. The prior preview was replaced
through the checked release publisher; source and other-provider data were not
removed. This verifies the build/handoff checklist item, not the entire goal.

OS picker audit: main `select_chat_files` and graph `manage_graph_files(Select)`
call real `rfd::FileDialog::pick_files`; existing media acceptance supplies the
result paths after that boundary. Neither static inspection nor those passing
tests prove selecting/cancelling in the actual Windows dialog. This gate remains
open; no dialog was launched during this audit.

Deterministic native review-policy diagnostic (2026-09-07):
`central-native-review-policy.log` records CLI 0.153.4, isolated native home and
empty workspace, loopback Responses fixture and one fixed print-only command.
The production `start_thread(ReadOnly)` and `start_review(Custom)` constructors
are used. The fixture requests no escalation, rule or network authority. The
native tool output says `blocked by policy`, no approval or command-execution
item is emitted, `exitedReviewMode` arrives and native history has two turns.
The workspace remains empty. The explicit diagnostic **fails** because no native
approval was resolved; normal review completion is not a replacement oracle.
The initial diagnostic startup failed with redundant policy config entries;
the recorded run instead selects policy through the production thread request.
No production configuration was changed. The fixed-output safety unit test and
runtime suite passed: **112 tests, 15 opt-in ignored**, in
`central-native-review-policy-runtime-tests.log`; Clippy and format passed.
No account-backed inference, UI/OS permission change or visual test. This is test
code/documentation only; the published executable remains the 21:29 preview.

Reserved-host network diagnostic (2026-09-07, after the loopback diagnostic):
`central-native-network-invalid.log` records the same actual CLI/profile with
the fixed destination `central-agent-network-denial.invalid`. The native command
was allowed once after complete argv/action and owned working-directory checks.
It failed with a transport connection-aborted error; there were two local model
requests, zero loopback sentinel requests and no network callback. The opt-in
test correctly **failed**, not passed as a result of an unreachable hostname.
The oracle now correlates network resolution by exact request ID and thread,
separately from prerequisite command approval, and requires a failed/declined
native command. A unit regression rejects prerequisite, sibling, wrong ID type
and missing-key resolutions. `central-native-network-invalid-runtime-tests.log`
records **111 passing runtime tests, 14 opt-in ignored**. No public service,
account-backed model, Windows setup, permission change or visual test was used.
This test/documentation-only increment does not change the published executable.

Native network diagnostic (2026-09-07, after the 21:29 preview):
`central-native-network-denial.log` records actual CLI 0.153.4 with an owned
temporary home, a `:read-only`-derived named profile, network proxy enabled,
and only local model/target fixtures. A native positive loopback port was needed:
port zero was rejected when reserving shared managed Windows proxy ingress.
The runtime returned a readOnly sandbox and requested approval of the exact
PowerShell expression. The test checked its entire native proposed argv, known
system/bundled executable and full command action, then sent Allow once only.
It did not save a command rule or send a network-policy response. Observations:
two local model requests, one target request, command exit success/HTTP 200,
and **no networkApprovalContext callback**. The expected denial therefore fails.
This is not account-backed inference, general network-policy acceptance or a
claim about why this profile allowed a literal loopback destination.

Two ordinary unit tests reject wrong ports, extra argv, altered command actions
and text injection in this diagnostic. `central-native-network-runtime-tests.log`
records 110 runtime tests passed, 14 opt-in tests ignored; all-target runtime
Clippy and formatting passed. Only test code/documentation changed, so the
published executable and its verification below remain unchanged. No visual
test, Windows setup or full-access fallback was performed.

Previous published preview: **2026-09-07 21:29:59 Europe/Rome**, at
`outputs/codex-app-server-preview/CentralAgent.exe`, **32,963,072 bytes**,
SHA-256 `26557090fe3dd1bf1ba1d1c1c7fb96612da2e4a5627ec209c14ce11d0e76ea43`.
All five release records were independently verified at
`2026-09-07T19:30:33.8179958Z`. `central-native-goal-host-release.log` records
610 passing Rust tests, 15 opt-in ignored tests, 142 frontend tests, zero Svelte
errors/warnings, format/check/clippy, generated-contract/provider-boundary checks,
the dependency audit with 13 existing allowed warnings and hidden WebView startup.
It is still an unsigned development preview requiring external official CLI
0.153.4, not a claim that the entire objective is complete.
`central-native-goal-release-main.log` and `central-native-goal-release-graph.log`
also record passing Goal host acceptance on this exact published executable.

Native Goal desktop-host increment (2026-09-07):
`central-native-goal-host-main.log` and `central-native-goal-host-graph.log`
record Debug acceptance through actual compiled controls and the official
runtime. The initial graph harness omitted its composer-state event observer:
it reported inactive while native output and Stop were present. Installing the
same test observer used by delivery acceptance resolved that false negative;
no production UI or scheduling change was necessary. The main harness likewise
now targets the Goal button beside (not inside) the history disclosure.
The owned profile and its test histories are removed after the child closes;
neither mode touches personal native history or account configuration.

Additional native Goal evidence (test-only increment, 2026-09-07):
`central-native-goal-activation.log` records the passing opt-in
`native_goal_activation_and_paused_restart_use_server_execution` test against
actual CLI 0.153.4 with a temporary profile and loopback Responses fixture.
The first draft incorrectly rejected native `turn/started`; that observed event
changed the test to follow native execution instead of assuming metadata-only
activation. An unused sibling has no durable rollout after restart in this probe;
its isolation is verified while loaded, not by inventing a persisted conversation.
`central-native-goal-runtime-tests.log` records 108 ordinary runtime tests passed,
13 opt-in tests ignored. Runtime all-target Clippy and format checks passed.
Only test code and documentation changed; the published executable below is
unchanged. No account-backed inference or visual test was performed.

Logs are in `%TEMP%`. This audit reread:

- `central-native-delivery-acceptance.log`, `central-native-graph-delivery.log`;
- `central-native-providers-main.log`, `central-native-providers-graph.log`;
- `central-native-profile-host-main.log`, `central-native-profile-host-graph.log`;
- `central-native-png-steer-main.log`, `central-native-png-steer-graph.log`;
- published `central-native-restart-wire-{before|after}-release-{main|graph}.log`;
- `central-native-restart-wire-release.log` (full published build pipeline).

The audit initially verified the 20:40 executable (SHA-256
`b8fa9a1183bfc269439c2433d5808cb388868a0ca016bea98d3940d41d7fbd5f`).
It was replaced by the diagnostic increment's **21:05:11 Europe/Rome**
build at `outputs/codex-app-server-preview/CentralAgent.exe`: **32,916,480 bytes**,
SHA-256 `f28102b7e4b890176f8ae11b992be4f2d76387d8e528b417e79357e24cd7a7de`.
That previous build requires external official Codex CLI 0.153.4. All five release records
were independently reverified at `2026-09-07T19:05:28.0913337Z`.
`central-native-diagnostics-release.log` records 610 passing Rust tests, 142
passing frontend tests, 14 opt-in ignored tests, full static verification and
hidden WebView startup checks. No visual testing or model inference was used.

The earlier audit verification (before the diagnostic code change) also passed:

- `scripts/verify-workspace.ps1`: 606 Rust tests passed, 14 opt-in tests ignored
  by default, 141 frontend tests, Svelte with zero errors/warnings, generated
  contract checks, provider-reset boundary, format, all-target check/clippy,
  design/static checks and the dependency audit with 13 existing allowed warnings.
  Log: `central-native-completion-audit-workspace.log`.
- `cargo run --quiet --locked -p central-agent-codex-runtime --example handshake`:
  actual official 0.153.4 initialization, account/read, configRequirements/read
  and paginated model/list passed with seven entries. No inference, personal
  thread listing, credentials-file reads or configuration writes.
  Log: `central-native-completion-audit-handshake.log`.
- `git diff --check`: passed (existing line-ending conversion warnings only).

No visual tests or account-backed model calls were made during that historical
audit. The later physical wire-loss diagnostic and exact paginated receipt
reconciliation are now corrected and pass in four orderings, as recorded at the
top of this file. External account/OS gates above remain person-operated;
completed core checks must not be rerun as a substitute for addressing them.

## Windows console-window regression (2026-09-14)

The production App Server launcher now creates one hidden inheritable console
before resuming the contained process. The focused Windows regression executes
a real PowerShell process which starts a second PowerShell child; both report
the same nonzero console handle and both report that its window is invisible.
A separate full-duplex JSONL fixture passes through this launcher and verifies
request/reply traffic plus clean shutdown. The ordinary runtime suite passes
with these checks and no account inference or visual automation.
