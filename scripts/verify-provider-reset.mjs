import fs from "node:fs";
import path from "node:path";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import ts from "../ui/node_modules/typescript/lib/typescript.js";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const read = (name) => fs.readFileSync(path.join(root, name), "utf8");
for (const retired of ["src/codex_provider.rs", "src/codex_provider", "src/browser/codex_conversations.rs", "protocol/codex"]) {
  assert.equal(fs.existsSync(path.join(root, retired)), false, `${retired} must remain removed`);
}
const providers = read("src/provider.rs");
for (const id of ["ClaudeCode", "Cursor", "GithubCopilot", "GoogleAntigravity", "OpencodeGo"]) assert.ok(providers.includes(id), `Missing ${id}`);
assert.doesNotMatch(providers, /\bCodex\s*,/);
assert.doesNotMatch(read("src/main.rs"), /mod codex_provider/);

for (const file of ["assets/agent-panel.html", "assets/agent-graph.html"]) {
  const html = read(file);
  assert.doesNotMatch(html, /respond_codex_request|retry_codex_provider|codex_command|codex_history|data-codex-/);
  if (file.includes("agent-panel")) {
    assert.match(html, /window\.renderAgentPanelState\s*=/);
    assert.match(html, /currentConversationOwner = state\.activeProjectChatId/);
  } else assert.match(html, /central-agent:graph-composer-state/);
  for (const [index, match] of [...html.matchAll(/<script(?:\s[^>]*)?>([\s\S]*?)<\/script>/g)].entries()) {
    const name = path.join(root, `inline-${path.basename(file)}-${index}.js`);
    const options = { allowJs: true, checkJs: true, noEmit: true, target: ts.ScriptTarget.ESNext, lib: ["lib.esnext.d.ts", "lib.dom.d.ts"], skipLibCheck: true };
    const host = ts.createCompilerHost(options);
    const previousRead = host.readFile.bind(host), previousExists = host.fileExists.bind(host);
    host.readFile = (fileName) => fileName === name ? match[1] : previousRead(fileName);
    host.fileExists = (fileName) => fileName === name || previousExists(fileName);
    const program = ts.createProgram([name], options, host);
    const failures = ts.getPreEmitDiagnostics(program).filter((diagnostic) => [2304, 2552].includes(diagnostic.code));
    assert.deepEqual(failures.map((d) => ts.flattenDiagnosticMessageText(d.messageText, " ")), [], `${file}: dangling JavaScript references`);
  }
}
console.log("Provider reset verified: retired transport absent, other providers retained, UI routes resolve.");
