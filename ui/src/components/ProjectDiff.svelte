<script lang="ts">
  import { requestId as newRequestId } from "../lib/request-id";
  import { onMount } from "svelte";
  import DiffViewer from "./DiffViewer.svelte";
  import { groupedGitEntries, matchesProjectDiffReply, type GitEntry, type GitSection } from "../lib/project-diff";
  let { eventTarget = window, inputId = "chat-input" }: { eventTarget?: EventTarget; inputId?: string } = $props();
  const titleId = $derived(`${inputId}:project-diff-title`);
  let host: HTMLDivElement;
  let dialog: HTMLDialogElement;
  let viewer: DiffViewer;
  let owner = "", viewId = "", requestId = "", serial = 0, enabled = false;
  let root = $state("");
  let entries = $state<GitEntry[]>([]);
  let query = $state("");
  let groups = $derived(groupedGitEntries(entries, query));
  let unsupportedNames = $state(0);
  let loading = $state(false), loaded = $state(false);
  let error = $state(""), selectedPath = $state(""), selectedSection = $state<GitSection>("unstaged");
  let content = $state("");
  $effect(() => { viewer?.update(content, `${selectedSection}:${selectedPath}`); });

  function send(action: Record<string, unknown>): void {
    requestId = `${viewId}:${++serial}`; loading = action.kind !== "close"; error = "";
    host.dispatchEvent(new CustomEvent("central-agent:project-diff", {bubbles:true,detail:{owner,view_id:viewId,request_id:requestId,action}}));
  }
  function list(): void {
    entries = []; content = ""; selectedPath = ""; loaded = false; unsupportedNames = 0;
    send({kind:"list"});
  }
  function close(): void {
    if (!dialog?.open) return;
    send({kind:"close"}); requestId = ""; viewId = ""; entries = []; content = "";
    dialog.close(); document.getElementById(inputId)?.focus();
  }
  function open(): void {
    if (!enabled || dialog.open) return;
    viewId = `${inputId}:diff:${newRequestId()}:${++serial}`; query = "";
    dialog.showModal(); list();
  }
  function read(entry: GitEntry, section: GitSection): void {
    if (entry.blocked || loading) return;
    selectedPath = entry.path; selectedSection = section; content = "";
    send({kind:"read",path:entry.path,section});
  }
  function icon(node: HTMLElement, key: string): void {
    const graphic = (window as Window & {timeMachineFileIcon?:(key:string)=>HTMLElement}).timeMachineFileIcon?.(key);
    if (graphic) node.replaceChildren(graphic);
  }
  onMount(() => {
    const state = (event: Event) => {
      const detail = (event as CustomEvent).detail;
      if (owner !== detail.owner || root !== (detail.projectRoot || "") || !detail.enabled) close();
      owner = detail.owner; root = detail.projectRoot || ""; enabled = detail.enabled === true;
    };
    const command = (event: Event) => {
      const detail = (event as CustomEvent).detail;
      if (detail.owner === owner && detail.action === "diff") open();
    };
    const result = (event: Event) => {
      const detail = (event as CustomEvent).detail;
      if (!matchesProjectDiffReply(detail,owner,viewId,requestId,dialog.open)) return;
      loading = false; requestId = "";
      if (detail.error) { error = String(detail.error); return; }
      if (detail.result?.kind === "list") {
        entries = detail.result.listing.entries; unsupportedNames = detail.result.listing.unsupportedNames; loaded = true;
      } else if (detail.result?.kind === "read" && detail.result.path === selectedPath && detail.result.section === selectedSection) {
        content = detail.result.content;
      }
    };
    eventTarget.addEventListener("central-agent:slash-state",state);
    eventTarget.addEventListener("central-agent:slash-result",command);
    eventTarget.addEventListener("central-agent:project-diff-result",result);
    return () => {
      close(); eventTarget.removeEventListener("central-agent:slash-state",state);
      eventTarget.removeEventListener("central-agent:slash-result",command);
      eventTarget.removeEventListener("central-agent:project-diff-result",result);
    };
  });
</script>

