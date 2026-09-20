<script lang="ts">
  import { onMount, tick } from "svelte";

  interface PickerOption {
    value: string;
    label: string;
    description?: string;
    disabled?: boolean;
  }

  let {
    id,
    label,
    value,
    options,
    disabled = false,
    presentation = "settings",
    contained = false,
    description,
    describedBy,
    onSelect,
  }: {
    id: string;
    label: string;
    value: string;
    options: PickerOption[];
    disabled?: boolean;
    presentation?: "settings" | "workspace";
    contained?: boolean;
    description?: string;
    describedBy?: string;
    onSelect: (value: string) => void;
  } = $props();

  let field: HTMLDivElement;
  let button: HTMLButtonElement;
  let menu: HTMLDivElement;
  let open = $state(false);
  const selected = () => options.find(option => option.value === value) || options[0];
  const available = () => options.filter(option => !option.disabled);

  $effect(() => {
    if ((disabled || !available().length) && open) open = false;
  });

  function close(restoreFocus = false): void {
    if (!open) return;
    open = false;
    if (restoreFocus) void tick().then(() => button?.focus());
  }

  function openMenu(focusSelected = false): void {
    if (disabled || !available().length) return;
    open = true;
    if (!focusSelected) return;
    void tick().then(() => {
      const current = menu?.querySelector<HTMLElement>('[aria-selected="true"]');
      (current || menu?.querySelector<HTMLElement>('[role="option"]:not([aria-disabled="true"])'))?.focus();
    });
  }

  function toggle(): void {
    if (open) close();
    else openMenu();
  }

  function choose(next: string): void {
    const option = options.find(candidate => candidate.value === next);
    if (disabled || !option || option.disabled) return;
    close(true);
    if (next !== value) onSelect(next);
  }

  function nativeChange(event: Event): void {
    const select = event.currentTarget as HTMLSelectElement;
    const next = select.value;
    select.value = value;
    if (next !== value) onSelect(next);
  }

  function buttonKey(event: KeyboardEvent): void {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      openMenu(true);
    } else if (event.key === "Escape" && open) {
      event.preventDefault();
      event.stopPropagation();
      close(true);
    }
  }

  function menuKey(event: KeyboardEvent): void {
    const rows = [...menu.querySelectorAll<HTMLButtonElement>('[role="option"]:not(:disabled)')];
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      close(true);
      return;
    }
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key) || !rows.length) return;
    event.preventDefault();
    const current = rows.indexOf(document.activeElement as HTMLButtonElement);
    const next = event.key === "Home" ? 0 : event.key === "End" ? rows.length - 1 : current < 0
      ? event.key === "ArrowDown" ? 0 : rows.length - 1
      : (current + (event.key === "ArrowDown" ? 1 : -1) + rows.length) % rows.length;
    rows[next].focus();
  }

  onMount(() => {
    const outside = (event: PointerEvent) => {
      if (open && event.target instanceof Node && !field.contains(event.target)) close();
    };
    document.addEventListener("pointerdown", outside);
    return () => document.removeEventListener("pointerdown", outside);
  });
</script>

