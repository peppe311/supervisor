# Project board and preserved cards

The former graph is now a four-column board, based on the user's project →
conversation → supervisor → files mockup. `ProjectBoard.svelte` owns navigation;
`ProjectRegistry.svelte` owns explicit project onboarding. `ProjectChatTree.svelte`
uses native ancestry, never inferred links from titles. Project selection is
presentation/navigation, not an agent instruction or permission grant.
Pinned projects appear first; rows otherwise retain their stable list order.
Selecting a project updates the startup reopen target without moving its row.

## Card preservation

### Board workflow, September 20

Conversation cards now sit directly under their matching chat/Supervisor row.
The legacy host still owns each mounted card and its IPC: slot changes move the
same article without recreating the composer or timeline. Filtering parks a
card without stopping its run. Focus widens the selected conversation lane,
gives that card a landscape reading area and temporarily compacts other cards;
Escape restores their prior minimized state. Pointer and keyboard column
resizers, and a collapsible Files lane, use the same shared spacing and palette.

Open cards, minimized state, selections and lane dimensions remain scoped per
project while the current board instance is alive. They are presentation only:
they are not written to the native session and never become agent instructions.
A first project-chat prompt starts the linked Supervisor only after native
acceptance; association by itself does not start a model turn.

The selected Supervisor exposes one compact **Supervisor settings** trigger for
its **Supervised conversation** association. Its bounded popover opens without
moving the underlying card, uses the shared workspace picker, stays inside the
viewport and closes through Escape or outside interaction. Project chats do not
gain a duplicate settings menu.

New task in a worktree is available from a local chat's actions. Supervisor
checks that the selected project is a Git repository with a commit containing
files before opening the native folder picker. The picker selects the parent
for a new named directory outside the original root; selecting or creating the
parent itself does not put project files in that parent.
Git creates a unique `codex/` branch from HEAD with hooks and checkout filters
disabled. Dirty original files are not carried over or modified. The linked
working copy becomes its own connected project with a named conversation; no
agent starts automatically. Existing destinations are rejected, and worktrees
are user source folders, never automatically cleaned caches. Review and merge
remain explicit Git operations.

The original card is retained in source, not copied as a second implementation:

- `ui/src/components/AgentGraphCard.svelte`: portrait card and composer.
- `AgentGraphHeader.svelte`: animated profile selector and close control.
- `GraphTimeline.svelte`, `GraphMessage.svelte`: shared work timeline/history.
- `assets/agent-graph.html`: existing owner-addressed bridge, approvals,
  attachments, plugins, stream events, artifacts and Time Machine.
- `src/browser/agent_graph.rs`: assignments, saved sessions and runtime owners.

Changing projects hides Supervisor cards belonging to another project without
unmounting their input/history. Project-chat previews can reload from their
original owners and durable drafts; the current board instance keeps each
project's open-card arrangement while the app remains open.
Closing a card remains distinct from stopping an agent.
Project-menu Terminal and Git status actions open Supervisor's detached
terminal while the board is visible; the docked terminal is intentionally
hidden there. Git status on a non-Git folder gives an inline explanation rather
than a disabled button with no feedback.
Existing drafts, profiles, checkpoints, links, native bindings and conversation
UUIDs retain their existing storage. No production chat/account data is copied.
Cards without a connected project remain reachable in **Other saved agents**.

New supervisors get a distinct conversation UUID before any prompt is sent.
The optional `projectChatId` persists a same-project supervision link. Rust
rejects remote, archived, missing or cross-project links. The card keeps two
separate views: **Supervisor** for its own thread and **Observed agent** for the
linked project's complete display-safe timeline. Native prompts, public reasoning
summaries, commands, file changes, tools, requests, final messages and turn state
reuse the same projection as the worker card; private reasoning and opaque tool
payloads are never copied.

When Codex accepts a prompt in a project chat, every valid linked native
Supervisor starts automatically from a bounded snapshot of that worker turn.
The snapshot is untrusted evidence, hidden from the visible user prompt. A
direct Supervisor request can also arm continuous native supervision. Rejected
and cancelled prompts do not start a Supervisor; queued prompts wait for native
acceptance. Turn start, plan updates, completed commands/tools/file
changes, pending decisions and turn completion become coalesced checkpoints;
streaming text deltas do not create review calls. Each automatic checkpoint uses
a read-only Supervisor turn without selected Skills or Apps and a strict
`observe | steer` structured result.
The final worker checkpoint receives one last review, after which the loop waits
without polling until the linked conversation starts another turn.
The same loop owns one compact delivery state rather than a separate workflow:
**Working** while the worker runs, **Checking** while its terminal checkpoint is
reviewed, **Ready** when that review finds no unresolved failure, blocker or
required validation, and **Blocked** when a request or concrete revision still
needs action. The state opens the bounded Supervisor assessment from both the
project-chat and Supervisor context. Those selector rows show the text without
a second collaboration mark or hover tooltip; it never claims that an
unreported command or test ran.

