<script lang="ts">
  import { onMount } from "svelte";
  import ConfigurationSection from "./ConfigurationSection.svelte";
  import SettingsPicker from "./SettingsPicker.svelte";
  type Access="readOnly"|"workspaceWrite"|"fullAccess";
  type Summary="auto"|"concise"|"detailed"|"none";
  type Presentation="all"|"summary"|"workspace";
  type View={visible:boolean;selected:Access;summary:Summary|null;disabled:boolean;permissionsDisabled?:boolean;permissionsBusy?:boolean;error?:string|null;options:{value:Access;label:string;allowed:boolean;reason?:string}[]};
  let {owner:fixedOwner,eventTarget,settingsVisible=true,presentation="all"}:{owner?:string;eventTarget?:HTMLElement;settingsVisible?:boolean;presentation?:Presentation}=$props();
  let owner=$state(""),view=$state<View|null>(null),error=$state("");
  let confirmation=$state<HTMLDialogElement>();
  let confirmingOwner="";
  onMount(()=>{
    owner=fixedOwner||"";
    const target=eventTarget||window;
    const switchOwner=(event:Event)=>{
      if(fixedOwner)return;
      const next=String((event as CustomEvent).detail||"");
      if(next!==owner){owner=next;view=null;error="";confirmation?.close();}
    };
    const update=(event:Event)=>{
      const d=(event as CustomEvent).detail;
      if(d?.owner!==owner)return;
      if(d.nativeAccess)view=d.nativeAccess;
      if(event.type==="central-agent:app-server-access")error=d.error||"";
      if(!view?.visible||(view.permissionsDisabled??view.disabled))confirmation?.close();
    };
    window.addEventListener("central-agent:conversation-key",switchOwner);
    window.addEventListener("central-agent:app-server-access",update);
    target.addEventListener("central-agent:conversation-state",update);
    return ()=>{window.removeEventListener("central-agent:conversation-key",switchOwner);window.removeEventListener("central-agent:app-server-access",update);target.removeEventListener("central-agent:conversation-state",update);};
  });
  function choose(value:Access):void {
    if(!view?.visible || (view.permissionsDisabled??view.disabled) || !view.options.some(option=>option.value===value && option.allowed))return;
    if(value==="fullAccess"){confirmingOwner=owner;confirmation?.showModal();return;}
    send(value==="workspaceWrite"?"workspace_write":"read_only");
  }
  function selectSummary(value:string):void {
    if(!view?.visible || view.disabled)return;
    if(value!=="inherit" && !["auto","concise","detailed","none"].includes(value))return;
    send("set_summary",{value:value==="inherit"?null:value});
  }
  function send(kind:string,fields:Record<string,unknown>={}):void {
    error="";
    (eventTarget||document).dispatchEvent(new CustomEvent("central-agent:app-server-access-control",{bubbles:true,detail:{owner,action:{kind,...fields}}}));
  }
  function confirm():void {
    const dialog=confirmation;
    if(!dialog?.open)return;
    dialog.close();
    if(confirmingOwner===owner && view?.visible && !(view.permissionsDisabled??view.disabled) && view.options.some(option=>option.value==="fullAccess" && option.allowed))send("confirm_full_access");
  }
  const accessDescription=(value:Access)=>value==="readOnly"?"Inspect projects and ask before making changes.":value==="workspaceWrite"?"Edit assigned projects and ask before accessing anything else.":"Run commands and edit files without Codex approval prompts.";
  const summaryOptions=[
    {value:"auto",label:"Automatic",description:"Let Codex choose the appropriate progress detail."},
    {value:"concise",label:"Concise",description:"Show shorter public progress summaries."},
    {value:"detailed",label:"Detailed",description:"Show more complete public progress summaries."},
    {value:"none",label:"Off",description:"Hide public reasoning summaries."},
    {value:"inherit",label:"Inherit native setting",description:"Keep the setting already reported by Codex."},
  ];
