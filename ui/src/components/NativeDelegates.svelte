<script lang="ts">
  import type {NativeConversationView} from '../lib/native-history';
  let {owner,eventTarget,conversation}:{owner:string;eventTarget?:HTMLElement;conversation:NativeConversationView|null}=$props();
  function open(destination:string):void {
    if(!conversation?.binding || !conversation.delegates?.some(child=>child.owner===destination))return;
    (eventTarget||document).dispatchEvent(new CustomEvent('central-agent:app-server-conversation-control',{bubbles:true,detail:{owner,action:{kind:'open_delegate',expected_thread_id:conversation.binding.threadId,destination}}}));
  }
  function status(value:string):string {
    return ({running:'Working',needs_attention:'Needs your input',completed:'Completed',interrupted:'Stopped',failed:'Failed',not_loaded:'Open to load history',deleted:'Deleted',idle:'Idle'} as Record<string,string>)[value]||'Status not reported';
  }
</script>
{#if conversation?.delegates?.length}
  <section class="native-delegates" aria-label="Native delegation">
    <h3>Delegated work</h3>
    {#each conversation.delegates as relative (relative.threadId)}
      <button type="button" class:attention={relative.pendingRequests>0} data-delegate-owner={relative.owner} onclick={()=>open(relative.owner)}>
        <span class="delegate-name">{relative.name}</span>
        <span>{relative.relation==='parent'?'Open supervisor':status(relative.status)}</span>
      </button>
    {/each}
    <p>Open an agent to read its work, respond to its requests, stop it or continue the conversation.</p>
  </section>
{/if}
<style>
  .native-delegates {display:grid;min-width:0;gap:var(--ca-space-2);}
  h3,p {margin:0;font:inherit;}
  h3,.delegate-name {font-weight:600;color:var(--ca-text);}
  p,button {color:var(--ca-muted);}
  button {display:grid;gap:var(--ca-space-1);width:100%;min-width:0;text-align:left;overflow-wrap:anywhere;background:var(--ca-surface-2);border:0;border-radius:var(--ca-control-radius);padding:var(--ca-space-2);font:inherit;cursor:pointer;}
  button.attention {box-shadow:inset var(--ca-space-1) 0 var(--ca-accent);}
  button:focus-visible {outline:none;box-shadow:var(--ca-focus-ring);}
</style>
