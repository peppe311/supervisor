<script lang="ts">
  let { onToggle }: { onToggle: () => void } = $props();
  let minimized = $state(false);
  let disabled = $state(false);

  // The native runtime owns visibility; never optimistically hide a WebView.
  export function update(nextMinimized: boolean, nextDisabled: boolean): void {
    minimized = nextMinimized;
    disabled = nextDisabled;
  }

  const label = $derived(minimized ? "Restore web panel" : "Minimize web panel");
</script>

<button
  id="toggle-web-panel"
  type="button"
  aria-label={label}
  aria-expanded={!minimized}
  title={disabled ? "Web panel control is available in the main Agent workspace" : label}
  {disabled}
  onclick={onToggle}>
  <svg viewBox="0 0 24 24" aria-hidden="true">
    <rect x="3" y="4" width="18" height="16" rx="3" />
    <path d="M15 4v16" />
    {#if minimized}
      <path d="m10 9-3 3 3 3" />
    {:else}
      <path d="m8 9 3 3-3 3" />
    {/if}
  </svg>
</button>

<style>
  button {
    display: grid;
    place-items: center;
    width: var(--ca-control-default);
    height: var(--ca-control-default);
    padding: var(--ca-space-1);
    border: none;
    border-radius: var(--ca-radius-small);
    background: transparent;
    color: var(--ca-muted);
    cursor: pointer;
    transition: background-color var(--ca-duration-fast) var(--ca-ease-standard);
  }
  button:hover:not(:disabled) { background: var(--ca-surface-2); color: var(--ca-text); }
  button:focus-visible { outline: 2px solid var(--ca-text); outline-offset: 2px; }
  button:disabled { color: var(--ca-faint); cursor: default; }
  svg { width: 24px; height: 24px; fill: none; stroke: currentColor; stroke-width: 2; stroke-linecap: round; stroke-linejoin: round; }
  @media (prefers-reduced-motion: reduce) { button { transition: none; } }
</style>
