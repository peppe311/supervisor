import { EditorState, StateEffect, StateField, type Text } from "@codemirror/state";
import { Decoration, EditorView, keymap, lineNumbers, type DecorationSet } from "@codemirror/view";
import { SearchQuery } from "@codemirror/search";

export interface DiffMatch { from: number; to: number }
export type DiffLineKind = "addition" | "deletion" | "hunk" | "context";

export function diffLineKind(line: string): DiffLineKind {
  if (line.startsWith("+") && !line.startsWith("+++")) return "addition";
  if (line.startsWith("-") && !line.startsWith("---")) return "deletion";
  return line.startsWith("@@") ? "hunk" : "context";
}

export function findDiffMatches(doc: Text, text: string): DiffMatch[] {
  if (!text) return [];
  const cursor = new SearchQuery({ search: text, caseSensitive: false, literal: true }).getCursor(doc);
  const matches: DiffMatch[] = [];
  for (let result = cursor.next(); !result.done; result = cursor.next()) matches.push(result.value);
  return matches;
}

export function nextDiffMatch(current: number, count: number, direction: number): number {
  if (!count) return -1;
  if (current < 0) return direction < 0 ? count - 1 : 0;
  return (current + direction + count) % count;
}

function lineDecorations(doc: Text): DecorationSet {
  const decorations = [];
  for (let number = 1; number <= doc.lines; number++) {
    const line = doc.line(number), kind = diffLineKind(line.text);
    if (kind !== "context") decorations.push(Decoration.line({ class: "diff-code-" + kind }).range(line.from));
  }
  return Decoration.set(decorations);
}
const diffLines = StateField.define<DecorationSet>({
  create: state => lineDecorations(state.doc),
  update: (value, transaction) => transaction.docChanged ? lineDecorations(transaction.newDoc) : value,
  provide: field => EditorView.decorations.from(field),
});

export const setDiffSearchQuery = StateEffect.define<string>();
// The built-in search highlighter only runs while CodeMirror's own panel is
// open. Our Svelte search bar needs persistent marks independent of that panel
// and of which element currently holds keyboard focus.
export const diffSearchHighlights = StateField.define<{
  query: string; matches: DiffMatch[]; decorations: DecorationSet;
}>({
  create: () => ({ query: "", matches: [], decorations: Decoration.none }),
  update: (value, transaction) => {
    let query = value.query;
    for (const effect of transaction.effects) if (effect.is(setDiffSearchQuery)) query = effect.value;
    const changed = query !== value.query || transaction.docChanged;
    if (!changed && !transaction.selection) return value;
    const matches = changed ? findDiffMatches(transaction.newDoc, query) : value.matches;
    const selected = transaction.newSelection.main;
    const decorations = Decoration.set(matches.map(match => Decoration.mark({
      class: "cm-searchMatch" + (selected.from === match.from && selected.to === match.to ? " cm-searchMatch-selected" : ""),
    }).range(match.from, match.to)));
    return { query, matches, decorations };
  },
  provide: field => EditorView.decorations.from(field, value => value.decorations),
});

export function createDiffState(source: string, focusSearch: () => void, navigate: (direction: number) => void): EditorState {
  return EditorState.create({ doc: source, extensions: [
    EditorState.readOnly.of(true), EditorView.editable.of(false),
    EditorView.contentAttributes.of({ "aria-label": "Read-only unified diff", tabindex: "0" }),
    lineNumbers(), diffLines, diffSearchHighlights,
    keymap.of([
      { key: "Mod-f", run: () => { focusSearch(); return true; }, stopPropagation: true },
      { key: "F3", run: () => { navigate(1); return true; }, stopPropagation: true },
      { key: "Shift-F3", run: () => { navigate(-1); return true; }, stopPropagation: true },
    ]),
    EditorView.theme({
      "&": { height: "100%", backgroundColor: "var(--ca-app-background)", color: "var(--ca-text)", fontSize: "calc(var(--ca-type-diff-code) + var(--ca-chat-font-offset))" },
      "&.cm-focused": { outline: "none" },
      ".cm-scroller": { overflow: "auto", fontFamily: "var(--ca-font-mono)", lineHeight: "var(--ca-leading-body)" },
      ".cm-gutters": { backgroundColor: "var(--ca-app-background)", color: "var(--ca-muted)", border: "none" },
      ".cm-content": { padding: "var(--ca-space-2) 0" },
      ".cm-line": { padding: "0 var(--ca-space-3)" },
      ".diff-code-addition": { backgroundColor: "color-mix(in srgb, var(--ca-diff-addition) 12%, transparent)", color: "var(--ca-diff-addition)" },
      ".diff-code-deletion": { backgroundColor: "color-mix(in srgb, var(--ca-diff-deletion) 12%, transparent)", color: "var(--ca-diff-deletion)" },
      ".diff-code-hunk": { color: "var(--ca-muted)" },
      ".cm-searchMatch": { backgroundColor: "var(--ca-accent-soft)", color: "var(--ca-text)", outline: "1px solid var(--ca-border-strong)" },
      ".cm-searchMatch-selected": { backgroundColor: "var(--ca-accent)", color: "var(--ca-on-accent)", outline: "1px solid var(--ca-accent)" },
      ".cm-content ::selection, .cm-content::selection": { backgroundColor: "var(--ca-accent-soft)", color: "var(--ca-text)" },
    }),
  ] });
}
