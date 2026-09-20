# Supervisor design contract

This document teaches contributors and coding agents how to make interface
decisions for Supervisor. It defines judgment and product character.
Mechanical values live in `assets/themes.css`; reusable presentation belongs in
Svelte components under `ui/src/components`.

## Provider reset boundary

The current Codex phase is native App Server only (user decision 2026-09-06).
Render native conversation, tool, approval and diff events; keep Supervisor
Time Machine, SSH and desktop tools outside the native conversation runtime.
The verified first-party Browser plugin is the narrow browser exception: its
official service talks through Supervisor's local `iab` bridge to the existing
WebView2 tabs. Those existing capabilities remain intact for other providers.
Do not describe native conversation rewind as file restore or create a second
agent engine.

The previous Codex integration has been removed, not hidden behind a flag.
Codex is selectable with native text conversations on local directories, using
an initially read-only sandbox and an explicit saved access choice shared by all
Supervisor Codex agents. The
native binding store, streamed timeline and owner-scoped decision/recovery cards
are connected. Native steering, usage reports, history, precise forks, review,
goals, skills and extra roots, Apps, hook inspection, MCP management/direct
operations, bounded extended forms, account information, audio input,
personality and public reasoning-summary presentation are connected. Do not
restore those components from old plans. The official App
Server client lives in `central-agent-codex-runtime` and targets the generated
stable Codex CLI 0.155.1 contract with experimental APIs disabled. The exact
supported runtimes are listed in `protocol/app-server/supported-versions.json`,
  including the verified desktop builds 0.154.0-alpha.6.2,
  0.155.0-alpha.2.6 and 0.155.1. Runtime discovery and
contract checks use that same list; unverified updates remain unsupported.
First-class coverage of the remaining public surface and all
live Windows/account/approval/visual acceptance gates are still in progress.

Preserve the existing interface, editor, browser tabs, graph, native sessions,
Time Machine and the five remaining provider adapters. Shared catalog and input
types live in src/provider_types.rs, with no provider runtime ownership.
Capabilities removed with Codex must be unavailable, never silently substituted
with another provider or simulated locally. Native provider credentials and
account-wide conversation stores are outside the local chat-reset scope.

## Baseline status

Last synchronized with the implemented product on **2026-09-08**. This section
is the baseline for future feature work, not a speculative roadmap. When a new
feature changes a primary object, durable interaction, or material state,
update this document and `ui/evals/scenarios.json` in the same change.

Supervisor is currently a Windows-first, local-first desktop application:

- Rust owns `winit`, Wry, WebView2 instances, native windows, persistence,
  providers, filesystem and process access, permissions, and validation.
- Svelte 5 and TypeScript own new reusable presentation and interaction
  structure. The static bundle is embedded in the executable; Node.js is not a
  runtime dependency.
  Trusted hosts load that bundle through a parser-blocking script from the
  fixed `central-agent-ui` resource protocol, before their legacy bridges.
  Do not inline the bundle into HTML: WebView2's `NavigateToString` has a 2 MB
  limit. The protocol serves only the compiled script, without a network port,
  CDN, arbitrary file access, or changes to page origins and IPC authority.
- Internal request correlation uses the shared getRandomValues-based UUID helper.
  NavigateToString is not a secure web context: do not require randomUUID or
  change the trusted origin just to generate IDs. IDs never confer authority.
- Existing `assets/*.html` files are trusted legacy hosts and compatibility
  bridges. They may coordinate stable DOM hooks and Rust IPC while their
  structure is migrated, but they are not the default home for new UI.
- Browser tabs remain independent native WebViews. Terminal, remote desktop,
  detached tabs, and detached agent surfaces remain native capabilities rather
  than simulated web widgets.
- `central` and `central_dark` are the only product appearance modes. They are
  one identity expressed in Light and Dark, not the beginning of a theme
  gallery.
- Full HD, 2K, and 4K are supported layout profiles. The initial window is
  centered when the display can contain it and maximized when it cannot.
- The product is Supervisor (brand update authorized 2026-09-10). Its monochrome
  Regia mark uses two interlocking angular forms and an open verification gap.
  It is shared by the native application icon, graph launcher,
  configuration launcher and working-project indicator. The canonical geometry
  is `assets/supervisor-mark.svg`; `SupervisorLogo.svelte` owns reusable display.
  Native raster/icon assets are generated from that SVG and shared theme tokens,
  with `assets/supervisor-mark-native-16.svg` providing the optical 16 px master.
  Icons and logo controls have transparent backgrounds and no decorative tile
  or border. Native window icons switch ink with the current Light/Dark theme.
  The start-page heading embeds `assets/supervisor-wordmark.svg`, the approved
  outlined lettering, at the existing heading height. Keep the separate native
  graph launcher in its reserved slot; do not duplicate it inside the wordmark.
  On Home, that launcher may stack offset copies of the same monochrome geometry
  into a restrained directional extrusion. It still has one canonical front face,
  one action, a transparent background and no decorative tile or border.
  The separate native graph launcher is visible only over a fully loaded Home
  page in the main browser pane. Hide it during navigation and over websites,
  remote desktops, hidden browser content and Settings covering the main window.
  Refresh this native visibility on page-load events as well as explicit
  navigation and tab/layout changes. An explicitly expanded graph remains
  independent of browser navigation. Anchor the collapsed launcher inside its
  Home pane when browser tabs or the terminal share the window.
  Keep storage directories, native client IDs, IPC and legacy DOM hooks stable;
  a brand change must not migrate, orphan or relabel personal data.

### Current surface map

| Surface | Primary object | Current contract |
| --- | --- | --- |
| Application shell | Native work surface | Real browser tabs contained by the visible browser pane, tab-flow `+`, fixed Settings entry, resizable sectors, continuity across docked or detached windows, and a native Windows notification-area lifecycle. |
| Browser | Live web page | Back, Forward, Reload, Home, address/search, independent WebViews, and direct tab drag into Agent as bounded visual context. |
| Agent workspace | Project conversation | Projects and chats provide orientation; conversation, streamed work, artifacts, and composer form one vertically anchored workspace. |
| Projects and Files | Local coding scope | Several projects and chat groups may stay expanded; one project is active for Explorer and future runs; file operations remain project-relative. |
| Composer | Next user instruction | Auto-growing text input, file/context attachment, four-part agent configuration, Stop, and active-request delivery choices. No repeated chat-title/Chat banner or context report beside the prompt. |
| Settings | Product configuration | A full application surface with search and one visible category at a time. Agent behavior and permissions remain separate from the selected conversation's Codex Skills and Apps. |
| Agent Graph | Project and agent topology | Four-column project board: explicit projects, project conversations and forks, supervisors, and project files. It never maps the underlying local or SSH filesystem. The board begins at the top of the window. A bare back arrow occupies a transparent 40px overlay flush with the upper-left edge, and every lane heading aligns with the icon's upper edge. The arrow remains transparent on hover while keyboard focus stays visible. Terminal, browser controls and redundant project/supervisor counts are absent. |
| Graph agent window | One node-bound conversation | Select an agent to open its preserved card in the supervisors column; each agent keeps its history, four profile choices, approvals, artifacts and checkpoint, without a context report or tab/shell attachment menu. Project-chat and Supervisor prompt fields are borderless and retain a visible focus state; their Send, Stop and Resume control occupies the bottom-right corner with the same inset from the right and bottom edges, matching the main agent composer. |
| Terminal | Interactive shell session | Multiple direct ConPTY or SSH sessions, reorder, dock/detach, and bounded chat attachment with optional Follow live. |
| Remote desktop | Remote Linux desktop | SSH-tunneled RDP or VNC, direct user input, file transfer, explicit Give agent control, and an immediately available stop path. |
| Time Machine | Run-scoped code changes | Pre-change checkpoint, changed-file tree, language icons, aggregate diff statistics, unified diff, and full or single-file restore. |

### Background lifecycle

Closing the main Supervisor window hides every Supervisor work window in the
Windows notification area. The same process, WebViews, conversations and agent
runs remain alive; reopening the tray icon restores that live state without a
reload or a second submission. The tray uses the standalone Supervisor mark and
offers only `Open Supervisor` and `Close Supervisor` from its right-click menu.

Supervisor owns one Windows application instance. Launching it again restores
and focuses the existing process instead of starting another archive owner; the
second process exits before opening application data or starting providers.

`Close Supervisor` is an operational Stop before it is an application
exit. It clears queued submissions, routes the native interrupt to every active
Codex owner, stops active main and graph runs for other providers, and waits for
their cancellation and any Time Machine finalization. A bounded fallback then
terminates only Supervisor-owned work before the icon and process disappear.
The normal window close must never cancel or pause an agent run.

## Product character

Supervisor is a local control room for browsing, coding, system work, remote
machines, and long-running agents. It should feel deliberate, calm, technical,
and trustworthy. It must not resemble a generic chat page, a dashboard kit, or
an assortment of independently styled tools.

The visual signature is a monochrome working field: white, black, and neutral
grays; strong alignment; recognizable object shapes; and a small number of
purposeful transitions. Hierarchy comes from contrast, scale, spacing, and
weight rather than hue. Controlled color is limited to canonical file or
technology identity, diff additions and removals, and unfiltered user content
such as web pages, images, video, or a remote desktop. Light and Dark are two
expressions of the same product language, not separate themes.

## Protect this priority order

When requirements compete, preserve them in this order:

1. User data, local-system safety, privacy, and an honest representation of
   what the agent or computer is doing.
2. Existing Rust/Wry ownership, WebView boundaries, native browser behavior,
   accessibility, and stable IPC or DOM contracts.
3. The user's immediate task and the action or evidence needed to complete it.
4. Readable hierarchy, predictable state, and continuity across surfaces.
5. Supervisor identity and visual refinement.
6. Decorative detail.

Never hide uncertainty, failure, approval requirements, or the target of a
system action to make the interface appear cleaner.

## Frame the operator's job

Before composing a surface, determine privately:

- What is the operator trying to inspect, decide, run, or recover?
- Which object is primary: conversation, web content, files, graph, shell, diff,
  remote desktop, or settings?
- What state changes the meaning of the screen?
- What needs immediate attention and what can remain available on demand?
- Which action is destructive, privileged, remote, or difficult to reverse?

Order information by the operator's task, not by the order in which the runtime
returned it. Show one clear primary object per working region. Supporting
metadata must remain nearby without competing with it.

## Compose before decorating

Choose layout and ownership before styling controls.

- The browser content, Agent workspace, Agent Graph, shell, settings, and
  remote desktop are first-class work surfaces.
- A full-screen surface fills the application content region; it is not a large
  modal pretending to be a page.
- Resizable boundaries must remain obvious through geometry and cursor behavior,
  not visible separator rules.
- Board column resizers keep the full space between lanes as their hit target,
  while rendering only a short, centered pill. Hover and keyboard focus affect
  the pill instead of filling the space between sections.
- Browser chrome, the home field, Agent, Settings, Explorer, and Agent Graph
  share the exact application background. Separate adjacent regions with space,
  alignment, and content hierarchy rather than hairline rules or alternating
  structural fills.
- Detached tabs, shells, and agent panels retain the same hierarchy and state as
  their docked form.
- Keep the composer attached to its conversation and keep its message history as
  the independently scrollable region.
- The project board replaces the free canvas. Four ordered columns communicate
  project → conversation → supervisor → project files. Selection scopes each
  column; real chat ancestry and explicit associations express relationships.
  Keep a compact top-left back arrow and let the four lanes use the remaining
  height without a repeated project heading or live counts. Use one shared
  spacing token for every inter-lane gap and for all four outer board insets;
  compact layouts may reduce that token only for the whole grid. Reserve the
  board scrollbar gutter on both inline edges so the first and last lanes keep
  equal visible distance from the window boundaries. The left inset of the
  Projects heading and the right edge of the project-file cards share one
  content-inset token and must remain optically aligned.
- Every visible scrollbar uses the active Supervisor palette for its track,
  thumb, hover state and directional buttons. Keep intentionally hidden tab
  strip scrollbars hidden; visible scroll regions must not fall back to a
  browser-specific light or dark scrollbar.
- In Project chat and Supervisor conversation cards, the scrollbar track merges
  with the chat surface and the draggable thumb uses the same fill as the prompt
  field. Apply the same pairing to the card stack when several chats are open.
- Browser Back, Forward, Reload, and Home use heavier, consistent line icons
  without persistent circles or outlined containers; a soft rounded hover state
  is sufficient. The address field starts directly with editable text, without
  a decorative or security-status dot.
- The start-page search field submits with Enter and has no trailing submit
  button; the field itself is the complete interaction. Its focused state uses
  one outer focus ring; the idle hairline becomes transparent while focused so
  a second, thinner inner outline never appears.
- On Home, the vertical gap from the logo slot to the Supervisor wordmark equals
  the gap from the wordmark to its tagline. The Agent toolbar action is
  borderless in every state and shares the address field's exact outer height
  at every supported toolbar scale.
- Folder icons share one quiet monochrome outline across Explorer, Time Machine,
  and graph nodes. Do not add internal labels, fills, badges, or folder-specific
  decorative marks.
- Dense technical records may use disclosure, filtering, local scrolling, and
  progressive expansion. Do not remove detail merely to create empty space.

