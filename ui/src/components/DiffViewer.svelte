<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { EditorView } from "@codemirror/view";
  import { createDiffState, findDiffMatches, nextDiffMatch, setDiffSearchQuery, type DiffMatch } from "../lib/diff-search";

  let { content = "", identity = "", inline = false }: { content?: string; identity?: string; inline?: boolean } = $props();
  let query = $state("");
  let current = $state(-1), count = $state(0), diffLine = $state(0);
  let input: HTMLInputElement;
  let code: HTMLDivElement;
  let view: EditorView | undefined;
  let source = untrack(() => content), sourceIdentity = untrack(() => identity);
  let matches: DiffMatch[] = [];

  function focusSearch(): void { input?.focus(); input?.select(); }
  function selectMatch(scroll: boolean): void {
    const match = matches[current];
    if (!view || !match) { diffLine = 0; return; }
    diffLine = view.state.doc.lineAt(match.from).number;
    view.dispatch({
      selection: { anchor: match.from, head: match.to },
      effects: scroll ? EditorView.scrollIntoView(match.from, { y: "center", x: "nearest" }) : [],
    });
  }
  function searchChanged(): void {
    if (!view) return;
    matches = findDiffMatches(view.state.doc, query);
    count = matches.length; current = count ? 0 : -1;
    view.dispatch({ effects: setDiffSearchQuery.of(query) });
    if (!count) view.dispatch({ selection: { anchor: view.state.selection.main.from } });
    selectMatch(true);
  }
  function navigate(direction: number): void {
    current = nextDiffMatch(current, count, direction);
    selectMatch(true);
  }
  function clearSearch(): void { query = ""; searchChanged(); focusSearch(); }
  function searchKey(event: KeyboardEvent): void {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f") {
      event.preventDefault(); event.stopPropagation(); focusSearch();
    } else if (event.key === "Enter" || event.key === "F3") {
      event.preventDefault(); event.stopPropagation(); navigate(event.shiftKey ? -1 : 1);
    } else if (event.key === "Escape") {
      event.preventDefault(); event.stopPropagation(); clearSearch();
    }
  }

  export function update(text: string, key = ""): void {
    if (text === source && key === sourceIdentity) return;
    const newFile = key !== sourceIdentity;
    source = text; sourceIdentity = key;
    if (!view) return;
    const top = view.scrollDOM.scrollTop, left = view.scrollDOM.scrollLeft;
    const previousMatch = matches[current]?.from;
    const anchor = Math.min(view.state.selection.main.from, text.length);
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text }, selection: { anchor } });
    matches = findDiffMatches(view.state.doc, query); count = matches.length;
    const retained = newFile ? -1 : matches.findIndex(match => match.from === previousMatch);
    current = !count ? -1 : retained >= 0 ? retained : 0;
    selectMatch(newFile);
    if (!newFile) { view.scrollDOM.scrollTop = top; view.scrollDOM.scrollLeft = left; }
    else if (!count) { view.scrollDOM.scrollTop = 0; view.scrollDOM.scrollLeft = 0; }
  }

  onMount(() => {
    view = new EditorView({ parent: code, state: createDiffState(source, focusSearch, navigate) });
    return () => view?.destroy();
  });
</script>

<section class="diff-view" class:inline aria-label="Searchable diff">
  <div class="diff-search" role="search" aria-label="Search this diff">
    <input bind:this={input} type="search" placeholder="Find in this diff…" aria-label="Find in this diff" spellcheck="false" value={query} oninput={event => { query = event.currentTarget.value; searchChanged(); }} onkeydown={searchKey} />
    <span class="diff-results" data-diff-results role="status" aria-live="polite">{query ? count ? `${current + 1} / ${count}` : "No matches" : ""}</span>
    <button type="button" aria-label="Previous match" title="Previous match (Shift+Enter)" disabled={!count} onclick={() => navigate(-1)}>↑</button>
    <button type="button" aria-label="Next match" title="Next match (Enter)" disabled={!count} onclick={() => navigate(1)}>↓</button>
    <button type="button" aria-label="Clear diff search" title="Clear search (Escape)" disabled={!query} onclick={clearSearch}>×</button>
    {#if diffLine}<span class="diff-location" data-diff-location>Diff line {diffLine}</span>{/if}
  </div>
  <div bind:this={code} class="diff-code"></div>
</section>

<style>
  .diff-view { display: flex; flex-direction: column; flex: 1; height: 100%; min-width: 0; min-height: 0; overflow: hidden; padding: 0; border: 0; box-shadow: none; border-radius: var(--ca-radius-large); background: var(--ca-app-background); color: var(--ca-text); font: var(--ca-type-body)/var(--ca-leading-body) var(--ca-font-body); }
  .diff-search { display: flex; flex: 0 0 auto; align-items: center; flex-wrap: wrap; gap: var(--ca-space-1); padding: var(--ca-space-2); background: var(--ca-app-background); }
  input { flex: 1 1 14ch; min-width: 0; width: 14ch; padding: var(--ca-space-2); border: none; border-radius: var(--ca-radius-small); background: var(--ca-surface-2); color: var(--ca-text); font: inherit; }
  input::placeholder { color: var(--ca-muted); }
  input::-webkit-search-cancel-button { display: none; }
  button { border: none; border-radius: var(--ca-radius-small); background: transparent; color: var(--ca-text); padding: var(--ca-space-2); font: inherit; cursor: pointer; }
  button:hover:not(:disabled) { background: var(--ca-surface-2); }
  button:disabled { opacity: .45; cursor: default; }
  input:focus-visible, button:focus-visible { outline: 1px solid var(--ca-border-strong); box-shadow: var(--ca-focus-ring); }
  .diff-results { color: var(--ca-muted); font-size: var(--ca-type-label); min-width: 5ch; text-align: center; }
  .diff-location { color: var(--ca-muted); font-size: var(--ca-type-caption); white-space: nowrap; }
  .diff-code { flex: 1; min-height: 0; min-width: 0; overflow: hidden; border-radius: var(--ca-radius-medium); }
  .inline { height: auto; }
  .inline .diff-code { flex: none; }
  .inline :global(.cm-editor) { height: auto; }
  .inline :global(.cm-scroller) { max-height: min(52vh, 32rem); }
</style>
