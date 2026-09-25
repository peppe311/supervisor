<script lang="ts">
  import {onMount, tick, type Snippet} from 'svelte';

  let {owner, label, active = true, compact = false, children}: {
    owner: string;
    label: string;
    active?: boolean;
    compact?: boolean;
    children: Snippet;
  } = $props();

  const id = $props.id();
  let button: HTMLButtonElement;
  let panel: HTMLDivElement;
  let open = $state(false);

  $effect(() => { if (!active) close(); });

  function place(): void {
    if (!open || !button || !panel) return;
    const anchor = button.getBoundingClientRect();
    const style = getComputedStyle(panel);
    const inset = parseFloat(style.getPropertyValue('--ca-space-3'));
    const gap = parseFloat(style.getPropertyValue('--ca-space-2'));
    const width = panel.offsetWidth, height = panel.offsetHeight;
    const below = innerHeight - anchor.bottom - gap - inset;
    const above = anchor.top - gap - inset;
    const top = below >= height || below >= above
      ? anchor.bottom + gap : anchor.top - gap - height;
    panel.style.left = `${Math.max(inset, Math.min(anchor.left, innerWidth - width - inset))}px`;
    panel.style.top = `${Math.max(inset, Math.min(top, innerHeight - height - inset))}px`;
  }

  function close(restoreFocus = false): void {
    if (!open) return;
    panel.hidePopover();
    if (restoreFocus) button.focus({preventScroll: true});
  }

  function toggle(): void {
    if (!active) return;
    if (open) { close(); return; }
    panel.showPopover();
    place();
    panel.focus({preventScroll: true});
  }

  function beforeToggle(event: ToggleEvent): void {
    open = event.newState === 'open';
    if (open) void tick().then(place);
  }

  function keydown(event: KeyboardEvent): void {
    if (event.key !== 'Escape') return;
    event.preventDefault();
    event.stopPropagation();
    close(true);
  }

  onMount(() => {
    const outside = (event: Event) => {
      if (open && event.target instanceof Node && !panel.contains(event.target) && !button.contains(event.target)) close();
    };
    const scroll = (event: Event) => {
      if (!open || event.target instanceof Node && panel.contains(event.target)) return;
      const anchor = button.getBoundingClientRect();
      if (anchor.bottom < 0 || anchor.top > innerHeight || anchor.right < 0 || anchor.left > innerWidth) close();
      else place();
    };
    const observer = new ResizeObserver(place);
    observer.observe(panel);
    document.addEventListener('pointerdown', outside);
    document.addEventListener('focusin', outside);
    window.addEventListener('resize', place);
    document.addEventListener('scroll', scroll, true);
    return () => {
      observer.disconnect();
      document.removeEventListener('pointerdown', outside);
      document.removeEventListener('focusin', outside);
      window.removeEventListener('resize', place);
      document.removeEventListener('scroll', scroll, true);
    };
  });
</script>

<div class="conversation-menu" class:compact data-board-conversation-menu={owner}>
  <button bind:this={button} class="menu-trigger" type="button" aria-haspopup="dialog" title={compact?label:undefined}
    aria-label={label} aria-expanded={open} aria-controls={`${id}-panel`} onclick={toggle}
    onkeydown={event => { if (event.key === 'ArrowDown') { event.preventDefault(); if (!open) toggle(); else panel.focus(); } }}>
    <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M3 6h5m4 0h5M3 14h9m4 0h1"/><circle cx="10" cy="6" r="2"/><circle cx="14" cy="14" r="2"/></svg>
    <span id={`${id}-label`}>{label}</span>
    <svg class="chevron" class:expanded={open} viewBox="0 0 20 20" aria-hidden="true"><path d="m6 8 4 4 4-4"/></svg>
  </button>
  <div bind:this={panel} id={`${id}-panel`} class="menu-panel" popover="auto"
    role="dialog" aria-labelledby={`${id}-label`} tabindex="-1"
    onbeforetoggle={beforeToggle} onkeydown={keydown}>
    {@render children()}
  </div>
</div>

<style>
  .conversation-menu{min-width:0;margin-block:var(--ca-space-1)}
  .conversation-menu.compact{width:var(--ca-control-compact);min-width:var(--ca-control-compact);margin:0}
  .menu-trigger{display:flex;align-items:center;gap:var(--ca-space-2);box-sizing:border-box;width:100%;min-width:0;min-height:var(--ca-control-compact);padding:var(--ca-space-1) var(--ca-space-3);border:0;border-radius:var(--ca-control-radius);background:var(--ca-surface);color:var(--ca-muted);font:500 var(--ca-type-caption)/var(--ca-leading-body) var(--ca-font-body);text-align:left;cursor:pointer}
  .compact .menu-trigger{display:grid;place-items:center;width:var(--ca-control-compact);height:var(--ca-control-compact);padding:0;background:transparent}
  .compact .menu-trigger span{position:absolute;width:1px;height:1px;padding:0;margin:-1px;overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0}
  .compact .menu-trigger .chevron{display:none}
  .menu-trigger:hover,.menu-trigger[aria-expanded="true"]{background:var(--ca-surface-2);color:var(--ca-text)}
  .compact .menu-trigger:hover,.compact .menu-trigger[aria-expanded="true"]{background:var(--ca-surface-3)}
  .menu-trigger:focus-visible{outline:none;box-shadow:var(--ca-focus-ring)}
  svg{flex:none;width:var(--ca-space-4);height:var(--ca-space-4);fill:none;stroke:currentColor;stroke-width:1.5;stroke-linecap:round;stroke-linejoin:round}
  .chevron{margin-left:auto;transition:transform var(--ca-duration-fast) var(--ca-ease-standard)}.chevron.expanded{transform:rotate(180deg)}
  .menu-panel{position:fixed;inset:auto;box-sizing:border-box;width:min(var(--ca-board-menu-width),calc(100vw - 2 * var(--ca-space-3)));max-height:calc(100dvh - 2 * var(--ca-space-3));margin:0;padding:var(--ca-space-4);overflow:auto;overscroll-behavior:contain;border:0;border-radius:var(--ca-radius-large);outline:none;background:var(--ca-surface);color:var(--ca-text);box-shadow:var(--ca-shadow-float);font:var(--ca-type-caption)/var(--ca-leading-body) var(--ca-font-body)}
  .menu-panel:popover-open{display:grid;gap:var(--ca-space-4);animation:menu-in var(--ca-duration-fast) var(--ca-ease-standard)}
  @keyframes menu-in{from{opacity:0;transform:translateY(calc(-1 * var(--ca-space-1))) scale(.985)}to{opacity:1;transform:none}}
  @media(prefers-reduced-motion:reduce){.chevron{transition:none}.menu-panel{animation:none}}
</style>
