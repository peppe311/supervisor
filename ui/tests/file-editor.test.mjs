import assert from "node:assert/strict";
import test from "node:test";
import { undo, redo } from "@codemirror/commands";
import { syntaxTree } from "@codemirror/language";
import { EditorState } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { bufferFits, createCodeState, languageFor } from "../src/lib/file-editor.ts";

const doc = { id: "fixture", root: "C:/fixture", path: "main.rs", content: "fn main() {}\n", sha256: "fixture", lineEnding: "LF", bom: false, readOnly: false, dirty: false, version: 0, iconKey: "rust" };
const callbacks = { save() {}, changed() {}, validate: text => bufferFits(doc, text, [doc]), cursor() {} };

test("editor callbacks report the changed buffer before UTF-16 selection offsets", () => {
  const calls=[];
  const state=createCodeState({...doc,content:"a🦀b"},{...callbacks,changed:text=>calls.push(["change",text]),cursor:(...position)=>calls.push(["selection",...position])});
  const transaction=state.update({changes:{from:0,to:1,insert:"c"},selection:{anchor:1,head:3}});
  // Exercise the installed listener without mounting or visually testing a WebView.
  for(const listener of state.facet(EditorView.updateListener)) listener({docChanged:true,selectionSet:true,state:transaction.state});
  assert.deepEqual(calls,[["change","c🦀b"],["selection",1,4,1,3]]);
});

test("UTF-8 and CRLF byte accounting rejects edits that cannot be saved", () => {
  assert.equal(bufferFits(doc, "a".repeat(1_000_000), [doc]), true);
  assert.equal(bufferFits(doc, "🦀".repeat(250_001), [doc]), false);
  const crlf = { ...doc, lineEnding: "CRLF", bom: true };
  assert.equal(bufferFits(crlf, "\n".repeat(499_999), [crlf]), false);
  assert.equal(bufferFits(doc, "bad\0text", [doc]), false);
  const full = Array.from({ length: 16 }, (_, index) => ({ ...doc, id: String(index), content: "a".repeat(1_000_000) }));
  assert.equal(bufferFits(doc, "x", [...full, doc]), false);
});

test("the editor rejects oversize transactions without truncating the buffer", () => {
  const state = createCodeState(doc, callbacks);
  const transaction = state.update({ changes: { from: 0, insert: "x".repeat(1_000_001) } });
  assert.equal(transaction.docChanged, false);
  assert.equal(transaction.newDoc.toString(), doc.content);
});

test("undo and redo retain exact text in independent file states", () => {
  let state = createCodeState(doc, callbacks);
  const untouched = createCodeState({ ...doc, id: "other", content: "second file" }, callbacks);
  state = state.update({ changes: { from: 0, insert: "// edit\n" } }).state;
  const target = { get state() { return state; }, dispatch: transaction => { state = transaction.state; } };
  assert.equal(undo(target), true);
  assert.equal(state.doc.toString(), doc.content);
  assert.equal(redo(target), true);
  assert.equal(state.doc.toString(), "// edit\n" + doc.content);
  assert.equal(untouched.doc.toString(), "second file");
});

test("source grammars are local and unknown extensions use plain text", () => {
  for (const [path, source] of [["main.rs", "fn main() {}"], ["main.cpp", "int main() {}"], ["package.json", '{"name":"fixture"}'], ["Cargo.toml", '[package]\nname="fixture"'], ["app.ts", "const ready: boolean = true"]]) {
    const state = EditorState.create({ doc: source, extensions: [languageFor(path)] });
    assert.ok(syntaxTree(state).length > 0, path);
  }
  const plain = EditorState.create({ doc: "fixture", extensions: [languageFor("unknown.custom")] });
  assert.equal(syntaxTree(plain).length, 0);
});
