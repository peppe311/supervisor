<script lang="ts">
  import { onMount, tick } from "svelte";
  import DiffViewer from "./DiffViewer.svelte";
  import { requestId as newRequestId } from "../lib/request-id";
  import {canImportHistory,historyProjectLabel,historyTitle,type CloudTask,type NativeConversationView,type NativeHistoryItem,type NativeHistoryView} from "../lib/native-history";
  let {owner,eventTarget,conversation,dialogOnly=false}:{owner:string;eventTarget?:HTMLElement;conversation:NativeConversationView|null;dialogOnly?:boolean}=$props();
  let dialog:HTMLDialogElement;
  let confirmation=$state<HTMLElement>();
  let search=$state(""),archived=$state(false),loading=$state(false),submitting=$state(false),error=$state("");
  let source=$state<"local"|"cloud">("local");
  let history=$state<NativeHistoryView|null>(null);
  let choice=$state<{item:NativeHistoryItem;fork:boolean;viewId:string;directory:string;owner:string}|null>(null);
  let requestId="",dialogOwner="";
  let cloudTimer:ReturnType<typeof setTimeout>|undefined;
  $effect(()=>{
    if(owner!==dialogOwner || !conversation?.visible || !conversation.connected) {
      close();dialog?.close();
    }
  });
  function send(action:Record<string,unknown>):void {
    (eventTarget||document).dispatchEvent(new CustomEvent("central-agent:app-server-history-control",{bubbles:true,detail:{owner,action}}));
  }
  function close():void {
    if(cloudTimer!==undefined){clearTimeout(cloudTimer);cloudTimer=undefined;}
    if(history && dialogOwner)(eventTarget||document).dispatchEvent(new CustomEvent("central-agent:app-server-history-control",{bubbles:true,detail:{owner:dialogOwner,action:{kind:"close",view_id:history.viewId}}}));
    requestId="";history=null;choice=null;loading=false;submitting=false;
  }
  function scheduleCloudRefresh():void {
    if(cloudTimer!==undefined){clearTimeout(cloudTimer);cloudTimer=undefined;}
    const cloud=history?.cloud,viewId=history?.viewId;
    if(!dialog?.open||source!=="cloud"||!cloud||cloud.busy||cloud.error||!viewId)return;
    const settled=new Set(["ready","error","cancelled","canceled"]);
    if(!cloud.items.some(item=>!settled.has(item.status.toLowerCase())))return;
    cloudTimer=setTimeout(()=>{
      cloudTimer=undefined;
      if(dialog?.open&&history?.viewId===viewId&&!history.cloud.busy)cloudRefresh();
    },8000);
  }
  function showSource(value:"local"|"cloud"):void {source=value;scheduleCloudRefresh();}
  function load():void {
    if(!conversation?.visible || !conversation.connected || submitting)return;
    if(history)send({kind:"close",view_id:history.viewId});
    requestId=newRequestId();history=null;choice=null;loading=true;error="";
    send({kind:"open",request_id:requestId,archived,search});
  }
  function open():void {
    if(!conversation?.visible || !conversation.connected)return;
    dialogOwner=owner;dialog.showModal();load();
  }
  function more():void {
    if(!history?.hasMore || loading || submitting)return;
    choice=null;loading=true;error="";send({kind:"more",view_id:history.viewId});
  }
  function cloudRefresh():void {
    if(!history || history.cloud?.busy || submitting)return;
    error="";send({kind:"cloud_refresh",view_id:history.viewId});
  }
  function cloudMore():void {
    if(!history?.cloud?.hasMore || history.cloud.busy || submitting)return;
    error="";send({kind:"cloud_more",view_id:history.viewId});
  }
  function openCloud(item:CloudTask):void {
    if(!history || history.cloud?.busy || submitting)return;
    error="";send({kind:"cloud_open",view_id:history.viewId,task_id:item.id});
  }
  function cloudDiff(item:CloudTask):void {
    if(!history || history.cloud?.busy || history.cloud?.diff?.busy || submitting)return;
    error="";send({kind:"cloud_diff",view_id:history.viewId,task_id:item.id,attempt:Math.max(1,item.attemptTotal)});
  }
  function newCloud():void {
    if(!history || history.cloud?.busy || submitting)return;
    error="";send({kind:"cloud_new",view_id:history.viewId});
  }
  function cloudStatus(value:string):string {
    return ({ready:"Ready",running:"Running",pending:"Pending",queued:"Queued",error:"Error",cancelled:"Cancelled",canceled:"Cancelled"} as Record<string,string>)[value.toLowerCase()]||value;
  }
  function cloudDate(value:string):string {
    const parsed=new Date(value);return Number.isNaN(parsed.valueOf())?value:parsed.toLocaleString();
  }
  function select(item:NativeHistoryItem,fork:boolean):void {
    if(!history || loading || history.error || submitting || !canImportHistory(conversation) || item.ephemeral)return;
    if(fork && history.archived)return;
    if(conversation?.pendingFork && (fork || item.forkedFromId!==conversation.pendingFork))return;
    choice={item,fork,viewId:history.viewId,directory:conversation!.directory!,owner};
    dialog.scrollTop=0;
    void tick().then(()=>confirmation?.focus({preventScroll:true}));
  }
  function confirm():void {
    if(!choice || submitting || loading || !canImportHistory(conversation))return;
    if(choice.owner!==owner || choice.directory!==conversation?.directory || choice.viewId!==history?.viewId || choice.fork && history?.archived || conversation?.pendingFork && (choice.fork || choice.item.forkedFromId!==conversation.pendingFork)) {
      error="The destination changed. Select the conversation again.";choice=null;return;
    }
    submitting=true;error="";
    send({kind:"import",view_id:choice.viewId,thread_id:choice.item.id,fork:choice.fork});
  }
  onMount(()=>{
    const command=(event:Event)=>{
      const detail=(event as CustomEvent).detail;
      if(detail?.owner===owner && (detail.command==="resume"||detail.command==="cloud") && conversation?.visible && conversation.connected){source=detail.command==="cloud"?"cloud":"local";open();event.preventDefault();}
    };
    window.addEventListener("central-agent:native-command",command);
    const update=(event:Event)=>{
      const d=(event as CustomEvent).detail;
      if(d?.owner!==owner || !dialog.open)return;
      if(event.type==="central-agent:app-server-history") {
        if(d.error){error=d.error;loading=false;submitting=false;}
        if(d.view?.requestId===requestId){history=d.view;loading=!!d.view.busy;scheduleCloudRefresh();}
      } else if(submitting) {
        if(d.error){error=d.error;submitting=false;}
        else if(d.binding && d.change && d.change!=="stream"){submitting=false;dialog.close();}
      }
    };
    window.addEventListener("central-agent:app-server-history",update);
    window.addEventListener("central-agent:app-server-conversation",update);
    return ()=>{close();window.removeEventListener("central-agent:native-command",command);window.removeEventListener("central-agent:app-server-history",update);window.removeEventListener("central-agent:app-server-conversation",update);};
  });
