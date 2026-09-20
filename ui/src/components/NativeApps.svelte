<script lang="ts">
  import {appAvailability,appMatches as appMatchesNative,type AppsAction,type NativeApp,type NativeAppsView} from '../lib/native-apps';
  import PluginIcon from './PluginIcon.svelte';
  let {view,connected,onAction}:{view?:NativeAppsView|null;connected:boolean;onAction:(action:AppsAction)=>void}=$props();
  let filter=$state('');
  const blocked=$derived(!connected || !!view?.busy);
  const normalizedFilter=$derived(filter.trim().toLocaleLowerCase());
  function appMatches(app:NativeApp):boolean {
    return appMatchesNative(app,normalizedFilter);
  }
  const visibleApps=$derived((view?.items||[]).filter(appMatches));
</script>

<details class="native-apps">
  <summary>Plugins</summary>
  <p>Inspect the user-facing plugins installed in Codex. Select them from the message composer; Codex still owns installation, authentication, policy and execution.</p>
  <button type="button" disabled={blocked} onclick={()=>onAction({kind:'refresh'})}>{view?.busy?'Loading…':'Refresh Plugins'}</button>
  {#if view?.error}<p role="alert">{view.error}</p>{/if}
  {#if view?.unavailableReason}<p class="unavailable" role="status">{view.unavailableReason}</p>{/if}
  {#if view?.notice}<p role="status">{view.notice}</p>{/if}
  {#if view?.current && view.items.length}
    <label class="filter">Filter plugins<input type="search" bind:value={filter} placeholder="Name, description or availability" /></label>
    {#if normalizedFilter}<p>{visibleApps.length} of {view.items.length} plugins match.</p>{/if}
  {/if}
  {#if !view?.current}
    {#if !view?.error && !view?.unavailableReason && !view?.notice}
      <p>{view?.inventoryId?'The last plugin inventory is stale. Refresh before selecting a plugin.':'Refresh to inspect plugins available for the next prompt.'}</p>
    {/if}
  {:else if !view.items.length}
    <p>No user-facing plugins are installed for this conversation.</p>
  {:else if !visibleApps.length}
    <p>No plugin matches this filter.</p>
  {:else}
    <ul>
      {#each visibleApps as app (app.id)}
        <li>
          <div class="plugin-heading"><PluginIcon appId={app.id} name={app.name} iconUrl={app.iconUrl} /><strong>{app.name}</strong><span>{appAvailability(app)}</span></div>
          {#if app.description}<p>{app.description}</p>{/if}
        </li>
      {/each}
    </ul>
  {/if}
</details>

<style>
  .native-apps{margin-block:var(--ca-space-3);font:var(--ca-type-body)/var(--ca-leading-body) var(--ca-font-body);color:var(--ca-text);overflow-wrap:anywhere}
  summary{cursor:pointer;font-size:var(--ca-type-label)}
  summary:focus-visible,button:focus-visible,input:focus-visible{box-shadow:var(--ca-focus-ring)}
  p,span{color:var(--ca-muted);margin-block:var(--ca-space-2)}
  p[role="alert"],p.unavailable,strong{color:var(--ca-text)}
  button{border:0;border-radius:var(--ca-control-radius);padding:var(--ca-space-2);background:var(--ca-app-background);color:var(--ca-text);font:inherit}
  button:disabled{color:var(--ca-muted)}
  label{display:grid;gap:var(--ca-space-1)}
  input{width:100%;box-sizing:border-box;border:0;border-radius:var(--ca-control-radius);padding:var(--ca-space-2);background:var(--ca-app-background);color:var(--ca-text);font:inherit}
  .filter{margin-block:var(--ca-space-3)}
  ul{padding-inline-start:var(--ca-space-6)}
  li{margin-block:var(--ca-space-3)}
  li>div{display:flex;flex-wrap:wrap;gap:var(--ca-space-2)}
  .plugin-heading{align-items:center}
</style>
