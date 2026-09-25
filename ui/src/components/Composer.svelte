<script lang="ts">
  import { onMount } from "svelte";
  import SupervisorLogo from "./SupervisorLogo.svelte";
  import ContextWindowMonitor from "./ContextWindowMonitor.svelte";
  import ProjectDiff from "./ProjectDiff.svelte";
  import NativeCommands from "./NativeCommands.svelte";
  import PluginPicker from "./PluginPicker.svelte";
  import LiveDiffStats from "./LiveDiffStats.svelte";
  import CacheWindowTimer from "./CacheWindowTimer.svelte";
  import {consumeNativeCommand} from "../lib/native-commands";
  import { acceptsComposerReceipt } from "../lib/composer-receipt";
  import { mainDrafts as drafts } from "../lib/persisted-drafts";
  import { animateSendControl, armPromptSendMotion } from "../lib/prompt-send-motion";
  let conversationKey = "";
  const draftWarnings = new Map<string, string>();
  let draftWarning = $state("");
  let draftSaveError = $state("");
  let draftSaveStatus = $state("loading");

  let composerElement: HTMLDivElement;
  let inputElement: HTMLTextAreaElement;
  let sendControl: HTMLDivElement;
  let dragOver = $state(false);
  let agentActive = $state(false);
  let supportsSteer = $state(false);
  let canResume = $state(false);
  let stopping = $state(false);
  let submissionPending = $state(false);
  let draftPayload = $state(false);
  const primaryAction = $derived(canResume && !draftPayload ? "resume" : "send");
  let deliveryPending = $state(false);
  let deliveryTurnId = "";

  function emit(type: string, detail: Record<string, unknown> = {}, cancelable = false): boolean {
    return composerElement.dispatchEvent(
      new CustomEvent(type, { bubbles: true, cancelable, detail }),
    );
  }

  function syncAgentState(): void {
    agentActive = composerElement.dataset.agentActive === "true";
    supportsSteer = composerElement.dataset.supportsSteer === "true";
    canResume = composerElement.dataset.canResume === "true" && !agentActive;
    if (!agentActive) { deliveryPending = false; stopping = false; }
    else submissionPending = false;
  }

  function hasDraftPayload(): boolean {
    const attachments = composerElement.querySelector("#composer-attachments");
    return Boolean(inputElement.value.trim()) || Boolean(attachments?.childElementCount);
  }

  function syncPayloadState(): void { draftPayload = hasDraftPayload(); }

  function dispatchSubmission(delivery: "start" | "steer" | "queue", resume = false): void {
    if (submissionPending || stopping) return;
    const message = resume ? "" : inputElement.value.trim();
    const dispatched = emit(
      "central-agent:submit",
      { message, delivery, resume, timing: {clickedAtMs: Date.now()} },
      true,
    );
    if (dispatched) {
      submissionPending = true;
      if (!resume) {
        armPromptSendMotion(conversationKey, message);
        animateSendControl(sendControl);
      }
    }
    // Clear only after Rust has saved the submission. An IPC dispatch is not acceptance.
  }

  function submit(): void {
    if(consumeNativeCommand(inputElement))return;
    syncAgentState();
    syncPayloadState();
    if (submissionPending || stopping) return;
    if (!hasDraftPayload()) return;
    if (agentActive) {
      deliveryTurnId = composerElement.dataset.nativeTurnId || "";
      deliveryPending = true;
      return;
    }
    dispatchSubmission("start");
  }

  function primaryWorkAction(): void {
    syncAgentState();
    syncPayloadState();
    if (primaryAction === "resume") dispatchSubmission("start", true);
    else submit();
  }

  function stopWork(): void {
    if (!agentActive || stopping) return;
    stopping = true;
    deliveryPending = false;
    if (!emit("central-agent:stop", {}, true)) stopping = false;
  }

  function deliverActiveRequest(delivery: "steer" | "queue"): void {
    if(consumeNativeCommand(inputElement))return;
    // The choice targets the turn visible when it opened, never its successor.
    if (delivery === "steer" && deliveryPending && deliveryTurnId) {
      const message = inputElement.value;
      if (emit("central-agent:native-steer", {owner: conversationKey, expectedTurnId: deliveryTurnId, message, timing: {clickedAtMs: Date.now()}})) {
        armPromptSendMotion(conversationKey, message);
        animateSendControl(sendControl);
      }
      return;
    }
    syncAgentState();
    if (!deliveryPending) return;
    if (!agentActive) {
      dispatchSubmission("start");
      return;
    }
    if (delivery === "steer") return;
    dispatchSubmission("queue");
  }

  function queueShortcut(): void {
    if(consumeNativeCommand(inputElement))return;
    syncAgentState();
    if (agentActive && hasDraftPayload()) dispatchSubmission("queue");
  }

  function handleAgentState(event: Event): void {
    const detail = event instanceof CustomEvent ? event.detail : null;
    agentActive = detail?.active === true;
    supportsSteer = detail?.supportsSteer === true;
    canResume = detail?.canResume === true && !agentActive;
    if (!agentActive) { deliveryPending = false; stopping = false; }
    else submissionPending = false;
  }

  onMount(() => {
    const switchConversation = (event: Event) => {
      const next = String((event as CustomEvent<string>).detail || "unbound");
      if (next === conversationKey) return;
      if (conversationKey) persistDraft();
      conversationKey = next;
      inputElement.value = readDraft();
      draftSaveError = drafts.error(next);
      draftSaveStatus = drafts.status(next);
      draftWarning = draftWarnings.get(next) || "";
      deliveryPending = false;
      submissionPending = false;
      stopping = false;
      queueMicrotask(resizeInput);
      queueMicrotask(syncPayloadState);
    };
    window.addEventListener("central-agent:conversation-key", switchConversation);
    const savedDraft=(event:Event)=>{
      const detail=(event as CustomEvent).detail;
      if(detail?.owner!==conversationKey)return;
      inputElement.value=readDraft();draftSaveError=detail.error||"";draftSaveStatus=drafts.status(conversationKey);resizeInput();syncPayloadState();
    };
    window.addEventListener("central-agent:composer-draft-updated",savedDraft);
    const showError = (event: Event) => { const detail = (event as CustomEvent).detail; if (detail?.owner === conversationKey) { draftWarning = detail.error || "Request rejected"; submissionPending = false; stopping = false; } };
    window.addEventListener("central-agent:conversation-error", showError);
    syncAgentState();
    composerElement.addEventListener("central-agent:agent-state", handleAgentState);
    const accept = (event: Event) => {
      const receipt = (event as CustomEvent).detail;
      if (typeof receipt?.owner === "string" && receipt.owner !== conversationKey) {
        if (acceptsComposerReceipt(receipt, receipt.owner, drafts.read(receipt.owner))) drafts.write(receipt.owner, "");
        return;
      }
      if (!acceptsComposerReceipt(receipt, conversationKey, inputElement.value)) return;
      inputElement.value = "";
      persistDraft();
      deliveryPending = false;
      submissionPending = false;
      resizeInput();
      syncPayloadState();
    };
    window.addEventListener("central-agent:submission-accepted", accept);
    const preservePromotedDraft = (event: Event) => {
      const detail = (event as CustomEvent).detail;
      if (detail?.from === conversationKey && typeof detail?.to === "string") {
        drafts.write(detail.to, inputElement.value);
        drafts.write(detail.from, "");
      }
    };
    window.addEventListener("central-agent:preserve-promoted-draft", preservePromotedDraft);
    const prefillDraft = (event: Event) => {
      const detail = (event as CustomEvent).detail;
      if (typeof detail?.owner !== "string" || typeof detail?.text !== "string") return;
      if (detail.owner === conversationKey) persistDraft();
      if (!drafts.seed(detail.owner, detail.text)) {
        draftWarnings.set(detail.owner, "Your current draft was preserved. Recover the edited prompt from History → Saved operations after clearing this draft, or copy its saved text.");
      } else {
        draftWarnings.delete(detail.owner);
        if (detail.owner === conversationKey) { inputElement.value = readDraft(); resizeInput(); syncPayloadState(); }
      }
      if (detail.owner === conversationKey) draftWarning = draftWarnings.get(conversationKey) || "";
    };
    window.addEventListener("central-agent:prefill-draft", prefillDraft);
    const attachments = composerElement.querySelector("#composer-attachments");
    let attachmentObserver: MutationObserver | null = null;
    if (typeof MutationObserver !== "undefined" && attachments) {
      attachmentObserver = new MutationObserver(syncPayloadState);
      attachmentObserver.observe(attachments, {childList:true});
    }
    syncPayloadState();
    return () => {
      window.removeEventListener("central-agent:conversation-key", switchConversation);
      window.removeEventListener("central-agent:composer-draft-updated",savedDraft);
      window.removeEventListener("central-agent:submission-accepted", accept);
      window.removeEventListener("central-agent:preserve-promoted-draft", preservePromotedDraft);
      window.removeEventListener("central-agent:prefill-draft", prefillDraft);
      composerElement.removeEventListener("central-agent:agent-state", handleAgentState);
      window.removeEventListener("central-agent:conversation-error", showError);
      attachmentObserver?.disconnect();
    };
  });

  function resizeInput(): void {
    inputElement.style.height = "auto";
    inputElement.style.height = `${inputElement.scrollHeight}px`;
  }

  function readDraft(): string {
    return drafts.read(conversationKey);
  }

  function persistDraft(): void {
    drafts.write(conversationKey, inputElement.value);
    draftSaveStatus=drafts.status(conversationKey);
  }

  function handleInput(): void {
    persistDraft();
    resizeInput();
    syncPayloadState();
  }

  function prepareConversation(): void {
    if (conversationKey) emit("central-agent:native-prepare", {owner: conversationKey});
  }

  function handleInputKeydown(event: KeyboardEvent): void {
    if (event.isComposing || event.key !== "Enter" || event.shiftKey) return;
    event.preventDefault();
    submit();
  }

  function handleDragEnter(event: DragEvent): void {
    event.preventDefault();
    dragOver = true;
  }

  function handleDragOver(event: DragEvent): void {
    event.preventDefault();
    if (event.dataTransfer) event.dataTransfer.dropEffect = "copy";
    dragOver = true;
  }

  function handleDragLeave(event: DragEvent): void {
    const related = event.relatedTarget;
    if (!(related instanceof Node) || !composerElement.contains(related)) dragOver = false;
  }

  function handleDrop(event: DragEvent): void {
    event.preventDefault();
    dragOver = false;
    emit("central-agent:drop-context", { dataTransfer: event.dataTransfer });
  }
