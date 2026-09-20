<script lang="ts">
  import { untrack } from "svelte";
  import type { AgentTimelineMessage } from "../lib/agent-timeline";
  let { initial }: { initial: AgentTimelineMessage } = $props();
  let status = $state(untrack(() => initial.submissionStatus));
  export function update(message: AgentTimelineMessage): void { status = message.submissionStatus; }
</script>

{#if status === "sending"}<span class="input-delivery-status" role="status">Sending…</span>{/if}

<style>
  .input-delivery-status {
    display:block;
    margin-top:var(--ca-space-1);
    color:var(--ca-muted);
    font:var(--ca-type-caption)/var(--ca-leading-body) var(--ca-font-body);
    text-align:right;
  }
</style>
