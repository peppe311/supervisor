# Supervisor identity and compatibility

User-authorized product rename, 2026-09-10: Central Agent becomes Supervisor.

## Product surfaces

- Native main/detached/terminal window titles, start page, Settings, dialogs,
  permission explanations, diagnostics and provider-facing display titles use
  Supervisor. The product package names are `Supervisor.exe`,
  `Supervisor.release.json` and `Supervisor.dependencies.json`.
- The approved logo is Regia: two angular forms express intelligence governing
  and checking other intelligence. The clipped upper corner, asymmetrical foot
  and open negative space make the silhouette recognizable without text or color.
  `assets/supervisor-mark.svg` is the canonical geometry.
  `SupervisorLogo.svelte` displays it in the configuration and graph launchers;
  the activity indicator clones that same bundled SVG. Each launcher retains
  its own accessible action name. The mark alone never grants system access.
- `assets/supervisor-wordmark.svg` contains the approved original lettering as
  outlines. The existing start-page heading embeds this trusted asset in
  `currentColor`, retaining its accessible name and line height. The separate
  native graph launcher supplies the symbol above it. No runtime font, external
  image request or frontend protocol is added to browser tabs.
- The configuration launcher keeps its position, hit target, native owner,
  disabled/expanded/focus state and existing menu/shortcut behavior. The old
  tetrahedron and its rotation, decomposition and piece transfers are removed.
  Active work uses a still mark, not a fabricated progress percentage.
- `scripts/build-brand-assets.ps1` generates native PNG/ICO from the same SVG
  and shared theme colors. The icon contains 16/24/32/48/64/128/256 px variants.
  Each size is rendered independently; `assets/supervisor-mark-native-16.svg`
  preserves the approved 16 px optical correction. All native variants have a
  transparent background and no tile or keyline. File/taskbar artwork uses
  neutral `--ca-brand-file-ink`; window artwork uses the active theme ink.
  `src/brand.rs` uses the PNG for native windows; `assets/supervisor.rc` embeds
  the ICO and Supervisor product/version metadata into the main executable.
  Building on Windows requires the Windows SDK resource compiler. Regenerate
  native assets when changing the SVG or its native palette.
  Native windows select the small ICO frame for the current display scale and
  theme (resource 2 for Light, 3 for Dark), with file/taskbar artwork in resource
  1. Crossing displays or switching themes refreshes the window icon.

## Shape and use

The mark has two filled paths, no outline strokes, gradients or animation.
Colors come only from `assets/themes.css`: the interface uses its current ink;
native windows use `--ca-brand-ink` from Light or Dark. The graph and configuration
buttons keep a transparent background and no border in idle, hover and expanded
states, while preserving their keyboard focus indicator and original hit areas.
Keep a clear area of one quarter of the visible mark height for standalone
branding. The minimum native icon is 16 px; the outlined wordmark is at least
128 px wide. Do not stretch, rotate, close the gaps or reconstruct the lettering
with a system font. Full brand deliverables, including the visual identity PDF,
are retained locally in the original brand kit. The public source includes
canonical artwork in `assets/`; see [brand deliverables](brand/README.md).
The canonical runtime artwork remains in `assets/`.

## Companion typography

The interface follows SF Pro's neutral proportions and compact rhythm: SF Pro
Text for interface copy and SF Pro Display for larger roles where the operating
system supplies them. Apple does not license its font files for embedding in a
Windows application, so Supervisor embeds unmodified Inter Variable as its close,
licensed Windows fallback. Existing regular and medium weights serve copy;
semibold serves controls and headings. The approved wordmark remains an
autonomous vector asset. Commands, code and terminal output continue to use the
shared monospace family.

Project-chat titles use the same shared 13 px label scale and interface family
as file names. Full titles remain available to assistive technology and on hover.

`assets/fonts/inter/Inter-Variable.woff2` is the unmodified official Inter 4.1
OFL release (optical-size 14–32 and weight 100–900 axes), embedded in every
internal host without a network request. License and exact file provenance are stored
alongside it; the full OFL text is included in `THIRD_PARTY_NOTICES.md` for
packaged executables.

## Deliberately retained identifiers

The rename does not migrate or delete data. Retain these compatibility contracts:

- `ProjectDirs::from("dev", "CentralAgent", "CentralAgent")`, existing profile
  paths, saved chats, drafts, native bindings, preferences and shared locks;
- `CENTRAL_AGENT_*` environment variables and existing remote upload paths;
- `central_agent` native client identity and Codex App Server protocol schemas;
- `central-agent:*` events, `CentralAgentSvelte`, `central-agent-ui` resource
  protocol, legacy DOM/data hooks including `tetra-config-*` and graph logo IDs;
- internal Cargo/npm package names, build cache paths and historical artifacts.

Personal project/chat titles remain user-owned. A project folder actually named
Central Agent will still have that name unless the user renames it separately.
Do not rename the repository directory, adjust account credentials, resubmit
native work or alter signing requirements as part of rebranding.

## Verification and release boundary

The visual scenario is `supervisor-brand`; existing configuration and graph
startup regressions also verify logo presence, shared geometry and absence of
tetrahedron pieces. Brand tests check native raster contrast/transparency and
the multi-size ICO. Release-manifest tests now enforce Supervisor package names
without relaxing provenance, signing or path validation.

The working tree contains earlier user changes. A locally compiled unsigned
preview is not a signed release. Verification results and artifact identities
are recorded in the Codex acceptance/validation notes. Dated output directories,
screenshots and temporary profiles were removed during the user-requested
workspace cleanup; `outputs/Supervisor/` contains the current local application.
Earlier live-account/F1 results remain historical; no new account mutation or
independent-fork F2 validation is implied by the new brand.
