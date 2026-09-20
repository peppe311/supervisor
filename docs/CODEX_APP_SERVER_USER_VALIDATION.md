# Codex App Server: live validation and remaining checks

## 2026-09-10: Review, delivery and recovery closure

The previous Review block is resolved for the tested inline path. The native
read-only scope is still checked in full. Paginated history now completes before
review reconciliation, and a rate-limit refresh no longer disables a configured
provider. Main and graph acceptance with the remembered account passed dialog
cancellation, one confirmed review, native completion/readback, one displayed
instruction/result and independent drafts. Native Windows inspection also
confirmed the completed main review and its draft after restarting the app.

Main and graph live delivery passed steering, a held queue, exactly-once queue
drain, Stop and explicit reconnect/read/resume. Main recovered from termination
of its owned Codex process during streaming and continued the same native thread.
Graph recovered from a delayed real host ACK, with uncertainty persisted until
authoritative history inspection. A graph conversation also survived normal
whole-application restart in two separate processes. Acceptance drivers now
wait for paginated reads after both Read and Resume before taking another action.

Goal activation was verified in main and graph against the real native runtime
using an isolated local model fixture: paused creation, activation, an autonomous
turn, pause and clear. It used no account inference. Stop availability was
observed there; actual Stop was exercised by the live delivery tests.

The current portable build and final verification evidence are recorded in the
runtime acceptance audit (historical report removed during workspace cleanup).
The package is a local unsigned development build. Its source snapshot excludes
test Codex homes, credentials, user histories and build caches; the original
working tree remains untouched by release commits.

This closes these specific paths, not every native capability or external
integration. Successful audio transcription, OAuth/OS protected prompts,
physical monitor/DPI coverage and the documented native 0.153.4 limitations are
not newly passed. Temporary test histories are retained pending explicit cleanup
approval. The following sections describe earlier builds and remain historical.

## 2026-09-10: graph, attachments, drafts and native operations

The remembered account also enabled live Windows UI checks of graph forks,
independent continuation and recovery after restart, plus text/image attachments
in both main chat and graph. The graph fork retained ALFA while its source
retained BETA. Main, graph source and graph fork restored separate unsent drafts.
Native compaction completed, and a Goal was created paused, read back and marked
complete with no active Goal left running.

The run fixed graph drafts prepared before assignment, the main composer's
Audio label, shared native-dialog layout, remaining graph typography/contrast,
and errors disappearing during background stream updates. **48 focused tests
passed**, Svelte reported zero errors/warnings, and the final packaged hidden
WebView checks passed in Light/Dark. Browser inspection covered **48 dialog
combinations plus 16 selector/audio combinations**, using production HTML and
bundle with synthetic state in Full HD, 2K, 4K and narrow viewports. Native GUI
evidence is Dark; the browser matrix is not a physical monitor/DPI matrix.

**Review remains blocked before submission:** Codex did not confirm the
requested local read-only review scope. The error now remains visible; no review
result or native review turn is claimed. Audio was tested for correct selection,
labelling and rejection by an incompatible model, not successful transcription.
Goal activation and stopping an active turn were not newly tested in this run.

Evidence and exact scope: live acceptance audit (historical report removed during workspace cleanup).
The latest package is `outputs/supervisor-live-acceptance-preview-2026-09-10`,
identified by its `BUILD_INFO.json`. Ordinary tests no longer require hands-on
user presence when the account remains connected. Protected login/MFA, OAuth,
OS permissions and external-service prerequisites are separate, as are the
previously recorded native 0.153.4 limits. Older pending graph/F1/F2 entries below
are superseded only for the specific paths documented in these live audits.

## 2026-09-10: chat/fork/tree live acceptance completed

With the remembered ChatGPT account connection, Computer Use completed **F1
history cutoff and F2 independent continuation** in the actual Windows app,
without hands-on user input. A source with ALFA then BETA was forked through its
first completed turn. Continuing the fork returned ALFA; continuing the original
returned BETA. A nested fork continued with GAMMA without changing its parent.
Source renaming, immediate-parent navigation, subtree collapse/expand and all
three histories after an app restart were verified. Saved metadata contains
three distinct native IDs and the exact source → fork → nested relationships.

