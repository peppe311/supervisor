import { flushSync, mount, unmount } from "svelte";
import WorkDisclosure from "../components/WorkDisclosure.svelte";
import ActivityDisclosure from "../components/ActivityDisclosure.svelte";
import QuestionReply from "../components/QuestionReply.svelte";
import InputDeliveryStatus from "../components/InputDeliveryStatus.svelte";
import { createAgentTimelineController, type AgentTimelineMessage } from "./agent-timeline";

export function createWorkTimeline(...[target, renderer, options = {}]: Parameters<typeof createAgentTimelineController>) {
  const replies = new Map<HTMLElement, {update:(value:AgentTimelineMessage)=>void}>();
  const deliveries = new Map<HTMLElement, {update:(value:AgentTimelineMessage)=>void}>();
  const timeline = createAgentTimelineController(target, {
    createMessageRow(message) {
      if (!message.nativeQuestionReply?.entries.length) {
        const row = renderer.createMessageRow(message);
        if (message.submissionStatus) flushSync(() => deliveries.set(row, mount(InputDeliveryStatus, {target:row,props:{initial:message}})));
        return row;
      }
      const row=document.createElement("article");
      row.className="chat-message user message native-question-reply";
      row.dataset.messageId=String(message.id);
      flushSync(() => replies.set(row,mount(QuestionReply,{target:row,props:{initial:message}})));
      return row;
    },
    patchMessageRow(row,message) {
      const reply=replies.get(row);
      if (reply) flushSync(() => reply.update(message));
      else renderer.patchMessageRow(row,message);
      const delivery=deliveries.get(row);
      if (delivery) flushSync(() => delivery.update(message));
    },
  }, {...options, mountActivityDisclosure(group, title, meta) {
    group.dataset.activityDisclosure = "true";
    const component = flushSync(() => mount(ActivityDisclosure, {target:group, props:{title,meta}}));
    return () => { void unmount(component); };
  }, mountWorkDisclosure(group, title, history) {
    group.dataset.workDisclosure = "true";
    const component = flushSync(() => mount(WorkDisclosure, {target:group, props:{group,title,history}}));
    return () => { void unmount(component); };
  }});
  const prune=(all=false) => { for (const components of [replies,deliveries]) for (const [row,component] of components) if(all || !target.contains(row)) { void unmount(component);components.delete(row); } };
  return {...timeline, render(...args:Parameters<typeof timeline.render>) { timeline.render(...args);prune(); }, destroy() { timeline.destroy();prune(true); }};
}