Neither observation nor review grants worker permissions. While the linked
native Codex turn is active, the observed composer and a validated automatic
review can add a text correction through `turn/steer`. The backend revalidates
the saved node/chat pair and `expectedTurnId` after the review; stale and idle
targets fail without starting or queueing a turn, and a manual draft is retained.
Acknowledgements return to the Supervisor card while the accepted correction is
recorded only in the worker's native history. Forks and Supervisor threads retain
independent owners.

## Files and project scope

Local file navigation uses the existing bounded workspace inventory and guarded
editor. SSH metadata lists at most 600 files/directories to depth three within
the explicitly connected directory, omitting heavy/hidden dependency trees and
not following directory symlinks. Remote files open the project's SSH terminal;
this view does not claim to be a remote file editor. No filesystem mapping runs.

The four lanes scroll independently. Narrow windows scroll the lane grid
horizontally, preserving card dimensions. One shared spacing token creates
physical, visually identical resize columns between Projects/Project chats and
Supervisors/Project files, and also controls the inner conversation divider plus
the board's right, bottom and left insets. Compact layouts reduce those distances
together. A dedicated 10px top inset
aligns every lane heading with the upper edge of the return-arrow glyph. The scrolling
grid reserves an equal gutter on its left and right edges, preventing the
vertical scrollbar from making the visible outer spacing asymmetric. Inside
those gutters, the `Projects` title's left edge and the project-file cards'
right edge use the same additional inset. A transparent 40px square overlay
contains only a bare accessible arrow in the upper-left corner; Terminal and browser actions
stay hidden, so the lanes begin at the top without a redundant
Workspace/project-name heading.
Prompt fields inside both project-chat cards and Supervisor cards use the same
borderless filled surface, with focus communicated by the shared focus ring.
Their Send, Stop and Resume control occupies the bottom-right corner with the
same inset from the right and bottom edges, matching the main agent composer.
An opened project chat or Supervisor uses its selector row as the heading of
one continuous rounded card. The row and conversation share the chat-card
surface and meet without a gap, border, shadow, seam or nested frame. The
Supervisor association control sits in its heading beside the actions menu;
spacing applies only between complete card groups in both stacks.
Each project-chat and Supervisor card can be minimized from its header. The card
then keeps its selector width but releases the portrait body from the vertical
stack, leaving title, live collaboration state, Restore and Close visible. This
is presentation state only: it does not stop, close, remount or submit anything,
and restoring reveals the same timeline, draft, attachments and active controls.
The last local project reopens by default without a visible registry toggle.
Both themes use `assets/themes.css`.
Visible scrollbars also use that shared theme contract: track, thumb, hover and
all four directional buttons resolve from Supervisor tokens instead of native or
surface-specific colors. Scrollbars intentionally hidden for compact tab strips
remain hidden.

## Project creation and live task files

`Add project → New project` asks for a name, then opens the native Windows
parent-folder picker. Rust creates one empty child directory exclusively and
connects it through the same registry/inspection flow as an existing folder.
Existing files and folders are never overwritten. Cancellation preserves the
form draft; invalid/reserved names and collisions have explicit errors.

Each project's active task links follow its main chat, fork or supervisor owner.
The files column includes **Task activity**, with **All tasks** and individual
task filters, owner labels and Reading/Editing/Waiting/Done/Stopped/Failed states.
The file tree annotates both individual files and collapsed ancestor folders.
Parallel tools and chats retain independent ownership, including shared files.
The same small interlocking collaboration mark now propagates from this
structured live state to the owning project card, open conversation card, Task
activity row and exact file row. Project-chat and Supervisor selector rows use
the explicit Working/Checking/Ready/Blocked text instead. The mark disappears as
each operation ceases to be live; collapsed directories retain their textual
active count without pretending that the directory itself is a reported target.
The mark is monochrome, labelled for assistive technology and stops pulsing when
reduced motion is requested.

Project-chat actions expose Rename and Delete. Delete uses the existing guarded
local-history operation, requires explicit confirmation, is unavailable while
the chat is mutation-locked and leaves project files and checkpoints unchanged.

This is a bounded view of the latest available turn per owner, not a disk watcher
or a new agent runtime. Native Codex uses structured `fileChange` targets and
`commandExecution.commandActions` reads. Claude and ACP use explicit file paths
reported by read/edit tools. Shell commands, searches, prose and model reasoning
are not parsed to invent paths. Tools that do not supply paths (including opaque
shell/MCP tools) cannot be attributed; the UI explains missing file activity.
Only targets inside the registered root are displayed, with local paths matched
case-insensitively and SSH roots kept case-sensitive. At most 256 files per native
turn are displayed, and truncation is visible. Closing/interrupting or losing a
runtime connection clears active markers while confirmed completed work remains.

