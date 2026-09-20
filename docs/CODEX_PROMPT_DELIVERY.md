# Prompt delivery — 2026-09-13

Supervisor keeps its existing persistent Codex App Server connection and now
reduces work between Submit and `turn/start`/`turn/steer`. Main chat and graph
composers use the same native path. Model, effort, speed and permissions retain
their existing selection and persistence rules.

## Changes

1. **Measure delivery.** Event diagnostics includes up to eight samples for the
   exact owner/thread, drawn from a 64-sample memory-only ring. Each sample records
   the estimated UI click-to-host delay, then monotonic elapsed preparation, pipe
   write, native ACK, native user-item observation and first public text/summary
   update. Queue wait is included. Missing stages remain unknown. Correlation uses
   the client message ID and native thread/turn IDs, never prompt matching. Receive
   timestamps prevent a delayed UI callback from misattributing old text to a new
   follow-up. A steer update still belongs to the ongoing turn, not proof of the
   model having processed the added instruction. No prompt/file contents are kept
   in the timing ring or sent as telemetry.
2. **Prepare the selected existing session.** Main chat selection/startup and
   composer focus select a native session for best-effort preparation. Graph
   composer focus does the same for that card. An eligible existing idle binding
   receives one `thread/resume` with `excludeTurns`; nothing is submitted and no
   empty thread is created. Repeated focus coalesces; an actual submission can
   join the in-flight resume. Loaded, active, archived, deleted, unused, uncertain,
   invalid or remote targets are skipped. A failed automatic attempt does not loop
   and does not block an explicit user retry. Reconnection permits preparation
   again. Cancellation and frozen destination/access checks still run before send.
3. **Prepare attachments on selection.** Validated snapshots keep shared immutable
   file contents and cached native parts, including binary encoding. Cloning a
   draft or queue entry shares those snapshots. The cache is excluded from saved
   queue JSON and rebuilt once after restoration; changing the original file cannot
   change the captured input. Existing content, ownership and aggregate size checks
   remain, including JSON escaping. Counting wire size no longer allocates another
   complete JSON byte buffer.
4. **Remove diagnostic I/O from dispatch.** One bounded 256-entry background queue
   writes the metadata journal, retaining its two 1,024-entry segments. Disk failure
   or backpressure is reported in diagnostics while in-memory events remain usable.
   The native delivery receipt is still saved before any prompt bytes are sent.
   After opening a session, a waiting prompt is dispatched before history pagination,
   delegate projection and UI refresh. These displays remain provider-owned and are
   never fed back as reconstructed model context.

The lifecycle follows the [official App Server documentation](https://learn.chatgpt.com/docs/app-server):
`thread/read` inspects history; `thread/resume` loads an existing conversation into
the persistent session; `turn/start` submits input. Preparing a session is not an
inference request and does not predict remote model response latency.

## Local measurement

An opt-in, account-free test compares the previous attachment assembly path
(fresh binary encoding and allocating a serialized size-check buffer) with the
prepared path. For a synthetic 4,000,000-byte WAV, 15 alternating samples in the
unoptimized Rust test profile gave median **109.23 ms before / 96.30 ms after**.
Both paths produced identical native input. This measures only assembly at send,
excluding file selection, IPC, durable receipts, transport and inference; it is
not a production or end-to-end speed claim. Session preparation separately removes
one resume round trip from a ready conversation's submission path.

The benchmark is `measure_prepared_attachment_delivery` in the attachment tests.
Run it explicitly with `--ignored --nocapture` inside the repository build-storage
scope. It uses temporary synthetic data and does not access the user's account.

## Verification coverage

Tests cover message/owner/turn correlation, ACK before and after the native user
item, stale UI delivery, bounded samples, coalesced preparation, explicit retry,
reconnection, skipped unsafe targets, shared snapshots, restored snapshots,
aggregate escaped size limits, journal ordering/rotation and writer backpressure.
Existing runtime tests retain the cancellation, scope and delivery-recovery checks.

The hidden WebView probe exercises the real Svelte diagnostics in Light/Dark at
280, 480, 1920, 2560 and 3840 px, including empty/missing values, bounded lists,
keyboard focus and overflow. The graph host probe checks focus preparation and
preservation of click timing through its real submission bridge. No Computer Use
or account inference is required by these checks.

Final verification: 602 application tests passed (four opt-in tests skipped),
173 native runtime tests passed, and 25 focused UI tests passed. The local
attachment benchmark was run explicitly. Type/design checks, embedded script
checks, both pinned protocol contracts and Clippy with warnings denied passed.
Three older UI assertions were aligned with the existing settings wording,
Svelte profile animation/ownership and Windows line endings; no unrelated
product behavior changed to satisfy those checks.

The canonical updater passed hidden WebView startup and package checks, then
an independent Windows launcher verified the actual application profile and
all five automatic history reads (journal sequences 803, 805, 807, 809, 811).
It observed no new model turns. Real prompt timings are populated when the user
sends input; no account inference was performed solely to obtain a benchmark.
Only `outputs/Supervisor/` remains in the delivery directory.

Delivered executable SHA-256:
`3710c090fcce45c02fe450702d7760aecec8a02403f5f9bfdd6314330de9c86e`.
