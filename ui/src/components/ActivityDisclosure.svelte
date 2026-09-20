<script lang="ts">
  let { title, meta }: { title:string; meta:string } = $props();
</script>

<summary class="activity-disclosure-toggle">
  <svg class="activity-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <g class="command-icon"><rect x="3" y="3" width="18" height="18" rx="4"/><path d="m7 8 3 3-3 3m6 1h4"/></g>
    <g class="web-icon"><circle cx="12" cy="12" r="9"/><ellipse cx="12" cy="12" rx="4" ry="9"/><path d="M3 12h18"/></g>
    <g class="other-icon"><path d="M7 6h10M7 12h10M7 18h6"/><path d="M3 6h.1M3 12h.1M3 18h.1"/></g>
  </svg>
  <span class="activity-group-title">{title}</span>
  <span class="activity-group-meta">{meta}</span>
  <span class="activity-group-chevron" aria-hidden="true"><span class="closed">&gt;</span><span class="opened">▾</span></span>
</summary>

<style>
  :global(.activity-group[data-activity-disclosure]) > summary {
    display:flex; align-items:center; gap:var(--ca-space-2); min-width:0;
    min-height:var(--ca-control-compact); padding:var(--ca-space-1) 0;
    border:0; background:transparent; color:var(--ca-muted);
    font:var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body);
    cursor:pointer; list-style:none;
  }
  summary::-webkit-details-marker { display:none; }
  summary:hover { color:var(--ca-text); }
  summary:focus-visible { outline:0; box-shadow:var(--ca-focus-ring); border-radius:var(--ca-radius-small); }
  .activity-icon { width:var(--ca-space-4); height:var(--ca-space-4); flex:none; }
  .command-icon,.web-icon { display:none; }
  :global(.activity-group[data-activity-icon="command"]) .command-icon,
  :global(.activity-group[data-activity-icon="web"]) .web-icon { display:inline; }
  :global(.activity-group[data-activity-icon="command"]) .other-icon,
  :global(.activity-group[data-activity-icon="web"]) .other-icon { display:none; }
  :global(.activity-group[data-activity-disclosure]) > summary .activity-group-title { flex:0 1 auto; min-width:0; overflow-wrap:anywhere; font:inherit; }
  .activity-group-meta { display:none; color:var(--ca-muted); font:var(--ca-type-caption)/var(--ca-leading-body) var(--ca-font-body); }
  :global(.activity-group[data-live="true"]) .activity-group-meta,
  :global(.activity-group.has-errors) .activity-group-meta { display:inline; }
  :global(.activity-group[data-activity-disclosure][open]) > summary .activity-group-chevron,
  :global(.activity-group[data-activity-disclosure]:not([open])) > summary .activity-group-chevron {
    flex:none; inline-size:var(--ca-space-3); transform:none; transition:none;
    font:var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body); text-align:center;
  }
  .opened { display:none; }
  :global(.activity-group[open]) .opened { display:inline; }
  :global(.activity-group[open]) .closed { display:none; }
</style>
