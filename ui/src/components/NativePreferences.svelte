<script lang="ts">
  import { requestId as newRequestId } from "../lib/request-id";
  import {onMount} from "svelte";
  import type {NativeConversationView} from "../lib/native-history";
  import {preferenceLabels,preferenceChoices,type NativePreference,type NativePreferencesView} from "../lib/native-preferences";
  let {owner,eventTarget,conversation,dialogOnly=false}:{owner:string;eventTarget?:HTMLElement;conversation:NativeConversationView|null;dialogOnly?:boolean}=$props();
  let dialog:HTMLDialogElement;
  let inventory=$state<NativePreferencesView|null>(null),error=$state(""),notice=$state("");
  let loading=$state(false),writing=$state(false),directory=$state("");
  let editing=$state<NativePreference|null>(null),value=$state("");
  let choice=$state<{operation:"save"|"clear";key:string;value:string;owner:string;directory:string;viewId:string;file:string;version:string}|null>(null);
  let requestId="",dialogOwner="";
  $effect(()=>{
    if(owner!==dialogOwner || !conversation?.visible || !conversation.connected || conversation.directory!==directory) {close();dialog?.close();}
  });
  function available():boolean {return !!conversation?.visible && conversation.connected && !!conversation.directory;}
  function send(action:Record<string,unknown>,destination=owner):void {
    (eventTarget||document).dispatchEvent(new CustomEvent("central-agent:app-server-preferences-control",{bubbles:true,detail:{owner:destination,action}}));
  }
  function close():void {
    if(requestId && dialogOwner)send({kind:"close",request_id:requestId},dialogOwner);
    requestId="";inventory=null;choice=null;editing=null;loading=false;
  }
  function refresh():void {
    if(!available() || writing)return;
    close();requestId=newRequestId();error="";loading=true;
    send({kind:"refresh",request_id:requestId,expected_directory:directory});
  }
  function open():void {
    if(!available())return;
    dialogOwner=owner;directory=conversation!.directory!;writing=false;notice="";
    dialog.showModal();refresh();
  }
  function canEdit():boolean {return available() && !!inventory?.current && !!inventory.snapshot?.target && !loading && !writing && !conversation?.busy && inventory.directory===conversation?.directory;}
  function select(preference:NativePreference):void {
    if(!canEdit())return;
    editing=preference;value=preference.userValue??preference.effective??"";choice=null;
  }
  function review():void {
    if(!editing || !canEdit() || !value.trim())return;
    const target=inventory!.snapshot!.target!;
    choice={operation:"save",key:editing.key,value,owner,directory,viewId:inventory!.viewId,file:target.file,version:target.version};
  }
  function reviewClear(preference:NativePreference):void {
    if(!canEdit())return;
    const observed=inventory!.snapshot!.preferences.find(p=>p.key===preference.key);
    if(!observed || observed.userValue===null)return;
    const target=inventory!.snapshot!.target!;
    editing=null;
    choice={operation:"clear",key:observed.key,value:observed.userValue,owner,directory,viewId:inventory!.viewId,file:target.file,version:target.version};
  }
  function confirm():void {
    if(!choice || !canEdit())return;
    const target=inventory!.snapshot!.target!;
    if(owner!==choice.owner || directory!==choice.directory || inventory!.viewId!==choice.viewId || target.file!==choice.file || target.version!==choice.version || !inventory!.snapshot!.preferences.some(p=>p.key===choice!.key && (choice!.operation!=="clear" || p.userValue===choice!.value))) {
      error="The configuration or project changed. Refresh and review the change again.";choice=null;return;
    }
    writing=true;error="";
    send(choice.operation==="clear"?{kind:"clear",view_id:choice.viewId,key:choice.key}:{kind:"save",view_id:choice.viewId,key:choice.key,value:choice.value});choice=null;editing=null;
  }
  onMount(()=>{
    const command=(event:Event)=>{
      const detail=(event as CustomEvent).detail;
      if(detail?.owner===owner && detail.command==="config" && available()){open();event.preventDefault();}
    };
    window.addEventListener("central-agent:native-command",command);
    const update=(event:Event)=>{
      const d=(event as CustomEvent).detail;
      if(d?.owner!==owner || !dialog.open)return;
      writing=!!d.writing;notice=d.notice||"";
      if(d.error){error=d.error;loading=false;}
      if(d.view?.requestId===requestId){inventory=d.view;loading=!!d.view.loading;if(!d.view.current){choice=null;editing=null;}}
    };
    window.addEventListener("central-agent:app-server-preferences",update);
    return ()=>{close();window.removeEventListener("central-agent:native-command",command);window.removeEventListener("central-agent:app-server-preferences",update);};
  });
</script>

