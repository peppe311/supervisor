<script lang="ts">
  import { onDestroy, tick, untrack } from "svelte";
  import PluginIcon from "./PluginIcon.svelte";
  import {appSelectable,type AppsAction,type NativeApp,type SelectedNativeApp} from "../lib/native-apps";
  import type {SelectedSkill} from "../lib/native-skills";
  import type {NativeConversationView as View} from "../lib/native-history";

  const BROWSER_APP_ID="browser@openai-bundled";
  const BROWSER_SKILL_NAME="browser:control-in-app-browser";

  let {owner:fixedOwner,eventTarget,compact=false}:{owner?:string;eventTarget?:HTMLElement;compact?:boolean}=$props();
  let owner=$state(untrack(()=>fixedOwner||""));
  let view=$state<View|null>(null);
  let open=$state(false);
  let host=$state<HTMLDivElement>();
  let trigger=$state<HTMLButtonElement>();
  let selectedSkills=$state<SelectedSkill[]>([]);
  let browserUse=$state("unavailable");
  let skillsWriting=$state(false);
  let skillIdentity="";

  const apps=$derived(view?.apps||null);
  const selectedApps=$derived(apps?.selected||[]);
  const browserSelection=$derived(selectedSkills.find(skill=>skill.name===BROWSER_SKILL_NAME));
  const selectionCount=$derived(selectedApps.length+(browserSelection?1:0));
  const browserAvailable=$derived(browserUse==="available");
  const catalogItems=$derived(apps?.current?apps.items.filter(app=>!browserAvailable||app.id!==BROWSER_APP_ID):[]);
  const hasChoices=$derived(browserAvailable||catalogItems.length>0);
  const canControl=$derived(Boolean(view?.visible && view.connected && (!view.binding || !view.binding.deleted && !view.binding.archived)));
  const appBlocked=$derived(!canControl || !!apps?.busy);
  const browserBlocked=$derived(!canControl || skillsWriting);
  const suffix=$derived((fixedOwner||"main").replace(/[^a-zA-Z0-9_-]/g,"-")||"main");
  const buttonId=$derived(`plugin-picker-${suffix}-button`);
  const popoverId=$derived(`plugin-picker-${suffix}-popover`);
  const title=$derived(selectionCount?`${selectionCount} ${selectionCount===1?'plugin selected':'plugins selected'} for the next prompt`:"Choose a plugin for the next prompt");

  function chosen(appId:string):SelectedNativeApp|undefined {
    return selectedApps.find(item=>item.appId===appId);
  }

  function dispatch(action:AppsAction):void {
    if(!canControl || !view)return;
    if(action.kind==='attach' && (!apps?.current || apps.inventoryId!==action.inventory_id))return;
    (eventTarget||document).dispatchEvent(new CustomEvent("central-agent:app-server-conversation-control",{
      bubbles:true,
      detail:{owner,action:{kind:"apps_control",expected_thread_id:view.binding?.threadId??null,action}},
    }));
  }

  function refresh():void {
    if(!appBlocked)dispatch({kind:"refresh"});
  }

  function dispatchSkill(action:Record<string,unknown>):void {
    if(!canControl || !view)return;
    (eventTarget||document).dispatchEvent(new CustomEvent("central-agent:app-server-skills-control",{
      bubbles:true,
      detail:{owner,action},
    }));
  }

  function choose(app:NativeApp):void {
    if(appBlocked)return;
    const selection=chosen(app.id);
    if(selection){close(true);return;}
    if(apps?.current && apps.inventoryId && appSelectable(app))dispatch({kind:'attach',inventory_id:apps.inventoryId,app_id:app.id});
    else return;
    close(true);
  }

  function remove(selection:SelectedNativeApp):void {
    if(!appBlocked)dispatch({kind:'remove',selection_id:selection.selectionId});
  }

  function chooseBrowser():void {
    if(browserBlocked || !browserAvailable)return;
    if(!browserSelection)dispatchSkill({kind:"attach_official_browser"});
    close(true);
  }

  function removeBrowser():void {
    if(!browserBlocked && browserSelection)dispatchSkill({kind:"remove",id:browserSelection.id});
  }

  function close(returnFocus=false):void {
    if(!open)return;
    open=false;
    if(returnFocus)queueMicrotask(()=>trigger?.focus());
  }

  async function toggleMenu():Promise<void> {
    if(open){close(true);return;}
    if(!canControl)return;
    open=true;
    if(!apps?.busy && !apps?.error && !apps?.unavailableReason && (!apps?.current || !apps.inventoryId))refresh();
    await tick();
    host?.querySelector<HTMLButtonElement>(".plugin-choice:not(:disabled)")?.focus();
  }

  const switchOwner=(event:Event)=>{
    if(fixedOwner)return;
    const next=String((event as CustomEvent<string>).detail||"");
    if(next!==owner){owner=next;view=null;selectedSkills=[];browserUse="unavailable";skillsWriting=false;skillIdentity="";close(false);}
  };
  const update=(event:Event)=>{
    const detail=(event as CustomEvent).detail;
    if(detail?.owner!==owner || !detail.nativeConversation)return;
    const previousThread=view?.binding?.threadId||"";
    const next=detail.nativeConversation as View;
    view=next;
    const nextSkillIdentity=`${owner}|${next.directory||""}|${next.visible}|${next.connected}`;
    if(nextSkillIdentity!==skillIdentity){
      skillIdentity=nextSkillIdentity;
      selectedSkills=[];
      browserUse="unavailable";
      skillsWriting=false;
      if(next.visible)dispatchSkill({kind:"selection"});
    }
    if(!next.visible || previousThread && previousThread!==next.binding?.threadId)close(false);
  };
  const updateSkills=(event:Event)=>{
    const detail=(event as CustomEvent).detail;
    if(detail?.owner!==owner)return;
    if(Array.isArray(detail.selected))selectedSkills=detail.selected;
    if(typeof detail.browserUse==="string")browserUse=detail.browserUse;
    skillsWriting=detail.writing===true;
  };
  const outside=(event:Event)=>{
    if(open && event.target instanceof Node && !host?.contains(event.target))close(false);
  };
  const keyboard=(event:KeyboardEvent)=>{
    if(open && event.key==="Escape"){event.preventDefault();event.stopPropagation();close(true);}
  };
  const target=untrack(()=>eventTarget)||window;
  if(typeof window!=="undefined"){
    window.addEventListener("central-agent:conversation-key",switchOwner);
    window.addEventListener("central-agent:app-server-conversation",update);
    window.addEventListener("central-agent:app-server-skills",updateSkills);
    target.addEventListener("central-agent:conversation-state",update);
    document.addEventListener("pointerdown",outside,true);
    window.addEventListener("keydown",keyboard,true);
  }
  onDestroy(()=>{
    if(typeof window!=="undefined"){
      window.removeEventListener("central-agent:conversation-key",switchOwner);
      window.removeEventListener("central-agent:app-server-conversation",update);
      window.removeEventListener("central-agent:app-server-skills",updateSkills);
      target.removeEventListener("central-agent:conversation-state",update);
      document.removeEventListener("pointerdown",outside,true);
      window.removeEventListener("keydown",keyboard,true);
    }
  });
