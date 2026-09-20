<script lang="ts">
  import {accountLabel,accountStatus,canAuthenticate,emptyAppServerAccount,rateLimitWindowText,runtimeNoticeLines,runtimeNoticeTitle,type AccountAction,type AppServerAccountView} from "../lib/app-server-account";

  let {onAction}:{onAction:(action:AccountAction)=>void}=$props();
  let accountView=$state(emptyAppServerAccount());
  let logoutConfirmation:HTMLDialogElement;
  let setupConfirmation:HTMLDialogElement;
  let setupMode=$state<"elevated"|"unelevated">("elevated");
  const setupBusy=$derived(["starting","running","finishing"].includes(accountView.sandboxSetup?.phase||""));
  const canLogin=$derived(canAuthenticate(accountView));
  const chatReload=$derived(accountView.chatReload??emptyAppServerAccount().chatReload);

  export function update(view:AppServerAccountView):void {
    accountView=view;
    if(!canAuthenticate(view))logoutConfirmation?.close();
    if(!view.connected || ["starting","running","finishing"].includes(view.sandboxSetup?.phase||""))setupConfirmation?.close();
  }
  function logout():void {logoutConfirmation.close();onAction("logout_confirmed");}
  function reviewSetup(mode:"elevated"|"unelevated"):void {setupMode=mode;setupConfirmation.showModal();}
</script>

