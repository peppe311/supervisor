<script lang="ts">
  import { onMount } from "svelte";
  import { usageSnapshot, usagePresentation, formatUsage, usageCounters, type NativeUsage } from "../lib/native-usage";
  let { owner:fixedOwner, eventTarget, visible=$bindable(false), compact=false }: {owner?:string;eventTarget?:HTMLElement;visible?:boolean;compact?:boolean}=$props();
  let owner="";
  let usage=$state<NativeUsage|null>(null);
  const presentation=$derived(usagePresentation(usage));
  const compactValue=$derived(presentation.percent ?? (usage?.report?.last.totalTokens != null ? formatUsage(usage.report.last.totalTokens) : "—"));
  onMount(()=>{
    owner=fixedOwner || "";
    const switchOwner=(event:Event)=>{const next=String((event as CustomEvent).detail||"");if(!fixedOwner && next!==owner){owner=next;usage=null;visible=false;}};
    const update=(event:Event)=>{const next=usageSnapshot((event as CustomEvent).detail,owner);if(next){usage=next;visible=next.visible;}};
    window.addEventListener("central-agent:conversation-key",switchOwner);
    const target=eventTarget || window;
    target.addEventListener("central-agent:conversation-state",update);
    return ()=>{window.removeEventListener("central-agent:conversation-key",switchOwner);target.removeEventListener("central-agent:conversation-state",update);};
  });
</script>
{#if visible}
  <div class="native-usage" class:compact data-native-usage={compact ? "compact" : "full"} data-stale={presentation.stale} title={compact ? presentation.status : undefined}>
    {#if compact}
      <div class="usage-head" role="status" aria-label={`Context ${compactValue}. ${presentation.status}`}><span>Context</span><strong>{compactValue}</strong></div>
      {#if presentation.meter !== null}
        <div class="usage-meter" role="progressbar" aria-label="Latest reported tokens relative to native context capacity" aria-valuemin="0" aria-valuemax="100" aria-valuenow={presentation.meter} aria-valuetext={`${presentation.summary}. ${presentation.status}`}><span style:width={`${presentation.meter}%`}></span></div>
      {/if}
    {:else}
      <div class="usage-head"><span>Context · native report</span><strong>{presentation.summary}</strong></div>
      {#if presentation.meter !== null}
        <div class="usage-meter" role="progressbar" aria-label="Latest reported tokens relative to native context capacity" aria-valuemin="0" aria-valuemax="100" aria-valuenow={presentation.meter} aria-valuetext={`${presentation.summary}. ${presentation.status}`}><span style:width={`${presentation.meter}%`}></span></div>
      {/if}
      <p>{presentation.status}</p>
      {#if usage?.report}
        <details>
          <summary>Token details · thread total {formatUsage(usage.report.total.totalTokens)}</summary>
          <div class="usage-table"><table><caption>Native Codex token counters</caption><thead><tr><th scope="col">Metric</th><th scope="col">Latest report</th><th scope="col">Thread total</th></tr></thead><tbody>
            {#each usageCounters as [key,label]}<tr><th scope="row">{label}</th><td>{formatUsage(usage.report.last[key])}</td><td>{formatUsage(usage.report.total[key])}</td></tr>{/each}
          </tbody></table></div>
          <p>Reported context capacity: {formatUsage(usage.report.modelContextWindow)}. The comparison uses the latest report, not cumulative usage. Cached and reasoning counters are shown separately and are not added again. No billing amount is inferred.</p>
        </details>
      {/if}
    {/if}
  </div>
{/if}
<style>
  .native-usage {display:grid;gap:var(--ca-space-2);min-width:0;color:var(--ca-text);font:var(--ca-type-caption)/var(--ca-leading-body) var(--ca-font-body);}
  .usage-head {display:flex;flex-wrap:wrap;justify-content:space-between;gap:var(--ca-space-2);}
  .native-usage.compact {flex:1 1 7rem;width:min(8rem,32%);max-width:8rem;gap:var(--ca-space-1);align-self:center;}
  .compact .usage-head {flex-wrap:nowrap;align-items:baseline;gap:var(--ca-space-1);color:var(--ca-muted);white-space:nowrap;}
  .compact .usage-head strong {margin-left:auto;color:var(--ca-text);}
  strong {font-weight:600;font-variant-numeric:tabular-nums;overflow-wrap:anywhere;}
  p {margin:0;color:var(--ca-muted);}
  .usage-meter {height:var(--ca-space-1);background:var(--ca-surface);border-radius:var(--ca-radius-small);overflow:hidden;}
  .usage-meter span {display:block;height:100%;background:var(--ca-text);}
  [data-stale="true"] .usage-meter span {background:var(--ca-muted);}
  summary {cursor:pointer;overflow-wrap:anywhere;}
  summary:focus-visible {box-shadow:var(--ca-focus-ring);}
  .usage-table {overflow:auto;min-width:0;margin-block:var(--ca-space-2);}
  table {width:100%;border-collapse:collapse;font:inherit;}
  caption {text-align:left;color:var(--ca-muted);}
  th,td {text-align:left;padding:var(--ca-space-1);font-variant-numeric:tabular-nums;}
  td {text-align:right;}
</style>
