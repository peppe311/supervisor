<script lang="ts">
  import { onMount } from "svelte";
  import ConfigurationSection from "./ConfigurationSection.svelte";
  import NativeHistory from "./NativeHistory.svelte";
  import NativeSkills from "./NativeSkills.svelte";
  import NativePreferences from "./NativePreferences.svelte";
  import NativeGoal from "./NativeGoal.svelte";
  import NativeApps from "./NativeApps.svelte";
  import type {AppsAction} from "../lib/native-apps";
  import type {NativeConversationView as View} from "../lib/native-history";
  import type {ReviewTarget} from "../../../protocol/app-server/0.153.4/typescript/v2/ReviewTarget";
  type Operation="rename"|"archive"|"unarchive"|"delete"|"new_fork"|"start_review"|"reset_unused"|"compact";
  let {owner:fixedOwner,eventTarget,settingsVisible=true}:{owner?:string;eventTarget?:HTMLElement;settingsVisible?:boolean}=$props();
  let owner=$state(""),view=$state<View|null>(null),error=$state(""),pending=$state(false);
  let operation=$state<Operation>("rename"),name=$state("");
  let reviewKind=$state<ReviewTarget["type"]>("uncommittedChanges"),reviewValue=$state("");
  let reviewDelivery=$state<"inline"|"detached">("inline"),reviewName=$state("");
  let forkTurnId=$state("");
  let dialog:HTMLDialogElement;
  let confirmedOwner="",confirmedThread="";
  let confirmedDirectory=$state("");
  onMount(()=>{
    owner=fixedOwner||"";
    const switchOwner=(event:Event)=>{
      if(fixedOwner)return;
      const next=String((event as CustomEvent).detail||"");
      if(next!==owner){owner=next;view=null;error="";pending=false;dialog?.close();}
    };
    const update=(event:Event)=>{
      const d=(event as CustomEvent).detail;
      if(d?.owner!==owner)return;
      const wasConnected=!!view?.connected;
      if(d.nativeConversation)view=d.nativeConversation;
      if(!view?.connected)pending=false;
      if(event.type==="central-agent:app-server-conversation") {
        if(d.error || d.change && d.change!=="stream")pending=false;
        if(d.error)error=d.error;
        else if(d.change && d.change!=="stream")error="";
      } else if(!wasConnected && view?.connected) {
        // A connection-required action error is no longer actionable after the
        // account reconnects. Native operation failures still persist until an
        // operation event resolves or replaces them.
        error="";
      }
      if(!view?.visible || !view.connected || view.binding?.deleted || view.binding?.threadId!==confirmedThread || (operation==="new_fork"||operation==="start_review") && view.binding?.archived)dialog?.close();
    };
    const target=eventTarget||window;
    const command=(event:Event)=>{
      const detail=(event as CustomEvent).detail;
      if(detail?.owner!==owner || !view?.visible)return;
      const operations:Record<string,Operation>={rename:"rename",archive:"archive",unarchive:"unarchive",fork:"new_fork",review:"start_review",compact:"compact"};
      if(detail.command==="history") {if(canRead()){read();event.preventDefault();}return;}
      const operation=Object.hasOwn(operations,detail.command)?operations[detail.command]:undefined;
      if(operation && begin(operation))event.preventDefault();
    };
    window.addEventListener("central-agent:native-command",command);
    window.addEventListener("central-agent:conversation-key",switchOwner);
    window.addEventListener("central-agent:app-server-conversation",update);
    target.addEventListener("central-agent:conversation-state",update);
    return ()=>{window.removeEventListener("central-agent:native-command",command);window.removeEventListener("central-agent:conversation-key",switchOwner);window.removeEventListener("central-agent:app-server-conversation",update);target.removeEventListener("central-agent:conversation-state",update);};
  });
  function allowed():boolean {return Boolean(view?.visible && view.connected && view.binding && !view.binding.deleted && !view.busy && !pending);}
  function canRead():boolean {return Boolean(view?.visible && view.connected && view.binding && !view.binding.deleted && !pending);}
  function begin(kind:Operation):boolean {
    if(!allowed() || !view?.binding || !view.observed && kind!=="unarchive" && kind!=="reset_unused")return false;
    if(kind==="reset_unused" && !view.unusedLink)return false;
    if(kind==="compact" && !view.canCompact)return false;
    if((kind==="start_review"||kind==="new_fork") && !view.directory)return false;
    if((kind==="start_review"||kind==="new_fork") && view.binding.archived)return false;
    operation=kind;name=kind==="new_fork"?`Fork of ${view.name||"conversation"}`:view.name||"";forkTurnId="";reviewDelivery="inline";reviewName=`Review of ${view.name||"conversation"}`;confirmedOwner=owner;confirmedThread=view.binding.threadId;confirmedDirectory=view.directory||"";
    dialog.showModal();
    return true;
  }
  function send(action:Record<string,unknown>):void {
    error="";pending=true;
    (eventTarget||document).dispatchEvent(new CustomEvent("central-agent:app-server-conversation-control",{bubbles:true,detail:{owner,action}}));
  }
  function confirm():void {
    if(!allowed() || owner!==confirmedOwner || view?.binding?.threadId!==confirmedThread)return;
    if(operation==="reset_unused" && !view.unusedLink)return;
    if(operation==="compact" && !view.canCompact)return;
    if((operation==="new_fork"||operation==="start_review") && view.binding.archived){dialog.close();return;}
    if((operation==="rename"||operation==="new_fork") && !name.trim())return;
    if((operation==="new_fork"||operation==="start_review") && (!confirmedDirectory || confirmedDirectory!==view.directory)){error="The project changed. Confirm the operation again.";dialog.close();return;}
    if(operation==="start_review") {
      const target=reviewTarget();if(!target)return;
      if(reviewDelivery==="detached" && (view.historyMode!=="legacy" || !reviewName.trim()))return;
      dialog.close();send({kind:operation,expected_thread_id:confirmedThread,expected_directory:confirmedDirectory,target,delivery:reviewDelivery,destination_name:reviewDelivery==="detached"?reviewName.trim():null});return;
    }
    dialog.close();
    send({kind:operation,expected_thread_id:confirmedThread,...(operation==="rename"||operation==="new_fork"?{name:name.trim()}:{}),...(operation==="new_fork"?{expected_directory:confirmedDirectory,last_turn_id:forkTurnId||null}:{})});
  }
  function reviewTarget():ReviewTarget|null {
    if(reviewKind==="uncommittedChanges")return {type:reviewKind};
    const value=reviewValue.trim();if(!value || value.includes("\0"))return null;
    if(reviewKind==="baseBranch")return {type:reviewKind,branch:value};
    if(reviewKind==="commit")return {type:reviewKind,sha:value,title:null};
    return {type:"custom",instructions:value};
  }
  function read():void {if(canRead())send({kind:"read"});}
  function controlApps(action:AppsAction):void {
    if(!view?.visible || !view.connected || view.binding?.deleted || view.binding?.archived)return;
    if(action.kind==='attach' && (!view.apps?.current || view.apps.inventoryId!==action.inventory_id))return;
    (eventTarget||document).dispatchEvent(new CustomEvent("central-agent:app-server-conversation-control",{bubbles:true,detail:{owner,action:{kind:"apps_control",expected_thread_id:view.binding?.threadId??null,action}}}));
  }
  function separateReviewNotice():string {
    return view?.historyMode==="paginated"
      ? "This Codex version does not support separate review delivery for paginated histories. To keep the review separate, explicitly create a fork, open it, then start an inline review there. These are two distinct native actions."
      : view?.historyMode==="legacy"
        ? "Separate delivery creates a new local conversation and binds only the reviewThreadId returned by Codex. It is never inferred from the source thread."
        : "Codex did not report a compatible history mode, so separate delivery stays unavailable. Refresh native history or use an explicit fork followed by inline review.";
  }