**41 focused tests passed**, together with the packaged hidden-WebView checks
in Light/Dark. The live GUI proof uses Dark; it does not claim another complete
physical DPI/monitor matrix or live graph-card acceptance.

Evidence: chat/fork/tree audit (historical report removed during workspace cleanup),
including eleven screenshots and scoped persisted lineage. The three disposable
`CHAIN_*_2026-09-10` chats remain in the audit workspace for review.

The connected account enables ordinary scoped live conversation tests without
requiring the user's presence. Protected login/MFA, external OAuth consent and
Windows elevation remain separate prerequisites when encountered. Other real
service, destructive-action and graph-card checks are not passed by this result.
Entries below are historical: statements that F2 was pending describe the state
before this live run and are superseded by this section.

## Earlier implementation and validation records

P3-A note (2026-09-09): Git metadata, sections and protected revert were added
and tested in the backend only; no corresponding UI controls were exposed.
There is **no new personal/Computer Use task** for this increment. Do not search
for P3-A controls in the existing P2 preview. Future surfaces need their own
visual/destructive-action acceptance; see [P3-A](CODEX_APP_SERVER_P3A.md).

Status: **all autonomous P0/P1/P2 engineering work is complete; remaining live
checks depend on their account, OS, OAuth and external-service prerequisites**.
They do not all require manual user operation. On 2026-09-08 the
user confirmed account readback and a completed main-chat prompt. The P1 closure
run additionally confirmed the real account/capability states and global MCP
inventory without sending a prompt or calling a resource/tool. Real-account
summary emission, Windows sandbox setup and external-service actions below remain
unverified or opt-in. OS-picker evidence is partially complete as recorded below.
The original checks were written 2026-09-07 against the 23:01:03 unsigned
development preview and the current component labels. This checklist records
remaining evidence for the existing objective; it does not replace automated
tests or declare full parity. Screenshots are not required; the explicitly
labelled post-hardening pass below was completed against the current source build.

The protocol-version check itself is complete: 0.153.4 is the current official
release and the checked-in stable schemas match fresh generation. This checklist
covers only the remaining human-operated evidence. Product/API gaps are tracked
separately in `CODEX_APP_SERVER_GAP_ANALYSIS.md`.

Current local test executable:
`outputs/supervisor-live-acceptance-preview-2026-09-10/Supervisor.exe`, 34,589,184 bytes.
It requires official Codex CLI **0.153.4**. SHA-256:
`5cead80fc316e70a0b468dac698c424a867e0486113ef23575f809a147839fb8`.
Close only instances whose work you have finished before opening the preview.
Do not stop an active conversation just to collect a result.

This preview contains P2, the desktop-runtime discovery fix, the already-tested
P3-A backend, silent draft saving in main/graph composers, the graph launcher
transition fix, the chat-tree increment below, remembered ChatGPT connection and
the ordered Agent configuration menu and the Supervisor product identity.
P3-A still has no new UI controls. Previous previews are
preserved; an already running copy
does not acquire the rebuilt interface until the new executable is launched.

Brand update (2026-09-10): the app is now **Supervisor**. Open Agent configuration
through the Regia mark beside the composer; the same logo opens the Knowledge
Graph. The tetrahedron is removed, not its model/configuration functionality.
The native executable icon and Windows product name also use Supervisor.
Keep the dependency inventory and notices beside the executable. Existing account,
chat/draft stores and internal paths retain their previous identities; a personal
project or folder named Central Agent is not automatically renamed.

Rebrand verification passed: 738 ordinary Rust tests (23 ignored), three scope
probes, 181 frontend tests (one existing skip), Svelte checks/build, 38 scenarios,
locked release and packaged hidden-WebView startup. Browser inspected synthetic
production UI in both themes at Full HD, 2K, 4K and narrow layout, including the
new graph/configuration marks. Metadata and icon extraction from the packaged
executable passed. Evidence: `outputs/supervisor-brand-audit-2026-09-10/AUDIT.md`.
No new live account/OS/external-service acceptance is claimed; F2 remains pending.

