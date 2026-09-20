<script lang="ts">
  import {activityFiles,fileWorkLabel,isWorking,workLabel,type ProjectWork} from '../lib/project-board';
  import CollaborationIndicator from './CollaborationIndicator.svelte';
  let {work,remote=false,owner='',onSelect,onOpen}:{work:ProjectWork[];remote?:boolean;owner?:string;onSelect:(owner:string)=>void;onOpen:(path:string)=>void}=$props();
  const selected=$derived(work.find(task=>task.owner===owner));
  const files=$derived(activityFiles(selected?[selected]:work,remote));
  const active=$derived(files.filter(file=>isWorking(file.state)).length);
  const working=$derived((selected?[selected]:work).filter(task=>isWorking(task.state)).length);
</script>

{#if work.length}
  <section class="file-activity" aria-label="Task file activity">
    <header><h3>Task activity</h3><span>{active ? `${active} files active` : working ? 'Working' : 'Latest work'}</span></header>
    <div class="work-filters" aria-label="Filter file activity by task">
      <button type="button" aria-pressed={!selected} onclick={()=>onSelect('')}>All tasks</button>
      {#each work as task (task.owner)}
        <button type="button" aria-pressed={owner===task.owner} title={`${task.label} · ${workLabel(task.state)}`} onclick={()=>onSelect(task.owner)}><span aria-hidden="true">↳</span> {task.label}<small>{workLabel(task.state)}</small></button>
      {/each}
    </div>
    <div class="activity-files">
      {#each files as file (file.path)}
        <button type="button" class:working={isWorking(file.state)} data-file-activity={file.path} data-file-state={file.state}
          title={`${file.path}\n${file.owners.map(task=>`${task.label} · ${task.operation==='edit'?'Edit':'Read'} · ${workLabel(task.state)}`).join('\n')}`}
          onclick={()=>onOpen(file.path)}>
          <span class="state-symbol" aria-hidden="true">{file.state==='working'?'●':file.state==='done'?'✓':file.state==='failed'?'!':file.state==='waiting'?'◷':'■'}</span>
          <span class="file-copy"><strong>{file.path}</strong><small>{file.owners.map(task=>task.label).join(' · ')}</small></span>
          {#if isWorking(file.state)}<CollaborationIndicator compact label={`Agents are collaborating on ${file.path}`} />{/if}
          <span class="file-state">{fileWorkLabel(file)}</span>
        </button>
      {:else}
        <p>{working?'Waiting for file activity reported by the tools.':'No file activity reported for this task.'}</p>
      {/each}
      {#if (selected?[selected]:work).some(task=>task.truncated)}<p>Showing a limited file activity list.</p>{/if}
    </div>
  </section>
{/if}

<style>
  .file-activity{display:flex;flex-direction:column;min-height:0;flex-shrink:0;max-height:var(--ca-project-file-activity-height);gap:var(--ca-space-2);padding:var(--ca-space-3);margin-bottom:var(--ca-space-3);border-radius:var(--ca-radius-large);background:var(--ca-surface)}
  header{display:flex;align-items:center;justify-content:space-between;gap:var(--ca-space-2)}h3{margin:0;font-size:var(--ca-type-label);font-weight:600}header>span,p{color:var(--ca-muted);font-size:var(--ca-type-caption)}p{margin:var(--ca-space-2) 0}
  .work-filters{display:flex;gap:var(--ca-space-1);overflow:auto;flex-shrink:0;padding-bottom:var(--ca-space-1)}
  button{font:inherit;cursor:pointer;color:var(--ca-text);border:0;border-radius:var(--ca-radius-medium);background:var(--ca-surface-2)}button:focus-visible{outline:none;box-shadow:var(--ca-focus-ring)}button:hover{background:var(--ca-surface-3)}
  .work-filters button{padding:var(--ca-space-1) var(--ca-space-2);font-size:var(--ca-type-caption);white-space:nowrap;min-height:var(--ca-control-compact)}.work-filters button[aria-pressed=true]{background:var(--ca-accent);color:var(--ca-on-accent)}.work-filters small{margin-left:var(--ca-space-2);font-size:inherit}
  .activity-files{display:grid;gap:var(--ca-space-1);min-height:0;overflow:auto;scrollbar-width:thin}
  .activity-files button{display:flex;align-items:center;gap:var(--ca-space-2);min-width:0;min-height:var(--ca-control-prominent);padding:var(--ca-space-2);text-align:left;font-size:var(--ca-type-caption)}.activity-files button.working{background:var(--ca-accent-soft)}
  .file-copy{display:grid;gap:var(--ca-space-1);min-width:0;flex:1}.file-copy strong{font-weight:500;overflow-wrap:anywhere}.file-copy small{font-size:inherit;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;color:var(--ca-muted)}
  .file-state{max-width:40%;text-align:right;font-size:var(--ca-type-caption)}.state-symbol{flex-shrink:0;color:var(--ca-muted)}.working .state-symbol{color:var(--ca-text)}
</style>
