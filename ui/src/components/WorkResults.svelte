<script lang="ts">
  import {onMount} from 'svelte';
  import SettingsPicker from './SettingsPicker.svelte';
  import WorkEvidence from './WorkEvidence.svelte';
  import {requestId} from '../lib/request-id';
  import {comparisonReady,type WorkMode,type WorkReport,type WorkCandidate} from '../lib/work-results';
  let {eventTarget,projectKey,active=true}:{eventTarget:HTMLElement;projectKey:string;active?:boolean}=$props();
  let dialog:HTMLDialogElement;
  let mode=$state<WorkMode>('summary'),owner=$state(''),openedProject='';
  let report=$state<WorkReport|null>(null),otherReport=$state<WorkReport|null>(null);
  let candidates=$state<WorkCandidate[]>([]),other=$state(''),reviewer=$state('');
  let error=$state(''),notice=$state(''),pending=$state(false),destination=$state(''),forkReady=$state(false);
  let title=$state(''),instruction=$state('Continue the original objective from the point reached in this conversation. Keep its constraints and decisions, check the current files and reported failures, then complete the remaining work.');
  let mainRequest='',otherRequest='',mutationRequest='';
  let inspectTimeout:ReturnType<typeof setTimeout>|undefined;
  const reviewers=$derived(candidates.filter(candidate=>candidate.reviewer&&candidate.owner!==other));
  const canCompare=$derived(comparisonReady(report,otherReport,reviewers.find(candidate=>candidate.owner===reviewer)));
  const heading=$derived(mode==='summary'?'Work summary':mode==='compare'?'Compare attempts':'Hand off work');
  $effect(()=>{if((projectKey!==openedProject||!active)&&owner){dialog?.close();owner='';mainRequest='';otherRequest='';mutationRequest='';pending=false;clearTimeout(inspectTimeout);}});
  function send(action:Record<string,unknown>):string {
    const request_id=requestId();
    eventTarget.dispatchEvent(new CustomEvent('central-agent:project-board',{bubbles:true,detail:{action:'backend',backendAction:{type:'work',request_id,action}}}));
    return request_id;
  }
  export function open(nextOwner:string,nextMode:WorkMode):void {
    if(pending)return;
    owner=nextOwner;mode=nextMode;openedProject=projectKey;report=null;otherReport=null;candidates=[];other='';reviewer='';error='';notice='';destination='';forkReady=false;title='';
    dialog.showModal();refresh(true);
  }
  function refresh(load=false):void {
    if(!owner||pending)return;
    error='';mainRequest=send({kind:'inspect',owner,load});
    clearTimeout(inspectTimeout);
    const expected=mainRequest;
    inspectTimeout=setTimeout(()=>{
      if(dialog.open&&mainRequest===expected&&!report){mainRequest='';error='Work details did not respond. Try Refresh details.';}
    },15000);
    if(other)otherRequest=send({kind:'inspect',owner:other,load});
  }
  function chooseOther(value:string):void {other=value;otherReport=null;error='';if(reviewer===other)reviewer='';otherRequest=send({kind:'inspect',owner:other,load:true});}
  function compare():void {
    const selected=reviewers.find(candidate=>candidate.owner===reviewer);
    if(pending||!canCompare||!report||!otherReport||!selected)return;
    pending=true;error='';mutationRequest=send({kind:'compare',owner,revision:report.revision,other,other_revision:otherReport.revision,reviewer,reviewer_thread:selected.threadId});
  }
  function handoff():void {
    if(pending||!report?.canHandoff||!title.trim()||!instruction.trim()||destination)return;
    pending=true;error='';mutationRequest=send({kind:'handoff',owner,revision:report.revision,title:title.trim(),instruction:instruction.trim()});
  }
  function openDestination():void {if(!destination||!forkReady)return;send({kind:'open',owner:destination});dialog.close();}
  onMount(()=>{
    const receive=(event:Event)=>{
      const d=(event as CustomEvent).detail;
      if(!d?.requestId)return;
      if(d.requestId===mutationRequest){
        mutationRequest='';pending=false;
        if(d.error){error=d.error;return;}
        if(d.result?.handoff){destination=d.result.destination;notice='Creating the native continuation…';if(d.result.warning)error=d.result.warning;}
        else if(d.result?.started){notice='Comparison started in the selected Supervisor. Its result will appear in that conversation.';dialog.close();}
        return;
      }
      if(!dialog.open)return;
      if(d.requestId!==mainRequest&&d.requestId!==otherRequest)return;
      if(d.error){if(d.requestId===mainRequest){mainRequest='';clearTimeout(inspectTimeout);}else otherRequest='';error=d.error;return;}
      if(d.requestId===mainRequest){mainRequest='';clearTimeout(inspectTimeout);if(d.result?.report?.owner!==owner)return;report=d.result.report;candidates=d.result.candidates||[];if(!title)title=`Handoff · ${report?.title||'Work'}`.slice(0,80);}
      else{otherRequest='';if(d.result?.report?.owner===other)otherReport=d.result.report;}
    };
    const nativeUpdate=(event:Event)=>{
      if(!dialog.open)return;
      const d=(event as CustomEvent).detail;
      if(destination&&(d?.owner===destination||d?.owner===owner)) {
        if(d.error){pending=false;error=d.error;notice='The continuation may need recovery. Inspect the new card before retrying.';return;}
        if(d.owner===destination&&d.change==='forked'||d.owner===owner&&d.change==='branched'&&d.nativeConversation?.lastBranch?.owner===destination){forkReady=true;notice='Ready. The new agent retains the native history and attachments. Open it, choose its model and send the prepared continuation.';}
      }
      if(!pending&&!destination&&[owner,other].includes(d?.owner)) {
        if(['opened','history_hydrated','updated'].includes(d.change))refresh(true);
        else if(['work_history_loading','work_history_loaded'].includes(d.change))refresh(false);
        else if(d.change==='stream') {
          const previous=d.owner===owner?report:otherReport;
          if(previous&&typeof d.nativeConversation?.busy==='boolean'&&previous.busy!==d.nativeConversation.busy)refresh(false);
        }
      }
      if(d?.error&&[owner,other].includes(d.owner))error=d.error;
    };
    window.addEventListener('central-agent:work-results',receive);
    window.addEventListener('central-agent:app-server-conversation',nativeUpdate);
    return()=>{clearTimeout(inspectTimeout);window.removeEventListener('central-agent:work-results',receive);window.removeEventListener('central-agent:app-server-conversation',nativeUpdate);};
  });
