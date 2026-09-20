<script lang="ts">
  import { supervisorMark } from "../lib/supervisor-brand";

  let { dimensional = false }: { dimensional?: boolean } = $props();
  const depthMark = supervisorMark.replace(' data-supervisor-logo="true"', "");
  const depthLayers = [
    { x: 10, y: 8, weight: 18 },
    { x: 9, y: 7, weight: 22 },
    { x: 8, y: 6, weight: 26 },
    { x: 7, y: 5, weight: 30 },
    { x: 6, y: 4, weight: 34 },
    { x: 5, y: 4, weight: 38 },
    { x: 4, y: 3, weight: 42 },
    { x: 3, y: 2, weight: 46 },
    { x: 2, y: 1, weight: 50 },
    { x: 1, y: 1, weight: 54 },
  ];
</script>

<span class:supervisor-logo--dimensional={dimensional} class="supervisor-logo" aria-hidden="true">
  {#if dimensional}
    <span class="supervisor-logo-cast supervisor-logo-layer">{@html depthMark}</span>
    {#each depthLayers as layer}
      <span
        class="supervisor-logo-depth supervisor-logo-layer"
        style={`--logo-depth-x:${layer.x}px;--logo-depth-y:${layer.y}px;--logo-depth-weight:${layer.weight}%`}
      >{@html depthMark}</span>
    {/each}
  {/if}
  <span class="supervisor-logo-face supervisor-logo-layer">{@html supervisorMark}</span>
</span>

<style>
  .supervisor-logo { position: relative; display: block; width: 100%; height: 100%; color: inherit; pointer-events: none; }
  .supervisor-logo-layer { display: block; width: 100%; height: 100%; }
  .supervisor-logo-face { color: inherit; }
  .supervisor-logo :global(svg) { display: block; width: 100%; height: 100%; overflow: visible; }
  .supervisor-logo--dimensional {
    isolation: isolate;
    transform: perspective(300px) rotateX(7deg) rotateY(-8deg);
    transform-style: preserve-3d;
  }
  .supervisor-logo--dimensional .supervisor-logo-layer { position: absolute; inset: 0; }
  .supervisor-logo--dimensional .supervisor-logo-cast {
    z-index: 0;
    color: var(--ca-brand-depth-shadow);
    filter: blur(5px);
    transform: translate(6px, 13px) scale(.96);
  }
  .supervisor-logo--dimensional .supervisor-logo-depth {
    z-index: 1;
    color: color-mix(in srgb, var(--ca-brand-ink) var(--logo-depth-weight), var(--ca-app-background));
    transform: translate(var(--logo-depth-x), var(--logo-depth-y));
  }
  .supervisor-logo--dimensional .supervisor-logo-face {
    z-index: 2;
    color: var(--ca-brand-ink);
    filter: drop-shadow(-1px -1px 0 var(--ca-brand-depth-highlight));
    transform: translate(-1px, -1px);
  }
</style>
