<script lang="ts">
  let {
    label = "Agents collaborating",
    compact = false,
  }: { label?: string; compact?: boolean } = $props();
</script>

<span
  class="collaboration-indicator"
  class:compact
  data-collaboration-active
  role="img"
  aria-label={label}
  title={label}
>
  <svg viewBox="0 0 18 18" aria-hidden="true">
    <path d="M7.25 4.25H5.5L3.25 6.5v5l2.25 2.25h1.75" />
    <path d="M10.75 4.25h1.75l2.25 2.25v5l-2.25 2.25h-1.75" />
    <path class="collaboration-bridge" d="M6.25 9h5.5" />
    <path class="collaboration-core" d="m9 6.9 2.1 2.1L9 11.1 6.9 9Z" />
  </svg>
</span>

<style>
  .collaboration-indicator {
    display: inline-grid;
    width: 18px;
    height: 18px;
    flex: 0 0 auto;
    place-items: center;
    color: var(--ca-text);
  }
  .collaboration-indicator.compact { width: 16px; height: 16px; }
  svg {
    display: block;
    width: 100%;
    height: 100%;
    overflow: visible;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.45;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .collaboration-bridge { opacity: .7; }
  .collaboration-core {
    fill: currentColor;
    stroke: none;
    transform-box: fill-box;
    transform-origin: center;
    animation: collaboration-pulse 1.6s var(--ca-ease-standard) infinite;
  }
  @keyframes collaboration-pulse {
    0%, 100% { opacity: .48; transform: scale(.78); }
    50% { opacity: 1; transform: scale(1); }
  }
  @media (prefers-reduced-motion: reduce) {
    .collaboration-core { animation: none; opacity: 1; transform: none; }
  }
</style>
