<script lang="ts">
  import {activityFiles,fileActivityLabel,fileRows,isWorking,type BoardProject,type BoardFile,type ProjectWork} from '../lib/project-board';
  import CollaborationIndicator from './CollaborationIndicator.svelte';

  let {project,entries,work,onOpen}:{
    project:BoardProject|null;entries:BoardFile[];work:ProjectWork[];onOpen:(path:string)=>void;
  }=$props();
  let query=$state(''),expanded=$state(new Set<string>());
  const rows=$derived(fileRows(entries,expanded,query));
  const activity=$derived(activityFiles(work,project?.source==='ssh'));

  function toggle(path:string):void {
    const next=new Set(expanded);
    if(next.has(path))next.delete(path);else next.add(path);
    expanded=next;
  }
  function info(path:string){return activity.find(file=>project?.source==='ssh'?file.path===path:file.path.toLowerCase()===path.toLowerCase());}
  function fileIcon(element:HTMLElement,key:string):{update:(key:string)=>void} {
    const draw=(next:string)=>(window as unknown as {populateCentralFileIcon?:(element:HTMLElement,key:string)=>void}).populateCentralFileIcon?.(element,next||'file');
    draw(key);return{update:draw};
  }
  $effect(()=>{void project?.nodeKey;query='';expanded=new Set();});
</script>

<input class="search" type="search" aria-label="Find a project file" placeholder="Find a file" bind:value={query}/>
<div class="file-scroll" aria-label="Files and folders">
  {#each rows as file (file.path)}
    {@const activityInfo=info(file.path)}
    <button type="button" class="file-row"
      style:padding-inline-start={`calc(var(--ca-space-2) + ${Math.min(file.depth,8)} * var(--ca-space-3))`}
      aria-label={`${file.kind==='directory'?(file.expanded?'Collapse':'Expand'):'Open'} ${file.path}`}
      aria-expanded={file.kind==='directory'?file.expanded:undefined}
      onclick={()=>file.kind==='directory'?toggle(file.path):onOpen(file.path)}>
      <span aria-hidden="true">{file.kind==='directory'?(file.expanded?'⌄':'›'):''}</span>
      {#if file.kind==='directory'}
        <svg class="file-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M3 6h7l2 2h9v11H3z"/></svg>
      {:else}<span class="file-icon ca-file-icon" aria-hidden="true" use:fileIcon={file.iconKey||'file'}></span>{/if}
      <span class="file-copy"><strong>{file.name}</strong>{#if file.kind==='directory'}<small>{fileActivityLabel(activity,file.path,true,project?.source==='ssh')}</small>{/if}</span>
      {#if activityInfo&&isWorking(activityInfo.state)}<CollaborationIndicator compact label={`Agents are working on ${file.path}`}/>{/if}
    </button>
  {:else}<p class="notice">{project?'No matching files.':'Choose a project to browse its files.'}</p>{/each}
</div>
{#if project?.source==='ssh'}<p class="notice">Selecting a remote file opens the SSH terminal.</p>{/if}

<style>
  .search{box-sizing:border-box;min-width:0;width:100%;min-height:var(--ca-control-default);border:0;border-radius:var(--ca-radius-medium);padding:var(--ca-space-2) var(--ca-space-3);background:var(--ca-surface-2);color:var(--ca-text);font:var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body);margin-bottom:var(--ca-space-3)}
  .search:focus-visible,button:focus-visible{outline:none;box-shadow:var(--ca-focus-ring)}
  .file-scroll{box-sizing:border-box;inline-size:calc(100% + var(--ca-scrollbar-size));max-inline-size:none;flex:1;min-height:0;margin-inline-end:calc(-1 * var(--ca-scrollbar-size));overflow:auto;scrollbar-gutter:stable}
  .file-row{display:flex;width:100%;align-items:center;text-align:left;gap:var(--ca-space-2);margin-bottom:var(--ca-space-1);min-height:var(--ca-control-default);border:0;border-radius:var(--ca-control-radius);background:var(--ca-surface-2);color:var(--ca-text);font:var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body);cursor:pointer}
  .file-row:hover{background:var(--ca-surface-3)}
  .file-icon{width:var(--ca-space-4);height:var(--ca-space-4);flex-shrink:0;fill:none;stroke:currentColor;stroke-width:1.5}
  .file-copy{flex:1;min-width:0;display:grid;gap:var(--ca-space-1)}strong{font-weight:500;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}small,.notice{font-size:var(--ca-type-caption);color:var(--ca-muted);overflow-wrap:anywhere}.notice{padding:var(--ca-space-2);margin:0}
</style>
