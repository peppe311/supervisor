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
  type ProfileRole = "provider" | "model" | "effort" | "speed";
  type ProfilePicker = {
    role: ProfileRole;
    field: HTMLDivElement;
    select: HTMLSelectElement;
    button: HTMLButtonElement;
    value: HTMLSpanElement;
    menu: HTMLDivElement;
    signature: string;
  };
  const profileRoles: { role: ProfileRole; label: string }[] = [
    { role: "provider", label: "Provider" },
    { role: "model", label: "Model" },
    { role: "effort", label: "Effort" },
    { role: "speed", label: "Speed" },
  ];
  let profilePickers: ProfilePicker[] = [];
  let profileRevision = 0;
  let popupMotion: Animation | undefined;
  let markMotion: Animation[] = [];
  const profileId = $derived(`graph-profile-${owner}`);
  const reduced = () => window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const markPaths = () => [...profileButton.querySelectorAll<SVGPathElement>("[data-supervisor-logo] path")];

  function closePicker(picker: ProfilePicker, returnFocus = false): void {
    if (picker.menu.hidden) return;
    picker.menu.hidden = true;
    picker.button.setAttribute("aria-expanded", "false");
    if (returnFocus) picker.button.focus();
  }

  function closeAllPickers(except?: ProfilePicker): void {
    profilePickers.forEach(picker => { if (picker !== except) closePicker(picker); });
  }

  function syncPicker(picker: ProfilePicker): void {
    const options = [...picker.select.options];
    const selected = options.find(option => option.value === picker.select.value) || options[0];
    const disabled = picker.select.disabled || !options.length;
    const signature = JSON.stringify([disabled, selected?.value, options.map(option => [option.value, option.textContent, option.title])]);
    if (picker.signature === signature) return;
    picker.signature = signature;
    picker.button.disabled = disabled;
    picker.value.textContent = selected?.textContent || "—";
    picker.button.title = selected?.title || selected?.textContent || "";
    if (disabled) closePicker(picker);
    const hadOptionFocus = picker.menu.contains(document.activeElement);
    picker.menu.replaceChildren(...options.map(option => {
      const active = option.value === selected?.value;
      const row = document.createElement("button");
      row.type = "button";
      row.className = `model-picker-option${active ? " is-selected" : ""}`;
      row.setAttribute("role", "option");
      row.setAttribute("aria-selected", String(active));
      row.dataset.value = option.value;
      const copy = document.createElement("span");
      const title = document.createElement("span");
      title.className = "model-picker-option-title";
      title.textContent = option.textContent;
      copy.append(title);
      if (option.title) {
        const description = document.createElement("span");
        description.className = "model-picker-option-description";
        description.textContent = option.title;
        copy.append(description);
      }
      const check = document.createElement("span");
      check.className = "model-picker-check";
      check.setAttribute("aria-hidden", "true");
      check.textContent = "✓";
      row.append(copy, check);
      row.addEventListener("click", () => {
        const changed = picker.select.value !== option.value;
        picker.select.value = option.value;
        syncPicker(picker);
        closePicker(picker, true);
        if (changed) picker.select.dispatchEvent(new Event("change", { bubbles: true }));
      });
      return row;
    }));
    if (hadOptionFocus && !picker.menu.hidden) picker.menu.querySelector<HTMLButtonElement>('[aria-selected="true"]')?.focus();
  }

  function openPicker(picker: ProfilePicker, focusSelected = false): void {
    syncPicker(picker);
    if (picker.button.disabled) return;
    const shouldOpen = picker.menu.hidden;
    closeAllPickers();
    if (!shouldOpen) return;
    picker.menu.hidden = false;
    picker.button.setAttribute("aria-expanded", "true");
    requestAnimationFrame(() => {
      if (picker.menu.hidden) return;
      const selected = picker.menu.querySelector<HTMLButtonElement>('[aria-selected="true"]');
      selected?.scrollIntoView({ block: "nearest" });
      if (focusSelected) selected?.focus();
    });
  }

  function movePickerFocus(picker: ProfilePicker, step: number): void {
    const options = [...picker.menu.querySelectorAll<HTMLButtonElement>(".model-picker-option")];
    if (!options.length) return;
    const current = options.indexOf(document.activeElement as HTMLButtonElement);
    const next = current < 0 ? step > 0 ? 0 : options.length - 1 : (current + step + options.length) % options.length;
    options[next].focus();
  }

  function focusProfileRole(role: string): void {
    profilePickers.find(picker => picker.role === role)?.button.focus();
  }

  async function setProfileOpen(next: boolean, role?: string, returnFocus = false, immediate = false): Promise<void> {
    if (next === profileOpen && !immediate) {
      if (next && role) {
        await tick();
        focusProfileRole(role);
      }
      return;
    }
    const wasVisible = profileVisible && !profilePanel.hidden;
    const current = wasVisible ? getComputedStyle(profilePanel) : null;
    const pieces = markPaths();
    const transforms = pieces.map(path => getComputedStyle(path).transform);
    const generation = ++profileRevision;
    profileOpen = next;
    if (!next) closeAllPickers();
    popupMotion?.cancel();
    markMotion.forEach(animation => animation.cancel());
    markMotion = [];
    if (next) profileVisible = true;
    profilePanel.inert = !next;
    await tick();
    if (generation !== profileRevision) return;
    profilePanel.inert = !next;
    profilePanel.dataset.phase = next ? "opening" : "closing";
    if (next) {
      profilePickers.forEach(syncPicker);
      if (role) focusProfileRole(role);
    }
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
    const observers: MutationObserver[] = [];
    profilePickers = profileRoles.map(({ role }) => {
      const field = profilePanel.querySelector<HTMLDivElement>(`[data-picker="${role}"]`)!;
      const picker: ProfilePicker = {
        role, field,
        select: field.querySelector<HTMLSelectElement>("select")!,
        button: field.querySelector<HTMLButtonElement>(".model-picker-button")!,
        value: field.querySelector<HTMLSpanElement>(".model-picker-value")!,
        menu: field.querySelector<HTMLDivElement>(".model-picker-menu")!,
        signature: "",
      };
      const sync = () => syncPicker(picker);
      picker.select.addEventListener("change", sync);
      picker.select.addEventListener("central-agent:profile-sync", sync);
      const observer = new MutationObserver(sync);
      observer.observe(picker.select, { childList: true, subtree: true, attributes: true, attributeFilter: ["disabled", "title", "label", "value"] });
      observers.push(observer);
      picker.button.addEventListener("click", () => openPicker(picker));
      picker.button.addEventListener("keydown", event => {
        if (event.key === "ArrowDown" || event.key === "ArrowUp") {
          event.preventDefault();
          if (picker.menu.hidden) openPicker(picker, true);
          else movePickerFocus(picker, event.key === "ArrowDown" ? 1 : -1);
        } else if (event.key === "Escape" && !picker.menu.hidden) {
          event.preventDefault(); event.stopPropagation(); closePicker(picker, true);
        }
      });
      picker.menu.addEventListener("keydown", event => {
        if (event.key === "ArrowDown" || event.key === "ArrowUp") {
          event.preventDefault(); movePickerFocus(picker, event.key === "ArrowDown" ? 1 : -1);
        } else if (event.key === "Home" || event.key === "End") {
          event.preventDefault();
          const options = picker.menu.querySelectorAll<HTMLButtonElement>(".model-picker-option");
          (event.key === "Home" ? options[0] : options[options.length - 1])?.focus();
        } else if (event.key === "Escape") {
          event.preventDefault(); event.stopPropagation(); closePicker(picker, true);
        }
      });
      sync();
      return picker;
    });
    const outsidePicker = (event: Event) => {
      if (!(event.target instanceof Node)) return;
      profilePickers.forEach(picker => { if (!picker.field.contains(event.target as Node)) closePicker(picker); });
    };
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
    document.addEventListener("pointerdown", outsidePicker, true);
    document.addEventListener("focusin", outsidePicker);
    eventTarget.addEventListener("keydown", profileKey);
    eventTarget.addEventListener("central-agent:slash-ui", modelShortcut);
    preference.addEventListener("change", motionChanged);
    return () => {
      profileRevision++;
      popupMotion?.cancel();
      markMotion.forEach(animation => animation.cancel());
      observers.forEach(observer => observer.disconnect());
      document.removeEventListener("pointerdown", outsideProfile, true);
      document.removeEventListener("focusin", outsideProfile);
      document.removeEventListener("pointerdown", outsidePicker, true);
      document.removeEventListener("focusin", outsidePicker);
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
      {#each profileRoles as { role, label }}
        <div class="agent-console-field model-field" data-picker={role}>
          <span id={`${profileId}-${role}-label`} class="model-label">{label}</span>
          <select class="graph-native-select" data-role={role} aria-hidden="true" tabindex="-1"></select>
          <button class="model-picker-button" data-profile-trigger={role} type="button" aria-haspopup="listbox" aria-expanded="false" aria-controls={`${profileId}-${role}-menu`} aria-labelledby={`${profileId}-${role}-label ${profileId}-${role}-value`}>
            <span id={`${profileId}-${role}-value`} class="model-picker-value">—</span>
            <span class="model-picker-chevron" aria-hidden="true"></span>
          </button>
          <div id={`${profileId}-${role}-menu`} class="model-picker-menu" role="listbox" aria-labelledby={`${profileId}-${role}-label`} hidden></div>
        </div>
      {/each}
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
  .graph-profile-fields { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: var(--ca-space-3); min-width: 0; }
  .graph-profile-fields .agent-console-field { display: grid; min-width: 0; gap: var(--ca-space-1); color: var(--ca-muted); font: var(--ca-type-caption)/var(--ca-leading-body) var(--ca-font-body); letter-spacing: normal; text-transform: none; }
  .graph-profile-fields .agent-console-field[data-picker="provider"], .graph-profile-fields .agent-console-field[data-picker="model"] { grid-column: 1 / -1; }
  .graph-profile-fields .graph-native-select { display: none; }
  .model-picker-button { display: grid; width: 100%; min-width: 0; min-height: var(--ca-control-default); grid-template-columns: minmax(0, 1fr) 10px; align-items: center; gap: var(--ca-space-2); padding: var(--ca-space-2); border: 0; border-radius: var(--ca-control-radius); background: var(--ca-surface-3); color: var(--ca-text); font: var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body); text-align: left; cursor: pointer; }
  .model-picker-button:hover:not(:disabled) { background: var(--ca-surface-2); }
  .model-picker-button:focus-visible, .graph-profile-fields :global(.model-picker-button[aria-expanded="true"]) { box-shadow: var(--ca-focus-ring); }
  .model-picker-button:disabled { color: var(--ca-faint); cursor: default; }
  .model-picker-value { min-width: 0; overflow-wrap: anywhere; }
  .model-picker-chevron { width: 7px; height: 7px; border-right: 1.5px solid currentColor; border-bottom: 1.5px solid currentColor; transform: rotate(45deg) translateY(-2px); }
  .graph-profile-fields :global(.model-picker-button[aria-expanded="true"] .model-picker-chevron) { transform: rotate(225deg) translateY(-2px); }
  .model-picker-menu { display: grid; width: 100%; min-width: 0; max-height: var(--ca-command-menu-max-height); gap: var(--ca-space-1); overflow: auto; overscroll-behavior: contain; padding: var(--ca-space-1); border: 0; border-radius: var(--ca-radius-medium); background: var(--ca-surface-2); scrollbar-color: var(--ca-scrollbar-thumb) var(--ca-scrollbar-track); }
  .model-picker-menu[hidden] { display: none; }
  .model-picker-menu :global(.model-picker-option) { display: grid; width: 100%; grid-template-columns: minmax(0, 1fr) 18px; align-items: center; gap: var(--ca-space-2); padding: var(--ca-space-2); border: 0; border-radius: var(--ca-radius-small); background: transparent; color: var(--ca-text); font: var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body); text-align: left; cursor: pointer; }
  .model-picker-menu :global(.model-picker-option:hover), .model-picker-menu :global(.model-picker-option:focus-visible) { background: var(--ca-surface-3); outline: none; }
  .model-picker-menu :global(.model-picker-option.is-selected) { background: var(--ca-accent); color: var(--ca-on-accent); }
  .model-picker-menu :global(.model-picker-option-title) { display: block; overflow-wrap: anywhere; font-weight: 650; }
  .model-picker-menu :global(.model-picker-option-description) { display: block; margin-top: var(--ca-space-1); color: var(--ca-muted); font-size: var(--ca-type-caption); line-height: var(--ca-leading-body); overflow-wrap: anywhere; }
  .model-picker-menu :global(.model-picker-option.is-selected .model-picker-option-description) { color: inherit; opacity: .82; }
  .model-picker-menu :global(.model-picker-check) { display: grid; width: 18px; height: 18px; place-items: center; border-radius: var(--ca-radius-pill); opacity: 0; }
  .model-picker-menu :global(.model-picker-option.is-selected .model-picker-check) { opacity: 1; background: var(--ca-on-accent); color: var(--ca-accent); }
</style>
