<script lang="ts">
  import { onMount, tick } from "svelte";
  import {isWorking, type ProjectWork} from '../lib/project-board';
  import CollaborationIndicator from './CollaborationIndicator.svelte';

  interface ProjectCommand { label: string; command: string }
  interface ProjectMetadata {
    gitRepository: boolean;
    gitBranch?: string | null;
    gitModified: number;
    worktreeCount: number;
    stack: string[];
    instructionFiles: string[];
    commands: ProjectCommand[];
    error?: string | null;
  }
  interface ProjectCard {
    id: string;
    nodeKey: string;
    source: "local" | "ssh";
    name: string;
    path: string;
    active: boolean;
    pinned: boolean;
    lastOpenedAtMs: number;
    agentCount: number;
    access: string;
    sshProfileId?: string | null;
    sshProfileName?: string | null;
    metadata?: ProjectMetadata | null;
    scanning: boolean;
    activity?: ProjectWork[];
  }
  interface SshProfile { id: string; name: string; target: string; agentEnabled: boolean }
  interface RegistryState {
    projects: ProjectCard[];
    sshProfiles: SshProfile[];
    import: { active: boolean; kind?: string | null; message?: string | null; error: boolean };
    dropActive: boolean;
    addRequestId?: number;
  }

  let { eventTarget, compact = false, selectedNodeKey = "" }: { eventTarget: HTMLElement; compact?: boolean; selectedNodeKey?: string } = $props();
  let registry = $state<RegistryState>({
    projects: [], sshProfiles: [], import: { active: false, error: false },
    dropActive: false,
  });
  let query = $state("");
  let addOpen = $state(false);
  let addMode = $state<"choose" | "create" | "clone" | "ssh">("choose");
  let projectName = $state("");
  let repository = $state("");
  let sshProfileId = $state("");
  let sshDirectory = $state("");
  let sshName = $state("");
  let menuId = $state<string | null>(null);
  let removeProject = $state<ProjectCard | null>(null);
  let importWasActive = false;
  let observedAddRequestId = 0;
  let searchInput: HTMLInputElement;
  let modal = $state<HTMLDivElement>();

  const projects = $derived(registry.projects.filter(project => {
    const normalized = query.trim().toLocaleLowerCase();
    return !normalized || `${project.name}\n${project.path}\n${project.sshProfileName || ""}\n${project.metadata?.stack?.join(" ") || ""}`.toLocaleLowerCase().includes(normalized);
  }));
  const pinned = $derived(projects.filter(project => project.pinned));
  const recent = $derived(projects.filter(project => !project.pinned));

  export function update(next: RegistryState): void {
    registry = {
      projects: Array.isArray(next?.projects) ? next.projects : [],
      sshProfiles: Array.isArray(next?.sshProfiles) ? next.sshProfiles : [],
      import: next?.import && typeof next.import === "object"
        ? { active: Boolean(next.import.active), kind: next.import.kind || null, message: next.import.message || null, error: Boolean(next.import.error) }
        : { active: false, error: false },
      dropActive: Boolean(next?.dropActive),
      addRequestId: Number(next?.addRequestId || 0),
    };
    if (!sshProfileId || !registry.sshProfiles.some(profile => profile.id === sshProfileId)) {
      sshProfileId = registry.sshProfiles[0]?.id || "";
    }
    if (registry.import.active) importWasActive = true;
    else if ((importWasActive || registry.import.kind === "ssh" || registry.import.kind === "create") && registry.import.message && !registry.import.error) {
      importWasActive = false;
      addOpen = false;
      addMode = "choose";
      repository = "";
      projectName = "";
      sshDirectory = "";
      sshName = "";
    }
    const addRequestId = Number(registry.addRequestId || 0);
    if (addRequestId > 0 && addRequestId !== observedAddRequestId) {
      observedAddRequestId = addRequestId;
      openAdd();
    }
  }

  function action(name: string, detail: Record<string, unknown> = {}): void {
    eventTarget.dispatchEvent(new CustomEvent("central-agent:project-registry", {
      bubbles: true,
      detail: { action: name, ...detail },
    }));
  }

  function openAdd(mode: "choose" | "create" | "clone" | "ssh" = "choose"): void {
    addMode = mode;
    addOpen = true;
    menuId = null;
    void tick().then(() => modal?.querySelector<HTMLElement>("button, input, select")?.focus());
  }

  function closeAdd(): void {
    if (registry.import.active) return;
    addOpen = false;
    addMode = "choose";
  }

  function submitClone(event: SubmitEvent): void {
    event.preventDefault();
    if (!repository.trim() || registry.import.active) return;
    action("clone", { repository: repository.trim() });
  }

  function submitSsh(event: SubmitEvent): void {
    event.preventDefault();
    if (!sshProfileId || !sshDirectory.trim()) return;
    action("connect-ssh", {
      profileId: sshProfileId,
      directory: sshDirectory.trim(),
      name: sshName.trim() || null,
    });
  }

  function gitSummary(metadata?: ProjectMetadata | null): string {
    if (!metadata?.gitRepository) return "Not a Git repository";
    const branch = metadata.gitBranch || "Git repository";
    const modified = metadata.gitModified === 1 ? "1 modified" : `${metadata.gitModified || 0} modified`;
    const worktrees = metadata.worktreeCount > 1 ? ` · ${metadata.worktreeCount} worktrees` : "";
    return `${branch} · ${modified}${worktrees}`;
  }

  function cardKey(project: ProjectCard): string { return `${project.source}:${project.id}`; }

  function requestRemove(project: ProjectCard): void {
    menuId = null;
    removeProject = project;
  }

  function confirmRemove(): void {
    if (!removeProject) return;
    action("remove", { source: removeProject.source, id: removeProject.id });
    removeProject = null;
  }

  function keydown(event: KeyboardEvent): void {
    if ((event.ctrlKey || event.metaKey) && event.key.toLocaleLowerCase() === "k") {
      event.preventDefault();
      searchInput?.focus();
      searchInput?.select();
      return;
    }
    if (event.key === "Escape") {
      if (removeProject) removeProject = null;
      else if (menuId) menuId = null;
      else if (addOpen) closeAdd();
    }
  }

  onMount(() => {
    document.addEventListener("keydown", keydown);
    const outside = (event: PointerEvent) => {
      if (!menuId || !(event.target instanceof Node)) return;
      if (!eventTarget.querySelector(`[data-project-menu="${CSS.escape(menuId)}"]`)?.contains(event.target)) menuId = null;
    };
    document.addEventListener("pointerdown", outside);
    return () => {
      document.removeEventListener("keydown", keydown);
      document.removeEventListener("pointerdown", outside);
    };
  });