</script>

<div class="native-configuration" hidden={!view?.visible}>
  <NativeHistory {owner} {eventTarget} conversation={view} dialogOnly />
  <NativeGoal {owner} {eventTarget} conversation={view} dialogOnly />
  <NativePreferences {owner} {eventTarget} conversation={view} dialogOnly />
  {#if settingsVisible}
  <ConfigurationSection section="tools" label="Codex Skills & Apps" scope="Current conversation" icon="tools" initiallyOpen collapsible={false}>
      <NativeSkills {owner} {eventTarget} conversation={view} />
      {#if view?.visible && (!view.binding || !view.binding.deleted)}
        {#key `${owner}:${view.binding?.threadId||"draft"}`}<NativeApps view={view.apps} connected={view.connected && !view.binding?.archived} onAction={controlApps} />{/key}
      {:else if view?.visible}
        <p>Connect this conversation to Codex to choose skills and apps.</p>
      {/if}
    </ConfigurationSection>
  {:else}
    <NativeSkills {owner} {eventTarget} conversation={view} dialogOnly />
  {/if}
  {#if !view?.connected}<p>Connect Codex in AI accounts to use native conversation actions.</p>{:else if pending || view.busy}<p role="status">Waiting for the current operation.</p>{/if}
  {#if view?.compaction}<p role="status">{view.compaction==="stop_requested"?"Stop requested. Waiting for a native turn to interrupt.":view.compaction==="running"?"Codex is compacting this context; follow native progress in the conversation.":view.compaction==="accepted"?"Compaction accepted. Waiting for native activity; it is not complete yet.":"Requesting native context compaction…"}</p>{/if}
  {#if error}<p role="alert">{error}</p>{/if}
</div>
<dialog bind:this={dialog} class="workspace-confirm-dialog" aria-label="Manage Codex conversation">
  <form onsubmit={(event)=>{event.preventDefault();confirm();}}>
    <h2>{operation==="compact"?"Compact native context?":operation==="reset_unused"?"Reset unused Codex connection?":operation==="start_review"?"Review code with Codex":operation==="new_fork"?"Fork into a new conversation?":operation==="rename"?"Rename Codex conversation":operation==="archive"?"Archive Codex conversation?":operation==="unarchive"?"Restore Codex conversation?":"Delete Codex conversation?"}</h2>
    {#if operation==="compact"}
      <p>Ask Codex to condense this conversation's model context. Details available to future turns may be summarized. Codex owns this operation and may use your account allowance.</p>
      <p>This does not reset the chat, restore files or create a Time Machine checkpoint. Your draft, attachments and other conversations remain unchanged. An acknowledgement means started, not completed; progress follows native events. Stop requests interruption when Codex reports the active turn.</p>
    {:else if operation==="reset_unused"}
      <p>Supervisor has not submitted a turn through this connection. If Codex has no saved history for it after reconnecting, you can remove this unused local link.</p>
      <p>No Codex history or files are deleted. Any existing native history remains available through Browse Codex history. Your draft, attachments and other-provider messages stay here. Your next prompt will start a new native conversation.</p>
    {:else if operation==="start_review"}
      <label>Review target<select bind:value={reviewKind}><option value="uncommittedChanges">Uncommitted changes</option><option value="baseBranch">Compare with branch</option><option value="commit">Specific commit</option><option value="custom">Custom instructions</option></select></label>
      {#if reviewKind!=="uncommittedChanges"}<label>{reviewKind==="baseBranch"?"Base branch":reviewKind==="commit"?"Commit SHA":"Review instructions"}<textarea bind:value={reviewValue} rows={3} required></textarea></label>{/if}
      <label>Review destination<select bind:value={reviewDelivery}><option value="inline">This conversation</option><option value="detached" disabled={view?.historyMode!=="legacy"}>New separate conversation</option></select></label>
      {#if reviewDelivery==="detached"}<label>Separate conversation name<input bind:value={reviewName} required /></label>{/if}
      <p>Directory: {confirmedDirectory}</p>
      <p>This starts Codex's native reviewer in this conversation and uses your account allowance. The session is prepared in read-only mode with native command approvals. The configured Codex reviewer chooses its model; this is not a normal composer submission.</p>
      <p>Your draft and attachments stay unchanged. Stop remains available while reviewing. No checkpoint or file restore is created.</p>
      <p>{separateReviewNotice()}</p>
    {:else if operation==="new_fork"}
      <label>New conversation name<input bind:value={name} required /></label>
      <label>History boundary<select bind:value={forkTurnId}><option value="">Entire observed history</option>{#each view?.completedTurns||[] as turn,index (turn.id)}<option value={turn.id}>Through turn {index+1} · {turn.status}</option>{/each}</select></label>
      <p>Codex will copy its history into a new native thread, linked to a new project chat or a second agent card on the same graph node. No files, worktree or other-provider messages are copied. No prompt is sent.</p>
      <p>Directory: {confirmedDirectory}</p><p>The original draft stays here. The new conversation uses Supervisor's shared Codex workspace access. Use Open branch when you want to switch.</p>
    {:else if operation==="rename"}
      <label>Conversation name<input bind:value={name} required /></label>
    {:else if operation==="archive"}
      <p>Codex will archive this conversation and attempt to archive its spawned child conversations. It can be restored later. No project files are deleted.</p>
    {:else if operation==="unarchive"}
      <p>Restore this conversation's native history. Child conversations are not automatically restored. No files are restored or changed.</p>
    {:else}
      <p>Permanently delete this conversation and its spawned child conversations from Codex, including archived children. This cannot be undone. Project files, the local chat and other providers' histories are not deleted.</p>
      <p>Codex may refuse deletion while a fork still references this history. A refusal keeps the conversation available; forks are never deleted automatically to bypass it.</p>
    {/if}
    <div class="workspace-confirm-actions">
      <button type="button" onclick={()=>dialog.close()}>Cancel</button>
      <button type="submit" aria-label={operation==="compact"?"Start compaction":operation==="reset_unused"?"Reset local link":operation==="start_review"?"Start review":operation==="new_fork"?"Create fork":operation==="rename"?"Save name":operation==="archive"?"Archive":operation==="unarchive"?"Restore conversation":"Permanently delete"} disabled={!allowed() || (operation==="rename"||operation==="new_fork") && !name.trim() || (operation==="new_fork"||operation==="start_review") && view?.binding?.archived || operation==="start_review" && (!reviewTarget() || reviewDelivery==="detached" && (view?.historyMode!=="legacy" || !reviewName.trim())) || operation==="reset_unused" && !view?.unusedLink || operation==="compact" && !view?.canCompact}>{operation==="compact"?"Start compaction":operation==="reset_unused"?"Reset local link":operation==="start_review"?"Start review":operation==="new_fork"?"Create fork":operation==="rename"?"Save name":operation==="archive"?"Archive":operation==="unarchive"?"Restore conversation":"Permanently delete"}</button>
    </div>
  </form>
</dialog>

<style>
  .native-configuration {display:grid;min-width:0;gap:var(--ca-space-2);color:var(--ca-text);font:var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body);}
  .native-configuration[hidden] {display:none;}
  p {margin:var(--ca-space-2) 0;color:var(--ca-muted);overflow-wrap:anywhere;}
  button,input,select,textarea {font:inherit;color:var(--ca-text);background:var(--ca-app-background);border:0;border-radius:var(--ca-control-radius);padding:var(--ca-space-2);}
  button:disabled {color:var(--ca-muted);cursor:not-allowed;}
  button:focus-visible,input:focus-visible,select:focus-visible,textarea:focus-visible {box-shadow:var(--ca-focus-ring);}
  label {display:grid;gap:var(--ca-space-2);}
  input,select,textarea {width:100%;min-width:0;box-sizing:border-box;}
  dialog {border:0;background:var(--ca-surface);color:var(--ca-text);}
</style>
