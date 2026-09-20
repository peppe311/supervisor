<script lang="ts">
  import { onMount } from "svelte";
  import { liveDiff, liveDiffLabel, type LiveDiff } from "../lib/live-diff";

  let {
    eventTarget,
    owner = "",
  }: {
    eventTarget?: EventTarget;
    owner?: string;
  } = $props();

  let stats = $state<LiveDiff | null>(null);
  let currentOwner = $state("");

  onMount(() => {
    currentOwner = owner;
    const target = eventTarget ?? window;
    const selectOwner = (event: Event) => {
      if (owner) return;
      currentOwner = String((event as CustomEvent).detail || "");
      stats = null;
    };
    const update = (event: Event) => {
      const detail = (event as CustomEvent).detail;
      if (!detail || typeof detail !== "object") return;
      const eventOwner = typeof detail.owner === "string" ? detail.owner : "";
      const expectedOwner = owner || currentOwner;
      if (expectedOwner && eventOwner !== expectedOwner) return;
      stats = liveDiff(detail.liveDiff);
    };
    if (!owner) window.addEventListener("central-agent:conversation-key", selectOwner);
    target.addEventListener("central-agent:conversation-state", update);
    target.addEventListener("central-agent:app-server-conversation", update);
    return () => {
      if (!owner) window.removeEventListener("central-agent:conversation-key", selectOwner);
      target.removeEventListener("central-agent:conversation-state", update);
      target.removeEventListener("central-agent:app-server-conversation", update);
    };
  });
</script>

{#if stats}
  <span
    class="live-diff"
    class:active={stats.active}
    role="status"
    aria-live="polite"
    aria-atomic="true"
    aria-label={liveDiffLabel(stats)}
    title={liveDiffLabel(stats)}
  >
    <span aria-hidden="true">(</span><span class="added">+{stats.additions}</span><span aria-hidden="true">, </span><span class="removed">−{stats.deletions}</span><span aria-hidden="true">)</span>
  </span>
{/if}

<style>
  .live-diff {
    display: inline-flex;
    flex: 0 0 auto;
    align-items: center;
    min-height: var(--ca-control-compact);
    padding-inline: var(--ca-space-2);
    border: 0;
    border-radius: var(--ca-radius-pill);
    background: var(--ca-surface-2);
    color: var(--ca-muted);
    font: 600 var(--ca-type-caption)/var(--ca-leading-compact) var(--ca-font-mono);
    font-variant-numeric: tabular-nums;
    letter-spacing: -.01em;
    white-space: nowrap;
  }
  .live-diff.active { color: var(--ca-text); }
  .added { color: var(--ca-diff-addition); }
  .removed { color: var(--ca-diff-deletion); }
</style>
