import assert from "node:assert/strict";
import test from "node:test";
import { Text, EditorState } from "@codemirror/state";
import { diffLineKind, findDiffMatches, nextDiffMatch, createDiffState, setDiffSearchQuery, diffSearchHighlights } from "../src/lib/diff-search.ts";

test("diff search is literal, case-insensitive and returns every occurrence", () => {
  const doc = Text.of(["+foo(bar) foo(bar)", "-FOO(BAR)", " unchanged"]);
  const matches = findDiffMatches(doc, "foo(bar)");
  assert.equal(matches.length, 3);
  assert.deepEqual(matches.map(match => doc.sliceString(match.from, match.to)), ["foo(bar)", "foo(bar)", "FOO(BAR)"]);
  assert.deepEqual(findDiffMatches(doc, ""), []);
  assert.deepEqual(findDiffMatches(doc, "missing"), []);
});

test("Unicode and long lines retain exact source offsets", () => {
  const doc = Text.of(["+🦀 città", "+" + "x".repeat(12000) + " CITTÀ"]);
  const matches = findDiffMatches(doc, "città");
  assert.equal(matches.length, 2);
  assert.equal(doc.lineAt(matches[1].from).number, 2);
  assert.equal(doc.sliceString(matches[0].from, matches[0].to), "città");
  assert.equal(findDiffMatches(doc, "🦀")[0].to - findDiffMatches(doc, "🦀")[0].from, 2);
});

test("next and previous wrap safely, including no results", () => {
  assert.equal(nextDiffMatch(-1, 3, 1), 0);
  assert.equal(nextDiffMatch(-1, 3, -1), 2);
  assert.equal(nextDiffMatch(2, 3, 1), 0);
  assert.equal(nextDiffMatch(0, 3, -1), 2);
  assert.equal(nextDiffMatch(0, 0, 1), -1);
});

test("file headers are not additions or deletions and diff remains read-only", () => {
  assert.equal(diffLineKind("+++ b/main.rs"), "context");
  assert.equal(diffLineKind("--- a/main.rs"), "context");
  assert.equal(diffLineKind("+new"), "addition");
  assert.equal(diffLineKind("-old"), "deletion");
  assert.equal(diffLineKind("@@ -1 +1 @@"), "hunk");
  const state = createDiffState("+first\n-second", () => {}, () => {});
  assert.equal(state.facet(EditorState.readOnly), true);
  assert.equal(state.doc.toString(), "+first\n-second");
});

function marks(state) {
  const result = [];
  state.field(diffSearchHighlights).decorations.between(0, state.doc.length, (from, to, value) => {
    result.push({ text: state.doc.sliceString(from, to), from, to, selected: value.spec.class.includes("cm-searchMatch-selected") });
  });
  return result;
}

test("phrases stay highlighted without a native search panel, and navigation marks the active result", () => {
  let state = createDiffState("+find this phrase\n-find THIS phrase", () => {}, () => {});
  state = state.update({ effects: setDiffSearchQuery.of("this phrase") }).state;
  const matches = marks(state);
  assert.deepEqual(matches.map(match => match.text), ["this phrase", "THIS phrase"]);
  const { from, to } = matches[1];
  state = state.update({ selection: { anchor: from, head: to } }).state;
  assert.deepEqual(marks(state).map(match => match.selected), [false, true]);
  state = state.update({ selection: { anchor: 0 } }).state;
  assert.equal(marks(state).length, 2, "moving focus/selection must not hide matches");
  state = state.update({}).state;
  assert.equal(marks(state).length, 2, "unrelated updates must retain marks");
  state = state.update({ effects: setDiffSearchQuery.of("") }).state;
  assert.deepEqual(marks(state), []);
});

test("highlight ranges recompute on streamed text and clear on a no-result search", () => {
  let state = createDiffState("+needle", () => {}, () => {});
  state = state.update({ effects: setDiffSearchQuery.of("needle") }).state;
  state = state.update({ changes: { from: 0, insert: " context\n" } }).state;
  assert.equal(marks(state)[0].from, 10);
  state = state.update({ changes: { from: state.doc.length, insert: "\n+NEEDLE" } }).state;
  assert.equal(marks(state).length, 2);
  state = state.update({ effects: setDiffSearchQuery.of("not found") }).state;
  assert.deepEqual(marks(state), []);
});
