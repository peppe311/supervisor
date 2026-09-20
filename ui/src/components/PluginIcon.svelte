<script lang="ts">
  import {appIconKind,appInitial} from "../lib/native-apps";
  let {appId,name,iconUrl=null}:{appId:string;name:string;iconUrl?:string|null}=$props();
  let failedUrl=$state<string|null>(null);
  const kind=$derived(appIconKind(appId,name));
  const initial=$derived(appInitial(name));
  const showCatalogIcon=$derived(!!iconUrl && failedUrl!==iconUrl);
  function iconFailed():void {
    failedUrl=iconUrl;
  }
</script>

<span class="plugin-icon" data-plugin-icon={kind} data-plugin-icon-source={showCatalogIcon?'catalog':'local'} aria-hidden="true">
  {#if showCatalogIcon}
    <img src={iconUrl||''} alt="" loading="lazy" decoding="async" referrerpolicy="no-referrer" onerror={iconFailed} />
  {:else if kind==="browser"}
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <rect x="2.75" y="4" width="18.5" height="16" rx="3" />
      <path d="M3 8.25h18" />
      <circle cx="6.1" cy="6.15" r=".65" fill="currentColor" stroke="none" />
      <circle cx="8.65" cy="6.15" r=".65" fill="currentColor" stroke="none" />
      <path d="m9.4 14.25 2.05 2.05 3.75-4.1" />
    </svg>
  {:else if kind==="github"}
    <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
      <path d="M12 1.8a10.4 10.4 0 0 0-3.29 20.27c.52.1.71-.22.71-.5v-1.84c-2.9.63-3.51-1.23-3.51-1.23-.48-1.2-1.16-1.52-1.16-1.52-.95-.65.07-.64.07-.64 1.05.08 1.6 1.08 1.6 1.08.94 1.6 2.45 1.14 3.05.87.1-.68.37-1.14.67-1.4-2.31-.26-4.74-1.15-4.74-5.14 0-1.13.4-2.06 1.08-2.79-.11-.26-.47-1.32.1-2.75 0 0 .88-.28 2.86 1.07A9.97 9.97 0 0 1 12 6.93c.88 0 1.75.12 2.57.35 1.98-1.35 2.85-1.07 2.85-1.07.57 1.43.21 2.49.1 2.75.67.73 1.08 1.66 1.08 2.79 0 4-2.44 4.87-4.76 5.13.38.33.71.97.71 1.95v2.74c0 .28.19.6.72.5A10.4 10.4 0 0 0 12 1.8Z" />
    </svg>
  {:else if kind==="cloudflare"}
    <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
      <path d="M6.4 18.4h11.9c.9 0 1.5-.8 1.2-1.7a3.1 3.1 0 0 0-2.9-2.1 5.5 5.5 0 0 0-10.5-.8 4 4 0 0 0-.9-.1A3.3 3.3 0 0 0 2 16.4c-.2 1 .5 2 1.6 2h2.8Z" />
      <path d="M13.2 8.1a4.8 4.8 0 0 1 4.6 3.5 4.6 4.6 0 0 1 3.5 2.2A7.1 7.1 0 0 0 8.2 11a6.2 6.2 0 0 1 5-2.9Z" opacity=".72" />
    </svg>
  {:else if kind==="figma"}
    <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
      <path d="M8.5 2h3.5v7H8.5a3.5 3.5 0 1 1 0-7Zm3.5 0h3.5a3.5 3.5 0 1 1 0 7H12V2Zm-3.5 7H12v7H8.5a3.5 3.5 0 1 1 0-7Zm3.5 0h3.5a3.5 3.5 0 1 1 0 7A3.5 3.5 0 0 1 12 12.5V9Zm-3.5 7H12v3.5A3.5 3.5 0 1 1 8.5 16Z" />
    </svg>
  {:else if kind==="linear"}
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" aria-hidden="true">
      <path d="M20.2 7.1A9.5 9.5 0 0 1 7.1 20.2M16.9 3.8 3.8 16.9M13.2 2.6 2.6 13.2M9.3 3.1 3.1 9.3" />
      <path d="M20.4 11.8A8.6 8.6 0 0 1 11.8 20.4" />
    </svg>
  {:else if kind==="notion"}
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-linejoin="round" aria-hidden="true">
      <path d="M4 4.5 15.8 3l4.2 3.2v13.1L7.2 21 4 18.1V4.5Z" stroke-width="1.6" />
      <path d="M7.7 17.3V7.5l2.3-.2 6.1 8.9V6.7l2-.2M7.7 7.5l8.4-.8" stroke-width="1.45" stroke-linecap="round" />
    </svg>
  {:else}
    <span>{initial}</span>
  {/if}
</span>

<style>
  .plugin-icon{display:grid;width:var(--ca-space-6);min-width:var(--ca-space-6);height:var(--ca-space-6);place-items:center;overflow:hidden;border-radius:var(--ca-radius-small);background:var(--ca-surface-3);color:var(--ca-text);font:700 var(--ca-type-caption)/1 var(--ca-font-body)}
  .plugin-icon svg,.plugin-icon img{display:block;width:var(--ca-space-5);height:var(--ca-space-5)}
  .plugin-icon img{object-fit:contain}
</style>
