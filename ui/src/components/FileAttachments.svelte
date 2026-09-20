<script lang="ts">
  import { attachmentList, attachmentPreview } from "../lib/file-attachments";
  let { files = [], remove, disabled = false }: {files?:unknown[]; remove?:(id:string)=>void; disabled?:boolean} = $props();
  const items = $derived(attachmentList(files));
  function icon(element:HTMLElement, key:string) {
    const render = (value:string) => {
      const graphic = (window as Window & {timeMachineFileIcon?:(key:string)=>HTMLElement}).timeMachineFileIcon?.(value || "file");
      if (graphic) element.replaceChildren(graphic);
    };
    render(key); return {update:render};
  }
</script>

{#if items.length}
  <ul aria-label={remove ? "Files attached to this draft" : "Attached files"}>
    {#each items as file (file.id)}
      <li>
        {#if attachmentPreview(file.previewDataUrl)}<img src={attachmentPreview(file.previewDataUrl)} alt={file.name} />
        {:else}<span class="file-icon" aria-hidden="true" use:icon={file.iconKey || "file"}></span>{/if}
        <div><strong>{file.name}</strong>{#if file.sourceLabel}<small>{file.sourceLabel}</small>{/if}<small>{file.kind === "image" ? "Image" : file.kind === "audio" ? "Audio" : "Text/code"} · {typeof file.byteCount === "number" ? `${Math.ceil(file.byteCount / 1024)} KB` : "Size not reported"}{file.kind === "text" && typeof file.estimatedTokenCount === "number" ? ` · ~${file.estimatedTokenCount.toLocaleString()} tokens` : ""}{file.redactionCount ? ` · ${file.redactionCount} redactions` : ""}</small></div>
        {#if remove}<button type="button" {disabled} aria-label={`Remove ${file.name} from draft`} onclick={() => remove?.(file.id)}>×</button>{/if}
      </li>
    {/each}
  </ul>
{/if}

<style>
  ul { display:grid; gap:var(--ca-space-2); list-style:none; margin:0; padding:var(--ca-space-1) 0; min-width:0; }
  li { display:flex; align-items:center; gap:var(--ca-space-2); min-width:0; min-height:var(--ca-control-prominent); padding:var(--ca-space-1) var(--ca-space-2); border-radius:var(--ca-radius-pill); background:var(--ca-surface-2); color:var(--ca-text); font:var(--ca-type-body)/var(--ca-leading-body) var(--ca-font-body); }
  li > div { flex:1; min-width:0; overflow-wrap:anywhere; }
  strong, small { display:block; }
  strong { font-weight:500; }
  small { color:var(--ca-muted); font:var(--ca-type-caption)/var(--ca-leading-body) var(--ca-font-body); }
  img, .file-icon { width:var(--ca-space-8); height:var(--ca-space-8); flex:0 0 var(--ca-space-8); border-radius:var(--ca-radius-pill); object-fit:contain; }
  img { object-fit:cover; }
  .file-icon :global(svg) { width:100%; height:100%; }
  button { display:grid; width:var(--ca-space-8); height:var(--ca-space-8); flex:0 0 var(--ca-space-8); place-items:center; padding:0; border:0; border-radius:var(--ca-radius-pill); background:transparent; color:var(--ca-muted); cursor:pointer; font:var(--ca-type-body)/var(--ca-leading-body) var(--ca-font-body); }
  button:hover:not(:disabled) { background:var(--ca-surface-3); color:var(--ca-text); }
  button:focus-visible { outline:var(--ca-focus-ring); }
  button:disabled { cursor:default; }
</style>
