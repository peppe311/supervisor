<script lang="ts">
  import { onMount, tick } from "svelte";
  import { EditorView } from "@codemirror/view";
  import type { EditorState } from "@codemirror/state";
  import { openSearchPanel } from "@codemirror/search";
  import { bufferFits, createCodeState, editorAction, subscribeEditor, type EditorDocument } from "../lib/file-editor";

  let documents: EditorDocument[] = $state([]);
  let activeId: string | null = $state(null);
  let status = $state("");
  let line = $state(1), column = $state(1);
  let host: HTMLDivElement;
  let bar: HTMLDivElement;
  let dialog: HTMLDialogElement;
  let confirm: { id: string; type: "close" | "reload"; name: string } | null = $state(null);
  let view: EditorView | undefined;
  let viewedId: string | null = null;
  let codeWasVisible = false;
  let chatScroll = { top: 0, follow: false };
  const states = new Map<string, { state: EditorState; top: number; left: number }>();
  const savedContents = new Map<string, string>();
  let active = $derived(documents.find(doc => doc.id === activeId));

  function fileIcon(node: HTMLElement, key: string): void {
    const icon = (window as Window & { timeMachineFileIcon?: (key: string) => HTMLElement }).timeMachineFileIcon?.(key || "file");
    if (icon) node.replaceChildren(icon);
  }

  function save(id = activeId): void {
    const doc = documents.find(doc => doc.id === id);
    if (!doc || doc.readOnly) return;
    status = "Saving…";
    editorAction({ type: "save", id, content: doc.content, version: ++doc.version });
    if (view && viewedId === id) selection(doc.id, view.state.selection.main.from, view.state.selection.main.to);
  }
  function selection(id:string,start:number,end:number):void {
    const doc=documents.find(doc=>doc.id===id);
    if(doc)editorAction({type:"selection",id,version:doc.version,start,end});
  }
  function request(type: "close" | "reload", doc: EditorDocument): void {
    if (doc.dirty) { confirm = { id: doc.id, type, name: doc.path }; dialog.showModal(); }
    else editorAction({ type, id: doc.id });
  }
  function cancel(): void { dialog.close(); confirm = null; }
  function discard(): void {
    if (confirm) editorAction({ type: confirm.type, id: confirm.id, discard: true });
    cancel();
  }
  function activate(id: string | null): void {
    activeId = id;
    void showDocument();
    editorAction({ type: "activate", id });
  }
  function remember(): void {
    if (view && viewedId && documents.some(doc => doc.id === viewedId) && view.scrollDOM.clientHeight) states.set(viewedId, { state: view.state, top: view.scrollDOM.scrollTop, left: view.scrollDOM.scrollLeft });
  }
  async function showDocument(reset = false): Promise<void> {
    remember();
    const chat = document.getElementById("chat-messages");
    const enteringCode = !!activeId && !codeWasVisible;
    const leavingCode = !activeId && codeWasVisible;
    if (enteringCode && chat) chatScroll = { top: chat.scrollTop, follow: chat.scrollHeight - chat.scrollTop - chat.clientHeight < 48 };
    codeWasVisible = !!activeId;
    bar?.closest<HTMLElement>(".conversation-section")?.classList.toggle("file-editor-active", !!activeId);
    await tick();
    if (leavingCode && chat) chat.scrollTop = chatScroll.follow ? chat.scrollHeight : chatScroll.top;
    const doc = documents.find(item => item.id === activeId);
    if (!doc) {
      if (viewedId && !documents.some(item => item.id === viewedId)) { view?.destroy(); view = undefined; viewedId = null; }
      return;
    }
    const saved = states.get(doc.id);
    const makeState = () => createCodeState(doc, {
      save: () => save(doc.id),
      validate: text => {
        const current = documents.find(item => item.id === doc.id);
        const valid = !!current && bufferFits(current, text, documents);
        if (!valid) status = "Edit not applied: text files support up to 1 MB each and 16 MB across open tabs. Save and close other tabs if needed.";
        return valid;
      },
      changed: text => {
        const current = documents.find(item => item.id === doc.id);
        if (!current) return;
        current.content = text; current.dirty = savedContents.get(current.id) !== text; current.version++;
        status = "";
        editorAction({ type: "change", id: current.id, content: text, version: current.version });
      },
      cursor: (l, c, start, end) => { line = l; column = c; selection(doc.id,start,end); },
    });
    // Keep the live view for acknowledgments; saved states retain undo/selection across tabs.
    if (viewedId === doc.id && view && !reset && view.state.doc.toString() === doc.content) return;
    const state = !reset && saved?.state.doc.toString() === doc.content ? saved.state : makeState();
    if (view) view.setState(state);
    else view = new EditorView({ parent: host, state });
    viewedId = doc.id;
    view.scrollDOM.scrollTop = reset ? 0 : saved?.top ?? 0;
    view.scrollDOM.scrollLeft = reset ? 0 : saved?.left ?? 0;
    const head = view.state.selection.main.head, currentLine = view.state.doc.lineAt(head);
    line = currentLine.number; column = head - currentLine.from + 1;
    selection(doc.id,view.state.selection.main.from,view.state.selection.main.to);
    view.requestMeasure();
  }

  onMount(() => {
    const unsubscribe = subscribeEditor(state => {
      const before = documents.find(doc => doc.id === state.activeId);
      const incoming = state.documents.find(doc => doc.id === state.activeId);
      const reset = !!before && !!incoming && (incoming.version > before.version || incoming.readOnly !== before.readOnly);
      documents = state.documents.map(doc => {
        if (!doc.dirty) savedContents.set(doc.id, doc.content);
        const local = documents.find(item => item.id === doc.id);
        if (local && local.version > doc.version) {
          local.dirty = savedContents.get(local.id) !== local.content;
          return local;
        }
        return doc;
      });
      for (const id of states.keys()) if (!documents.some(doc => doc.id === id)) states.delete(id);
      for (const id of savedContents.keys()) if (!documents.some(doc => doc.id === id)) savedContents.delete(id);
      activeId = state.activeId;
      status = state.status;
      void showDocument(reset);
    });
    editorAction({ type: "ready" });
    return () => { unsubscribe(); view?.destroy(); };
  });
