<script lang="ts">
  import { onMount } from "svelte";
  import ModelPicker from "./ModelPicker.svelte";
  import ConfigurationSection from "./ConfigurationSection.svelte";
  import NativeAccess from "./NativeAccess.svelte";
  import NativeConversation from "./NativeConversation.svelte";

  let { host }: { host:HTMLElement } = $props();
  let chatName = $state("");
  let restoreHost = () => {};
  const contextWindows = [
    {value:"",label:"Automatic"}, {value:"32000",label:"32K"},
    {value:"64000",label:"64K"}, {value:"128000",label:"128K"},
    {value:"256000",label:"256K"}, {value:"400000",label:"400K"},
    {value:"1000000",label:"1M"},
  ];

  // Keep one mounted controller tree. A shortcut can open its native dialog
  // from chat while Settings is closed, without hiding a modal ancestor or
  // duplicating subscriptions, drafts or owner-scoped native actions.
  export function prepareDialogs():void {
    if (!host.closest("[hidden]") && !host.dataset.dialogOnly) return;
    host.dataset.dialogOnly = "true";
    if (host.parentElement !== document.body) document.body.append(host);
    queueMicrotask(() => restoreHost());
  }

  onMount(() => {
    const parent = host.parentNode!, next = host.nextSibling;
    const restore = () => {
      if (!host.dataset.dialogOnly || host.querySelector("dialog[open]")) return;
      parent.insertBefore(host, next?.parentNode === parent ? next : null);
      delete host.dataset.dialogOnly;
    };
    restoreHost = restore;
    const observer = new MutationObserver(restore);
    observer.observe(host, {subtree:true, attributes:true, attributeFilter:["open"]});
    const identity = (event:Event) => { chatName = (event as CustomEvent).detail?.chat?.title || ""; };
    window.addEventListener("central-agent:chat-identity", identity);
    return () => {
      observer.disconnect();
      window.removeEventListener("central-agent:chat-identity", identity);
      if (host.dataset.dialogOnly) {
        parent.insertBefore(host, next?.parentNode === parent ? next : null);
        delete host.dataset.dialogOnly;
      }
    };
  });
</script>

<div class="main-agent-settings">
  <div class="agent-settings-primary">
    <ConfigurationSection section="response" label="Agent behavior" scope="Current conversation" icon="agents" initiallyOpen collapsible={false}>
      <div class="response-settings">
        <div class="settings-option-panel">
          <ModelPicker kind="personality" label="Personality" description="Set the tone of the agent's responses." initialValue="No override" />
        </div>
        <div class="compatibility-setting" hidden aria-hidden="true">
          <ModelPicker kind="context" label="Context window" initialValue="Automatic" options={contextWindows} />
        </div>
        <NativeAccess presentation="summary" />
      </div>
    </ConfigurationSection>
    <NativeAccess presentation="workspace" />
  </div>
  <div class="codex-tools-settings">
    <p class="codex-tools-copy settings-section-copy">{chatName ? `Selected conversation: ${chatName}` : "Select a Codex conversation to choose its tools."} Skill and App selections are applied only to the next accepted prompt.</p>
    <NativeConversation />
  </div>
</div>

<style>
  .main-agent-settings { display:grid; min-width:0; gap:var(--ca-space-5); color:var(--ca-text); font:var(--ca-type-body)/var(--ca-leading-body) var(--ca-font-body); }
  .agent-settings-primary,.codex-tools-settings {display:grid;min-width:0;gap:var(--ca-space-5);}
  .codex-tools-copy {display:none;}
  .response-settings { display:grid; gap:var(--ca-space-5); min-width:0; }
  .response-settings :global(.model-field) { position:relative;display:grid;grid-template-columns:minmax(0,1fr) minmax(16rem,var(--ca-settings-field-width));align-items:center;column-gap:var(--ca-space-12);row-gap:var(--ca-space-2);min-width:0; }
  .response-settings :global(.model-field-copy) {display:grid;grid-column:1;grid-row:1;align-content:center;gap:var(--ca-space-1);}
  .response-settings :global(.model-label) { margin:0;font:600 var(--ca-type-settings-control-label)/var(--ca-leading-body) var(--ca-font-body); text-transform:none; letter-spacing:normal; }
  .response-settings :global(.model-description) {font:400 var(--ca-type-settings-help)/var(--ca-leading-body) var(--ca-font-body);}
  .response-settings :global(.model-picker-button) { grid-column:2;grid-row:1;width:100%; min-height:calc(var(--ca-control-prominent) + var(--ca-space-2)); height:auto; padding:var(--ca-space-2) var(--ca-space-4);border-radius:var(--ca-pill); }
  .response-settings :global(.model-picker-value) { white-space:normal; overflow-wrap:anywhere; text-align:left; }
  .response-settings :global(.model-picker-menu) { position:absolute;top:calc(100% + var(--ca-space-2));right:0;left:auto;z-index:20;width:var(--ca-settings-field-width);min-width:min(100%,20rem);max-height:var(--ca-command-menu-max-height); margin:0;border:0;border-radius:var(--ca-radius-large);background:var(--ca-surface);box-shadow:var(--ca-shadow-float);overflow:auto;transform:none; }
  .response-settings :global(.model-picker-option-title), .response-settings :global(.model-picker-option-description) { white-space:normal; overflow-wrap:anywhere; overflow:visible; text-overflow:clip; }
  .compatibility-setting[hidden] {display:none;}
  @media (max-width:760px) {
    .response-settings :global(.model-field) {grid-template-columns:minmax(0,1fr);}
    .response-settings :global(.model-field-copy) {display:grid;gap:var(--ca-space-1);}
    .response-settings :global(.model-field-copy),.response-settings :global(.model-picker-button) {grid-column:1;grid-row:auto;}
    .response-settings :global(.model-picker-menu) {right:auto;left:0;width:100%;min-width:0;}
  }
  :global(#agent-settings-host[data-dialog-only]) { position:fixed; top:0; left:0; width:0; height:0; visibility:hidden; pointer-events:none; }
  :global(#agent-settings-host[data-dialog-only] dialog[open]) { visibility:visible; pointer-events:auto; }
</style>
