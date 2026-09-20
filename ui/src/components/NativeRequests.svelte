<script lang="ts">
  import { onMount } from "svelte";
  import NativeRequestCard from "./NativeRequestCard.svelte";
  import { requestSnapshot, type NativeRequest } from "../lib/native-requests";
  let { owner: fixedOwner, eventTarget }: {owner?:string;eventTarget?:HTMLElement}=$props();
  let owner=$state(""),requests=$state<NativeRequest[]>([]),error=$state("");
  let bound=$state(false), loaded=$state(false), connected=$state(false), receipt=$state(""), reviewed=$state(false);
  let deleted=$state(false);
  let compactionReceipt=$state(false);
  onMount(()=>{
    owner=fixedOwner || "";
    const switchOwner=(event:Event)=>{
      const next=String((event as CustomEvent).detail || "");
      if(!fixedOwner && next!==owner) {owner=next;requests=[];error="";bound=false;loaded=false;connected=false;deleted=false;receipt="";reviewed=false;}
    };
    const update=(event:Event)=>{
      const detail=(event as CustomEvent).detail;
      const snapshot=requestSnapshot(detail,owner);
      if(snapshot) requests=snapshot;
      if(detail?.owner===owner) {
        if("nativeConversation" in detail) {
          const nextConnected=Boolean(detail.nativeConversation?.connected);
          if(!connected && nextConnected) error="";
          connected=nextConnected;
        }
        if("nativeBinding" in detail) {bound=Boolean(detail.nativeBinding);deleted=Boolean(detail.nativeBinding?.deleted);}
        if("binding" in detail) {bound=Boolean(detail.binding);deleted=Boolean(detail.binding?.deleted);}
        if("nativeLoaded" in detail) loaded=Boolean(detail.nativeLoaded);
        if("thread" in detail) loaded=Boolean(detail.thread);
        if("unresolved" in detail) {
          const next=String(detail.unresolved?.messageId || "");
          if(next!==receipt) reviewed=false;
          receipt=next;
          compactionReceipt=detail.unresolved?.compact===true;
        }
        if(event.type==="central-agent:app-server-conversation") {
          error=detail.error || "";
          if(detail.change==="updated" && receipt) reviewed=true;
        }
      }
      if(detail?.owner===owner && event.type==="central-agent:app-server-requests") error=detail.error || "";
    };
    window.addEventListener("central-agent:conversation-key",switchOwner);
    window.addEventListener("central-agent:app-server-requests",update);
    window.addEventListener("central-agent:app-server-conversation",update);
    const target=eventTarget || window;
    target.addEventListener("central-agent:conversation-state",update);
    return ()=>{
      window.removeEventListener("central-agent:conversation-key",switchOwner);
      window.removeEventListener("central-agent:app-server-requests",update);
      window.removeEventListener("central-agent:app-server-conversation",update);
      target.removeEventListener("central-agent:conversation-state",update);
    };
  });
  function act(action:Record<string,unknown>):void {
    error="";
    (eventTarget || document).dispatchEvent(new CustomEvent("central-agent:app-server-request-control",{bubbles:true,detail:{owner,action}}));
  }
  function control(action:Record<string,unknown>):void {
    error="";
    (eventTarget || document).dispatchEvent(new CustomEvent("central-agent:app-server-conversation-control",{bubbles:true,detail:{owner,action}}));
  }
</script>

{#if requests.length || error || receipt || (bound && !loaded && !deleted)}
  <div class="native-requests" role="region" aria-label="Codex decisions">
    {#if receipt}
      <p>{compactionReceipt?"The native compaction outcome is uncertain. Check Codex history before deciding whether another compaction is needed. The acknowledgement alone does not prove completion.":"Codex has not confirmed this request. Check its history before trying again."} Nothing will be resent automatically.</p>
      <button type="button" onclick={()=>control({kind:"read"})}>Check native history</button>
      {#if reviewed}<button type="button" onclick={()=>control({kind:"review_delivery",message_id:receipt})}>I reviewed the history · dismiss delivery warning</button>{/if}
    {:else if bound && !loaded}
      <button type="button" onclick={()=>control({kind:"read"})}>Load Codex history</button>
    {/if}
    {#if error}<p role="alert">{error}</p><button type="button" onclick={()=>error=""}>Dismiss message</button>{/if}
    {#each requests as request (request.ticket)}<NativeRequestCard {request} onaction={act} />{/each}
  </div>
{/if}

<style>
  .native-requests { flex:0 1 auto; min-height:0; max-height:40vh; overflow:auto; min-width:0; padding:var(--ca-space-3); color:var(--ca-text); background:var(--ca-bg); font-size:var(--ca-type-body); }
  p { margin:0 0 var(--ca-space-2); overflow-wrap:anywhere; }
  button { color:var(--ca-text); background:var(--ca-surface); border:0; border-radius:var(--ca-control-radius); padding:var(--ca-space-2); font:inherit; }
  button:focus-visible { box-shadow:var(--ca-focus-ring); }
</style>
