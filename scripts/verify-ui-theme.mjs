import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDirectory, "..");
const read = (relativePath) => fs.readFileSync(path.join(projectRoot, relativePath), "utf8");
const themes = read("assets/themes.css");
const panel = read("assets/agent-panel.html");
const graph = read("assets/agent-graph.html");
const toolbar = read("assets/toolbar.html");
const startPage = read("assets/start-page.html");
const preview = read("assets/preview.html");
const remoteDesktop = read("assets/remote-desktop.html");
const backdrop = read("assets/backdrop.html");
const fileIcons = read("assets/file-icons.js");
const agentTimeline = read("ui/src/lib/agent-timeline.ts");
const agentGraphCard = read("ui/src/components/AgentGraphCard.svelte");
const composer = read("ui/src/components/Composer.svelte");
const projectBoard = read("ui/src/components/ProjectBoard.svelte");
const projectRegistry = read("ui/src/components/ProjectRegistry.svelte");
const projectChatTree = read("ui/src/components/ProjectChatTree.svelte");
const projectFileActivity = read("ui/src/components/ProjectFileActivity.svelte");
const boardFiles = read("ui/src/components/BoardFiles.svelte");
const agentGraphHeader = read("ui/src/components/AgentGraphHeader.svelte");
const collaborationIndicator = read("ui/src/components/CollaborationIndicator.svelte");
const browserRust = read("src/browser.rs");
const agentGraphRust = read("src/browser/agent_graph.rs");
const settingsShell = read("ui/src/components/SettingsShell.svelte");

function requireMatch(source, pattern, description) {
  if (!pattern.test(source)) throw new Error(`UI theme verification failed: ${description}`);
}

function requireCount(source, pattern, expected, description) {
  const count = [...source.matchAll(pattern)].length;
  if (count !== expected) {
    throw new Error(`UI theme verification failed: ${description}; expected ${expected}, found ${count}`);
  }
}

function afterMarker(source, marker, description) {
  const index = source.lastIndexOf(marker);
  if (index < 0) throw new Error(`UI theme verification failed: ${description}`);
  return source.slice(index);
}

