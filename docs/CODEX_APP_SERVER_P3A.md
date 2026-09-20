# P3-A: Git metadata, backend sections and protected revert

Implemented in source on **2026-09-09**, against the pinned official **Codex CLI
0.153.4** contract. This is a backend milestone, not a new visual surface or a
replacement executable. The existing P2 source-preview package is unchanged.

## Delivered scope

| Capability | Backend result | Product exposure |
| --- | --- | --- |
| Git metadata | Read metadata; capture/revalidate the bound repository; explicitly update and verify SHA, branch and sanitized origin | Rust API only; no automatic synchronization |
| Sections | Bounded list, create, rename, delete; move/remove an owned thread, optionally before a verified owned anchor | Rust API only; no sidebar/IPC controls |
| Protected revert | Prepare/confirm, durable write-ahead receipt, exact-prefix reconciliation, stale-read barriers and native lifecycle recovery | Rust API only; no destructive UI action |
| `thread/items/list` | Constructor, bounded decoder and contract/regression coverage | **Off**, rejected at the transport boundary even for direct requests |

`experimentalApi` remains **false**. No `fs/*`, realtime, remote environment,
collaboration, pinning, deprecated rollback or custom conversation engine was
introduced. There are no new Svelte surfaces or visual tokens in this increment.

## Git implementation

`thread_metadata::RepositorySnapshot` captures an existing absolute worktree
directory, its canonical root, HEAD (including unborn/detached cases), branch and
local `remote.origin.url`. Reads are bounded to five seconds and 8 KiB per Git
command. The executable is resolved from absolute PATH entries before switching
directories; inherited `GIT_*` overrides, global/system Git configuration,
interactive credential prompts and fsmonitor are excluded from these reads.

Only read-only Git commands are used. There is no fetch, commit, checkout,
reset, index update or file restoration. Origin URLs lose user/password, query
and fragment; local/unsupported origins are omitted. This sanitizes conventional
URL credential locations, not arbitrary secrets embedded in a repository path.

`Conversations::prepare_git_metadata` binds a confirmation to the current local
owner, native thread, canonical directory, connection, request generation and
observed revision. `confirm_git_metadata` rechecks that scope and recaptures the
repository before producing `thread/metadata/update`. The high-level operation
sends all three fields deliberately: absent local SHA/branch/origin becomes an
explicit native clear. The existing low-level `GitInfoUpdate` still distinguishes
omitted fields from null and replacement values.

The response must identify the same native thread, directory and requested Git
values. Git metadata, section data and raw cwd stay Rust-side: the display mirror
does not serialize these new fields into the WebView or the binding store.

## Section implementation

`thread_sections::Sections` owns the server inventory and its revision. Pages
are limited to 64 entries, 128 pages, 4,096 total sections and 16 KiB cursors.
Incomplete pages, repeated identities/cursors and stale/foreign responses never
publish a partial inventory. Names are bounded to 160 bytes of visible text.

Create/rename/delete require a fresh, non-deserializable, single-use confirmation.
A successful mutation schedules only an inventory **read**, never a replay.
Malformed or uncertain acknowledgements invalidate the inventory; the caller
must refresh and review native state before another action. Rename omits
`appearance`, preserving native icon/color. Appearance editing is outside P3-A.

Thread moves require the same owned idle-thread scope used for metadata. A
destination must exist in the current inventory. An optional `before_owner` must
resolve to a different locally owned, observed, nonarchived thread whose current
native section matches the destination. A null destination removes membership.
The move ACK is not a metadata snapshot: the host follows it with `thread/read`,
then existing bounded history hydration where applicable.

A fresh native 0.153.4 profile already contains a default **Pinned** section.
Tests preserve that baseline and mutate only their own newly returned section
ID. A section named Pinned is not an implementation of the absent `isPinned` API.

## Protected revert implementation

Official semantics: `thread/revert {threadId, beforeTurnId}` replaces a
**paginated** thread's durable conversation history with the prefix **before**
the selected turn. That turn and every later turn are excluded. **Local files
and Git state are not reverted.** There is no rollback substitution, transcript
injection or hidden backup/fork.

1. Read metadata and the complete bounded `thread/turns/list` sequence. Partial
   reads, live/unknown turns, concurrent mirror changes, archived/deleted/unloaded
   threads, wrong directories and uncertain operations cannot authorize revert.
