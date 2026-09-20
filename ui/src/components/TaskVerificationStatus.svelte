<script lang="ts">
  import {taskVerificationLabel,type TaskVerification} from '../lib/task-verification';

  let {owner,verification}:{owner:string;verification:TaskVerification|null|undefined}=$props();
  let trigger=$state<HTMLButtonElement>();
  let top=$state(0),left=$state(0);
  const panelId=$derived(`task-verification-${owner.replace(/[^a-z0-9_-]/gi,'-')}`);
  const label=$derived(verification?taskVerificationLabel(verification.state):'');

  function place():void {
    if(!trigger)return;
    const bounds=trigger.getBoundingClientRect();
    const panelWidth=Math.min(300,Math.max(220,window.innerWidth-24));
    left=Math.max(12,Math.min(window.innerWidth-panelWidth-12,bounds.right-panelWidth));
    top=Math.max(12,Math.min(window.innerHeight-150,bounds.bottom+8));
  }
</script>

{#if verification}
  <button bind:this={trigger} class="verification-trigger" class:blocked={verification.state==='blocked'} type="button"
    popovertarget={panelId} onclick={place}
    aria-label={`${label}: ${verification.summary}`} data-task-verification={verification.state} data-verification-owner={owner}>
    <span class="verification-mark" aria-hidden="true"></span><span>{label}</span>
  </button>
  <div class="verification-panel" id={panelId} popover="auto" role="dialog" aria-label={`${label} details`}
    style:top={`${top}px`} style:left={`${left}px`}>
    <strong>{label}</strong>
    <p>{verification.summary}</p>
  </div>
{/if}

<style>
  .verification-trigger{display:inline-flex;min-width:0;min-height:var(--ca-control-compact);align-items:center;gap:var(--ca-space-1);padding:0 var(--ca-space-2);border:0;border-radius:var(--ca-control-radius);background:var(--ca-surface-2);color:var(--ca-muted);font:600 var(--ca-type-caption)/var(--ca-leading-compact) var(--ca-font-body);white-space:nowrap;cursor:pointer}
  .verification-trigger:hover,.verification-trigger:focus-visible,.verification-trigger.blocked{background:var(--ca-surface-3);color:var(--ca-text)}
  .verification-trigger:focus-visible{outline:none;box-shadow:var(--ca-focus-ring)}
  .verification-mark{width:var(--ca-space-1);height:var(--ca-space-1);border-radius:var(--ca-radius-pill);background:currentColor;flex:0 0 auto}
  .verification-panel{position:fixed;inset:auto;box-sizing:border-box;width:min(300px,calc(100vw - var(--ca-space-6)));max-height:min(240px,calc(100vh - var(--ca-space-6)));overflow:auto;margin:0;padding:var(--ca-space-3);border:0;border-radius:var(--ca-radius-large);background:var(--ca-surface);color:var(--ca-text);box-shadow:var(--ca-shadow-float)}
  .verification-panel::backdrop{background:transparent}
  .verification-panel strong{display:block;margin-bottom:var(--ca-space-2);font-weight:650}
  .verification-panel p{margin:0;color:var(--ca-muted);font-size:var(--ca-type-label);line-height:var(--ca-leading-body);overflow-wrap:anywhere}
</style>