Configuration navigation (2026-09-10): the main panel and graph cards now use the
same five sections. Expand the relevant heading before following older checklist
steps; the native actions and confirmations themselves have not changed.

| Section | Existing controls |
| --- | --- |
| Model & response | Provider/model, effort, speed and context; personality in the main panel. Initially open. |
| Permissions & summaries | Access preset and public reasoning-summary preference. |
| Codex conversation | Browse/load/refresh/resume history, rename, fork, review, compact and goal; native settings report. |
| Tools & integrations | Skills, MCP servers, Apps and hooks. |
| Saved defaults | Shared Codex preferences, not overrides for just this chat. |

Archive/Restore, Reset and Delete are under **Codex conversation -> Manage or
remove history**. Errors and selected skills stay visible when their group is
closed. Sections retain their state during background refresh; shortcut dialogs
remain available even when the section is closed. Narrow windows and graph cards
can scroll to lower controls; collapsing the model section makes more room.

This increment passed complete workspace verification (737 ordinary Rust tests,
23 ignored; three scope probes; 181 frontend tests, one existing skip), release
build and packaged hidden-WebView checks. Browser inspected the production UI
with synthetic state in both themes at Full HD, 2K, 4K and 720x900. Evidence:
`outputs/agent-configuration-audit-2026-09-10/AUDIT.md`. No personal app data or
live account was changed; existing manual gates and F2 remain pending as below.

Remembered connection (2026-09-10): when ChatGPT was already connected, reopening
this preview restores it automatically. **Connecting… / Refreshing…** can briefly
appear, followed by **ChatGPT connected**, without another login. This was
verified with Computer Use over two real app processes using a separate Central
Agent profile. No extra manual gate is added. The live account was not logged
out; revoked/expired native credentials still require explicit sign-in. Choosing
Sign out disables reconnection and does not delete chats. An older profile with
no saved Codex chat or model may need Connect once after upgrade. Permission
consent remains session-only. Existing independent-fork F2 is still pending.

The draft-saving label was removed at the user's request, not delayed or hidden
with CSS. Local persistence, diagnostic data attributes and visible error alerts
are unchanged. At the silent-draft increment: 173 frontend tests passed, one existing skip;
Svelte check/build and packaged hidden-WebView startup passed. An isolated
Browser fixture checked pending/acknowledged drafts in Light/Dark at Full HD,
2K and 4K, plus error alerts in both themes. These are component-level evidence,
not new real-account or OS-picker acceptance results. Evidence is retained in
`outputs/silent-drafts-audit-2026-09-09/`.

Chat-tree clarity (2026-09-10): local project chats now show **Original**,
**Fork** and **Fork unconfirmed** relationships, source titles and direct fork
counts. Forks are nested under their source; nested forks gain another level.
The chevron expands/collapses descendants without selecting or modifying a chat.
The composer identifies the current chat and offers **Open source chat** when
its source is available. Missing/archived/filtered/cross-project sources never
hide a fork; pinned shortcuts stay flat and include relationship text. User
names are not rewritten. A fork copies no project files or working tree.

The new runtime can read previous binding stores. Older forks may initially
remain flat: select the fork and use **Agent configuration -> Codex conversation
-> Refresh history** once, so a valid native `forkedFromId` can be saved. No
automatic history scan or guessed title-based relationship is used. If the
source is not linked locally, the UI says so rather than inventing an original.
**Upgrade note:** once nonempty ancestry is saved, older previews that reject
unknown store fields cannot read that updated binding file. Do not run an older
preview concurrently on, or downgrade it onto, the updated profile. No personal
store was migrated or app instance closed as part of this implementation.

