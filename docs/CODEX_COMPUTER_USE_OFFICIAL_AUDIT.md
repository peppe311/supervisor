# Official Windows Computer Use: audit and proposed changes

Date: 2026-09-16. Status: audit completed; baseline, connection recovery and measurement changes implemented.

This audit follows the [speed comparison and component measurements](CODEX_COMPUTER_USE_SPEED_TEST.md).
It inspects installed OpenAI documentation, public SDK signatures, readable
JavaScript wrappers and the desktop application's JavaScript broker, plus
current official documentation. The initial audit performed no desktop actions,
new model turns, plugin updates, configuration changes or native executable inspection.
The subsequently authorized implementation and validation are recorded at the end.
No vendor source is copied into this repository.

## Conclusion

The former Supervisor companion did not demonstrate a speed benefit.
The official skill is now the default. Measure actual redundant work, and
validate any subsequent change under controlled conditions. The expensive
operation measured so far is observation through the official Windows stack,
not input injection. No documented capture-speed control was found.

Improve connection diagnostics separately: a discoverable JavaScript tool is
not proof that the desktop service behind it is reachable. Do not describe
connection recovery or companion removal as a measured capture acceleration.

## Installed components

| Component | Observed version | Meaning |
| --- | --- | --- |
| Windows desktop package | `26.908.9136.0` | Installed `OpenAI.Codex` Windows package |
| Desktop application bundle | `26.908.70816` | Version in the installed application's `package.json` |
| Official Computer Use plugin | `26.908.70816` | Only version present in the local bundled plugin cache |
| JavaScript SDK `@oai/sky` | `0.6.32` | SDK under the official `cua_node` runtime |
| App Server in the recorded benchmark | `0.154.0-alpha.6.2` | Native conversation runtime, distinct from the desktop service |

Different package and application version formats are not, by themselves,
evidence of a mismatch. The plugin manifest identifies OpenAI as author and
uses a proprietary license. Changing its installed files would create an
unsupported local variant and can be overwritten by the official updater.