2. `prepare_revert` freezes owner/thread, cutoff, retained IDs, removed count,
   connection and observed revision. The trusted host must present those
   consequences and obtain explicit confirmation. The confirmation cannot be
   deserialized from a website payload, cloned or reused on another book instance.
3. `confirm_revert` revalidates it and creates an ID-only `Saved.reverts` receipt.
   **Persist `saved()` atomically before sending**. The host dispatcher enforces
   that save for `thread/revert`, as for fork/compact. Failed persistence means
   local rejection without sending.
4. Starting the operation, native `thread/reverted` notifications and successful
   ACKs establish history-revision barriers and clear stale display turns. Older
   read responses/pages cannot reintroduce removed turns. Host page sequences
   have unique identities across cancellation/restart, even with identical
   owner/thread/cursor.
5. The ACK must match thread/directory, paginated mode, empty inline turns and
   valid nullable hydration cursors. Retained history uses the existing ascending
   `thread/turns/list` path, never item listing. An ACK does **not** clear the
   receipt.
6. Only a complete current read with exactly the expected retained IDs reconciles
   automatically. Persist before emitting success. Unknown delivery, malformed
   ACK and internal RPC error preserve the receipt across restart and block
   further mutations. A differing read proves neither success nor failure;
   explicit `resolve_revert_after_user_review` releases the exact reviewed warning
   without replaying anything.
7. Native 0.153.4 releases the previous loaded session during revert. Closure may
   precede the ACK, and metadata can say idle while the subscription remains
   unloaded. Reconciliation does not invent a loaded session. Explicit
   `thread/resume` reopens the same retained history before a new turn.

The server exposes no expected-history-revision/CAS parameter on revert. These
guards fence observed client state; they cannot prevent another client's write
between confirmation and execution. This is not a cross-client lock, reversible
filesystem operation or full transcript backup.

## Verification

Run the isolated native probe explicitly:

```powershell
cargo test --locked -p central-agent-codex-runtime native_p3_git_sections_and_protected_revert -- --ignored --nocapture
```

It uses new temporary `CODEX_HOME` and `CODEX_SQLITE_HOME`, a temporary Git
worktree and a loopback Responses fixture. No personal account, history,
configuration or repository is mutated. It verifies:

- native Git update/read-back and section CRUD/move/remove with default inventory
  preserved;
- three real persisted native turns reduced to the exact one-turn prefix;
- native closure/revert notifications before the ACK and explicit subsequent
  resume;
- restoration of the pre-ACK receipt from disk, reconciled by read only;
- untouched local file and Git branch, no tools and exactly three loopback model
  requests;
- rejection of item listing before transport dispatch.

Deterministic regressions cover stale/foreign confirmations, unborn/detached
Git, URL sanitation, changed repositories, pagination limits/loops, first-turn
empty-prefix revert, wrong/partial responses, internal RPC failure, old binding
files and old-history races. Contract verification covers **121 request samples**
and **10 additional P3-A response/notification samples** against the selected
schema.

Full-workspace results are in `CODEX_APP_SERVER_ACCEPTANCE.md`; the local gate
log is `outputs/p3-a-verification-2026-09-09.log` and the explicit native evidence
is `outputs/p3-a-native-2026-09-09.log`. The native probe is ignored by
ordinary tests and was run explicitly. No new Computer Use or independent
Astra/visual audit is claimed for this backend increment.

## Remaining work outside P3-A

Future UI work needs metadata presentation, section controls/order, destructive
revert preview/confirmation and recovery states, plus inspection in both themes
and supported layouts under `DESIGN.md`. No such surface is silently enabled.
Item pagination requires a separate decision, privacy-safe projection and
item-level hydration design. Earlier P2 human validation and release/signing gates
are unchanged.

## Sources

- [Official rolling App Server guide](https://learn.chatgpt.com/docs/app-server).
- [Pinned revert semantics](../protocol/app-server/0.153.4/typescript/v2/ThreadRevertParams.ts)
  and [response hydration contract](../protocol/app-server/0.153.4/typescript/v2/ThreadRevertResponse.ts).
- [Pinned section move contract](../protocol/app-server/0.153.4/typescript/v2/ThreadSectionMoveParams.ts)
  and [Git update contract](../protocol/app-server/0.153.4/typescript/v2/ThreadMetadataGitInfoUpdateParams.ts).

Rolling documentation is not a reason to change existing stable turn hydration
or expose fields absent from the selected version. The generated 0.153.4 types
and explicit native probe are the implementation boundary for this milestone.