<div bind:this={field} class="settings-picker settings-choice-row model-field" class:workspace={presentation === "workspace"} class:contained data-settings-picker={id} data-picker-presentation={presentation}>
  <span class="settings-picker-copy">
    <span id={`${id}-picker-label`} class="settings-picker-label model-label">{label}</span>
    {#if description}<span id={`${id}-picker-description`} class="settings-picker-description">{description}</span>{/if}
  </span>
  <select id={id} class="model-select settings-native-select" aria-hidden="true" tabindex="-1" {disabled} value={value} onchange={nativeChange}>
    {#each options as option (option.value)}
      <option value={option.value} disabled={option.disabled}>{option.label}</option>
    {/each}
  </select>
  <button
    bind:this={button}
    id={`${id}-picker-button`}
    class="model-picker-button settings-picker-button"
    type="button"
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-controls={`${id}-picker-menu`}
    aria-labelledby={`${id}-picker-label ${id}-picker-value`}
    aria-describedby={[description ? `${id}-picker-description` : "", describedBy || ""].filter(Boolean).join(" ") || undefined}
    title={selected()?.description || selected()?.label || ""}
    {disabled}
    onclick={toggle}
    onkeydown={buttonKey}
  >
    <span id={`${id}-picker-value`} class="model-picker-value">{selected()?.label || "—"}</span>
    <span class="model-picker-chevron" aria-hidden="true"></span>
  </button>
  <div bind:this={menu} id={`${id}-picker-menu`} class="model-picker-menu settings-picker-menu" role="listbox" aria-labelledby={`${id}-picker-label`} tabindex="-1" hidden={!open} onkeydown={menuKey}>
    {#each options as option (option.value)}
      <button class="model-picker-option" class:is-selected={option.value === value} type="button" role="option" aria-selected={option.value === value} aria-disabled={option.disabled} disabled={option.disabled} onclick={() => choose(option.value)}>
        <span>
          <span class="model-picker-option-title">{option.label}</span>
          {#if option.description}<span class="model-picker-option-description">{option.description}</span>{/if}
        </span>
        <span class="model-picker-check" aria-hidden="true">✓</span>
      </button>
    {/each}
  </div>
</div>

<style>
  .workspace { position: relative; display: grid; width: 100%; min-width: 0; gap: var(--ca-space-2); }
  .workspace .settings-picker-copy { display: grid; min-width: 0; gap: var(--ca-space-1); }
  .workspace .settings-picker-label { display: block; margin: 0; color: var(--ca-muted); font: 550 var(--ca-type-caption)/var(--ca-leading-compact) var(--ca-font-body); letter-spacing: 0; text-transform: none; }
  .workspace .settings-picker-description { color: var(--ca-muted); font: 400 var(--ca-type-caption)/var(--ca-leading-body) var(--ca-font-body); }
  .workspace .settings-native-select { display: none; }
  .workspace .settings-picker-button { display: grid; width: 100%; height: var(--ca-control-default); min-width: 0; grid-template-columns: minmax(0, 1fr) var(--ca-space-3); align-items: center; gap: var(--ca-space-2); padding: 0 var(--ca-space-3); border: 1px solid var(--ca-border); border-radius: var(--ca-control-radius); outline: 0; background: var(--ca-input); color: var(--ca-text); box-shadow: none; font: 550 var(--ca-type-label)/1 var(--ca-font-body); text-align: left; cursor: pointer; }
  .workspace .settings-picker-button:hover:not(:disabled) { border-color: var(--ca-border-strong); background: var(--ca-surface-2); }
  .workspace .settings-picker-button:focus-visible,
  .workspace .settings-picker-button[aria-expanded="true"] { border-color: var(--ca-border-strong); box-shadow: var(--ca-focus-ring); }
  .workspace .settings-picker-button:disabled { color: var(--ca-faint); background: var(--ca-surface-2); cursor: default; }
  .workspace .model-picker-value { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .workspace .model-picker-chevron { width: 7px; height: 7px; margin-top: -3px; border-right: 1.5px solid currentColor; border-bottom: 1.5px solid currentColor; transform: rotate(45deg); transition: transform var(--ca-duration-fast) var(--ca-ease-standard), margin var(--ca-duration-fast) var(--ca-ease-standard); }
  .workspace .settings-picker-button[aria-expanded="true"] .model-picker-chevron { margin-top: 4px; transform: rotate(225deg); }
  .workspace .settings-picker-menu { position: absolute; top: calc(100% + var(--ca-space-2)); right: 0; left: 0; z-index: 50; display: grid; width: 100%; max-height: var(--ca-command-menu-max-height); gap: var(--ca-space-1); margin: 0; padding: var(--ca-space-2); overflow-x: hidden; overflow-y: auto; overscroll-behavior: contain; border: 1px solid var(--ca-border); border-radius: var(--ca-radius-medium); background: var(--ca-surface); box-shadow: var(--ca-shadow-float); animation: workspace-picker-in var(--ca-duration-fast) var(--ca-ease-standard); }
  .workspace .settings-picker-menu[hidden] { display: none; }
  .workspace.contained .settings-picker-menu { position: static; box-sizing: border-box; }
  .workspace .model-picker-option { display: grid; width: 100%; min-height: var(--ca-control-default); grid-template-columns: minmax(0, 1fr) 18px; align-items: center; gap: var(--ca-space-2); padding: var(--ca-space-2) var(--ca-space-3); border: 0; border-radius: var(--ca-control-radius); outline: 0; background: transparent; color: var(--ca-text); text-align: left; cursor: pointer; }
  .workspace .model-picker-option:hover:not(:disabled),
  .workspace .model-picker-option:focus-visible { background: var(--ca-surface-2); }
  .workspace .model-picker-option.is-selected { background: var(--ca-accent); color: var(--ca-on-accent); }
  .workspace .model-picker-option-title { display: block; overflow: hidden; font: 550 var(--ca-type-label)/var(--ca-leading-compact) var(--ca-font-body); text-overflow: ellipsis; white-space: nowrap; }
  .workspace .model-picker-option-description { display: block; margin-top: var(--ca-space-1); color: inherit; font: 400 var(--ca-type-caption)/var(--ca-leading-body) var(--ca-font-body); white-space: normal; }
  .workspace .model-picker-check { display: grid; width: 18px; height: 18px; place-items: center; border-radius: var(--ca-radius-pill); color: transparent; font-size: var(--ca-type-caption); font-weight: 750; }
  .workspace .model-picker-option.is-selected .model-picker-check { background: var(--ca-on-accent); color: var(--ca-accent); }
  @keyframes workspace-picker-in { from { opacity: 0; transform: translateY(-4px) scale(.985); } to { opacity: 1; transform: none; } }
  @media (prefers-reduced-motion: reduce) {
    .workspace .model-picker-chevron { transition: none; }
    .workspace .settings-picker-menu { animation: none; }
  }
</style>
