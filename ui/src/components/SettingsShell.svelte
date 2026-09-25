<script lang="ts">
  import {onMount,tick} from "svelte";
  import SettingsNavigation from "./SettingsNavigation.svelte";
  import {resolveSettingsId,settingsCategories,type SettingsId} from "../lib/settings-catalog";
  let {sections,source}:{sections:HTMLElement[];source:HTMLElement}=$props();
  let active=$state<SettingsId>("settings-general");
  let shell:HTMLDivElement,pages:HTMLDivElement,content:HTMLDivElement;
  const category=$derived(settingsCategories.find(item=>item.id===active)!);
  const scrollPositions=new Map<string,number>();
  let mounted=false,visible=false,revision=0;

  function sectionGroup(section:HTMLElement):string {
    return section.dataset.settingsGroup || resolveSettingsId(section.id) || section.id;
  }

  function selectedSections(id:SettingsId):HTMLElement[] {
    if(id==="settings-codex-tools")return sections.filter(section=>section.id==="settings-agent");
    return sections.filter(section=>sectionGroup(section)===id);
  }

  function revealActiveContent():void {
    if(active!=="settings-codex-tools")return;
    const toggle=sections.find(section=>section.id==="settings-agent")?.querySelector<HTMLButtonElement>('[data-config-section="tools"] .section-toggle[aria-expanded="false"]');
    toggle?.click();
  }

  function applySelection():void {
    const selected=new Set(selectedSections(active));
    for(const section of sections) {
      section.hidden=!selected.has(section);
      if(section.id==="settings-agent")section.dataset.settingsPresentation=active;
      if(!section.hidden && section instanceof HTMLDetailsElement)section.open=true;
    }
    revealActiveContent();
  }
  function rememberScroll():void {
    if(content)scrollPositions.set(active,content.scrollTop);
  }
  export function select(sectionId:string,selector?:string):boolean {
    const resolved=resolveSettingsId(sectionId);
    if(!resolved || pages?.querySelector('dialog[open]'))return false;
    rememberScroll();
    active=resolved;applySelection();
    const current=++revision;
    void tick().then(()=>{
      if(current!==revision || !mounted)return;
      revealActiveContent();
      content.scrollTop=scrollPositions.get(active)||0;
      if(!selector)return;
      const activeSections=selectedSections(active);
      let target:HTMLElement|null=null;
      for(const candidate of activeSections) {
        target=candidate.querySelector<HTMLElement>(selector);
        if(target)break;
      }
      if(!target)return;
      const section=activeSections.find(section=>section.contains(target))!;
      let parent:HTMLElement|null=target;
      while(parent && parent!==section.parentElement) {
        if(parent instanceof HTMLDetailsElement)parent.open=true;
        if(parent.hasAttribute('data-config-section')) {
          const toggle=parent.querySelector<HTMLButtonElement>('.section-toggle[aria-expanded="false"]');
          toggle?.click();
        }
        parent=parent.parentElement;
      }
      void tick().then(()=>{
        if(current!==revision || !target.getClientRects().length)return;
        target.scrollIntoView({block:"center",behavior:"instant"});
        if(!target.matches('button,input,select,textarea,[tabindex]'))target.tabIndex=-1;
        target.focus({preventScroll:true});
      });
    });
    return true;
  }
  export function visibilityChanged(open:boolean):void {
    if(visible===open)return;
    visible=open;
    if(!open) {
      rememberScroll();
      pages?.querySelectorAll<HTMLDialogElement>('dialog[open]').forEach(dialog=>dialog.close());
      return;
    }
    applySelection();
    void tick().then(()=>{if(visible && content){revealActiveContent();content.scrollTop=scrollPositions.get(active)||0;}});
  }
  function keyboard(event:KeyboardEvent):void {
    if(!visible)return;
    if(event.key!=="Tab" || event.defaultPrevented || document.querySelector('dialog[open],.modal:not([hidden]),.workspace-confirm-backdrop'))return;
    const controls=[...shell.querySelectorAll<HTMLElement>('button,input,select,textarea,summary,a[href],[tabindex="0"]')].filter(control=>!control.matches(':disabled,[hidden]') && control.getClientRects().length && getComputedStyle(control).visibility!=="hidden");
    const first=controls[0],last=controls.at(-1);
    if(!first || !last)return;
    if(!shell.contains(document.activeElement) || (!event.shiftKey && document.activeElement===last) || (event.shiftKey && document.activeElement===first)) {
      event.preventDefault();(event.shiftKey?last:first).focus();
    }
  }
  onMount(()=>{
    pages.append(...sections);mounted=true;applySelection();
    const jump=(event:MouseEvent)=>{
      const button=event.target instanceof Element?event.target.closest<HTMLElement>('[data-settings-jump]'):null;
      if(button)select(button.dataset.settingsJump!,button.dataset.settingsSelector);
    };
    pages.addEventListener('click',jump);
    return()=>{mounted=false;revision++;pages.removeEventListener('click',jump);source.append(...sections);};
  });