At 1920x1080, 2560x1440, and 3840x2160 the hierarchy must remain equivalent.
Scaling may increase breathing room and information density, but must not turn
the application into a differently structured product.

## Implemented interaction contracts

These contracts describe behavior that future features must compose with
rather than replace locally.

### Projects, chats, and files

- A project is a persistent connection to a local folder, not a copy of that
  folder. Ejecting a project never deletes its files.
- Every project owns persistent chats that can be created, selected, renamed,
  pinned, archived, restored, moved to another connected project, or deleted.
  Moving a chat preserves its complete conversation and rebinds future runs and
  Explorer to the destination project.
- Several project chat groups may remain expanded at once. Expansion is
  navigation state and must not silently change the active project.
- Project chat navigation groups confirmed Codex forks below their source chat,
  with an explicit Original/Fork relationship, direct fork count and expandable
  descendants. Preserve user titles, stable chat IDs, lifecycle menus, pinning,
  drafts, and independent run ownership. A fork of a fork remains a branch, not
  a second original. Hierarchy is metadata, never another runtime or Git worktree.
  Native child/source IDs are retained separately from uncertain-fork receipts;
  existing older histories acquire this metadata on explicit history read/resume.
  Never infer ancestry from names, timestamps or shared project directories.
  Missing, archived, filtered or moved sources keep the fork visible with its
  source label; no child is hidden or reparented to an unrelated chat. Unknown
  ancestry stays an ordinary chat, and pending forks are explicitly unconfirmed.
  The active Projects row identifies the destination; the composer does not
  repeat a chat title, Chat badge or identity banner. Selecting a source row
  submits no prompt, copies no files and grants no access. Use neutral
  glyphs/text and bounded indentation, not color coding or native IDs as labels.
- The active project alone owns the visible Files tree and the target for new
  runs. Project names are more prominent than chat titles; chats align with the
  project content column without receiving a competing selected-card fill.
- Project-file cards and the `Find a file` field share the same visible inline
  edges. Place the file scrollbar in the lane's reserved outer inset so its
  gutter never shortens the cards or shifts their content.
- Explorer expands progressively. New file, new folder, import, refresh, and
  folder selection stay near the Files heading. Creation uses an explicit
  target, supports Enter and Escape, rejects collisions, and never writes
  outside the project root.
- Removing or archiving a project includes its Supervisor chats. If an agent
  is active in that project, the confirmation must either cancel removal or
  stop the overlapping runs and finish their Time Machine checkpoints before
  the project disappears.

### Conversation, reasoning, and artifacts

- Preserve the shared chat/timeline rendering, Markdown, code blocks, links,
  attachments, activity disclosures and unified diff viewer. Provider adapters
  supply their own streaming events; the UI must not invent reasoning.
- User rows do not repeat a You label. Assistant rows do not repeat a provider
  label. Progress, tool activity and the final answer remain visually distinct.
  In main and graph chats, user prompts occupy compact neutral rounded bubbles
  aligned to the right, capped at 80% of the transcript's content width. Agent
  prose starts at the left edge on the chat background. Quoted user answers
  use the same bubble treatment, including inside an expanded work disclosure.
  Long text and attachments wrap within the bubble. Alignment, shape and spacing
  distinguish the authors in both themes, during streaming and after reloading.
  Timing updates preserve each row's user, assistant or system presentation.
  Inline Review projects one instruction and one final result when native
  supervisor/worker items echo them. Use native item identity and review scope
  to identify these copies; preserve raw history and unrelated repeated prompts.
  Graph prose and Markdown emphasis inherit the message foreground, including
  in Light; legacy terminal colors must not reduce conversation contrast.
- Completed work collapses into a duration disclosure while the final answer
  remains visible. Preserve expanded activity rows and manual scroll position
  while streaming; new content must not force the user back to the bottom.