At the chat-tree increment: 724 ordinary Rust tests passed (23 ignored), 180 frontend
tests passed (one existing skip), Svelte check/build, formatting, locked release
build and packaged hidden-WebView startup passed. Browser inspected the full
production host with synthetic chats in Light/Dark at Full HD, 2K and 4K, plus
narrow long names, collapse/refresh, search, archived source and pinned duplicate
titles. Source navigation and return preserved the draft. Evidence is in
`outputs/chat-tree-audit-2026-09-10/`. This is not new live-account/F2 evidence:
the earlier real F1 result below is preserved and **F2 remains pending**.

Graph launcher fix (2026-09-10): opening Settings now projects the collapsed
graph state before resizing its native surface. Graph -> Settings -> close no
longer leaves the expanded node-count heading cropped into the launcher. The
Rust transition-order regression failed before the fix and passed afterward;
the actual hidden graph WebView also checks repeated collapse/reopen in both
themes. Workspace tests: 720 passed, 23 opt-in tests ignored; frontend: 173
passed, one existing skip. Format, Svelte check/build and packaged startup passed.
Computer Use verified the real native roundtrip and successful reopening at the
1920x1080 client size in Light/Dark, plus detached Agent/Settings in Dark. An
isolated Browser fixture checked graph/launcher rendering in Light/Dark at
Full HD, 2K and 4K; it is not native multi-resolution window-geometry evidence.
Details and component screenshots: `outputs/graph-launcher-audit-2026-09-10/`.
The personal profile was not used or modified; no prompt, SSH map, Codex login,
permission grant or file attachment was initiated for this fix.

Guided picker checks in chat: the user reported **1A, 1B, 1C and 1D passed**
(main picker cancellation, text selection/removal and image selection).
On 2026-09-10 the user also reported **2A passed**: the graph card's `+` opens
the picker, `audit.txt` appears only in that card, its `AUDIT_GRAFO` draft stays
unchanged and the main chat retains its own contents without receiving the file.
The user then reported **2B passed**: removing `audit.txt` from the graph card
removes only that attachment, preserving `AUDIT_GRAFO` and the main chat's text
and attachments. The user also reported **2C passed**: cancelling the graph-card
file picker closes it without adding an attachment or changing `AUDIT_GRAFO`
or the main chat's text and attachments. The user then reported **2D passed**:
a test PNG/JPEG appears in the graph card's attachments, `AUDIT_GRAFO` remains
unchanged, the main chat receives no new attachments or content changes, and
no agent processing starts. This is user-reported picker evidence, separate
from the automated launcher fix; no message submission or model recognition
is implied. On 2026-09-10 the user chose to suspend the remaining simple picker
checks and audit the advanced features already implemented, not develop new
features. **3A and the remaining audio checks are deferred, not passed.** The
user then requested a direct check of the completed operation. **F1's history
cutoff and source preservation were verified with Computer Use** on the current
preview; see the evidence below. **F2, independent continuation**, is next and
has not been performed in this guided audit.

It is `NotSigned`, built from uncommitted source and non-distributable; use it
only for local checks. Launch it first with `CENTRAL_AGENT_CODEX_BIN` absent: it
searches the bounded official desktop installation after
packaged and PATH candidates. Use the variable on that build only as a strict
diagnostic/pinning override, never as a global PATH or credential workaround.
In the native profile controls, Reasoning summaries starts at Automatic. Retry
a normal prompt and report whether progressive summaries/commentary appear.
Detailed is another native choice, not a promise of private reasoning. No fresh
login, shared-config change or Windows elevation is needed for this check.

### Optional check: native summary emission

On the development PC, launch the verified preview from PowerShell with the
explicit existing CLI below. These assignments affect only this PowerShell
session and its child application, not global Codex configuration or PATH.
Do not close an older instance while it still has active work.

```powershell
Remove-Item Env:CENTRAL_AGENT_CODEX_BIN -ErrorAction SilentlyContinue
& 'C:\Users\<user>\Documents\Central Agent\outputs\agent-configuration-preview-2026-09-10\CentralAgent.exe'
```

In a local project chat, choose **Reasoning summaries: Automatic** or
**Detailed**, then submit a read-only request, for example:

> Describe the structure of this project. Do not modify any file.

