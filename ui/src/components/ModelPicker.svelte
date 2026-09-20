<script lang="ts">
  interface PickerOption {
    label: string;
    value: string;
  }

  interface Props {
    kind: "provider" | "model" | "effort" | "speed" | "personality" | "context";
    label: string;
    description?: string;
    initialValue: string;
    options?: PickerOption[];
  }

  let { kind, label, description = "", initialValue, options = [] }: Props = $props();

  const labelId = $derived(`${kind}-picker-label`);
  const selectId = $derived(`${kind}-select`);
  const buttonId = $derived(`${kind}-picker-button`);
  const valueId = $derived(`${kind}-picker-value`);
  const menuId = $derived(`${kind}-picker-menu`);
</script>

<div class="model-field" data-picker={kind}>
  <span class="model-field-copy">
    <span id={labelId} class="model-label">{label}</span>
    {#if description}<span id={`${kind}-picker-description`} class="model-description">{description}</span>{/if}
  </span>
  <select id={selectId} class="model-select" aria-hidden="true" tabindex="-1">
    {#if options.length}
      {#each options as option (option.value)}
        <option value={option.value}>{option.label}</option>
      {/each}
    {:else}
      <option>{initialValue}</option>
    {/if}
  </select>
  <button
    id={buttonId}
    class="model-picker-button"
    type="button"
    aria-haspopup="listbox"
    aria-expanded="false"
    aria-controls={menuId}
    aria-labelledby={`${labelId} ${valueId}`}
    aria-describedby={description ? `${kind}-picker-description` : undefined}
  >
    <span id={valueId} class="model-picker-value">{initialValue}</span>
    <span class="model-picker-chevron" aria-hidden="true"></span>
  </button>
  <div id={menuId} class="model-picker-menu" role="listbox" aria-labelledby={labelId} hidden></div>
</div>

<style>
  .model-field-copy {display:grid;min-width:0;gap:var(--ca-space-1);}
  .model-description {color:var(--ca-muted);font-size:var(--ca-type-caption);line-height:var(--ca-leading-body);overflow-wrap:anywhere;}
</style>