</script>

<svelte:window onkeydown={keyboard} />
<div bind:this={shell} class="settings-shell" role="dialog" aria-modal="true" aria-labelledby="settings-title" tabindex="-1">
  <header class="settings-topbar">
    <h1>Settings</h1>
    <button id="settings-back" class="settings-back" type="button" aria-label="Back to workspace" title="Back to workspace">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m15 18-6-6 6-6"/><path d="M9 12h11"/></svg>
    </button>
  </header>
  <nav id="settings-nav" class="settings-nav" aria-label="Settings categories"><SettingsNavigation {active} onSelect={id=>select(id)} /></nav>
  <div class="settings-main">
    <div class="settings-page-head">
      <div class="settings-page-head-inner">
        <div class="settings-page-title">
          <h2 id="settings-title"><span class="visually-hidden">Settings: </span><span id="settings-category-title">{category.title}</span></h2>
          <p>{category.description}</p>
        </div>
      </div>
    </div>
    <div bind:this={content} id="settings-content" class="settings-content" tabindex="-1">
      <div class="settings-reading-column">
        <div bind:this={pages} class="settings-pages"></div>
      </div>
    </div>
  </div>
</div>

<style>
  :global([data-central-agent-svelte="settings-shell"]) {width:100%;height:100%;min-width:0;min-height:0;container-type:inline-size;container-name:settings;}
  .settings-shell {display:grid;width:100%;height:100%;max-width:none;min-width:0;min-height:0;grid-template-rows:auto auto minmax(0,1fr);overflow:hidden;border:0;border-radius:var(--ca-radius-none);box-shadow:none;background:var(--ca-app-background);color:var(--ca-text);font:var(--ca-type-body)/var(--ca-leading-body) var(--ca-font-body);}
  .settings-topbar {display:grid;grid-template-columns:minmax(0,1fr) auto;align-items:center;gap:var(--ca-space-4);min-width:0;padding:0 var(--ca-space-8);border:0;background:var(--ca-app-background);box-shadow:none;}
  .settings-topbar h1 {margin:0;font:670 var(--ca-type-settings-sidebar-title)/1.2 var(--ca-font-display);letter-spacing:-.02em;text-transform:none;}
  .settings-back {display:grid;width:var(--ca-control-prominent);height:var(--ca-control-prominent);place-items:center;justify-self:end;padding:0;border:0;border-radius:var(--ca-radius-none);background:transparent;color:var(--ca-muted);cursor:pointer;transition:color var(--ca-duration-fast) var(--ca-ease-standard);}
  .settings-back:hover {background:transparent;color:var(--ca-text);}
  .settings-back svg {width:20px;height:20px;}
  :global(body.settings-open .workspace-confirm-dialog),:global(body.settings-open #full-access-modal .modal-card),:global(body.settings-open #full-access-modal .modal-icon),:global(body.settings-open #full-access-modal .action) {border:0;}
  button:focus-visible {outline:2px solid var(--ca-settings-control-focus);outline-offset:var(--ca-space-1);}
  .settings-nav {min-width:0;min-height:0;padding:0 var(--ca-space-8) var(--ca-space-3);overflow:visible;}
  .settings-main {display:grid;grid-template-rows:auto minmax(0,1fr);min-width:0;min-height:0;overflow:hidden;}
  .settings-page-head {padding:0 var(--ca-space-12) var(--ca-space-4);border:0;background:transparent;}
  .settings-page-head-inner,.settings-reading-column {width:100%;max-width:var(--ca-settings-content-width);margin-inline:auto;min-width:0;}
  .settings-page-title {min-width:0;}
  h2 {margin:0;font:700 var(--ca-type-settings-title)/1.15 var(--ca-font-display);letter-spacing:-.025em;}
  .visually-hidden {position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0;}
  .settings-page-title p {max-width:48rem;margin:var(--ca-space-2) 0 0;color:var(--ca-muted);font:400 var(--ca-type-settings-subtitle)/var(--ca-leading-body) var(--ca-font-body);}
  .settings-content {min-width:0;min-height:0;overflow:auto;padding:0 var(--ca-space-12) var(--ca-space-12);background:transparent;scroll-behavior:auto;scrollbar-gutter:stable;}
  .settings-pages {min-width:0;}
  .settings-pages :global([data-settings-section]) {margin:0;padding:0;min-width:0;border:0;border-radius:var(--ca-radius-none);background:transparent;box-shadow:none;}
  .settings-pages :global([data-settings-section][hidden]) {display:none;}
  .settings-pages :global([data-settings-section] > .settings-section-copy) {max-width:48rem;margin:0 0 var(--ca-space-6);}
  .settings-pages :global(.settings-section-copy),.settings-pages :global(.setting-value),.settings-pages :global(.workspace-note),.settings-pages :global(.system-detail),.settings-pages :global(.permission-description),.settings-pages :global(.preference-description) {font:400 var(--ca-type-settings-help)/var(--ca-leading-body) var(--ca-font-body);color:var(--ca-muted);}
  .settings-pages :global(.provider-grid) {display:grid;grid-template-columns:minmax(0,1fr);gap:var(--ca-space-4);}
  .settings-pages :global(.provider-card),.settings-pages :global(.workspace-card),.settings-pages :global(.system-card),.settings-pages :global(.ssh-client-card),.settings-pages :global(.remote-desktop-card),.settings-pages :global(.ssh-profile-form),.settings-pages :global(.ssh-profile-card) {min-width:0;padding:var(--ca-space-4);margin:0;border:0;border-radius:var(--ca-radius-medium);background:var(--ca-surface);box-shadow:none;}
  .settings-pages :global(.provider-card-badge),.settings-pages :global(.workspace-badge),.settings-pages :global(.workspace-capability),.settings-pages :global(.ssh-client-badge),.settings-pages :global(.ssh-profile-badge),.settings-pages :global(.ssh-agent-option),.settings-pages :global(.ssh-empty),.settings-pages :global(.system-safety),.settings-pages :global(.system-window),.settings-pages :global(.system-window-preview),.settings-pages :global(.repair-option),.settings-pages :global(.setting-list),.settings-pages :global(.workspace-note) {border:0;}
  .settings-pages :global(.ssh-agent-option),.settings-pages :global(.ssh-empty),.settings-pages :global(.system-safety),.settings-pages :global(.system-window) {background:var(--ca-surface-2);}
  .settings-pages :global(.provider-card-title),.settings-pages :global(.remote-desktop-title),.settings-pages :global(.ssh-form-title) {font-size:var(--ca-type-title);font-weight:670;line-height:var(--ca-leading-compact);}
  .settings-pages :global(.action),.settings-pages :global(.mini-action),.settings-pages :global(.provider-card button),.settings-pages :global(.native-configuration button:not(.section-toggle):not(.settings-picker-button)),.settings-pages :global(.native-access button:not(.settings-picker-button)) {min-height:var(--ca-control-prominent);height:auto;padding:var(--ca-space-2) var(--ca-space-3);border:0;border-radius:var(--ca-radius-medium);box-shadow:none;background:var(--ca-settings-selectable-background);color:var(--ca-text);font:650 var(--ca-type-body)/var(--ca-leading-body) var(--ca-font-body);white-space:normal;cursor:pointer;}
  .settings-pages :global(.action.primary) {background:var(--ca-settings-selected-background);color:var(--ca-settings-selected-text);}
  .settings-pages :global(select),.settings-pages :global(input:not([type="checkbox"]):not([type="radio"]):not([type="hidden"])),.settings-pages :global(textarea),.settings-pages :global(.model-picker-button) {min-height:var(--ca-control-prominent);max-width:100%;font:var(--ca-type-body)/var(--ca-leading-body) var(--ca-font-body);border:0;border-radius:var(--ca-radius-medium);background:var(--ca-settings-selectable-background);color:var(--ca-text);box-shadow:none;}
  .settings-pages :global(select) {appearance:auto;padding:var(--ca-space-2) var(--ca-space-3);cursor:pointer;}
  .settings-pages :global(.preference-select),.settings-pages :global(.ssh-form-input) {padding:var(--ca-space-2) var(--ca-space-3);}
  .settings-pages :global(.preference-label),.settings-pages :global(.ssh-form-label) {font-size:var(--ca-type-settings-control-label);font-weight:600;text-transform:none;letter-spacing:normal;}
  .settings-pages :global(.preference-grid) {display:grid;grid-template-columns:minmax(0,1fr);gap:var(--ca-space-5);margin:0;}
  .settings-pages :global(.configuration-section) {display:grid;min-width:0;gap:0;}
  .settings-pages :global(.configuration-section-title),.settings-pages :global(.settings-card-title) {font-size:var(--ca-type-settings-section);font-weight:650;line-height:var(--ca-leading-compact);letter-spacing:-.012em;}
  .settings-pages :global(.section-description) {font-size:var(--ca-type-settings-help);font-weight:400;}
  .settings-pages :global(.section-heading),.settings-pages :global(.settings-plain-section-head) {display:flex;align-items:baseline;justify-content:space-between;gap:var(--ca-space-6);min-width:0;margin:0;padding:0;border:0;background:transparent;color:var(--ca-text);font:inherit;}
  .settings-pages :global(.section-heading .section-scope) {color:var(--ca-muted);font-size:var(--ca-type-settings-scope);font-weight:400;white-space:nowrap;}
  .settings-pages :global(.configuration-content[data-collapsed="false"]) {gap:var(--ca-space-5);padding:var(--ca-space-4) 0 var(--ca-space-8);}
  .settings-pages :global(.native-access select) {max-width:var(--ca-settings-field-width);width:100%;}
  .settings-pages :global(.native-access p),.settings-pages :global(.settings-note) {font-size:var(--ca-type-settings-help);font-weight:400;line-height:var(--ca-leading-body);}
  .settings-pages :global(.native-access label) {font-size:var(--ca-type-settings-control-label);line-height:var(--ca-leading-body);}
  .settings-pages :global(#settings-agent .settings-option-panel),.settings-pages :global(#settings-access .supervisor-permission-content) {position:relative;box-sizing:border-box;min-width:0;width:100%;padding:var(--ca-space-5) var(--ca-space-6);border:0;border-radius:var(--ca-radius-large);background:var(--ca-settings-option-background);box-shadow:none;}
  .settings-pages :global(#settings-agent .settings-option-panel .model-picker-button),.settings-pages :global(#settings-access .supervisor-permission-content .settings-picker-button) {background:var(--ca-settings-option-control-background);}
  .settings-pages :global(#settings-agent .settings-option-panel .model-picker-button:hover:not(:disabled)),.settings-pages :global(#settings-agent .settings-option-panel .model-picker-button[aria-expanded="true"]),.settings-pages :global(#settings-access .supervisor-permission-content .settings-picker-button:hover:not(:disabled)),.settings-pages :global(#settings-access .supervisor-permission-content .settings-picker-button[aria-expanded="true"]) {background:var(--ca-settings-option-control-hover);}
  .settings-pages :global(.supervisor-permission-settings) {display:grid;gap:0;margin:0;padding:0;border:0;background:transparent;}
  .settings-pages :global(.supervisor-permission-content) {display:grid;width:100%;max-width:none;gap:var(--ca-space-3);margin-top:var(--ca-space-4);}
  .settings-pages :global(.supervisor-permission-content > .settings-section-copy),.settings-pages :global(.supervisor-permission-content > p) {margin:0;}
  .settings-pages :global(.supervisor-permission-content .session-action) {width:auto;justify-self:start;margin-top:var(--ca-space-1);}
  .settings-pages :global(.setting-row) {display:grid;grid-template-columns:minmax(0,1fr);gap:var(--ca-space-2);padding:var(--ca-space-4) 0;border:0;}
  .settings-pages :global(.setting-name) {font-size:var(--ca-type-settings-control-label);font-weight:600;}
  .settings-pages :global(.settings-details) {margin-block:var(--ca-space-5);min-width:0;border:0;}
  .settings-pages :global(.settings-detail-copy) {margin:0;color:var(--ca-muted);font-size:var(--ca-type-settings-help);font-weight:400;line-height:var(--ca-leading-body);}
  .settings-pages :global(.settings-section-stack) {display:grid;gap:var(--ca-space-4);padding-bottom:var(--ca-space-4);}
  .settings-pages :global(#settings-servers > .ssh-client-card),.settings-pages :global(#settings-servers > .remote-desktop-card),.settings-pages :global(#settings-servers > .ssh-profile-form) {margin-block:var(--ca-space-4);}
  .settings-pages :global(.remote-desktop-detail),.settings-pages :global(.ssh-client-detail),.settings-pages :global(.ssh-agent-option),.settings-pages :global(.ssh-note),.settings-pages :global(.section-title) {font-size:var(--ca-type-body);line-height:var(--ca-leading-body);}
  .settings-pages :global(.provider-card-actions),.settings-pages :global(.ssh-form-actions),.settings-pages :global(.workspace-actions) {gap:var(--ca-space-2);flex-wrap:wrap;}

  .settings-pages :global(details:not([data-settings-section])) {min-width:0;margin-block:var(--ca-space-4);padding:0;border:0;background:transparent;}
  .settings-pages :global(details details:not([data-settings-section])) {margin-inline-start:var(--ca-space-4);}
  .settings-pages :global(details:not([data-settings-section]) > summary),.settings-pages :global(.section-toggle) {position:relative;display:grid;box-sizing:border-box;min-width:0;width:100%;min-height:var(--ca-settings-row-height);grid-template-columns:auto minmax(0,1fr) auto auto;align-items:center;gap:var(--ca-space-3);padding:var(--ca-space-3) var(--ca-space-12) var(--ca-space-3) var(--ca-space-5);border:0;border-radius:var(--ca-radius-medium);background:var(--ca-settings-selectable-background);box-shadow:none;color:var(--ca-text);font:650 var(--ca-type-body)/var(--ca-leading-body) var(--ca-font-body);text-align:left;white-space:normal;overflow-wrap:anywhere;list-style:none;cursor:pointer;user-select:none;transition:background-color var(--ca-duration-fast) var(--ca-ease-standard);}
  .settings-pages :global(details:not([data-settings-section]) > summary::-webkit-details-marker) {display:none;}
  .settings-pages :global(details:not([data-settings-section]) > summary::marker) {content:"";}
  .settings-pages :global(details:not([data-settings-section]) > summary::before),.settings-pages :global(.section-toggle .section-chevron) {display:none;}
  .settings-pages :global(.section-toggle .section-icon),.settings-pages :global(.settings-card-icon) {display:grid;width:24px;height:24px;place-items:center;color:var(--ca-muted);}
  .settings-pages :global(.section-toggle .section-icon svg),.settings-pages :global(.settings-card-icon svg) {width:100%;height:100%;}
  .settings-pages :global(.section-toggle .section-copy) {display:grid;min-width:0;gap:var(--ca-space-1);}
  .settings-pages :global(.section-toggle .section-scope) {min-width:0;color:var(--ca-muted);font-weight:500;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;}
  .settings-pages :global(details:not([data-settings-section]) > summary::after),.settings-pages :global(.section-toggle::after) {position:absolute;display:block;top:50%;right:var(--ca-space-6);width:var(--ca-space-2);height:var(--ca-space-2);margin:0;border:0;border-right:2px solid currentColor;border-bottom:2px solid currentColor;background:none;color:var(--ca-text);content:"";pointer-events:none;transform:translateY(-50%) rotate(-45deg);transition:transform var(--ca-duration-fast) var(--ca-ease-standard);}
  .settings-pages :global(details[open]:not([data-settings-section]) > summary),.settings-pages :global(.section-toggle[aria-expanded="true"]) {background:var(--ca-settings-selectable-hover);}
  .settings-pages :global(details[open]:not([data-settings-section]) > summary) {margin-bottom:var(--ca-space-4);}
  .settings-pages :global(details[open]:not([data-settings-section]) > summary::after),.settings-pages :global(.section-toggle[aria-expanded="true"]::after) {transform:translateY(-65%) rotate(45deg);}
  .settings-pages :global(details:not([data-settings-section]) > summary:hover),.settings-pages :global(.section-toggle:hover),.settings-pages :global(select:hover:not(:disabled)),.settings-pages :global(.model-picker-button:hover:not(:disabled)),.settings-pages :global(.action:hover:not(:disabled)),.settings-pages :global(.mini-action:hover:not(:disabled)),.settings-pages :global(.provider-card button:hover:not(:disabled)),.settings-pages :global(.native-configuration button:hover:not(:disabled)),.settings-pages :global(.native-access button:hover:not(:disabled)) {background:var(--ca-settings-selectable-hover);color:var(--ca-text);}
  .settings-pages :global(:is(summary,button,input,select,textarea):focus-visible) {outline:2px solid var(--ca-settings-control-focus);outline-offset:var(--ca-space-1);}
  .settings-pages :global(:is(button,input,select,textarea):disabled) {opacity:.55;cursor:not-allowed;}

  .settings-pages :global(.settings-picker) {position:relative;display:grid;width:100%;min-width:0;max-width:none;gap:var(--ca-space-2);}
  .settings-pages :global(.settings-picker-label) {margin:0;color:var(--ca-text);font:600 var(--ca-type-settings-control-label)/var(--ca-leading-body) var(--ca-font-body);letter-spacing:normal;text-transform:none;}
  .settings-pages :global(.settings-picker-button) {display:grid;width:100%;min-height:calc(var(--ca-control-prominent) + var(--ca-space-2));grid-template-columns:minmax(0,1fr) var(--ca-space-4);align-items:center;gap:var(--ca-space-3);padding:var(--ca-space-2) var(--ca-space-4);border:0;border-radius:var(--ca-pill);background:var(--ca-settings-selectable-background);color:var(--ca-text);box-shadow:none;font-weight:570;text-align:left;cursor:pointer;}
  .settings-pages :global(.settings-picker-button[aria-expanded="true"]) {background:var(--ca-settings-selectable-hover);box-shadow:none;}
  .settings-pages :global(.settings-picker-button .model-picker-value) {overflow:hidden;text-overflow:ellipsis;white-space:nowrap;}
  .settings-pages :global(.settings-picker-menu) {position:absolute;top:calc(100% + var(--ca-space-2));right:0;left:0;z-index:30;display:grid;width:100%;min-width:0;max-height:var(--ca-command-menu-max-height);gap:var(--ca-space-1);margin:0;padding:var(--ca-space-2);overflow:auto;overscroll-behavior:contain;border:0;border-radius:var(--ca-radius-large);background:var(--ca-surface);box-shadow:var(--ca-shadow-float);transform:none;animation:picker-in var(--ca-duration-fast) var(--ca-ease-standard);}
  .settings-pages :global(.settings-picker-menu[hidden]) {display:none;}
  .settings-pages :global(.settings-picker-menu .model-picker-option) {display:grid;width:100%;min-height:var(--ca-control-prominent);grid-template-columns:minmax(0,1fr) var(--ca-space-6);align-items:center;gap:var(--ca-space-3);padding:var(--ca-space-3);border:0;border-radius:var(--ca-radius-medium);background:transparent;color:var(--ca-text);box-shadow:none;text-align:left;cursor:pointer;}
  .settings-pages :global(.settings-picker-menu .model-picker-option:hover:not(:disabled)),.settings-pages :global(.settings-picker-menu .model-picker-option:focus-visible) {background:var(--ca-settings-selectable-hover);color:var(--ca-text);}
  .settings-pages :global(.settings-picker-menu .model-picker-option.is-selected) {background:var(--ca-settings-selected-background);color:var(--ca-settings-selected-text);}
  .settings-pages :global(.settings-picker-menu .model-picker-option-description) {display:block;margin-top:var(--ca-space-1);overflow:visible;color:inherit;font-size:var(--ca-type-caption);line-height:var(--ca-leading-body);white-space:normal;-webkit-line-clamp:unset;line-clamp:unset;}
  .settings-pages :global(.settings-picker-menu .model-picker-check) {justify-self:end;}
  .settings-pages :global(.settings-choice-row) {position:relative;grid-template-columns:minmax(0,1fr) minmax(16rem,var(--ca-settings-field-width));align-items:center;width:100%;max-width:none;column-gap:var(--ca-space-12);row-gap:var(--ca-space-2);padding-block:var(--ca-space-1);}
  .settings-pages :global(.settings-choice-row .settings-picker-copy) {display:grid;grid-column:1;grid-row:1;align-content:center;min-width:0;gap:var(--ca-space-1);}
  .settings-pages :global(.settings-choice-row .settings-picker-label) {margin:0;color:var(--ca-text);}
  .settings-pages :global(.settings-choice-row .settings-picker-description) {min-width:0;color:var(--ca-muted);font:400 var(--ca-type-settings-help)/var(--ca-leading-body) var(--ca-font-body);overflow-wrap:anywhere;}
  .settings-pages :global(.settings-choice-row > .settings-picker-button) {grid-column:2;grid-row:1;min-width:0;}
  .settings-pages :global(.settings-choice-row > .settings-picker-menu) {position:absolute;top:calc(100% + var(--ca-space-1));right:0;left:auto;width:var(--ca-settings-field-width);min-width:min(100%,20rem);z-index:30;}
  .settings-pages :global(#settings-agent[data-settings-presentation="settings-agent"] .codex-tools-copy),.settings-pages :global(#settings-agent[data-settings-presentation="settings-agent"] [data-config-section="tools"]) {display:none;}
  .settings-pages :global(#settings-agent[data-settings-presentation="settings-codex-tools"] .agent-settings-primary) {display:none;}
  .settings-pages :global(#settings-agent[data-settings-presentation="settings-codex-tools"] .codex-tools-copy) {display:block;margin:0 0 var(--ca-space-4);}

  @media (prefers-reduced-motion:reduce) {
    .settings-back,.settings-pages :global(details:not([data-settings-section]) > summary),.settings-pages :global(details:not([data-settings-section]) > summary::after),.settings-pages :global(.section-toggle),.settings-pages :global(.section-toggle::after) {transition:none;}
    .settings-pages :global(.settings-picker-menu) {animation:none;}
  }
  @container settings (max-width:980px) {
    .settings-topbar {gap:var(--ca-space-3);padding:0 var(--ca-space-4);}
    .settings-topbar h1 {grid-column:1;grid-row:1;align-self:center;}
    .settings-back {grid-column:2;grid-row:1;}
    .settings-nav {padding:0 var(--ca-space-4) var(--ca-space-3);overflow:hidden;}
  }
  @container settings (max-width:760px) {
    .settings-page-head {padding:0 var(--ca-space-4) var(--ca-space-3);}
    h2 {font-size:var(--ca-type-display);}
    .settings-content {padding:0 var(--ca-space-4) var(--ca-space-8);scrollbar-gutter:auto;}
    .settings-pages :global(.ssh-form-grid),.settings-pages :global(.remote-desktop-grid) {grid-template-columns:minmax(0,1fr);}
    .settings-pages :global(.ssh-form-field.wide) {grid-column:1/-1;}
    .settings-pages :global(.settings-choice-row) {grid-template-columns:minmax(0,1fr);gap:var(--ca-space-2);}
    .settings-pages :global(.settings-choice-row .settings-picker-copy) {display:grid;grid-column:1;grid-row:auto;gap:var(--ca-space-1);}
    .settings-pages :global(.settings-choice-row > .settings-picker-button) {grid-column:1;grid-row:auto;}
    .settings-pages :global(.settings-choice-row > .settings-picker-menu) {right:auto;left:0;width:100%;min-width:0;}
    .settings-pages :global(.section-heading),.settings-pages :global(.settings-plain-section-head) {align-items:flex-start;}
  }
  @container settings (max-width:520px) {
    .settings-back {width:calc(var(--ca-control-prominent) + var(--ca-space-3));padding:0;justify-content:center;}
    .settings-topbar h1 {font-size:var(--ca-type-settings-sidebar-title-compact);}
  }
</style>
