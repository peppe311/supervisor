# Central Agent Time Machine V1

## Outcome

Every local-workspace agent run receives a durable checkpoint immediately before its first action that can change workspace files. Pure inspection runs skip checkpoint creation. When a protected run ends, Central Agent presents the workspace changes observed since that checkpoint, their folder hierarchy, language-aware local icons, aggregate line statistics, a per-file unified diff, and safe restore controls.

## Runtime design

1. Let the provider reason and use recognized read-only tools without a snapshot. Capture the baseline asynchronously and hold the first potentially mutating action until it is durable; ambiguous shell commands remain protected.
2. Store file contents in a local content-addressed blob store and persist one manifest per run. Identical content is written only once.
3. Capture the post-run tree and derive changed-file records plus additions/deletions locally.
4. Attach the resulting checkpoint summary to the final chat message and, for node agents, to the durable Knowledge Graph turn.
5. Keep generated dependency/build directories out of snapshots (`.git`, `node_modules`, `target`, `outputs-*`, and common build/cache directories). Record each unique skipped or unreadable path once; recovery notes are not counted as skipped paths.
6. Keep the run in a visible, stoppable finalizing phase until its provider has exited and every background command owned by the run has terminated and flushed its bounded output. Managed commands stop after 30 minutes or 64 MiB of combined output; **Stop** can cancel them earlier. Long-lived preview servers should run in the interactive shell instead.
7. On application shutdown, never capture a post-run tree while writers may still be active. Leave the durable active manifest unfinished so startup recovery can complete it safely.

## Restore safety

- Paths remain relative to the connected workspace and may not traverse links or escape the root.
- Before restoring, compare each current file with the checkpoint's post-run hash.
- Treat content already equal to the pre-run state as already restored.
- Abort the complete restore before writing anything when a manual-edit conflict is detected.
- Permit restoring the whole checkpoint or one file. Created files are removed; modified/deleted files are restored from the local blob store.

## UI

- Render a Changed Files card in the chat and in Knowledge Graph agent history.
- Group files into collapsible folders and aggregate `+added / -deleted` counts recursively.
- Resolve icons locally from exact filenames and extensions (Cargo, Rust, C/C++, TypeScript, JavaScript, Docker, package.json, and common formats).
- Open a full diff viewer from the card or an individual file row.
- Expose `Restore file` and `Restore checkpoint` with an explicit confirmation.

## Verification

- Unit-test added/modified/deleted detection, line statistics, icon resolution, conflict rejection, single-file restore, complete restore, and path containment.
- Run formatting, the Rust test suite, and Clippy.

## Deferred

Remote SSH/VPS snapshots, full operating-system rollback, attribution between agents concurrently editing the same local workspace, and configurable ignore rules are outside V1.
