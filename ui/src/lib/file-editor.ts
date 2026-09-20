import { basicSetup } from "codemirror";
import { EditorState, Prec, type Extension } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { indentWithTab } from "@codemirror/commands";
import { HighlightStyle, StreamLanguage, syntaxHighlighting } from "@codemirror/language";
import { tags } from "@lezer/highlight";
import { rust } from "@codemirror/lang-rust";
import { cpp } from "@codemirror/lang-cpp";
import { javascript } from "@codemirror/lang-javascript";
import { json } from "@codemirror/lang-json";
import { html } from "@codemirror/lang-html";
import { css } from "@codemirror/lang-css";
import { python } from "@codemirror/lang-python";
import { markdown } from "@codemirror/lang-markdown";
import { toml } from "@codemirror/legacy-modes/mode/toml";
import { yaml } from "@codemirror/legacy-modes/mode/yaml";
import { shell } from "@codemirror/legacy-modes/mode/shell";

export interface EditorDocument {
  id: string; root: string; path: string; content: string; sha256: string;
  lineEnding: string; bom: boolean; readOnly: boolean; dirty: boolean; version: number; iconKey: string;
}
export interface FileEditorState { documents: EditorDocument[]; activeId: string | null; status: string }
let receive: ((state: FileEditorState) => void) | undefined;
export function updateFileEditor(state: FileEditorState): void { receive?.(state); }
export function subscribeEditor(listener: typeof receive): () => void {
  receive = listener;
  return () => { if (receive === listener) receive = undefined; };
}
export function editorAction(action: Record<string, unknown>): void {
  document.dispatchEvent(new CustomEvent("central-agent:editor", { detail: action }));
}

export function languageFor(path: string): Extension {
  const name = path.split(/[\\/]/).pop()?.toLowerCase() ?? "";
  const ext = name.split(".").pop();
  if (ext === "rs") return rust();
  if (["c", "h", "cpp", "hpp", "cc", "cxx", "hxx"].includes(ext ?? "")) return cpp();
  if (["js", "jsx", "mjs", "cjs", "ts", "tsx"].includes(ext ?? "")) return javascript({ typescript: ext === "ts" || ext === "tsx", jsx: ext === "jsx" || ext === "tsx" });
  if (ext === "json") return json();
  if (["html", "htm", "svelte", "vue"].includes(ext ?? "")) return html();
  if (ext === "css") return css();
  if (ext === "py") return python();
  if (ext === "md") return markdown();
  if (ext === "toml" || name === "cargo.lock") return StreamLanguage.define(toml);
  if (ext === "yaml" || ext === "yml") return StreamLanguage.define(yaml);
  if (ext === "sh" || name === ".bashrc") return StreamLanguage.define(shell);
  return [];
}

const bufferSizes = new WeakMap<EditorDocument, { text: string; bytes: number }>();
export function bufferFits(doc: EditorDocument, text: string, documents: EditorDocument[]): boolean {
  const encoder = new TextEncoder();
  const encoded = encoder.encode(doc.lineEnding === "CRLF" ? text.replaceAll("\n", "\r\n") : text).length + (doc.bom ? 3 : 0);
  const total = documents.reduce((bytes, item) => {
    if (item.id === doc.id) return bytes + encoder.encode(text).length;
    let cached = bufferSizes.get(item);
    if (!cached || cached.text !== item.content) {
      cached = { text: item.content, bytes: encoder.encode(item.content).length };
      bufferSizes.set(item, cached);
    }
    return bytes + cached.bytes;
  }, 0);
  return !text.includes("\0") && encoded <= 1_000_000 && total <= 16_000_000;
}

const theme = EditorView.theme({
  "&": { height: "100%", backgroundColor: "var(--ca-app-background)", color: "var(--ca-text)", fontSize: "var(--ca-type-title)" },
  ".cm-scroller": { overflow: "auto", fontFamily: "var(--ca-font-mono)", lineHeight: "var(--ca-leading-body)" },
  ".cm-content": { caretColor: "var(--ca-text)", padding: "var(--ca-space-2) 0" },
  ".cm-gutters": { backgroundColor: "var(--ca-app-background)", color: "var(--ca-muted)", border: "none" },
  ".cm-activeLine, .cm-activeLineGutter": { backgroundColor: "var(--ca-surface-2)" },
  ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--ca-text)" },
  "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection": { backgroundColor: "var(--ca-accent-soft)" },
  "&.cm-focused": { outline: "none" },
  ".cm-panels, .cm-tooltip": { backgroundColor: "var(--ca-surface)", color: "var(--ca-text)", border: "none", fontFamily: "var(--ca-font-body)" },
  ".cm-search": { display: "flex", flexWrap: "wrap", gap: "var(--ca-space-2)", padding: "var(--ca-space-2)" },
  ".cm-textfield, .cm-button": { background: "var(--ca-input)", color: "var(--ca-text)", border: "1px solid var(--ca-border)", borderRadius: "var(--ca-radius-small)", fontSize: "var(--ca-type-body)" },
  ".cm-matchingBracket": { backgroundColor: "var(--ca-accent-soft)", outline: "1px solid var(--ca-border-strong)" },
  ".cm-searchMatch, .cm-searchMatch-selected, .cm-selectionMatch": { backgroundColor: "var(--ca-accent-soft)", outline: "1px solid var(--ca-border-strong)" },
});
const highlighting = syntaxHighlighting(HighlightStyle.define([
  { tag: [tags.keyword, tags.operatorKeyword, tags.modifier], color: "var(--ca-text)", fontWeight: "700" },
  { tag: [tags.string, tags.number, tags.bool, tags.regexp], color: "var(--ca-muted)" },
  { tag: [tags.comment, tags.meta], color: "var(--ca-muted)", fontStyle: "italic" },
  { tag: [tags.typeName, tags.className, tags.function(tags.variableName)], color: "var(--ca-text)", fontWeight: "600" },
  { tag: tags.invalid, textDecoration: "underline wavy" },
]));

export function createCodeState(doc: EditorDocument, callbacks: {
  save: () => void; changed: (text: string) => void; validate: (text: string) => boolean;
  cursor: (line: number, column: number, start: number, end: number) => void;
}): EditorState {
  return EditorState.create({ doc: doc.content, extensions: [
    basicSetup, languageFor(doc.path), theme, highlighting,
    EditorState.readOnly.of(doc.readOnly), EditorView.editable.of(!doc.readOnly),
    EditorView.contentAttributes.of({ "aria-label": "Source code: " + doc.path, spellcheck: "false", autocorrect: "off", autocapitalize: "off" }),
    Prec.highest(keymap.of([
      { key: "Mod-s", run: () => { callbacks.save(); return true; }, stopPropagation: true },
      indentWithTab,
    ])),
    EditorState.transactionFilter.of(transaction => transaction.docChanged && !callbacks.validate(transaction.newDoc.toString()) ? [] : transaction),
    EditorView.updateListener.of(update => {
      if (update.docChanged) callbacks.changed(update.state.doc.toString());
      if (update.selectionSet || update.docChanged) {
        const head = update.state.selection.main.head, line = update.state.doc.lineAt(head);
        callbacks.cursor(line.number, head - line.from + 1, update.state.selection.main.from, update.state.selection.main.to);
      }
    }),
  ] });
}
