---
name: supervisor-computer-use
description: Reduce avoidable setup, observation and retry work when using the official Computer Use plugin in Supervisor. Applies to desktop operation with that plugin, not explanations about it.
---

# Computer Use in Supervisor

<!-- Retired instructions, retained only for explicit A/B measurement. -->

Use the installed official Computer Use skill and `@oai/sky` runtime. This
companion supplies efficiency guidance; the official skill, its permissions,
confirmation rules and recovery limits remain authoritative.
Use it alongside the enabled official skill. If that skill is unavailable or
disabled, this companion does not provide an alternative way to operate the desktop.

## Start with the current API

Before the first desktop action, read the installed runtime guidance and API
reference together, once for the current plugin version. If both are already
in context, reuse them. Read the official confirmation guidance before deciding
whether an action needs confirmation. The installed reference paths appear below.

Use the documented method signatures and returned object shapes. A window object,
an app identifier, an accessibility index and a screenshot identifier are distinct
values. Do not discover argument names by repeatedly calling the desktop bridge.
If a call fails, check its signature and the official recovery guidance before
retrying; an uncertain input outcome requires a fresh observation.

## Reuse setup, refresh observations

Import `@oai/sky` once per fresh `node_repl` session with the official guarded
`globalThis.sky` initialization. Keep the selected returned window and useful
session values on `globalThis`. Repeated top-level `const` or `let` declarations
can fail on retries; use block-local names for temporary values.

Reuse the selected window while it remains valid. Rediscover it after a window
closes, a modal appears, the target changes, or the session resets. Input methods
activate their target window; avoid an extra activation before every action.
Use the official activation recovery when another window obstructs the target.

## Observe only what the next decision needs

- Use a screenshot for visual judgment, canvas content or weak accessibility.
- Use accessibility text for readable controls, element indexes or focus.
- Request both when the next decision actually requires both. A combined capture
  can be cheaper than two separate observations when focus and visual content
  both matter.
- Inspect the returned observation before choosing an action. Then perform one
  state-derived action and refresh immediately in the same JavaScript call.
  Inspect that refreshed result before the next action. Do not add a redundant
  capture when the just-returned observation is still current.
- Screenshot payloads are displayed by the runtime. Do not print, save or re-emit
  their encoded contents for inspection; print only needed accessibility fields.
- Prefer a documented keyboard shortcut when it reduces navigation. Confirm the
  intended editable surface and current focus before entering text.

Keep the official observe-act-refresh cycle. Reusing session setup does not make
old coordinates, screenshot IDs or element indexes valid after a state change.
Do not batch dependent clicks using one old observation, add speculative retries,
or insert fixed sleeps after every successful action. Follow the official wait
and retry rules for an actual timeout or loading state. Stop desktop input when
the user interrupts or Computer Use reports that the turn has ended.

Preserve the user's model, effort, speed and permissions. Efficiency guidance is
not authorization to change those settings or operate additional applications.