function balancedCss(source, name) {
  const stripped = source
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .replace(/"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'/g, "");
  let depth = 0;
  for (const char of stripped) {
    if (char === "{") depth += 1;
    if (char === "}") depth -= 1;
    if (depth < 0) throw new Error(`UI theme verification failed: ${name} closes a CSS block too early`);
  }
  if (depth !== 0) throw new Error(`UI theme verification failed: ${name} has ${depth} unclosed CSS block(s)`);
}

function relativeLuminance(hex) {
  const channels = hex.match(/[0-9a-f]{2}/gi).map((value) => Number.parseInt(value, 16) / 255);
  const linear = channels.map((value) => value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4);
  return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
}

function contrast(left, right) {
  const first = relativeLuminance(left);
  const second = relativeLuminance(right);
  return (Math.max(first, second) + 0.05) / (Math.min(first, second) + 0.05);
}

const panelMinimal = afterMarker(
  panel,
  "/* Monochrome control surface */",
  "the final monochrome application-surface override is missing",
);
const panelUnified = afterMarker(
  panel,
  "/* Unified divider-free work surface */",
  "the Agent surface must provide the divider-free structural override",
);
const toolbarUnified = afterMarker(
  toolbar,
  "/* Unified divider-free work surface */",
  "the browser chrome must provide the divider-free structural override",
);

requireCount(themes, /:root\[data-theme="central"\]/g, 1, "one light theme must be defined");
requireCount(themes, /:root\[data-theme="central_dark"\]/g, 1, "one dark theme must be defined");
requireCount(panel, /data-theme-choice=/g, 2, "settings must expose only Light and Dark");
requireMatch(themes, /one monochrome product language/i, "the monochrome product language must be documented");
requireMatch(themes, /--ca-accent:\s*#242424;/i, "the light graphite action token is missing");
requireMatch(themes, /--ca-app-background:\s*#0d0d0d;/i, "the flat black dark application surface is missing");
requireMatch(themes, /--ca-graph-canvas:\s*#eeeeec;/i, "the light graph canvas must match the application field");
requireMatch(themes, /--ca-graph-canvas:\s*#0d0d0d;/i, "the dark graph canvas must match the application field");
requireMatch(themes, /--ca-graph-edge:\s*#b2b2ae;/i, "the light graph edge must be neutral gray");
requireMatch(themes, /--ca-font-body:\s*"SF Pro Text"[^;]*"Inter"/i, "the Supervisor SF Pro-led interface font stack is missing");
requireMatch(themes, /--ca-font-display:\s*"SF Pro Display"[^;]*"Inter"/i, "the Supervisor display family must use the SF Pro-led stack");
requireMatch(themes, /--ca-font-navigation:\s*var\(--ca-font-body\)/i, "project-chat navigation must use the platform interface family");
requireMatch(themes, /--ca-type-navigation:\s*var\(--ca-type-label\)/i, "project-chat titles must match file-name size");
requireMatch(themes, /--ca-scrollbar-track:\s*var\(--ca-app-background\);[\s\S]*?--ca-scrollbar-thumb:\s*var\(--ca-border-strong\);[\s\S]*?--ca-scrollbar-arrow:\s*var\(--ca-icon\);/i, "visible scrollbars must consume the shared Supervisor palette");
requireMatch(themes, /::\-webkit-scrollbar-thumb\s*\{[\s\S]*?background:\s*var\(--ca-scrollbar-thumb\);/i, "WebView scroll thumbs must use the shared palette");
for (const direction of ["vertical:decrement", "vertical:increment", "horizontal:decrement", "horizontal:increment"]) {
  requireMatch(themes, new RegExp(`::\\-webkit-scrollbar-button:single-button:${direction.replace(":", "\\:")}\\s*\\{[\\s\\S]*?mask-image`, "i"), `the ${direction} scrollbar arrow is missing`);
}
requireMatch(themes, /@font-face\s*\{[^}]*font-family:\s*"Inter";[^}]*__SUPERVISOR_UI_FONT_DATA_URL__[^}]*format\("woff2"\)/i, "the licensed Windows interface fallback must be embedded as WOFF2 without a font service");
requireMatch(themes, /--ca-control-radius:\s*8px;/i, "the symmetric control radius is missing");
requireMatch(themes, /\.ca-folder-icon\s*\{[\s\S]*?--ca-folder-tone:\s*var\(--ca-icon\)\s*!important;[\s\S]*?background:\s*transparent;/i, "generic folders must use the minimal neutral icon treatment");
requireMatch(themes, /\.ca-folder-icon svg\s*\{[\s\S]*?fill:\s*none;[\s\S]*?stroke:\s*currentColor;/i, "folder icons must use one shared outline glyph");
requireMatch(fileIcons, /function centralFolderIconElement/i, "the shared minimal folder glyph factory is missing");
requireMatch(themes, /\.ca-file-icon\.has-brand/, "original file-brand icon support must remain enabled");
requireMatch(themes, /\.ca-file-icon\s*\{[\s\S]*?--ca-file-tone:\s*var\(--ca-icon\)\s*;/i, "file icons must retain a neutral fallback tone");
requireMatch(themes, /\.ca-brand-icon\s*\{[\s\S]*?color:\s*var\(--ca-file-tone\)\s*!important;/i, "file-brand silhouettes must consume their type color");
requireMatch(themes, /@media\s*\(prefers-reduced-motion:\s*reduce\)/i, "reduced-motion support is missing");
requireMatch(themes, /button:focus-visible[\s\S]*?outline:/i, "keyboard focus visibility is missing");
requireMatch(panel, /\.conversation-section\s*\{[\s\S]*?grid-template-rows:\s*auto auto minmax\(0,\s*1fr\) auto;[\s\S]*?overflow:\s*hidden;/i, "the conversation must reserve a bounded scroll row");
requireMatch(panel, /\.conversation-section \.chat-messages\s*\{[\s\S]*?min-height:\s*0;[\s\S]*?overflow-y:\s*auto;/i, "chat messages must remain independently scrollable");
requireMatch(panel, /\.activity-group-body\s*\{[\s\S]*?max-height:[^;]+;[\s\S]*?overflow-y:\s*auto;/i, "expanded activity groups must remain scrollable");
requireMatch(agentTimeline, /rememberActivityGroupViewState/, "activity-group open and scroll state must be remembered");
requireMatch(agentTimeline, /restoreActivityGroupViewState/, "activity-group open and scroll state must be restored");
requireMatch(agentTimeline, /chatFollowsEnd/, "streaming chat must retain explicit follow-end state");
requireMatch(panel, /finalizing:\s*"Finalizing"/, "the main agent panel must label checkpoint finalization explicitly");
requireMatch(panel, /window\.confirm\("Discard this failed checkpoint\? Its baseline will be deleted and cannot be restored\."\)/, "discarding a failed checkpoint must require confirmation in the main panel");
requireMatch(graph, /window\.confirm\("Discard this failed checkpoint\? Its baseline will be deleted and cannot be restored\."\)/, "discarding a failed checkpoint must require confirmation in the graph");
requireMatch(panelMinimal, /\.composer:focus-within\s*\{[\s\S]*?border-color:\s*var\(--ca-border-strong\);[\s\S]*?box-shadow:\s*var\(--ca-focus-ring\)/i, "the composer must expose one neutral tokenized focus ring");
requireMatch(panel, /\.composer textarea:focus-visible\s*\{\s*outline:\s*none;/i, "the composer textarea must not draw a second focus rectangle");
requireMatch(settingsShell, /\.settings-shell\s*\{[\s\S]*?background:var\(--ca-app-background\);color:var\(--ca-text\);/i, "the settings shell must use the active theme surfaces and text");
requireMatch(settingsShell, /\.settings-page-head\s*\{[\s\S]*?background:transparent;/i, "the settings header must reset legacy accent surfaces");
requireMatch(settingsShell, /\.settings-page-title p\s*\{[\s\S]*?color:var\(--ca-muted\);/i, "the settings description must use the active theme muted token");
requireMatch(graph, /body\.expanded\s+\.graph-shell\s*\{[\s\S]*?position:\s*fixed;[\s\S]*?100vw;[\s\S]*?100vh;/i, "Agent Graph must fill the application viewport");
requireCount(projectBoard, /id="graph-summary"/g, 0, "the retired project and supervisor count must stay absent");
requireCount(projectBoard, /id="close-graph"/g, 0, "Workspace must not consume a second row inside the project board");
requireMatch(projectBoard, /background:var\(--ca-app-background\)/, "project board must consume the application surface token");
requireMatch(projectBoard, /\.board\{[^}]*--board-section-spacing:var\(--ca-space-4\)/i, "project board must define one shared section-spacing token");
requireMatch(projectBoard, /\.board\{[^}]*--board-top-inset:10px/i, "project board headings must align with the upper edge of the return glyph");
requireMatch(projectBoard, /\.board-columns\{[^}]*grid-template-columns:[^;]*var\(--board-section-spacing\)[^;]*var\(--board-section-spacing\)[^;]*;[^}]*gap:0;[^}]*padding:var\(--board-top-inset\) var\(--board-section-spacing\) var\(--board-section-spacing\)/i, "project board must use identical physical divider columns with its aligned outer insets");
requireMatch(projectBoard, /\.board-columns\{[^}]*box-sizing:border-box;[^}]*inline-size:100%;[^}]*scrollbar-gutter:stable both-edges/i, "project board must reserve equal visible space at both inline edges");
requireMatch(projectBoard, /\.project-lane\{--project-registry-padding:0 var\(--board-edge-content-inset\) var\(--board-edge-content-inset\);[^}]*\}[\s\S]*?\.file-lane\{[^}]*padding-inline-end:var\(--board-edge-content-inset\)/i, "Projects title and project-file cards must share the outer content inset without a second top offset");
requireMatch(projectBoard, /\.lane-heading\{[^}]*padding:0 var\(--ca-space-2\) var\(--ca-space-4\)/i, "project chat, Supervisor and file headings must not add another top offset");
requireMatch(projectRegistry, /\.project-registry\{[^}]*padding:var\(--project-registry-padding,var\(--ca-space-4\)\)/i, "project registry must accept the board-owned edge inset");
requireMatch(projectRegistry, /\.compact \.registry-header\{align-items:flex-start;/i, "Projects heading must share the other lane headings' top alignment");
requireMatch(projectBoard, /\.conversation-lanes\{[^}]*grid-template-columns:[^;]*var\(--board-section-spacing\)[^;]*;[^}]*gap:0/i, "the resizable conversation lanes must dedicate the shared spacing token to their real divider column");
requireMatch(agentGraphCard, /\.graph-prompt-field\s*\{[^}]*border:\s*0;[^}]*background:\s*var\(--ca-input\);/i, "project-chat and Supervisor prompt fields must remain borderless");
requireMatch(projectBoard, /\.chat-scroll\s*:global\(\.chat-entry:has\(\.agent-console\)\)\{[^}]*background:var\(--project-chat-surface\);[^}]*box-shadow:none/i, "an open project chat must use one borderless shared outer surface");
requireMatch(projectBoard, /\.project-chat-window-layer\s*:global\(\.chat-entry \.agent-console[^)]*\)\{[^}]*--agent-card-background:var\(--project-chat-surface\);[^}]*border:0;[^}]*border-radius:var\(--ca-radius-none\)/i, "the project conversation must not draw a second frame inside its chat row");
requireMatch(agentGraphCard, /\.graph-prompt-field:focus-within\s*\{[^}]*box-shadow:\s*var\(--ca-focus-ring\);/i, "borderless card prompt fields must preserve a visible focus state");
requireMatch(agentGraphCard, /\.graph-prompt-field\s*\{[^}]*position:\s*relative;/i, "card prompt fields must anchor their work action");
requireMatch(agentGraphCard, /\.graph-work-slot\s*\{[^}]*position:\s*absolute;[^}]*inset-inline-end:\s*var\(--ca-space-2\);[^}]*inset-block-end:\s*var\(--ca-space-2\);/i, "card Send, Stop and Resume must keep equal right and bottom prompt insets");
requireMatch(agentGraphHeader, /data-action="minimize"[^>]*aria-label=\{minimized\?'Restore conversation':'Minimize conversation'\}/i, "conversation cards need one accessible Minimize and Restore control");
requireMatch(agentGraphCard, /data-card-minimized="true"[^}]*--agent-card-aspect-ratio:auto/i, "minimized cards must release their portrait height");
requireMatch(agentGraphCard, /\.agent-console-body\[hidden\]\s*\{\s*display:none;/i, "minimized cards must hide their retained body without unmounting it");
requireMatch(composer, /\.composer-work-slot\s*\{[^}]*position:\s*absolute;[^}]*inset-inline-end:\s*var\(--ca-space-2\);[^}]*inset-block-end:\s*var\(--ca-space-2\);/i, "main Send, Stop and Resume must keep equal right and bottom composer insets");
requireMatch(projectBoard, /@media\(max-width:1100px\)\{\.board\{--board-section-spacing:var\(--ca-space-3\)\}/i, "compact project-board spacing must reduce as one shared token");
for (const lane of ['projects','chats','supervisors','files']) requireMatch(projectBoard,new RegExp('data-board-lane="'+lane+'"'), 'missing project-board lane: '+lane);
requireMatch(fs.readFileSync(path.join(projectRoot,"ui/src/components/BoardFiles.svelte"),"utf8"), /populateCentralFileIcon/, "project files must use the shared file-brand registry");
requireMatch(collaborationIndicator, /data-collaboration-active/, "the shared collaboration-state hook is missing");
requireMatch(collaborationIndicator, /role="img"[\s\S]*?aria-label=\{label\}/i, "the collaboration mark needs an accessible label");
requireMatch(collaborationIndicator, /@media\s*\(prefers-reduced-motion:\s*reduce\)[\s\S]*?animation:\s*none/i, "the collaboration pulse must honor reduced motion");
for (const [source,name] of [[projectRegistry,"project cards"],[projectFileActivity,"task file rows"],[boardFiles,"project file rows"],[agentGraphHeader,"open conversation cards"]]) {
  requireMatch(source, /<CollaborationIndicator\b/, `${name} must reuse the collaboration mark`);
}
requireCount(projectChatTree, /<CollaborationIndicator\b/g, 0, "project chat rows must not duplicate their textual work state with a collaboration mark");
requireCount(projectBoard, /<CollaborationIndicator\b/g, 0, "Supervisor rows must not duplicate their textual work state with a collaboration mark");
requireMatch(agentGraphCard, /<AgentGraphHeader[^>]*working=\{active\|\|Boolean\(supervision\?\.active\)\}/i, "open cards must project their live work state into the header collaboration mark");
requireCount(graph, /getContext\(["']2d/g, 0, "retired graph canvas must not be rendered");
requireCount(graph, /<span class="graph-mark"/g, 0, "the expanded graph header must not render a logo");
requireCount(graph, /<h1>Knowledge graph<\/h1>/gi, 0, "the expanded graph header must not render a legacy title");
requireMatch(graph, /\.artifact-viewer-body iframe\s*\{[\s\S]*?background:\s*var\(--ca-surface\);/i, "artifact frames must not expose a hard-coded white loading surface");
requireMatch(fileIcons, /const centralFileIconData\s*=\s*Object\.freeze/i, "the local file-brand registry is missing");
requireMatch(fileIcons, /["']rust["']:\s*Object\.freeze/i, "the original Rust brand icon is missing");
requireMatch(fileIcons, /cargo:\s*["']rust["']/i, "Cargo files must resolve to the Rust brand");
requireMatch(fileIcons, /const centralFileIconTones\s*=\s*Object\.freeze/i, "the local file-color registry is missing");
requireMatch(fileIcons, /function populateCentralFileIcon/i, "Explorer file-brand rendering is missing");
requireMatch(fileIcons, /function drawCentralFileBrand/i, "graph file-brand rendering is missing");
requireMatch(fileIcons, /svg\.style\.color\s*=\s*["']var\(--ca-file-tone\)["']/, "DOM file-brand icons must use their type color");
requireMatch(graph, /data-graph-launcher-icon/, "Agent Graph must preserve its launcher hook");
requireMatch(graph, /data-central-agent-svelte="supervisor-logo"/, "Agent Graph must use the shared Supervisor logo");
requireMatch(read("ui/src/components/Composer.svelte"), /<SupervisorLogo\s*\/>/, "the configuration launcher must use the same Supervisor logo");
requireCount(panel, /createWorkspaceTetrahedron|data-tetra-piece|tetrahedron-in-flight/g, 0, "the retired tetrahedron must not render or disable the launcher");
requireCount(toolbar, /__CENTRAL_AGENT_MARK_SVG__/g, 0, "the retired brand placeholder must not return to the toolbar");
requireCount(panel, /__CENTRAL_AGENT_MARK_SVG__/g, 0, "the retired brand placeholder must not return to the agent panel");
requireCount(graph, /__CENTRAL_AGENT_MARK_SVG__/g, 0, "the retired brand placeholder must not return to the graph");
requireCount(preview, /<span class="mark"/g, 0, "the native preview header must not render the Supervisor mark");
requireMatch(toolbar, /\/\* Monochrome control surface \*\/[\s\S]*?border-radius:\s*var\(--ca-control-radius\)\s*!important;/i, "toolbar controls must have four equivalent corners");
requireMatch(toolbar, /\.navigation\s*>\s*\.icon-button\s*\{[\s\S]*?border:\s*0;[\s\S]*?background:\s*transparent;/i, "browser navigation controls must not render persistent circles");
requireMatch(toolbar, /\.navigation\s*>\s*\.icon-button svg\s*\{[\s\S]*?stroke-width:\s*2\.4;/i, "browser navigation icons must use the heavier stroke");
requireMatch(toolbar, /id="back-to-workspace"[\s\S]*?aria-label="Back to workspace"/i, "the compact Projects toolbar must expose an accessible workspace return action");
requireMatch(toolbar, /body\.project-board-open \.tab-strip\s*\{\s*display:\s*none;/i, "Projects must remove the browser tab strip");
requireMatch(toolbar, /body\.project-board-open \.navigation\s*>\s*:not\(\.workspace-toggles\)\s*\{\s*display:\s*none;/i, "Projects must remove browser navigation controls");
requireMatch(toolbar, /body\.project-board-open #toggle-agent,\s*body\.project-board-open #toggle-terminal\s*\{\s*display:\s*none;/i, "Projects must hide Agent and Terminal toolbar actions");
requireMatch(toolbar, /body\.project-board-open \.navigation\s*\{[\s\S]*?justify-content:\s*flex-start;[\s\S]*?padding:\s*0;/i, "Projects must place its return action flush with the upper-left corner");
requireMatch(toolbar, /body\.project-board-open \.workspace-toggles\s*>\s*\.workspace-back:hover\s*\{[^}]*background:\s*transparent;/i, "Projects return action must remain transparent on hover");
requireMatch(browserRust, /const PROJECT_TOOLBAR_HEIGHT_LOGICAL:\s*f64\s*=\s*40\.0;/i, "Projects must use the compact 40px return control");
requireMatch(browserRust, /fn project_toolbar_bounds[\s\S]*?PhysicalSize::new\(side,\s*side\)/i, "Projects return WebView must cover only its 40px square");
requireMatch(agentGraphRust, /fn agent_graph_bounds[\s\S]*?position:\s*PhysicalPosition::new\(0,\s*0\)[\s\S]*?size:\s*size\.into\(\)/i, "project board must begin at the top of the window");
requireMatch(agentGraphRust, /raise_webview\(surface\);[\s\S]*?raise_webview\(toolbar\);/i, "the transparent Projects return control must remain above the full-height board");
requireMatch(toolbar, /body\.project-board-open \.workspace-back span\s*\{\s*display:\s*none;/i, "Projects must render a bare return arrow");
requireCount(toolbar, /id="security"/g, 0, "the address field must not render a leading status dot");
requireCount(startPage, /<button\b/gi, 0, "the start-page search must submit with Enter without a trailing button");
requireMatch(startPage, /\/\* Monochrome control surface \*\/[\s\S]*?form\s*\{[\s\S]*?background:\s*var\(--ca-app-background\);/i, "the start-page search field must share the home surface");
requireMatch(startPage, /\.mark-slot\s*\{[\s\S]*?margin:\s*0 auto var\(--ca-space-3\);[\s\S]*?\.tagline\s*\{\s*margin:\s*var\(--ca-space-3\) 0 32px;/i, "Home logo and tagline gaps must be symmetric around the wordmark");
requireMatch(startPage, /form:focus-within\s*\{\s*border-color:\s*transparent;\s*box-shadow:\s*var\(--ca-focus-ring\);/i, "Home search focus must not draw a second inner border");
requireMatch(toolbar, /\.workspace-toggles\s*>\s*\.agent-toggle,[\s\S]*?\.agent-toggle\[aria-pressed="true"\]\s*\{\s*border:\s*0;/i, "Agent must remain borderless in idle, hover and selected states");
requireMatch(toolbar, /\.address-shell,\s*\.workspace-toggles,\s*\.workspace-toggles\s*>\s*button\s*\{[\s\S]*?height:\s*var\(--ca-browser-row-control-height\);/i, "Agent, Terminal and address field must consume one shared toolbar height");
requireMatch(panel, /\/\* Monochrome control surface \*\/[\s\S]*?border-radius:\s*var\(--ca-control-radius\)\s*!important;/i, "agent controls must have four equivalent corners");
requireMatch(toolbarUnified, /\.tab-strip,\s*\.navigation\s*\{\s*border-bottom-color:\s*transparent\s*!important;/i, "browser chrome must not draw section dividers");
requireMatch(panelUnified, /\.panel\s*>\s*header,[\s\S]*?\.conversation-toolbar,[\s\S]*?border-color:\s*transparent\s*!important;/i, "Agent sections must not draw divider lines");
requireMatch(toolbar, /html,\s*body\s*\{\s*background:\s*var\(--ca-app-background\);/i, "the toolbar must use the application surface token");
requireMatch(startPage, /body\s*\{\s*background:\s*var\(--ca-app-background\);/i, "the start page must use the application surface token");
requireMatch(preview, /html,\s*body\s*\{\s*background:\s*var\(--ca-app-background\);/i, "the preview must use the application surface token");
requireMatch(remoteDesktop, /html,\s*body,\s*\.desktop-shell\s*\{\s*background:\s*var\(--ca-app-background\);/i, "the remote desktop shell must use the application surface token");
requireMatch(backdrop, /background:\s*var\(--ca-app-background\);/i, "the window backdrop must use the application surface token");
if (contrast("242424", "ffffff") < 4.5) throw new Error("UI theme verification failed: light graphite actions do not meet WCAG AA contrast");
if (contrast("eeeeec", "111111") < 4.5) throw new Error("UI theme verification failed: dark monochrome actions do not meet WCAG AA contrast");

for (const file of fs.readdirSync(path.join(projectRoot, "assets")).filter((name) => name.endsWith(".html"))) {
  const source = read(path.join("assets", file));
  requireCount(source, /\/\*__THEMES_CSS__\*\//g, 1, `${file} must inject the shared theme stylesheet exactly once`);
  const styles = [...source.matchAll(/<style>([\s\S]*?)<\/style>/gi)];
  for (const [index, match] of styles.entries()) balancedCss(match[1], `${file} style ${index + 1}`);
}
balancedCss(themes, "themes.css");

process.stdout.write("UI theme verification passed (Light, Dark, surfaces, scroll, graph, icons, accessibility).\n");
