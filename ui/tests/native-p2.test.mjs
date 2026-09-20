import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import { compile } from "svelte/compiler";
import { render } from "svelte/server";
import { emptyAppServerP2 } from "../src/lib/app-server-p2.ts";

async function renderP2(view) {
  const source = fs.readFileSync(new URL("../src/components/NativeP2Settings.svelte", import.meta.url), "utf8");
  let code = compile(source, { filename: "NativeP2Settings.svelte", generate: "server" }).js.code;
  code = code.replace(/from (["'])([^"']+)\1/g, (whole, quote, specifier) =>
    `from ${JSON.stringify(specifier === "../lib/app-server-p2" ? new URL("../src/lib/app-server-p2.ts", import.meta.url).href : import.meta.resolve(specifier))}`);
  const { default: Component } = await import(`data:text/javascript;base64,${Buffer.from(code).toString("base64")}`);
  return render(Component, { props: { view, connected: true, accountSupported: true, availableResetCredits: "2", resetCreditsCurrent: true, requirementsLoaded: true, onAction: () => {} } }).body;
}

test("P2 renders only sanitized snapshots and keeps each authority explicit", async () => {
  const view = emptyAppServerP2();
  view.scope = { id: "scope", project: "C:\\project" };
  view.features = { viewId: "features", loading: false, loaded: true, current: true, error: null, notice: null, entries: [
    { name: "safe_feature", stage: "beta", displayName: "Safe feature", description: "Description <unsafe>", announcement: null, enabled: false, defaultEnabled: false, mutable: true }
  ] };
  view.migration = { ...view.migration, viewId: "migration", current: true, items: [
    { id: "opaque-item", itemType: "SKILLS", description: "One detected skill", scope: "Current project", details: "PRIVATE_RAW_DETAILS" }
  ] };
  view.feedback.allowed = true;
  view.command.process = { id: "opaque-process", project: "C:\\project", argv: ["git", "status; whoami"], access: "readOnly", status: "completed", exitCode: 0, output: [{ stream: "stdout", text: "safe <output>" }], truncated: false, nativeProcessId: "PRIVATE_NATIVE_ID" };
  const html = await renderP2(view);
  assert.match(html, /Advanced Codex tools/);
  assert.match(html, /This scope follows the active local project automatically/);
  assert.match(html, /Filter feature flags/);
  assert.doesNotMatch(html, /Synchronize project/);
  assert.match(html, /never a shell-interpolated command/);
  assert.match(html, /No logs, file paths, attachments, tags or conversation content/);
  assert.match(html, /Description &lt;unsafe>/);
  assert.match(html, /safe &lt;output>/);
  assert.match(html, /status; whoami/);
  assert.doesNotMatch(html, /PRIVATE_RAW_DETAILS|PRIVATE_NATIVE_ID/);
  assert.doesNotMatch(html, /dangerFullAccess|Full access/);
});

test("P2 source sends no logs, raw migration details or arbitrary filesystem methods", () => {
  const source = fs.readFileSync(new URL("../src/components/NativeP2Settings.svelte", import.meta.url), "utf8");
  assert.doesNotMatch(source, /fs\/readFile|fs\/writeFile|thread\/inject_items|config\/value\/write/);
  assert.match(source, /No logs, files, paths, tags or conversation/);
  assert.match(source, /item_ids/);
  assert.match(source, /process_id/);
  assert.match(source, /scope_id: operation\.scopeId/);
  assert.doesNotMatch(source, /failureMessages/);
  assert.match(source, /class="choice"/);
  assert.match(source, /input\[type="checkbox"\]/);
});

test("P2 terminal failures never retain requesting or uploading presentation", async () => {
  const view = emptyAppServerP2();
  view.scope = { id: "scope", project: "C:\\project" };
  view.feedback = { allowed: true, busy: false, status: "failed", error: "App Server rejected the operation." };
  view.accountActions = {
    busy: false,
    resetRetryId: null,
    resetStatus: "failed",
    nudgeStatus: "uncertain",
    error: "The final result is unavailable."
  };
  view.migration = {
    ...view.migration,
    busy: true,
    activeImport: { status: "accepted", results: [] }
  };
  const html = await renderP2(view);
  assert.match(html, /Feedback was not sent/);
  assert.match(html, /Reset was not applied/);
  assert.match(html, /Email delivery unknown/);
  assert.match(html, /Import accepted · waiting for progress/);
  assert.doesNotMatch(html, /Uploading text-only feedback|Requesting reset|Requesting workspace email/);
});

test("P2 uncertain termination remains active without unsafe controls or clear", async () => {
  const view = emptyAppServerP2();
  view.scope = { id: "scope", project: "C:\\project" };
  view.command.process = {
    id: "opaque-process",
    project: "C:\\project",
    argv: ["fixture"],
    access: "readOnly",
    status: "termination_uncertain",
    exitCode: null,
    output: [],
    truncated: false
  };
  view.command.error = "Termination outcome is unknown.";
  const html = await renderP2(view);
  assert.match(html, /Termination outcome unknown/);
  assert.doesNotMatch(html, /Resize PTY|Terminate…|Standard input|Clear result/);
});
