<script lang="ts">
  import type { McpConfiguredServer, McpConfigurationEdit, McpOptionKey } from "../lib/app-server-mcp";
  import { optionsFor, optionValue } from "../lib/mcp-options";
  let {server,viewId,editable,onReview}:{server:McpConfiguredServer;viewId:string;editable:boolean;onReview:(edit:McpConfigurationEdit)=>void}=$props();
  let editing=$state(false), openedView=$state("");
  let key=$state<McpOptionKey>("command"), value=$state(""), error=$state("");
  const options=$derived(optionsFor(server.userTransport));
  const selected=$derived(options.find(option=>option.key===key));
  const current=$derived(editable && openedView===viewId && server.canEdit);
  function begin():void {
    if(!editable || !server.canEdit || !options.length)return;
    openedView=viewId;key=options[0].key;value="";error="";editing=true;
  }
  function review(clear:boolean):void {
    error="";
    if(!current || !selected)return;
    try {
      if(clear) {
        if(selected.required || !server.savedOptions?.includes(key))return;
        onReview({kind:"clear_option",name:server.name,key});
      } else onReview({kind:"set_option",name:server.name,key,value:optionValue(key,value)});
    } catch(e) {error=e instanceof Error?e.message:"Check the option value.";}
  }
</script>

{#if options.length}
  <button type="button" disabled={!editable} onclick={begin}>Edit connection options…</button>
  {#if editing}
    <form autocomplete="off" onsubmit={event=>{event.preventDefault();review(false);}}>
      <p>Editing {server.name}'s saved {server.userTransport} entry. Existing values are not displayed because they can contain credentials. Only the selected option changes.</p>
      {#if openedView!==viewId}<p role="status">Configuration changed. Reopen Edit connection options before saving.</p>{/if}
      <fieldset disabled={!current}>
        <label>Option<select bind:value={key} onchange={()=>{value="";error="";}}>{#each options as option}<option value={option.key}>{option.label}</option>{/each}</select></label>
        {#if selected}
          <p>{selected.help}</p>
          <p>{server.savedOptions?.includes(key)?"A user override is saved for this option.":"No user override is saved for this option."}</p>
          {#if selected.input==="boolean"}<label>New value<select bind:value><option value="" disabled>Select a value</option><option value="true">True</option><option value="false">False</option></select></label>
          {:else if selected.input==="json"}<label>New JSON value<textarea bind:value rows="3" spellcheck="false" required></textarea></label>
          {:else if selected.input==="number"}<label>New value<input type="text" inputmode="decimal" bind:value required /></label>
          {:else}<label>New value<input bind:value required spellcheck="false" /></label>{/if}
          <div class="actions"><button type="submit">Review option change…</button>{#if !selected.required && server.savedOptions?.includes(key)}<button type="button" onclick={()=>review(true)}>Clear saved option…</button>{/if}</div>
        {/if}
      </fieldset>
      <button type="button" onclick={()=>{editing=false;value="";error="";}}>Close editor</button>
      {#if error}<p role="alert">{error}</p>{/if}
    </form>
  {/if}
{/if}

<style>
  form {margin-block:var(--ca-space-3);min-width:0;}
  fieldset {border:0;padding:0;display:grid;gap:var(--ca-space-2);min-width:0;}
  label {display:grid;gap:var(--ca-space-1);min-width:0;}
  p {color:var(--ca-muted);margin-block:var(--ca-space-2);overflow-wrap:anywhere;font:var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body);}
  input,select,textarea {width:100%;min-width:0;box-sizing:border-box;}
  button,input,select,textarea {font:inherit;color:var(--ca-text);background:var(--ca-app-background);border:0;border-radius:var(--ca-control-radius);padding:var(--ca-space-2);}
  button:focus-visible,input:focus-visible,select:focus-visible,textarea:focus-visible {box-shadow:var(--ca-focus-ring);}
  button:disabled {color:var(--ca-muted);cursor:not-allowed;}
  .actions {display:flex;flex-wrap:wrap;gap:var(--ca-space-2);}
</style>