</script>

<section class="project-registry" class:compact aria-label="Supervisor projects">
  <header class="registry-header">
    <div>
      <h2>Projects</h2>
      {#if !compact}<p>Local folders and explicit SSH directories</p>{/if}
    </div>
    <button class="add-project" type="button" onclick={() => openAdd()}>
      <svg class="project-add-icon" viewBox="0 0 16 16" aria-hidden="true"><path d="M8 3v10M3 8h10"/></svg> Add project
    </button>
  </header>

  <label class="registry-search">
    <svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="8.5" cy="8.5" r="4.75"></circle><path d="m12 12 4.2 4.2"></path></svg>
    <input bind:this={searchInput} bind:value={query} type="search" placeholder="Find a project" autocomplete="off" aria-label="Find a project" />
    <kbd>Ctrl K</kbd>
  </label>

  <button class:active={registry.dropActive} class="drop-folder" type="button" onclick={() => action("select-local")}>
    <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 7.5h7l2 2h9v9.5H3z"></path><path d="M12 5v8m-3-3 3 3 3-3"></path></svg>
    <span><strong>{registry.dropActive ? "Release to add this project" : "Drop a folder here"}</strong><small>or choose a local folder</small></span>
  </button>

  {#if registry.import.message && !addOpen}
    <div class:error={registry.import.error} class="registry-status" role="status">{registry.import.message}</div>
  {/if}

  <div class="project-list">
    {#if pinned.length}
      <h3>Pinned</h3>
      {#each pinned as project (cardKey(project))}
        {@render projectCard(project)}
      {/each}
    {/if}
    <h3>{pinned.length ? "Recent" : "Recent projects"}</h3>
    {#if recent.length}
      {#each recent as project (cardKey(project))}
        {@render projectCard(project)}
      {/each}
    {:else if !pinned.length}
      <div class="empty-projects">
        <strong>No projects yet</strong>
        <span>Add a folder, clone a repository, or connect an SSH directory.</span>
      </div>
    {/if}
  </div>

</section>

{#snippet projectCard(project: ProjectCard)}
  {@const liveWork=(project.activity||[]).filter(work=>isWorking(work.state))}
  <article class:active={compact ? project.nodeKey === selectedNodeKey : project.active} class:working={liveWork.length>0} class="project-card" data-project-node={project.nodeKey}>
    <div class="project-card-head">
      <span class:remote={project.source === "ssh"} class="project-icon" aria-hidden="true">
        {#if project.source === "ssh"}
          <svg viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="6" rx="2"></rect><rect x="3" y="14" width="18" height="6" rx="2"></rect><path d="M7 7h.01M7 17h.01M11 7h6M11 17h6"></path></svg>
        {:else}
          <svg viewBox="0 0 24 24"><path d="M3 6.5h7l2 2h9v10H3z"></path></svg>
        {/if}
      </span>
      <button type="button" class="project-title" class:project-select={compact} aria-pressed={project.nodeKey === selectedNodeKey} title={project.path} onclick={() => action(compact ? "select-project" : "open-project", { source: project.source, id: project.id, nodeKey: project.nodeKey })}>
        <strong>{project.name}</strong>
        <small>{compact ? (project.source === "ssh" ? project.sshProfileName || "SSH" : "Local") : project.path}{compact && project.metadata?.gitBranch ? ` · ${project.metadata.gitBranch}` : ""}</small>
      </button>
      <div class="project-card-actions">
        {#if liveWork.length}<CollaborationIndicator label={`${liveWork.length} active ${liveWork.length===1?'agent is':'agents are'} collaborating in ${project.name}`} />{/if}
        <div class="project-menu" data-project-menu={cardKey(project)}>
          <button class="menu-toggle" type="button" aria-label={`Project actions for ${project.name}`} aria-expanded={menuId === cardKey(project)} onclick={() => menuId = menuId === cardKey(project) ? null : cardKey(project)}><svg class="project-menu-icon" viewBox="0 0 16 16" aria-hidden="true"><circle cx="4" cy="8" r="1"/><circle cx="8" cy="8" r="1"/><circle cx="12" cy="8" r="1"/></svg></button>
          {#if menuId === cardKey(project)}
            <div class="menu-popover">
              {#if compact}<button type="button" onclick={() => {action("start-agent", { source: project.source, id: project.id, nodeKey: project.nodeKey }); menuId = null;}}>New supervisor</button>{/if}
              <button type="button" onclick={() => { action("terminal", { source: project.source, id: project.id }); menuId = null; }}>Open terminal</button>
              {#if project.source === "local"}<button type="button" onclick={() => { action("folder", { root: project.id }); menuId = null; }}>Show folder</button>{/if}
              <button type="button" disabled={!project.metadata?.gitRepository} onclick={() => { action("git", { source: project.source, id: project.id }); menuId = null; }}>Git status</button>
              <button type="button" aria-label={project.pinned ? "Unpin project" : "Pin project"} onclick={() => { action("pin", { source: project.source, id: project.id, pinned: !project.pinned }); menuId = null; }}>{project.pinned ? "Unpin" : "Pin project"}</button>
              <button type="button" onclick={() => { action("refresh", { source: project.source, id: project.id }); menuId = null; }}>Refresh metadata</button>
              <button class="remove" type="button" onclick={() => requestRemove(project)}>Remove from Supervisor</button>
            </div>
          {/if}
        </div>
      </div>
    </div>

    {#if liveWork.length}
      <div class="project-work" aria-label={`Active work in ${project.name}`}>
        {#each liveWork as work (work.owner)}
          {@const fileCount=work.files.filter(file=>isWorking(file.state)).length}
          <button type="button" data-work-owner={work.owner} title={`Follow ${work.label}`} onclick={()=>action('focus-work',{nodeKey:project.nodeKey,owner:work.owner})}>
            <span aria-hidden="true">↳</span><span>{work.label}</span><small>{fileCount ? `${fileCount} ${fileCount===1?'file':'files'}` : work.state==='waiting'?'Waiting':'Working'}</small>
          </button>
        {/each}
      </div>
    {/if}

    {#if !compact}<dl class="project-facts">
      <div><dt>Git</dt><dd>{project.scanning ? "Detecting…" : gitSummary(project.metadata)}</dd></div>
      <div><dt>Stack</dt><dd>{project.scanning && !project.metadata ? "Detecting…" : project.metadata?.stack?.length ? project.metadata.stack.join(" · ") : "Not detected"}</dd></div>
      <div><dt>Agents</dt><dd>{project.agentCount} available</dd></div>
      <div><dt>Access</dt><dd>{project.access}</dd></div>
    </dl>
    {#if project.metadata?.instructionFiles?.length}
      <p class="detected-note">Instructions · {project.metadata.instructionFiles.join(" · ")}</p>
    {:else if project.metadata?.error}
      <p class="detected-note error">{project.metadata.error}</p>
    {/if}
    {#if project.metadata?.commands?.length}
      <p class="detected-note">Commands · {project.metadata.commands.map(command => command.label).join(" · ")}</p>
    {/if}
    <div class="project-actions">
      <button type="button" onclick={() => action("open-project", { source: project.source, id: project.id, nodeKey: project.nodeKey })}>Open project</button>
      <button class="primary" type="button" onclick={() => action("start-agent", { source: project.source, id: project.id, nodeKey: project.nodeKey })}>Start agent</button>
    </div>
    {/if}
  </article>
{/snippet}

{#if addOpen}
  <div bind:this={modal} class="add-backdrop" role="presentation" onpointerdown={(event) => { if (event.target === event.currentTarget) closeAdd(); }}>
    <div class="add-dialog" role="dialog" aria-modal="true" aria-labelledby="add-project-title">
      <header>
        <div><h2 id="add-project-title">Add project</h2><p>Choose one explicit project root. Supervisor will inspect it in the background.</p></div>
        <button type="button" aria-label="Close" onclick={closeAdd}>×</button>
      </header>

      {#if addMode === "choose"}
        <button class="add-choice" type="button" onclick={() => addMode = "create"}>
          <span class="choice-icon"><svg viewBox="0 0 24 24"><path d="M3 6.5h7l2 2h9v10H3zM12 11v5m-2.5-2.5h5"/></svg></span>
          <span><strong>New project</strong><small>Create an empty project folder</small></span><i>›</i>
        </button>
        <button class="add-choice" type="button" onclick={() => { closeAdd(); action("select-local"); }}>
          <span class="choice-icon"><svg viewBox="0 0 24 24"><path d="M3 6.5h7l2 2h9v10H3z"></path></svg></span>
          <span><strong>Local folder</strong><small>Open the native Windows folder picker</small></span><i>›</i>
        </button>
        <button class="add-choice" type="button" onclick={() => addMode = "clone"}>
          <span class="choice-icon"><svg viewBox="0 0 24 24"><circle cx="6" cy="5" r="2"></circle><circle cx="18" cy="7" r="2"></circle><circle cx="8" cy="19" r="2"></circle><path d="M7.5 6.2 16.4 7M7 6.7l.8 10.2"></path></svg></span>
          <span><strong>Clone repository</strong><small>Clone a Git URL into a folder you choose</small></span><i>›</i>
        </button>
        <button class="add-choice" type="button" disabled={!registry.sshProfiles.length} onclick={() => addMode = "ssh"}>
          <span class="choice-icon"><svg viewBox="0 0 24 24"><rect x="3" y="4" width="18" height="6" rx="2"></rect><rect x="3" y="14" width="18" height="6" rx="2"></rect><path d="M7 7h.01M7 17h.01"></path></svg></span>
          <span><strong>SSH directory</strong><small>{registry.sshProfiles.length ? "Use a saved server and an absolute remote path" : "Add an SSH server in Settings first"}</small></span><i>›</i>
        </button>
      {:else if addMode === "create"}
        <form onsubmit={event=>{event.preventDefault();if(projectName.trim())action('create',{name:projectName.trim()});}}>
          <button class="back-choice" type="button" onclick={() => addMode = "choose"}>← All project sources</button>
          <label>Project name<input bind:value={projectName} type="text" maxlength="80" placeholder="My project" autocomplete="off" required /></label>
          <p class="form-help">Choose a destination next. A new, empty folder will be created and added to Supervisor.</p>
          {#if registry.import.kind === 'create' && registry.import.error}<p class="form-status error" role="alert">{registry.import.message}</p>{/if}
          <button class="submit-project" type="submit" disabled={!projectName.trim()}>Choose location and create</button>
        </form>
      {:else if addMode === "clone"}
        <form onsubmit={submitClone}>
          <button class="back-choice" type="button" onclick={() => addMode = "choose"}>← All project sources</button>
          <label>Repository URL<input bind:value={repository} type="text" inputmode="url" maxlength="2048" placeholder="https://github.com/owner/repository.git" autocomplete="off" required /></label>
          <p class="form-help">You will choose the destination folder with the native Windows picker.</p>
          {#if registry.import.kind === "clone" && registry.import.message}<p class:error={registry.import.error} class="form-status">{registry.import.message}</p>{/if}
          <button class="submit-project" type="submit" aria-label={registry.import.active ? "Repository clone in progress" : "Choose destination and clone repository"} disabled={registry.import.active || !repository.trim()}>{registry.import.active ? "Cloning…" : "Choose destination and clone"}</button>
        </form>
      {:else}
        <form onsubmit={submitSsh}>
          <button class="back-choice" type="button" onclick={() => addMode = "choose"}>← All project sources</button>
          <label>Server<select bind:value={sshProfileId} required>{#each registry.sshProfiles as profile (profile.id)}<option value={profile.id}>{profile.name} · {profile.target}</option>{/each}</select></label>
          <label>Remote directory<input bind:value={sshDirectory} type="text" maxlength="1024" placeholder="/srv/projects/my-app" autocomplete="off" required /></label>
          <label>Project name <small>Optional</small><input bind:value={sshName} type="text" maxlength="80" placeholder="Detected from the directory" autocomplete="off" /></label>
          {#if registry.import.kind === "ssh" && registry.import.message}<p class:error={registry.import.error} class="form-status">{registry.import.message}</p>{/if}
          <button class="submit-project" type="submit" disabled={!sshProfileId || !sshDirectory.trim()}>Connect SSH project</button>
        </form>
      {/if}
    </div>
  </div>
{/if}

{#if removeProject}
  <div class="add-backdrop" role="presentation" onpointerdown={(event) => { if (event.target === event.currentTarget) removeProject = null; }}>
    <div class="remove-dialog" role="alertdialog" aria-modal="true" aria-labelledby="remove-project-title">
      <h2 id="remove-project-title">Remove project?</h2>
      <p><strong>{removeProject.name}</strong> will be removed from Supervisor. Its local or remote files will remain unchanged.</p>
      <div><button type="button" onclick={() => removeProject = null}>Cancel</button><button class="remove-confirm" type="button" onclick={confirmRemove}>Remove project</button></div>
    </div>
  </div>
{/if}

<style>
  .project-work{display:grid;gap:var(--ca-space-1);margin-top:var(--ca-space-2);max-height:var(--ca-project-work-links-height);overflow:auto}
  .project-work button{display:flex;align-items:center;gap:var(--ca-space-2);padding:var(--ca-space-1) var(--ca-space-2);border:0;border-radius:var(--ca-radius-small);background:var(--ca-surface-3);color:var(--ca-text);text-align:left;font-size:var(--ca-type-caption);cursor:pointer;min-height:var(--ca-control-compact)}
  .project-work button>span:nth-child(2){flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .project-work small{color:var(--ca-muted);font-size:inherit}
  .project-title{padding:0;border:0;background:transparent;color:inherit;text-align:left;cursor:pointer}
  .project-title:focus-visible{outline:none;box-shadow:var(--ca-focus-ring)}
  .compact{padding:var(--ca-space-3);gap:var(--ca-space-3);container-type:normal}
  .compact .registry-header{align-items:flex-start;flex-direction:row}
  .compact .registry-header h2{font-weight:600}
  .compact .add-project{min-width:var(--ca-control-compact);padding:0 var(--ca-space-2);font-size:var(--ca-type-caption);font-weight:500;background:var(--ca-surface-3);color:var(--ca-text)}
  .compact .project-card{padding:var(--ca-space-3) var(--ca-space-2);box-shadow:none;background:transparent;gap:0}
  .compact .project-card:hover{background:var(--ca-surface-2)}
  .compact .project-card.active{background:var(--ca-accent-soft)}
  .compact .project-title strong{font-size:var(--ca-type-body);font-weight:550}
  .compact .project-icon,.compact .project-icon.remote{background:transparent;color:var(--ca-icon);width:var(--ca-space-6)}
  .compact .project-select{min-height:var(--ca-control-default)}
  .compact .drop-folder{min-height:var(--ca-control-prominent);padding:var(--ca-space-2);font-size:var(--ca-type-caption)}
  .compact .drop-folder strong{font-weight:450}
  .compact .drop-folder small,.compact .registry-search kbd{display:none}
  button,input,select{font:inherit}
  button{color:inherit}
  .project-registry{display:flex;height:100%;min-height:0;flex-direction:column;gap:var(--ca-space-3);padding:var(--project-registry-padding,var(--ca-space-4));background:var(--ca-app-background);color:var(--ca-text);container-type:inline-size}
  .registry-header{display:flex;align-items:flex-start;justify-content:space-between;gap:var(--ca-space-3)}
  h2,h3,p{margin:0}
  .registry-header h2{font:680 var(--ca-type-title)/1.2 var(--ca-font-display);letter-spacing:-.015em}
  .registry-header p{margin-top:3px;color:var(--ca-muted);font-size:var(--ca-type-caption)}
  .add-project{display:flex;min-height:var(--ca-control-compact);align-items:center;gap:6px;padding:0 var(--ca-space-3);border:0;border-radius:var(--ca-radius-medium);background:var(--ca-text);color:var(--ca-app-background);font-weight:650;cursor:pointer;white-space:nowrap}
  .add-project .project-add-icon{display:block;width:var(--ca-space-3);height:var(--ca-space-3);fill:none;stroke:currentColor;stroke-width:2;stroke-linecap:round}
  .registry-search{display:flex;min-height:var(--ca-control-default);align-items:center;gap:var(--ca-space-2);padding:0 var(--ca-space-3);border:0;border-radius:var(--ca-radius-medium);background:var(--ca-surface-2)}
  .registry-search svg{width:18px;height:18px;fill:none;stroke:var(--ca-muted);stroke-width:1.7;stroke-linecap:round}
  .registry-search input{min-width:0;flex:1;border:0;outline:0;background:transparent;color:var(--ca-text)}
  .registry-search kbd{padding:2px 6px;border:0;border-radius:var(--ca-radius-small);background:var(--ca-surface-3);color:var(--ca-muted);font:500 var(--ca-type-caption)/1.4 var(--ca-font-body)}
  .drop-folder{display:flex;min-height:76px;align-items:center;gap:var(--ca-space-3);padding:var(--ca-space-3);border:0;border-radius:var(--ca-radius-large);background:var(--ca-surface-2);text-align:left;cursor:pointer;transition:background var(--ca-duration-standard) var(--ca-ease-standard),transform var(--ca-duration-standard) var(--ca-ease-standard)}
  .drop-folder:hover,.drop-folder:focus-visible,.drop-folder.active{outline:0;background:var(--ca-surface-3)}
  .drop-folder.active{transform:scale(.985)}
  .drop-folder svg{width:28px;height:28px;fill:none;stroke:var(--ca-text);stroke-width:1.5;stroke-linecap:round;stroke-linejoin:round}
  .drop-folder span,.project-title,.add-choice>span:nth-child(2){display:grid;min-width:0;gap:2px}
  .drop-folder strong,.project-title strong,.add-choice strong{font-weight:650}
  .drop-folder small,.project-title small,.add-choice small{overflow:hidden;color:var(--ca-muted);font-size:var(--ca-type-caption);text-overflow:ellipsis;white-space:nowrap}
  .registry-status,.form-status{padding:var(--ca-space-2) var(--ca-space-3);border-radius:var(--ca-radius-medium);background:var(--ca-surface-2);color:var(--ca-muted);font-size:var(--ca-type-caption)}
  .registry-status.error,.form-status.error,.detected-note.error{color:var(--ca-text)}
  .project-list{display:flex;min-height:0;flex:1;flex-direction:column;gap:var(--ca-space-2);overflow:auto;padding-right:2px}
  .project-list h3{padding:var(--ca-space-2) 2px 0;color:var(--ca-muted);font:650 var(--ca-type-caption)/1.2 var(--ca-font-body);text-transform:uppercase;letter-spacing:.06em}
  .project-card{position:relative;display:grid;gap:var(--ca-space-3);padding:var(--ca-space-3);border:0;border-radius:var(--ca-radius-large);background:var(--ca-surface);box-shadow:var(--ca-shadow-keyline)}
  .project-card.active{background:var(--ca-surface-2)}
  .project-card-head{display:grid;grid-template-columns:auto minmax(0,1fr) auto;align-items:center;gap:var(--ca-space-2)}
  .project-card-actions{display:flex;align-items:center;gap:var(--ca-space-1)}
  .project-icon{display:grid;width:34px;height:34px;place-items:center;border-radius:var(--ca-radius-medium);background:var(--ca-surface-3);color:var(--ca-text)}
  .project-icon svg{width:20px;height:20px;fill:none;stroke:currentColor;stroke-width:1.5;stroke-linecap:round;stroke-linejoin:round}
  .project-icon.remote{background:var(--ca-text);color:var(--ca-app-background)}
  .project-title strong{overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .project-menu{position:relative}
  .menu-toggle{display:grid;width:var(--ca-control-compact);height:var(--ca-control-compact);place-items:center;padding:0;border:0;border-radius:var(--ca-radius-small);background:transparent;color:var(--ca-muted);line-height:1;cursor:pointer}.menu-toggle .project-menu-icon{width:var(--ca-space-3);height:var(--ca-space-3);fill:currentColor;stroke:none}
  .menu-toggle:hover,.menu-toggle:focus-visible{outline:0;background:var(--ca-surface-3);color:var(--ca-text)}
  .menu-popover{position:absolute;z-index:12;top:34px;right:0;display:grid;width:190px;padding:6px;border-radius:var(--ca-radius-medium);background:var(--ca-surface-2);box-shadow:var(--ca-shadow-float)}
  .menu-popover button{min-height:32px;padding:0 var(--ca-space-2);border:0;border-radius:var(--ca-radius-small);background:transparent;text-align:left;cursor:pointer}
  .menu-popover button:hover,.menu-popover button:focus-visible{outline:0;background:var(--ca-surface-3)}
  .menu-popover button:disabled{opacity:.42;cursor:default}
  .menu-popover .remove{color:var(--ca-muted)}
  .project-facts{display:grid;margin:0;gap:6px}
  .project-facts div{display:grid;grid-template-columns:52px minmax(0,1fr);gap:var(--ca-space-2)}
  .project-facts dt{color:var(--ca-muted);font-size:var(--ca-type-caption)}
  .project-facts dd{min-width:0;margin:0;overflow:hidden;color:var(--ca-text);font-size:var(--ca-type-caption);text-overflow:ellipsis;white-space:nowrap}
  .detected-note{overflow:hidden;color:var(--ca-muted);font-size:var(--ca-type-caption);text-overflow:ellipsis;white-space:nowrap}
  .project-actions{display:grid;grid-template-columns:1fr 1fr;gap:var(--ca-space-2)}
  .project-actions button,.submit-project{min-height:34px;border:0;border-radius:var(--ca-radius-medium);background:var(--ca-surface-3);font-weight:640;cursor:pointer}
  .project-actions button:hover,.project-actions button:focus-visible,.submit-project:hover,.submit-project:focus-visible{outline:0;filter:brightness(1.08)}
  .project-actions .primary,.submit-project{background:var(--ca-text);color:var(--ca-app-background)}
  .empty-projects{display:grid;place-items:center;gap:4px;min-height:130px;padding:var(--ca-space-5);color:var(--ca-muted);text-align:center}
  .empty-projects strong{color:var(--ca-text)}
  .empty-projects span{font-size:var(--ca-type-caption)}
  .add-backdrop{position:fixed;z-index:100;inset:0;display:grid;place-items:center;padding:var(--ca-space-5);background:color-mix(in srgb,var(--ca-app-background) 72%,transparent);backdrop-filter:blur(6px)}
  .add-dialog,.remove-dialog{display:grid;width:min(480px,calc(100vw - 40px));max-height:calc(100dvh - 40px);gap:var(--ca-space-3);overflow:auto;padding:var(--ca-space-5);border:0;border-radius:var(--ca-radius-large);background:var(--ca-surface);box-shadow:var(--ca-shadow-float);animation:dialog-in var(--ca-duration-deliberate) var(--ca-ease-emphasized)}
  .add-dialog>header{display:flex;align-items:flex-start;justify-content:space-between;gap:var(--ca-space-3)}
  .add-dialog header h2,.remove-dialog h2{font:680 var(--ca-type-settings-sidebar-title-compact)/1.2 var(--ca-font-display);letter-spacing:-.015em}
  .add-dialog header p,.remove-dialog p{margin-top:4px;color:var(--ca-muted);font-size:var(--ca-type-body)}
  .add-dialog header>button{width:32px;height:32px;border:0;border-radius:var(--ca-radius-small);background:transparent;font-size:var(--ca-type-settings-sidebar-title-compact);cursor:pointer}
  .add-choice{display:grid;grid-template-columns:auto minmax(0,1fr) auto;align-items:center;gap:var(--ca-space-3);min-height:68px;padding:var(--ca-space-3);border:0;border-radius:var(--ca-radius-large);background:var(--ca-surface-2);text-align:left;cursor:pointer}
  .add-choice:hover,.add-choice:focus-visible{outline:0;background:var(--ca-surface-3)}
  .add-choice:disabled{opacity:.48;cursor:default}
  .choice-icon{display:grid;width:36px;height:36px;place-items:center;border-radius:var(--ca-radius-medium);background:var(--ca-surface-3)}
  .choice-icon svg{width:21px;height:21px;fill:none;stroke:currentColor;stroke-width:1.55;stroke-linecap:round;stroke-linejoin:round}
  .add-choice i{color:var(--ca-muted);font:400 22px/1 var(--ca-font-body)}
  .add-dialog form{display:grid;gap:var(--ca-space-3)}
  .back-choice{justify-self:start;padding:0;border:0;background:transparent;color:var(--ca-muted);cursor:pointer}
  .add-dialog label{display:grid;gap:6px;font-weight:640}
  .add-dialog label small{color:var(--ca-muted);font-weight:500}
  .add-dialog input,.add-dialog select{width:100%;min-height:42px;padding:0 var(--ca-space-3);border:0;border-radius:var(--ca-radius-medium);outline:0;background:var(--ca-surface-2);color:var(--ca-text)}
  .add-dialog input:focus,.add-dialog select:focus{box-shadow:var(--ca-focus-ring)}
  .form-help{color:var(--ca-muted);font-size:var(--ca-type-caption)}
  .submit-project{min-height:42px}
  .submit-project:disabled{opacity:.5;cursor:default}
  .remove-dialog p{line-height:var(--ca-leading-body)}
  .remove-dialog>div{display:flex;justify-content:flex-end;gap:var(--ca-space-2)}
  .remove-dialog button{min-height:38px;padding:0 var(--ca-space-4);border:0;border-radius:var(--ca-radius-medium);background:var(--ca-surface-2);font-weight:640;cursor:pointer}
  .remove-dialog .remove-confirm{background:var(--ca-text);color:var(--ca-app-background)}
  @keyframes dialog-in{from{opacity:0;transform:translateY(8px) scale(.98)}to{opacity:1;transform:none}}
  @container (max-width:300px){.registry-header{align-items:stretch;flex-direction:column}.add-project{justify-content:center}.registry-search kbd{display:none}.project-actions{grid-template-columns:1fr}.project-facts div{grid-template-columns:44px minmax(0,1fr)}}
  @media(prefers-reduced-motion:reduce){.add-dialog,.remove-dialog{animation:none}.drop-folder{transition:none}}
</style>
