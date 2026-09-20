<script lang="ts">
  import {settingsCategories,settingsGroups,type SettingsId} from "../lib/settings-catalog";

  let {active,onSelect}:{active:SettingsId|null;onSelect:(id:SettingsId)=>void}=$props();
  let secondaryOpen=$state(true);
  const categories=new Map(settingsCategories.map(category=>[category.id,category]));
  const activeGroup=$derived(settingsGroups.find(group=>active && group.categories.includes(active)) || settingsGroups[0]);
  const groupIconPaths:Record<string,string[]>={
    personal:["M20 21a8 8 0 0 0-16 0","M12 13a5 5 0 1 0 0-10 5 5 0 0 0 0 10"],
    ai:["M9 3h6","M12 3v3","M6 8h12a3 3 0 0 1 3 3v7a3 3 0 0 1-3 3H6a3 3 0 0 1-3-3v-7a3 3 0 0 1 3-3Z","M8 13h.01","M16 13h.01","M8 17h8"],
    workspace:["M3 7h7l2 2h9v10H3V7Z","M3 7V5h7l2 2"],
    connections:["M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71","M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"],
  };
  const categoryIconPaths:Record<SettingsId,string[]>={
    "settings-general":["M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8Z","M12 2v3","M12 19v3","M4.93 4.93l2.12 2.12","m16.95 16.95-2.12-2.12","M2 12h3","M19 12h3","m4.93 19.07 2.12-2.12","m16.95 7.05 2.12-2.12"],
    "settings-ai":["M20 21a8 8 0 0 0-16 0","M12 13a5 5 0 1 0 0-10 5 5 0 0 0 0 10"],
    "settings-agent":["M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2","M9 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8","M22 21v-2a4 4 0 0 0-3-3.87","M16 3.13a4 4 0 0 1 0 7.75"],
    "settings-codex-tools":["m8 9-3 3 3 3","m16 9 3 3-3 3","m14 5-4 14"],
    "settings-workspace":["M3 7h7l2 2h9v10H3V7Z","M3 7V5h7l2 2"],
    "settings-servers":["M4 4h16v6H4V4Z","M4 14h16v6H4v-6Z","M7 7h.01","M7 17h.01","M11 7h6","M11 17h6"],
    "settings-system":["M3 4h18v14H3V4Z","M3 9h18","M8 21h8","M12 18v3"],
  };

  function selectGroup(group:typeof settingsGroups[number]):void {
    secondaryOpen=true;
    const next=active && group.categories.includes(active)?active:group.categories[0];
    onSelect(next);
  }
</script>

<div class="settings-nav-stack">
  <div class="settings-primary-tabs" role="tablist" aria-label="Settings groups">
    {#each settingsGroups as group}
      <button
        class="settings-primary-tab"
        class:active={activeGroup.id===group.id}
        type="button"
        role="tab"
        aria-selected={activeGroup.id===group.id}
        aria-current={active===group.categories[0]?'page':undefined}
        data-settings-nav-group={group.id}
        data-settings-target={group.categories.length===1?group.categories[0]:undefined}
        onclick={()=>selectGroup(group)}
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          {#each groupIconPaths[group.id] as path}<path d={path}/>{/each}
        </svg>
        <span>{group.title}</span>
      </button>
    {/each}
  </div>

  {#each settingsGroups as group}
    {#if group.categories.length>1}
      <div class="settings-secondary-tabs" hidden={activeGroup.id!==group.id || !secondaryOpen} aria-label={`${group.title} settings`}>
        {#each group.categories as id}
          {@const category=categories.get(id)!}
          <button
            class="settings-secondary-tab"
            class:active={active===category.id}
            aria-current={active===category.id?'page':undefined}
            type="button"
            data-settings-target={category.id}
            onclick={()=>onSelect(category.id)}
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.65" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              {#each categoryIconPaths[category.id] as path}<path d={path}/>{/each}
            </svg>
            <span>{category.title}</span>
          </button>
        {/each}
        <button class="settings-secondary-collapse" type="button" aria-label={`Collapse ${group.title} categories`} onclick={()=>secondaryOpen=false}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m18 15-6-6-6 6"/></svg>
        </button>
      </div>
    {/if}
  {/each}
</div>

<style>
  .settings-nav-stack {display:grid;justify-items:center;gap:var(--ca-space-3);min-width:0;}
  .settings-primary-tabs,.settings-secondary-tabs {box-sizing:border-box;display:flex;align-items:center;min-width:0;border:0;background:var(--ca-settings-nav-background);box-shadow:none;}
  .settings-primary-tabs {width:min(100%,var(--ca-settings-navigation-width));gap:var(--ca-space-2);padding:var(--ca-space-3);border-radius:var(--ca-pill);}
  .settings-primary-tab,.settings-secondary-tab,.settings-secondary-collapse {display:flex;align-items:center;justify-content:center;gap:var(--ca-space-3);min-width:0;border:0;background:transparent;color:var(--ca-muted);font:560 var(--ca-type-body)/var(--ca-leading-compact) var(--ca-font-navigation);cursor:pointer;transition:background-color var(--ca-duration-standard) var(--ca-ease-standard),color var(--ca-duration-standard) var(--ca-ease-standard),box-shadow var(--ca-duration-standard) var(--ca-ease-standard);}
  .settings-primary-tab {flex:1 1 0;min-height:calc(var(--ca-control-prominent) + var(--ca-space-1));padding:var(--ca-space-2) var(--ca-space-4);border-radius:var(--ca-pill);}
  .settings-primary-tab svg {width:22px;height:22px;flex:0 0 auto;}
  .settings-primary-tab:hover,.settings-secondary-tab:hover,.settings-secondary-collapse:hover {background:var(--ca-settings-selectable-hover);color:var(--ca-text);}
  .settings-primary-tab.active {background:var(--ca-settings-selected-background);box-shadow:none;color:var(--ca-settings-selected-text);font-weight:680;}
  .settings-primary-tab:focus-visible,.settings-secondary-tab:focus-visible,.settings-secondary-collapse:focus-visible {outline:0;box-shadow:var(--ca-focus-ring);}
  .settings-secondary-tabs {width:max-content;max-width:100%;gap:var(--ca-space-1);padding:var(--ca-space-2);border-radius:var(--ca-pill);}
  .settings-secondary-tabs[hidden] {display:none;}
  .settings-secondary-tab {min-height:var(--ca-control-prominent);padding:var(--ca-space-2) var(--ca-space-4);border-radius:var(--ca-pill);white-space:nowrap;}
  .settings-secondary-tab svg {width:20px;height:20px;flex:0 0 auto;}
  .settings-secondary-tab.active {background:var(--ca-settings-subnav-selected-background);color:var(--ca-settings-subnav-selected-text);font-weight:670;}
  .settings-secondary-collapse {width:var(--ca-control-prominent);height:var(--ca-control-prominent);padding:0;border-radius:var(--ca-pill);}
  .settings-secondary-collapse svg {width:18px;height:18px;}
  @media (prefers-reduced-motion:reduce) {.settings-primary-tab,.settings-secondary-tab,.settings-secondary-collapse {transition:none;}}
  @container settings (max-width:760px) {
    .settings-nav-stack {justify-items:start;width:100%;overflow-x:auto;scrollbar-width:none;}
    .settings-nav-stack::-webkit-scrollbar {display:none;}
    .settings-primary-tabs {width:max-content;min-width:100%;}
    .settings-primary-tab {min-width:max-content;}
    .settings-secondary-tabs {margin-inline:auto;}
  }
</style>