</script>

<div bind:this={bar} class="file-editor-tabs" class:empty={documents.length === 0} aria-label="Chat and open files">
  <button type="button" class:selected={!activeId} aria-pressed={!activeId} onclick={() => activate(null)}>Chat</button>
  {#each documents as doc (doc.id)}
    <div class="file-editor-tab" class:selected={activeId === doc.id}>
      <button type="button" class="file-name" title={doc.root + " / " + doc.path} aria-pressed={activeId === doc.id} onclick={() => activate(doc.id)}>
        <span class="file-icon" aria-hidden="true" use:fileIcon={doc.iconKey}></span>{doc.path.split(/[\\/]/).pop()}<span aria-label={doc.dirty ? "Unsaved changes" : ""}>{doc.dirty ? "●" : ""}</span>
      </button>
      <button type="button" class="close-file" aria-label={"Close " + doc.path} onclick={() => request("close", doc)}>×</button>
    </div>
  {/each}
</div>
<div class="file-editor-surface" class:inactive={!active}>
  <div class="file-editor-toolbar">
    <span class="file-path" title={active ? active.root + " / " + active.path : ""}>{active?.path}</span>
    <button type="button" disabled={!active || active.readOnly} onclick={() => save()} title="Save (Ctrl+S)">Save</button>
    <button type="button" disabled={!active} onclick={() => active && request("reload", active)}>Reload</button>
    <button type="button" disabled={!active} onclick={() => view && openSearchPanel(view)} title="Find / replace (Ctrl+F)">Find</button>
  </div>
  <div class="file-editor-code" bind:this={host}></div>
  <div class="file-editor-footer">
    <span>{active?.readOnly ? "Read-only" : active?.dirty ? "Unsaved" : "Saved"}</span>
    <span>Ln {line}, Col {column}</span>
    <span>UTF-8{active?.bom ? " BOM" : ""} · {active?.lineEnding}</span>
  </div>
</div>
{#if status}<div class="file-editor-status" role="status">{status}</div>{/if}
<dialog bind:this={dialog} class="file-editor-dialog" aria-labelledby="editor-confirm-title" oncancel={cancel}>
  <h2 id="editor-confirm-title">Discard unsaved changes?</h2>
  <p>{confirm?.name}</p>
  <p>Your unsaved edits will be discarded. The file on disk will not be changed.</p>
  <div><button type="button" onclick={cancel}>Cancel</button><button type="button" onclick={discard}>Discard changes</button></div>
</dialog>

<style>
  .file-editor-tabs { display: flex; flex: 0 0 auto; gap: var(--ca-space-1); overflow-x: auto; padding: var(--ca-space-2); min-width: 0; }
  .file-editor-tabs.empty, .file-editor-surface.inactive { display: none; }
  button { flex: 0 0 auto; border: none; border-radius: var(--ca-radius-small); background: transparent; color: var(--ca-muted); font: inherit; cursor: pointer; padding: var(--ca-space-2) var(--ca-space-3); white-space: nowrap; }
  button:hover, button.selected, .file-editor-tab.selected { background: var(--ca-surface-2); color: var(--ca-text); }
  button:focus-visible { outline: 1px solid var(--ca-border-strong); box-shadow: var(--ca-focus-ring); }
  button:disabled { opacity: .5; cursor: default; }
  .file-editor-tab { display: flex; flex: 0 0 auto; border-radius: var(--ca-radius-small); min-width: 0; }
  .file-name { display: flex; align-items: center; gap: var(--ca-space-2); max-width: 30ch; overflow: hidden; text-overflow: ellipsis; }
  .close-file { padding-inline: var(--ca-space-2); }
  .file-icon { display: inline-flex; align-items: center; flex: 0 0 auto; }
  .file-editor-surface { display: flex; flex: 1 1 0; flex-direction: column; min-width: 0; min-height: 0; overflow: hidden; }
  .file-editor-toolbar, .file-editor-footer { display: flex; align-items: center; flex: 0 0 auto; gap: var(--ca-space-2); padding: var(--ca-space-2); flex-wrap: wrap; }
  .file-path { flex: 1 1 20ch; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: var(--ca-font-mono); color: var(--ca-muted); }
  .file-editor-code { flex: 1 1 0; min-width: 0; min-height: 0; overflow: hidden; }
  .file-editor-footer { justify-content: flex-end; color: var(--ca-muted); font-size: var(--ca-type-caption); }
  .file-editor-footer span:first-child { margin-right: auto; }
  .file-editor-status { flex: 0 0 auto; padding: var(--ca-space-2) var(--ca-space-3); color: var(--ca-text); font-size: var(--ca-type-label); overflow-wrap: anywhere; }
  .file-editor-dialog { max-width: min(54ch, calc(100% - var(--ca-space-8))); border: 1px solid var(--ca-border); border-radius: var(--ca-radius-large); background: var(--ca-surface); color: var(--ca-text); padding: var(--ca-space-6); box-shadow: var(--ca-shadow-float); }
  .file-editor-dialog::backdrop { background: color-mix(in srgb, var(--ca-bg) 65%, transparent); }
  .file-editor-dialog h2 { margin: 0 0 var(--ca-space-4); font-size: var(--ca-type-title); }
  .file-editor-dialog p { overflow-wrap: anywhere; margin-block: var(--ca-space-3); }
  .file-editor-dialog > div { display: flex; justify-content: flex-end; gap: var(--ca-space-2); margin-top: var(--ca-space-6); }
</style>
