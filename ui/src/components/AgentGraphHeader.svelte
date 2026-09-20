<script lang="ts">
  import { onMount, tick } from "svelte";
  import CollaborationIndicator from "./CollaborationIndicator.svelte";
  import NativeUsage from "./NativeUsage.svelte";
  import SupervisorLogo from "./SupervisorLogo.svelte";

  let {
    eventTarget, owner, title="", working=false, minimized=false, focused=false, onToggleFocused, onToggleMinimized,
  }: {
    eventTarget: HTMLElement; owner: string; title?:string; working?:boolean;
    minimized?:boolean; onToggleMinimized:()=>void;
    focused?:boolean; onToggleFocused:()=>void;
  } = $props();
  let profileOpen = $state(false);
  let profileVisible = $state(false);
  let profileButton: HTMLButtonElement;
  let profilePanel: HTMLDivElement;
  let profileRevision = 0;
  let popupMotion: Animation | undefined;
  let markMotion: Animation[] = [];
  const profileId = $derived(`graph-profile-${owner}`);
  const reduced = () => window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const markPaths = () => [...profileButton.querySelectorAll<SVGPathElement>("[data-supervisor-logo] path")];

  async function setProfileOpen(next: boolean, role?: string, returnFocus = false, immediate = false): Promise<void> {
    if (next === profileOpen && !immediate) {
      if (next && role) {
        await tick();
        profilePanel?.querySelector<HTMLSelectElement>(`[data-role="${role}"]`)?.focus();
      }
      return;
    }
    const wasVisible = profileVisible && !profilePanel.hidden;
    const current = wasVisible ? getComputedStyle(profilePanel) : null;
    const pieces = markPaths();
    const transforms = pieces.map(path => getComputedStyle(path).transform);
    const generation = ++profileRevision;
    profileOpen = next;
    popupMotion?.cancel();
    markMotion.forEach(animation => animation.cancel());
    markMotion = [];
    if (next) profileVisible = true;
    profilePanel.inert = !next;
    await tick();
    if (generation !== profileRevision) return;
    profilePanel.inert = !next;
    profilePanel.dataset.phase = next ? "opening" : "closing";
    if (next && role) profilePanel.querySelector<HTMLSelectElement>(`[data-role="${role}"]`)?.focus();
    if (!next && returnFocus) profileButton.focus();
    const style = getComputedStyle(profilePanel);
    const duration = immediate || reduced() ? 0 : parseFloat(style.getPropertyValue("--ca-profile-motion-duration"));
    const finish = () => {
      if (generation !== profileRevision) return;
      profilePanel.dataset.phase = next ? "open" : "closed";
      popupMotion?.cancel();
      popupMotion = undefined;
      if (!next) {
        markMotion.forEach(animation => animation.cancel());
        markMotion = [];
        profileVisible = false;
      }
    };
    if (!duration) {
      pieces.forEach(path => { path.style.transform = "none"; });
      finish();
      return;
    }
    const split = parseFloat(style.getPropertyValue("--ca-profile-mark-split"));
    const angle = parseFloat(style.getPropertyValue("--ca-profile-mark-angle"));
    const scale = style.getPropertyValue("--ca-profile-popup-scale").trim();
    const easing = style.getPropertyValue("--ca-ease-emphasized").trim();
    markMotion = pieces.map((path, index) => {
      path.style.transform = "";
      const direction = index === 0 ? 1 : -1;
      return path.animate(
        [{ transform: transforms[index] }, { transform: next ? `translate(${direction * split}px, ${-direction * split}px) rotate(${direction * angle}deg)` : "none" }],
        { duration, easing, fill: "forwards" },
      );
    });
    popupMotion = profilePanel.animate(
      [
        { opacity: current?.opacity ?? "0", transform: wasVisible ? current?.transform ?? "none" : `scale(${scale})` },
        { opacity: next ? "1" : "0", transform: next ? "none" : `scale(${scale})` },
      ],
      { duration, easing, fill: "forwards" },
    );
    void popupMotion.finished.then(finish, () => {});
  }

  function openProfile(role = "provider"): void {
    void setProfileOpen(true, role);
  }

  function closeProfile(returnFocus = false): void {
    void setProfileOpen(false, undefined, returnFocus);
  }

  onMount(() => {
    const outsideProfile = (event: Event) => {
      if (
        profileOpen &&
        event.target instanceof Node &&
        !profilePanel.contains(event.target) &&
        !profileButton.contains(event.target)
      ) closeProfile(false);
    };
    const profileKey = (event: KeyboardEvent) => {
      if (profileOpen && event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        closeProfile(true);
      }
    };
    const modelShortcut = (event: Event) => {
      const detail = (event as CustomEvent).detail;
      if (detail?.action === "model" && (!detail.owner || detail.owner === owner)) {
        if (minimized) onToggleMinimized();
        void openProfile("model");
      }
    };
    const preference = window.matchMedia("(prefers-reduced-motion: reduce)");
    const motionChanged = () => { if (preference.matches) void setProfileOpen(profileOpen, undefined, false, true); };
    document.addEventListener("pointerdown", outsideProfile, true);
    document.addEventListener("focusin", outsideProfile);
    eventTarget.addEventListener("keydown", profileKey);
    eventTarget.addEventListener("central-agent:slash-ui", modelShortcut);
    preference.addEventListener("change", motionChanged);
    return () => {
      profileRevision++;
      popupMotion?.cancel();
      markMotion.forEach(animation => animation.cancel());
      document.removeEventListener("pointerdown", outsideProfile, true);
      document.removeEventListener("focusin", outsideProfile);
      eventTarget.removeEventListener("keydown", profileKey);
      eventTarget.removeEventListener("central-agent:slash-ui", modelShortcut);
      preference.removeEventListener("change", motionChanged);
    };
  });
