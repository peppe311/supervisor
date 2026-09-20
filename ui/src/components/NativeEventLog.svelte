<script lang="ts">
  import {onMount} from 'svelte';
  import DeliveryTimings from './DeliveryTimings.svelte';
  import type {NativeConversationView} from '../lib/native-history';
  import {eventMeaning,eventTime,type NativeEventLog} from '../lib/native-event-log';
  let {owner,eventTarget,conversation}:{owner:string;eventTarget?:HTMLElement;conversation:NativeConversationView|null}=$props();
  let log=$state<NativeEventLog|null>(null);
  onMount(()=>{
    const update=(event:Event)=>{
      const detail=(event as CustomEvent).detail;
      if(detail?.owner===owner && detail.threadId===conversation?.binding?.threadId && detail.log)log=detail.log;
    };
    window.addEventListener('central-agent:native-event-log',update);
    eventTarget?.addEventListener('central-agent:native-event-log',update);
    return ()=>{window.removeEventListener('central-agent:native-event-log',update);eventTarget?.removeEventListener('central-agent:native-event-log',update);};
  });
  function refresh():void {
    if(!conversation?.binding)return;
    (eventTarget||document).dispatchEvent(new CustomEvent('central-agent:app-server-conversation-control',{bubbles:true,detail:{owner,action:{kind:'event_log',expected_thread_id:conversation.binding.threadId}}}));
  }
</script>
<details class="native-event-log" data-owner={owner} data-thread-id={conversation?.binding?.threadId} ontoggle={event=>{if(event.currentTarget.open)refresh();}}>
  <summary>Event diagnostics</summary>
  <p>Observed order in Supervisor. Local receive times; acknowledgement delivery can follow activity. This view contains identifiers and states only.</p>
  <button type="button" onclick={refresh}>Refresh event log</button>
  {#if log}
    <DeliveryTimings samples={log.deliveryTimings || []} />
    <p>{log.persistent?'Saved locally across restarts.':'Available in this session.'} Latest {log.entries.length} events for this conversation; up to {log.retainedLimit} recent events retained across the app.</p>
    {#if log.error}<p role="alert">{log.error}. New events remain available during this session.</p>{/if}
    {#if !log.entries.length}<p>No events recorded for this conversation yet.</p>{/if}
    <ol aria-label="Conversation event log">
      {#each log.entries as entry (entry.sequence)}
        <li>
          <details>
            <summary><span class="event-title">{entry.sequence} · {eventMeaning(entry)}</span><span class="event-state">{eventTime(entry)}{entry.state?` · ${entry.state}`:''}</span></summary>
            <dl>
              <dt>Method</dt><dd>{entry.method}</dd>
              <dt>Direction</dt><dd>{entry.direction}</dd>
              <dt>Connection</dt><dd>{entry.connectionEpoch}</dd>
              {#if entry.threadId}<dt>Thread</dt><dd>{entry.threadId}</dd>{/if}
              {#if entry.turnId}<dt>Turn</dt><dd>{entry.turnId}</dd>{/if}
              {#if entry.itemId}<dt>Item</dt><dd>{entry.itemId}</dd>{/if}
              {#if entry.requestId}<dt>{entry.requestId.startsWith('host:')?'Host correlation':'Native request'}</dt><dd>{entry.requestId}</dd>{/if}
              {#if entry.summaryIndex!==null}<dt>Summary part</dt><dd>{entry.summaryIndex}</dd>{/if}
              {#if entry.deltaBytes!==null}<dt>Update size</dt><dd>{entry.deltaBytes} bytes</dd>{/if}
            </dl>
          </details>
        </li>
      {/each}
    </ol>
  {/if}
</details>
<style>
  .native-event-log {min-width:0;color:var(--ca-text);font:var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body);}
  summary,button {cursor:pointer;border:0;border-radius:var(--ca-control-radius);padding:var(--ca-space-2);font:inherit;color:inherit;background:transparent;}
  button {background:var(--ca-surface-2);}
  summary:focus-visible,button:focus-visible {outline:none;box-shadow:var(--ca-focus-ring);}
  p,.event-state {color:var(--ca-muted);}
  p {margin:var(--ca-space-2) 0;overflow-wrap:anywhere;}
  ol {padding:0;margin:var(--ca-space-2) 0;list-style:none;max-height:40vh;overflow:auto;overscroll-behavior:contain;}
  li {border-bottom:1px solid var(--ca-border);min-width:0;}
  .event-title {font-weight:600;}
  .event-state {display:block;padding-top:var(--ca-space-1);}
  dl {display:grid;gap:var(--ca-space-1);margin:var(--ca-space-2);min-width:0;}
  dt {color:var(--ca-muted);}
  dd {margin:0 0 var(--ca-space-2);font-family:var(--ca-font-mono);overflow-wrap:anywhere;}
</style>
