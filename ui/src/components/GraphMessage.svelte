<script lang="ts">
  import { untrack } from "svelte";
  import DiffViewer from "./DiffViewer.svelte";
  import FileAttachments from "./FileAttachments.svelte";
  import NativeMedia from "./NativeMedia.svelte";
  import SnapshotPreviews from "./SnapshotPreviews.svelte";
  import { contextDraft } from "../lib/graph-contexts";
  import type { AgentTimelineMessage } from "../lib/agent-timeline";
  import type { GraphRenderHooks } from "../lib/graph-history";
  let { initial, hooks, owner }: { initial: AgentTimelineMessage; hooks: GraphRenderHooks; owner:string } = $props();
  let message = $state(untrack(() => initial));
  export function update(value: AgentTimelineMessage): void { message = value; }
  function richContent(element:HTMLElement, value:AgentTimelineMessage) {
    let previous = "";
    function render(next:AgentTimelineMessage): void {
      const signature = JSON.stringify([next.renderedHtml,next.text]);
      if (signature === previous) return;
      previous = signature;
      if (typeof next.renderedHtml === "string") { element.innerHTML = next.renderedHtml; hooks.decorate(element); }
      else element.textContent = next.text || "";
    }
    render(value); return {update:render};
  }
  function artifacts(element:HTMLElement, items:unknown[]) { let previous=""; const render = (value:unknown[]) => { const signature=JSON.stringify(value); if (signature!==previous) { previous=signature; element.replaceChildren(hooks.artifacts(value)); } }; render(items); return {update:render}; }
  function checkpoint(element:HTMLElement, value:unknown) { let previous=""; const render = (next:unknown) => { const signature=JSON.stringify(next); if (signature!==previous) { previous=signature; element.replaceChildren(hooks.checkpoint(next)); } }; render(value); return {update:render}; }
</script>

{#if message.kind === "reasoning"}<span class="chat-role">Reasoning</span>
{:else if message.kind === "activity"}<span class="chat-role">{message.activityStatus || "Action"}</span>
{:else if message.role === "system" && message.text}<span class="chat-role">System</span>{/if}
<div class="chat-text agent-console-text" class:markdown={typeof message.renderedHtml === "string"} use:richContent={message}></div>
<FileAttachments files={message.fileAttachments} />
{#if message.provider === "codex_app_server" && message.kind === "native_media"}<NativeMedia value={message.nativeMedia} />{/if}
<SnapshotPreviews draft={contextDraft({tabs:message.attachments,shells:message.terminalAttachments})} />

{#if message.kind === "activity"}
  <div class="activity-detail">
    {#if message.activityDetail}<code>{message.activityDetail}</code>{/if}
    {#if message.activityContext}<span class="activity-context">{message.activityContext}</span>{/if}
    {#if message.activityAdditions != null || message.activityDeletions != null}<span class="diff-stats"><span class="added">+{message.activityAdditions ?? 0}</span> / <span class="removed">−{message.activityDeletions ?? 0}</span></span>{/if}
    {#if message.activityDiff}<details class="activity-inline-diff"><summary>View diff</summary><DiffViewer content={message.activityDiff} identity={String(message.id)} inline /></details>{/if}
  </div>
{/if}
{#if message.artifacts?.length}<div use:artifacts={message.artifacts}></div>{/if}
{#if message.checkpoint}<div use:checkpoint={message.checkpoint}></div>{/if}

<style>
  .chat-role, .activity-context { display:block; color:var(--ca-muted); font:var(--ca-type-caption)/var(--ca-leading-body) var(--ca-font-body); margin-bottom:var(--ca-space-1); }
  .chat-text { color:inherit; overflow-wrap:anywhere; white-space:pre-wrap; font:calc(var(--ca-type-body) + var(--ca-chat-font-offset, 0px))/var(--ca-leading-body) var(--ca-font-body); }
  .chat-text.markdown { white-space:normal; }
  .chat-text.markdown :global(strong), .chat-text.markdown :global(h1), .chat-text.markdown :global(h2), .chat-text.markdown :global(h3), .chat-text.markdown :global(h4), .chat-text.markdown :global(h5), .chat-text.markdown :global(h6), .chat-text.markdown :global(a), .chat-text.markdown :global(li::marker) { color:inherit; }
  .activity-detail { display:grid; gap:var(--ca-space-2); min-width:0; }
  code { white-space:pre-wrap; overflow-wrap:anywhere; font:var(--ca-type-body)/var(--ca-leading-body) var(--ca-font-mono); }
  .activity-inline-diff { min-width:0; color:var(--ca-muted); }
  summary { display:inline-flex; box-sizing:border-box; width:auto; max-width:100%; min-height:var(--ca-control-compact); align-items:center; padding:0; border:0; border-radius:var(--ca-radius-small); background:transparent; color:inherit; cursor:pointer; list-style:none; font:650 var(--ca-type-caption)/var(--ca-leading-body) var(--ca-font-body); }
  summary::-webkit-details-marker { display:none; }
  summary::after { margin-left:var(--ca-space-1); content:'›'; }
  details[open]>summary::after { content:'⌄'; }
  summary:hover { color:var(--ca-text); }
  summary:focus-visible { outline:0; box-shadow:var(--ca-focus-ring); }
  .added { color:var(--ca-diff-addition); } .removed { color:var(--ca-diff-deletion); }
</style>