The September 11 desktop release describes Windows Appshots and other changes;
it does not establish a fix for the 3–5 second observations measured here.
The inspected release notes do not identify such a fix. Neither a single local
cache version nor a CLI release proves that no newer desktop component exists.
Use the official desktop update mechanism when an update is available, then
measure again. [Official changelog](https://learn.chatgpt.com/docs/changelog).

## Execution path and ownership

```text
Supervisor conversation
  -> Codex App Server
  -> official node_repl tool
  -> public @oai/sky methods
  -> official desktop app's native-pipe broker
  -> official Windows Computer Use service
  -> requested window observation or input
```

Supervisor attaches the installed official skill, mirrors the verified
`node_repl` configuration into its separate Codex profile, and retains the
official Stop/Interrupt/SubagentStop lifecycle hooks. The retired companion
added instructions; it did not replace the Windows execution engine.

The SDK forwards `get_window_state` to the service, normalizes its result, and
awaits automatic screenshot display when images are returned. Timing the public
method therefore includes service/bridge work and image delivery. It is not
an isolated Windows.Graphics.Capture measurement.

The inspected desktop broker forwards calls to its official transport and
rejects an overlapping ordinary request while another is active. Lifecycle
requests have separate handling. This is not a public queue and does not
establish exclusive ownership for an entire multi-step agent task. Parallel
desktop agents are not a supported performance optimization; interleaving
can invalidate an observation between a decision and its action.

On Windows, the documented workflow uses the active desktop. Background use
while the user independently operates the same Windows session is not offered.
[Official Computer Use documentation](https://learn.chatgpt.com/docs/computer-use).

## What the supported controls actually do

| Control | Supported purpose | Performance implication |
| --- | --- | --- |
| `include_screenshot` | Capture/display a window image; default `true` | Request when visual verification is needed; disabling it did not remove the observed delay in our text-only trials |
| `include_text` | Request accessibility state; default `false` | Useful when the target supplies meaningful text/indexes; Calculator returned `null` even when requested |
| Existing `node_repl` state | Retain initialization and valid returned window objects | Avoid repeating setup; observations still expire when state changes |
| `activate_window` | Explicit activation or recovery when necessary | Input methods already activate their target; unconditional extra activation is usually redundant |
| Window-specific keyboard/element actions | Operate currently observed controls | Can reduce navigation steps when the app supports them; success must be verified |
| Saved app approvals | Remember a user's app access decision | Permission handling, not a capture-speed setting; no permission relaxation is proposed |

The public Windows `GetWindowStateInput` has the target window and the two
observation flags. It has no exposed frame interval, resolution, settle delay
or capture timeout parameter. Both flags cannot be false. Windows client
creation does not apply the separate Linux client's action-settling options.

The installed official guidance already covers persistent setup, selective
image/text capture, automatic image display, keyboard navigation, automatic
input activation and the observe/one-action/refresh cycle. These substantially
overlap the companion. Additional instruction text is not evidence of a
different or faster execution path, nor is its token overhead proven to explain
the full A/B timing difference.

## Timeouts are not speed controls

The inspected JavaScript SDK/transport uses default request deadlines of
10 seconds, or 15 seconds for application launch. The desktop broker also has
a separate approval deadline. These bound outstanding operations; they do not
delay every successful operation until the deadline.

The mirrored native-pipe connection timeout and MCP startup timeout are also
connection/startup controls. Lowering any of these could cause earlier failure
without making a successful window observation faster.

No unconditional 3- or 5-second delay was found in the inspected SDK state
method, broker forwarding path or JavaScript transport. The native capture
implementation was not inspected. A native observation timeout, frame wait,
accessibility behavior, or another service stage remains a hypothesis.
There is no basis for a specific internal patch or timeout reduction.

## Evidence from the completed tests

| Measurement | Result | Limit |
| --- | --- | --- |
| Verified key input | Mean 40 ms | Three inputs in one application |
| Verified click | Mean 80 ms | Three inputs in one application |
| Image refresh after input | Mean 3.05 s | Six observations; includes SDK delivery |
| Idle image observations | Approximately 3.08–5.08 s | Three samples |
| Idle text-only observations | Approximately 3.03 s, text `null` | Three samples; not a usable screenshot replacement here |
| Official baseline task | Mean 81.693 s | Two complete task runs |
| Same task with companion | Mean 90.812 s | Two complete task runs; 11.16% slower in this sample |
| Tool calls/failures | 21 calls and one failed call per variant | Aggregate counts include setup and other tools, not only observations |

The baseline averaged 54.851 seconds outside measured tools. That remainder
includes model, service and transport waits; it is not pure reasoning time.
Reducing observations cannot remove that entire remainder. The isolated
15.606-second call from the A/B comparison was not reproduced or explained.

See the [measurement report](CODEX_COMPUTER_USE_SPEED_TEST.md) for conditions,
sample values, recovered failures and the keypad input with no visible effect.

## Proposed work, including the three earlier points

### 1. Restore the official baseline

Stop automatically adding `supervisor-computer-use` to main-chat, graph-card,
queued and steered Computer Use inputs. Ensure the Supervisor-owned companion
does not remain implicitly discoverable as an active skill after migration.
Remove only managed content after the established physical-path checks;
preserve user-owned files, unrelated skill settings and the official plugin.
Keep a baseline/variant comparison possible in an explicit test fixture.

Check manual and automatic official skill selection, native disablement,
project ownership and queue references. The desktop frame, approvals and
official interruption hooks must continue to work. This removes an unproven
addition; it does not claim a specific time saving.

### 2. Identify and correct confirmed redundant work

Extend the opt-in measurement probe with timings around awaited public `sky`
methods, distinguishing setup, input and observation. Record only bounded
operation metadata, observation mode, duration, whether useful text/image
state was returned, and failures. Do not retain screenshots, window titles,
typed content or unbounded logs.

Normal App Server events expose the outer `node_repl` call. They do not give
Supervisor a documented per-`sky` callback for suppressing inner captures.
Keep detailed instrumentation in the explicit probe; do not patch the SDK,
replace the transport, parse arbitrary generated JavaScript as a reliable
action plan, or promise automatic deduplication in the host.

Review traces for repeated setup, unnecessary activation, duplicate image
emission and redundant captures with no intervening reason to refresh.
Correct an observed cause at a supported boundary and test one change at a
time. Preserve the official observation cycle: inspect current state, act once,
refresh and inspect again. User activity, focus/layout changes, modals,
interleaving and uncertain outcomes all require re-observation.

An old screenshot cache is not an acceptable substitute for this check.
Accessibility-only observation is appropriate only when it actually returns
the state needed for the next decision. A successful input API return alone
does not prove that the intended visible change happened.

### 3. Repeat controlled end-to-end tests

Keep Luna / low / Standard, identical prompts and permissions, and the same
official runtime versions within each comparison. Separate startup from warm
execution and use balanced baseline/variant ordering. Start with a small
budgeted pilot; expand repetitions only for a promising candidate rather than
spending model calls on an ineffective change.

Use Calculator plus an authorized small fixture with meaningful accessibility
state to test whether an improvement depends on one application. Report every
attempt, correctness, errors, observation counts and per-operation/end-to-end
times. Use medians and ranges for small samples; do not present a reliable
tail percentile or statistical guarantee from two runs.

Keep a change only when repeated results show a benefit without weaker
verification or reliability. Separate an official runtime update from a
Supervisor code change so the comparison remains interpretable. Run future
visual tests only when explicitly requested by the user.

### Additional finding: connection and update handling

Currently, `ComputerUseMcp` considers the integration available when
`mcpServerStatus/list` contains `node_repl.js`. That proves tool discovery,
not live reachability of the desktop service. Runtime preparation happens on
App Server connection; desktop restarts can change the mirrored pipe address.

Distinguish configuration/tool discovery from a failed or verified desktop
connection. Surface a useful recovery message on native connection failure,
refresh verified official references at a safe idle/reconnect boundary, and
leave an input with an unknown result for re-observation rather than automatic
replay. Do not add periodic screen capture merely to display a ready status.

At audit time the skill finder sorted cached directory names and chose the last
valid package. Only one candidate was present, so no active mismatch was
observed. With multiple retained versions this was not proof of the official
app's active selection. Validate against an authoritative supported installed
reference where available; detect ambiguity rather than silently mixing skill,
SDK and host versions. Do not invent a dependency on private catalog APIs.

The official App Server documentation explicitly labels `plugin/list`,
`plugin/read`, `plugin/install` and `plugin/uninstall` as under development and
says production clients should not call them yet. Automatic installation via
those methods is therefore not part of this plan. Documented skill discovery,
MCP status and configuration reload are the integration boundaries to assess.
[Official App Server methods](https://learn.chatgpt.com/docs/app-server).

## Source map

- Official installed plugin: `%USERPROFILE%/.codex/plugins/cache/openai-bundled/computer-use/26.908.70816/`:
  `.codex-plugin/plugin.json`, `skills/computer-use/SKILL.md`, `docs/guidance.md`,
  `docs/api.md` and `docs/confirmations.md`.
- Official SDK: `%LOCALAPPDATA%/OpenAI/Codex/runtimes/cua_node/6f12e0ef1c6e5061/bin/node_modules/@oai/sky/`:
  `package.json` and `dist/project/cua/sky_js/src/targets/windows/` wrappers.
- Official desktop package: `OpenAI.Codex_26.908.9136.0_x64__2p2nqsd0c76g0`:
  read-only, in-memory inspection of `app/resources/app.asar`, `package.json`
  and `.vite/build/main-D8abTQQE.js` native-pipe broker. No archive extraction,
  private RPC calls, helper launches or native binary reverse engineering.
- Supervisor integration: [computer_use.rs](../src/browser/app_server/computer_use.rs),
  [skills.rs](../src/browser/app_server/skills.rs),
  [app_server.rs](../src/browser/app_server.rs),
  [retirement.rs](../src/browser/app_server/computer_use/retirement.rs),
  [retired benchmark fixture](../crates/central-agent-codex-runtime/examples/fixtures/computer_use_companion.md).
- Current official web references, retrieved 2026-09-16:
  [Computer Use](https://learn.chatgpt.com/docs/computer-use),
  [Plugins](https://learn.chatgpt.com/docs/plugins),
  [App Server](https://learn.chatgpt.com/docs/app-server),
  [Changelog](https://learn.chatgpt.com/docs/changelog).

The initial audit above was read-only. The implementation authorized afterward
is recorded below.


## Implementation record — 2026-09-16

- Retired automatic companion attachment and its production template. Cleanup
  removes only the small marked Supervisor-owned file; the A/B fixture remains
  outside normal skill roots. User-owned content and vendor files are preserved.
- Added passive, connection-scoped native tool status. Discovery is unverified;
  successful completion is labelled as the last request only. Scoped native
  connection errors enable idle-only reconnect with no automatic input replay.
  This is a best-effort hint for recognized direct public `sky` calls, not a
  heartbeat: opaque helper aliases and unrecognized failure messages can remain
  unverified. A completed request does not prove its intended visual effect.
- Preserve preparation failures as actionable errors. Reject ambiguous cached
  versions and remove only stale managed runtime/hooks before connecting.
  Reconnect refreshes the official pipe/configuration and lifecycle hooks.
- Added a bounded timing helper for public sky methods and a typed parser in the
  opt-in probe. It supports Calculator and a read-only Supervisor settings-search
  scenario. Raw screenshots, window titles, typed content and arbitrary error
  payloads are excluded from samples. Timings cover the public call, not private
  service stages; uninstrumented calls are not claimed as measured.
- Updated the existing skills-dialog status and recovery control, design contract
  and visual scenarios. No new polling loop, screenshot cache, helper transport,
  permission bypass or extra production model calls were introduced.

The previously reported timing results remain historical measurements, not an
assertion of a new speed improvement. The instrumented follow-up, including the
excluded companion run, is recorded in the [speed test report](CODEX_COMPUTER_USE_SPEED_TEST.md).

### Validation and local delivery

- Workspace verification passed: Svelte check and build, design contract,
  JavaScript/UI checks, Rust workspace tests and Clippy. Cargo audit passed with
  the repository's existing exception and four existing advisory warnings; no
  dependency was added. The timing helper's three checks and the probe's two
  parsing/usable-observation checks also passed.
- Focused native integration verified the enabled official skill through real
  App Server discovery in an isolated profile, and absence of the retired skill.
  Main/graph automatic and manual attachment, queued input, disabled skill
  behavior, stale connection events, source ambiguity, managed cleanup and idle
  reconnect guards are covered by deterministic tests.
- The canonical updater rebuilt and replaced
  `outputs/Supervisor/Supervisor.exe`, then reopened it through an independent
  hidden Windows launcher. Independent physical-path checks confirmed the actual
  LocalAppData store, all six saved conversation bindings unchanged, six fresh
  accepted native history reads, and the retired managed file absent. No native
  database or credentials were copied.

### UI inspection record

Scenario: `official-computer-use-availability`. Working-tree implementation
of 2026-09-16, using the corresponding `DESIGN.md` update. The real Svelte
`NativeSkills` component and shared theme stylesheet were built into a temporary
synthetic fixture; no account content, native IPC, settings writes or inference
was connected to that fixture.

The in-app Browser inspected configured/unverified, completed, disconnected,
busy, ambiguous-version, loading and unavailable states in Light and Dark.
The reconnect control appeared only for recovery states and was disabled for
busy work. Full-surface captures were inspected at Full HD, 2K and 4K; the
longest error text was additionally inspected at 420 × 800 in both themes.
Status, explanation and action remained distinct; desktop layouts contained
the complete dialog, while the narrow layout wrapped text and contained
vertical overflow. No new animation was introduced. Existing keyboard and
owner-routing behavior was checked by component tests, not by a new exhaustive
native keyboard/DPI pass.

The temporary viewport was reset and the fixture tab closed. Raw captures and
fixture files are not retained in the repository. This is a focused inspection
of the changed status/recovery surface, not an assertion that every unrelated
Supervisor surface was visually retested.
