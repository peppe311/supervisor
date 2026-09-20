# Official Computer Use in Supervisor

Checked 2026-09-15 with the installed Codex App Server `0.154.0-alpha.6.2`.

Supervisor keeps its dedicated `CODEX_HOME`, SQLite database and ChatGPT sign-in.
The official Codex desktop installation owns the Computer Use plugin and the
Windows desktop bridge. At each Supervisor Codex connection, Supervisor checks
that the official `computer-use@openai-bundled` plugin is enabled, verifies its
OpenAI manifest and cached skill, and reads only the official `node_repl` MCP
server table. Explicitly disabled server or skill toggles are respected. It mirrors that server table and the plugin's lifecycle hooks into
the separate Codex configuration, with ownership comments. It never copies
credentials, rollouts, databases, plugin binaries or skill contents. A foreign
`node_repl` server in the separate profile is left untouched.

After native initialization, Supervisor sets the official skill directory using
`skills/extraRoots/set`, then checks `mcpServerStatus/list`. The **Codex skills**
dialog reports whether the runtime is available and offers the native skill
for the next prompt. User-configured extra skill roots retain the official root.
If the official plugin is disabled or missing on a later connection, Supervisor
removes only its managed server/hook entries and leaves other Codex settings and
hooks intact. Existing conversations remain in Supervisor's own profile.

An isolated temporary App Server profile confirmed that `skills/list` returned
`computer-use:computer-use`, `mcpServerStatus/list` returned `node_repl` with
`js` and `turn_ended`, and a thread-scoped `mcpServer/tool/call` imported the
official `@oai/sky` library without error. The temporary thread had no model
turn. No Computer Use desktop operation, screen read, click or typing occurred.
The product's configuration merge was also tested against the installed plugin
with a disposable profile, plus synthetic tests for preservation, reconnection
and removal after disabling the plugin.

This attachment uses the official plugin's installed skill, MCP runtime and
hook definition, but it does **not** register a second installed plugin in
Supervisor's isolated profile. `plugin/read` rejected the bundled marketplace
as reserved in the isolated client; OpenAI marks App Server `plugin/install`
under development and advises production clients not to call it. Native plugin
inventory therefore remains empty for Computer Use in Supervisor. Plugin
installation, activation and Windows app access decisions remain in the
official Codex desktop app. Hook trust is governed by Codex's normal hook
policy; Supervisor does not bypass it. If the official desktop host stops or
changes its bridge while Supervisor is connected, reconnect Codex in Supervisor
to refresh the bridge configuration.

An explicit visual acceptance run on 2026-09-15 found that an ordinary prompt
mentioning the official plugin did not reliably trigger implicit skill use: the
agent treated Computer Use as unavailable. Selecting `computer-use:computer-use`
in **Codex skills** for the next prompt made the agent use `node_repl` and the
official `@oai/sky` bridge. It opened Windows Calculator, clicked `2 + 3 =`,
and reported `5`; the Calculator window independently displayed `5`.

Supervisor now attaches that same native skill input automatically when a prompt
directly asks to use Computer Use and the verified runtime is available. A fresh
chat, with no manual skill selection, used `node_repl` to clear Calculator,
click `4 + 1 =`, and read `5`; independent visual inspection again showed `5`.
Explanatory and negated mentions do not attach it. Several `js` calls in both
turns failed while the agent learned the bridge's argument shapes; the agent
recovered and completed each calculation. Supervisor's display projection omits
the transport-only skill marker from the user bubble and replaces encoded
screenshot payloads in tool details with a placeholder while retaining useful
diagnostics. Native turn data stays intact.

The App Server's direct MCP result panel still shows text and metadata but does
not project screenshot image blocks. In-turn use is owned by Codex and its
native tools. Visual acceptance was performed only after the user's explicit
request, per `AGENTS.md`.

Supervisor now draws a narrow, dual-contrast native frame at the edge of each
active Windows monitor when a verified `node_repl` `js` call invokes the
official `@oai/sky` bridge. Selecting the skill or mentioning Computer Use
without an actual bridge call does not display it. The frame is layered,
topmost, click-through, non-activating and hidden from task switching. It stays
through the Computer Use turn, then closes on turn completion, thread closure,
disconnect, reconnect or application exit. This is a desktop activity indicator;
Supervisor still does not reproduce the official app's in-chat live screenshot
viewer.
Native checks confirmed the frame's layered, non-activating and click-through
window styles. A synthetic Windows paint probe measured the fixed dark edge
(RGB 17) and light edge (RGB 247) against matching light and dark panels;
both sides of the frame remain legible without starting a Computer Use task.

References: [Computer Use](https://learn.chatgpt.com/docs/computer-use),
[Plugins](https://learn.chatgpt.com/docs/plugins),
[App Server](https://learn.chatgpt.com/docs/app-server),
[Hooks](https://learn.chatgpt.com/docs/hooks).

## Official baseline and connection recovery

The extra Supervisor efficiency skill was retired on 2026-09-16 after the
four-run comparison showed no measured benefit (90.812 seconds with the
companion versus 81.693 seconds without it). Main chat, graph cards, queues
and steering attach only the enabled official skill. The retired instructions
are retained solely as an explicit benchmark fixture outside normal skill roots.

On a normal connection, Supervisor removes only the old companion file bearing
its exact ownership marker. Missing files are a no-op; foreign skills and
unrelated configuration/files are preserved. A redirected or invalid target
is not followed. No vendor plugin file is modified.

The skills dialog distinguishes tool discovery from desktop connection evidence.
Configured means that the native tool was discovered, not that a desktop call
has succeeded. A recognized native connection failure explains recovery and
offers Reconnect Codex only when all native work, queued inputs and settings
writes are idle. Reconnection rereads the official references; no desktop action
is replayed. A successful observed tool invocation is described only as the last
request having completed, not as proof of its visible effect or future health.
These passive status updates perform no captures or additional model calls.

Preparation errors are no longer silently discarded. More than one valid cached
plugin version is reported as ambiguous instead of selecting by directory name.
An invalid/ambiguous source removes only Supervisor-managed runtime/hooks from
its own configuration; unrelated native servers remain unchanged.

The opt-in probe can time individual public sky methods using a bounded helper.
It records operation names, phases, durations, result-availability booleans and
failures; it never saves screenshot payloads, input text or window titles and
never patches the vendor SDK. All actual desktop actions still use the official
public API and observe/act/refresh cycle.

See the [speed report](CODEX_COMPUTER_USE_SPEED_TEST.md) and
[official-component audit](CODEX_COMPUTER_USE_OFFICIAL_AUDIT.md) for evidence,
limitations, the proposed comparison protocol and implementation validation.
