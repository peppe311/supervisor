<script lang="ts">
  import {deliveryMode, deliveryTime, type DeliveryTiming} from '../lib/native-event-log';
  let {samples}:{samples:DeliveryTiming[]}=$props();
</script>
<section class="delivery-timings" aria-label="Prompt delivery timings">
  <p class="title">Prompt delivery</p>
  <p>Elapsed time from receipt in Supervisor, including any prompt queue wait. Click to app is a separate clock estimate. Latest eight samples; timings are kept only for this session.</p>
  {#if !samples.length}<p>Send a prompt, then refresh to inspect its delivery.</p>{/if}
  <ol>
    {#each samples.slice(-8).reverse() as sample (sample.messageId)}
      <li><details>
        <summary>{deliveryMode(sample.mode)} · {sample.outcome}<span>Sent to Codex: {deliveryTime(sample.writtenMs)}</span></summary>
        <dl>
          <dt>Click to app (estimate)</dt><dd>{deliveryTime(sample.clientToHostMs)}</dd>
          <dt>Input prepared</dt><dd>{deliveryTime(sample.preparedMs)}</dd>
          <dt>Written to Codex</dt><dd>{deliveryTime(sample.writtenMs)}</dd>
          <dt>Codex acknowledged</dt><dd>{deliveryTime(sample.acknowledgedMs)}</dd>
          <dt>Native prompt observed</dt><dd>{deliveryTime(sample.nativeInputMs)}</dd>
          <dt>First text or reasoning summary</dt><dd>{deliveryTime(sample.firstUpdateMs)}</dd>
        </dl>
        {#if sample.mode==='steer'}<p>For a follow-up, the first update belongs to the ongoing turn; it does not prove the new instruction has been processed.</p>{/if}
      </details></li>
    {/each}
  </ol>
</section>
<style>
  .delivery-timings {min-width:0;margin-block:var(--ca-space-3);}
  p {margin:var(--ca-space-2) 0;color:var(--ca-muted);overflow-wrap:anywhere;}
  .title {font-weight:600;color:var(--ca-text);}
  ol {list-style:none;padding:0;margin:0;max-height:40vh;overflow:auto;overscroll-behavior:contain;}
  li {border-bottom:1px solid var(--ca-border);}
  summary {cursor:pointer;padding:var(--ca-space-2);border-radius:var(--ca-control-radius);}
  summary:hover {background:var(--ca-surface-2);}
  summary:focus-visible {outline:none;box-shadow:var(--ca-focus-ring);}
  summary span {display:block;color:var(--ca-muted);padding-top:var(--ca-space-1);}
  dl {display:grid;grid-template-columns:minmax(0,1fr) auto;gap:var(--ca-space-2);margin:var(--ca-space-2);}
  dt {color:var(--ca-muted);overflow-wrap:anywhere;}
  dd {margin:0;font-variant-numeric:tabular-nums;}
</style>
