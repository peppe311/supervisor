<script lang="ts">
  import {onMount} from 'svelte';
  import type {NativeConversationView} from '../lib/native-history';
  import {goalAvailable,goalEditable,goalBudget,goalEditError,goalChoiceValid,goalStatus,type GoalChoice,type GoalEdit} from '../lib/native-goals';
  let {owner,eventTarget,conversation,dialogOnly=false}:{owner:string;eventTarget?:HTMLElement;conversation:NativeConversationView|null;dialogOnly?:boolean}=$props();
  let dialog:HTMLDialogElement;
  let objective=$state(''),limited=$state(false),budget=$state(''),error=$state('');
  let choice=$state<GoalChoice|null>(null),waiting=$state(false),seenBusy=$state(false);
  let openedScope=$state('');
  function scope():string {return JSON.stringify([owner,conversation?.binding?.threadId,conversation?.directory]);}
  function send(action:Record<string,unknown>):void {
    if(!conversation?.binding || !conversation.directory)return;
    waiting=true;seenBusy=false;error='';
    (eventTarget||document).dispatchEvent(new CustomEvent('central-agent:app-server-conversation-control',{bubbles:true,detail:{owner,action:{kind:'goal',expected_thread_id:conversation.binding.threadId,expected_directory:conversation.directory,action}}}));
  }
  function refresh():void {
    if(!goalAvailable(conversation)||waiting||conversation?.goal?.busy)return;
    choice=null;send({kind:'refresh'});
  }
  function open():boolean {
    if(!goalAvailable(conversation))return false;
    openedScope=scope();choice=null;error='';dialog.showModal();refresh();return true;
  }
  function close():void {dialog?.close();choice=null;}
  function review(edit:GoalEdit):void {
    if(waiting || !goalEditable(conversation) || !conversation?.binding || !conversation.goal || !conversation.directory)return;
    error=goalEditError(edit)||'';if(error)return;
    if(edit.kind!=='replace' && !conversation.goal.goal){error='Refresh the native goal before changing it.';return;}
    choice={owner,thread:conversation.binding.threadId,directory:conversation.directory,viewId:conversation.goal.viewId,edit:{...edit}};
  }
  function reviewObjective():void {
    try {review({kind:'replace',objective,status:'paused',token_budget:goalBudget(limited,budget)});}catch(e){error=String(e instanceof Error?e.message:e);}
  }
  function reviewBudget():void {
    try {review({kind:'budget',token_budget:goalBudget(limited,budget)});}catch(e){error=String(e instanceof Error?e.message:e);}
  }
  function confirm():void {
    if(!choice||waiting)return;
    const selected=choice;choice=null;
    if(!goalChoiceValid(selected,owner,conversation)){error='The goal or conversation changed. Refresh and confirm again.';return;}
    send({kind:'change',view_id:selected.viewId,edit:selected.edit});
  }
  $effect(()=>{
    if(!goalAvailable(conversation) || openedScope && openedScope!==scope()) {
      close();openedScope='';waiting=false;seenBusy=false;objective='';limited=false;budget='';
    }
    if(conversation?.goal?.busy)seenBusy=true;
    else if(seenBusy){waiting=false;seenBusy=false;}
    if(choice && !goalChoiceValid(choice,owner,conversation))choice=null;
  });
  onMount(()=>{
    const command=(event:Event)=>{
      const detail=(event as CustomEvent).detail;
      if(detail?.owner===owner && detail.command==='goal' && open())event.preventDefault();
    };
    const failure=(event:Event)=>{
      const detail=(event as CustomEvent).detail;
      if(detail?.owner===owner && detail.error){waiting=false;error=String(detail.error);}
    };
    window.addEventListener('central-agent:native-command',command);
    window.addEventListener('central-agent:app-server-conversation',failure);
    return ()=>{window.removeEventListener('central-agent:native-command',command);window.removeEventListener('central-agent:app-server-conversation',failure);};
  });
</script>

