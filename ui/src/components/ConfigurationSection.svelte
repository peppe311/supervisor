<script lang="ts">
  import { getContext, untrack, type Snippet } from "svelte";

  type SectionIcon = "agents" | "folder" | "tools";
  const iconPaths: Record<SectionIcon, string[]> = {
    agents: ["M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2", "M9 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8", "M22 21v-2a4 4 0 0 0-3-3.87", "M16 3.13a4 4 0 0 1 0 7.75"],
    folder: ["M3 6h6l2 2h10v11H3V6Z", "M3 6V4h6l2 2"],
    tools: ["M4 4h6v6H4V4Z", "M14 4h6v6h-6V4Z", "M4 14h6v6H4v-6Z", "M14 14h6v6h-6v-6Z"],
  };

  let { section, label, description = "", scope = "", icon, initiallyOpen = false, collapsible = true, className = "", children }: {
    section: string;
    label: string;
    description?: string;
    scope?: string;
    icon?: SectionIcon;
    initiallyOpen?: boolean;
    collapsible?: boolean;
    className?: string;
    children: Snippet;
  } = $props();
  const sectionId = $props.id();
  const contentId = `${sectionId}-configuration-content`;
  const compact = getContext<boolean>("central-agent:compact-configuration") === true;
  let expanded = $state(untrack(() => initiallyOpen));
</script>

<section class="configuration-section {className}" class:compact data-config-section={section}>
  {#if !compact}
    {#if collapsible}
      <h2>
        <button type="button" class="section-toggle" aria-expanded={expanded} aria-controls={contentId} onclick={() => expanded = !expanded}>
          {#if icon}
            <span class="section-icon" aria-hidden="true">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.65" stroke-linecap="round" stroke-linejoin="round">
                {#each iconPaths[icon] as path}<path d={path}/>{/each}
              </svg>
            </span>
          {/if}
          <span class="section-copy"><span class="configuration-section-title">{label}</span>{#if description}<span class="section-description">{description}</span>{/if}</span>
          {#if scope}<span class="section-scope">{scope}</span>{/if}
          <span class="section-chevron" aria-hidden="true">›</span>
        </button>
      </h2>
    {:else}
      <h2 class="section-heading">
        <span class="section-copy"><span class="configuration-section-title">{label}</span>{#if description}<span class="section-description">{description}</span>{/if}</span>
        {#if scope}<span class="section-scope">{scope}</span>{/if}
      </h2>
    {/if}
  {/if}
  <div id={contentId} class="configuration-content" data-collapsed={compact || (collapsible && !expanded)}>
    {@render children()}
  </div>
</section>

<style>
  .configuration-section { min-width: 0; margin: 0; padding: 0; border: 0; background: transparent; box-shadow: none; color: var(--ca-text); font: var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body); }
  h2 { margin: 0; font: inherit; }
  .section-heading {display:flex;align-items:baseline;justify-content:space-between;gap:var(--ca-space-6);padding:0;color:var(--ca-text);}
  .section-toggle { display: grid; grid-template-columns:auto minmax(0,1fr) auto auto; align-items:center; gap: var(--ca-space-3); width: 100%; padding: var(--ca-space-3) var(--ca-space-4); border: 0; border-radius: var(--ca-radius-medium); background: var(--ca-settings-control-background); color: inherit; font: inherit; text-align: left; cursor: pointer; }
  .section-toggle:hover { background: var(--ca-surface-2); }
  .section-toggle:focus-visible { box-shadow: var(--ca-focus-ring); outline: 0; }
  .section-copy { display: grid; min-width: 0; gap: var(--ca-space-1); overflow-wrap: anywhere; }
  .configuration-section-title { font-weight: 600; }
  .section-heading .configuration-section-title {font-size:var(--ca-type-settings-section);font-weight:650;letter-spacing:-.012em;}
  .section-description { color: var(--ca-muted); font-size: var(--ca-type-settings-help); font-weight: 400; }
  .section-icon {display:grid;width:var(--ca-space-6);height:var(--ca-space-6);place-items:center;color:var(--ca-muted);}
  .section-icon svg {width:100%;height:100%;}
  .section-scope {min-width:0;color:var(--ca-muted);font-size:var(--ca-type-settings-scope);font-weight:400;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;}
  .section-chevron { flex: 0 0 auto; width: var(--ca-space-4); text-align: center; font-weight: 650; }
  [aria-expanded="true"] .section-chevron { transform: rotate(90deg); }
  .configuration-content { display: grid; min-width: 0; gap: var(--ca-space-2); }
  /* Compact agent cards retain owned dialogs and selected context without
     rendering the advanced configuration menus or reserving space for them. */
  .compact, .compact .configuration-content { display: contents; }
  .configuration-content[data-collapsed="false"] { padding: var(--ca-space-1) var(--ca-space-2) var(--ca-space-3); }
  .section-heading + .configuration-content[data-collapsed="false"] {padding:var(--ca-space-4) 0 var(--ca-space-6);}
  /* Keep direct native dialogs mounted and renderable for slash shortcuts.
     Hiding a modal ancestor in WebView2 can leave an invisible, inert window. */
  .configuration-content[data-collapsed="true"] > :global(:not(dialog):not([data-config-persistent])) { display: none; }
  .configuration-content > :global(button) { width: 100%; text-align: left; }
  @media (max-width:520px) {
    .section-toggle {grid-template-columns:auto minmax(0,1fr) auto;}
    .section-scope {display:none;}
    .section-heading {align-items:flex-start;}
  }
</style>