</script>

{#if !dialogOnly && conversation?.visible}
  {#if conversation.pendingFork}<p>A native fork is pending or its result is uncertain. Use native history to link the fork of the original source. No new prompt will create a replacement conversation here.</p>{/if}
  <button type="button" disabled={!conversation.connected} onclick={open}>Browse Codex history…</button>
{/if}
<dialog bind:this={dialog} class="native-history workspace-confirm-dialog" aria-label="Native Codex history" onclose={close}>
  <header><h2>Codex history</h2><button type="button" onclick={()=>dialog.close()}>Close</button></header>
  {#if !choice}
    <nav class="source-switch" aria-label="Codex history source">
      <button type="button" class:active={source==="local"} aria-pressed={source==="local"} onclick={()=>showSource("local")}>Local</button>
      <button type="button" class:active={source==="cloud"} aria-pressed={source==="cloud"} onclick={()=>showSource("cloud")}>
        <svg class="cloud-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M7.5 18.25h9.25a4 4 0 0 0 .4-7.98A5.75 5.75 0 0 0 6.2 8.9a4.7 4.7 0 0 0 1.3 9.35Z" /></svg>
        Cloud{#if history?.cloud?.items.length} <span>{history.cloud.items.length}</span>{/if}
      </button>
    </nav>
  {/if}
  {#if choice}
    <section bind:this={confirmation} class="confirmation" aria-label="Confirm native conversation destination" tabindex="-1">
      <h3>{choice.fork?"Create an independent fork?":"Link this native conversation?"}</h3>
      <p>{historyTitle(choice.item)} · {choice.item.projectLabel}</p><p>Destination project: {historyProjectLabel(choice.directory)}</p>
      <p>{choice.fork?"Codex copies the stored history into a new thread ID. This is not a copy or checkpoint of the project files. An unfinished source turn may be represented as interrupted in the fork.":"This links the same native conversation, not a copy. Future history changes here also affect that conversation in other Codex clients."}</p>
      <p>Future prompts use this local directory and your selected permissions. No prompt is sent now; other-provider history and the draft are preserved.</p>
      {#if history?.archived && !choice.fork}<p>The conversation remains archived until you explicitly restore it.</p>{/if}
      <div class="actions"><button type="button" disabled={submitting} onclick={()=>choice=null}>Cancel choice</button><button type="button" aria-label={choice.fork?"Create fork":"Link conversation"} disabled={submitting||!canImportHistory(conversation)} onclick={confirm}>{choice.fork?"Create fork":"Link conversation"}</button></div>
      {#if error || history?.error}<p role="alert">{error || history?.error}</p>{/if}
      {#if submitting}<p role="status">Waiting for Codex. Closing this list does not cancel or retry the accepted request.</p>{/if}
    </section>
  {:else}
    {#if source==="local"}
      <p>History from the local Codex runtime, including CLI and other App Server clients. Browsing does not resume a conversation or send a model prompt. Prompt text, native titles and full source paths stay outside this list.</p>
      <form onsubmit={(event)=>{event.preventDefault();load();}}>
        <label>Search native titles (case-sensitive)<input bind:value={search} disabled={submitting} /></label>
        <label class="archive"><input type="checkbox" bind:checked={archived} disabled={submitting} />Archived only</label>
        <button type="submit" disabled={submitting}>Search / refresh</button>
      </form>
      {#if !canImportHistory(conversation)}<p>Create or select a local project chat or graph agent without a Codex binding to link history or create a fork. Existing histories are never replaced.</p>{/if}
      {#if error || history?.error}<p role="alert">{error || history?.error}</p>{/if}
      {#if history?.archived}<p>Codex requires an active conversation to create a fork. You can link archived history here, then explicitly restore it using its conversation controls before forking. Nothing is restored automatically.</p>{/if}
      {#if loading}<p role="status">Loading native history…</p>{/if}
      <div class="results">
        {#if history && !loading && !history.items.length && !history.error}<p>No conversations match this search.</p>{/if}
        {#each history?.items||[] as item (item.id)}
          <article>
            <h3>{historyTitle(item)}</h3>
            <p>Project: {item.projectLabel}</p>
            <p>{item.modelProvider} · {item.status.type} · {new Date(item.updatedAt*1000).toLocaleString()}</p>
            <div class="actions">
              <button type="button" disabled={!canImportHistory(conversation)||loading||submitting||!!history?.error||item.ephemeral||!!conversation?.pendingFork&&item.forkedFromId!==conversation.pendingFork} onclick={()=>select(item,false)}>Link to this chat…</button>
              <button type="button" disabled={!canImportHistory(conversation)||loading||submitting||!!history?.error||!!history?.archived||item.ephemeral||!!conversation?.pendingFork} onclick={()=>select(item,true)}>Fork into this chat…</button>
            </div>
          </article>
        {/each}
        {#if history?.hasMore}<button type="button" disabled={loading||submitting} onclick={more}>Load more</button>{/if}
      </div>
    {:else}
      <div class="cloud-intro">
        <div><h3>ChatGPT subscription</h3><p>Cloud chats from the account connected to Supervisor. They run in OpenAI cloud environments and remain separate from local App Server conversations.</p></div>
        <div class="actions"><button type="button" disabled={!history||history.cloud?.busy} onclick={cloudRefresh}>Refresh</button><button type="button" disabled={!history||history.cloud?.busy} onclick={newCloud}>New cloud chat</button></div>
      </div>
      {#if error}<p role="alert">{error}</p>{/if}
      {#if history?.cloud?.error}<p role="alert">{history.cloud.error}</p>{/if}
      {#if history?.cloud?.busy && !history.cloud.items.length}<p role="status">Loading Codex Cloud chats…</p>{/if}
      <div class="results cloud-results">
        {#if history?.cloud && !history.cloud.busy && !history.cloud.items.length && !history.cloud.error}<p>No Codex Cloud chats were found for this account.</p>{/if}
        {#each history?.cloud?.items||[] as item (item.id)}
          <article class="cloud-task">
            <div class="cloud-heading">
              <svg class="cloud-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M7.5 18.25h9.25a4 4 0 0 0 .4-7.98A5.75 5.75 0 0 0 6.2 8.9a4.7 4.7 0 0 0 1.3 9.35Z" /></svg>
              <div><h3>{item.title}</h3><p>{item.environmentLabel}</p></div>
              <span class="cloud-status" data-status={item.status.toLowerCase()}>{cloudStatus(item.status)}</span>
            </div>
            <p class="cloud-meta">{cloudDate(item.updatedAt)} · {item.attemptTotal} {item.attemptTotal===1?"attempt":"attempts"}{#if item.isReview} · Code review{/if}</p>
            <p class="cloud-summary">{item.summary.filesChanged} {item.summary.filesChanged===1?"file":"files"} changed · +{item.summary.linesAdded} · −{item.summary.linesRemoved}</p>
            <div class="actions">
              <button type="button" disabled={history?.cloud?.busy} onclick={()=>openCloud(item)}>Open chat</button>
              <button type="button" disabled={history?.cloud?.busy||history?.cloud?.diff?.busy||item.summary.filesChanged===0} onclick={()=>cloudDiff(item)}>View changes</button>
            </div>
          </article>
        {/each}
        {#if history?.cloud?.hasMore}<button type="button" disabled={history.cloud.busy} onclick={cloudMore}>Load more</button>{/if}
      </div>
      {#if history?.cloud?.diff}
        {@const diff=history.cloud.diff}
        <section class="cloud-diff" aria-label="Codex Cloud changes">
          <div class="cloud-diff-heading"><h3>Cloud changes · attempt {diff.attempt}</h3>{#if diff.busy}<span role="status">Loading…</span>{/if}</div>
          {#if diff.error}<p role="alert">{diff.error}</p>{:else if !diff.busy && !diff.content}<p>No file changes were reported for this attempt.</p>{:else if diff.content}<DiffViewer content={diff.content} identity={`${diff.taskId}:${diff.attempt}`} inline />{/if}
        </section>
      {/if}
    {/if}
  {/if}
</dialog>

<style>
  button,input {font:inherit;color:var(--ca-text);background:var(--ca-app-background);border:0;border-radius:var(--ca-control-radius);padding:var(--ca-space-2);}
  button:disabled {color:var(--ca-muted);cursor:not-allowed;}
  button:focus-visible,input:focus-visible {box-shadow:var(--ca-focus-ring);}
  .native-history {width:min(92vw,70rem);max-width:92vw;max-height:88vh;padding:var(--ca-space-6);border:0;background:var(--ca-surface);color:var(--ca-text);font:var(--ca-type-body)/var(--ca-leading-body) var(--ca-font-body);overflow:auto;}
  header,.actions {display:flex;flex-wrap:wrap;align-items:center;gap:var(--ca-space-2);}
  header h2 {flex:1;}
  h2 {font-size:var(--ca-type-title);margin:0;}
  h3 {font-size:var(--ca-type-label);margin:0;overflow-wrap:anywhere;}
  p {margin:var(--ca-space-2) 0;overflow-wrap:anywhere;color:var(--ca-muted);}
  p[role="alert"] {color:var(--ca-text);}
  form {display:flex;flex-wrap:wrap;align-items:end;gap:var(--ca-space-3);margin-block:var(--ca-space-4);}
  label {display:grid;gap:var(--ca-space-1);min-width:0;}
  label:first-child {flex:1;}
  .archive {display:flex;align-items:center;}
  input {min-width:0;}
  article {padding-block:var(--ca-space-4);}
  .confirmation {margin-block:var(--ca-space-4);padding:var(--ca-space-5);border:1px solid var(--ca-border);border-radius:var(--ca-radius-large);background:var(--ca-app-background);}
  .source-switch {display:flex;align-items:center;gap:var(--ca-space-1);width:max-content;margin:var(--ca-space-4) 0;padding:var(--ca-space-1);border-radius:var(--ca-radius-pill);background:var(--ca-app-background);}
  .source-switch button {display:flex;align-items:center;gap:var(--ca-space-2);min-width:7.5rem;justify-content:center;background:transparent;color:var(--ca-muted);border-radius:var(--ca-radius-pill);}
  .source-switch button.active {background:var(--ca-surface-2);color:var(--ca-text);}
  .source-switch span {min-width:1.4em;padding-inline:.4em;border-radius:var(--ca-radius-pill);background:var(--ca-app-background);font-size:var(--ca-type-caption);}
  .cloud-icon {width:1.15rem;height:1.15rem;flex:0 0 auto;fill:none;stroke:currentColor;stroke-width:1.65;stroke-linecap:round;stroke-linejoin:round;}
  .cloud-intro {display:flex;align-items:end;justify-content:space-between;gap:var(--ca-space-5);margin-block:var(--ca-space-4);}
  .cloud-intro > div:first-child {max-width:48rem;}
  .cloud-results {display:grid;gap:var(--ca-space-2);}
  .cloud-task {padding:var(--ca-space-4);border-radius:var(--ca-radius-large);background:var(--ca-surface-2);}
  .cloud-heading {display:grid;grid-template-columns:auto minmax(0,1fr) auto;align-items:start;gap:var(--ca-space-3);}
  .cloud-heading h3 {font-size:var(--ca-type-body);line-height:var(--ca-leading-body);}
  .cloud-heading p {margin:var(--ca-space-1) 0 0;}
  .cloud-status {padding:var(--ca-space-1) var(--ca-space-2);border-radius:var(--ca-radius-pill);background:var(--ca-app-background);color:var(--ca-muted);font-size:var(--ca-type-caption);white-space:nowrap;}
  .cloud-status[data-status="running"],.cloud-status[data-status="pending"],.cloud-status[data-status="queued"] {color:var(--ca-text);}
  .cloud-meta,.cloud-summary {font-size:var(--ca-type-label);}
  .cloud-summary {color:var(--ca-text);}
  .cloud-diff {margin-top:var(--ca-space-5);padding:var(--ca-space-3);border-radius:var(--ca-radius-large);background:var(--ca-surface-2);}
  .cloud-diff-heading {display:flex;align-items:center;justify-content:space-between;gap:var(--ca-space-3);padding:var(--ca-space-2);}
  @media (max-width:42rem) {.cloud-intro{align-items:stretch;flex-direction:column}.cloud-heading{grid-template-columns:auto minmax(0,1fr)}.cloud-status{grid-column:2}.source-switch{width:100%}.source-switch button{flex:1;min-width:0}}
</style>
