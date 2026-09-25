<script lang="ts">
  import { flushSync, onMount, setContext } from "svelte";
  import AgentGraphHeader from "./AgentGraphHeader.svelte";
  import ProjectDiff from "./ProjectDiff.svelte";
  import NativeCommands from "./NativeCommands.svelte";
  import {consumeNativeCommand} from "../lib/native-commands";
  import GraphTimeline from "./GraphTimeline.svelte";
  import NativeRequests from "./NativeRequests.svelte";
  import NativeAccess from "./NativeAccess.svelte";
  import NativeConversation from "./NativeConversation.svelte";
  import PluginPicker from "./PluginPicker.svelte";
  import LiveDiffStats from "./LiveDiffStats.svelte";
  import CacheWindowTimer from "./CacheWindowTimer.svelte";
  import FileAttachments from "./FileAttachments.svelte";
  import TaskVerificationStatus from "./TaskVerificationStatus.svelte";
  import type {TaskVerification} from "../lib/task-verification";
  import GraphContexts from "./GraphContexts.svelte";
  import { contextDraft, contextIds, type ContextDraft } from "../lib/graph-contexts";
  import { attachmentList, type FileAttachment } from "../lib/file-attachments";
  import { acceptsComposerReceipt } from "../lib/composer-receipt";
  import { bridgeConversationEvents, graphDraft, graphDraftStatus, retainGraphDraft, graphDeliveryIntent } from "../lib/conversation-events";
  import { animateSendControl, armPromptSendMotion } from "../lib/prompt-send-motion";
  let { eventTarget, owner, title="" }: { eventTarget: HTMLElement; owner: string; title?:string } = $props();
  setContext("central-agent:compact-configuration", true);
  const inputId = $derived(`graph-input-${owner}`);
  let input: HTMLTextAreaElement;
  let sendControl: HTMLDivElement;
  let canSubmit = $state(false), pending = $state(false);
  let blockedReason = $state("");
  const availabilityId = $derived(`graph-availability-${owner}`);
  let canStop = $state(false), canResume = $state(false), stopping = $state(false);
  let draftText = $state("");
  let active = $state(false), supportsSteer = $state(false), queued = $state(0);
  let minimized = $state(false);
  let focused = $state(false);
  let fileDragOver = $state(false);
  type SupervisionState = {
    targetOwner:string; title:string; run:unknown; active:boolean; supportsSteer:boolean;
    nativeTurnId:string; pendingRequests:number; monitoring:boolean; verification:TaskVerification|null; hooks:unknown;
  };
  let supervision = $state<SupervisionState|null>(null);
  let timelineScope = $state<"supervisor"|"observed">("supervisor");
  function observingAgent():boolean { return timelineScope === "observed" && supervision !== null; }
  function canSteerAgent():boolean { return Boolean(supervision?.supportsSteer && supervision.nativeTurnId && input?.value.trim()); }
  function toggleMinimized():void {
    minimized=!minimized;
    eventTarget.dataset.cardMinimized=String(minimized);
    eventTarget.dispatchEvent(new CustomEvent('central-agent:board-card-layout',{bubbles:true,detail:{owner,minimized}}));
  }
  function toggleFocused():void {
    eventTarget.dispatchEvent(new CustomEvent('central-agent:board-card-focus',{bubbles:true,detail:{owner}}));
  }
  const steeringObserved = $derived(observingAgent());
  let deliveryPending = $state(false);
  let nativeTurnId = "", deliveryTurnId = "";
  let files = $state<FileAttachment[]>([]), filesEnabled = $state(false), fileError = $state("");
  let contexts = $state<ContextDraft>({tabs:[],shells:[]});
  const hasPayload = $derived(steeringObserved
    ? Boolean(draftText.trim())
    : Boolean(draftText.trim() || files.length || contexts.tabs.length || contexts.shells.length));
  const primaryAction = $derived(canResume && !hasPayload ? "resume" : "send");
  const observedBlockedReason = $derived(!steeringObserved ? ""
    : !supervision?.active ? "The observed agent has no active turn to steer."
    : !supervision?.supportsSteer || !supervision.nativeTurnId ? "The active agent cannot be steered from this conversation."
    : "");
  const visibleBlockedReason = $derived(steeringObserved ? observedBlockedReason : blockedReason);
  const actionEnabled = $derived(steeringObserved
    ? Boolean(supervision?.supportsSteer && supervision.nativeTurnId && draftText.trim())
    : canSubmit);
  let draftSaveError = $state("");
  let draftSaveStatus = $state("loading");
  let draftPersistent = false;
  function manageFiles(action:Record<string,unknown>): void {
    if (action.kind === "select" && (!filesEnabled || pending || files.length >= 12)) return;
    eventTarget.dispatchEvent(new CustomEvent("central-agent:graph-files", {bubbles:true,detail:{owner,action}}));
  }
  function changed(): void {
    draftText = input.value;
    retainGraphDraft(owner, input.value, draftPersistent);
    draftSaveStatus=draftPersistent ? graphDraftStatus(owner) : "local";
    input.style.height = "auto";
    input.style.height = `${input.scrollHeight}px`;
  }
  function submit(event: SubmitEvent): void {
    event.preventDefault();
    if (pending || stopping) return;
    if (observingAgent()) {
      if (!canSteerAgent() || !supervision || !input.value.trim()) return;
      const message=input.value;
      const dispatched=eventTarget.dispatchEvent(new CustomEvent("central-agent:supervisor-steer", {bubbles:true,cancelable:true,detail:{owner,targetOwner:supervision.targetOwner,expectedTurnId:supervision.nativeTurnId,message,timing:{clickedAtMs:Date.now()}}}));
      pending=dispatched;
      if(dispatched){armPromptSendMotion(owner,message);animateSendControl(sendControl);}
      return;
    }
    if (!canSubmit) return;
    if (event.submitter instanceof HTMLButtonElement && event.submitter.dataset.workAction === "resume" && canResume && !hasPayload && !canStop) {
      pending = true;
      eventTarget.dispatchEvent(new CustomEvent("central-agent:graph-submit", {detail:{owner,input:"Riprendi il lavoro interrotto dal punto in cui ti sei fermato. Verifica lo stato attuale e completa la richiesta precedente.",delivery:"start",resume:true,fileIds:[],tabIds:[],shellIds:[],timing:{clickedAtMs:Date.now()}}}));
      return;
    }
    if (!input.value.trim() && !files.length && !contexts.tabs.length && !contexts.shells.length) return;
    deliver("start");
  }
  function stopWork(): void {
    if (!canStop || stopping) return;
    stopping = true;
    deliveryPending = false;
    eventTarget.dispatchEvent(new CustomEvent("central-agent:graph-stop", {detail:{owner}}));
  }
  function deliver(delivery: "start" | "steer" | "queue"): void {
    if(consumeNativeCommand(input))return;
    if (!canSubmit || pending || stopping) return;
    if (delivery === "steer" && deliveryTurnId) {
      const message=input.value;
      const dispatched=eventTarget.dispatchEvent(new CustomEvent("central-agent:native-steer", {bubbles:true,cancelable:true,detail:{owner,expectedTurnId:deliveryTurnId,message,fileIds:files.map(file => file.id),...contextIds(contexts),timing:{clickedAtMs:Date.now()}}}));
      pending=dispatched;
      if(dispatched){armPromptSendMotion(owner,message);animateSendControl(sendControl);}
      return;
    }
    const intent = graphDeliveryIntent(active, supportsSteer, delivery);
    if (intent === "choose") { if (input.value.trim() || files.length || contexts.tabs.length || contexts.shells.length) { deliveryTurnId = nativeTurnId; deliveryPending = true; } return; }
    if (delivery === "steer") return;
    const message=input.value;
    const dispatched=eventTarget.dispatchEvent(new CustomEvent("central-agent:graph-submit", {cancelable:true,detail:{owner,input:message,delivery:intent,fileIds:files.map(file => file.id),...contextIds(contexts),timing:{clickedAtMs:Date.now()}}}));
    pending=dispatched;
    if(dispatched){armPromptSendMotion(owner,message);animateSendControl(sendControl);}
  }
  onMount(() => {
    eventTarget.dataset.cardMinimized="false";
    const presentation=(event:Event)=>{
      const detail=(event as CustomEvent).detail;
      if(detail?.owner!==owner)return;
      minimized=detail.minimized===true;focused=detail.focused===true;
      eventTarget.dataset.cardMinimized=String(minimized);
      eventTarget.dataset.cardFocused=String(focused);
    };
    const nativeFileDrag=(event:Event)=>{
      const detail=(event as CustomEvent).detail||{};
      const phase=String(detail.phase||"");
      const eligible=Number(detail.fileCount)>0&&!steeringObserved&&filesEnabled&&!pending&&files.length<12;
      fileDragOver=(phase==="enter"||phase==="over")&&eligible;
      eventTarget.dataset.fileDragOver=String(fileDragOver);
    };
    eventTarget.addEventListener('central-agent:board-card-presentation',presentation);
    eventTarget.addEventListener('central-agent:native-file-drag',nativeFileDrag);
    input.value = graphDraft(owner, false); changed();
    const savedDraft=(event:Event)=>{
      const detail=(event as CustomEvent).detail;
      if(detail?.owner!==owner)return;
      if (!draftPersistent) return;
      input.value=graphDraft(owner);draftSaveError=detail.error||"";changed();
    };
    window.addEventListener("central-agent:composer-draft-updated",savedDraft);
    const state = (event: Event) => {
      const detail = (event as CustomEvent).detail;
      if (detail.owner !== owner) return;
      if (detail.assigned === true && !draftPersistent) {
        draftPersistent = true;
        input.value = graphDraft(owner);
        changed();
      }
      canSubmit = detail.canSubmit === true;
      blockedReason = canSubmit ? "" : typeof detail.blockedReason === "string" ? detail.blockedReason : "";
      filesEnabled = detail.canAttachFiles === true;
      active = detail.active === true; supportsSteer = detail.supportsSteer === true;
      canStop = detail.canStop === true || active;
      canResume = detail.canResume === true && !canStop;
      if (!canStop && stopping) { stopping = false; pending = false; }
      nativeTurnId = typeof detail.nativeTurnId === "string" ? detail.nativeTurnId : "";
      queued = Number(detail.queued) || 0;
      if (!active) deliveryPending = false;
    };
    const supervised = (event:Event) => {
      const detail=(event as CustomEvent).detail;
      if(detail?.owner!==owner)return;
      const next = detail.targetOwner && detail.run ? {
        targetOwner:String(detail.targetOwner), title:String(detail.title||"Observed agent"), run:detail.run,
        active:detail.active===true, supportsSteer:detail.supportsSteer===true,
        nativeTurnId:typeof detail.nativeTurnId==="string"?detail.nativeTurnId:"",
        pendingRequests:Number(detail.pendingRequests)||0, monitoring:detail.monitoring===true,
        verification:detail.verification||null, hooks:detail.hooks
      } satisfies SupervisionState : null;
      const changedTarget=next?.targetOwner!==supervision?.targetOwner;
      flushSync(()=>{supervision=next;if(!next)timelineScope="supervisor";else if(changedTarget)timelineScope="observed";});
      if(next)queueMicrotask(()=>eventTarget.dispatchEvent(new CustomEvent("central-agent:graph-timeline",{detail:{owner:next.targetOwner,run:next.run,hooks:next.hooks}})));
    };
    const disposeBridge = bridgeConversationEvents(window, eventTarget, owner);
    const contextState = (event:Event) => {
      const detail=(event as CustomEvent).detail;
      if(detail?.owner===owner) contexts=contextDraft(event.type==="central-agent:conversation-state" ? detail.contextAttachments : detail);
    };
    eventTarget.addEventListener("central-agent:conversation-state",contextState);
    eventTarget.addEventListener("central-agent:graph-contexts-result",contextState);
    const fileState = (event:Event) => {
      const detail = (event as CustomEvent).detail;
      if (detail?.owner !== owner) return;
      if (event.type === "central-agent:conversation-state") { files = attachmentList(detail.fileAttachments); if (detail.enabled === false) filesEnabled = false; }
      else { if (Array.isArray(detail.files)) files = attachmentList(detail.files); fileError = detail.error || ""; }
    };
    eventTarget.addEventListener("central-agent:conversation-state", fileState);
    eventTarget.addEventListener("central-agent:graph-files-result", fileState);
    const accepted = (event: Event) => {
      const detail = (event as CustomEvent).detail;
      if (detail?.owner !== owner) return;
      pending = false;
      deliveryPending = false;
      if (acceptsComposerReceipt(detail, owner, input.value)) { input.value = ""; changed(); }
    };
    const failed = (event: Event) => { if ((event as CustomEvent).detail?.owner === owner) { pending = false; stopping = false; fileError = (event as CustomEvent).detail.error || "Request rejected"; } };
    const operationResult = (event: Event) => { const detail = (event as CustomEvent).detail; if (detail?.owner === owner && detail.error) stopping = false; };
    const prefill = (event:Event) => {
      if ((event as CustomEvent).detail?.owner === owner && !input.value) { input.value = graphDraft(owner, draftPersistent); changed(); }
    };
    eventTarget.addEventListener("central-agent:graph-prefill", prefill);
    eventTarget.addEventListener("central-agent:graph-composer-state", state);
    eventTarget.addEventListener("central-agent:supervision-state", supervised);
    eventTarget.addEventListener("central-agent:submission-accepted", accepted);
    eventTarget.addEventListener("central-agent:conversation-error", failed);
    eventTarget.addEventListener("central-agent:app-server-conversation", operationResult);
    return () => {
      eventTarget.removeEventListener('central-agent:board-card-presentation',presentation);
      eventTarget.removeEventListener('central-agent:native-file-drag',nativeFileDrag);
      retainGraphDraft(owner, input.value, draftPersistent);
      window.removeEventListener("central-agent:composer-draft-updated",savedDraft);
      eventTarget.removeEventListener("central-agent:graph-prefill", prefill);
      eventTarget.removeEventListener("central-agent:graph-composer-state", state);
      eventTarget.removeEventListener("central-agent:supervision-state", supervised);
      eventTarget.removeEventListener("central-agent:submission-accepted", accepted);
      eventTarget.removeEventListener("central-agent:conversation-error", failed);
      eventTarget.removeEventListener("central-agent:app-server-conversation", operationResult);
      disposeBridge();
      eventTarget.removeEventListener("central-agent:conversation-state",contextState);
      eventTarget.removeEventListener("central-agent:graph-contexts-result",contextState);
      eventTarget.removeEventListener("central-agent:conversation-state", fileState);
      eventTarget.removeEventListener("central-agent:graph-files-result", fileState);
      delete eventTarget.dataset.cardMinimized;
      delete eventTarget.dataset.fileDragOver;
    };
  });
