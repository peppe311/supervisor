<script lang="ts">
  import { safeSnapshotImage, type ContextDraft } from "../lib/graph-contexts";
  let {draft,disabled=false,action}: {draft:ContextDraft;disabled?:boolean;action?:(value:Record<string,unknown>)=>void}=$props();
</script>
{#each draft.tabs as tab (tab.tabId)}
  <section aria-label={action ? "Attached tab" : "Sent tab snapshot"}>
    {#if safeSnapshotImage(tab.previewDataUrl)}<img src={safeSnapshotImage(tab.previewDataUrl)} alt={`Page preview: ${tab.title}`} loading="lazy" />{/if}
    <strong>{tab.title}</strong><p>{tab.url}</p>
    <p>{action ? tab.status : "Captured page"}{action && tab.stale ? " · Refresh required" : ""} · ≈{tab.estimatedTokenCount} structured tokens{tab.truncated ? " · Partial capture" : ""}</p>
    {#if tab.error}<p role="alert">{tab.error}</p>{/if}
    {#if action}
      <button type="button" {disabled} onclick={()=>action?.({kind:"tab",id:tab.tabId})}>Refresh tab</button>
      <button type="button" {disabled} aria-label={`Remove tab ${tab.title}`} onclick={()=>action?.({kind:"removeTab",id:tab.tabId})}>Remove</button>
    {/if}
  </section>
{/each}
{#each draft.shells as shell (shell.sessionId)}
  <section aria-label={action ? "Attached terminal output" : "Sent terminal output snapshot"}>
    <strong>{shell.label}</strong><p>{shell.cwd}</p>
    <!-- svelte-ignore a11y_no_noninteractive_tabindex (The bounded output region needs keyboard focus for native scrolling and text selection; it is not a button.) -->
    <div role="region" tabindex="0" aria-label="Captured shell output"><pre>{shell.outputPreview}</pre></div>
    <p>≈{shell.estimatedTokenCount} tokens{shell.truncated ? " · Partial snapshot" : ""}{action && !shell.available ? " · Session closed" : ""}{!action ? " · Frozen at submission" : ""}</p>
    {#if action}
      <label><input type="checkbox" checked={shell.followLive} disabled={disabled || !shell.available} onchange={event=>action?.({kind:"follow",id:shell.sessionId,enabled:event.currentTarget.checked})}/>Follow live until submission</label>
      <button type="button" disabled={disabled || !shell.available} onclick={()=>action?.({kind:"shell",id:shell.sessionId})}>Refresh output</button>
      <button type="button" {disabled} aria-label={`Remove terminal output ${shell.label}`} onclick={()=>action?.({kind:"removeShell",id:shell.sessionId})}>Remove</button>
    {/if}
  </section>
{/each}
<style>
  section { margin:0;padding:var(--ca-space-2) 0;border:0;min-width:0; }
  p,label,strong { font:calc(var(--ca-type-label) + var(--ca-chat-font-offset,0px))/var(--ca-leading-body) var(--ca-font-body);overflow-wrap:anywhere; }
  p { color:var(--ca-muted);margin:var(--ca-space-2) 0; }
  strong { color:var(--ca-text);font-weight:600; }
  img { width:100%;max-height:10rem;object-fit:contain;object-position:left;border-radius:var(--ca-control-radius); }
  div[role="region"] { max-height:12rem;overflow:auto;overscroll-behavior:contain; }
  pre { margin:0;font:calc(var(--ca-type-caption) + var(--ca-chat-font-offset,0px))/var(--ca-leading-body) var(--ca-font-mono);color:var(--ca-text);background:var(--ca-app-background);white-space:pre-wrap;overflow-wrap:anywhere; }
  button { border:0;border-radius:var(--ca-control-radius);background:var(--ca-app-background);color:var(--ca-text);font:var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body);padding:var(--ca-space-2);cursor:pointer; }
  button:hover { background:var(--ca-surface-2); }
  button:disabled { opacity:.55;cursor:default; }
  button:focus-visible,input:focus-visible,div:focus-visible { outline:var(--ca-focus-ring); }
  label { display:flex;gap:var(--ca-space-2);align-items:center; }
  input { accent-color:var(--ca-text); }
</style>
