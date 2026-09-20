import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDirectory, "..");
const files = fs
  .readdirSync(path.join(projectRoot, "assets"), { withFileTypes: true })
  .filter((entry) => entry.isFile() && entry.name.endsWith(".html"))
  .map((entry) => path.join("assets", entry.name))
  .sort();
let scriptCount = 0;

for (const relativePath of files) {
  const source = fs.readFileSync(path.join(projectRoot, relativePath), "utf8");
  const identifiers = [...source.matchAll(/\bid\s*=\s*["']([^"']+)["']/gi)].map((match) => match[1]);
  const duplicates = [...new Set(identifiers.filter((id, index) => identifiers.indexOf(id) !== index))];
  if (duplicates.length > 0) {
    throw new Error(`${relativePath}: duplicate element id(s): ${duplicates.join(", ")}`);
  }
  const scripts = [...source.matchAll(/<script(?:\s[^>]*)?>([\s\S]*?)<\/script>/gi)];
  for (const [index, match] of scripts.entries()) {
    try {
      // Parse only. The WebView bridge and DOM are intentionally not executed here.
      new Function(match[1]);
      scriptCount += 1;
    } catch (error) {
      throw new Error(`${relativePath} inline script ${index + 1}: ${error.message}`);
    }
  }
}

process.stdout.write(`Inline JavaScript verification passed (${scriptCount} scripts).\n`);
