<script lang="ts">
  import { onMount } from "svelte";
  import { droppedContext, type ContextDraft } from "../lib/graph-contexts";
  import SnapshotPreviews from "./SnapshotPreviews.svelte";
  let {owner,eventTarget,draft,disabled=false,enabled=false}: {
    owner:string;eventTarget:HTMLElement;draft:ContextDraft;disabled?:boolean;enabled?:boolean;
  } = $props();
  let error = $state("");
  function send(action: Record<string,unknown>): void {
    eventTarget.dispatchEvent(new CustomEvent("central-agent:graph-contexts",{bubbles:true,detail:{owner,action}}));
  }
  onMount(()=>{
    const update = (event:Event)=>{
      const detail=(event as CustomEvent).detail;
      if(detail?.owner!==owner) return;
      error=detail.error || "";
    };
    const dragover=(event:DragEvent)=>{
      if(!enabled || disabled || !event.dataTransfer) return;
      const types=Array.from(event.dataTransfer.types);
      if(types.some(type=>type.startsWith("application/x-central-agent-")) || types.includes("text/plain")) {
        event.preventDefault();event.stopPropagation();event.dataTransfer.dropEffect="copy";
      }
    };
    const drop=(event:DragEvent)=>{
      if(!enabled || disabled || !event.dataTransfer) return;
      const action=droppedContext(event.dataTransfer);
      if(!action) return;
      event.preventDefault();event.stopPropagation();send(action);
    };
    eventTarget.addEventListener("central-agent:graph-contexts-result",update);
    eventTarget.addEventListener("dragover",dragover);
    eventTarget.addEventListener("drop",drop);
    return ()=>{
      eventTarget.removeEventListener("central-agent:graph-contexts-result",update);
      eventTarget.removeEventListener("dragover",dragover);
      eventTarget.removeEventListener("drop",drop);
    };
  });
</script>

<div class="contexts">
  {#if error}<p role="alert">{error}</p>{/if}
  <SnapshotPreviews {draft} {disabled} action={send} />
  {#if draft.tabs.length || draft.shells.length}<p>Snapshots are sent only with your prompt. Follow live stops at submission and does not grant command access.</p>{/if}
</div>

<style>
  .contexts { min-width:0;display:flex;flex-direction:column;gap:var(--ca-space-2); }
  p { font:var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body);overflow-wrap:anywhere;color:var(--ca-muted);margin:var(--ca-space-2) 0; }
</style>
