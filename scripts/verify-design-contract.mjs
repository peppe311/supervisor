import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDirectory, "..");
const read = (relativePath) => fs.readFileSync(path.join(projectRoot, relativePath), "utf8");
const failures = [];

function requireCondition(condition, message) {
  if (!condition) failures.push(message);
}

function collectFiles(directory) {
  const files = [];
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const absolutePath = path.join(directory, entry.name);
    if (entry.isDirectory()) files.push(...collectFiles(absolutePath));
    if (entry.isFile()) files.push(absolutePath);
  }
  return files;
}

function relative(absolutePath) {
  return path.relative(projectRoot, absolutePath).replaceAll(path.sep, "/");
}

const design = read("DESIGN.md");
const requiredSections = [
  "## Product character",
  "## Protect this priority order",
  "## Frame the operator's job",
  "## Compose before decorating",
  "## Required states",
  "## Reject generated-interface reflexes",
  "## Component and token contract",
  "## Accessibility and resilience",
  "## Inspect and revise",
];

for (const section of requiredSections) {
  requireCondition(design.includes(section), `DESIGN.md is missing ${section}`);
}

const themes = read("assets/themes.css");
const requiredTokens = [
  "--ca-space-1",
  "--ca-space-2",
  "--ca-space-3",
  "--ca-space-4",
  "--ca-space-6",
  "--ca-space-8",
  "--ca-space-12",
  "--ca-type-caption",
  "--ca-type-body",
  "--ca-type-label",
  "--ca-type-title",
  "--ca-type-display",
  "--ca-leading-compact",
  "--ca-leading-body",
  "--ca-control-compact",
  "--ca-control-default",
  "--ca-control-radius",
  "--ca-duration-fast",
  "--ca-duration-standard",
  "--ca-duration-deliberate",
  "--ca-ease-standard",
  "--ca-ease-emphasized",
  "--ca-focus-ring",
];

for (const token of requiredTokens) {
  requireCondition(themes.includes(`${token}:`), `assets/themes.css is missing ${token}`);
}

const sourceRoot = path.join(projectRoot, "ui", "src");
const sourceFiles = collectFiles(sourceRoot).filter((file) => /\.(css|svelte|ts)$/.test(file));
const rawColor = /(?:#[0-9a-f]{3,8}\b|\b(?:rgb|rgba|hsl|hsla|lab|lch|oklab|oklch)\s*\()/i;
const literalTypeSize = /font-size\s*:\s*(?:\d|\.)(?:[^;}]*)/i;
const literalRadius = /border-radius\s*:\s*(?:\d|\.)(?:[^;}]*)/i;

for (const file of sourceFiles) {
  const source = fs.readFileSync(file, "utf8");
  const name = relative(file);
  const styleRelevant = file.endsWith(".css") || file.endsWith(".svelte");

  if (styleRelevant) {
    requireCondition(!rawColor.test(source), `${name} contains a raw color; consume a --ca-* token`);
    requireCondition(!literalTypeSize.test(source), `${name} contains a literal font-size; consume a --ca-type-* token`);
    requireCondition(!literalRadius.test(source), `${name} contains a literal border-radius; consume a --ca-radius-* token`);
    requireCondition(!/transition\s*:\s*all\b/i.test(source), `${name} uses transition: all`);
    requireCondition(!/\sstyle\s*=\s*["']/i.test(source), `${name} contains an inline style attribute`);
    requireCondition(!/!important\b/i.test(source), `${name} uses !important`);
    requireCondition(!/--ca-[\w-]+\s*:/i.test(source), `${name} redeclares a shared --ca-* token`);
  }

  if (file.endsWith(".svelte")) {
    for (const match of source.matchAll(/<button\b([^>]*)>([\s\S]*?)<\/button>/gi)) {
      const attributes = match[1];
      const body = match[2]
        .replace(/<[^>]+>/g, " ")
        .replace(/\{[^}]*\}/g, " ")
        .replace(/&[a-z0-9#]+;/gi, " ")
        .replace(/[^\p{L}\p{N}]+/gu, "")
        .trim();
      const named = /\baria-label\s*=|\baria-labelledby\s*=/i.test(attributes);
      requireCondition(Boolean(body || named), `${name} contains an icon-only button without an accessible name`);
    }
  }
}

const manifest = JSON.parse(read("ui/evals/scenarios.json"));
requireCondition(manifest.schemaVersion === 1, "ui/evals/scenarios.json must use schemaVersion 1");
requireCondition(manifest.capture?.fixturePolicy === "synthetic-redacted", "visual fixtures must be synthetic and redacted");

const expectedThemes = ["central", "central_dark"];
requireCondition(
  JSON.stringify(manifest.capture?.themes) === JSON.stringify(expectedThemes),
  "visual scenarios must cover exactly the Supervisor Light and Dark themes",
);

const expectedViewports = new Map([
  ["full-hd", [1920, 1080]],
  ["two-k", [2560, 1440]],
  ["four-k", [3840, 2160]],
]);
for (const viewport of manifest.capture?.viewports ?? []) {
  const expected = expectedViewports.get(viewport.id);
  requireCondition(Boolean(expected), `unexpected visual viewport ${viewport.id}`);
  if (expected) {
    requireCondition(
      viewport.width === expected[0] && viewport.height === expected[1],
      `${viewport.id} must remain ${expected[0]}x${expected[1]}`,
    );
    expectedViewports.delete(viewport.id);
  }
}
requireCondition(expectedViewports.size === 0, `missing visual viewports: ${[...expectedViewports.keys()].join(", ")}`);

const requiredScenarios = new Set([
  "home-agent-idle",
  "home-agent-streaming",
  "browser-tab-context",
  "workspace-time-machine",
  "agent-graph-dense",
  "agent-graph-active",
  "terminal-multi-session",
  "settings-complete",
  "remote-desktop-control",
]);
const seenScenarios = new Set();
for (const scenario of manifest.scenarios ?? []) {
  requireCondition(!seenScenarios.has(scenario.id), `duplicate visual scenario ${scenario.id}`);
  seenScenarios.add(scenario.id);
  requiredScenarios.delete(scenario.id);
  requireCondition(Boolean(scenario.surface), `${scenario.id} is missing its surface`);
  requireCondition(Boolean(scenario.readerJob), `${scenario.id} is missing its readerJob`);
  requireCondition(Boolean(scenario.setup), `${scenario.id} is missing deterministic setup`);
  requireCondition((scenario.assertions?.length ?? 0) >= 3, `${scenario.id} needs at least three observable assertions`);
}
requireCondition(requiredScenarios.size === 0, `missing visual scenarios: ${[...requiredScenarios].join(", ")}`);

if (failures.length > 0) {
  for (const failure of failures) process.stderr.write(`Design contract violation: ${failure}\n`);
  process.exit(1);
}

process.stdout.write(
  `Design contract verified (${sourceFiles.length} modular UI files, ${seenScenarios.size} visual scenarios).\n`,
);
