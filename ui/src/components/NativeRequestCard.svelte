<script lang="ts">
  import { untrack } from "svelte";
  import DiffViewer from "./DiffViewer.svelte";
  import { extendedFormMode, formFields, formAnswer, requestPresentation, schemaFormAnswer, schemaFormTemplate, type NativeRequest } from "../lib/native-requests";
  let {request,onaction}:{request:NativeRequest;onaction:(action:Record<string,unknown>)=>void}=$props();
  let error=$state("");
  const p=$derived(request.params);
  const choices=$derived(request.choices);
  // A card is keyed by the immutable native ticket. Snapshot its field schema
  // once so unrelated state snapshots cannot reapply defaults over typed input.
  const fields=untrack(()=>formFields(request.params.requestedSchema));
  let schemaContent=$state(untrack(()=>schemaFormTemplate(request.params.requestedSchema)));
  const presentation=$derived(requestPresentation(request));
  function answer(decision:Record<string,unknown>):void { onaction({kind:"answer",ticket:request.ticket,decision}); }
  function chooseAnswer(event:MouseEvent,id:string,label:string):void {
    const field=(event.currentTarget as HTMLButtonElement).form?.elements.namedItem(id);
    if(field instanceof HTMLInputElement || field instanceof HTMLTextAreaElement) {field.value=label;field.focus();}
  }
  function submit(event:SubmitEvent):void {
    event.preventDefault();error="";
    const data=new FormData(event.currentTarget as HTMLFormElement);
    if(request.kind==="questions") {
      const answers=Object.fromEntries((p.questions || []).map(q=>[q.id,[String(data.get(q.id) || "")]]));
      answer({kind:"answers",answers});
    } else {
      try {
        const content=extendedFormMode(p.mode) && !fields ? schemaFormAnswer(schemaContent) : formAnswer(fields || [],data);
        answer({kind:"elicitation",action:"accept",content});
      }
      catch(e) { error=e instanceof Error?e.message:"Check the requested fields."; }
    }
  }
</script>

