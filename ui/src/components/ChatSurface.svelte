<script lang="ts">
  import { onMount } from "svelte";
  import Composer from "./Composer.svelte";
  import FileEditor from "./FileEditor.svelte";
  import NativeRequests from "./NativeRequests.svelte";

  let messages: HTMLDivElement;
  onMount(() => {
    const composer = messages.parentElement?.querySelector<HTMLElement>("#composer");
    if (!composer) return;
    let frame = 0;
    // Share the actual composer edges, including the space consumed by a native
    // scrollbar. Separate percentage widths drift as chat starts scrolling.
    const align = () => {
      frame = 0;
      if (!composer.offsetWidth || !messages.offsetWidth) return;
      const prompt = composer.getBoundingClientRect(), chat = messages.getBoundingClientRect();
      const left = chat.left + messages.clientLeft;
      messages.style.paddingLeft = `${Math.max(0, prompt.left - left)}px`;
      messages.style.paddingRight = `${Math.max(0, left + messages.clientWidth - prompt.right)}px`;
    };
    const observer = new ResizeObserver(() => { if (!frame) frame = requestAnimationFrame(align); });
    observer.observe(messages); observer.observe(composer);
    align();
    return () => { observer.disconnect(); cancelAnimationFrame(frame); };
  });
</script>

<FileEditor />

<div class="conversation-toolbar" hidden aria-hidden="true">
  <span class="conversation-title">Agent</span>
  <span id="conversation-workspace" class="conversation-workspace">No workspace connected</span>
  <span id="conversation-state" class="conversation-state">Idle</span>
</div>
<div id="chat-messages" class="chat-messages" bind:this={messages}></div>
<NativeRequests />


<Composer />

<style>
  :global(.conversation-section .chat-messages .chat-message.user.message) {
    align-self:flex-end; justify-self:end;
    width:fit-content; max-width:var(--ca-chat-user-max-width);
    margin:var(--ca-space-2) 0 var(--ca-space-4) auto;
    padding:var(--ca-space-3) var(--ca-space-4); border:0;
    border-radius:var(--ca-chat-user-radius);
    background:var(--ca-chat-user-background); color:var(--ca-text);
    font-size:calc(var(--ca-agent-chat-font-size, var(--ca-type-body)) + var(--ca-chat-font-offset));
    font-weight:400; text-align:start;
  }
</style>