</script>

<dialog bind:this={dialog} class="work-dialog" class:comparing={mode==='compare'} aria-labelledby="work-results-title" onclose={()=>{mainRequest='';otherRequest='';clearTimeout(inspectTimeout);}} onkeydown={event=>{if(event.key==='Escape')event.stopPropagation();}}>
  <header><div><h2 id="work-results-title">{heading}</h2><p>Recorded work, with its original context.</p></div><button type="button" aria-label="Close work summary" onclick={()=>dialog.close()}>×</button></header>
  {#if error}<p class="notice" role="alert">{error}</p>{/if}
  {#if notice}<p class="notice" role="status">{notice}</p>{/if}
  {#if mode==='handoff'}
    <p>A new Codex agent continues from this conversation using its native history, objective and attachments. You can choose another model before sending the continuation.</p>
    <p class="muted">Project files are shared. No worktree or file snapshot is created. An active worker must be stopped first.</p>
    <label for="handoff-name">New agent name</label><input id="handoff-name" bind:value={title} maxlength="80" disabled={pending||!!destination}/>
    <label for="handoff-instruction">Continuation</label><textarea id="handoff-instruction" bind:value={instruction} maxlength="4000" rows="4" disabled={pending||!!destination}></textarea>
    {#if report&&!report.canHandoff}<p class="muted">Hand off requires an idle Codex conversation with loaded native history. Other providers keep their work summaries.</p>{/if}
    {#if report}<details><summary>Source work · {report.title}</summary><WorkEvidence {report}/></details>{/if}
  {:else}
    {#if mode==='compare'}
      <SettingsPicker id="work-other" label="Other attempt" presentation="workspace" contained value={other} disabled={pending||!report||!candidates.some(candidate=>!candidate.busy)} options={[{value:'',label:'Choose a conversation',disabled:true},...candidates.map(candidate=>({value:candidate.owner,label:candidate.title,disabled:candidate.busy}))]} onSelect={chooseOther}/>
      {#if report&&!candidates.length}<p class="muted">Add another conversation in this local project to compare two attempts.</p>
      {:else if report&&candidates.every(candidate=>candidate.busy)}<p class="muted">Wait for another conversation to finish before comparing attempts.</p>{/if}
    {/if}
    <div class="reports">{#if report}<WorkEvidence {report}/>{:else if error}<p class="muted">No work details are available. Refresh to try again.</p>{:else}<p role="status">Loading recorded work…</p>{/if}{#if mode==='compare'&&otherReport}<WorkEvidence report={otherReport}/>{/if}</div>
    {#if mode==='compare'}
      <SettingsPicker id="work-reviewer" label="Review with" presentation="workspace" contained value={reviewer} disabled={pending||!report||!reviewers.length} options={[{value:'',label:'Choose an idle Supervisor',disabled:true},...reviewers.map(candidate=>({value:candidate.owner,label:candidate.title}))]} onSelect={value=>reviewer=value}/>
      <p class="muted">The selected Codex Supervisor reviews these recorded attempts in read-only mode and explains its preference. This uses your connected account. It does not apply either diff. Attachment names are included; their content remains in the source history.</p>
      {#if report&&!reviewers.length}<p class="muted">Open an idle Codex Supervisor in this project and load its conversation to make it available here.</p>{/if}
    {/if}
  {/if}
  <footer><button type="button" disabled={pending||!!destination} onclick={()=>refresh(true)}>Refresh details</button><div>
    {#if mode==='compare'}<button class="primary" type="button" aria-label="Ask Supervisor to compare attempts" disabled={pending||!canCompare} onclick={compare}>{pending?'Starting…':'Ask Supervisor'}</button>
    {:else if mode==='handoff'}{#if destination}<button class="primary" type="button" disabled={!forkReady} onclick={openDestination}>Open new agent</button>{:else}<button class="primary" type="button" aria-label="Create handoff" disabled={pending||!report?.canHandoff||!title.trim()||!instruction.trim()} onclick={handoff}>{pending?'Creating…':'Create handoff'}</button>{/if}
    {:else}<button type="button" onclick={()=>{send({kind:'open',owner});dialog.close();}}>Open conversation</button>{/if}
  </div></footer>
</dialog>

<style>
  .work-dialog{box-sizing:border-box;width:min(720px,calc(100vw - var(--ca-space-8)));max-height:calc(100dvh - var(--ca-space-8));overflow:auto;padding:var(--ca-space-6);border:0;border-radius:var(--ca-radius-large);background:var(--ca-surface);color:var(--ca-text);box-shadow:var(--ca-shadow-float);font:var(--ca-type-body)/var(--ca-leading-body) var(--ca-font-body)}
  .work-dialog[open]{display:grid;gap:var(--ca-space-4)}.work-dialog.comparing{width:min(1120px,calc(100vw - var(--ca-space-8)))}.work-dialog::backdrop{background:color-mix(in srgb,var(--ca-app-background) 72%,transparent);backdrop-filter:blur(4px)}
  header,footer{display:flex;justify-content:space-between;align-items:center;gap:var(--ca-space-4);background:transparent}h2{margin:0;font:650 var(--ca-type-title)/var(--ca-leading-compact) var(--ca-font-display)}p{margin:0}header p,.muted{color:var(--ca-muted);font-size:var(--ca-type-caption)}
  button{min-height:var(--ca-control-compact);padding:var(--ca-space-2) var(--ca-space-3);border:0;border-radius:var(--ca-control-radius);background:var(--ca-surface-2);color:var(--ca-text);font:inherit;cursor:pointer}button:hover{background:var(--ca-surface-3)}button:disabled{opacity:.5;cursor:default}button:focus-visible,input:focus-visible,textarea:focus-visible,summary:focus-visible{outline:none;box-shadow:var(--ca-focus-ring)}.primary{background:var(--ca-text);color:var(--ca-app-background)}.primary:hover{background:var(--ca-muted)}
  .reports{display:grid;grid-template-columns:minmax(0,1fr);gap:var(--ca-space-4);min-width:0}.comparing .reports{grid-template-columns:repeat(2,minmax(0,1fr))}summary{cursor:pointer;padding-block:var(--ca-space-2)}.notice{padding:var(--ca-space-3);border-radius:var(--ca-radius-medium);background:var(--ca-surface-2);overflow-wrap:anywhere}
  input,textarea{box-sizing:border-box;width:100%;min-width:0;padding:var(--ca-space-3);border:0;border-radius:var(--ca-radius-medium);background:var(--ca-input);color:var(--ca-text);font:inherit}textarea{resize:vertical}label{font-weight:600;font-size:var(--ca-type-label)}
  @media(max-width:760px){.comparing .reports{grid-template-columns:minmax(0,1fr)}.work-dialog{padding:var(--ca-space-4)}footer{flex-wrap:wrap}}
</style>