- During a turn, show chronological commentary as ordinary readable text,
  alternating with compact, initially closed action groups. Their summaries
  describe the reported actions (for example, "Ha modificato file e ha eseguito
  comandi") and use `>` closed / a downward triangle open. A single command
  reports its actual running, completed, denied, stopped or failed state.
  Native context compaction is an inline event at its recorded position:
  "Ottimizzazione della conversazione…" becomes "Conversazione ottimizzata"
  only on completion. It neither disappears into a generic tool group nor
  becomes a final answer. Keep the work duration header hidden until the turn
  ends; streaming preserves open actions, commentary identity and keyboard focus.
- The shared work disclosure reads "Durata lavoro: 27m 29s", with `>` when
  closed and a downward triangle when open. Use the recorded native duration;
  missing timing remains explicit. Summary-only imported turns still offer the
  disclosure and fetch only that turn's full native history when first opened.
  Keep loading, retry and empty states inline. Reading details never resumes a
  session, sends a prompt or changes the draft, profile or permissions.
  Preserve chronological narrative checkpoints between adjacent action groups;
  include actual file changes and named tools in their descriptions. Full
  details already loaded remain available through later lightweight reloads.
- Group native work by its authoritative thread/turn identity, including
  in-turn user input, rather than starting another duration section after each
  user message. A local graph run ID cannot terminate an active native turn.
  Display elapsed whole seconds without rounding up. Public reasoning summaries
  remain accessible inside activity disclosures and do not fragment the visible
  commentary/action outline. Reasoning-only intervals join the next action
  disclosure (or the last one at the end); preserve their source order there.
  If a turn has only summaries, provide a separate compact reasoning disclosure.
  Web searches and page reads have explicit action descriptions.
- Recognize the official desktop's persisted question-reply envelope only when
  valid bounded fields refer to an existing question in the same native turn.
  Show its quoted question and answer in a user bubble at the original position,
  without raw XML/JSON or renewed controls. An answered question becomes an
  intermediate checkpoint, even if its stored source phase was final_answer.
  Malformed or unrelated text remains ordinary user content. This is display
  projection only: no history rewriting, approval, tool execution or new prompt.
- The transcript aligns with the composer edges; user bubbles share its right
  edge while assistant content uses the full available width. Prompt growth
  pushes the transcript upwards instead of covering it. Chat zoom affects code
  and prose together.
- Chat identities and run ownership remain independent of the selected project
  or visible conversation. Background work cannot enter another chat.
- Trusted host state renderers are installed during initial script evaluation,
  before any picker, click or focus event. A new window must receive its first
  conversation state without requiring an incidental UI interaction.
- Native Codex messages retain thread/turn/item identity in the shared timeline.
  Reasoning summaries use an explicit, owner-scoped native summary selection
  beside permissions: Automatic initially, Concise, Detailed, Off, or Inherit
  native setting (omit the override, retaining a loaded session's setting).
  Freeze it with accepted/queued input; Send now
  does not change an active turn's settings. Reset to Automatic on restart.
  Never write shared configuration to enable summaries. Empty native reasoning
  items produce no blank Reasoning row; later public summary deltas reveal the
  same identity. Model availability is not a guarantee of summary emission.
  Show only actual exposed summaries, command output and native diffs; no custom
  checkpoint or graph-context injection. Native duration is shown only when
  reported. Other-provider messages remain separate from the native model input.
- File-change items are proposed changes until the native item reports completed.
  Native patchUpdated snapshots replace the same item's displayed paths and diff
  while work is active; they neither apply files nor establish a successful write.
  Completion remains authoritative and late updates cannot overwrite its outcome.
  Preserve their diff on rejection or cancellation without claiming a file write.
  A terminal turn cannot leave an unfinished activity labelled running; project
  it as stopped or failed while retaining explicit native item outcomes and IDs.
- Native web and collaboration activities expose query/page/pattern, addressed
  threads, requested child profile and reported child responses in the existing
  text disclosure. Completing a collaboration call does not imply its children
  finished. Native completion replaces the same item, not a second log row.
  Tool-output text remains plain text; encrypted blocks, opaque search payloads,
  hook instructions and media bytes/URLs never become raw detail or automatic
  fetches. Unavailable media previews are explicit. Historical dynamic-tool
  display does not enable experimental tool execution.
- Native MCP progress messages appear as plain text inside the same tool activity
  in main and graph. Keep their order and row identity without treating a message
  as completion or permission. Completed items reject late progress. These are
  session-only observations; loading native history never invents missing progress.
- Completed native imageGeneration results have a separate, stable media row in
  main and graph, outside the collapsible work disclosure and distinct from the
  final answer. The shared NativeMedia component shows embedded PNG/JPEG output
  and an explicitly opened, keyboard-closeable enlargement. A stream refresh
  preserves the same image and open dialog. Keep source colors and neutral,
  borderless controls; no automatic download, local-path read or URL fetch.
  Rust validates encoded size, raster format and dimensions; the frontend checks
  its descriptor again. Missing, unsupported or oversized results and browser
  decode failures show a fallback without changing native history or claiming
  generation failed. Actual native failures remain in the activity disclosure.
- Native model-routing, account-verification, service-buffering, immediate turn
  errors, runtime/guardian warnings, strict/automatic approval-review state and
  opaque moderation presence are service notices, not final answers or tool work.
  Main and graph keep them
  visible outside the completed work disclosure, with stable identity and normal
  manual scroll behavior. Show the reported model change without changing the
  selected profile; a suggested faster model is informational, never automatic.
  Buffering is shown only when the native showBufferingUi flag requests it and
  is no longer labelled ongoing after the reported turn ends. Disconnect/unload
  marks previous notices stale; history reads cannot make them current again.
  These session-only notices do not create a turn, grant access, retry a request
  or invent verification links. Service text is escaped, not active Markdown.
  Automatic-review action bodies/rationales and moderation metadata never cross
  the Rust presentation boundary; only bounded documented state is retained.
- Native terminal-interaction notifications update the existing command activity
  with a byte count only. Never retain or render stdin or the native process ID;
  completed command items reject late interaction updates.
- Stop during native thread opening cancels the unsent prompt; after submission
  it requests native interruption. Keep the original draft until native acceptance.
  On uncertain delivery offer Check native history, then explicit dismissal of
  the exact warning. Neither action resends input. Loading saved native history
  must be available without starting a model turn.
- Reuse a loaded idle native session for the next actual prompt, including after
  cancelling before the first turn. Reapply the frozen directory/profile/access
  through native turn/start; do not resume unnecessarily or send a dummy prompt.
  After disconnect, Reset unused connection is an explicitly confirmed local
  unlink only for known never-submitted bindings. Existing, imported, attempted,
  archived or deleted histories are ineligible. Preserve drafts, attachments and
  other-provider messages; no native history or project file is deleted.
  On connect and explicit refresh, reconcile all paginated thread/loaded/list
  pages atomically against existing bindings only. Removing a local chat, graph
  card or workspace unlinks its binding and sends thread/unsubscribe only for a
  connection-loaded native thread. Preflight active or uncertain native work
  before clearing its local draft. Unsubscribe never deletes Codex history.

### Composer and concurrent prompts

- Keep the borderless, autosizing composer, readable text, + affordance and
  Supervisor-logo configuration launcher. Provider/model, effort and speed controls
  appear in a compact popup anchored to that logo, with full-width provider/model
  rows and paired effort/speed controls. Native Codex exposes a separate borderless
  plugin control beside Attach. In its normal state, the bounded popup lists only
  each available plugin's local icon and name. Do not show descriptions, command
  suggestions, tool counts, protocol IDs or availability prose there. A selected
  plugin remains visible beside the composer as the same icon-and-name card with
  a dedicated X until it is removed or its matching prompt is accepted. Clicking
  the card or its selected menu row never removes it. The control never enters
  the four-choice profile popup. It is usable before the first prompt: opening it
  reads the installed inventory for the card's captured local directory without
  creating a native thread or sending a model turn. A draft selection remains
  owner-scoped and moves to the exact native thread created by that submission.
  The control appears with the card's first conversation snapshot; it never
  depends on opening the file attachment picker or another composer control.
  When Codex exposes a verified first-party Browser runtime, add `Browser` to
  this same identity list with its local browser mark. Its one-shot selection
  attaches the exact official `browser:control-in-app-browser` skill to the
  captured prompt; it does not synthesize a marketplace entry or create a
  second browser-tool loop. The same X, owner, draft-promotion, acceptance and
  stale-state rules apply in main chat and graph cards.
- Main chat, forks and graph cards share one compact line-delta status in the
  composer actions: `(+N, −M)`. It resets for the exact new owner/turn, updates
  in place while code changes, can decrease when work is reverted, and retains
  the last non-empty completed total without moving the prompt or controls.
  Plus/minus signs carry meaning independently of addition/deletion color;
  assistive text names added and removed lines and the affected file count.
- Enter submits and Shift+Enter adds a line. Keep a draft while work is active;
  on a second submission offer Queue outside the composer. Native Codex also
  offers Send now against the turn visible when the choices opened. A late click
  cannot target a successor turn, change model/directory/permissions, silently
  create a new turn or fall back to queueing. Other adapters keep Queue only.
- Steering clears only its matching owner's unchanged draft after acknowledgement.
  Rejection retains the draft; disconnect or a mismatched acknowledgement requires
  native history review, never replay. Pending steering does not appear as an
  uncertain-delivery warning until its actual response is lost or invalid.
- Native Codex has an owner-scoped slash suggestion menu above its main/graph
  input. It filters implemented UI shortcuts only: resume, skills, config,
  history, goal, compact, rename, fork, review, archive and unarchive. Arrow keys navigate,
  Tab completes, Enter opens the existing control, Escape hides suggestions,
  and Shift+Enter/IME composition retain normal editing. Use shared neutral
  tokens and a bounded scrolling list without covering the transcript.
- Shortcuts open existing native dialogs and confirmations; they never grant
  permissions or start a model turn by themselves. Main controls reuse the
  mounted Agent settings host for dialogs while keeping its settings hidden.
  Return that same host to Settings after its last modal closes. Clear only the exact
  shortcut after its control acknowledges opening; preserve attachments and
  later draft edits. Unsupported slash commands or inline arguments remain in
  the draft with guidance, rather than becoming a queued/steered model prompt.
  A leading space explicitly sends literal slash text; multiline instructions
  and paths remain normal input. Other providers keep their own existing behavior.
  Native mode/app UI and commands without implemented stable API mappings
  are not claimed available. Do not restore the retired CLI command interpreter.
- Compact context asks the native runtime to condense existing model context.
  Confirm its effect and possible account usage; it is not /clear, file restore
  or a custom summarizer. Require an idle loaded native history. Resume connection
  reopens the same native thread without a prompt after reconnecting. Compaction
  acknowledgement is not completion: show accepted/running/stop-requested states
  until authoritative native events finish. Unknown outcomes retain a durable
  metadata-only receipt and explicit history review, never an automatic retry.
  Preserve drafts, attachments, selected skills and sibling conversations.
- A submission is cleared only after acceptance for its original owner. Failed
  validation keeps the draft. Queued snapshots retain their original destination.
- Once local submission validation succeeds, show the user's prompt and frozen
  attachment previews immediately in its main/graph transcript, with a quiet
  Sending… label while awaiting native acceptance. This session-only projection
  must not wait for thread opening or resuming, and must never become saved native
  history. Reconcile by the native client message ID, never by matching text.
  Acceptance before the native user item keeps the bubble visible; an item before
  acceptance replaces it without duplication. Rejection or cancellation before
  dispatch removes the preview and preserves the draft. Queue admission keeps its
  existing queue presentation until that captured submission starts.
- Unsent composer text is local UI state, not native Codex history. Rust persists
  it per project chat, graph conversation or connected-project draft using atomic
  replacement and revision checks. Recover text after restart without submitting
  a prompt or restoring permissions. Late hydration and save acknowledgements
  cannot replace newer typing. Routine saving stays silent in main and graph
  composers: no saving label, live announcement or reserved layout space. Errors
  remain explicit; conflicts preserve text for copying rather than silently
  overwriting it. Deleting a chat/card or
  ejecting its project clears saved text; archive preserves it. Empty versioned
  records prevent old caches from resurrecting cleared text. Draft files are local
  plaintext, not cloud sync or a credential store. Attachments and approval answers
  are not part of this text store; text enters native input only on submission.
- Native Codex's + selects PNG/JPEG images, MP3/WAV audio, or UTF-8 text/code
  files. Use the
  existing snapshot/preview chips, secret-path checks and explicit removal.
  Text files become native text inputs; images become native image inputs; audio
  becomes a frozen native audio data URL only when the selected model explicitly
  reports audio input. Validate file signatures rather than extensions, keep
  image/audio snapshots at 8 MiB each and the serialized native input aggregate
  at 12 MiB. Never publish local audio paths or retain audio bytes in projected
  history. This
  is not a generic PDF/Office/binary upload API. Show redaction metadata and do
  not claim an image/audio token estimate. Native catalog modality capability is checked
  before starting a turn; the runtime remains authoritative for steering input.
  Queue and Send now retain exact selected snapshots. Rejected/uncertain input
  retains attachments; acceptance removes only that owner's exact captured IDs.
  Native history previews never fetch remote URLs or read localImage paths;
  only bounded inline PNG/JPEG previews are shown, with an explicit fallback.
- Tab and shell snapshots remain available to existing other-provider flows,
  not to native Codex in this phase. File uploads and visual input remain
  disabled for adapters that do not consume them, with an honest explanation.

### Context-window selection and telemetry

- Keep exact context-window selection disabled until native configuration supports
  it in this client. The main composer omits context-report chrome. Each native
  graph card shows one small, borderless monitor in its header: the `Context`
  label, the latest report's percentage when capacity is known, and a short meter.
  Before the first report it uses an em dash instead of waiting prose. Detailed
  counters, tables and cumulative totals do not appear in the card.
- Never use cumulative tokens as context occupancy or add cached/reasoning subsets
  to native totals. Details show each counter with its original category. Missing
  counts remain Not reported; reported zero is zero. No billing cost is inferred.
- Token events are scoped to native thread and reported turn. Switching owner or
  provider cannot inherit another conversation's numbers. A disconnected/reconnected
  snapshot and a previous-turn report are explicitly stale until new native data.
  The graph monitor marks disconnected, reconnected and previous-turn values as
  stale in its accessible status without borrowing another card's report.
- Keep legacy context hooks hidden, inert and outside the reading/tab order;
  they reserve no layout space. Graph cards omit the tab/shell attachment menu;
  existing attachment previews, removal and supported drag-and-drop stay available.
- Current chat names and relationships belong in the Projects tree; the main
  composer does not repeat a title, generic Chat badge or identity banner.
  Keep the chat-identity event for Settings ownership and native commands.

### Provider configuration and capability honesty

- Native Codex workspace access is selected in Settings → Agents & permissions and shared by all Codex
  agents, including existing/new main chats, graph agents and fork destinations
  (user direction 2026-09-13). Compact graph cards do not repeat this menu.
  Read only is the initial default when no saved choice exists;
  project access uses the native
  workspace-write/on-request preset. Full access requires an explicit in-product
  confirmation that it removes the Codex sandbox and command approval prompts
  for all Supervisor Codex agents and remains active after restarting the app.
- Save this single preset atomically in Supervisor's own data directory before
  acknowledging a change. Restore it on startup without changing native defaults,
  accounts or request-specific grants. Invalid/missing primary files never restore
  broader access from a backup; preserve them and explain the read-only fallback.
  The shared preset is captured with accepted input. Finish active/queued Codex
  work before changing it; summaries retain their separate owner-scoped guard.
  A saved choice does not bypass fresh native managed requirements: disable
  unsupported choices and revalidate before every execution. Named managed
  profiles are not overridden by presets. Other providers retain their own policy.
- Windows sandbox setup is a recovery action inside AI accounts → Account options,
  labelled Repair Codex on Windows. Show it only as troubleshooting for command or
  project-edit failures, with elevated and legacy modes confirmed separately. Never
  start setup, elevate or retry automatically. Display its asynchronous state,
  retain completion-before-ACK and unknown-after-disconnect honestly; setup
  completion does not grant full access.

- Reported thread settings remain runtime-owned metadata driven by the stable
  public fields in thread start/resume/fork
  responses and by later native thread/settings/updated events, never by
  config/read or composer intent. Fields not present in the initial response
  remain Not reported until a settings event supplies them. Show public
  model/profile/directory and policy-kind labels;
  exclude collaboration instructions and raw permission/config objects. These
  labels are neither permission consent nor a complete allowed-path list. Missing
  reports are explicit. Disconnect, unload or malformed updates mark the previous
  report stale. Do not expose this technical report as a general-user setting;
  history reads and late start/resume/fork acknowledgements cannot
  make it current. A settings notification received after a session request owns
  the newer revision and cannot be overwritten by that request's later response.
  Preserve typed drafts and selected profiles; never start a prompt or reload just
  to fill this disclosure.

- Settings uses English labels and short explanations of effect and scope.
  `SettingsShell.svelte` owns the top bar, static search, category heading,
  category switching and reading column. The top bar keeps Settings at the left,
  a wide searchable field in the center and a bare back arrow at the right.
  The arrow retains an accessible Back to workspace name without a surrounding
  circle or visible text. `SettingsNavigation.svelte` exposes a centered two-level selector: the
  primary row contains Personal, AI, Workspace and Connections, and a compact
  secondary row appears only when the selected group contains several pages.
  General, AI accounts, Agents & permissions, Codex tools, Projects, Servers and
  Windows access remain the only selectable pages.
  Supervisor tool confirmations share Agents & permissions while retaining their
  legacy section ID and hooks. Privacy explanations and diagnostics are not
  preference categories. Keep existing native hooks and mounted controls while
  useful legacy content is adopted once into this shell. Switching,
  searching and reopening preserve drafts, selections, disclosures and each
  category's reading position during the app session. Navigation sends no native
  action and cannot hide an open modal. Close owned dialogs before hiding Settings.
  Search uses a local catalog of labels and aliases only; never index chat text,
  account identifiers, credentials, file contents or unsaved form values. Results
  open their category and reveal the indicated setting, then focus it.
  Escape clears a nonempty search before the normal Settings close behavior.
  Use shared tokens for a bounded wide reading column, a clear display heading,
  body-sized labels and neutral controls. Selected primary and secondary tabs
  must remain obvious without relying on hue. In narrow windows, preserve the
  two navigation levels as horizontally scrollable rows without compressing
  their labels. Keep the page background continuous and remove obsolete legacy
  shell styling after migration.
  Keep the top bar and search area free of a bottom divider. Use a fixed
  typographic reading order: page title largest and strongest, page subtitle
  lighter, section heading smaller than the page title but larger than a control
  name, and each control explanation one step smaller and lighter than its name.
  Put explanations below the name in both wide and narrow layouts rather than
  sharing its baseline. Scope labels remain secondary to section headings;
  selector values remain legible without competing with headings. Use shared
  settings type tokens from `assets/themes.css` for this scale.
  In Agents & permissions, keep the three plain section headings on the page
  background and place each actual setting inside a quiet filled, borderless
  option panel. Workspace access and both of its permission explanations belong
  in one panel; the section heading and scope remain outside it. The panel is
  written content, so only its inset value selector gets pointer, hover and
  expanded states. Keep the panel and selector fills distinct in Light and Dark.
  Common account actions and actionable errors remain easy to find. Other
  providers, saved-chat reload and account recovery may use named disclosures.
  Protocol diagnostics, runtime inventories, account activity, workspace messages,
  reported thread settings and storage internals do not belong in user Settings;
  retain their runtime ownership and existing authority boundaries.
  Explicitly separate shared Codex workspace access from Supervisor tool confirmations
  and from preferences scoped to the selected conversation.
  Stable page sections are plain headings and remain open so they cannot be
  mistaken for controls. Settings disclosures must look interactive before
  hovering: use a full-width borderless neutral fill, readable label and trailing
  chevron pointing right when closed and down when open. Use the same treatment
  for native summary elements and configuration section buttons, including nested
  options. Keep expanded content on the page background, with restrained nesting
  indentation instead of another enclosing card. Select fields retain a visible
  dropdown arrow and match the control surfaces. Distinguish hover, expanded,
  focus and disabled states; preserve native keyboard operation and reduced motion.
  Every exclusive preference uses a closed selector: its alternatives appear only
  after the user opens it. Boolean preferences remain switches. Do not render theme,
  access or confirmation alternatives as permanently visible cards or button rows.
  Settings selectors use the same visual grammar as the main Provider, Model, Effort
  and Speed pickers: a borderless filled value button, a clear chevron and an on-demand list
  with the selected item marked. Longer access choices may add one short description.
  Preserve native select IDs as hidden compatibility controls, keyboard navigation,
  Escape/outside dismissal, focus return and acknowledgement-driven values.
  Settings uses no resting borders on search, navigation cylinders, selectors,
  inputs, cards, confirmation surfaces, badges, disclosures or menu options. Separate selectable and
  written surfaces with fill, spacing, type, hover and selected states. Preserve
  visible keyboard focus even though resting outlines are absent.
  Confirmation overlays use a centered card no wider than 560px, retain at least
  18px of viewport clearance and scroll internally when vertical space is limited.
  A confirmation must never expand to the full application width.
- The main agent's profile menu contains only Provider, Model, Effort and Speed,
  without nested section menus or advanced settings. Settings → Agents &
  permissions contains Agent behavior (Personality and Reasoning summaries),
  Codex workspace access and Supervisor tool confirmations. Settings → Codex
  tools separately contains Skills and Apps. The retired context selector stays
  mounted as a hidden compatibility hook and reserves no layout. Generic scope
  labels identify the current conversation without repeating its title; Codex
  workspace access explicitly identifies its persistent app-wide scope. These
  three groups start open so their available controls can be scanned as one page.
  Non-Codex providers
  keep applicable preferences without inactive Codex groups. Preserve one mounted
  instance of each picker, all IDs, selections, listeners and native owner routing.
  In main, the four-choice profile is a compact floating popup anchored to the
  logo. Prefer the empty gutter between Explorer and transcript content; when
  that gutter is narrow, constrain it to the chat inset and available viewport.
  Explorer stays visible and interactive. Opening, resizing and closing never
  change either column's width or move the transcript/composer. Long option lists
  expand and scroll within the bounded popup; resizing keeps it near its anchor.
- Graph cards expose Provider, Model, Effort and Speed through the two-path
  Supervisor mark immediately before Close. Opening separates those paths while
  the initially closed, two-column panel scales into place; closing recomposes the
  mark and makes the panel inert before hiding it. Rapid reversals retain the last
  requested state, and reduced motion changes state immediately with a complete
  mark. The panel overlays the conversation without moving it. Escape closes it
  and returns focus to the button; outside interaction closes it without losing
  selected values or drafts. The model shortcut opens this panel before focusing
  its selector.
  Native graph composers reuse the main owner-scoped plugin picker. The menu
  shows only icon and name; a selection becomes an icon-and-name chip with its
  own X and is consumed only by that card's next accepted prompt. New cards can
  choose a plugin before their first prompt; the selection is promoted only to
  the native thread created for that card.
  Cards do not repeat the app/project
  name, directory/profile rows, assignment fields, context selector or advanced
  configuration menus. Keep the existing hidden hooks and assignment values;
  the next message uses the existing assignment/submission path. This changes
  presentation, not saved names, permissions, missions or conversation identity.
  Native slash-command dialogs, selected context evidence and operation errors
  remain owned and available without reopening persistent configuration menus.
- Use `ConfigurationSection.svelte` for this hierarchy; no nested bordered
  cards or inherited legacy section padding. Titles, descriptions and model
  labels use shared body typography. Keep selected skill references/removal and
  actionable operation errors visible even when their group is collapsed.
  Group expansion and inner disclosures survive background native updates.
  Closing a group sends no RPC and changes no draft, profile or permission.
- Native history, goal, fork, review, skills and saved-default dialogs stay mounted
  as dialog-only controllers exempt from collapsing; slash
  shortcuts still show a visible modal when Settings is closed. The main Agent
  settings host temporarily moves outside the hidden Settings ancestor, with
  only open dialogs visible and interactive; it returns without remounting after
  the last modal closes. Give every
  group toggle a unique controlled-content ID and keyboard-visible focus.
- History, fork, review, compact, goal and saved-default operations live in their
  contextual chat/tree actions and shortcut dialogs. Confirmations and busy/stale
  guards remain unchanged. They are not general settings.
- Graph cards retain their explicit slash-command dialogs without rendering
  settings menus or shared Codex workspace access controls.
  They operate on Codex history, not on the local
  project/chat lifecycle. Read history after reconnecting before rename/archive/
  delete; show native names without renaming another provider's local chat.
  Archive/delete confirmations disclose spawned-child scope. Restore means
  unarchive one conversation, never restore project files. Deletion can be
  refused by Codex while a fork references the source history; disclose this
  dependency and preserve the conversation on rejection. Never auto-delete a
  dependent fork or retry to bypass the native refusal. Confirmations freeze
  owner and native thread identity and close on owner/provider changes.
  Deleted history disappears and cannot be resurrected by late events or queued
  input. The remaining local chat and other-provider histories are preserved.
- Browse Codex history opens an on-demand dialog from the native conversation
  controls in main and graph. Search is the server's case-sensitive title search;
  Archived only is an exclusive filter, and Load more follows native pagination.
  Each result identifies its human-readable directory, native provider,
  timestamp and reported state. A preview used as the fallback title is
  redacted, whitespace-collapsed and bounded, with truncation disclosed. List
  summaries never become local transcripts or implicit permissions, and opaque
  native identifiers are not presentation labels.
  Linking uses the same native ID; forking creates an independent native history,
  not a copy of files. Both require a frozen confirmation and an existing local
  destination without a Codex binding. Preserve its draft and other-provider
  history. No prompt or automatic retry follows either action. Switching owner
  closes the dialog, and changing project invalidates a pending choice.
  The selected runtime refuses forking archived sources. Archived results keep
  Link available and explain the separate explicit restore step; disable Fork
  and reject it again in Rust, without automatic unarchive or retry.
- The same history dialog exposes **Cloud** as a distinct source for Codex Cloud
  chats owned by the ChatGPT account connected to Supervisor. Opening the dialog
  loads the first cloud page automatically through the official `codex cloud`
  subscription command; visible nonterminal tasks refresh on a bounded interval,
  while pagination and manual Refresh remain available and never query Agents
  API billing. Cloud rows use a recognizable cloud silhouette and show
  the returned title, environment, status, timestamp, attempt count and bounded
  change totals without exposing credentials or opaque IDs. A requested unified
  diff stays read-only and bounded inside the dialog. **Open chat** and **New
  cloud chat** navigate to validated `https://chatgpt.com/codex` pages in a real
  Supervisor browser tab. The official CLI does not expose cloud transcripts as
  local App Server threads, so the complete conversation and follow-up composer
  remain on that first-party page; never synthesize, replay, link or fork them as
  local history. Closing, switching owner or disconnecting invalidates late list
  and diff replies without changing project files or sending a model prompt.
- When the loaded history exposes completed turn IDs, **Fork into a new
  conversation** may stop at one explicitly chosen completed turn. Freeze the
  exact `lastTurnId`, directory, source and read-only destination profile before
  dispatch; a late or foreign turn cannot redirect it. The normal product fork
  is durable and resumable. Keep ephemeral/config-override constructors as
  protocol-only until a concrete disposable workflow owns their lifecycle.
- Fork into a new conversation creates and saves a new project chat or a second
  agent card on the same physical graph node before requesting native thread/fork.
  Validate the observed source lifecycle before allocating that local destination.
  Archived/deleted sources cannot create empty branch chats or pending receipts;
  explicit restore remains separate. Archive events close an open fork confirmation,
  and Rust rechecks source eligibility even if stale UI intent arrives later.
  Its confirmation freezes the source and local directory. Keep the original
  draft and card; Open branch is an explicit navigation action, not a prompt.
  The new owner uses the shared saved Codex preset, never copied request grants.
  A persisted pending-fork
  reference prevents a replacement conversation after uncertain delivery; recovery
  links only an explicitly selected native fork of the original source. Do not
  replay forks, copy local/other-provider messages or imply copied project files.
- Review code uses the native reviewer in the current main/graph conversation.
  Confirm the immutable directory, review target and account usage before start.
  Targets are uncommitted changes, base branch, commit or custom instructions.
  Prepare the native session with read-only scope and user-routed approvals;
  reject a broader or different native scope, never fall back to full access.
  The reviewer model is configured by Codex, not inferred from the composer.
  Keep drafts and attachments intact. Stop cancels unsent review intent or
  interrupts the native turn. Unknown delivery requires native history review.
  Entered-review activity is distinct from the final exited-review text, which
  uses normal safe Markdown outside the completed work disclosure. Native
  history may also contain a completed, phase-less agentMessage immediately after
  exitedReviewMode with the identical nonempty result text. Render that exact echo
  only through the canonical exit row, preserving its stable identity and all raw
  native items. Do not deduplicate different text, explicit message phases,
  unfinished items, nonadjacent messages or repeated results in another turn.
  Plan updates retain one stable disclosure with reported step states. Neither review
  nor conversation history implies a Time Machine checkpoint or file restore.
  Separate review delivery is unavailable for the native paginated histories
  created by the selected runtime: its actual review/start rejects that delivery.
  Explain this limit in the existing review dialog when Codex reports paginated
  history. Missing history mode is unknown, not legacy compatibility. Explicit
  native fork followed by inline review remains two separate user actions; never
  silently substitute it for detached review or force an experimental history mode.
  If Codex explicitly reports a compatible legacy history, separate delivery is
  allowed and must bind only the exact `reviewThreadId` returned by the response;
  response-before-notification and notification-before-response are both valid.
  Never infer the destination from `thread/started`, the source, or timing.

- Native Goal opens the same on-demand dialog from main/graph controls or /goal.
  The native thread owns objective, status and accounting; no local scheduler or
  synthetic continuation prompt is added. Read explicitly before editing, freeze
  owner/thread/project/view and confirm every write. Native state updates retain
  unsaved text; accounting alone does not invalidate a pending confirmation.
  New objectives start paused; budget is optional and initially unlimited. Native
  account/runtime limits still apply. Active state uses the loaded Codex session,
  not pending composer selections. Explain that pause/complete/remove are not
  turn interruption or file restore. Stop remains separate. Replacement can reset
  native accounting; status/budget-only changes omit the objective. Shared-client
  races cannot be made atomic by this API: a fresh preflight read detects known
  changes, never claims compare-and-swap. Disconnect and scope changes invalidate
  edit authority without replay; late replies cannot overwrite newer events.
  Closing a dialog does not cancel an already sent write. Keep all controls
  neutral, keyboard accessible and scrollable, without changing chat drafts.

- Native Codex server requests use the same reusable decision region in main
  and graph conversations. Show command/directory, requested permission scope,
  proposed file diffs, native questions or the requesting MCP server beside the
  decision. The region is absent when idle, independently scrollable, and never
  replaces the transcript or composer. Preserve typed answers on stream updates.
  Append pending cards in arrival order for their owner, never lexical ticket or
  RPC-ID order. Later requests, sending state and resolution retain the identities
  and relative order of surviving cards; no queue or execution priority is implied.
- Managed-network approvals identify the native destination and protocol, not
  a shell command. Stdin callbacks explicitly identify input to a running process.
  In both cases any reported command stays in a context disclosure; it is not
  relabelled as the network target or fabricated input bytes. Main and graph
  share this distinction without changing the native decision or granting scope.
- Allow once, Decline and Cancel are explicit; longer session or policy choices
  live in a disclosure describing their scope. A permissions approval grants
  only the displayed native request. Pending decisions are keyed to the original
  owner and native connection; resolution, completion or disconnect invalidates
  them. No client-side timer approves a request automatically.
- If the native command request supplies available decisions, show only the
  supported choices it actually offers. Rust derives the button availability
  and revalidates the exact response against that same request. An empty or
  unsupported list grants no fallback; explain that the turn can be stopped.
  Do not invent broader session grants or edit the server's proposed rule.
- Secret question answers and MCP responses remain session-only, never part of
  the local transcript, logs or draft storage. External MCP authorization URLs
  open only after a click on the displayed address action; opening a page is not
  approval. Unsupported form modes are explained rather than simulated.
- MCP form defaults are initialized once per native request ticket. Refreshing
  an existing card or its sending state must not reapply defaults over entered
  values; a different ticket creates fresh fields without carrying prior answers.
  The initialization capabilities declare `openai/form` only because the host
  now has bounded JSON-Schema normalization and response validation. Render flat
  primitive properties as typed controls; nested arrays/objects use a sanitized
  JSON editor inside the same card. Reject unsupported keywords/types, excessive
  depth/size/counts and invalid results. Never render schema HTML or executable
  content, and keep all answers session-only.

- Codex App Server connection setup lives in AI accounts. First connection is
  explicit; show managed ChatGPT browser/device-code sign-in, cancellation,
  account refresh and actionable connection errors. The native model catalog
  populates contextual Provider, Model, Effort and Speed controls rather than an
  inventory inside Settings.
  Explicit sign-in/service authorization links use the OS default browser's
  HTTP(S) association, with the full URL kept intact. Do not pass web addresses
  to file managers or command shells. A launch failure stays visible beside
  the pending sign-in and must not expose URL queries in errors or logs.
  Do not mark account connectivity as conversation readiness while turn controls
  are unavailable. Unknown or non-ChatGPT auth modes are not subscription access.
- Supervisor owns a separate native profile at `<app-data>/codex`. Scope
  CODEX_HOME, SQLite and file-backed account storage to that child process;
  never fall back to the official app's profile. First-use migration copies only
  Supervisor-linked native histories, including known forks and delegates, with
  unchanged identifiers and verified bytes. Preserve missing links and originals;
  never import credentials, global configuration or shared databases. A completed
  migration is authoritative and must not restore deleted data on later startup.
  Before first use, let the native runtime rebuild transferred paginated history
  by reopening only copied IDs with read-only access, then unsubscribe/archive.
  Send no turn or tool approval, retain archived state and fail visibly if native
  hydration fails. Normal startup never repeats completed migration hydration.
  Preserve native titles and original goal snapshots through public metadata
  reads, never shared database copies. Completed goals stay completed; unfinished
  goals never restart automatically or receive a replenished token allowance.
  Account setup explains the separate history/settings and one-time sign-in.
- Saved Codex chats load automatically on launch and after connection, including
  their display history while signed out. Prioritize the selected chat, then
  read other saved chat/graph bindings one at a time through native metadata and
  paginated history APIs. Do not resume a thread, send a prompt, import unrelated
  chats, clear a draft or restore permission grants. Keep archived state, missing
  links and existing messages on failures. Skip active conversations and recheck
  ownership before every queued read. AI accounts offers Reload chats with
  loading, empty, complete and partial/unavailable feedback; manual reload uses
  the same reader without restarting a connected runtime or rerunning migration.
- Remember a confirmed ChatGPT connection as a local, non-secret intent and
  reconnect once on the next app launch using Codex's own cached authentication.
  Existing native bindings or a saved Codex model profile allow one read-only
  startup check on upgrade; neither is proof of authentication. Show Connecting
  and Checking while the fresh account/catalog/policy reads complete, not a new
  sign-in request. Never open OAuth, resume a thread, replay work or restore
  session permission grants automatically. Sign-out must durably disable the
  remembered account connection before its native request; late account replies
  cannot undo it. Local saved history may still start the isolated runtime once
  on the next launch to read messages, without opening authentication.
  Temporary failures retain the preference without a retry loop; an authoritative
  missing/unsupported account disables it. Corrupt preference files are preserved
  and never recovered from a backup that could predate sign-out.
- Open the official sign-in page only on user intent. Device codes are selectable
  and temporary. Sign-out requires an in-product confirmation explaining that
  only Supervisor's native profile is signed out; no local file or chat deletion
  is implied. Tokens and raw configuration never cross into the UI or persistence.
- Connection, auth and catalog errors remain actionable in Settings. A failed
  refresh cannot silently present a partial model catalog as complete. Responses
  from an old connection/account refresh cannot replace the current state.
- A transition from disconnected to connected clears a superseded local
  connection-required alert in both main and graph decision surfaces. It must
  not clear a real native operation error received while already connected.
- Model/provider capability, personality and upgrade data is native catalog
  metadata, not authority. Offer personality only when the selected model reports
  support and freeze it with accepted Start/Queue input; Send now cannot retune an
  active turn. Sanitize upgrade links/Markdown and present upgrades as information,
  never automatic replacement. Provider capability flags control only matching
  UI affordances and never imply account access or tool permission.
- API-key and Amazon Bedrock account responses are valid but explicitly
  unsupported execution modes for this subscription-only client. Show that policy
  in Settings and provider readiness; never read, persist or forward their keys or
  credential-chain data. Only a confirmed ChatGPT account unlocks native turns.
- Apply the stable paginated permissionProfile/list inventory internally. Managed
  requirements that name profiles keep incompatible access choices unavailable
  and provide an actionable explanation. Do not expose the technical inventory or
  a named-profile selector, send experimental thread/start.permissions, or
  reconstruct a named profile as sandbox policy.
- Show authoritative ChatGPT rate-limit buckets with missing/current/stale/loading
  states. Treat account/rateLimits/updated as invalidation and refetch the complete
  account/rateLimits/read snapshot, including one coalesced follow-up when a read
  is already in flight; never merge the sparse notification or expose account
  IDs, individual credit records/IDs, spend controls or upsell payloads. The
  exact available reset-credit count remains internal until a dedicated,
  contextual redemption flow needs it.
- Keep `account/usage/read` and workspace messages bounded and Rust-owned, with
  64-bit counters exact and nullable values distinct from zero. They are not user
  preferences and do not appear in Settings.
- Configuration, deprecation, global runtime and Windows world-writable warnings
  are bounded visible Settings notices, separate from protocol diagnostics.
  Preserve only documented display fields in process state, mark them stale on disconnect,
  and escape their contents. Unknown fields and raw payloads never reach the UI.
- Protocol diagnostics is runtime-only, never a Settings disclosure, alert or
  transcript row. Count notifications without a projected UI update, including
  unsupported and out-of-scope events, without calling them failures. Retain only
  stable-schema method names; unknown names use SHA-256 fingerprints. No payloads,
  thread IDs, commands or credentials are retained.
  Keep the 32 most recent method identities in process state with explicit count scope.
  Disconnect retains last-connection observations; reconnect clears them. This
  diagnostic capacity is not a limit on agent actions or conversation output.
- Optional stable App Server workflows stay outside general-user Settings. They
  do not imply full public-method parity and never set `experimentalApi`. Any
  dedicated contextual surface for a side effect requires an in-product confirmation,
  bounded projected state and no automatic replay after disconnect. The dialog
  freezes the exact view, scope and values it presents; a later project or
  inventory change cannot retarget the confirmed operation.
- The P2 project scope follows active local-project changes in Rust and is
  available on first render without a manual synchronization step. Windows
  extended-path prefixes remain internal. Long feature inventories expose a
  text filter and native checkbox labels remain one connected hit target.
- Runtime feature inventory is paginated and atomic. Feature enablement is
  process-wide, bound to a fresh inventory view and available only for entries
  reported as beta or stable. Under-development, deprecated and removed entries
  remain read-only. A change updates only the named flag and never writes raw
  config or enables the experimental protocol surface.
- External-agent migration is a detect-preview-confirm workflow for home and/or
  the currently active local project. Rust chooses the paths, retains raw native
  migration details and gives the WebView opaque item handles plus bounded
  descriptions. Import sends only exact selected detected items. Changed project
  scope invalidates the preview; progress and completion show sanitized counts,
  not raw failure messages, and history omits source/target paths. Unknown delivery requires history
  reconciliation before another attempt. After acceptance, import remains a busy
  mutation through progress until authoritative completion; it blocks a second
  import and reconnect. Completion cannot be downgraded by a later progress event,
  and disconnect after acceptance becomes an explicit uncertain state. Central
  Agent does not fabricate an external history record for an import it already initiated.
- Reset-credit redemption is ChatGPT-account-only and consumes no WebView-visible
  credit ID. A new attempt requires a current authoritative positive available
  count. Rust creates the idempotency key; an unknown delivery retains it and
  an invalid success envelope also remains uncertain and retains it; an explicit
  Retry uses the same logical attempt. A confirmed reset refreshes
  authoritative limits. Add-credits/usage-limit email nudges are explicit and
  report sent versus cooldown without automatic resend.
- Feedback is an explicit text-only upload with a bounded category and reason.
  The confirmation previews exactly that text and managed requirements may
  disable submission. Logs, file paths, attachments, tags and conversation
  content are always omitted. Rejection, invalid response and disconnect end the
  progress presentation in explicit failed or uncertain states; disconnect never
  retries it or reports uncertainty as success.
- App Server sandboxed command is visually and semantically separate from the
  Supervisor Terminal. It accepts a confirmed argv JSON array for the current
  local project, never shell-interpolated text. Only read-only or project-write
  sandbox policies are available; full access, network access and environment
  overrides are absent. The client supplies an opaque connection-scoped process
  identity, bounds/escapes output, and exposes explicit stdin, resize and
  terminate controls. Project changes cannot retarget a process and connection
  closure is reported as termination, never replayed. A certain terminate rejection
  restores controls only if the process is still running; completion remains
  terminal if it races the response. Invalid or delivery-unknown termination stays
  active and explicit until process completion or connection closure.
- Native `fs/*`, `thread/inject_items`, `thread/shellCommand` and
  `config/value/write` remain intentionally absent: Explorer/editor, Terminal
  and revision-checked batch configuration already own those capabilities.
  Plugin/marketplace and experimental thread pagination remain maturity holds,
  not hidden Settings actions.
- Native MCP inventory remains runtime-owned and may support agent tool execution;
  it is not a general-user Settings disclosure. Explicitly
  distinguish configured inventory from an individual thread's runtime tools;
  absent runtime/auth state is not success. Publish complete paginated inventories
  atomically and label the last observed list stale during refresh/disconnect.
  Tool/resource metadata is escaped text, never executable content or fetched
  previews. Native configuration reload needs an inventory-bound confirmation;
  its acknowledgement means refresh is queued, not complete. OAuth requires a
  separate service sign-in and explicit opening of its validated URL; display
  only the authorization origin, not query tokens. No automatic retry, invented
  cancellation, credential storage, custom MCP registration or model prompt.
  Large server/tool/resource inventories provide a local text filter and never
  use an opaque thread ID as visible metadata.
- Direct conversation MCP resource reads use Rust-owned opaque entry handles;
  resource URIs never enter the WebView. Direct tool calls require a separate
  confirmation frozen to owner, native thread, inventory, server, tool and JSON
  object arguments. They bypass model prompting but not native policy or approval
  callbacks. Bound and sanitize text/structured results, omit binary/media bodies,
  and disclose that disconnect can leave a side-effecting result uncertain. Never
  retry either operation automatically.
- Any contextual native MCP tools must be presented beside the conversation that
  owns them. Internal configuration supports explicit versioned inspection,
  disabled STDIO/HTTP creation, enable/disable and removal of an observed base-user
  entry. Confirm the frozen server intent, shared target file and view before any
  write. Preserve other entries and unedited fields; no raw saved commands, URLs,
  arguments, environment values or credentials are projected. New input remains
  session-only until explicit saving; credentials use environment-name references.
  Saving and reloading are separate. Active/queued work and shared config writes
  block mutation; stale views/disconnect require refresh, never retry. Removal
  can reveal another layer and does not sign out OAuth.
  A delayed close event from an earlier MCP confirmation must not clear a newly
  opened confirmation. Clear cancelled intent only while its dialog is closed;
  the current inventory/revision must still be validated on the next confirm.
  Edit connection options operates on one explicitly selected key of the saved
  user entry, not the possibly different effective transport. Report only the
  saved key names; existing values remain private. Typed replacement covers
  executable, arguments, directory, URL, environment-name references, native MCP
  timeouts, required startup and tool lists. No timeout or filter is set by
  opening the editor. Clear saved option is separately confirmed, only for an
  observed optional key; [] and false remain real values, not deletion.
  Freeze the editor view, server, key and newly entered value. Replacing a map
  replaces that exact map, not a merge retaining hidden entries. Other options
  and credentials remain unchanged; confirm their possible reuse with a changed
  executable or endpoint. Arbitrary raw environment/header secrets and switching
  an existing entry between transport kinds are not exposed. No auto reload.
  Refresh targets that exact loaded native thread, not the Settings inventory;
  it never resumes a conversation automatically or starts a model turn. Show the
  native thread identity, tools/resources metadata and actual runtime/auth status
  using the shared MCP disclosure. Shared reload remains in Settings, while
  service OAuth can be started explicitly in its native thread scope. Freeze
  the observed inventory/server and validate the thread again in Rust. Opening
  the returned authorization page is a separate click bound to that exact attempt;
  show only its origin. Explain that Codex owns the credentials, which may be
  shared with other conversations/clients, not a Supervisor permission grant.
  Runtime startup/reload events invalidate metadata without cancelling the login
  or losing an early completion. Match completion by server and exact native
  thread, never the name alone; a global completion cannot complete a thread login.
  Disconnect discards pending state without claiming cancellation or replaying it.
  Pagination stays bound to its original
  owner/thread; replacement, disconnect, unload or server changes invalidate old
  responses. Preserve the last observed list as stale until explicit refresh,
  without replaying reads or clearing drafts, approvals or another pending action.
- Native Apps are selected from the exact loaded thread through atomic
  `app/list` pagination, `app/installed` runtime state and chunked `app/read`
  metadata. Settings may show bounded names/descriptions and public tool summaries;
  never fetch logos, install URLs or arbitrary metadata. The composer owns the
  optional per-prompt selector and presents only local icon-and-name identities;
  Settings owns inventory inspection and refresh. Opening the composer selector may
  perform its first explicit inventory refresh, but selecting a row sends no model
  request or tool call. Keep the exact `$app-id` selection internal and show the
  matching icon-and-name chip until its prompt is accepted. Installed
  plugins remain available to Codex's automatic tool choice when the user makes no
  explicit selection. Freeze at most 16 safe app IDs with accepted input and prepend the
  official `$app-id` reference plus the matching native `mention` item. Codex owns
  authentication, policy, tool execution and approvals. Runtime/account/list
  invalidation makes unsubmitted selections stale; a list update racing an
  explicit read is coalesced into at most one bounded follow-up snapshot instead
  of cancelling itself or looping. The explicit Refresh action requests fresh
  catalog and installed-runtime snapshots. If the catalog alone denies access,
  load the installed runtime IDs and resolve their public metadata directly.
  Label this as connected apps with the full catalog unavailable. Selection still
  requires fresh effective enabled/callable state and accessible metadata; missing
  metadata never grants access. Do not install a duplicate plugin, change account
  permissions or add a local connector runtime to recover catalog discovery.
  An HTTP denial from installed-state or metadata reads is a distinct,
  non-current account/workspace-unavailable state: it clears superseded loading
  notices, never suggests that resuming the conversation will fix entitlement,
  never labels unavailable data as an empty catalog, and performs no install,
  selection, call or retry. Large App/tool inventories remain filterable by plugin
  or command in Settings; the composer popup is a plain scrollable identity list.
  Use bundled local marks for GitHub, Figma, Linear and Notion and a deterministic
  local fallback for others, never fetched remote branding. Keyboard focus, Escape, outside
  dismissal and reduced motion match the model selector. Main chat and graph cards
  share the same owner-scoped control. No local connector loop exists.
  The first-party Browser package is the deliberate exception to the installed
  marketplace inventory: verify its OpenAI manifest, single cached version,
  skill, browser client/service and shared official runtime before showing it.
  Selection sends the native skill reference through the existing Codex turn;
  it never claims that an in-app browser is connected merely because files are
  cached. Availability additionally requires Supervisor's live, process-scoped
  WebView2 bridge. Copy only the verified service configuration into the
  isolated Supervisor Codex profile, then force its exact backend pipe,
  `iab` backend type and Supervisor build flavor; never redirect or modify the
  official Codex profile. The official service retains policy, approval,
  Playwright and CDP orchestration while Supervisor owns the tabs and forwards
  CDP calls and events. User-created tabs persist. Agent-created tabs are
  temporary and close at turn end unless the official deliverable or handoff
  mark keeps them; marks apply only to that turn. Browser remains usable when
  the separate Computer Use plugin is disabled but its own verified runtime is
  available.
- Codex hooks are a read-only native inventory and recent activity disclosure for
  the exact local directory/thread. Never expose command bodies, MCP target names,
  hashes or plugin internals, and never execute, edit or retry hooks locally.
  Sanitize bounded lifecycle output, correlate by native run identity, and mark
  inventory stale on directory, lifecycle or connection changes.
- Codex skills opens an on-demand native inventory for the explicit local
  project from the main profile controls or /skills in either composer.
  Show description, scope, enabled
  state, path and discovery issues as escaped metadata, not loaded instructions
  or remote icons. Enable/disable requires a frozen inventory/path confirmation
  explaining configuration shared by Codex conversations in Supervisor. Native
  effective state may differ from the request; do not infer success from intent.
  Changed skills, project switches and disconnect invalidate old choices. A
  pending write survives closing the dialog, blocks new native work briefly and
  is never replayed after lost delivery. Refresh is explicit and sends no prompt.
  Use with next prompt selects only enabled inventory entries, with removable
  owner-scoped references shown near the native controls. Selection sends no
  prompt and changes no shared configuration. Native start/Queue/Send now capture
  exact references and send explicit $name text plus native skill input; Codex,
  not the client, loads instructions. Queue acceptance and native acknowledgements
  consume only matching selection IDs, preserving later edits. Changed skills
  invalidate unsubmitted selections until explicit refresh/removal. Frozen queued
  references retain their original project and are never recaptured from another
  chat; the runtime owns the actual instructions loaded at execution time.
  Extra roots are an explicit process-scoped native setting, limited to 16
  existing canonical absolute directories. Reject secret/credential directories,
  invalidate every discovery view after a write, and explain that reconnect or
  restart clears the setting. Never persist or infer extra roots from a project.

- Official Computer Use presents a narrow dual-contrast frame along every
  active Windows monitor while an agent uses the verified desktop bridge. The
  frame starts on a native `node_repl` `js` item that invokes `@oai/sky`, not on
  a prompt mention or skill selection. It remains through that turn so the
  operator can tell that foreground control is ongoing. Completion, thread
  closure, disconnect, reconnect and application exit remove it. It is native,
  topmost, input-transparent, absent from task switching, and never steals
  focus or blocks the target application's clicks. The desktop may contain
  arbitrary Light/Dark content, so a dark outer edge and light inner edge
  provide contrast independent of Supervisor's appearance mode.

- Computer Use requests attach only the verified official native skill. The
  retired Supervisor companion is removed only when its ownership marker is
  present; user files and preferences are preserved. Main chat, graph cards,
  queued input and steering keep the same frozen official references and respect
  native disablement. Tool discovery is displayed as configured, never as proof
  of a live desktop connection. Native connection failures give an explicit idle
  reconnect action without replaying input; a completed tool call does not claim
  visual correctness. Reconnect refreshes verified official references. Multiple
  cached plugin versions without an authoritative selection remain unavailable
  with an actionable explanation instead of choosing one by directory order.
  Status detection is passive: no periodic captures, extra model turns, changed
  permissions or model/effort/speed choices.

- Official Browser reuses the verified first-party interaction runtime and
  native skill path. It appears in the composer only after native skill-root and
  `node_repl` discovery succeed; choosing it performs no navigation, screenshot
  or model turn. The selection is frozen with one prompt, owner-scoped, removable
  only with its X, and consumed only after native acceptance. Browser and
  Computer Use may be enabled independently while sharing one verified server
  and one set of lifecycle hooks.

- Codex defaults, opened from the main native controls or /config in either
  composer, inspects effective on-disk project
  preferences, their origins and the base user-file values. It does not claim to
  describe an existing conversation's loaded settings. Expose only public model,
  effort, summary, verbosity, service-tier, web-search and context preferences;
  raw config, credentials, instructions and environment values stay Rust-side.
  Each edit requires a frozen view/project/key/value confirmation explaining the
  shared user config target and native expected-version check. Never fall back to
  unversioned writes, automatic retries, direct TOML edits or broader file targets.
  Saved and effective are separate outcomes; Refresh observes project overrides.
  Loaded conversations are not reloaded, and composer selections can override
  defaults. Closing a dialog does not cancel a write already sent. Concurrent
  native config/skill writes and active work cannot reuse stale edit authority.
  Clear saved value is separate from editing and requires its own confirmation
  of the observed base-user value. It removes only that public key through the
  native versioned writer, not the whole configuration or any project/managed
  layer. Empty text is not an implicit clear. Codex determines the resulting
  default; refresh observes it, and loaded conversations are not silently reset.
  Automatic compaction counting exposes native total/body_after_prefix choices,
  explaining full context versus growth after the retained compaction-window
  prefix. It changes neither the context-usage monitor nor model capacity; native
  Codex owns threshold accounting and compaction. Use the existing edit/clear
  confirmations, with no automatic reload or prompt.

- Available providers are native Codex, Claude Code, Cursor, GitHub Copilot, Google AI Pro /
  Ultra through Antigravity, and OpenCode Go. Claude Code is the default.
- Keep each existing adapter's authentication, catalog, permission and streaming
  behavior. Authentication remains owned by its CLI; never read native tokens.
- Model/effort/speed options come from the connected provider catalog. Keep
  Claude Code automatic model discovery and the other existing catalog probes.
- The project and agent graph remains provider-neutral and does not persist a separate knowledge store.
  A graph assignment does not imply browser, SSH or computer-control support.
- Clearly distinguish unavailable capabilities from an authenticated provider.
  Removing Codex does not confer its tools on another provider.

### Time Machine and change review

- `/diff` opens the read-only **Project changes** dialog, not a checkpoint or
  agent-attribution card. Group every listed path by staged, unstaged and
  untracked state, with the shared original file icons. A file can occur in both
  staged and unstaged groups; the total counts unique paths. Keep the list in one
  full-height scroll container with a local filename filter and no row cap.
- Only selecting a file loads its patch into the existing searchable diff
  viewer. Preserve the composer draft, owner, selected project and running work.
  Closing or changing owner/root invalidates late replies. Missing Git, an empty
  repository, unreadable/non-UTF-8 data and excluded paths are explicit states,
  never invented empty results. No model call, stage, commit or restore is implied.
  Renames use deletion/addition pairs; submodule worktrees are not scanned and
  staged gitlink entries remain visible. Known secret paths and links are blocked.
  Use shared monochrome tokens and symmetric borderless corners for the dialog.
- Main and graph conversations reuse this same dialog with independent owner,
  view, request and directory identities. A node's `/diff` never substitutes
  the selected main project. SSH nodes, relative paths and nodes without an
  explicit local directory are rejected before launching Git. Closing one
  popup invalidates only its reads; stale results cannot populate a sibling.
  Dialog titles and focus return target the originating composer, including
  multiple branches attached to the same physical node.

- Expanded activity diffs and Time Machine use `DiffViewer.svelte`, with the
  exact application background and readable `--ca-type-diff-code` text that
  follows chat zoom. Keep addition/deletion colors; do not use the terminal's
  blue-gray background or draw a frame around the code. Clip the viewer and
  code surface to shared, symmetric rounded corners. Reset inherited section
  padding, borders and shadows so a legacy separator cannot outline the diff.
- Search is literal, case-insensitive, local to the loaded diff and read-only.
  Typing selects and scrolls directly to the first match. Enter/Shift+Enter and
  previous/next controls navigate all matches and wrap. Ctrl+F in the code
  focuses search; Escape clears it. Count and diff-line location are explicit.
  Every occurrence stays highlighted even when focus leaves the search field;
  the active result has stronger contrast. Do not depend on CodeMirror's native
  search panel being open for these marks to exist.
- Streaming and unrelated state updates retain the mounted diff, open
  disclosure, query and reading position. A different selected file recomputes
  results; search does not imply access to omitted hunks or unopened files.

- A coding run creates its checkpoint before its first potentially mutating
  local workspace action. A read-only request must not claim changed files just
  because unrelated build output already exists.
- The live composer delta uses the native turn diff and replacement patch
  snapshots for Codex App Server conversations. Other local providers compare
  the current workspace with that run's immutable Time Machine baseline in a
  read-only scan that creates no blobs or manifests. Unified-diff headers do not
  count as code lines; binary changes may affect file count but never invent
  text additions or removals. Missing remote or provider evidence stays absent.
- Overlapping mutating runs are coordinated so each checkpoint has one clear
  owner and restore boundary. A finalizing checkpoint still counts as active.
- The summary shows changed-file count and aggregate additions/removals. The
  expandable tree preserves folder hierarchy and the shared language/file icon
  identity; the unified diff uses explicit addition and deletion styling.
  Render every captured file: the tree has no fixed-height inner scroll box or
  row cap. In the viewer, the left column uses its full available height and is
  the sole tree scroll container; inline trees grow with the conversation.
- Restore is available for the complete checkpoint or one file. A newer manual
  edit blocks overwrite and produces a recoverable explanation.
- Interrupted checkpoints remain reviewable. Recovery messages must distinguish
  safely captured changes, skipped unreadable paths, and a fully restorable
  checkpoint rather than presenting all three as equivalent success.

### Project board and saved agent cards

The September 20 board workflow keeps each open chat directly below its own
navigation row. The full saved card remains the one implementation. Focus
widens one conversation lane and temporarily compacts the other cards without
changing their saved minimized state or runtime; Escape exits Focus. Columns
resize with pointer and keyboard controls, and Files can collapse. Keep these
presentation choices scoped per project for the current UI session only. They
do not become native session data or agent instructions.

The selected Supervisor exposes its Supervised conversation association through
one compact Supervisor settings trigger. The initially closed, bounded floating
panel opens above the board without moving or resizing conversation cards and
uses the shared workspace picker. Escape returns focus to the trigger and
outside interaction closes the panel. Project chats do not gain a second
settings surface.

New isolated tasks use explicit native destination selection and real Git
worktrees, shown as connected projects with their own conversations. Existing
dirty files stay in the original checkout. No layout or project-creation action
starts a model request.

- Open the board across the full application surface. Retire the canvas, force
  layout, zoom, filters and node dragging. Preserve native surface IDs and IPC
  only as inert compatibility hooks while Svelte owns the visible board.
- The left column contains explicit pinned/recent project folders. The next two
  columns contain project chats with their real fork trees and supervisor agents.
  The last column shows files inside the selected project. At compact widths,
  scroll the column grid horizontally instead of shrinking the portrait cards.
- Never enumerate a computer, drive, home or SSH server to populate this board.
  Local files use the bounded workspace explorer; SSH metadata includes a
  bounded depth-three listing of the explicitly connected directory. Remote
  rows lead to that project's terminal; they are never opened as local files.
- An agent may be associated with a same-project local conversation or the
  whole project. A same-project association is an operational supervision link:
  the card keeps its independent Supervisor conversation and exposes a separate
  Observed agent timeline containing the linked conversation's public prompts,
  reasoning summaries, tool activity, commands, file changes, requests, final
  responses and state in native order. It never exposes private chain of thought
  or opaque tool payloads. Each Supervisor request receives a bounded snapshot
  of that same display-safe evidence so the model can assess the worker's latest
  turn. Association and creation alone do not run either agent or grant new
  permissions. The first explicit request sent to a linked native Supervisor
  arms an event-driven supervision loop for that association. Worker turn start,
  plan, completed tool/command/file checkpoints, pending requests and turn end
  are coalesced before starting another bounded Supervisor review; token deltas
  never trigger model calls. Automatic reviews use a structured decision and a
  read-only turn. A completed worker gets one final review, then the loop remains
  dormant until that linked conversation starts work again. Invalid, archived
  and cross-project associations are rejected by Rust.
- Reuse that supervision loop for one compact delivery state. Show Working for
  an active linked turn, Checking for its terminal review, Ready only when the
  structured final assessment finds no unresolved failure, blocker or required
  validation, and Blocked for pending decisions or a concrete revision. The
  state opens a bounded plain-text assessment beside the project chat and its
  Supervisor without adding another dashboard, task model or execution engine.
  The selector rows use this text alone, without a duplicate collaboration mark
  or hover tooltip; details remain available only through explicit activation.
  It never invents a build, test or command that the observed timeline did not
  report.
- While the observed conversation has an active native Codex turn, its composer
  becomes an explicit **Steer active turn** control. Rust revalidates the saved
  supervisor → conversation link and exact active `turnId` before `turn/steer`.
  A stale or completed turn never falls back to start or queue, and rejection
  retains the correction draft. The Supervisor and worker histories remain
  separate; accepted steering appears in the worker timeline. A structured
  automatic review may request the same correction path, but the host repeats
  the link and exact-turn validation after the review finishes. If the worker
  advanced or stopped while it was being reviewed, no correction is sent.
- Project changes hide other cards without unmounting their timeline/composer.
  Existing unassociated cards remain available under Other saved agents. The
  card implementation and persistence contract are recorded in
  docs/PROJECT_BOARD.md; never archive production chat data as a source snapshot.
- In the Project chats lane, an opened conversation and its selector row form
  one continuous card. The row acts as its heading; both parts share the chat
  card surface, touch without an inner gap, border, shadow or second frame, and
  keep one filled rounded silhouette. Separate chat groups retain the
  Supervisor-card spacing.
- `Add project` owns one bounded dialog with four sources: New project (a name
  plus a native parent-folder picker), a native Windows folder picker for an
  existing project, a Git repository URL plus a native destination picker, and an
  explicit absolute directory on a saved SSH server. The visible drop target
  accepts exactly one local folder. Canonical local roots and normalized
  server/directory pairs are deduplicated, so adding the same project activates
  its existing record instead of creating a second copy.
- New project exclusively creates an empty child folder; a name cannot escape
  the chosen parent or reuse existing contents. Cancelling leaves the draft intact.
- Active tasks appear as named links inside their owning project row. Following
  one selects its project/conversation and filters the file activity panel without
  launching a turn. Read/edit targets come only from structured tool events and
  remain relative to that project's root. Show the owner, Reading/Editing and
  Done/Stopped/Failed/Waiting states without relying on color. Concurrent file
  operations and owners remain distinct; completion clears only that operation.
  A disconnect or stop must not leave an unconfirmed operation looking active.
  Never infer file activity from reasoning, prose, shell text, or unrelated disk
  changes. Tools without reported file targets retain an explicit empty state.
- One compact monochrome collaboration mark mirrors the same structured live
  state across the owning project, its open conversation card and each exact
  file currently being read or edited. Project-chat and Supervisor selector
  rows use the textual delivery state instead of duplicating this mark.
  The mark is never inferred from prose or disk changes, never appears on a
  folder as if the folder itself were a file target, and clears when that owner
  is done, stopped, failed or disconnected. Waiting for an active turn decision
  remains live. Its shape and accessible label carry meaning without color; only
  the central bridge may pulse, and reduced-motion preferences disable that pulse.
- Inspect a selected project in the background after it is added and on explicit
  refresh. Detect repository, branch, modified-file count, worktrees, primary
  technologies, bounded `AGENTS.md` locations, common build/test commands,
  associated agents and the selected SSH profile. Inspection must stay inside
  the explicit root, skip dependency/build/credential trees, avoid executing
  project code or hooks, keep subprocess windows hidden, and report partial or
  unavailable metadata honestly.
- Project rows support Open project, Start agent, pin/unpin, terminal, folder
  reveal for local projects, Git status, metadata refresh and removal from
  Supervisor. Removal never deletes the local folder, remote directory or cloned
  repository. `Ctrl+K` focuses registry search. Reopen the most recent local
  project by default without presenting a redundant checkbox in the registry.
- Begin the four lanes directly below the 48px top-left return row. Do not repeat
  a `Workspace` eyebrow or the selected project name above the board; project
  selection is already clear in the projects lane.
- Opening Settings from the graph collapses both Rust's graph state and its
  projected DOM before restoring launcher bounds. Closing Settings must reveal
  the usable Supervisor logo, never a clipped graph heading. Repeat openings,
  both themes and docked/detached Settings follow the same transition; graph
  data, conversation identities and drafts remain intact.
- Selecting a supervisor opens its existing agent card in the third column.
  Keep multiple cards and their distinct conversation identities. Closing a
  card hides its view without stopping its agent; Stop is a separate action.
- Cards retain their 9:16 portrait aspect ratio and shared width token, bounded
  by the available column space. The timeline scrolls independently and the
  composer stays at the bottom, beneath bounded decisions and attachments.
- Every project-chat and Supervisor card exposes a compact Minimize control in
  its header. Minimize collapses only that card to its full-width header, frees
  vertical stack space and changes the same control to Restore. It never closes,
  stops or remounts the conversation: active work, streamed state, approvals,
  history, scroll position, attachments and drafts continue under the collapsed
  body. Restoring recovers the same 9:16 card and DOM identity. Project changes
  may hide a minimized card but must retain its minimized state while mounted.
  Close remains a separate adjacent action, and keyboard/assistive labels must
  distinguish Minimize from Restore.
- Hide the Setup/phase label, Connect, the profile-change hint and Start.
  The attachment plus sits inside the prompt field and opens the same native
  picker, filters and snapshot validation as the main chat. Its current profile
  is assigned before selecting files, so no initial model prompt is required.
  Match the main availability and 12-file limit, retain previews/removal, and
  keep cancellation, failed sends and another owner's events from losing files.
  A Windows file or photo dropped anywhere on an open Project-chat or Supervisor
  card uses that same owner-scoped snapshot path and shows a temporary drop cue.
  Native code retains the filesystem paths; the web surface returns only the
  card owner under the pointer. Folder drops outside cards remain project import.
  A compact icon action in the bottom-right corner of the prompt field sends
  text or files, becomes Stop during work, and offers Resume after confirmed
  interruption when the draft is empty. Its right and bottom clearance use the
  same spacing token in both card composers and the main agent composer. Stop remains pending until
  the owner's reported work ends;
  errors allow retry without claiming interruption. Resume explicitly submits a
  continuation in the same conversation through the existing input path, never
  replays the original prompt, changes provider or consumes a newer draft. It is
  also available from restored stopped history; empty Enter never resumes work.
  Preserve native hooks. Enter sends
  nonempty input through the existing guarded submission path; Shift+Enter edits,
  composing/repeated Enter does not submit, and pending acceptance prevents a
  duplicate send. Existing file/context drafts and exact-owner receipts remain.
  If authentication, the selected profile or pending work prevents submission,
  show the reported reason beside the composer and associate it with the input
  and Send action. Loading local history does not prove that Codex is signed in.
  A successful account/profile update enables the existing draft automatically,
  without reopening the card or adding permanent setup controls.
- Preserve the four model/provider, effort and speed choices, shared timeline,
  approvals and per-node drafts. Node relationships provide orientation without
  repeating a project heading or directory/profile rows inside each card.
  Retired assignment/metadata DOM hooks live in one inert compatibility block;
  keep the saved assignment values and IPC actions, without rebuilding hidden
  telemetry, profile labels or conversation menus on stream updates. The shared
  timeline owns message rendering; no second legacy line renderer is retained.
- Other provider agents remain usable on local nodes. Native Codex requires a
  local directory. A remote project stays an explicit SSH scope and is never
  substituted as a local Windows working directory.
- Manual SSH, explicit remote projects, RDP and VNC remain available independently of
  an agent. Their configuration and authentication must survive chat deletion.
- Graph drafts and activity use owner-addressed events. Queueing one node must
  not block or clear another node's input. Errors remain visible in that card.
- Native message deltas invalidate the graph timeline even when the runtime
  phase is unchanged. Update existing message identities without remounting
  the card, clearing a sibling draft or waiting for turn completion.
- Preserve local conversation history and links between assigned agents. Native
  Codex browsing and branching use the shared native conversation controls;
  custom graph coordination, edited-history commands and the retired outbox are
  not restored.
- Selecting native Codex updates only the graph assignment and native binding;
  it never creates or relabels an other-provider session. Existing provider
  transcripts and their metadata remain intact when changing the assignment.

### Native sessions and remote control

- The Web panel can be minimized from a persistent, keyboard-accessible toolbar
  toggle. The Agent conversation expands into its space without reloading either
  surface; docked shells retain their own visible area. Restore uses the saved
  split width, and explicit tab/address navigation restores the Web panel too.
  Minimized state persists across launches. Hiding or detaching Agent restores
  the Web surface rather than leaving an empty main window. Settings and Graph
  keep their full-surface behavior; the Web toggle is disabled while they are open.
- The browser tab strip belongs to the visible Web panel. Its native bounds begin
  at the browser pane edge when Agent is docked, while Agent uses the full height
  of its own pane. The strip is absent when Web is minimized and on Settings or
  Projects; those transitions retain every tab, URL, history entry, split and
  detached window. Hiding or detaching Agent expands the strip with the browser.
- A browser tab, shell, remote desktop, preview, or graph agent remains the same
  logical object when docked or detached. Detaching must not reload, restart,
  or discard history.
- Shell interaction is direct terminal interaction. Do not reintroduce Run,
  Restart, Clear output, or Control-C chrome around an ordinary shell session.
- Typed agent browser commands keep a run-owned active tab; selecting another
  visible tab does not redirect them. A closed target requires a new explicit
  selection. Typed local-shell commands use an owner-bound session and the
  accepted working directory, not the currently selected shell or project.
  Remote-desktop actions bind to the granted connection generation. Restarted
  processes and reconnected remote desktops never silently reuse saved native IDs.
- Dragging a tab into Agent creates a compressed, bounded preview with source
  and token estimate. Dragging a shell creates a bounded snapshot; Follow live
  mirrors later output only until submission and grants no command authority.
- Remote desktop pixels remain unfiltered. Connection, transfer, and agent
  control state belong around the canvas and never obscure its primary work
  area. Give agent control is explicit and session-scoped.
- Local computer control, SSH, filesystem mutation, and remote input remain
  Rust-authorized actions. Presentation code never converts a visual state into
  authority.

## Information hierarchy

Use three durable reading levels:

1. **Orientation:** current surface, target, connection, and agent state.
2. **Work:** messages, files, graph objects, terminal output, diffs, or remote
   desktop content.
3. **Evidence:** paths, timestamps, model configuration, token estimates,
   diagnostics, and provenance.

Primary actions should be recognizable by position and label before color.
Status colors supplement explicit text; they never carry meaning alone. Use
plain language for user-facing state and reserve protocol or runtime vocabulary
for details that help diagnose a problem.

## Visual system

### Color

- Consume `--ca-*` semantic tokens from `assets/themes.css`.
- Use only white, black, and perceptually neutral gray tokens in application
  chrome and trusted product surfaces.
- Selection, connection, and primary actions use stronger neutral contrast.
- Success, warning, and danger remain explicit words or symbols; color never
  carries their meaning.
- Diff additions and removals use the shared green and red semantic tokens.
  Those colors belong to changed lines and statistics, not general decoration.
- Recognized file and technology icons may use their canonical shared
  `--ca-file-tone`. This exception identifies file type only and must not be
  reused as selection, status, project, or agent color.
- Web content, attached images or media, previews, and remote desktop pixels
  retain their source colors. Product chrome around them remains neutral.
- The terminal may keep its dedicated dark surface in both appearance modes.
- Do not use decorative gradients, colored fog, glow, or hue-based accents.
- New Svelte code must not contain raw color literals.

### Typography

- The interface follows the SF Pro typographic voice. Request `SF Pro Text` for
  interface copy and `SF Pro Display` for display roles when the operating system
  provides them. The Windows package embeds unmodified Inter Variable as the
  licensed, metrically similar fallback; Apple SF font files are never bundled.
  No font network request or system installation is needed. Retain the original
  outlined wordmark.
- Use `--ca-font-body` for interface language and `--ca-font-mono` for commands,
  paths, code, measurements, and machine output.
- Project-chat navigation uses the same interface family and `--ca-type-label`
  scale as file names. Its dedicated tokens keep that equality explicit.
- Consume the shared `--ca-type-*` and `--ca-leading-*` scale.
- Ordinary interface copy must not be smaller than `--ca-type-caption`. Truncated
  chat titles always expose the complete value through their accessible name and
  tooltip.
- Use weight, alignment, and spacing before uppercase or letter spacing.
- Truncation is appropriate for paths and tab titles only when the full value is
  available through selection, expansion, or an accessible label.

### Shape and boundaries

- Use the shared radius tokens. Similar controls at the same hierarchy use the
  same shape.
- Controls have four geometrically equivalent corners. Where a border is needed,
  keep it continuous and uniform. Do not use cut corners, asymmetric radii, colored border
  segments, inset edge bars, or corner markers on controls.
- Pills are reserved for compact toggles, counts, or short states. They are not
  containers for ordinary metadata or every button.
- Prefer alignment, spacing, and a change of surface over additional borders.
- Browser and terminal tabs and the Terminal button have no border. Preserve
  selected-state surfaces, keyboard focus and drag insertion feedback. Agent
  and Terminal share equal-width columns and the same height at every profile;
  the terminal label and count determine the width, with no fixed text clipping.
- Do not draw divider lines between persistent application regions. Borders are
  reserved for interactive controls, selected objects, bounded content such as
  diffs, and floating surfaces that need a perceptible edge.
- Avoid cards inside cards. A bordered region must own a distinct task, state,
  or interaction.
- Shadows are reserved for floating or detached objects and should communicate
  elevation rather than decoration.

### Icons

- Use the same Supervisor mark for the application and its two existing logo
  launchers. Keep their distinct accessible action names: configuration and
  Agent Graph. Do not scatter additional logos through operational headers,
  fabricate status from the mark, or reintroduce the retired brand/tetrahedron.
- Files and technologies use their original recognizable silhouettes when
  available and their shared canonical file tone. The same file must resolve to
  the same icon in Explorer, Time Machine, attachments, and graph nodes.
- Folders use one minimal neutral outline. Drives, servers, projects, and
  containers use distinct neutral system silhouettes; never color-code
  directory ownership or remote trust.
- An icon-only action requires an accessible name and a discoverable purpose.
- Do not mix unrelated icon families or place every icon on a decorative tile.

## Interaction and motion

Motion explains continuity:

- tab creation splits from the originating tab;
- tab closure resolves back into the remaining strip;
- a successfully dispatched prompt gives the Send control one compact
  press/rebound and lets only that conversation's new user bubble rise into its
  settled position with a clearly perceptible but finite scale, opacity, focus,
  and elevation transition. Use the shared deliberate 1200 ms send duration,
  keeping the rise visible through most of the interval and settling only at its end, so
  native acknowledgement and history reconciliation transfer the same elapsed motion
  to any replacement row instead of truncating it. Animate the persistent Send/Stop
  action slot so the immediate control change cannot end the feedback early. This keeps
  the transition visible without delaying prompt delivery;
- active project work reveals a still Supervisor indicator; it reports activity,
  not a percentage, piece count or predicted completion;
- a graph agent window follows its attached node;
- docking and detaching preserve the identity of the same surface;
- opening and closing settings or the Agent panel never expose an unthemed
  intermediate frame.

The main configuration logo separates its two original Regia paths when the
profile popup opens and reunites them when it closes (user direction 2026-09-13).
Coordinate this finite motion with the popup's scale/opacity transition from
the logo. Reversing a transition continues from its current position; an older
completion cannot hide a reopened popup. Closing controls become inert before
the popup disappears, and keyboard dismissal restores focus to the logo.
The canonical SVG and other logo instances stay unchanged; no tetrahedron,
continuous spin, progress percentage or model action is introduced. With
reduced motion, keep the mark whole and open/close the popup immediately.
Stable DOM IDs, native intent, disabled feedback and outside-click behavior
remain intact. Active-work indicators stay still.

When a configuration-owned native dialog is open, it has no hidden ancestor
and owns Escape/backdrop focus until it closes. Main shortcuts show only the
dialog while Settings and the profile menu remain closed. Clicking a
disabled control or replacing a focused control with its loading state is not an
outside-focus gesture and must not close the configuration. Long model
titles/descriptions wrap instead of clipping.

Use the shared `--ca-duration-*` and `--ca-ease-*` tokens. Avoid `transition:
all`, ambient motion, and effects that delay direct manipulation. Respect
`prefers-reduced-motion` without removing state feedback.

Prompt launch motion is finite and owner-scoped. Main chat, forks, and graph
cards use the same timeline behavior, consume the launch intent once, and match
the exact submitted text before animating. Loading history, switching owners,
receiving an agent message, resuming without a new prompt, or rerendering an
accepted native echo must not animate an old bubble. The prompt is readable in
its final layout throughout; motion never delays delivery, draft clearing, Stop,
or the native Sending status. Reduced motion keeps those state changes and
places the bubble immediately.

## Required states

Every interactive surface must deliberately handle relevant states:

- initial, empty, loading, ready, and stale;
- streaming, paused, completed, cancelled, and failed;
- disconnected, connecting, connected, and reconnecting;
- approval required, approved, denied, and expired;
- context Automatic, selected, waiting for telemetry, reported, high, critical,
  and not reported;
- focused, selected, dragged, docked, detached, and unavailable;
- light, dark, narrow, Full HD, 2K, and 4K where applicable.

State changes must not collapse the user's scroll position, disclosure state,
selection, typed input, or active agent history unless that reset is the action
the user requested.

## Manual source editor

- Local project files open in file tabs in the conversation column. Chat remains
  a persistent tab: switching to code never removes the conversation, draft,
  approvals, or running agent. Explorer remains available on the left.
- `FileEditor.svelte` owns tabs, dirty state presentation, confirmation, and the
  CodeMirror 6 view. Use shared neutral typography/syntax shades and the same
  canonical file icons as Explorer; never load an editor from a CDN.
- Rust owns complete UTF-8 reads, original project identity, revision hashes,
  atomic saves, and locally recovered unsaved drafts. Buffer content is not
  automatically appended to agent prompts or generic conversation state.
- Save is explicit (`Ctrl+S`). External changes block a save; Reload requires
  confirmation for a dirty draft. Closing a dirty file also requires an
  in-product confirmation. Saving into an overlapping active checkpoint is
  blocked; changing projects must never redirect an already open file.
- Initial support is local regular UTF-8 text (up to 1 MB per file), preserving
  BOM and uniform LF/CRLF. Binary, protected secret paths, links, unsupported
  encodings, and mixed line endings get an actionable error, not partial edits.
- Numbered lines, folding, search/replace, selection and undo history survive
  file-tab navigation. This is not a VS Code extension host, debugger, or LSP.

## Content and agent output

- Show agent activity in the conversation where it belongs, followed by a clear
  final response. Do not create a second runtime diary below the composer.
- Completed work histories use one duration disclosure. Inside it, narrative
  checkpoints keep chronological order and adjacent low-level operations use
  expandable, action-derived summaries rather than a permanently expanded log.
- Show only provider-exposed reasoning summaries and concrete progress. Never
  label summaries as complete private reasoning or chain-of-thought.
- Describe actions and outcomes, not internal plumbing that does not help the
  operator.
- Preserve Markdown hierarchy, lists, tables, code blocks, diffs, links, images,
  and file artifacts in final output.
- Long activity groups and conversations remain scrollable while streaming.
- Errors identify the failed operation, target, useful cause, and recoverable
  next action. Avoid raw protocol failures without explanation.

## Reject generated-interface reflexes

Do not introduce:

- a centered marketing hero inside a productivity surface;
- a grid of interchangeable cards when one workspace would be clearer;
- nested panels, redundant headers, or repeated summaries;
- a pill for every label, path, provider, model, or status;
- tiny low-contrast copy used to make a layout appear spacious;
- decorative gradients, glass, glow, noise, or arbitrary shadows;
- arbitrary color values, font sizes, radii, or spacing in a component;
- mixed icon styles, emoji standing in for product icons, or oversized symbols;
- controls that appear only on hover when they are necessary to discover the
  workflow;
- debug text, budgets, artificial action limits, or internal runtime labels in
  the primary interface;
- blank flashes, layout jumps, lost scrolling, or panels that close because a
  stream appended content;
- identical layouts for unrelated tasks merely because a component already
  exists.

Restraint must not become sterile. Create identity through proportion, rhythm,
alignment, original marks, and a distinctive relationship between the working
field and its controls.

## Adding a feature

Every future feature must answer these questions before implementation:

1. Which existing primary object owns it? Prefer extending one surface from the
   current map over creating a new panel.
2. What is the authoritative Rust state, and which Svelte event expresses user
   intent without granting authority itself?
3. What are its empty, active, waiting, success, failure, unavailable, and
   destructive states?
4. What existing scroll position, draft, selection, expanded group, process,
   or conversation identity must survive its updates?
5. Does it belong inline, in a disclosure, in the dedicated Settings surface,
   or in a genuinely new full application surface?
6. Which shared component, token, file icon, confirmation pattern, or timeline
   item already expresses the same interaction?
7. What is the honest fallback when a provider, operating system, CLI, remote
   host, permission, or protocol cannot supply the capability?
8. Which existing visual-evaluation scenario gains a material state, or which
   new operator task requires a scenario?

Default placement rules:

- configuration used before every run belongs near the composer;
- infrequent product-wide configuration belongs in Settings;
- project and chat lifecycle belongs in Projects;
- file scope and creation belong in the active project's Files surface;
- run progress, approvals, artifacts, and failures belong in that run's
  conversation;
- code mutations and recovery belong in Time Machine;
- machine or protocol diagnostics stay behind a details disclosure unless they
  are the actionable cause of failure;
- destructive confirmation uses the shared in-product dialog pattern, never a
  browser-native `alert` or `confirm` in new work.

Do not add a panel merely to expose a backend object. First translate the object
into the operator decision it supports.

## Component and token contract

The ownership boundary is intentional:

- `DESIGN.md`: judgment, hierarchy, interaction principles, and review criteria.
- `assets/themes.css`: all shared colors, typography, spacing, shape, focus,
  motion, and surface tokens.
- `ui/src/components`: reusable Svelte structure and component behavior.
- `ui/src/styles`: low-specificity Svelte host and composition rules that only
  consume shared tokens.
- `assets/*.html`: legacy hosts and bridge code during incremental migration.
- `scripts/verify-design-contract.mjs`: deterministic enforcement for new UI.
- `ui/evals/scenarios.json`: stable product scenarios used for visual review.

The current Svelte ownership baseline is:

- `ChatSurface.svelte`: conversation host and composer ownership;
- `Composer.svelte`: drafts, multiline growth, attachments, Supervisor-logo
  configuration, concurrent-prompt delivery, and Stop;
- `SupervisorLogo.svelte`: shared, theme-inheriting rendering of the canonical
  local SVG; the legacy activity bridge uses the same bundled asset;
- `AgentConfiguration.svelte`: anchored four-choice profile popup, reversible
  opening/closing transitions and the main logo's two-part motion;
- `MainAgentSettings.svelte`: personality, shared Codex workspace access, optional skills and
  Apps in Settings, with conversation operations retained as owned shortcut dialogs;
- `ConfigurationSection.svelte`: ordered plain Settings sections, optional
  disclosures and compact graph presentation, preserving native dialogs and
  selected context;
- `ContextWindowMonitor.svelte`: hidden compatibility hooks for native usage;
- `LiveDiffStats.svelte`: owner-scoped live additions/removals shared by main
  and graph composers;
- `ModelPicker.svelte`: accessible provider/model/effort/speed/context choices;
- `WorkspaceExplorer.svelte`: project, chat, file-tree, and confirmation
  structure;
- `ProjectRegistry.svelte`: explicit local/SSH project onboarding, recent and
  pinned project cards, metadata summaries and project-level actions;
- `AgentGraphCard.svelte`: node-bound agent configuration, history,
  approvals, context telemetry, and composer structure;
- `NativeRequests.svelte` and `NativeRequestCard.svelte`: owner-scoped native
  Codex approvals, user questions and stable MCP elicitation forms;
- `SettingsNavigation.svelte`: dedicated categorized Settings navigation;
- `BrowserPanelToggle.svelte`: accessible minimize/restore intent for the native
  Web panel, with Rust-owned state;
- `ui/src/lib/agent-timeline.ts`: stable streamed-message and activity identity.

Rust remains authoritative for provider state, session persistence, token-usage
events, permissions, filesystem changes, process ownership, checkpoints, SSH,
remote desktop, and window control. Svelte renders state and emits semantic user
intent. Legacy JavaScript may bridge those two during migration but must not
become a second state authority.

Do not duplicate a token in Svelte, inspect generated bundle internals, or edit
`ui/dist` manually. During migration, preserve stable IDs and `data-*` hooks so
Rust and the existing bridge continue to address the same interface contract.

## Accessibility and resilience

- Use native controls, landmarks, ordered headings, labels, and keyboard access.
- Meet WCAG AA contrast and never rely on color alone.
- Every focusable control has a visible tokenized focus state.
- Source order remains meaningful when a layout reflows.
- Grid and flex children use `min-width: 0`; scroll belongs to a clearly bounded
  region; application-level overflow is not concealed to hide a layout defect.
- A failed image, provider, remote connection, or optional integration must not
  make the rest of the workspace unusable.
- Respect user data and permission boundaries in screenshots, fixtures, logs,
  and visual evaluation artifacts.

## Inspect and revise

Native conversation flow is shared by main chats and graph cards. A turn plan
appears with the work, after its user input and before the final answer. A
finished turn does not complete pending plan steps: preserve each reported step
state and show an incomplete or stopped plan when appropriate.

Native delegated agents have distinct local conversations. Their relationship
comes from completed native spawn calls or explicit native subagent ancestry,
never titles or directories. Show "Delegated" separately from "Fork", allow
navigation to the supervisor and its children during work, and keep approvals,
Stop and follow-up prompts scoped to the exact child. Native session permissions
and local defaults remain distinct.

Event journals and prompt-delivery timings are runtime observability, not user
settings or conversation content. Keep their bounded owner/thread correlation for
tests and fault isolation, but do not render sequence IDs, transport stages,
protocol method names or timing samples in the main conversation, agent cards or
Settings. Project only an actionable delivery failure through the ordinary chat
status. Diagnostic records never restore or resubmit conversation history and
never contain prompt text, file contents, private reasoning, credentials, commands
or approval answers.

Prepare only the selected existing idle native conversation, on selection or
composer focus. This may resume its session but cannot create a thread or send
input. Coalesce repeated focus events and a user submission that joins preparation;
retain scope validation and cancellation before dispatch. Attachment snapshots
are prepared when selected and shared through drafts and queued submissions.
Diagnostic disk writes and transcript projection must not precede prompt dispatch;
the durable delivery receipt and authority checks still must.

For a material interface change:

1. Verify the operator's task and the dominant object.
2. Inspect initial, active, empty, error, and approval states when affected.
3. Inspect Light and Dark with equivalent hierarchy and contrast.
4. Inspect the relevant Full HD, 2K, and 4K profiles.
5. Check keyboard focus, scrolling, reflow, drag continuity, and reduced motion.
6. Compare against the stable scenarios in `ui/evals/scenarios.json`.
7. Fix the highest-impact systemic defect in tokens or components before adding
   a local override.

Do not present this internal checklist in the product. Deliver the coherent
surface and preserve the evidence needed to reproduce its review.