</script>
{#if settingsVisible && view?.visible}
  {#if presentation==="summary" || presentation==="all"}
  <div class="native-access native-summary settings-option-panel">
    <SettingsPicker id="reasoning-summary-select" label="Reasoning summaries" description="Progress detail shown while Codex works" value={view.summary??"inherit"} disabled={view.disabled} options={summaryOptions} onSelect={selectSummary} />
    {#if view.summary===null}<p class="settings-note">The selected conversation keeps the summary preference already reported by Codex.</p>{/if}
  </div>
  {/if}
  {#if presentation==="workspace" || presentation==="all"}
  <ConfigurationSection section="access" label="Codex workspace access" scope="All Codex agents" icon="folder" initiallyOpen collapsible={false}>
    <div class="native-access native-workspace-access settings-option-panel">
      <SettingsPicker id="codex-workspace-access-select" label="Workspace access" description="Applies to new and existing Codex conversations." value={view.selected} disabled={view.permissionsDisabled??view.disabled} options={view.options.map(option=>({value:option.value,label:`${option.label}${!option.allowed?" · managed restriction":""}`,description:option.reason||accessDescription(option.value),disabled:!option.allowed}))} onSelect={value=>choose(value as Access)} />
      <p class="access-note">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M12 11v6"/><path d="M12 7h.01"/></svg>
        <span>{view.selected==="readOnly"?"Agents can read project files. Editing requires approval.":view.selected==="workspaceWrite"?"Agents can edit assigned projects and ask before accessing anything else.":"Agents can run commands and edit files without Codex approval prompts."}</span>
      </p>
      <p class="settings-note">This choice is shared by every Codex agent and remains active after restarting Supervisor. Supervisor's own tools use the confirmation policy below.</p>
      {#if view.permissionsBusy}<p class="settings-note">Finish or clear active and queued Codex work before changing shared permissions.</p>{/if}
      {#if view.options.some(option=>option.value===view?.selected && !option.allowed)}<p class="settings-note">The saved choice is unavailable under the current managed restrictions. Select an allowed option before sending another prompt.</p>{/if}
      {#if view.options.every(o=>!o.allowed)}<p class="settings-note">{view.options[0]?.reason||"Native permission requirements are unavailable."}</p>{/if}
    </div>
  </ConfigurationSection>
  {/if}
  {#if error || view.error}<p role="alert">{error || view.error}</p>{/if}
{/if}
{#if presentation!=="summary"}
<dialog bind:this={confirmation} class="workspace-confirm-dialog" aria-label="Allow full Codex access?">
  <h2>Allow full access?</h2>
  <p>All Codex agents in Supervisor will be able to run commands and modify files outside their projects, with network access and without command approval prompts. This choice is saved and remains active after restarting the app, until you change it. Connected tools may still ask for separate permission.</p>
  <div class="workspace-confirm-actions"><button type="button" onclick={()=>confirmation?.close()}>Cancel</button><button type="button" onclick={confirm}>Allow full access</button></div>
</dialog>
{/if}
<style>
  .native-access {display:grid;gap:var(--ca-space-3);min-width:0;color:var(--ca-text);font:var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body);}
  .native-summary {padding:0;}
  .native-workspace-access {padding-block:var(--ca-space-2);}
  button {font:inherit;color:var(--ca-text);background:var(--ca-app-background);border:0;border-radius:var(--ca-control-radius);padding:var(--ca-space-2);}
  p {margin:0;color:var(--ca-muted);font-size:var(--ca-type-caption);overflow-wrap:anywhere;}
  .access-note {display:flex;align-items:flex-start;gap:var(--ca-space-2);}
  .access-note svg {width:var(--ca-space-5);height:var(--ca-space-5);flex:0 0 auto;margin-top:1px;color:var(--ca-muted);}
  button:focus-visible {box-shadow:var(--ca-focus-ring);}
  dialog {border:0;background:var(--ca-surface);color:var(--ca-text);}
</style>
