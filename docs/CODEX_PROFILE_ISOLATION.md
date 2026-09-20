# Supervisor's separate Codex profile

Supervisor launches its native App Server with `<Supervisor data>/codex` as
`CODEX_HOME`. On a normal Windows installation this is
`%LOCALAPPDATA%\CentralAgent\CentralAgent\data\codex`.
SQLite storage is pinned both through `CODEX_SQLITE_HOME` and the higher-priority
`sqlite_home` process override. ChatGPT account storage is pinned to native
`cli_auth_credentials_store="file"`. The parent environment is unchanged.

The official Codex app keeps its existing profile. Supervisor requires its own
one-time native sign-in; it can use the same ChatGPT account. No credential file
or shared database is imported. Project files and project
configuration still belong to their existing project directories. MCP service
authorization follows the native provider's own credential-store policy.

The optional official Computer Use attachment is the narrow exception for
runtime configuration: it mirrors the installed plugin's `node_repl` server
table and lifecycle hooks into the separate profile at connection time. The
official plugin's skill remains at its original read-only path. No account
credentials or chat storage are shared; details and limits are recorded in
`docs/CODEX_COMPUTER_USE_FEASIBILITY.md`.

This follows the official [environment-variable reference](https://learn.chatgpt.com/docs/config-file/environment-variables)
and [native authentication documentation](https://learn.chatgpt.com/docs/auth).
The checked-in `0.155.1` protocol is the current authoritative contract for API
calls; earlier exact versions remain allowlisted only after their own generated
contracts passed the same verifier.

## Automatic chat loading

Saved Supervisor bindings start the isolated runtime once on app launch, even
when its ChatGPT account is signed out. After native initialization the selected
chat is read first, followed by other saved chat and graph bindings, one at a
time. Metadata acknowledgement is not completion: paginated messages must finish
loading before the chat is counted as loaded. Existing archived state and missing
links are retained. Reads do not resume threads, open OAuth or submit prompts.

AI Settings provides **Reload chats** using the same reader. On a connected
runtime it refreshes display history without reconnecting or repeating migration.
The control reports progress, empty history, completed reads and partial failures.
Active work is left alone and captured bindings are rechecked before dispatch.
Uncertain delivery receipts may be reconciled by native history; input is never
resent. Existing messages and drafts are not cleared on a failed reload.

The saved account reconnection preference remains distinct from local history
loading. Signing out never deletes history or automatically opens another login.

## First connection after upgrading

The host holds the exclusive Supervisor binding lock before creating the profile.
It selects only IDs already linked to Supervisor, including known fork ancestry,
pending fork sources and native delegates. Deleted bindings are excluded. Missing
histories retain their original bindings and are listed in the migration record.

The transfer copies native rollout files byte-for-byte into a temporary directory,
validates their native IDs and compares SHA-256 checksums before publication. It
streams large histories, rejects filesystem redirection and concurrent changes,
and preserves archived paths. Originals remain available in the old profile;
they are not automatically archived or deleted from the official app.

The native runtime then reopens only copied IDs with read-only access, from a
neutral directory, to rebuild paginated history. A rollout can be recognized by
`thread/read` before its turn index has been rebuilt; metadata discovery alone
is not successful history recovery. No prompt, tool approval or replay is sent.
Archived histories are restored to their archived state afterwards.

Titles and goals are read from the original profile using public metadata APIs.
The source client preserves its original storage configuration and is used only
for `thread/read` and `thread/goal/get`. Title/goal writes target the new profile.
The complete original goal snapshot is retained in the migration record because
the public goal-write API cannot restore historical counters or timestamps.
Completed goals stay completed. Unfinished goals are paused with only their
remaining token allowance; exhausted goals remain in the migration record and
are not recreated with a fresh allowance. Nothing automatically resumes work.

`supervisor-profile.json` records the source, file hashes, missing IDs, original
public metadata and completed migration stages. A completed profile is
authoritative: later connections do not reimport deleted history, revive a
signed-out account or overwrite newer titles/goals. A failure is visible and
never starts product work against the old profile. Existing local bindings,
drafts, project ownership and fork/delegation links are retained.

## Verification

Coverage includes first-use migration, missing/deleted bindings, unopened native
children, archived paths, corrupt/duplicate inputs, overlapping profiles,
idempotence and paused goal/budget handling. A pinned native test uses a loopback
Responses fixture to verify persistence after restart, transfer rehydration,
archived recovery and rejection of the same thread ID in another profile. It
also tests conflicting SQLite environment/configuration and file-backed auth.

Live migration audits belong in the private application data directory. Public
documentation records the migration contract, not account-specific results.

Explicit disposable acceptance runs can still provide their own `CODEX_HOME`
fixture. Production app launches always select the Supervisor-owned profile.

## Importing selected conversations

An explicitly requested import copies only selected histories and preserves the
originals. It is a snapshot, not continuous synchronization. The native reader
validates complete paginated item content before attaching the imported chats.
Existing credentials, unrelated conversations and migration markers are retained.
No model turn is submitted as part of an import or history reload.

Windows maintenance must verify resolved physical paths in an independently
launched process. Codex-launched processes can inherit package file redirection
even when a package-identity check reports no package. Restart the canonical
Supervisor package in its original Windows execution context and confirm its
own history-read responses in the actual application data directory.

## Activity presentation

Completed turns keep final answers outside the expandable work-duration section.
Opening a section reads that turn's full public activity on demand, preserving
provider item order, commentary, tool groups and in-turn question replies.
Private reasoning is excluded from the display mirror. Late responses cannot
revive removed chats, clear drafts or submit a new model turn.

Tests use synthetic messages, paths and identifiers. Conversation names, native
thread/turn IDs, transcript excerpts, per-account usage, history sizes and local
audit journals must not be copied into public documentation or test fixtures.