</script>

<div
  id="composer"
  class="composer"
  role="group"
  aria-label="Agent message composer"
  class:drag-over={dragOver}
  bind:this={composerElement}
  ondragenter={handleDragEnter}
  ondragover={handleDragOver}
  ondragleave={handleDragLeave}
  ondrop={handleDrop}
>



  <ProjectDiff />
  <NativeCommands />




  {#if draftWarning}<p class="draft-warning" role="status">{draftWarning}</p>{/if}
  {#if draftSaveError}<p class="draft-warning" role="alert">Draft not saved: {draftSaveError}</p>{/if}
  <div id="queued-agent-request" class="queued-agent-request" role="status" hidden>
    <span>Request queued while Time Machine recovers the previous checkpoint.</span>
    <button
      id="discard-failed-checkpoint"
      type="button"
      hidden
      onclick={() => emit("central-agent:discard-failed-checkpoint")}>Discard failed checkpoint</button
    >
    <button
      id="cancel-queued-agent-request"
      type="button"
      onclick={() => emit("central-agent:cancel-queued-request")}>Cancel queued request</button
    >
  </div>
  <div id="composer-attachments" class="composer-attachments"></div>
  <textarea
    data-draft-status={draftSaveStatus}
    id="chat-input"
    bind:this={inputElement}
    maxlength="10000"
    aria-label="Message for the agent"
    oninput={handleInput}
    onfocus={prepareConversation}
    onkeydown={handleInputKeydown}></textarea
  >
  <div id="agent-prompt-queue" class="agent-prompt-queue" role="status" aria-live="polite" hidden>
    <span id="agent-prompt-queue-label"></span>
    <button
      id="clear-agent-prompt-queue"
      type="button"
      onclick={() => emit("central-agent:clear-prompt-queue")}>Clear queue</button
    >
  </div>
  <div class="composer-actions">
    <button
      id="attach-files"
      class="action attach-files"
      type="button"
      aria-label="Attach files"
      title="Attach PNG, JPEG, MP3, WAV, or UTF-8 text/code files"
      onclick={() => emit("central-agent:attach-files")}><span class="attach-files-plus" aria-hidden="true"></span></button
    >
    <PluginPicker />
    <div class="tetra-config" data-compound="tetra-configuration">
      <button
        id="tetra-config-button"
        class="tetra-config-button"
        type="button"
        aria-haspopup="dialog"
        aria-expanded="false"
        aria-controls="tetra-config-menu"
        aria-label="Configure provider, model, effort and speed"
      >
        <span id="tetra-config-visual" class="tetra-config-visual" aria-hidden="true"><SupervisorLogo /></span>
      </button>
    </div>
    <LiveDiffStats />
    <CacheWindowTimer />
  </div>
  <div class="composer-work-slot" bind:this={sendControl}>
    <button
      id="send-agent"
      class="action composer-work-action send-agent"
      data-work-action={primaryAction}
      type="button"
      hidden={agentActive}
      disabled={submissionPending || (!draftPayload && !canResume)}
      aria-label={submissionPending ? "Sending message" : primaryAction === "resume" ? "Resume interrupted response" : "Send message"}
      title={submissionPending ? "Waiting for acceptance" : primaryAction === "resume" ? "Resume interrupted response" : "Send message"}
      onclick={primaryWorkAction}>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        {#if primaryAction === "resume"}<path d="m9 5 10 7-10 7Z" fill="currentColor" stroke="none" />{:else}<path d="M12 19V5m-6 6 6-6 6 6" />{/if}
      </svg>
    </button>
    <button
      id="stop-agent"
      class="action composer-work-action stop-agent"
      type="button"
      hidden={!agentActive}
      disabled={stopping}
      aria-label={stopping ? "Stopping response" : "Stop response"}
      title={stopping ? "Waiting for the response to stop" : "Stop response"}
      onclick={stopWork}>
      <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><rect x="6" y="6" width="12" height="12" rx="2" /></svg>
    </button
    >
  </div>
  <div id="model-config-note" class="model-config-note" hidden></div>
  <ContextWindowMonitor />
</div>

<div
  id="agent-delivery-options"
  class="agent-delivery-options"
  role="group"
  aria-label="Delivery for the active request"
  hidden={!deliveryPending}
>
  <span class="agent-delivery-label">Active request</span>
  <button
    id="agent-delivery-now"
    class="agent-delivery-choice"
    type="button"
    disabled={!supportsSteer}
    onclick={() => deliverActiveRequest("steer")}>Send now</button
  >
  <button
    id="agent-delivery-queue"
    class="agent-delivery-choice"
    type="button"
    onclick={() => deliverActiveRequest("queue")}>Queue</button
  >
</div>

<style>
  .draft-warning { margin: var(--ca-space-2) 0; color: var(--ca-muted); font: var(--ca-type-body)/var(--ca-leading-body) var(--ca-font-body); }
  .composer-actions { flex-wrap:wrap; padding-inline-end:calc(var(--ca-control-compact) + var(--ca-space-4)); }
  .composer-work-slot { position:absolute;z-index:4;inset-inline-end:var(--ca-space-2);inset-block-end:var(--ca-space-2);display:grid;place-items:stretch;box-sizing:border-box;margin:0;width:var(--ca-control-compact);min-width:var(--ca-control-compact);height:var(--ca-control-compact);transform-origin:center; }
  .composer :global(.context-window-monitor), .model-config-note { padding-inline-end:calc(var(--ca-control-compact) + var(--ca-space-3)); }
  .composer-work-action { display:grid;place-items:center;box-sizing:border-box;grid-area:1/1;margin-left:0;width:var(--ca-control-compact);min-width:var(--ca-control-compact);height:var(--ca-control-compact);padding:var(--ca-space-1);border:0;border-radius:var(--ca-control-radius);background:var(--ca-accent);color:var(--ca-on-accent);box-shadow:none;cursor:pointer; }
  .composer-work-action[hidden] { display:none; }
  .composer-work-action svg { width:var(--ca-space-6);height:var(--ca-space-6); }
  .composer-work-action:hover:not(:disabled) { background:var(--ca-accent-hover); }
  .composer-work-action:disabled { color:var(--ca-muted);background:var(--ca-surface-2);cursor:default; }
  .composer-work-action:focus-visible { outline:1px solid var(--ca-border-strong);box-shadow:var(--ca-focus-ring); }
</style>