<div bind:this={host}>
  <dialog bind:this={dialog} aria-labelledby={titleId} oncancel={event => { event.preventDefault(); close(); }}>
    <header>
      <div><h2 id={titleId}>Project changes</h2><p class="path">{root}</p></div>
      <div><button type="button" disabled={loading} onclick={list}>Refresh</button><button type="button" aria-label="Close project changes" onclick={close}>×</button></div>
    </header>
    <p>Current Git changes in this project, including manual and other-agent edits. Not a Time Machine attribution. Read-only; nothing is sent to the model.</p>
    <p>Renames appear as deletion/addition pairs. Submodule worktrees are not scanned; known secret paths and links cannot be previewed.</p>
    {#if error}<p role="alert">{error}</p>{/if}
    {#if loading}<p role="status">Reading local Git changes…</p>{/if}
    {#if unsupportedNames}<p role="status">{unsupportedNames} filename entry/entries could not be represented as UTF-8 and are not shown. Review them with Git directly.</p>{/if}
    {#if loaded && !entries.length}<p role="status">No staged, unstaged or untracked files in this project.</p>{/if}
    <div class="columns">
      <nav aria-label="Changed project files">
        <input type="search" aria-label="Filter changed files" placeholder="Filter files…" bind:value={query} />
        {#if loaded}<p>{entries.length} changed paths</p>{/if}
        {#each groups as group (group.key)}
          <h3>{group.label} <span>{group.entries.length}</span></h3>
          {#each group.entries as entry (entry.path)}
            <button type="button" class="file" disabled={loading || Boolean(entry.blocked)} aria-current={selectedPath === entry.path && selectedSection === group.key ? "true" : undefined} title={entry.blocked || entry.path} onclick={() => read(entry,group.key)}>
              <span class="icon" aria-hidden="true" use:icon={entry.iconKey}></span><span class="filename">{entry.path}</span>
            </button>
            {#if entry.blocked}<p class="blocked">{entry.blocked}</p>{/if}
          {/each}
        {/each}
      </nav>
      <section class="preview" aria-label="Selected file diff">
        {#if selectedPath}<h3 class="path">{selectedPath} · {selectedSection}</h3>{:else}<p>Select a file to read its diff. Other file contents are not loaded into the viewer.</p>{/if}
        {#if selectedPath && !loading && !content && !error}<p>No diff remains in this section. The file may have changed since the list was read; refresh to update it.</p>{/if}
        <DiffViewer bind:this={viewer} />
      </section>
    </div>
  </dialog>
</div>

<style>
  dialog { width: min(78rem, calc(100vw - var(--ca-space-6))); height: min(50rem, calc(100vh - var(--ca-space-6))); box-sizing: border-box; padding: var(--ca-space-6); border: 0; border-radius: var(--ca-radius-large); background: var(--ca-app-background); color: var(--ca-text); box-shadow: var(--ca-shadow-float); overflow: hidden; }
  dialog[open] { display: flex; flex-direction: column; }
  dialog::backdrop { background: color-mix(in srgb, var(--ca-app-background) 75%, transparent); }
  header { display: flex; justify-content: space-between; align-items: flex-start; gap: var(--ca-space-3); }
  header > div:first-child { min-width: 0; }
  header > div:last-child { flex: 0 0 auto; }
  h2, h3 { margin: 0; font: 600 var(--ca-type-title)/var(--ca-leading-body) var(--ca-font-body); }
  h3 { margin: var(--ca-space-3) 0; font-size: var(--ca-type-body); }
  p { margin: var(--ca-space-2) 0; color: var(--ca-muted); font: var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body); overflow-wrap: anywhere; }
  .path { overflow-wrap: anywhere; }
  .columns { display: grid; grid-template-columns: minmax(12rem, 1fr) minmax(0, 3fr); flex: 1; min-height: 0; gap: var(--ca-space-4); margin-top: var(--ca-space-3); }
  nav { min-width: 0; overflow-y: auto; overscroll-behavior: contain; }
  .preview { display: flex; flex-direction: column; min-width: 0; min-height: 0; overflow: hidden; }
  button, input { color: var(--ca-text); background: var(--ca-app-background); border: 0; border-radius: var(--ca-control-radius); padding: var(--ca-space-2); font: var(--ca-type-body)/var(--ca-leading-body) var(--ca-font-body); }
  input { box-sizing: border-box; width: 100%; background: var(--ca-surface-2); }
  button { cursor: pointer; } button:hover:not(:disabled), button[aria-current="true"] { background: var(--ca-surface-2); }
  button:disabled { cursor: default; color: var(--ca-muted); }
  button:focus-visible, input:focus-visible { outline: var(--ca-focus-ring); }
  .file { display: flex; align-items: flex-start; gap: var(--ca-space-2); width: 100%; text-align: left; }
  .filename { overflow-wrap: anywhere; min-width: 0; }
  .icon { flex: 0 0 1.2em; width: 1.2em; height: 1.2em; }
  .blocked { margin-inline: var(--ca-space-2); }
  h3 span { color: var(--ca-muted); font-weight: normal; }
  @media (max-width: 680px) { .columns { grid-template-columns: minmax(0,1fr); grid-template-rows: minmax(0,1fr) minmax(0,2fr); } }
</style>
