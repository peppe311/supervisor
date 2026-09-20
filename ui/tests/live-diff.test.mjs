import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import { liveDiff, liveDiffLabel } from "../src/lib/live-diff.ts";

test("live diff accepts bounded owner-scoped counters and describes their state", () => {
  const active = liveDiff({ additions: 300, deletions: 400, fileCount: 7, active: true });
  assert.deepEqual(active, { additions: 300, deletions: 400, fileCount: 7, active: true });
  assert.equal(
    liveDiffLabel(active),
    "Live code changes: 300 lines added, 400 lines removed across 7 files",
  );
  const complete = liveDiff({ additions: 1, deletions: 0, fileCount: 1, active: false });
  assert.equal(liveDiffLabel(complete), "Last turn code changes: 1 line added, 0 lines removed across 1 file");
});

test("live diff rejects malformed values and hides an empty completed turn", () => {
  for (const value of [
    null,
    {},
    { additions: -1, deletions: 0, fileCount: 1, active: true },
    { additions: 1.5, deletions: 0, fileCount: 1, active: true },
    { additions: Number.MAX_SAFE_INTEGER + 1, deletions: 0, fileCount: 1, active: true },
    { additions: 0, deletions: 0, fileCount: 0, active: false },
  ]) assert.equal(liveDiff(value), null);
  assert.deepEqual(
    liveDiff({ additions: 0, deletions: 0, fileCount: 0, active: true }),
    { additions: 0, deletions: 0, fileCount: 0, active: true },
  );
});

test("the shared live diff component is mounted by main and graph composers", () => {
  const main = fs.readFileSync(new URL("../src/components/Composer.svelte", import.meta.url), "utf8");
  const graph = fs.readFileSync(new URL("../src/components/AgentGraphCard.svelte", import.meta.url), "utf8");
  const component = fs.readFileSync(new URL("../src/components/LiveDiffStats.svelte", import.meta.url), "utf8");
  assert.match(main, /<LiveDiffStats\s*\/>/);
  assert.match(graph, /<LiveDiffStats \{eventTarget\} \{owner\} \/>/);
  assert.match(component, /central-agent:conversation-state/);
  assert.match(component, /central-agent:app-server-conversation/);
  assert.match(component, /central-agent:conversation-key/);
  assert.match(component, /\+\{stats\.additions\}/);
  assert.match(component, /−\{stats\.deletions\}/);
});
