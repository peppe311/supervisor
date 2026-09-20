<script lang="ts">
  import type {NativeHooksView} from '../lib/native-hooks';
  let {view,connected,onRefresh}:{view?:NativeHooksView|null;connected:boolean;onRefresh:()=>void}=$props();
</script>

<details class="native-hooks">
  <summary>Codex hooks</summary>
  <p>Read-only handlers and recent lifecycle events reported by App Server for this project. Supervisor does not execute, edit, retry, or duplicate them.</p>
  <button type="button" aria-label="Refresh native Codex hooks" disabled={!connected || !!view?.busy} onclick={onRefresh}>{view?.busy?'Loading…':'Refresh hooks'}</button>
  {#if view?.error}<p role="alert">{view.error}</p>{/if}
  {#if view?.notice}<p role="status">{view.notice}</p>{/if}
  {#if view?.warnings.length}<details><summary>Warnings ({view.warnings.length})</summary><ul>{#each view.warnings as warning}<li>{warning}</li>{/each}</ul></details>{/if}
  {#if view?.errors.length}<details><summary>Configuration errors ({view.errors.length})</summary><ul>{#each view.errors as error}<li>{error}</li>{/each}</ul></details>{/if}
  {#if view?.current}
    {#if view.hooks.length}
      <ul>{#each view.hooks as hook (hook.key)}<li><strong>{hook.eventName} · {hook.handlerType}</strong><p>{hook.enabled?'Enabled':'Disabled'} · {hook.trustStatus} · {hook.managed?'Managed':hook.source}</p>{#if hook.matcher}<p>Matcher: {hook.matcher}</p>{/if}{#if hook.statusMessage}<p>{hook.statusMessage}</p>{/if}<p>{hook.sourcePath}</p></li>{/each}</ul>
    {:else}<p>No hooks were reported for this project.</p>{/if}
  {:else}<p>The hook inventory is stale or has not been loaded.</p>{/if}
  {#if view?.runs.length}
    <details open><summary>Recent native activity ({view.runs.length})</summary><ol>
      {#each [...view.runs].reverse() as run (run.id)}<li><strong>{run.eventName} · {run.status}</strong><p>{run.handlerType} / {run.executionMode} · {run.scope}{run.durationMs?` · ${run.durationMs} ms`:''}</p>{#if run.statusMessage}<p>{run.statusMessage}</p>{/if}{#each run.entries as entry}<pre>{entry.kind}: {entry.text}</pre>{/each}</li>{/each}
    </ol></details>
  {/if}
</details>

<style>
  .native-hooks{margin-block:var(--ca-space-3);font:var(--ca-type-body)/var(--ca-leading-body) var(--ca-font-body);color:var(--ca-text);overflow-wrap:anywhere}
  summary{cursor:pointer;font-size:var(--ca-type-label)}
  summary:focus-visible,button:focus-visible{box-shadow:var(--ca-focus-ring)}
  p{margin-block:var(--ca-space-2);color:var(--ca-muted)}
  p[role="alert"],strong{color:var(--ca-text)}
  button{border:0;border-radius:var(--ca-control-radius);padding:var(--ca-space-2);background:var(--ca-app-background);color:var(--ca-text);font:inherit}
  button:disabled{color:var(--ca-muted)}
  ul,ol{padding-inline-start:var(--ca-space-6)}
  li{margin-block:var(--ca-space-3)}
  pre{max-height:12rem;overflow:auto;white-space:pre-wrap;overflow-wrap:anywhere;font:var(--ca-type-mono)/var(--ca-leading-body) var(--ca-font-mono)}
</style>
