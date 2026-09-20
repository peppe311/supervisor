<script lang="ts">
  import { emptyAppServerP2, type AppServerP2View, type CommandAccess, type NudgeCreditType, type P2Action } from "../lib/app-server-p2";

  let {
    view = emptyAppServerP2(),
    connected,
    accountSupported,
    availableResetCredits,
    resetCreditsCurrent,
    requirementsLoaded,
    onAction
  }: {
    view?: AppServerP2View;
    connected: boolean;
    accountSupported: boolean;
    availableResetCredits: string | null;
    resetCreditsCurrent: boolean;
    requirementsLoaded: boolean;
    onAction: (action: P2Action) => void;
  } = $props();

  type FeedbackClassification = "bug" | "bad_result" | "suggestion" | "other";
  type Confirmation =
    | { kind: "feature"; viewId: string; name: string; enabled: boolean }
    | { kind: "import"; viewId: string; itemIds: string[] }
    | { kind: "reset" }
    | { kind: "retry_reset"; attemptId: string }
    | { kind: "nudge"; creditType: NudgeCreditType }
    | { kind: "feedback"; classification: FeedbackClassification; reason: string }
    | { kind: "command"; scopeId: string; project: string; argv: string[]; access: CommandAccess; rows: number; cols: number }
    | { kind: "terminate"; processId: string };
  let confirmationDialog: HTMLDialogElement;
  let pendingConfirmation = $state<Confirmation | null>(null);
  let includeHome = $state(true);
  let includeProject = $state(true);
  let selectedItems = $state(new Set<string>());
  let nudgeType = $state<NudgeCreditType>("credits");
  let feedbackClassification = $state<FeedbackClassification>("bug");
  let feedbackReason = $state("");
  let argvText = $state('["git", "status", "--short"]');
  let commandAccess = $state<CommandAccess>("readOnly");
  let rows = $state(24);
  let cols = $state(100);
  let stdinText = $state("");
  let localError = $state("");
  let featureFilter = $state("");

  const commandWritable = $derived(view.command.process?.status === "running");
  const commandActive = $derived(["running", "stopping", "termination_uncertain"].includes(view.command.process?.status || ""));
  const commandText = $derived(view.command.process?.output.map(chunk => chunk.text).join("") || "");
  const normalizedFeatureFilter = $derived(featureFilter.trim().toLocaleLowerCase());
  const visibleFeatures = $derived(view.features.entries.filter(feature => !normalizedFeatureFilter || `${feature.displayName || ""} ${feature.name} ${feature.stage} ${feature.description || ""} ${feature.announcement || ""}`.toLocaleLowerCase().includes(normalizedFeatureFilter)));

  function toggleItem(id: string): void {
    const next = new Set(selectedItems);
    next.has(id) ? next.delete(id) : next.add(id);
    selectedItems = next;
  }
  function detect(): void {
    localError = "";
    if (!view.scope.id) { localError = "Open or select a local project first."; return; }
    selectedItems = new Set();
    onAction({ kind: "detect_import", scope_id: view.scope.id, include_home: includeHome, include_project: includeProject });
  }
  function confirmFeature(name: string, enabled: boolean): void {
    if (!view.features.viewId) return;
    openConfirmation({ kind: "feature", viewId: view.features.viewId, name, enabled });
  }
  function parseArgv(): string[] | null {
    try {
      const value: unknown = JSON.parse(argvText);
      if (!Array.isArray(value) || !value.length || !value.every(item => typeof item === "string")) throw new Error();
      return value;
    } catch {
      localError = "Command must be a nonempty JSON array of string arguments.";
      return null;
    }
  }
  function beginCommand(): void {
    localError = "";
    const argv = parseArgv();
    if (!argv || !view.scope.id || !view.scope.project) {
      if (!view.scope.id || !view.scope.project) localError = "Open or select a local project first.";
      return;
    }
    openConfirmation({ kind: "command", scopeId: view.scope.id, project: view.scope.project, argv, access: commandAccess, rows, cols });
  }
  function beginImport(): void {
    if (!view.migration.viewId || !selectedItems.size) return;
    openConfirmation({ kind: "import", viewId: view.migration.viewId, itemIds: [...selectedItems] });
  }
  function beginRetryReset(): void {
    if (view.accountActions.resetRetryId) openConfirmation({ kind: "retry_reset", attemptId: view.accountActions.resetRetryId });
  }
  function beginTerminate(): void {
    if (view.command.process && commandWritable) openConfirmation({ kind: "terminate", processId: view.command.process.id });
  }
  function openConfirmation(operation: Confirmation): void {
    pendingConfirmation = operation;
    confirmationDialog.showModal();
  }
  function closeConfirmation(): void {
    confirmationDialog.close();
    pendingConfirmation = null;
  }
  function confirm(): void {
    const operation = pendingConfirmation;
    closeConfirmation();
    localError = "";
    if (!operation) return;
    switch (operation.kind) {
      case "feature": onAction({ kind: "set_feature", view_id: operation.viewId, name: operation.name, enabled: operation.enabled }); break;
      case "import": onAction({ kind: "import", view_id: operation.viewId, item_ids: operation.itemIds }); break;
      case "reset": onAction({ kind: "consume_reset" }); break;
      case "retry_reset": onAction({ kind: "retry_reset", attempt_id: operation.attemptId }); break;
      case "nudge": onAction({ kind: "send_nudge", credit_type: operation.creditType }); break;
      case "feedback": onAction({ kind: "submit_feedback", classification: operation.classification, reason: operation.reason }); break;
      case "command": onAction({ kind: "run_command", scope_id: operation.scopeId, command: operation.argv, access: operation.access, rows: operation.rows, cols: operation.cols }); break;
      case "terminate": onAction({ kind: "terminate_command", process_id: operation.processId }); break;
    }
  }
  function sendInput(closeStdin = false): void {
    if (!view.command.process || !commandWritable) return;
    onAction({ kind: "write_command", process_id: view.command.process.id, input: stdinText, close_stdin: closeStdin });
    stdinText = "";
  }
  function resize(): void {
    if (view.command.process && commandWritable) onAction({ kind: "resize_command", process_id: view.command.process.id, rows, cols });
  }
  function historyTime(value: string): string {
    try {
      const milliseconds = BigInt(value);
      if (milliseconds > BigInt(Number.MAX_SAFE_INTEGER)) return `${value} ms`;
      const date = new Date(Number(milliseconds));
      return Number.isNaN(date.getTime()) ? `${value} ms` : date.toISOString();
    } catch { return `${value} ms`; }
  }
  function confirmationTitle(operation: Confirmation): string {
    switch (operation.kind) {
      case "feature": return `${operation.enabled ? "Enable" : "Disable"} ${operation.name}?`;
      case "import": return "Import selected agent data?";
      case "reset": return "Use one reset credit?";
      case "retry_reset": return "Retry the same reset attempt?";
      case "nudge": return "Request a workspace email?";
      case "feedback": return "Send this feedback?";
      case "terminate": return "Terminate the App Server process?";
      case "command": return "Run this sandboxed command?";
    }
  }
  function feedbackStatusText(status: string): string {
    if (status === "sent_without_logs_or_files") return "Feedback sent without logs or files.";
    if (status === "uncertain") return "Feedback delivery is unknown; it was not sent again automatically.";
    if (status === "failed") return "Feedback was not sent.";
    return "Uploading text-only feedback…";
  }
  function importStatusText(status: string): string {
    if (status === "requesting") return "Import request pending";
    if (status === "accepted") return "Import accepted · waiting for progress";
    if (status === "in_progress") return "Import in progress";
    if (status === "completed") return "Import completed";
    if (status === "failed") return "Import failed";
    if (status === "uncertain") return "Import outcome unknown · reconcile history";
    return "Import status unavailable";
  }
  function resetStatusText(status: string): string {
    if (status === "requesting") return "Requesting reset…";
    if (status === "retrying_same_attempt") return "Retrying the same reset attempt…";
    if (status === "reset") return "Reset applied.";
    if (status === "nothingToReset") return "Nothing needed resetting.";
    if (status === "noCredit") return "No reset credit was available.";
    if (status === "alreadyRedeemed") return "This reset attempt was already redeemed.";
    if (status === "uncertain") return "Reset outcome unknown; the attempt identity was retained.";
    if (status === "failed") return "Reset was not applied.";
    return "Reset status unavailable.";
  }
  function nudgeStatusText(status: string): string {
    if (status === "requesting") return "Requesting workspace email…";
    if (status === "sent") return "Workspace email sent.";
    if (status === "cooldown_active") return "Workspace email cooldown is active.";
    if (status === "uncertain") return "Email delivery unknown; the request was not replayed.";
    if (status === "failed") return "Workspace email was not sent.";
    return "Email status unavailable.";
  }
  function commandStatusText(status: string): string {
    if (status === "running") return "Running";
    if (status === "stopping") return "Stopping";
    if (status === "termination_uncertain") return "Termination outcome unknown";
    if (status === "completed") return "Completed";
    if (status === "failed") return "Failed";
    if (status === "connection_closed") return "Connection closed";
    return "Status unavailable";
  }
