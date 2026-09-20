# Knowledge Graph Agent Windows V1

Status: graph-window workflow implemented. Codex-specific text below is bounded
by the 2026-09-06 native App Server reset: native Codex conversations work on
local-directory nodes, while custom graph coordination and Central Agent tool
injection are deferred. See `CODEX_APP_SERVER.md` for the current boundary.

## Outcome

Replace the fixed Knowledge Graph inspector and single selected-node console
with independent, node-anchored agent windows. A graph point opens its window
from the context menu. The window follows its point during drag, pan, zoom, and
force-layout movement while retaining an optional user-adjusted offset.

## Delivered milestones

1. **Graph workspace** — removed the permanent inspector column and restored
   the full graph width.
2. **Window manager** — added multiple persistent open windows, node-to-window
   connector curves, window drag offsets, auto-open for active runs, and local
   UI preference persistence.
3. **Provider profiles** — each graph binding and turn records provider, model,
   effort, and speed. Codex uses the official native App Server; Claude Code,
   Cursor, GitHub Copilot, Google AI, and OpenCode Go retain their existing
   official local adapters and model catalogs.
4. **Conversation continuity** — changing provider between turns keeps the same
   Central Agent node history. Live reasoning, messages, actions, approvals,
   artifacts, checkpoints, stop, and resume remain visible in the window. The
   WebView receives the 20 most recent turns; the complete history is kept in a
   dedicated local file and remains resumable.
5. **Agent collaboration** — users explicitly connect two or more assigned
   windows. Links persist locally for graph presentation. A later turn receives
   only a bounded recent excerpt from directly linked peers as untrusted
   collaboration context; it must still verify that context and every action
   remains subject to Central Agent permissions.
6. **Concurrent runtimes** — graph jobs have provider-specific job registries,
   so one popup does not replace another popup's active handle or stream.

## Safety rules

- Right-clicking or connecting nodes never starts an agent automatically.
- Linking shares conversation context, not credentials or hidden provider
  state, and does not create recursive autonomous agent-to-agent loops.
- Provider changes are allowed between turns; active turns must be stopped
  before changing their assignment.
- Existing permission modes, SSH scoping, Time Machine checkpoints, and the
  emergency stop remain authoritative.

## Verification

- Rust unit and integration tests.
- Clippy with warnings denied.
- Static JavaScript syntax validation for the embedded graph surface.
- No visual automation, per the project's current manual visual-test policy.