{#if !dialogOnly && conversation?.visible}<button type="button" disabled={!available()} onclick={open}>Codex defaults…</button>{/if}
<dialog bind:this={dialog} class="native-preferences workspace-confirm-dialog" aria-label="Native Codex defaults" onclose={close}>
  <header><h2>Codex defaults</h2><button type="button" onclick={()=>dialog.close()}>Close</button></header>
  <p class="path">{directory}</p>
  <p>Effective configuration on disk for this project, not the settings of an already-running conversation. Your composer selections can override these defaults.</p>
  <button type="button" disabled={!available()||loading||writing} onclick={refresh}>Refresh from Codex</button>
  {#if error||inventory?.error}<p role="alert">{error||inventory?.error}</p>{/if}
  {#if notice}<p role="status">{notice}</p>{/if}
  {#if writing}<p role="status">Waiting for the shared configuration write. Closing this dialog will not cancel or retry it.</p>{/if}
  {#if loading}<p role="status">Loading native configuration…</p>{:else if inventory&&!inventory.current}<p>Last observed settings. Refresh before editing.</p>{/if}
  {#if inventory?.snapshot}
    {#if !inventory.snapshot.target}<p>No enabled base user configuration file was returned. These preferences are read-only.</p>{/if}
    {#each inventory.snapshot.preferences as preference (preference.key)}
      <article>
        <div class="row"><h3>{preferenceLabels[preference.key]||preference.key}</h3><button type="button" disabled={!canEdit()} onclick={()=>select(preference)}>Edit…</button></div>
        <p>Effective: <span class="path">{preference.effective??"Not explicitly configured"}</span></p>
        {#if preference.key==="model_auto_compact_token_limit_scope"}<p>total counts the full active context (Codex default). body_after_prefix counts only growth after the retained compaction-window prefix. Codex owns compaction and token accounting; this does not change the context-usage monitor or increase model capacity.</p>{/if}
        <details><summary>Source and saved default</summary><p class="path">{preference.key}</p><p>User file: {preference.userValue??"Not explicitly configured"}</p>{#if preference.userValue!==null}<button type="button" disabled={!canEdit()} onclick={()=>reviewClear(preference)}>Clear saved value…</button>{/if}<p>Source: {preference.origin?.kind??"Not reported"}{preference.origin?.profile?` · ${preference.origin.profile}`:""}</p>{#if preference.origin?.location}<p class="path">{preference.origin.location}</p>{/if}</details>
        {#if editing?.key===preference.key||choice?.key===preference.key}{@render preferenceEditor()}{/if}
      </article>
    {/each}
    <details><summary>Configuration layers</summary>{#each inventory.snapshot.layers as layer}<p>{layer.kind}{layer.profile?` · ${layer.profile}`:""}{layer.disabledReason?` · Disabled: ${layer.disabledReason}`:""}</p>{#if layer.location}<p class="path">{layer.location}</p>{/if}{/each}</details>
  {/if}
</dialog>

{#snippet preferenceEditor()}
  {#if editing&&!choice}
    <form onsubmit={(event)=>{event.preventDefault();review();}}>
      <h3>Edit {preferenceLabels[editing.key]||editing.key}</h3>
      <label>New saved value
        {#if preferenceChoices[editing.key]}<select bind:value>{#if !preferenceChoices[editing.key].includes(value)}<option value={value}>{value||"Choose a value"}</option>{/if}{#each preferenceChoices[editing.key] as option}<option value={option}>{option}</option>{/each}</select>
        {:else}<input bind:value spellcheck="false" />{/if}
      </label>
      <p>Use the exact native model, effort or service-tier identifier. Codex validates availability and managed policies. Token overrides do not increase a model's supported capacity.</p>
      <div class="row"><button type="button" onclick={()=>editing=null}>Cancel</button><button type="submit" disabled={!canEdit()||!value.trim()}>Review change</button></div>
    </form>
  {/if}
  {#if choice}
    <section aria-label="Confirm shared Codex preference">
      <h3>{choice.operation==="clear"?"Clear this saved Codex value?":"Save this Codex default?"}</h3><p class="path">{choice.key} = {choice.value}</p><p class="path">{choice.file}</p>
      {#if choice.operation==="clear"}<p>Remove only this key from the base user configuration. Codex will determine the resulting value from its remaining configuration layers and built-in defaults. This does not reset the whole configuration.</p>{/if}
      <p>This changes Codex preferences shared by conversations in Supervisor. The native server checks the observed file version and rejects conflicting writes. Project or managed settings may override it.</p>
      <p>Existing conversations are not reloaded. No prompt is sent and no project file is changed.</p>
      <div class="row"><button type="button" onclick={()=>choice=null}>Back</button><button type="button" aria-label={choice.operation==="clear"?"Clear saved value":"Save default"} disabled={!canEdit()} onclick={confirm}>{choice.operation==="clear"?"Clear saved value":"Save default"}</button></div>
    </section>
  {/if}
{/snippet}

<style>
  button,input,select {font:inherit;color:var(--ca-text);background:var(--ca-app-background);border:0;border-radius:var(--ca-control-radius);padding:var(--ca-space-2);}
  button:disabled {color:var(--ca-muted);cursor:not-allowed;}
  button:focus-visible,input:focus-visible,select:focus-visible,summary:focus-visible {box-shadow:var(--ca-focus-ring);}
  .native-preferences {width:min(92vw,70rem);max-width:92vw;max-height:88vh;padding:var(--ca-space-6);border:0;background:var(--ca-surface);color:var(--ca-text);font:var(--ca-type-body)/var(--ca-leading-body) var(--ca-font-body);overflow:auto;}
  header,.row {display:flex;flex-wrap:wrap;align-items:center;gap:var(--ca-space-3);}
  header h2,.row h3 {flex:1;}
  h2 {font-size:var(--ca-type-title);margin:0;}
  h3 {font-size:var(--ca-type-label);margin:0;overflow-wrap:anywhere;}
  p {margin:var(--ca-space-2) 0;overflow-wrap:anywhere;color:var(--ca-muted);}
  p[role="alert"] {color:var(--ca-text);}
  label {display:grid;gap:var(--ca-space-1);min-width:0;}
  input,select {min-width:0;}
  article,section,form {padding-block:var(--ca-space-4);}
  .path {font-family:var(--ca-font-mono);}
  summary {cursor:pointer;}
</style>