<div class="provider-card">
  <div class="provider-card-head">
    <span class="provider-card-title">Codex</span>
    <span class="provider-card-badge" role="status">{accountStatus(accountView)}</span>
  </div>
  <p class="settings-section-copy">Connect a ChatGPT subscription to use Codex in Supervisor. This sign-in and these chats are separate from the official Codex app.</p>

  {#if accountView.account}
    <p class="account-detail">{accountView.account.email||accountLabel(accountView.account.type)}{accountView.account.planType?` · ${accountView.account.planType}`:""}</p>
    {#if accountView.account.policy}<p class="settings-section-copy" data-native-account-policy>{accountView.account.policy}</p>{/if}
  {/if}

  {#if !accountView.connected || !accountView.account}
    <div class="provider-card-actions">
      {#if !accountView.connected}<button class="action" type="button" disabled={accountView.connecting||accountView.authBusy||!!accountView.login} onclick={()=>onAction("connect")}>{accountView.connecting?"Connecting…":"Connect Codex"}</button>{/if}
      {#if !accountView.account}<button class="action primary" type="button" disabled={!canLogin} onclick={()=>onAction("login_browser")}>Sign in with ChatGPT</button>{/if}
    </div>
  {/if}

  {#if accountView.login}
    <div class="login-instructions">
      {#if accountView.login.userCode}<p>Enter this code on the OpenAI sign-in page:</p><code class="device-code">{accountView.login.userCode}</code>
      {:else}<p>Continue in your browser to authorize this connection.</p>{/if}
      <div class="provider-card-actions">
        <button class="action primary" type="button" onclick={()=>onAction("open_login")}>Open sign-in page</button>
        <button class="action" type="button" disabled={accountView.authBusy} onclick={()=>onAction("cancel_login")}>Cancel sign-in</button>
      </div>
    </div>
  {/if}

  {#each Object.entries(accountView.errors) as [key,error] (key)}
    <p class="account-error" role="alert">{error}</p>
  {/each}

  {#if accountView.runtimeNotices.length}
    <section class="runtime-notices" aria-labelledby="runtime-notices-title" data-native-runtime-notices>
      <h3 id="runtime-notices-title">Connection notices</h3>
      {#each accountView.runtimeNotices as notice (notice.sequence)}
        <article class="runtime-notice">
          <strong>{runtimeNoticeTitle(notice)}{notice.current?"":" · from the previous connection"}</strong>
          {#each runtimeNoticeLines(notice) as line}<span>{line}</span>{/each}
        </article>
      {/each}
    </section>
  {/if}

  {#if accountView.connected}
    <details data-native-rate-limits>
      <summary>Subscription usage &amp; limits</summary>
      {#if !accountView.account?.supported}<p class="settings-section-copy">Usage limits are available only for a supported ChatGPT subscription.</p>
      {:else if accountView.rateLimits.refreshing}<p class="settings-section-copy" role="status">Refreshing usage limits…</p>
      {:else if !accountView.rateLimits.loaded}<p class="settings-section-copy">Usage limits have not been reported yet.</p>
      {:else}
        {#if !accountView.rateLimits.current}<p class="settings-section-copy">This snapshot is stale. Refresh the account before relying on it.</p>{/if}
        {#each accountView.rateLimits.buckets as bucket (bucket.id)}
          <div class="rate-limit-bucket">
            <strong>{bucket.name||bucket.id}</strong>
            <span>Primary: {bucket.primary?rateLimitWindowText(bucket.primary):"Not reported"}</span>
            <span>Secondary: {bucket.secondary?rateLimitWindowText(bucket.secondary):"Not reported"}</span>
          </div>
        {/each}
      {/if}
    </details>
  {/if}

  <details data-native-chat-reload>
    <summary>Saved chats</summary>
    <p class="settings-section-copy">Saved chats load automatically when Supervisor opens. Reload them manually only if a chat is missing.</p>
    <button class="action" type="button" disabled={accountView.connecting||accountView.authBusy||!!accountView.login||chatReload.loading} onclick={()=>onAction("reload_chats")}>{chatReload.loading?"Loading chats…":"Reload chats"}</button>
    <p class="settings-section-copy" role="status" aria-live="polite">
      {#if chatReload.loading}
        Loading saved chats… {chatReload.loaded+chatReload.unavailable+chatReload.skipped} / {chatReload.total}
      {:else if chatReload.checked && chatReload.total===0}
        No saved Codex chats to reload.
      {:else if chatReload.checked}
        {chatReload.loaded} {chatReload.loaded===1?"chat loaded.":"chats loaded."}
        {#if chatReload.unavailable}{chatReload.unavailable} {chatReload.unavailable===1?"history unavailable.":"histories unavailable."} Existing links were kept.{/if}
        {#if chatReload.skipped}{chatReload.skipped} {chatReload.skipped===1?"conversation left unchanged.":"conversations left unchanged."} Reload again after current work finishes.{/if}
      {/if}
    </p>
  </details>

  <details data-settings-account-options>
    <summary>Account options</summary>
    <div class="provider-card-actions">
      {#if accountView.connected}<button class="action" type="button" disabled={accountView.connecting||accountView.authBusy||!!accountView.login} onclick={()=>onAction("connect")}>Reconnect</button>{/if}
      <button class="action" type="button" disabled={!accountView.connected||accountView.refreshing||accountView.authBusy} onclick={()=>onAction("refresh")}>Refresh account</button>
      <button class="action" type="button" disabled={!canLogin} onclick={()=>onAction("login_device")}>Sign in with a device code</button>
      {#if accountView.account}<button class="action" type="button" disabled={!canLogin} onclick={()=>logoutConfirmation.showModal()}>Sign out…</button>{/if}
    </div>
    <section class="repair-option" data-native-sandbox-setup>
      <strong>Repair Codex on Windows</strong>
      <p>Use this only if Codex cannot run commands or edit a project. Setup prepares its protected Windows environment and does not grant full access.</p>
      {#if accountView.sandboxSetup?.detail}<p role="status">{accountView.sandboxSetup.detail}</p>{/if}
      {#if accountView.connected && !accountView.requirementsLoaded}<p>Refresh the account before running repair.</p>{/if}
      <div class="provider-card-actions">
        <button type="button" disabled={!accountView.connected||!accountView.requirementsLoaded||setupBusy} onclick={()=>reviewSetup("elevated")}>Repair with administrator access…</button>
        <button type="button" disabled={!accountView.connected||!accountView.requirementsLoaded||setupBusy} onclick={()=>reviewSetup("unelevated")}>Repair without elevation…</button>
      </div>
    </section>
  </details>
</div>

<dialog bind:this={setupConfirmation} class="workspace-confirm-dialog" aria-label="Repair Codex on Windows">
  <h2>Repair Codex on Windows?</h2>
  <p>{setupMode==="elevated"?"Codex will prepare its protected Windows environment. Windows may ask for administrator approval. This does not enable full agent access.":"Codex will run its legacy setup without requesting elevation. Availability depends on your Windows configuration."}</p>
  <div class="workspace-confirm-actions"><button type="button" onclick={()=>setupConfirmation.close()}>Cancel</button><button type="button" onclick={()=>{setupConfirmation.close();onAction(setupMode==="elevated"?"setup_sandbox_elevated":"setup_sandbox_unelevated");}}>Start repair</button></div>
</dialog>

<dialog bind:this={logoutConfirmation} class="workspace-confirm-dialog" aria-labelledby="app-server-logout-title" aria-describedby="app-server-logout-description">
  <h2 id="app-server-logout-title">Sign out of Codex?</h2>
  <p id="app-server-logout-description">Sign out of Codex in Supervisor. The official Codex app keeps its own sign-in. Project files and conversations are not deleted.</p>
  <div class="workspace-confirm-actions">
    <button type="button" onclick={()=>logoutConfirmation.close()}>Cancel</button>
    <button class="workspace-confirm-remove" type="button" onclick={logout}>Sign out of Codex</button>
  </div>
</dialog>

<style>
  .account-detail,.account-error,.login-instructions {font-size:var(--ca-type-body);line-height:var(--ca-leading-body);overflow-wrap:anywhere;}
  .account-detail,.account-error {margin:var(--ca-space-3) 0;}
  .account-error {color:var(--ca-text);}
  .login-instructions {padding-block:var(--ca-space-3);}
  .device-code {display:block;margin-bottom:var(--ca-space-3);font-family:var(--ca-font-mono);font-size:var(--ca-type-title);user-select:all;}
  .rate-limit-bucket {display:grid;gap:var(--ca-space-1);padding-block:var(--ca-space-3);font-size:var(--ca-type-body);overflow-wrap:anywhere;}
  .rate-limit-bucket span {color:var(--ca-muted);}
  .runtime-notices {display:grid;gap:var(--ca-space-2);margin-block:var(--ca-space-4);}
  .runtime-notices h3 {margin:0;font-size:var(--ca-type-label);}
  .runtime-notice {display:grid;gap:var(--ca-space-1);padding:var(--ca-space-2);border-radius:var(--ca-radius-small);background:var(--ca-surface-2);font-size:var(--ca-type-caption);overflow-wrap:anywhere;}
  .runtime-notice span,.repair-option p {color:var(--ca-muted);}
  .repair-option {display:grid;gap:var(--ca-space-2);margin-top:var(--ca-space-6);padding-top:var(--ca-space-4);border-top:1px solid var(--ca-border);}
  .repair-option p {margin:0;font-size:var(--ca-type-body);line-height:var(--ca-leading-body);}
  dialog {border:0;background:var(--ca-surface);color:var(--ca-text);}
</style>
