<script lang="ts">
  import NativeMcpConfiguration from "./NativeMcpConfiguration.svelte";
  import { emptyNativeMcp, mcpAuthLabel, mcpCanLogin, mcpRuntimeLabel, type McpAction, type NativeMcpServer, type NativeMcpView } from "../lib/app-server-mcp";
  let { view = emptyNativeMcp(), connected, onAction, threadScope = false }: {view?:NativeMcpView;connected:boolean;onAction:(action:McpAction)=>void;threadScope?:boolean} = $props();
  let dialog: HTMLDialogElement;
  let toolDialog: HTMLDialogElement;
  let confirmedInventory = $state<string|null>(null);
  let filter = $state("");
  let toolServer=$state(''),toolName=$state(''),toolSchema=$state('{}'),toolArguments=$state('{}'),toolError=$state(''),toolInventory=$state<string|null>(null);
  const blocked = $derived(!connected || view.busy || !!view.login);
  const normalizedFilter = $derived(filter.trim().toLocaleLowerCase());
  function entryMatches(entry:{name:string;description:string|null}):boolean {
    return !normalizedFilter || `${entry.name} ${entry.description||""}`.toLocaleLowerCase().includes(normalizedFilter);
  }
  function serverMatches(server:NativeMcpServer):boolean {
    return !normalizedFilter || `${server.name} ${mcpRuntimeLabel(server.runtimeStatus)} ${mcpAuthLabel(server.authStatus)}`.toLocaleLowerCase().includes(normalizedFilter);
  }
  function toolsFor(server:NativeMcpServer) {
    const tools=Object.values(server.tools);
    return serverMatches(server)?tools:tools.filter(entryMatches);
  }
  function resourcesFor(server:NativeMcpServer) {
    return serverMatches(server)?server.resources:server.resources.filter(entryMatches);
  }
  function templatesFor(server:NativeMcpServer) {
    return serverMatches(server)?server.resourceTemplates:server.resourceTemplates.filter(entryMatches);
  }
  const visibleServers=$derived(view.servers.filter(server=>serverMatches(server)||toolsFor(server).length>0||resourcesFor(server).length>0||templatesFor(server).length>0));
  $effect(() => {
    if (blocked || !view.current || confirmedInventory !== view.inventoryId) {
      confirmedInventory = null;
      dialog?.close();
    }
    if (blocked || !view.current || toolInventory !== view.inventoryId) {
      toolInventory = null;
      toolDialog?.close();
    }
  });
  function reload():void {
    if (threadScope || blocked || !view.current || !view.inventoryId) return;
    confirmedInventory = view.inventoryId;
    dialog.showModal();
  }
  function confirmReload():void {
    const id = confirmedInventory;
    confirmedInventory = null;
    dialog.close();
    if (!threadScope && !blocked && view.current && id && id === view.inventoryId) onAction({kind:"reload",inventory_id:id});
  }
  function closed():void {
    if(!dialog.open)confirmedInventory=null;
  }
  function authorize(name:string):void {
    if(blocked || !view.current || !view.inventoryId)return;
    const server=view.servers.find(server=>server.name===name);
    if(!server || !mcpCanLogin(server))return;
    onAction({kind:"login",inventory_id:view.inventoryId,name});
  }
  function readResource(server:string,resource:{entryId?:string}):void {
    if(blocked || !view.current || !view.inventoryId || !resource.entryId)return;
    onAction({kind:'read_resource',inventory_id:view.inventoryId,server,resource_id:resource.entryId});
  }
  function prepareTool(server:string,tool:{name:string;inputSchema?:string|null}):void {
    if(!threadScope || blocked || !view.current || !view.inventoryId)return;
    toolServer=server;toolName=tool.name;toolSchema=tool.inputSchema||'{}';toolArguments='{}';toolError='';toolInventory=view.inventoryId;toolDialog.showModal();
  }
  function callTool():void {
    if(!threadScope || blocked || !view.current || !view.inventoryId || toolInventory!==view.inventoryId)return;
    let parsed:unknown;
    try{parsed=JSON.parse(toolArguments);}catch{toolError='Arguments must be valid JSON.';return;}
    if(!parsed || Array.isArray(parsed) || typeof parsed!=='object'){toolError='Arguments must be a JSON object.';return;}
    const server=view.servers.find(server=>server.name===toolServer);
    if(!server || !Object.hasOwn(server.tools,toolName)){toolError='The tool inventory changed. Refresh and confirm again.';return;}
    const inventory=view.inventoryId;toolInventory=null;toolDialog.close();
    onAction({kind:'call_tool',inventory_id:inventory,server:toolServer,tool:toolName,arguments:parsed as Record<string,unknown>});
  }
</script>

