<script lang="ts">
  import { requestId as newRequestId } from "../lib/request-id";
  import {onMount} from "svelte";
  import type {NativeConversationView} from "../lib/native-history";
  import type {NativeSkill,NativeSkillsView,SelectedSkill} from "../lib/native-skills";
  let {owner,eventTarget,conversation,dialogOnly=false}:{owner:string;eventTarget?:HTMLElement;conversation:NativeConversationView|null;dialogOnly?:boolean}=$props();
  let dialog:HTMLDialogElement;
  let inventory=$state<NativeSkillsView|null>(null),error=$state(""),notice=$state(""),search=$state("");
  let loading=$state(false),writing=$state(false);
  let selected=$state<SelectedSkill[]>([]);
  let extraRoots=$state<string[]>([]),rootDraft=$state(""),confirmRoots=$state(false);
  let computerUse=$state("unavailable");
  let browserUse=$state("unavailable");
  let computerUseConnection=$state("unchecked"),computerUseError=$state("");
  let computerUseCanReconnect=$state(false);
  function computerUseMessage():string {
    if(computerUse==="loading")return "Checking the official runtime…";
    if(computerUse!=="available")return "Enable the Computer Use plugin in Codex desktop, then reconnect.";
    if(computerUseConnection==="connection_error")return "Desktop connection failed. Open Codex desktop and reconnect when all agents are idle. The last action has not been repeated.";
    if(computerUseConnection==="responded")return "Last desktop request completed.";
    return "Configured. The desktop connection has not been verified in this session.";
  }
  function browserUseMessage():string {
    if(browserUse==="loading")return "Checking the official runtime…";
    if(browserUse!=="available")return "Enable the Browser plugin in Codex desktop, then reconnect.";
    return "Available in the composer plugin selector for the next prompt.";
  }
  function reconnectComputerUse():void {
    if(!available() || !computerUseCanReconnect || writing || loading || conversation?.busy)return;
    computerUseCanReconnect=false;send({kind:"reconnect"});
  }
  let selectionIdentity="";
  let choice=$state<{skill:NativeSkill;viewId:string;owner:string;directory:string;enabled:boolean}|null>(null);
  let requestId="",dialogOwner="";
  let directory=$state("");
  $effect(()=>{
    if(owner!==dialogOwner || !conversation?.visible || !conversation.connected || conversation.directory!==directory) {
      close();dialog?.close();
    }
  });
  $effect(()=>{
    const identity=`${owner}|${conversation?.directory||""}|${conversation?.visible}|${conversation?.connected}`;
    if(identity!==selectionIdentity){selectionIdentity=identity;selected=[];if(conversation?.visible)send({kind:"selection"});}
  });
  function available():boolean {return !!conversation?.visible && conversation.connected && !!conversation.directory;}
  function send(action:Record<string,unknown>,destination=owner):void {
    (eventTarget||document).dispatchEvent(new CustomEvent("central-agent:app-server-skills-control",{bubbles:true,detail:{owner:destination,action}}));
  }
  function close():void {
    if(requestId && dialogOwner)send({kind:"close",request_id:requestId},dialogOwner);
    requestId="";inventory=null;choice=null;loading=false;
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
  function select(skill:NativeSkill):void {
    if(!available() || !inventory?.current || loading || writing || conversation?.busy || inventory.directory!==conversation?.directory)return;
    choice={skill,viewId:inventory.viewId,owner,directory:inventory.directory,enabled:!skill.enabled};
  }
  function attach(skill:NativeSkill):void {
    if(!available()||!inventory?.current||inventory.directory!==conversation?.directory||loading||writing||!skill.enabled)return;
    send({kind:"attach",view_id:inventory.viewId,path:skill.path});
  }
  function remove(id:string):void {send({kind:"remove",id});}
  function prepareRoots():void {
    if(!available() || writing || loading || conversation?.busy)return;
    rootDraft=extraRoots.join("\n");confirmRoots=true;
  }
  function saveRoots():void {
    if(!confirmRoots || !available() || writing || loading || conversation?.busy)return;
    const roots=[...new Set(rootDraft.split(/\r?\n/).map(root=>root.trim()).filter(Boolean))];
    if(roots.length>16){error="Select at most 16 extra skill roots.";return;}
    writing=true;error="";confirmRoots=false;send({kind:"set_extra_roots",roots});
  }
  function confirm():void {
    if(!choice || !available() || writing || loading || conversation?.busy)return;
    const current=inventory?.items.find(item=>item.path===choice!.skill.path);
    if(!inventory?.current || choice.viewId!==inventory.viewId || owner!==choice.owner || conversation?.directory!==choice.directory || !current || current.enabled===choice.enabled) {
      error="The skill inventory or project changed. Refresh and select the skill again.";choice=null;return;
    }
    writing=true;error="";send({kind:"set_enabled",view_id:choice.viewId,path:choice.skill.path,enabled:choice.enabled});choice=null;
  }
  onMount(()=>{
    const command=(event:Event)=>{
      const detail=(event as CustomEvent).detail;
      if(detail?.owner===owner && detail.command==="skills" && available()){open();event.preventDefault();}
    };
    window.addEventListener("central-agent:native-command",command);
    const update=(event:Event)=>{
      const d=(event as CustomEvent).detail;
      if(d?.owner!==owner)return;
      if(Array.isArray(d.selected))selected=d.selected;
      if(Array.isArray(d.extraRoots))extraRoots=d.extraRoots;
      if(typeof d.computerUse==="string")computerUse=d.computerUse;
      if(typeof d.browserUse==="string")browserUse=d.browserUse;
      if(typeof d.computerUseConnection==="string")computerUseConnection=d.computerUseConnection;
      if("computerUseError" in d)computerUseError=typeof d.computerUseError==="string"?d.computerUseError:"";
      computerUseCanReconnect=d.computerUseCanReconnect===true;
      if(!dialog.open)return;
      writing=!!d.writing;notice=d.notice||"";
      if(d.error){error=d.error;loading=false;}
      if(d.view?.requestId===requestId){inventory=d.view;loading=!!d.view.loading;if(!d.view.current)choice=null;}
    };
    window.addEventListener("central-agent:app-server-skills",update);
    return ()=>{close();window.removeEventListener("central-agent:native-command",command);window.removeEventListener("central-agent:app-server-skills",update);};
  });
</script>

{#if !dialogOnly && conversation?.visible}<button type="button" disabled={!available()} onclick={open}>Codex skills…</button>{/if}
{#if !dialogOnly && conversation?.visible && selected.length}
  <section class="selected-skills" data-config-persistent aria-label="Skills selected for the next prompt">
    <p>With your next prompt</p>
    {#each selected as skill (skill.id)}<div class="actions"><span>${skill.name}{!skill.current||skill.directory!==conversation.directory?" · Refresh or remove":""}</span><button type="button" aria-label={`Remove ${skill.name} from the next prompt`} onclick={()=>remove(skill.id)}>Remove</button></div>{/each}
    <p>Codex loads the selected instructions when you send or queue a prompt. Selection alone runs nothing.</p>
  </section>
{/if}
<dialog bind:this={dialog} class="native-skills workspace-confirm-dialog" aria-label="Native Codex skills" onclose={close}>
  <header><h2>Codex skills</h2><button type="button" onclick={()=>dialog.close()}>Close</button></header>
  <p class="path">{directory}</p>
  <p>Skills discovered by Codex for this project. This list does not read their instructions into the chat or run a model.</p>
  <p role="status">Official Computer Use: {computerUseMessage()}</p>
  <p role="status">Official Browser: {browserUseMessage()}</p>
  {#if computerUseError}<p role="alert">{computerUseError}</p>{/if}
  {#if computerUse==="unavailable" && browserUse==="unavailable" || computerUseConnection==="connection_error" || computerUseError}
    <button type="button" disabled={!computerUseCanReconnect||writing||loading||!available()||!!conversation?.busy} onclick={reconnectComputerUse}>Reconnect Codex</button>
    <p>Reconnects all Codex agents and reloads the official plugin references. Finish active and queued work first.</p>
  {/if}
  <form onsubmit={(event)=>{event.preventDefault();refresh();}}>
    <label>Filter skills<input bind:value={search} /></label>
    <button type="submit" disabled={writing||loading||!available()}>Refresh from Codex</button>
  </form>
  <section aria-label="Process-scoped extra skill roots">
    <h3>Extra discovery roots</h3>
    <p>{extraRoots.length?extraRoots.join(" · "):"No extra roots are active in this App Server process."}</p>
    <button type="button" disabled={writing||loading||!available()||!!conversation?.busy} onclick={prepareRoots}>Configure extra roots…</button>
  </section>
  {#if error||inventory?.error}<p role="alert">{error||inventory?.error}</p>{/if}
  {#if notice}<p role="status">{notice}</p>{/if}
  {#if writing}<p role="status">Waiting for Codex to change its shared configuration. Closing this dialog does not cancel or retry the request.</p>{/if}
  {#if loading}<p role="status">Loading skills…</p>{:else if inventory && !inventory.current}<p>Last observed inventory. Refresh before changing a skill.</p>{/if}
  {#if inventory?.current && !inventory.items.length}<p>No skills were returned for this directory.</p>{/if}
  {#each (inventory?.items||[]).filter(item=>`${item.name} ${item.description} ${item.path}`.toLowerCase().includes(search.toLowerCase())) as skill (skill.path)}
    <article>
      <h3>{skill.name}</h3><p>{skill.description}</p>
      <p>{skill.scope} · {skill.enabled?"Enabled":"Disabled"}{skill.pluginId?` · ${skill.pluginId}`:""}</p>
      <details><summary>Skill path</summary><p class="path">{skill.path}</p></details>
      <button type="button" disabled={!inventory?.current||loading||writing||!skill.enabled||selected.some(s=>s.path===skill.path&&s.directory===inventory?.directory)} onclick={()=>attach(skill)}>{selected.some(s=>s.path===skill.path&&s.directory===inventory?.directory)?"Selected for next prompt":"Use with next prompt"}</button>
      <button type="button" disabled={!inventory?.current||loading||writing||!!conversation?.busy} onclick={()=>select(skill)}>{skill.enabled?"Disable…":"Enable…"}</button>
    </article>
  {/each}
  {#if inventory?.errors.length}<section aria-label="Skill discovery errors"><h3>Discovery issues</h3>{#each inventory.errors as issue}<p>{issue.path}: {issue.message}</p>{/each}</section>{/if}
  {#if choice}
    <section aria-label="Confirm shared skill configuration">
      <h3>{choice.enabled?"Enable":"Disable"} {choice.skill.name}?</h3><p class="path">{choice.skill.path}</p>
      <p>This changes Codex's shared skill configuration, including other conversations and clients using the same Codex configuration. It is not a setting for only this chat. No skill files are deleted and no prompt is sent.</p>
      <p>Codex policies remain authoritative. Refresh after the change to see the effective state; existing sessions may keep their loaded configuration.</p>
      <div class="actions"><button type="button" onclick={()=>choice=null}>Cancel</button><button type="button" disabled={writing||!!conversation?.busy} onclick={confirm}>Confirm {choice.enabled?"enable":"disable"}</button></div>
    </section>
  {/if}
  {#if confirmRoots}
    <section aria-label="Confirm process-scoped extra skill roots">
      <h3>Set extra skill roots for this Codex process?</h3>
      <label>One existing absolute directory per line<textarea bind:value={rootDraft} rows={5} spellcheck="false"></textarea></label>
      <p>This replaces the extra roots for the current App Server process and affects discovery in other native conversations. It does not copy, edit, enable, or run any skill. The setting is not persisted by Supervisor and is cleared on reconnect.</p>
      <div class="actions"><button type="button" onclick={()=>confirmRoots=false}>Cancel</button><button type="button" disabled={writing||loading||!!conversation?.busy} onclick={saveRoots}>Set process roots</button></div>
    </section>
  {/if}
</dialog>

<style>
  button,input,textarea {font:inherit;color:var(--ca-text);background:var(--ca-app-background);border:0;border-radius:var(--ca-control-radius);padding:var(--ca-space-2);}
  button:disabled {color:var(--ca-muted);cursor:not-allowed;}
  button:focus-visible,input:focus-visible,textarea:focus-visible,summary:focus-visible {box-shadow:var(--ca-focus-ring);}
  .native-skills {width:min(92vw,70rem);max-width:92vw;max-height:88vh;padding:var(--ca-space-6);border:0;background:var(--ca-surface);color:var(--ca-text);font:var(--ca-type-body)/var(--ca-leading-body) var(--ca-font-body);overflow:auto;}
  header,.actions,form {display:flex;flex-wrap:wrap;align-items:center;gap:var(--ca-space-3);}
  header h2,label {flex:1;}
  h2 {font-size:var(--ca-type-title);margin:0;}
  h3 {font-size:var(--ca-type-label);margin:0;overflow-wrap:anywhere;}
  p {margin:var(--ca-space-2) 0;overflow-wrap:anywhere;color:var(--ca-muted);}
  p[role="alert"] {color:var(--ca-text);}
  label {display:grid;gap:var(--ca-space-1);min-width:0;}
  input,textarea {min-width:0;box-sizing:border-box;}
  article,section,form {padding-block:var(--ca-space-4);}
  .path {font-family:var(--ca-font-mono);}
  summary {cursor:pointer;}
</style>
