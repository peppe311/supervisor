<script lang="ts">
  import { onMount } from "svelte";
  import type { NativeWorkHistory } from "../lib/agent-timeline";
  let { group, title, history }: { group:HTMLDetailsElement; title:string; history?:NativeWorkHistory|null } = $props();
  function request():void {
    if (!history || history.state === "loaded" || history.state === "loading" || history.state === "live") return;
    group.dispatchEvent(new CustomEvent("central-agent:app-server-conversation-control", {bubbles:true,detail:{
      owner:history.owner, action:{kind:"read_work",expected_thread_id:history.threadId,turn_id:history.turnId},
    }}));
  }
  onMount(() => {
    const toggle = () => { if (group.open && history?.state === "unloaded") request(); };
    group.addEventListener("toggle", toggle);
    return () => group.removeEventListener("toggle", toggle);
  });
</script>

<summary class="work-history-toggle">
  <span class="agent-work-group-title">{title}</span>
  <span class="work-history-chevron" aria-hidden="true"><span class="closed">&gt;</span><span class="opened">▾</span></span>
</summary>
{#if history?.state === "loading" || history?.state === "unloaded"}
  <p class="work-history-status" role="status">Caricamento delle attività…</p>
{:else if history?.state === "error"}
  <div class="work-history-status" role="status"><p>{history.error || "Impossibile caricare le attività."}</p><button type="button" onclick={request}>Riprova</button></div>
{:else if history?.state === "loaded"}
  <p class="work-history-empty">Non sono state registrate attività intermedie per questo turno.</p>
{/if}

<style>
  :global(.agent-work-group[data-work-disclosure]) { width:100%; min-width:0; border:0; background:transparent; }
  :global(.agent-work-group[data-work-disclosure] > .agent-work-group-body) { display:grid; grid-template-columns:minmax(0,1fr); width:100%; max-width:100%; min-width:0; gap:var(--ca-space-4); }
  :global(.agent-work-group[data-work-disclosure][data-live="true"]) { padding:0; margin:0; }
  :global(.agent-work-group[data-work-disclosure] > .agent-work-group-body > .work-progress) { padding:var(--ca-space-1) 0; background:transparent; border:0; }
  :global(.agent-work-group[data-work-disclosure] > .agent-work-group-body > .work-progress .chat-role) { display:none; }
  :global(.chat-message.activity.work-event), :global(.chat-message.activity.work-event:hover) { padding:var(--ca-space-1) 0; background:transparent; border:0; color:var(--ca-muted); }
  :global(.chat-message.activity.work-event .chat-role), :global(.chat-message.activity.work-event .activity-detail) { display:none; }
  :global(.chat-message.activity.work-event > .chat-text) { color:var(--ca-muted); font:var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body); }
  :global(.agent-work-group[data-live="true"]) .work-history-toggle { display:none; }
  .work-history-toggle { display:flex; align-items:center; gap:var(--ca-space-2); min-width:0; min-height:var(--ca-control-compact); padding:var(--ca-space-1) var(--ca-space-2); border:0; background:transparent; color:var(--ca-muted); font:var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body); cursor:pointer; list-style:none; user-select:none; }
  .work-history-toggle::-webkit-details-marker { display:none; }
  .work-history-toggle:hover { color:var(--ca-text); }
  .work-history-toggle:focus-visible, button:focus-visible { outline:0; box-shadow:var(--ca-focus-ring); border-radius:var(--ca-radius-small); }
  :global(.agent-work-group[data-work-disclosure]) .agent-work-group-title { flex:0 1 auto; min-width:0; overflow-wrap:anywhere; color:inherit; font:inherit; }
  .work-history-chevron { flex:none; inline-size:var(--ca-space-3); color:inherit; font:var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body); text-align:center; }
  .opened { display:none; }
  :global(.agent-work-group[open]) .opened { display:inline; }
  :global(.agent-work-group[open]) .closed { display:none; }
  .work-history-status, .work-history-empty { margin:var(--ca-space-2) 0; color:var(--ca-muted); overflow-wrap:anywhere; font:var(--ca-type-body)/var(--ca-leading-body) var(--ca-font-body); }
  .work-history-status p { margin:0; }
  .work-history-empty { display:none; }
  :global(.agent-work-group:not(:has(.agent-work-group-body > *))) .work-history-empty { display:block; }
  button { border:0; background:transparent; color:var(--ca-text); padding:var(--ca-space-2); font:inherit; text-decoration:underline; cursor:pointer; }
</style>
