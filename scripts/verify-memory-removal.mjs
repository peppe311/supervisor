import { existsSync, readFileSync, readdirSync } from "node:fs";
import { dirname, extname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const retiredPaths = [
  "src/local_memory.rs",
  "src/mcp_integration.rs",
  "crates/central-agent-memory/Cargo.toml",
  "crates/central-agent-local-mcp/Cargo.toml",
];
for (const path of retiredPaths) {
  if (existsSync(join(root, path))) throw new Error(`Retired Memory surface returned: ${path}`);
}

const forbidden = [
  ["memory.sqlite3", "the retired Memory database filename"],
  ["central-agent-memory", "the retired Memory crate"],
  ["central-agent-local-mcp", "the retired local MCP crate"],
  ["rusqlite", "the retired SQLite runtime dependency"],
  ["libsqlite3-sys", "the retired native SQLite dependency"],
  ["CapabilityScope::Memory", "the retired Memory permission scope"],
  ["Brain Core", "the retired Memory interface"],
];
const roots = ["src", "crates", "assets", "ui/src"];
const files = ["Cargo.toml", "Cargo.lock", "build.rs"];
function collect(directory) {
  for (const entry of readdirSync(join(root, directory), { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) collect(path);
    else if ([".rs", ".toml", ".html", ".ts", ".svelte"].includes(extname(entry.name))) files.push(path);
  }
}
for (const directory of roots) collect(directory);

const failures = [];
for (const file of files) {
  const source = readFileSync(join(root, file), "utf8");
  for (const [needle, description] of forbidden) {
    if (source.includes(needle)) failures.push(`${relative(root, join(root, file))}: ${description}`);
  }
}
if (failures.length) throw new Error(`Memory removal contract failed:\n${failures.join("\n")}`);
console.log(`Memory removal contract verified across ${files.length} runtime source files.`);
