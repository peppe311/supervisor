# Central Agent UI

This directory contains the modular Svelte and TypeScript layer rendered inside
Central Agent's existing Wry WebViews.

The migration is deliberately incremental:

- Rust, `winit`, Wry, WebView2, native browser tabs, RDP/VNC, and native bridges
  remain the application runtime.
- Svelte owns isolated presentation surfaces while the existing bridge code keeps
  using the stable DOM IDs and message contracts.
- The Agent composer owns keyboard, attachment-drop, and action gestures through
  cancelable semantic events. `src/lib/agent-timeline.ts` owns message identity,
  incremental updates, activity grouping, and scroll continuity; the legacy
  bridge supplies only specialized Markdown, artifact, and checkpoint rendering.
- `npm run build` writes deterministic static assets to `ui/dist`.
- `npm run dev:watch` rebuilds those assets after source changes; normally use
  `..\scripts\start-ui-development.ps1`, which also starts the Rust host and
  enables targeted WebView reloads.
- Rust embeds those generated assets with `include_str!`, so the released
  executable has no Node.js or web-server dependency.

The source of truth for shared colors, typography, radii, and spacing remains
`assets/themes.css`. Component-specific layout should use those design tokens
instead of introducing a second theme system.

Read `../DESIGN.md` before changing a user-facing surface. It contains the
product's hierarchy, interaction, content, accessibility, and review contract.
`evals/scenarios.json` holds stable operator scenarios for matched visual
comparison. `npm run check:design` validates the contract, token API, scenario
coverage, and the no-arbitrary-styling boundary for modular UI source.

Run `..\scripts\build-frontend.ps1` after changing a component. The normal
workspace verification and release pipeline also rebuild this bundle.