Report whether progress text appeared **before** the final answer, and whether
it was commentary, a reasoning summary, or only command activity. Do not send
private project content. Missing summaries alone do not prove a client defect:
the actual model may not emit them. If absent, the next diagnostic is a scoped
read of that application's bound native history, not repeating unrelated tests
or inventing progress text. This check does not authorize setup, logout or OAuth.

Completed on the current 2026-09-09 source build: the P2 section follows the
active project without **Synchronize project**; long Apps/MCP/feature lists
filter without closing Settings; and `/goal`, `/fork` and `/review` dialogs are
visible and return to their original host on Cancel/Escape. Light and dark theme
states were inspected. A 139-tool MCP inventory remained usable and its longest
direct-call heading wrapped without clipping or horizontal scroll. The Apps
service returned HTTP 403, so selection was not exercised. A final rebuilt-source
retest verified the explicit account/workspace-unavailable state in both themes,
with no upstream HTML, stale refresh copy, misleading Resume instruction or false
empty-inventory claim. The
Connect -> Load history -> Resume path completed; reconnect immediately cleared
the prior local connection-required alert. No command, import, reset, email,
feedback, OAuth, direct tool call, destructive lifecycle action or Full access
grant was executed merely to exercise presentation.

## 1. Connection and current account — completed read-only

The 2026-09-09 autonomous visual pass connected to the existing session, showed
runtime 0.153.4 and refreshed bounded account/capability state without changing
authentication. Repeat the steps below only after a real login/account-state
change; they are not an outstanding gate by themselves.

1. Open Settings using the gear, then **AI**, then the **Codex** card.
2. Choose **Connect App Server** if disconnected. Wait for the reported runtime
   version and model list; use **Refresh account** for an existing connection.
3. Report the status label, runtime version and any error text. A status of
   **ChatGPT connected** verifies account readback, not a fresh login ceremony.

Do not send an email address, device code, authorization URL query, password,
token, cookie, native credential file or personal conversation transcript.
Do not sign out of a working account just to pass this check: Codex CLI may
share that authentication. Logout testing needs a separate explicit decision.

## 2. Native Windows sandbox setup — explicit user decision

This check changes Windows sandbox configuration and may show an administrator
prompt. It must not run automatically or be silently substituted with Full access.
Complete active Codex work first. If you do not want system setup, leave this
check unverified and report that choice.

1. In the same Codex card, expand **Windows sandbox**.
2. Choose **Set up Windows sandbox…**. Review the confirmation. **Cancel** must
   close it without starting setup; cancellation does not prove setup succeeds.
3. Only if you want to proceed, open it again and choose **Start setup**.
   Respond to any Windows administrator prompt yourself.
4. Record the final text. The success message is
   **Native Windows sandbox setup completed.** Waiting or a successful RPC
   acknowledgement alone is not completion. If an error appears, report it
   rather than repeatedly retrying, choosing Full access, or disabling security.

**Legacy setup without elevation…** is a separate native mode with a separate
confirmation, not an automatic fallback. Do not execute both modes for this test.
Successful setup alone does not prove every filesystem/network sandbox boundary
or grant broader permissions to any conversation.

## 3. Actual OS file selection — no prompt needed initially

User-reported progress: main checks 1A–1D and graph checks 2A–2D passed: text
selection/isolation, attachment removal, picker cancellation and image selection.
Do not repeat those just to collect more evidence. Audio selection (check 3A,
main chat), graph audio isolation and model-modality submission behavior are
still unverified personally and deferred at the user's request on 2026-09-10.

Use only newly created disposable files, never credentials or a personal document.
Create a small UTF-8 `.txt` file containing `CENTRAL_AGENT_PICKER_CHECK`, an
ordinary harmless PNG/JPEG and, for the current preview, a short harmless MP3/WAV
using your usual tools.

1. In a main chat with a local project and Codex selected, use **+**, select the
   test text/image files in the Windows picker, and check that their attachment
   chips appear. This step alone must not start an agent.
2. Remove the chips, open the picker again and cancel. Cancellation must not
   attach another file or clear your text draft.
3. Repeat in one local-node Codex graph conversation; the main chat must not
   receive that graph card's attachments.