{#if !dialogOnly && conversation?.visible && conversation.binding && !conversation.binding.deleted}
  <button type="button" disabled={!goalAvailable(conversation)} onclick={open}>Goal…</button>
  {#if !conversation.nativeLoaded}<p class="hint">Resume the native conversation to manage its goal.</p>{/if}
{/if}
<dialog bind:this={dialog} class="native-goal workspace-confirm-dialog" aria-label="Native Codex goal" onclose={()=>choice=null}>
  <header><h2>Conversation goal</h2><button type="button" onclick={close}>Close</button></header>
  <p>Scoped to the currently loaded native Codex conversation.</p>
  <p>The goal belongs to the loaded Codex session. Changing the project, model or permissions in the composer does not reconfigure that session until a new prompt is accepted.</p>
  <button type="button" disabled={waiting || !!conversation?.goal?.busy || !goalAvailable(conversation)} onclick={refresh}>Refresh goal</button>
  {#if waiting || conversation?.goal?.busy}<p role="status">{conversation?.goal?.writing?'Updating native goal…':'Reading native goal…'}</p>{/if}
  {#if error || conversation?.goal?.error}<p role="alert">{error || conversation?.goal?.error}</p>{/if}
  {#if conversation?.goal}
    {#if !conversation.goal.current}<p>Last observed state only. Refresh before changing the goal.</p>{/if}
    {#if conversation.goal.goal}
      {@const goal=conversation.goal.goal}
      <h3>{goalStatus(goal.status)}</h3><p class="objective">{goal.objective}</p>
      <p>{goal.tokensUsed.toLocaleString()} tokens used · {goal.timeUsedSeconds.toLocaleString()} seconds · {goal.tokenBudget===null?'No goal token budget':`${goal.tokenBudget.toLocaleString()} token budget`}</p>
      <div class="row">
        <button type="button" disabled={waiting || !goalEditable(conversation) || goal.status==='active'} onclick={()=>review({kind:'status',status:'active'})}>Set active…</button>
        <button type="button" disabled={waiting || !goalEditable(conversation) || goal.status==='paused'} onclick={()=>review({kind:'status',status:'paused'})}>Pause goal…</button>
        <button type="button" disabled={waiting || !goalEditable(conversation) || goal.status==='blocked'} onclick={()=>review({kind:'status',status:'blocked'})}>Mark blocked…</button>
        <button type="button" disabled={waiting || !goalEditable(conversation) || goal.status==='complete'} onclick={()=>review({kind:'status',status:'complete'})}>Mark complete…</button>
        <button type="button" disabled={waiting || !goalEditable(conversation)} onclick={()=>review({kind:'clear'})}>Remove goal…</button>
      </div>
    {:else if conversation.goal.current}<p>No native goal in this conversation.</p>{/if}
  {/if}
  <details><summary>{conversation?.goal?.goal?'Replace objective or change budget':'Create an objective'}</summary>
    <label>Objective<textarea rows="4" bind:value={objective} placeholder="Describe the outcome"></textarea></label>
    <label class="row"><input type="checkbox" bind:checked={limited} /> Set an optional token budget</label>
    {#if limited}<label>Token budget<input inputmode="numeric" bind:value={budget} /></label>{/if}
    <div class="row">
      <button type="button" aria-label={conversation?.goal?.goal?'Replace objective':'Create paused goal'} disabled={waiting || !goalEditable(conversation)} onclick={reviewObjective}>{conversation?.goal?.goal?'Replace objective…':'Create paused goal…'}</button>
      {#if conversation?.goal?.goal}<button type="button" disabled={waiting || !goalEditable(conversation)} onclick={reviewBudget}>Change budget only…</button>{/if}
    </div>
  </details>
  {#if choice}
    <section aria-label="Confirm native goal change">
      <h3>Confirm goal change</h3>
      {#if choice.edit.kind==='replace'}
        <p class="objective">{choice.edit.objective}</p><p>The goal will be paused. A different objective, or replacing a completed goal, resets native accounting. The same unfinished objective retains its accounting.</p>
      {:else if choice.edit.kind==='status'}
        <p>Set goal state to {choice.edit.status}.</p>
        {#if choice.edit.status==='active'}<p>Codex owns execution and may consume your account usage. This does not grant new filesystem permissions or apply pending composer settings.</p>{/if}
      {:else if choice.edit.kind==='clear'}<p>Remove this goal from Codex. This does not delete conversation history or restore project files.</p>{/if}
      {#if choice.edit.kind==='replace'||choice.edit.kind==='budget'}<p>{choice.edit.token_budget===null?'No goal token budget. Account and native runtime limits still apply.':`Goal token budget: ${choice.edit.token_budget.toLocaleString()}.`}</p>{/if}
      <p>Pausing, completing or removing a goal is not a request to interrupt a running turn. Use Stop separately. Closing this dialog does not cancel a sent change.</p>
      <p>Codex is read again before the edit; this API cannot atomically prevent a simultaneous edit from another client.</p>
      <div class="row"><button type="button" onclick={()=>choice=null}>Back</button><button type="button" disabled={waiting || !goalChoiceValid(choice,owner,conversation)} onclick={confirm}>Confirm change</button></div>
    </section>
  {/if}
</dialog>

<style>
  button,input,textarea {font:inherit;color:var(--ca-text);background:var(--ca-app-background);border:0;border-radius:var(--ca-control-radius);padding:var(--ca-space-2);}
  button:disabled {color:var(--ca-muted);cursor:not-allowed;}
  button:focus-visible,input:focus-visible,textarea:focus-visible,summary:focus-visible {box-shadow:var(--ca-focus-ring);}
  .native-goal {width:min(92vw,60rem);max-width:92vw;max-height:88vh;overflow:auto;padding:var(--ca-space-6);border:0;border-radius:var(--ca-radius-large);background:var(--ca-surface);color:var(--ca-text);font:var(--ca-type-body)/var(--ca-leading-body) var(--ca-font-body);}
  header,.row {display:flex;flex-wrap:wrap;align-items:center;gap:var(--ca-space-3);}
  header h2 {flex:1;font-size:var(--ca-type-title);margin:0;}
  h3 {font-size:var(--ca-type-label);}
  p {margin:var(--ca-space-2) 0;overflow-wrap:anywhere;color:var(--ca-muted);}
  .objective,p[role="alert"] {color:var(--ca-text);white-space:pre-wrap;}
  label {display:grid;gap:var(--ca-space-2);margin-block:var(--ca-space-3);min-width:0;}
  textarea {min-width:0;width:100%;box-sizing:border-box;resize:vertical;}
  details,section {padding-block:var(--ca-space-4);}
  summary {cursor:pointer;}
  .hint {font-size:var(--ca-type-caption);}
</style>