## Work summary, comparison and handoff

The actions menu of a project chat or Supervisor now provides three compact
flows. Work summary reads the current/latest recorded turn and shows its
request, result, explicit command outcomes, file changes, recorded diffs and
public activity. It does not generate a model summary, scan the disk or infer
that tests passed. Missing details can load through the existing one-turn
native history reader. Evidence is bounded in size; omissions are visible.

Compare attempts displays two same-project reports before sending anything.
Ask Supervisor invokes the existing native read-only reviewer in a selected
idle Codex Supervisor. The original attempts and their revisions are checked
again before dispatch. The reviewer may choose either result, a tie or
insufficient evidence. No patch is selected or applied automatically; the
answer and its reasoning summary remain in the reviewing conversation.

Hand off creates a native fork, preserving the provider-owned conversation
history and attachments. The requested next instruction is saved as a draft
under the new owner and survives restart. After the native acknowledgement,
Open new agent stays inside the board; the user chooses its model and sends the
draft. Forks preserve context, not independent filesystem state. Other provider
transcripts are never fed back as native Codex history. A rejected or uncertain
fork retains the existing native recovery semantics, without replaying it.

These operations add no background process, new database or autonomous loop.

## Verification history

Validation on 2026-09-22: the complete workspace verification passed, including
the command-evidence, comparison eligibility and durable handoff-draft tests.
The canonical package was rebuilt with `scripts/update-supervisor.ps1` and
passed its hidden WebView startup and release-manifest checks. The new
synthetic Work results flow covers both themes, report failures and missing
outcomes, the shared selectors, exact owners/revisions, duplicate clicks,
rejected comparison, native fork acknowledgement and late replies after close.
Dialog bounds were checked alongside the board's supported layout widths.
These checks do not use Computer Use, production chat data or account inference;
they do not claim a live model comparison or a production-account fork.

After the selective rollback of board points 3–7,
`scripts/verify-workspace.ps1` passed 641 application tests and 182 native
runtime tests, with the repository's 21 explicit runtime exclusions unchanged.
The frontend suites, Svelte/type/design checks, formatting, Clippy, contract
checks, build/update probes and dependency-audit policy also passed.

The retained September 20 workflow covers Focus, minimizing, direct inline card
placement, keyboard and pointer resizing, Files collapse, the compact
Supervisor association selector and worktree IPC. The Git integration test
creates a disposable worktree and confirms that dirty original files and
pre-existing destinations are preserved. These fixtures use no production
project, account, model inference or Computer Use.

Project creation and task activity retain focused Rust coverage for exclusive
creation, path boundaries, structured provider targets, overlapping operations,
approval waits, interruption and disconnect. The hidden WebView exercises
creation/error/cancel/success, project/task links, task filtering and
stop/completion in both themes at 1024, 1920, 2560 and 3840 widths. These
fixtures use no live inference, Computer Use, production project creation or
account writes.

The updated canonical package passed its staged hidden startup and manifest
checks. After the independent Windows restart, a separate read-only process
verified the journal's physical LocalAppData path and a new accepted native
`thread/read` response (sequence 6340). The temporary check and scheduled task
were removed; account and chat stores were not modified by the check.

Pure tests cover project/owner isolation and safe file-tree construction. Rust
tests cover association validation, persistence and bounded remote metadata.
The disposable `--check-ui-startup` WebView tests actual project/chat/file/card
navigation in Light/Dark and supported layout widths, including retained drafts
and authenticated/unauthenticated send controls. It uses fixtures without account
inference or production data. No Computer Use is required.

Validation on 2026-09-18: `scripts/verify-workspace.ps1` passed, including Svelte
type/design checks, frontend state/ownership suites, 620 application tests,
181 Codex runtime tests, the five native-probe oracle tests, Clippy and the
dependency audit under the repository's existing advisory policy. The normal
integration exclusions remain (4 application and 21 runtime tests). The hidden
WebView fixture passed project switching, preserved drafts, fork hierarchy,
file navigation and send/authentication controls in both themes; column layout
was exercised at 1024, 1920, 2560 and 3840 pixels. Real SSH credentials and
live inference were not used by these fixtures.

The canonical package was updated with `scripts/update-supervisor.ps1`; its
staged hidden startup check and release manifest validation passed. The app
restarted through Windows Task Scheduler. An independent read-only process
verified the physical LocalAppData journal path and new accepted native
`thread/read` responses after restart (last observed read sequence 6294).
