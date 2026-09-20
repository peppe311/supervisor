<script lang="ts">
  import type { McpAction, McpConfiguration, McpConfigurationEdit } from "../lib/app-server-mcp";
  import NativeMcpOptions from "./NativeMcpOptions.svelte";
  let {configuration, blocked, onAction}:{configuration?:McpConfiguration|null;blocked:boolean;onAction:(action:McpAction)=>void}=$props();
  let dialog:HTMLDialogElement;
  let name=$state("");
  let transport=$state("stdio");
  let command=$state("");
  let args=$state("[]");
  let cwd=$state("");
  let envVars=$state("");
  let url=$state("");
  let tokenEnv=$state("");
  let error=$state("");
  let pending=$state<{viewId:string;edit:McpConfigurationEdit}|null>(null);
  const editable=$derived(!blocked && !!configuration?.current && !!configuration.snapshot.target);
  $effect(()=>{
    if(pending && (!editable || pending.viewId!==configuration?.viewId)) {
      pending=null;dialog?.close();
    }
  });
  function review(edit:McpConfigurationEdit):void {
    if(!editable || !configuration)return;
    pending={viewId:configuration.viewId,edit:structuredClone(edit)};
    dialog.showModal();
  }
  function add(event:SubmitEvent):void {
    event.preventDefault();error="";
    if(!editable)return;
    try {
      let options:McpConfigurationEdit;
      if(transport==="stdio") {
        const values:unknown=JSON.parse(args);
        if(!Array.isArray(values)||!values.every(value=>typeof value==="string"))throw new Error("Arguments must be a JSON array of strings.");
        options={kind:"add",name:name.trim(),transport:{kind:"stdio",command:command.trim(),args:values,cwd:cwd.trim()||null,env_vars:envVars.split(",").map(value=>value.trim()).filter(Boolean)}};
      } else {
        options={kind:"add",name:name.trim(),transport:{kind:"http",url:url.trim(),bearer_token_env_var:tokenEnv.trim()||null}};
      }
      review(options);
    } catch(e) {error=e instanceof Error?e.message:"Check the server settings.";}
  }
  function confirm():void {
    const choice=pending;
    pending=null;dialog.close();
    if(!choice || !editable || configuration?.viewId!==choice.viewId)return;
    onAction({kind:"change_configuration",view_id:choice.viewId,edit:choice.edit});
  }
  function closed():void {
    // A queued close event can arrive after a new confirmation was opened.
    if(!dialog.open)pending=null;
  }
</script>

