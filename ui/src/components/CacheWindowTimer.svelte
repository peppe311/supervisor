<script lang="ts">
  import { onMount } from "svelte";
  import { cacheWindowEstimate } from "../lib/cache-window";
  import { usageSnapshot, type NativeUsage } from "../lib/native-usage";

  let { owner:fixedOwner, eventTarget }: {owner?:string;eventTarget?:HTMLElement}=$props();
  let owner="";
  let usage=$state<NativeUsage|null>(null);
  let now=$state(Date.now());
  const estimate=$derived(cacheWindowEstimate(usage,now));

  onMount(()=>{
    owner=fixedOwner || "";
    const switchOwner=(event:Event)=>{
      const next=String((event as CustomEvent).detail||"");
      if(!fixedOwner && next!==owner){owner=next;usage=null;}
    };
    const update=(event:Event)=>{
      const next=usageSnapshot((event as CustomEvent).detail,owner);
      if(next){usage=next;now=Date.now();}
    };
    const target=eventTarget || window;
    window.addEventListener("central-agent:conversation-key",switchOwner);
    target.addEventListener("central-agent:conversation-state",update);
    const timer=window.setInterval(()=>{if(estimate && !estimate.elapsed)now=Date.now();},1000);
    return ()=>{
      window.clearInterval(timer);
      window.removeEventListener("central-agent:conversation-key",switchOwner);
      target.removeEventListener("central-agent:conversation-state",update);
    };
  });
</script>

{#if estimate}
  <span class="cache-window-timer" data-cache-window={estimate.elapsed ? "elapsed" : "estimated"}
    role="timer" aria-live="off"
    aria-label={estimate.elapsed ? "Last confirmed cache activity was over 30 minutes ago" : `Estimated cache window ${estimate.label} remaining`}
    title={estimate.description}>Cache <strong>{estimate.elapsed ? estimate.label : `~${estimate.label}`}</strong></span>
{/if}

<style>
  .cache-window-timer {display:inline-flex;flex:0 0 auto;align-items:baseline;gap:var(--ca-space-1);margin-inline-start:auto;min-width:0;white-space:nowrap;color:var(--ca-muted);font:var(--ca-type-caption)/var(--ca-leading-compact) var(--ca-font-body);font-variant-numeric:tabular-nums;}
  strong {color:var(--ca-text);font-weight:600;}
  [data-cache-window="elapsed"] strong {color:var(--ca-muted);}
</style>
