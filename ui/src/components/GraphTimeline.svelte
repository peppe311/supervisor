<script lang="ts">
  import { onMount, mount, unmount, flushSync } from "svelte";
  import GraphMessage from "./GraphMessage.svelte";
  import type { AgentTimelineMessage } from "../lib/agent-timeline";
  import { createWorkTimeline as createAgentTimelineController } from "../lib/work-disclosure";
  import { graphTimelineMessages, type GraphHistory, type GraphRenderHooks } from "../lib/graph-history";
  let { eventTarget, owner }: { eventTarget:HTMLElement; owner:string } = $props();
  let log:HTMLDivElement;
  let older = $state(0);
  onMount(() => {
    let hooks: GraphRenderHooks;
    const rows = new Map<HTMLElement, {update:(message:AgentTimelineMessage)=>void}>();
    const attrs = (row:HTMLElement, message:AgentTimelineMessage) => {
      row.className = `chat-message ${message.kind || "message"} ${message.role || "assistant"}`;
      row.dataset.messageId = String(message.id);
      row.classList.toggle("streaming", Boolean(message.streaming));
      row.dataset.activityCategory = message.activityCategory || "tool";
      row.dataset.activityStatus = message.activityStatus || "running";
      row.dataset.streaming = String(Boolean(message.streaming));
    };
    const timeline = createAgentTimelineController(log, {
      createMessageRow(message) {
        const row = document.createElement("article"); attrs(row, message);
        flushSync(() => rows.set(row, mount(GraphMessage, {target:row,props:{initial:message,hooks,owner}})));
        return row;
      },
      patchMessageRow(row, message) { attrs(row, message); flushSync(() => rows.get(row)?.update(message)); },
    }, {keyboardTarget:eventTarget,promptMotionScope:owner});
    const prune = () => { for (const [row, component] of rows) if (!log.contains(row)) { void unmount(component); rows.delete(row); } };
    const render = (event:Event) => {
      const detail = (event as CustomEvent).detail;
      if (detail?.owner !== owner) return;
      hooks = detail.hooks;
      const run = (detail.run || {}) as GraphHistory;
      const before = log.scrollHeight, top = log.scrollTop;
      const loadingOlder = older > Number(run.historyOffset || 0);
      older = Number(run.historyOffset) || 0;
      if (loadingOlder) timeline.pauseFollowing();
      timeline.render(graphTimelineMessages(run), run.agent);
      prune();
      if (loadingOlder) log.scrollTop = top + log.scrollHeight - before;
    };
    const stream = (event:Event) => {
      const detail = (event as CustomEvent).detail;
      if (detail?.owner === owner && timeline.update(detail.message)) event.preventDefault();
    };
    eventTarget.addEventListener("central-agent:graph-timeline", render);
    eventTarget.addEventListener("central-agent:graph-stream", stream);
    return () => { eventTarget.removeEventListener("central-agent:graph-timeline", render); eventTarget.removeEventListener("central-agent:graph-stream", stream); timeline.destroy(); for (const component of rows.values()) void unmount(component); rows.clear(); };
  });
</script>
{#if older > 0}<button class="earlier" type="button" onclick={() => eventTarget.dispatchEvent(new CustomEvent("central-agent:graph-history-earlier", {detail:{owner,before:older}}))}>Load earlier sessions · {older} remaining</button>{/if}
<div bind:this={log} class="agent-console-log graph-timeline" role="log" aria-live="off" aria-label="Agent conversation"></div>

<style>
  .earlier { flex:0 0 auto; background:var(--agent-card-background,var(--ca-app-background)); color:var(--ca-muted); border:0; padding:var(--ca-space-2); font:var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body); cursor:pointer; }
  .earlier:focus-visible { outline:1px solid var(--ca-border-strong); }
  .graph-timeline { --conversation-scrollbar-track:var(--agent-card-background,var(--ca-app-background));--conversation-scrollbar-thumb:var(--ca-input);display:flex; flex-direction:column; gap:var(--ca-space-3); min-height:0; min-width:0; overflow:auto; overscroll-behavior:contain; scrollbar-color:var(--conversation-scrollbar-thumb) var(--conversation-scrollbar-track); }
  .graph-timeline::-webkit-scrollbar { background:var(--conversation-scrollbar-track); }
  .graph-timeline::-webkit-scrollbar-track,.graph-timeline::-webkit-scrollbar-corner { background:var(--conversation-scrollbar-track); }
  .graph-timeline::-webkit-scrollbar-thumb,.graph-timeline::-webkit-scrollbar-thumb:hover { border-color:var(--conversation-scrollbar-track);background:var(--conversation-scrollbar-thumb); }
  .graph-timeline :global(.chat-message), .graph-timeline :global(.agent-work-group), .graph-timeline :global(.activity-group) { flex:0 0 auto; width:100%; min-width:0; border:0; padding:var(--ca-space-2) 0; margin:0; background:var(--agent-card-background,var(--ca-app-background)); color:var(--ca-text); }
  .graph-timeline :global(.chat-message.reasoning) { color:var(--ca-muted); }
  .graph-timeline :global(.chat-message.user.message) {
    align-self:flex-end; justify-self:end;
    width:fit-content; max-width:var(--ca-chat-user-max-width);
    margin:var(--ca-space-2) 0 var(--ca-space-4) auto;
    padding:var(--ca-space-3) var(--ca-space-4); border:0;
    border-radius:var(--ca-chat-user-radius);
    background:var(--ca-chat-user-background); color:var(--ca-text);
    font-weight:400; text-align:start;
  }
  .graph-timeline :global(.agent-work-group > summary), .graph-timeline :global(.activity-group > summary) { display:flex; align-items:center; gap:var(--ca-space-2); cursor:pointer; list-style:none; font:var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body); color:var(--ca-muted); }
  .graph-timeline :global(summary::-webkit-details-marker) { display:none; }
  .graph-timeline :global(.agent-work-group[data-live="true"] > summary) { display:none; }
  .graph-timeline :global(.agent-work-group[open] > summary .agent-work-group-chevron), .graph-timeline :global(.activity-group[open] > summary .activity-group-chevron) { transform:rotate(90deg); }
  .graph-timeline :global(summary:focus-visible) { outline:1px solid var(--ca-border-strong); }
  .graph-timeline :global(.activity-group-title), .graph-timeline :global(.agent-work-group-title) { flex:1; }
  .graph-timeline :global(.activity-group-meta) { font-size:var(--ca-type-caption); }
  .graph-timeline :global(.activity-group-body), .graph-timeline :global(.agent-work-group-body) { min-width:0; padding:var(--ca-space-2) 0; }
</style>