<details class="mcp-configuration">
  <summary>Manage configuration</summary>
  <p>Changes are saved to Supervisor's Codex configuration and can affect its other conversations. This list describes saved settings, not a running conversation's tools.</p>
  <button type="button" disabled={blocked} onclick={()=>onAction({kind:"inspect_configuration"})}>Refresh configuration</button>
  {#if configuration}
    {#if !configuration.current}<p>Last observed configuration — refresh before editing.</p>{/if}
    {#if configuration.snapshot.target}<p>Shared file <code>{configuration.snapshot.target.file}</code></p>
    {:else}<p>No editable base-user configuration was reported. Project and managed layers are not changed here.</p>{/if}
    {#each configuration.snapshot.servers as server (server.name)}
      <div class="server-setting">
        <strong>{server.name}</strong>
        <p>{server.transport} · {server.effectiveEnabled===true?"Enabled":server.effectiveEnabled===false?"Disabled":"Enabled state not reported"} · {server.inUserConfig?"User configuration":"Other configuration layer"}</p>
        {#if server.canEdit}
          <div class="actions">
            <button type="button" disabled={!editable} onclick={()=>review({kind:"set_enabled",name:server.name,enabled:true})}>Enable…</button>
            <button type="button" disabled={!editable} onclick={()=>review({kind:"set_enabled",name:server.name,enabled:false})}>Disable…</button>
            <button type="button" disabled={!editable} onclick={()=>review({kind:"remove",name:server.name})}>Remove user entry…</button>
          </div>
          <NativeMcpOptions {server} viewId={configuration.viewId} {editable} onReview={review} />
        {:else}<p>This entry is not editable here.</p>{/if}
      </div>
    {:else}<p>No MCP configuration entries were reported.</p>{/each}
    <details>
      <summary>Add a server</summary>
      <form onsubmit={add} autocomplete="off">
        <fieldset disabled={!editable}>
          <label>Server name<input bind:value={name} required pattern="[A-Za-z0-9_\-]+" /></label>
          <label>Transport<select bind:value={transport}><option value="stdio">STDIO — local process</option><option value="http">Streamable HTTP</option></select></label>
          {#if transport==="stdio"}
            <label>Executable<input bind:value={command} required /></label>
            <label>Arguments (JSON array)<textarea bind:value={args} rows="3" required spellcheck="false"></textarea></label>
            <label>Working directory (optional, absolute)<input bind:value={cwd} /></label>
            <label>Environment variable names (optional, comma-separated)<input bind:value={envVars} /></label>
          {:else}
            <label>MCP URL<input type="url" bind:value={url} required /></label>
            <label>Bearer token environment variable name (optional)<input bind:value={tokenEnv} /></label>
          {/if}
          <p>Use environment variable names for credentials. Do not paste tokens into commands, arguments or URLs. Codex owns OAuth; authorize it separately after configuration. New servers are saved disabled.</p>
          <button type="submit">Review disabled server…</button>
        </fieldset>
      </form>
    </details>
  {/if}
  {#if error}<p role="alert">{error}</p>{/if}
</details>

<dialog bind:this={dialog} class="workspace-confirm-dialog" aria-label="Confirm native MCP configuration change" onclose={closed}>
  {#if pending}
    <h2>{pending.edit.kind==="add"?"Save disabled MCP server?":pending.edit.kind==="remove"?"Remove this user entry?":pending.edit.kind==="set_option"?"Change this saved option?":pending.edit.kind==="clear_option"?"Clear this saved option?":pending.edit.enabled?"Enable this MCP server?":"Disable this MCP server?"}</h2>
    <p>Server <strong>{pending.edit.name}</strong></p>
    <p>File <code>{configuration?.snapshot.target?.file}</code></p>
    {#if pending.edit.kind==="add"}
      {#if pending.edit.transport.kind==="stdio"}<p>Executable <code>{pending.edit.transport.command}</code></p><pre>{JSON.stringify(pending.edit.transport.args,null,2)}</pre>
      {:else}<p>URL <code>{pending.edit.transport.url}</code></p>{/if}
      <p>This saves a disabled entry. Enable and reload separately after reviewing the server. Only run local servers and connect services you trust.</p>
    {:else if pending.edit.kind==="set_option" || pending.edit.kind==="clear_option"}
      <p>Option <code>{pending.edit.key}</code></p>
      {#if pending.edit.kind==="set_option"}<pre>{JSON.stringify(pending.edit.value,null,2)}</pre>
      {:else}<p>This removes only this saved user override. Codex determines the resulting default or inherited value; an empty list is not the same as removing a list.</p>{/if}
      <p>Other saved options, including credentials and headers, stay unchanged. If you change the command, URL or arguments, those settings may be used with the new process or destination. Only configure servers you trust. OAuth sign-in is not removed.</p>
    {:else if pending.edit.kind==="remove"}
      <p>This removes the server's entry from the shared user file, including its saved options. It does not remove project or managed entries, which may remain effective, and it does not sign out OAuth credentials.</p>
    {:else}<p>Codex clients can start enabled servers and expose their tools on later connection or refresh. Higher-priority configuration may override this setting.</p>{/if}
    <p>Saving does not reload running conversations. A conflicting file revision will be rejected; changes are never retried automatically.</p>
    <div class="workspace-confirm-actions"><button type="button" onclick={()=>dialog.close()}>Cancel</button><button type="button" disabled={!editable} onclick={confirm}>Save change</button></div>
  {/if}
</dialog>

<style>
  .mcp-configuration {margin-block:var(--ca-space-4);min-width:0;overflow-wrap:anywhere;}
  summary {cursor:pointer;font-size:var(--ca-type-label);}
  summary:focus-visible {box-shadow:var(--ca-focus-ring);}
  p {color:var(--ca-muted);margin-block:var(--ca-space-2);}
  .server-setting {margin-block:var(--ca-space-4);}
  .actions {display:flex;flex-wrap:wrap;gap:var(--ca-space-2);}
  fieldset {border:0;padding:0;display:grid;gap:var(--ca-space-3);min-width:0;}
  label {display:grid;gap:var(--ca-space-1);min-width:0;}
  input,select,textarea {width:100%;min-width:0;font:inherit;color:var(--ca-text);background:var(--ca-surface);}
  pre,code {font-family:var(--ca-font-mono);white-space:pre-wrap;overflow-wrap:anywhere;}
  dialog {border:none;color:var(--ca-text);background:var(--ca-surface);}
  p[role="alert"] {color:var(--ca-text);}
</style>