<article aria-label={presentation.title} aria-busy={request.responding}>
  <h3>{presentation.title}</h3>
  {#if presentation.explanation}<p>{presentation.explanation}</p>{/if}
  {#if presentation.network}
    <p>Destination <code>{presentation.network.host}</code></p>
    <p>Protocol <code>{presentation.network.protocol}</code></p>
  {/if}
  {#if p.serverName}<p>MCP server: <strong>{p.serverName}</strong></p>{/if}
  {#if p.reason || p.message}<p>{p.reason || p.message}</p>{/if}
  {#if p.cwd}<p>Directory <code>{p.cwd}</code></p>{/if}
  {#if p.environmentId}<p>Environment <code>{p.environmentId}</code></p>{/if}
  {#if p.command}
    {#if presentation.commandContext}<details><summary>Reported command context</summary><pre>{p.command}</pre></details>
    {:else}<pre>{p.command}</pre>{/if}
  {/if}
  {#if p.grantRoot}<p>Requested write root <code>{p.grantRoot}</code></p>{/if}
  {#if request.kind==="files"}
    {#each request.item?.changes || [] as change (change.path)}
      <details><summary>{change.path}</summary><DiffViewer content={change.diff} identity={`${request.ticket}:${change.path}`} inline /></details>
    {:else}<p>Codex has not supplied a file preview for this request.</p>{/each}
  {/if}
  {#if request.kind==="command" || request.kind==="files"}
    <div class="actions">
      {#if choices?.accept}<button type="button" disabled={request.responding} onclick={()=>answer({kind:"accept"})}>Allow once</button>{/if}
      {#if choices?.decline}<button type="button" disabled={request.responding} onclick={()=>answer({kind:"decline"})}>Decline</button>{/if}
      {#if choices?.cancel}<button type="button" disabled={request.responding} onclick={()=>answer({kind:"cancel"})}>Cancel request</button>{/if}
    </div>
    {#if choices?.session || choices?.execPolicy || choices?.networkPolicies.length}
    <details><summary>Session and policy choices</summary>
      <p>Session approval applies to the scope defined by Codex until this native session ends. A proposed policy amendment can affect future requests.</p>
      {#if choices?.session}<button type="button" disabled={request.responding} onclick={()=>answer({kind:"session"})}>Allow for session</button>{/if}
      {#if choices?.execPolicy}
        <pre>{JSON.stringify(p.proposedExecpolicyAmendment,null,2)}</pre>
        <button type="button" disabled={request.responding} onclick={()=>answer({kind:"exec_policy"})}>Accept proposed command policy</button>
      {/if}
      {#each p.proposedNetworkPolicyAmendments || [] as policy,index}
        {#if choices?.networkPolicies.includes(index)}
        <pre>{JSON.stringify(policy,null,2)}</pre>
        <button type="button" disabled={request.responding} onclick={()=>answer({kind:"network_policy",index})}>Apply this network policy</button>
        {/if}
      {/each}
    </details>
    {:else if !choices?.accept && !choices?.decline && !choices?.cancel}
      <p>Codex did not offer a supported decision for this request. You can stop the turn.</p>
    {/if}
  {:else if request.kind==="permissions"}
    <pre>{JSON.stringify(p.permissions,null,2)}</pre>
    <p>Only the permissions shown above will be granted. No other paths or network access are added.</p>
    <div class="actions">
      <button type="button" disabled={request.responding} onclick={()=>answer({kind:"permissions",grant:true,session:false})}>Grant for this turn</button>
      <button type="button" disabled={request.responding} onclick={()=>answer({kind:"permissions",grant:false,session:false})}>Deny permissions</button>
    </div>
    <details><summary>Longer permission scope</summary><button type="button" disabled={request.responding} onclick={()=>answer({kind:"permissions",grant:true,session:true})}>Grant for this session</button></details>
  {:else if request.kind==="questions"}
    <form onsubmit={submit} autocomplete="off">
      <fieldset disabled={request.responding}>
        {#each p.questions || [] as question (question.id)}
          <label>{question.question}
            {#if question.isSecret}<input type="password" name={question.id} required autocomplete="off" />
            {:else}<textarea name={question.id} required rows="2"></textarea>{/if}
          </label>
          {#if question.options?.length}
            <details open><summary>Suggested answers</summary>
              {#each question.options as option}<p><button type="button" onclick={(event)=>chooseAnswer(event,question.id,option.label)}>{option.label}</button> — {option.description}</p>{/each}
            </details>
          {/if}
        {/each}
        <div class="actions"><button type="submit">Send answers</button><button type="button" onclick={()=>answer({kind:"cancel"})}>Dismiss questions</button></div>
      </fieldset>
    </form>
  {:else if request.kind==="mcp"}
    {#if p.mode==="url"}
      <p>The external page is requested by this MCP server. Check the address before opening it; opening it does not approve the request.</p>
      <pre>{p.url}</pre>
      <div class="actions">
        <button type="button" disabled={request.responding} onclick={()=>onaction({kind:"open_url",ticket:request.ticket})}>Open authorization page</button>
        <button type="button" disabled={request.responding} onclick={()=>answer({kind:"elicitation",action:"accept",content:null})}>I have completed the external step</button>
      </div>
    {:else if fields}
      <form onsubmit={submit} autocomplete="off">
        <fieldset disabled={request.responding}>
          {#each fields as field (field.key)}
            <label>{field.title}{field.required ? " (required)" : ""}
              {#if field.description}<span>{field.description}</span>{/if}
              {#if field.options.length}
                <select name={field.key} required={field.required} multiple={field.type==="array"} value={field.initial}>
                  {#if field.type!=="array"}<option value="">Select…</option>{/if}
                  {#each field.options as option}<option value={option.value}>{option.label}</option>{/each}
                </select>
              {:else if field.type==="boolean"}
                <select name={field.key} required={field.required} value={field.initial===undefined?"":String(field.initial)}><option value="">Select…</option><option value="true">Yes</option><option value="false">No</option></select>
              {:else if field.type==="number" || field.type==="integer"}
                <input name={field.key} type="number" required={field.required} min={field.min} max={field.max} step={field.type==="integer"?1:"any"} value={field.initial} />
              {:else}
                <input name={field.key} type={field.format==="email"?"email":field.format==="uri"?"url":"text"} required={field.required} minlength={field.minLength} maxlength={field.maxLength} value={field.initial} />
              {/if}
            </label>
          {/each}
          {#if error}<p role="alert">{error}</p>{/if}
          <button type="submit">Send requested information</button>
        </fieldset>
      </form>
    {:else if extendedFormMode(p.mode)}
      <form onsubmit={submit} autocomplete="off">
        <fieldset disabled={request.responding}>
          <p>This extended form contains nested structured fields. Edit the generated JSON object; Supervisor validates every property, type and bound against the sanitized native schema before sending it.</p>
          <label>Structured response<textarea class="schema-form" bind:value={schemaContent} rows="10" spellcheck="false" required></textarea></label>
          {#if error}<p role="alert">{error}</p>{/if}
          <button type="submit">Validate and send information</button>
        </fieldset>
      </form>
    {:else}<p>This form cannot be displayed safely by this version. You can decline or cancel it.</p>{/if}
    <div class="actions">
      <button type="button" disabled={request.responding} onclick={()=>answer({kind:"elicitation",action:"decline",content:null})}>Decline</button>
      <button type="button" disabled={request.responding} onclick={()=>answer({kind:"elicitation",action:"cancel",content:null})}>Cancel request</button>
    </div>
  {/if}
  {#if request.responding}<p role="status">Sending your decision…</p>{/if}
</article>

<style>
  article { min-width:0; padding:var(--ca-space-3); font:inherit; color:var(--ca-text); background:var(--ca-bg); border-radius:var(--ca-radius-large); }
  h3 { margin:0 0 var(--ca-space-2); font-size:var(--ca-type-body); font-weight:600; }
  p,details,form { margin:var(--ca-space-2) 0; }
  p,summary,label { overflow-wrap:anywhere; }
  pre,code { font-family:var(--ca-font-mono); font-size:var(--ca-type-body); white-space:pre-wrap; overflow-wrap:anywhere; }
  .actions { display:flex; flex-wrap:wrap; gap:var(--ca-space-2); margin:var(--ca-space-2) 0; }
  button,input,select,textarea { color:var(--ca-text); background:var(--ca-surface); border:0; border-radius:var(--ca-control-radius); padding:var(--ca-space-2); font:inherit; }
  button { cursor:pointer; }
  button:disabled { cursor:default; color:var(--ca-muted); }
  button:focus-visible,input:focus-visible,select:focus-visible,textarea:focus-visible,summary:focus-visible { box-shadow:var(--ca-focus-ring); }
  label { display:flex; flex-direction:column; gap:var(--ca-space-2); margin:var(--ca-space-3) 0; }
  label span { font-size:var(--ca-type-caption); }
  input,select,textarea { min-width:0; max-width:100%; }
  .schema-form { width:100%; box-sizing:border-box; font-family:var(--ca-font-mono); resize:vertical; }
  fieldset { min-width:0; padding:0; margin:0; border:0; }
</style>
