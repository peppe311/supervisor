<script lang="ts">
  import { untrack } from "svelte";
  import type { AgentTimelineMessage } from "../lib/agent-timeline";
  let { initial }: { initial:AgentTimelineMessage } = $props();
  let message=$state(untrack(() => initial));
  export function update(value:AgentTimelineMessage):void { message=value; }
</script>

{#each message.nativeQuestionReply?.entries || [] as entry}
  <div class="reply-entry">
    <div class="question" title={entry.question}>{entry.question}</div>
    <div class="answer">{entry.answer}</div>
  </div>
{/each}

<style>
  :global(.chat-message.user.message.native-question-reply) {
    justify-self:end; align-self:flex-end; width:fit-content; max-width:var(--ca-chat-user-max-width);
    margin:var(--ca-space-2) 0 var(--ca-space-4) auto;
    padding:var(--ca-space-3) var(--ca-space-4); border:0;
    border-radius:var(--ca-chat-user-radius); background:var(--ca-chat-user-background); color:var(--ca-text);
    font:calc(var(--ca-type-body) + var(--ca-chat-font-offset, 0px))/var(--ca-leading-body) var(--ca-font-body);
  }
  .reply-entry { min-width:0; }
  .reply-entry + .reply-entry { margin-top:var(--ca-space-3); }
  .question { color:var(--ca-muted); white-space:nowrap; overflow:hidden; text-overflow:ellipsis; margin-bottom:var(--ca-space-1); }
  .answer { white-space:pre-wrap; overflow-wrap:anywhere; }
</style>