Frozen post-picker input, start/Queue/Steer and image recognition already have
separate recorded automated/native evidence. This check covers the actual OS
picker handoff, not PDF/Office/binary support or a new upload API. Sending a test
prompt is separate, uses the account quota and requires your explicit submission.

On the verified preview, repeat with MP3/WAV. A model without an explicitly
reported audio modality must reject submission while preserving the draft and
chip. A compatible model may send only after your explicit prompt. Do not use a
renamed non-audio file: signature mismatch must be rejected.

## Remaining optional external environment checks

These checks may access connected services or consume quota. Perform only the
ones needed for normal use and keep all service content disposable/redacted.

1. Refresh **Plugins & commands** in one loaded conversation, then open the plugin
   selector beside Attach. Select one accessible, enabled, installed and callable
   plugin. Confirm the menu row contains only its local icon and name, and the
   selected composer card adds only a dedicated X. Clicking the card does not
   remove it; clicking the X does. No commands, descriptions or `$app-id` appear.
   Command summaries remain in Settings and selection alone must not call a tool.
   Submit only if you intend to use that service and
   verify native approval remains authoritative. The audited account/workspace
   currently receives the explicit unavailable state, so this requires an account
   or workspace to which App Server actually grants catalog access; do not retry
   the same 403 or treat it as a local resume failure.
2. **Performed read-only on the closure build:** Settings reported the connected
   account, six model profiles, provider capability state, two rate-limit buckets
   and zero workspace messages. No account identity or message body is recorded.
3. **Global inventory performed read-only:** four configured servers loaded; one
   reported 138 tools and 40 resources, all with usable opaque handles. In
   **Conversation MCP tools**, read one harmless resource only when needed. Call a
   tool only after reviewing its separate confirmation and only if its possible
   external side effects are acceptable. Never retry automatically after a lost
   connection.
4. If an MCP server naturally requests `openai/form`, test one flat and one nested
   form. Invalid JSON must remain unsent and cancellation must preserve no answer.
5. Refresh **Codex hooks** only in a disposable project that already has a known
   hook. Confirm inventory/activity is read-only and does not expose its command.
6. A detached review can be accepted only on an explicitly reported legacy
   history. Paginated 0.153.4 histories must show the known unavailable state;
   do not force or simulate legacy mode.

## 4. Login and external MCP browser launch — only when needed

If sign-in is actually required, use **Sign in with ChatGPT** or **Use device
code**, then **Open sign-in page** and finish on the official page yourself.
Never copy the temporary code into a report. Verify that the pending instruction
disappears and **ChatGPT connected** appears after native completion. Report
only status/error text. **Cancel sign-in** can be tested on an intentionally
started unfinished attempt; do not disconnect an established account.

An MCP authorization-page test needs an explicitly chosen server/account.
Do not install a new service, grant scopes or start OAuth solely to fill this
checklist. When needed for normal use, open the displayed native authorization
page explicitly and report whether the OS browser opened and native completion
returned to the correct request. Opening a URL is not accepting an elicitation.

## 5. Checks that must remain person-operated

Computer-use can inspect the controls and their confirmation states, but it must
not complete authentication/security ceremonies, execute terminal commands or
silently authorize account/external side effects. Validate these personally only
when you actually need the workflow:

- ChatGPT browser/device-code sign-in, sign-out and recovery of expired login;
- Apps or MCP OAuth scopes, consent, revoke/expiry and an entitled Apps catalog,
  followed by a deliberate `$app-id` invocation;
- Windows sandbox setup, UAC/elevation, legacy setup, denial/reboot behavior and
  real filesystem/network isolation boundaries;
- App Server sandbox-command execution plus stdin, resize and terminate states;
- the Windows file picker and actual text/image/audio attachment handoff;
- process-wide feature mutation and restart behavior, external-agent import,
  reset-credit redemption, workspace email and feedback upload;
- real allow-once/session/decline/cancel approval paths, side-effecting MCP tools,
  live flat/nested elicitation forms, Goal/Review/Compact model work and permanent
  native conversation deletion.