<details class="native-mcp">
  <summary>{threadScope?"Conversation MCP tools":"Codex MCP servers"}</summary>
  {#if threadScope}
    <p>Tools and resources reported for this native conversation. Resume its connection before refreshing or authorizing a service. Inspection sends no model prompt and does not call tools. Shared configuration reload remains in Codex Settings.</p>
    {#if view.threadId}<p>Scoped to the currently loaded native Codex conversation.</p>{/if}
  {:else}
    <p>Servers configured in the native Codex runtime. This is the configured inventory, not the tool set of a selected conversation. Listing may initialize configured servers; it does not send a model prompt or call their tools.</p>
  {/if}
  {#if !threadScope}<NativeMcpConfiguration configuration={view.configuration} {blocked} {onAction} />{/if}
  <div class="actions">
    <button type="button" disabled={blocked} onclick={()=>onAction({kind:"refresh"})}>{view.busy?"Loading…":"Refresh servers"}</button>
    {#if !threadScope}<button type="button" disabled={blocked || !view.current} onclick={reload}>Reload configuration…</button>{/if}
  </div>
  {#if view.error}<p role="alert">{view.error}</p>{/if}
  {#if view.notice}<p role="status">{view.notice}</p>{/if}
  {#if view.directResult}
    <section class:error-result={view.directResult.isError} aria-label="Direct MCP result">
      <strong>{view.directResult.operation==='tool'?'Tool result':'Resource contents'} · {view.directResult.server} / {view.directResult.name}</strong>
      {#each view.directResult.contents as content,index (`${content.kind}:${index}`)}
        <p>{content.mimeType||content.kind}</p><pre>{content.text}</pre>
      {/each}
      {#if view.directResult.structuredContent}<p>Structured content</p><pre>{view.directResult.structuredContent}</pre>{/if}
    </section>
  {/if}
  {#if view.login}
    <div class="login" role="status">
      <strong>Sign in to {view.login.name}</strong>
      <p>{view.login.origin?`Continue at ${view.login.origin}.` : "Waiting for the native authorization response."} This authorizes the MCP service, not your ChatGPT account.</p>
      <button type="button" disabled={!connected || !view.login.canOpen} onclick={()=>{if(view.login)onAction({kind:"open_login",attempt_id:view.login.attemptId});}}>Open authorization page</button>
      <p>Waiting for Codex to report completion. There is no client-imposed timeout or automatic retry. Reconnecting discards this client's pending state; it does not revoke authorization.</p>
    </div>
  {/if}
  {#if view.current && view.servers.length}
    <label class="filter">Filter servers, tools and resources
      <input type="search" bind:value={filter} placeholder="Name, description or status" />
    </label>
    {#if normalizedFilter}<p>{visibleServers.length} of {view.servers.length} servers match.</p>{/if}
  {/if}
  {#if !view.current}
    <p>{view.inventoryId?"Last observed inventory — refresh before making changes.":"Load the configured servers to inspect their status."}</p>
  {:else if view.servers.length === 0}
    <p>No MCP servers were reported by the native runtime. This does not enable Supervisor's custom tools.</p>
  {:else if visibleServers.length === 0}
    <p>No MCP server, tool or resource matches this filter.</p>
  {/if}
  {#each visibleServers as server (server.name)}
    {@const tools=toolsFor(server)}
    {@const resources=resourcesFor(server)}
    {@const templates=templatesFor(server)}
    <details class="server">
      <summary>{server.name}</summary>
      <p>Runtime: {mcpRuntimeLabel(server.runtimeStatus)} · Authentication: {mcpAuthLabel(server.authStatus)}</p>
      <p>{Object.keys(server.tools).length} tools · {server.resources.length} resources · {server.resourceTemplates.length} resource templates</p>
      {#if mcpCanLogin(server)}
        <p>Authorization is handled and stored by native Codex for this service; it can also be used by other Codex conversations or clients. It does not grant Supervisor system access.</p>
        <button type="button" disabled={blocked || !view.current} onclick={()=>authorize(server.name)}>Authorize OAuth…</button>
      {/if}
      {#if tools.length}
        <details><summary>Tools ({tools.length}{normalizedFilter && tools.length!==Object.keys(server.tools).length?` of ${Object.keys(server.tools).length}`:""})</summary><ul>
          {#each tools as tool (tool.name)}<li><strong>{tool.name}</strong>{#if tool.description}<p>{tool.description}</p>{/if}{#if threadScope}<button type="button" disabled={blocked || !view.current} onclick={()=>prepareTool(server.name,tool)}>Call tool…</button>{/if}</li>{/each}
        </ul></details>
      {/if}
      {#if resources.length}
        <details><summary>Resources ({resources.length}{normalizedFilter && resources.length!==server.resources.length?` of ${server.resources.length}`:""})</summary><ul>
          {#each resources as resource (resource.entryId||resource.name)}<li><strong>{resource.name}</strong>{#if resource.description}<p>{resource.description}</p>{/if}<button type="button" disabled={blocked || !view.current || !resource.entryId} onclick={()=>readResource(server.name,resource)}>Read resource</button></li>{/each}
        </ul></details>
      {/if}
      {#if templates.length}
        <details><summary>Resource templates ({templates.length}{normalizedFilter && templates.length!==server.resourceTemplates.length?` of ${server.resourceTemplates.length}`:""})</summary><ul>{#each templates as entry (entry.name)}<li><strong>{entry.name}</strong>{#if entry.description}<p>{entry.description}</p>{/if}</li>{/each}</ul></details>
      {/if}
    </details>
  {/each}
</details>
<dialog bind:this={dialog} class="workspace-confirm-dialog" aria-label="Reload native MCP configuration" onclose={closed}>
  <h2>Reload Codex MCP configuration?</h2>
  <p>Codex will reread configuration from disk and queue MCP refreshes for loaded threads. Configured servers may restart or reconnect to external services. This does not edit configuration files or install Supervisor tools.</p>
  <div class="workspace-confirm-actions"><button type="button" onclick={()=>dialog.close()}>Cancel</button><button type="button" onclick={confirmReload}>Reload configuration</button></div>
</dialog>
<dialog bind:this={toolDialog} class="workspace-confirm-dialog tool-call-dialog" aria-label="Call native MCP tool">
  <form onsubmit={event=>{event.preventDefault();callTool();}}>
    <h2>Call {toolServer} / {toolName}?</h2>
    <p>This directly invokes the selected native MCP tool in the current Codex conversation without a model prompt. It may read or change an external service. Native policy and approval callbacks still apply; disconnecting can leave the outcome uncertain.</p>
    <details><summary>Reported input schema</summary><pre>{toolSchema}</pre></details>
    <label>JSON arguments<textarea bind:value={toolArguments} rows={8} spellcheck="false"></textarea></label>
    {#if toolError}<p role="alert">{toolError}</p>{/if}
    <div class="workspace-confirm-actions"><button type="button" onclick={()=>toolDialog.close()}>Cancel</button><button type="submit" disabled={blocked || !view.current}>Call native tool</button></div>
  </form>
</dialog>

<style>
  .native-mcp { margin-block: var(--ca-space-3); font-size: var(--ca-type-body); line-height: var(--ca-leading-body); overflow-wrap: anywhere; }
  summary { cursor: pointer; font-size: var(--ca-type-label); }
  summary:focus-visible { box-shadow: var(--ca-focus-ring); }
  p { margin-block: var(--ca-space-2); color: var(--ca-muted); }
  p[role="alert"], strong { color: var(--ca-text); }
  .actions { display: flex; flex-wrap: wrap; gap: var(--ca-space-2); }
  .server, .login { margin-block: var(--ca-space-4); }
  .server details { margin-block: var(--ca-space-2); }
  ul { padding-inline-start: var(--ca-space-6); }
  li { margin-block: var(--ca-space-2); }
  dialog { border: none; color: var(--ca-text); background: var(--ca-surface); }
  section {margin-block:var(--ca-space-3);padding:var(--ca-space-2);background:var(--ca-surface);border-radius:var(--ca-radius-medium)}
  section.error-result {box-shadow:inset var(--ca-status-rail-width) 0 var(--ca-danger)}
  pre {max-height:18rem;overflow:auto;white-space:pre-wrap;overflow-wrap:anywhere;font:var(--ca-type-mono)/var(--ca-leading-body) var(--ca-font-mono);color:var(--ca-text)}
  label {display:grid;gap:var(--ca-space-2)}
  input,textarea {width:100%;box-sizing:border-box;border:0;border-radius:var(--ca-control-radius);padding:var(--ca-space-2);background:var(--ca-app-background);color:var(--ca-text);font:inherit}
  textarea {font:var(--ca-type-mono)/var(--ca-leading-body) var(--ca-font-mono)}
  .filter {margin-block:var(--ca-space-3)}
  .tool-call-dialog {
    box-sizing:border-box;
    inline-size:min(430px,calc(100vw - 32px));
    max-inline-size:calc(100vw - 32px);
    overflow-x:hidden;
  }
  .tool-call-dialog form {
    display:grid;
    inline-size:auto;
    max-inline-size:100%;
    min-inline-size:0;
    overflow-x:hidden;
  }
  .tool-call-dialog form > * {max-inline-size:100%;min-inline-size:0}
  .tool-call-dialog h2 {white-space:normal;overflow-wrap:anywhere;word-break:break-all}
  .tool-call-dialog .workspace-confirm-actions {flex-wrap:wrap}
</style>