</script>

<section class="p2" data-native-p2-settings aria-labelledby="native-p2-title">
  <h3 id="native-p2-title">Advanced Codex tools</h3>
  <p class="copy">Manage optional features, imports and account actions. Changes require confirmation.</p>
  <p class="scope">Active project: {view.scope.project || "No local project"}</p>
  <p class="copy">This scope follows the active local project automatically.</p>
  {#if localError}<p role="alert">{localError}</p>{/if}

  <details data-native-feature-flags>
    <summary>Runtime feature flags ({view.features.entries.length})</summary>
    <p class="copy">Read-only inventory by default. A confirmed change is process-wide; only beta and stable entries can be changed here.</p>
    <button type="button" disabled={!connected || view.features.loading} onclick={() => onAction({ kind: "refresh_features" })}>{view.features.loading ? "Loading…" : "Refresh feature inventory"}</button>
    {#if view.features.loaded && !view.features.current}<p class="copy">The inventory is stale. Refresh before changing a feature.</p>{/if}
    {#if view.features.entries.length}
      <label class="filter">Filter feature flags<input type="search" bind:value={featureFilter} placeholder="Name, stage or description" /></label>
      {#if normalizedFeatureFilter}<p class="copy">{visibleFeatures.length} of {view.features.entries.length} feature flags match.</p>{/if}
    {/if}
    {#if view.features.entries.length && !visibleFeatures.length}<p class="copy">No feature flag matches this filter.</p>{/if}
    {#each visibleFeatures as feature (feature.name)}
      <article class="entry">
        <strong>{feature.displayName || feature.name}</strong>
        <span>{feature.stage} · {feature.enabled ? "Enabled" : "Disabled"} · default {feature.defaultEnabled ? "on" : "off"}</span>
        {#if feature.description}<span>{feature.description}</span>{/if}
        {#if feature.announcement}<span>{feature.announcement}</span>{/if}
        <button type="button" disabled={!connected || !view.features.current || !feature.mutable} onclick={() => confirmFeature(feature.name, !feature.enabled)}>{feature.enabled ? "Disable…" : "Enable…"}</button>
      </article>
    {/each}
    {#if view.features.notice}<p role="status">{view.features.notice}</p>{/if}
    {#if view.features.error}<p role="alert">{view.features.error}</p>{/if}
  </details>

  <details data-native-external-import>
    <summary>Import from another agent ({view.migration.items.length})</summary>
    <p class="copy">Detection builds a frozen preview. Raw details stay in Rust and only the exact selected native items are sent back for import.</p>
    <div class="choices">
      <label class="choice"><input type="checkbox" bind:checked={includeHome} /><span>Home configuration</span></label>
      <label class="choice"><input type="checkbox" bind:checked={includeProject} disabled={!view.scope.project} /><span>Current project</span></label>
    </div>
    <div class="actions">
      <button type="button" disabled={!connected || view.migration.busy || !view.scope.id || (!includeHome && !includeProject)} onclick={detect}>Detect importable items</button>
      <button type="button" disabled={!connected || view.migration.busy} onclick={() => onAction({ kind: "read_import_history" })}>Refresh import history</button>
    </div>
    {#each view.migration.connectors as connector}
      <p class="copy">{connector.name} · {connector.source} · {connector.sessionCount} sessions</p>
    {/each}
    {#if view.migration.current}
      <div class="selection-actions">
        <button type="button" onclick={() => selectedItems = new Set(view.migration.items.map(item => item.id))}>Select all</button>
        <button type="button" onclick={() => selectedItems = new Set()}>Clear</button>
      </div>
      {#each view.migration.items as item (item.id)}
        <label class="entry selectable"><input type="checkbox" checked={selectedItems.has(item.id)} onchange={() => toggleItem(item.id)} /><span><strong>{item.itemType} · {item.scope}</strong><br />{item.description}</span></label>
      {/each}
      <button type="button" disabled={!selectedItems.size || view.migration.busy} onclick={beginImport}>Import {selectedItems.size} selected item{selectedItems.size === 1 ? "" : "s"}…</button>
    {/if}
    {#if view.migration.activeImport}
      <article class="entry"><strong>{importStatusText(view.migration.activeImport.status)}</strong>{#each view.migration.activeImport.results as result}<span>{result.itemType}: {result.successes} succeeded · {result.failures} failed</span>{/each}</article>
    {/if}
    {#if view.migration.histories.length}
      <details><summary>Sanitized import history ({view.migration.histories.length})</summary>{#each view.migration.histories as history}<p>{historyTime(history.completedAtMs)} · {history.successes} succeeded · {history.failures} failed{history.provider ? " · provider recorded" : ""}</p>{/each}</details>
    {/if}
    {#if view.migration.notice}<p role="status">{view.migration.notice}</p>{/if}
    {#if view.migration.error}<p role="alert">{view.migration.error}</p>{/if}
  </details>

  <details data-native-account-side-effects>
    <summary>Explicit account actions</summary>
    <p class="copy">Available reset credits: {availableResetCredits ?? "Not reported"}. Credit identifiers never enter the WebView.</p>
    <div class="actions">
      <button type="button" disabled={!connected || !accountSupported || view.accountActions.busy || !resetCreditsCurrent || availableResetCredits === null || availableResetCredits === "0"} onclick={() => openConfirmation({ kind: "reset" })}>Use one reset credit…</button>
      {#if view.accountActions.resetRetryId}<button type="button" disabled={!connected || view.accountActions.busy} onclick={beginRetryReset}>Retry uncertain reset…</button>{/if}
    </div>
    <label>Workspace email request
      <select bind:value={nudgeType}><option value="credits">Add credits</option><option value="usage_limit">Increase usage limit</option></select>
    </label>
    <button type="button" disabled={!connected || !accountSupported || view.accountActions.busy} onclick={() => openConfirmation({ kind: "nudge", creditType: nudgeType })}>Request email…</button>
    {#if view.accountActions.resetStatus}<p role="status">{resetStatusText(view.accountActions.resetStatus)}</p>{/if}
    {#if view.accountActions.nudgeStatus}<p role="status">{nudgeStatusText(view.accountActions.nudgeStatus)}</p>{/if}
    {#if view.accountActions.error}<p role="alert">{view.accountActions.error}</p>{/if}
  </details>

  <details data-native-feedback>
    <summary>Send text-only feedback</summary>
    <p class="copy">No logs, file paths, attachments, tags or conversation content are included.{view.feedback.allowed ? "" : " Managed Codex settings currently disable this upload."}</p>
    <label>Category<select bind:value={feedbackClassification}><option value="bug">Bug</option><option value="bad_result">Incorrect result</option><option value="suggestion">Suggestion</option><option value="other">Other</option></select></label>
    <label>Feedback<textarea rows="4" maxlength="4096" bind:value={feedbackReason}></textarea></label>
    <button type="button" disabled={!connected || !requirementsLoaded || !view.feedback.allowed || view.feedback.busy || !feedbackReason.trim()} onclick={() => openConfirmation({ kind: "feedback", classification: feedbackClassification, reason: feedbackReason.trim() })}>Review and send…</button>
    {#if view.feedback.status}<p role="status">{feedbackStatusText(view.feedback.status)}</p>{/if}
    {#if view.feedback.error}<p role="alert">{view.feedback.error}</p>{/if}
  </details>

  <details data-native-sandbox-command>
    <summary>App Server sandboxed command</summary>
    <p class="copy">Separate from Supervisor Terminal. The value below is an argv JSON array, never a shell-interpolated command. Network and environment overrides are disabled; full access is unavailable.</p>
    <label>Arguments<textarea rows="3" spellcheck="false" bind:value={argvText} disabled={commandActive}></textarea></label>
    <label>Sandbox<select bind:value={commandAccess} disabled={commandActive}><option value="readOnly">Read only</option><option value="workspaceWrite">Project write</option></select></label>
    <div class="size"><label>Rows<input type="number" min="5" max="200" bind:value={rows} /></label><label>Columns<input type="number" min="20" max="500" bind:value={cols} /></label></div>
    <div class="actions">
      <button type="button" disabled={!connected || !requirementsLoaded || !view.scope.project || commandActive} onclick={beginCommand}>Run in App Server sandbox…</button>
      {#if commandWritable}<button type="button" disabled={view.command.controlBusy} onclick={resize}>Resize PTY</button><button type="button" disabled={view.command.controlBusy} onclick={beginTerminate}>Terminate…</button>{/if}
    </div>
    {#if view.command.process}
      <article class="command-result">
        <strong>{commandStatusText(view.command.process.status)}{view.command.process.exitCode === null ? "" : ` · exit ${view.command.process.exitCode}`}</strong>
        <span>{view.command.process.project}</span>
        <code>{JSON.stringify(view.command.process.argv)}</code>
        <pre aria-label="Sandboxed command output">{commandText || "No output."}</pre>
        {#if view.command.process.truncated}<p role="status">Output was truncated at the bounded display limit.</p>{/if}
      </article>
      {#if commandWritable}
        <label>Standard input<textarea rows="2" maxlength="16384" bind:value={stdinText}></textarea></label>
        <div class="actions"><button type="button" disabled={view.command.controlBusy || !stdinText} onclick={() => sendInput(false)}>Send input</button><button type="button" disabled={view.command.controlBusy} onclick={() => sendInput(true)}>Send and close stdin</button></div>
      {:else if !commandActive}<button type="button" onclick={() => view.command.process && onAction({ kind: "clear_command", process_id: view.command.process.id })}>Clear result</button>{/if}
    {/if}
    {#if view.command.notice}<p role="status">{view.command.notice}</p>{/if}
    {#if view.command.error}<p role="alert">{view.command.error}</p>{/if}
  </details>
</section>

<dialog bind:this={confirmationDialog} class="workspace-confirm-dialog" aria-label="Confirm optional App Server operation" onclose={() => pendingConfirmation = null}>
  {#if pendingConfirmation}
    <form onsubmit={(event) => { event.preventDefault(); confirm(); }}>
      <h2>{confirmationTitle(pendingConfirmation)}</h2>
      {#if pendingConfirmation.kind === "feature"}<p>This changes one process-wide runtime flag. It does not enable experimentalApi, persist arbitrary configuration or modify other flags.</p>
      {:else if pendingConfirmation.kind === "import"}<p>Codex will durably import {pendingConfirmation.itemIds.length} exact item{pendingConfirmation.itemIds.length === 1 ? "" : "s"} from the frozen preview. Existing settings or files may conflict; results and recovery history remain available.</p>
      {:else if pendingConfirmation.kind === "reset"}<p>The backend will select and redeem one available rate-limit reset credit. This changes your account allowance and cannot be undone here.</p>
      {:else if pendingConfirmation.kind === "retry_reset"}<p>The prior delivery result is unknown. This retry reuses the same idempotency key so the same logical attempt cannot consume a second credit.</p>
      {:else if pendingConfirmation.kind === "nudge"}<p>Codex will email the workspace owner about {pendingConfirmation.creditType === "credits" ? "adding credits" : "increasing the usage limit"}. The service may return a cooldown instead of sending another email.</p>
      {:else if pendingConfirmation.kind === "feedback"}<p>Category: {pendingConfirmation.classification}. Only the text shown below is uploaded. No logs, files, paths, tags or conversation are attached.</p><pre>{pendingConfirmation.reason}</pre>
      {:else if pendingConfirmation.kind === "terminate"}<p>This asks Codex to stop the separate sandboxed process. Project files already written are not rolled back.</p>
      {:else}<p>Project: {pendingConfirmation.project}</p><p>Sandbox: {pendingConfirmation.access === "readOnly" ? "read only" : "project write"}. Network access and environment overrides are disabled. This is not the Supervisor Terminal.</p><pre>{JSON.stringify(pendingConfirmation.argv)}</pre>{/if}
      <div class="dialog-actions"><button type="button" onclick={closeConfirmation}>Cancel</button><button type="submit">Confirm</button></div>
    </form>
  {/if}
</dialog>

<style>
  .p2 {display:grid;gap:var(--ca-space-2);min-width:0;margin-block:var(--ca-space-3);padding-block:var(--ca-space-2);color:var(--ca-text);}
  h3 {margin:0;font:600 var(--ca-type-body)/var(--ca-leading-body) var(--ca-font-body);}
  details {min-width:0;border-block-start:1px solid var(--ca-border);padding-block:var(--ca-space-2);}
  summary {cursor:pointer;padding-block:var(--ca-space-2);font-weight:600;}
  .copy,.scope,p {margin:var(--ca-space-2) 0;color:var(--ca-muted);overflow-wrap:anywhere;}
  .entry,.command-result {display:grid;gap:var(--ca-space-1);min-width:0;margin-block:var(--ca-space-2);padding:var(--ca-space-2);border-radius:var(--ca-control-radius);background:var(--ca-app-background);}
  .entry span,.command-result span {overflow-wrap:anywhere;color:var(--ca-muted);}
  .selectable,.choice {grid-template-columns:auto minmax(0,1fr);align-items:start;}
  .actions,.selection-actions,.size,.dialog-actions {display:flex;flex-wrap:wrap;gap:var(--ca-space-2);align-items:end;}
  .choices {display:grid;gap:var(--ca-space-1);}
  label {display:grid;gap:var(--ca-space-1);min-width:0;}
  button,input,select,textarea {font:inherit;color:var(--ca-text);background:var(--ca-app-background);border:0;border-radius:var(--ca-control-radius);padding:var(--ca-space-2);}
  input[type="checkbox"] {width:auto;margin:calc(var(--ca-space-1) / 2) 0 0;padding:0;accent-color:var(--ca-accent);}
  .choice {align-items:center;}
  .filter {margin-block:var(--ca-space-3);}
  button:disabled {color:var(--ca-muted);cursor:not-allowed;}
  button:focus-visible,input:focus-visible,select:focus-visible,textarea:focus-visible,summary:focus-visible {box-shadow:var(--ca-focus-ring);}
  textarea,select {width:100%;min-width:0;box-sizing:border-box;}
  pre,code {max-width:100%;overflow:auto;white-space:pre-wrap;overflow-wrap:anywhere;font:var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-mono);}
  .command-result pre {max-height:22rem;margin:0;padding:var(--ca-space-2);background:var(--ca-surface);}
  dialog {max-width:min(40rem,calc(100vw - 2 * var(--ca-space-4)));border:0;background:var(--ca-surface);color:var(--ca-text);}
</style>