</script>

<header class="agent-console-header" class:minimized>
  <span class="terminal-lights" aria-hidden="true"><i></i><i></i><i></i></span>
  {#if title}<strong class="agent-console-title" title={title}>{title}</strong>{/if}
  {#if working}<CollaborationIndicator compact label={`${title||'Agent'} is collaborating on active work`} />{/if}
  <NativeUsage {eventTarget} {owner} compact />
  <div class="agent-console-header-actions">
    <button class="agent-console-minimize" data-action="focus" type="button" aria-label={focused?'Exit focus':'Focus conversation'} title={focused?'Exit focus':'Focus conversation'} aria-pressed={focused} onclick={()=>{void setProfileOpen(false,undefined,false,true);onToggleFocused();}}><svg viewBox="0 0 24 24" aria-hidden="true"><path d={focused?'M9 3v6H3m12 12v-6h6':'M3 9V3h6m12 12v6h-6'}/></svg></button>
    <button class="agent-console-stop" data-action="remove" type="button" hidden>Remove</button>
    <button class="agent-console-minimize" data-action="minimize" type="button" aria-label={minimized?'Restore conversation':'Minimize conversation'} title={minimized?'Restore conversation':'Minimize conversation'} aria-pressed={minimized} onclick={()=>{void setProfileOpen(false,undefined,false,true);onToggleMinimized();}}>
      <svg viewBox="0 0 24 24" aria-hidden="true">{#if minimized}<path d="M8 8h9v9H8zM6 14H5V5h9v1"/>{:else}<path d="M6 12h12"/>{/if}</svg>
    </button>
    <button bind:this={profileButton} class="agent-profile-toggle" type="button" aria-label="Configure agent model" title="Provider, model, effort and speed" aria-expanded={profileOpen} aria-controls={profileId} onclick={() => { if (profileOpen) closeProfile(true); else openProfile(); }}>
      <SupervisorLogo />
    </button>
    <button class="agent-console-close" data-action="close" type="button" aria-label="Close agent window">×</button>
  </div>
  <div bind:this={profilePanel} id={profileId} class="agent-console-profile-controls" data-phase="closed" aria-hidden={!profileOpen} hidden={!profileVisible}>
    <div class="graph-profile-fields">
      <label class="agent-console-field">Provider<select data-role="provider"></select></label>
      <label class="agent-console-field">Model<select data-role="model"></select></label>
      <label class="agent-console-field">Effort<select data-role="effort"></select></label>
      <label class="agent-console-field">Speed<select data-role="speed"></select></label>
    </div>
  </div>
</header>

<!-- Stable hooks consumed by the Rust/legacy bridge. They keep old sessions and
     automation compatible while presentation belongs to Svelte. -->
<div class="agent-console-compatibility" hidden aria-hidden="true">
  <span class="agent-console-name"></span>
  <span class="agent-console-group"></span>
  <span class="agent-console-phase"></span>
  <button class="agent-console-link-button" data-action="link" type="button">Connect</button>
  <div class="agent-console-meta">
    <span data-role="directory"></span><span data-role="profile"></span>
  </div>
  <select data-role="context" aria-label="Stored context window" disabled><option value="">Automatic</option></select>
  <div class="agent-console-context" data-role="context-monitor" data-state="waiting">
    <span data-role="context-value"></span><span data-role="context-detail"></span>
    <span data-role="context-progress"><i data-role="context-fill"></i></span>
  </div>
  <div class="agent-console-assignment">
    <input data-role="name" maxlength="80" aria-label="Agent name" />
    <input data-role="mission" maxlength="2000" aria-label="Agent mission" />
    <button data-action="save" type="button">Assign</button>
  </div>
</div>

<style>
  [hidden] { display: none; }
  .agent-console-header { position: relative; display: flex; flex: 0 0 auto; min-width: 0; min-height: var(--ca-control-compact); align-items: center; padding: var(--ca-space-2) var(--ca-space-3); gap: var(--ca-space-2); background: var(--agent-card-background,var(--ca-app-background)); }
  .terminal-lights { flex: 0 0 auto; }
  .agent-console-title { min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font:600 var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body);color:var(--ca-text); }
  .agent-console-header-actions { display: flex; margin-left: auto; flex: 0 0 auto; align-items: center; flex-wrap: nowrap; gap: var(--ca-space-1); }
  .agent-profile-toggle, .agent-console-minimize, .agent-console-close { display: grid; place-items: center; flex: 0 0 auto; width: var(--ca-control-compact); height: var(--ca-control-compact); padding: var(--ca-space-1); border: 0; border-radius: var(--ca-control-radius); color: var(--ca-text); background: transparent; cursor: pointer; }
  .agent-console-minimize svg { width:var(--ca-space-5);height:var(--ca-space-5);fill:none;stroke:currentColor;stroke-width:1.6;stroke-linecap:round;stroke-linejoin:round; }
  .agent-console-header.minimized .agent-profile-toggle { display:none; }
  .agent-profile-toggle :global(.supervisor-logo) { width: var(--ca-space-6); height: var(--ca-space-6); }
  .agent-profile-toggle :global([data-supervisor-logo]) { overflow: visible; }
  .agent-profile-toggle :global([data-supervisor-logo] path) { transform-box: fill-box; transform-origin: center; }
  .agent-profile-toggle:hover, .agent-profile-toggle[aria-expanded="true"], .agent-console-minimize:hover, .agent-console-close:hover { background: var(--ca-surface-2); }
  .agent-profile-toggle:focus-visible, .agent-console-minimize:focus-visible, .agent-console-close:focus-visible { box-shadow: var(--ca-focus-ring); outline: 1px solid var(--ca-border-strong); }
  .agent-console-profile-controls { position: absolute; inset: 100% var(--ca-space-3) auto; z-index: 3; display: grid; grid-template-columns: minmax(0, 1fr); min-width: 0; max-height: min(20rem, 50vh); overflow: auto; gap: var(--ca-space-2); padding: var(--ca-space-3); border: 0; border-radius: var(--ca-radius-medium); background: var(--ca-surface); box-shadow: var(--ca-shadow-float); color: var(--ca-text); cursor: default; transform-origin: top right; will-change: opacity, transform; }
  .agent-console-profile-controls[hidden] { display: none; }
  .graph-profile-fields { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--ca-space-2); min-width: 0; }
  .graph-profile-fields .agent-console-field { min-width: 0; color: var(--ca-muted); font: var(--ca-type-caption)/var(--ca-leading-body) var(--ca-font-body); letter-spacing: normal; text-transform: none; }
  .graph-profile-fields select { min-width: 0; width: 100%; background: var(--ca-input); color: var(--ca-text); font: var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body); }
</style>