</script>

<AgentGraphHeader {eventTarget} {owner} {title} {minimized} {focused} onToggleFocused={toggleFocused} onToggleMinimized={toggleMinimized} working={active||Boolean(supervision?.active)} />
{#if fileDragOver}<div class="file-drop-overlay" aria-hidden="true"><span>Drop files or photos to attach</span></div>{/if}
<div class="agent-console-body" hidden={minimized}>
{#if supervision}
  <nav class="supervision-scope" aria-label="Supervisor conversation view">
    <button type="button" class:active={timelineScope === "supervisor"} aria-pressed={timelineScope === "supervisor"} onclick={() => timelineScope="supervisor"}>Supervisor</button>
    <button type="button" class:active={timelineScope === "observed"} aria-pressed={timelineScope === "observed"} onclick={() => timelineScope="observed"}>
      <span>Observed agent</span><small>{supervision.monitoring ? `Monitoring · ${supervision.active ? "Live" : "Idle"}` : supervision.active ? "Live" : "Idle"}</small>
    </button>
  </nav>
{/if}
<div class="timeline-pane" data-timeline-scope="supervisor" hidden={timelineScope !== "supervisor"}><GraphTimeline {eventTarget} {owner} /></div>
{#if supervision}
  {#key supervision.targetOwner}
    <div class="timeline-pane" data-timeline-scope="observed" hidden={timelineScope !== "observed"}><GraphTimeline {eventTarget} owner={supervision.targetOwner} /></div>
  {/key}
{/if}
{#if steeringObserved}
  <div class="observed-state" role="status">
    <span><strong>{supervision?.title}</strong> · {supervision?.active ? "Working now" : "Last known activity"}</span>
    {#if supervision?.monitoring}<small>Continuous supervision is active</small>{/if}
    {#if supervision?.pendingRequests}<small>{supervision.pendingRequests} {supervision.pendingRequests === 1 ? "request needs" : "requests need"} attention</small>{/if}
    {#if supervision?.verification}<TaskVerificationStatus owner={`supervisor-card:${owner}`} verification={supervision.verification}/>{/if}
  </div>
{/if}
{#if !steeringObserved}
<div class="graph-agent-status">
<div class="graph-conversation-controls">
  <NativeAccess {eventTarget} {owner} settingsVisible={false} />
  <NativeConversation {eventTarget} {owner} settingsVisible={false} />
  <NativeRequests {eventTarget} {owner} />
  <ProjectDiff {eventTarget} {inputId} />
  <div class="graph-file-drafts"><FileAttachments {files} disabled={pending} remove={(id) => manageFiles({kind:"remove",id})} /></div>
  <GraphContexts {owner} {eventTarget} draft={contexts} disabled={pending} enabled={true} />
  {#if fileError}<p role="alert">{fileError}</p>{/if}
  {#if draftSaveError}<p role="alert">Draft not saved: {draftSaveError}</p>{/if}
</div>
<NativeCommands {inputId} {owner} {eventTarget} />
{#if deliveryPending}
  <div class="graph-delivery" role="group" aria-label="Delivery for this node's active request">
    <span>Active request</span>
    <button type="button" disabled={!supportsSteer || pending} onclick={() => deliver("steer")}>Send now</button>
    <button type="button" disabled={pending} onclick={() => deliver("queue")}>Queue</button>
    <button type="button" disabled={pending} onclick={() => deliveryPending = false}>Cancel</button>
    {#if files.length}<span>Send now adds these files to the active turn; Queue starts a new turn.</span>{/if}
  </div>
{/if}
{#if queued > 0}
  <div class="graph-delivery" role="status">
    <span>{queued} {queued === 1 ? "request" : "requests"} queued</span>
    <button type="button" onclick={() => eventTarget.dispatchEvent(new CustomEvent("central-agent:graph-clear-queue", {detail:{owner}}))}>Clear queue</button>
  </div>
{/if}
<footer class="agent-console-approval" hidden>
  <span class="agent-console-approval-title">Approval required</span>
  <p class="agent-console-approval-summary"></p>
  <span class="agent-console-approval-meta"></span>
  <div class="agent-console-approval-actions">
    <button class="agent-console-action deny" data-action="deny" type="button">Deny</button>
    <button class="agent-console-action approve" data-action="approve" type="button">Approve once</button>
  </div>
</footer>
</div>
{/if}
<form class="agent-console-compose" onsubmit={submit}>
  <div class="graph-prompt-field">
  <textarea
    data-draft-status={draftSaveStatus}
    onfocus={() => { if (!steeringObserved) eventTarget.dispatchEvent(new CustomEvent("central-agent:native-prepare", {detail:{owner}})); }}
    bind:this={input}
    id={inputId}
    oninput={changed}
    onkeydown={event => {
      if (event.key === "Enter" && !event.shiftKey && !event.isComposing && !event.altKey) {
        event.preventDefault();
        if (!event.repeat) input.form?.requestSubmit();
      }
    }}
    data-role="message"
    maxlength="10000"
    placeholder={steeringObserved ? "Correct the active agent…" : "What should this agent do?"}
    aria-label={steeringObserved ? "Steer the observed agent" : "Message this project agent"}
    aria-describedby={visibleBlockedReason && (!canStop || steeringObserved) ? availabilityId : undefined}
    title="Enter to send · Shift+Enter for a new line"></textarea>
    <div class="graph-prompt-actions">
      {#if !steeringObserved}
        <button type="button" class="attach-file" aria-label="Attach files" title={filesEnabled ? "Attach PNG, JPEG, MP3, WAV, or UTF-8 text/code files" : "Connect Codex and select a local conversation to attach files"} disabled={!filesEnabled || pending || files.length >= 12} onclick={() => manageFiles({kind:"select"})}><span class="attach-files-plus" aria-hidden="true"></span></button>
        <PluginPicker {eventTarget} {owner} compact />
        <LiveDiffStats {eventTarget} {owner} />
      {:else}<span class="steer-label">Steer active turn</span>{/if}
      <CacheWindowTimer {eventTarget} {owner} />
    </div>
    <div class="graph-work-slot" bind:this={sendControl}>
      <button class="graph-work-action" data-action="stop" data-work-action="stop" type="button" aria-label={stopping ? "Stopping work" : "Stop work"} title={stopping ? "Waiting for work to stop" : "Stop work"} hidden={!canStop || steeringObserved} disabled={stopping} onclick={stopWork}>
        <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><rect x="6" y="6" width="12" height="12" rx="2" /></svg>
      </button>
      <button class="graph-work-action" data-action="submit" data-work-action={steeringObserved ? "steer" : primaryAction} type="submit" aria-label={pending ? "Sending message" : steeringObserved ? "Steer observed agent" : primaryAction === "resume" ? "Resume work" : "Send message"} aria-describedby={visibleBlockedReason && (!canStop || steeringObserved) ? availabilityId : undefined} title={visibleBlockedReason || (pending ? "Waiting for acceptance" : steeringObserved ? "Add this correction to the active observed turn" : primaryAction === "resume" ? "Resume interrupted work in this conversation" : "Send message")} hidden={canStop && !steeringObserved} disabled={!actionEnabled || pending || (!hasPayload && !(canResume && !steeringObserved))}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          {#if primaryAction === "resume"}<path d="m9 5 10 7-10 7Z" fill="currentColor" stroke="none" />{:else}<path d="M12 19V5m-6 6 6-6 6 6" />{/if}
        </svg>
      </button>
    </div>
  </div>
  {#if visibleBlockedReason && (!canStop || steeringObserved)}<p class="graph-availability" id={availabilityId} role="status">{visibleBlockedReason}</p>{/if}
</form>
</div>

<style>
  :global(.agent-console[data-central-agent-svelte="knowledge-agent-popup"]) { display: flex; flex-direction: column; box-sizing: border-box; width: min(var(--ca-agent-card-width), var(--agent-console-available-width, calc(100% - var(--ca-space-6))), calc(var(--agent-console-available-height, calc(100vh - var(--ca-space-6))) * 9 / 16)); aspect-ratio: var(--agent-card-aspect-ratio, 9 / 16); height: auto; min-width: 0; min-height: 0; max-width: none; max-height: none; overflow: hidden; background: var(--agent-card-background,var(--ca-app-background)); color: var(--ca-text); }
  :global(.agent-console[data-central-agent-svelte="knowledge-agent-popup"][data-card-minimized="true"]) { --agent-card-aspect-ratio:auto; }
  .file-drop-overlay { position:absolute;z-index:20;inset:var(--ca-space-2);display:grid;place-items:center;border-radius:var(--ca-radius-large);background:color-mix(in srgb,var(--agent-card-background,var(--ca-app-background)) 84%,var(--ca-accent-soft));color:var(--ca-text);box-shadow:inset 0 0 0 2px var(--ca-accent);font:600 var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body);pointer-events:none;backdrop-filter:blur(4px); }
  .agent-console-body { display:flex;flex:1 1 auto;min-width:0;min-height:0;flex-direction:column; }
  .agent-console-body[hidden] { display:none; }
  .timeline-pane { display:flex; flex:1 1 0; min-height:0; min-width:0; }
  .timeline-pane[hidden] { display:none; }
  .timeline-pane :global(.graph-timeline) { flex:1 1 0; min-height:0; }
  .supervision-scope { display:grid;grid-template-columns:1fr 1fr;gap:var(--ca-space-1);flex:0 0 auto;margin:0 var(--ca-space-3);padding:var(--ca-space-1);border:0;border-radius:var(--ca-control-radius);background:var(--ca-surface); }
  .supervision-scope button { display:flex;align-items:center;justify-content:center;gap:var(--ca-space-2);min-height:var(--ca-control-compact);padding:var(--ca-space-1) var(--ca-space-2);border:0;border-radius:var(--ca-control-radius);background:transparent;color:var(--ca-muted);font:var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body);cursor:pointer; }
  .supervision-scope button.active { background:var(--ca-surface-2);color:var(--ca-text); }
  .supervision-scope button:focus-visible { outline:1px solid var(--ca-border-strong);box-shadow:var(--ca-focus-ring); }
  .supervision-scope small { color:var(--ca-muted);font-size:var(--ca-type-caption); }
  .observed-state { display:flex;align-items:center;justify-content:space-between;gap:var(--ca-space-2);flex:0 0 auto;padding:var(--ca-space-2) var(--ca-space-3);background:var(--ca-surface);color:var(--ca-muted);font:var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body); }
  .observed-state strong { color:var(--ca-text);font-weight:600; }
  .steer-label { color:var(--ca-muted);font:var(--ca-type-caption)/var(--ca-leading-body) var(--ca-font-body); }
  [hidden] { display: none; }
  .graph-delivery { display: flex; flex-wrap: wrap; align-items: center; gap: var(--ca-space-2); padding: var(--ca-space-2) var(--ca-space-3); flex: 0 0 auto; font: var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body); color: var(--ca-text); background: var(--agent-card-background,var(--ca-app-background)); }
  .graph-delivery button { font: inherit; color: inherit; background: var(--ca-surface); border: 0; border-radius: var(--ca-radius-small); padding: var(--ca-space-1) var(--ca-space-2); cursor: pointer; }
  .graph-delivery button:focus-visible { outline: 1px solid var(--ca-border-strong); outline-offset: var(--ca-space-1); }
  .graph-delivery button:disabled { color: var(--ca-muted); cursor: default; }
  .graph-conversation-controls { position: relative; flex: 0 0 auto; min-width: 0; padding: var(--ca-space-2) var(--ca-space-3); background: var(--agent-card-background,var(--ca-app-background)); color: var(--ca-text); }
  .graph-agent-status { flex: 0 1 auto; min-height: 0; max-height: 45%; overflow: auto; overscroll-behavior: contain; }
  .agent-console-compose textarea { width: 100%; box-sizing: border-box; min-height: var(--ca-control-default); max-height: 7rem; resize: none; border: 0; background: transparent; color: var(--ca-text); font: var(--ca-type-body)/var(--ca-leading-body) var(--ca-font-body); }
  .agent-console-compose textarea:focus { outline: 0; box-shadow: none; }
  .graph-prompt-field { position:relative; display:grid; min-width:0; padding:var(--ca-space-1); border:0; border-radius:var(--ca-radius-medium); background:var(--ca-input); }
  .graph-prompt-field:focus-within { box-shadow: var(--ca-focus-ring); }
  .graph-prompt-actions { display:flex; flex-wrap:wrap; align-items:center; gap:0 var(--ca-space-1); padding:0 calc(var(--ca-control-compact) + var(--ca-space-4)) 0 var(--ca-space-1); }
  .graph-work-slot { position:absolute;inset-inline-end:var(--ca-space-2);inset-block-end:var(--ca-space-2);display:grid;place-items:stretch;box-sizing:border-box;margin:0;width:var(--ca-control-compact);min-width:var(--ca-control-compact);height:var(--ca-control-compact);transform-origin:center; }
  .agent-console-compose .graph-work-action { display: grid; place-items: center; box-sizing: border-box; flex: 0 0 auto; margin-left: auto; width: var(--ca-control-compact); min-width: var(--ca-control-compact); height: var(--ca-control-compact); padding: var(--ca-space-1); border: 0; border-radius: var(--ca-control-radius); color: var(--ca-on-accent); background: var(--ca-accent); box-shadow: none; cursor: pointer; }
  .agent-console-compose .graph-work-slot .graph-work-action { grid-area:1/1;margin-left:0; }
  .graph-work-action[hidden] { display: none; }
  .graph-work-action svg { width: var(--ca-space-6); height: var(--ca-space-6); }
  .agent-console-compose .graph-work-action:hover:not(:disabled) { background: var(--ca-accent-hover); }
  .agent-console-compose .graph-work-action:disabled { color: var(--ca-muted); background: var(--ca-surface-2); opacity: 1; cursor: default; }
  .agent-console-compose .graph-work-action:focus-visible { outline: 1px solid var(--ca-border-strong); box-shadow: var(--ca-focus-ring); }
  .agent-console-compose .attach-file { display: grid; place-items: center; width: var(--ca-control-compact); min-width: var(--ca-control-compact); height: var(--ca-control-compact); padding: 0; border: 0; border-radius: var(--ca-control-radius); background: transparent; color: var(--ca-text); box-shadow: none; cursor: pointer; }
  .agent-console-compose .attach-file:hover:not(:disabled) { background: var(--ca-surface-2); }
  .agent-console-compose .attach-file:disabled { color: var(--ca-muted); cursor: default; }
  .agent-console-compose .attach-file:focus-visible { outline: 1px solid var(--ca-border-strong); box-shadow: var(--ca-focus-ring); }
  .attach-files-plus { position: relative; display: block; width: var(--ca-type-body); height: var(--ca-type-body); }
  .attach-files-plus::before, .attach-files-plus::after { position: absolute; top: 50%; left: 50%; width: 100%; height: 1.5px; border-radius: var(--ca-radius-small); background: currentColor; content: ""; transform: translate(-50%, -50%); }
  .attach-files-plus::after { transform: translate(-50%, -50%) rotate(90deg); }
  .agent-console-compose { position: relative; flex: 0 0 auto; margin-top: auto; grid-template-columns: minmax(0, 1fr); padding: var(--ca-space-2) var(--ca-space-3) var(--ca-space-3); background: var(--agent-card-background,var(--ca-app-background)); }
  .graph-file-drafts { max-height:12rem; overflow:auto; min-width:0; }
  .graph-availability { overflow-wrap:anywhere; }
  p { margin: var(--ca-space-1) 0; font: var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body); color: var(--ca-muted); }
</style>
