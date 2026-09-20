<script lang="ts">
  import NativeUsage from "./NativeUsage.svelte";
  let nativeVisible=$state(false);
</script>
<section
  id="context-window-monitor"
  class="context-window-monitor"
  data-state="waiting"
  aria-label="Context window usage"
  hidden
  aria-hidden="true"
  inert
>
  <div class="legacy-usage" hidden={nativeVisible}>
  <div class="context-window-head">
    <span>Context usage</span>
    <span id="context-window-value" class="context-window-value" aria-live="polite"
      >Waiting for usage</span
    >
  </div>
  <div
    id="context-window-progress"
    class="context-window-progress"
    role="progressbar"
    aria-label="Context window used"
    aria-valuemin="0"
    aria-valuemax="100"
    aria-valuenow="0"
    aria-valuetext="Token usage is unavailable for this provider"
  >
    <span id="context-window-fill" class="context-window-fill"></span>
  </div>
  <span id="context-window-detail" class="context-window-detail"
    >Live usage is unavailable for the remaining provider integrations.</span
  >
  </div>
  <NativeUsage bind:visible={nativeVisible} />
</section>
<style>.legacy-usage:not([hidden]) {display:contents;}</style>