</script>

{#if view?.visible}
  <div class="plugin-picker" class:compact bind:this={host} data-plugin-picker={suffix}>
    <button
      id={buttonId}
      class="plugin-picker-trigger"
      class:selected={selectionCount>0}
      type="button"
      bind:this={trigger}
      aria-label={title}
      title={canControl?title:"Connect Codex to choose plugins"}
      aria-haspopup="listbox"
      aria-expanded={open}
      aria-controls={popoverId}
      disabled={!canControl}
      onclick={toggleMenu}
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M8.5 4.5v4m7-4v4M6.5 8.5h11v3a5.5 5.5 0 0 1-11 0v-3Z" />
        <path d="M12 17v3" />
      </svg>
    </button>

    {#if browserSelection}
      <div
        class="selected-plugin-chip"
        class:stale={!browserSelection.current}
        data-plugin-id={BROWSER_APP_ID}
        aria-label="Browser selected for the next prompt"
      >
        <PluginIcon appId={BROWSER_APP_ID} name="Browser" />
        <span class="selected-plugin-name">Browser</span>
        <button
          class="selected-plugin-remove"
          type="button"
          disabled={browserBlocked}
          aria-label="Remove Browser from the next prompt"
          title="Remove Browser"
          onclick={removeBrowser}
        >
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" aria-hidden="true">
            <path d="m4 4 8 8M12 4l-8 8" />
          </svg>
        </button>
      </div>
    {/if}

    {#each selectedApps as selection (selection.selectionId)}
      <div
        class="selected-plugin-chip"
        class:stale={!selection.current}
        data-plugin-id={selection.appId}
        aria-label={`${selection.name} selected for the next prompt`}
      >
        <PluginIcon appId={selection.appId} name={selection.name} iconUrl={selection.iconUrl} />
        <span class="selected-plugin-name">{selection.name}</span>
        <button
          class="selected-plugin-remove"
          type="button"
          disabled={appBlocked}
          aria-label={`Remove ${selection.name} from the next prompt`}
          title={`Remove ${selection.name}`}
          onclick={()=>remove(selection)}
        >
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" aria-hidden="true">
            <path d="m4 4 8 8M12 4l-8 8" />
          </svg>
        </button>
      </div>
    {/each}

    {#if open}
      <div id={popoverId} class="plugin-popover" role="listbox" aria-label="Plugins">
        {#if apps?.busy && !hasChoices}
          <p class="state">Loading plugins…</p>
        {:else if apps?.error && !hasChoices}
          <p class="state" role="alert">{apps.error}</p>
        {:else if apps?.unavailableReason && !hasChoices}
          <p class="state" role="status">{apps.unavailableReason}</p>
        {:else if !apps && !hasChoices}
          <p class="state" role="status">Loading plugins…</p>
        {:else if !apps?.current && !hasChoices}
          <p class="state">Plugins are unavailable. Refresh them in Settings.</p>
        {:else if !hasChoices}
          <p class="state">No plugins available.</p>
        {:else}
          <ul>
            {#if browserAvailable}
              <li>
                <button
                  class="plugin-choice"
                  class:selected={!!browserSelection}
                  type="button"
                  data-plugin-id={BROWSER_APP_ID}
                  disabled={browserBlocked}
                  role="option"
                  aria-selected={!!browserSelection}
                  aria-label={browserSelection?'Browser selected for the next prompt':'Select Browser for the next prompt'}
                  onclick={chooseBrowser}
                >
                  <PluginIcon appId={BROWSER_APP_ID} name="Browser" />
                  <span>Browser</span>
                </button>
              </li>
            {/if}
            {#each catalogItems as app (app.id)}
              {@const selection=chosen(app.id)}
              <li>
                <button
                  class="plugin-choice"
                  class:selected={!!selection}
                  type="button"
                  data-plugin-id={app.id}
                  disabled={appBlocked || (!selection && !appSelectable(app))}
                  role="option"
                  aria-selected={!!selection}
                  aria-label={selection?`${app.name} selected for the next prompt`:`Select ${app.name} for the next prompt`}
                  onclick={()=>choose(app)}
                >
                  <PluginIcon appId={app.id} name={app.name} iconUrl={app.iconUrl} />
                  <span>{app.name}</span>
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  .plugin-picker{position:relative;display:inline-flex;min-width:0;flex:0 1 auto;align-items:center;gap:var(--ca-space-1);font:var(--ca-type-body)/var(--ca-leading-body) var(--ca-font-body);color:var(--ca-text)}
  .plugin-picker-trigger{position:relative;display:grid;width:var(--ca-control-compact);min-width:var(--ca-control-compact);height:var(--ca-control-compact);place-items:center;padding:0;border:0;border-radius:var(--ca-control-radius);background:transparent;color:var(--ca-muted);box-shadow:none;cursor:pointer}
  .plugin-picker-trigger svg{width:var(--ca-space-5);height:var(--ca-space-5)}
  .plugin-picker-trigger:hover:not(:disabled),.plugin-picker-trigger[aria-expanded="true"],.plugin-picker-trigger.selected{background:var(--ca-surface-2);color:var(--ca-text)}
  .plugin-picker-trigger:focus-visible,.plugin-choice:focus-visible,.selected-plugin-remove:focus-visible{outline:0;box-shadow:var(--ca-focus-ring)}
  .plugin-picker-trigger:disabled{color:var(--ca-faint);cursor:default}
  .selected-plugin-chip{display:flex;min-width:0;max-width:calc(var(--ca-plugin-popup-width) - var(--ca-space-8));height:var(--ca-control-compact);align-items:center;gap:var(--ca-space-2);padding:0 var(--ca-space-1) 0 var(--ca-space-2);border:0;border-radius:var(--ca-pill);background:var(--ca-surface-2);color:var(--ca-text);font:650 var(--ca-type-label)/var(--ca-leading-compact) var(--ca-font-body)}
  .selected-plugin-chip.stale{color:var(--ca-muted)}
  .selected-plugin-name{min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .selected-plugin-remove{display:grid;width:var(--ca-space-6);min-width:var(--ca-space-6);height:var(--ca-space-6);place-items:center;padding:0;border:0;border-radius:var(--ca-pill);background:transparent;color:var(--ca-muted);cursor:pointer}
  .selected-plugin-remove svg{width:var(--ca-space-4);height:var(--ca-space-4)}
  .selected-plugin-remove:hover:not(:disabled){background:var(--ca-surface-3);color:var(--ca-text)}
  .selected-plugin-remove:disabled{color:var(--ca-faint);cursor:default}
  .plugin-popover{position:absolute;z-index:130;bottom:calc(100% + var(--ca-space-2));left:0;box-sizing:border-box;width:min(var(--ca-plugin-popup-width),calc(100vw - var(--ca-space-8)));max-height:var(--ca-plugin-popup-max-height);padding:var(--ca-space-2);overflow:auto;overscroll-behavior:contain;border:0;border-radius:var(--ca-radius-large);background:var(--ca-app-background);box-shadow:var(--ca-shadow-float),var(--ca-shadow-keyline);animation:plugin-popover-in var(--ca-duration-standard) var(--ca-ease-emphasized);transform-origin:bottom left}
  ul{display:grid;gap:var(--ca-space-1);margin:0;padding:0;list-style:none}
  .plugin-choice{display:flex;width:100%;min-height:var(--ca-control-prominent);align-items:center;gap:var(--ca-space-3);padding:var(--ca-space-2) var(--ca-space-3);border:0;border-radius:var(--ca-control-radius);background:transparent;color:var(--ca-text);font:650 var(--ca-type-body)/var(--ca-leading-body) var(--ca-font-body);text-align:left;cursor:pointer}
  .plugin-choice:hover:not(:disabled),.plugin-choice.selected{background:var(--ca-surface-2)}
  .plugin-choice:disabled{color:var(--ca-faint);cursor:default}
  .plugin-choice>span:last-child{min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .state{margin:0;padding:var(--ca-space-3);color:var(--ca-muted);font-size:var(--ca-type-caption);line-height:var(--ca-leading-body);overflow-wrap:anywhere}
  @keyframes plugin-popover-in{from{opacity:0;transform:translateY(var(--ca-space-2)) scale(.98)}to{opacity:1;transform:none}}
  @media (prefers-reduced-motion:reduce){.plugin-popover{animation:none}}
  @media (max-width:520px){.plugin-popover{left:calc(-1 * var(--ca-space-2));width:min(var(--ca-plugin-popup-width),calc(100vw - var(--ca-space-5)))}}
</style>