Use disposable data, review every native confirmation and record the final native
state. A visible confirmation or successful RPC acknowledgement is not proof that
the external, account or operating-system effect completed.

## Advanced guided audit — existing implementations

Requested on 2026-09-10. These are additional personal end-to-end checks, not
new implementation work or replacements for previous automated evidence. Do
not enable experimental APIs or expose P3-A backend controls for this audit.

### F1. Native fork through a selected completed turn — cutoff verified visually

Post-operation evidence (2026-09-10): at the user's request, Computer Use inspected
the already-running `codex-graph-launcher-preview-2026-09-10` executable in its
current dark-theme layout. The user-created `AUDIT_FORK` contains the ALFA prompt
and response only, with no BETA exchange. **Refresh history** on that branch
preserved this cutoff. Returning to the original `New chat` showed both ALFA
and BETA prompts/responses intact, confirmed by screenshots and accessibility
text. The original chat was left selected, with configuration closed.

The assistant did not create another fork, send a prompt, change permission
settings, or restart the app. This verifies the final history boundary and source
preservation, not the original creation-time sequence, a filesystem/Git snapshot,
restart persistence, or independent model continuation. The latter remains F2.

Use a new disposable local project chat in Central Agent with Codex selected,
no attachments and no active work. The two short prompts below use the normal
account allowance; no tools, file changes or elevated permissions are needed.

1. Send: `Non usare strumenti e non modificare file. Il codice corrente e ALFA.
   Rispondi soltanto ALFA.` Wait for the completed response `ALFA`.
2. Send: `Non usare strumenti e non modificare file. Da ora il codice corrente
   e BETA. Rispondi soltanto BETA.` Wait for the completed response `BETA`.
3. Open **Agent configuration > Codex conversation**. Choose **Load history**
   or **Refresh history**, wait for completion, then choose
   **Fork into a new conversation...**.
4. Set **New conversation name** to `AUDIT_FORK`. In **History boundary**, select
   **Through turn 1 · completed**, not **Entire observed history**. Choose
   **Create fork**, then **Open branch** when the new local conversation is reported.
5. Inspect the fork and return to the original chat without sending another prompt.

Expected: the fork contains the first exchange (`ALFA`) but not the later `BETA`
exchange; the original retains both exchanges. Creating/opening the fork alone
must not start a model turn, change project files, or grant write/full access.
This is a native conversation fork, not a Git branch or a separate worktree;
both conversations still refer to the same project directory. If the first-turn
choice is missing, an error appears, or the result is uncertain, stop and report
the visible state. Do not retry an uncertain fork automatically.

This checks the inclusive cutoff documented for
[`thread/fork` and `lastTurnId`](https://learn.chatgpt.com/docs/app-server), using
the controls shipped by this project. **F2, independent continuation in both
conversations**, follows only after the F1 result; persistence/recovery and
other advanced paths remain separate checks. F1 is not marked passed merely
because the confirmation dialog opens.

## Evidence and boundaries

The autonomous closure has already covered the full compile/test/lint/audit
gate, all 19 opt-in probes, main/graph no-inference hosts, four physical
wire-loss orders, settings reload characterization, deterministic terminal P2
rendering, optimized build, hidden UI startup, hashes and manifest. Do not repeat
those merely to add manual evidence.

For each attempted check, report: build time, main/graph/Settings location,
action taken, final status and redacted error if any. Leave untouched checks
**not performed**. Do not infer success from the absence of an error.

No user action above changes the distinct 0.153.4 runtime boundaries recorded in
`CODEX_APP_SERVER_ACCEPTANCE.md`: named-profile selection is experimental; the
isolated network-policy and review probes reached no approval callback; the MCP
progress route emitted no progress callbacks; and an MCP reload acknowledgement
did not refresh tools in the tested already-loaded thread. Those observed limits
are not reasons to emulate Codex locally, fabricate events or broaden access.

Official references:
[App Server](https://learn.chatgpt.com/docs/app-server) and
[permission profiles](https://learn.chatgpt.com/docs/permissions).
