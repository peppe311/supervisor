// Shared components consume a private EventTarget for each graph card. Provider
// responses remain owner-addressed; they never borrow the selected main chat.
import { graphDrafts, mainDrafts } from "./persisted-drafts.ts";
export const conversationReplyEvents = ["central-agent:conversation-error", "central-agent:submission-accepted", "central-agent:graph-files-result", "central-agent:graph-contexts-result", "central-agent:project-diff-result", "central-agent:app-server-conversation"] as const;

export function acceptsConversationEvent(event: string, detail: unknown, owner: string): boolean {
  if (!owner || !detail || typeof detail !== "object") return false;
  return (detail as { owner?: unknown }).owner === owner;
}

const draftsFor=(owner:string)=>owner.startsWith("chat:")?mainDrafts:graphDrafts;
export function graphDraft(owner: string, persistent = true): string { const drafts=draftsFor(owner);return persistent ? drafts.synchronize(owner) : drafts.readLocal(owner); }
export function graphDraftStatus(owner:string):string {return draftsFor(owner).status(owner);}
export function retainGraphDraft(owner: string, text: string, persistent = true): void {
  if (owner.startsWith("graph:") || owner.startsWith("chat:")) {
    const drafts=draftsFor(owner);
    if (persistent) drafts.write(owner, text);
    else drafts.writeLocal(owner, text);
  }
}
export function prefillGraphDraft(owner: string, text: string): boolean {
  if (!owner.startsWith("graph:") || (graphDraft(owner) && graphDraft(owner) !== text)) return false;
  retainGraphDraft(owner, text); return true;
}

export function acknowledgeGraphDraft(detail: { owner?: unknown; input?: unknown }): void {
  if (typeof detail.owner === "string" && detail.owner.startsWith("graph:") && typeof detail.input === "string"
    && graphDraft(detail.owner).trim() === detail.input.trim()) retainGraphDraft(detail.owner, "");
}

export function graphDeliveryIntent(active: boolean, supportsSteer: boolean, requested: "start" | "steer" | "queue"): "choose" | "start" | "steer" | "queue" {
  if (!active) return "start";
  if (requested === "start") return "choose";
  return requested === "steer" && supportsSteer ? "steer" : "queue";
}

export function bridgeConversationEvents(source: EventTarget, target: EventTarget, owner: string): () => void {
  const forward = (event: Event) => {
    const detail = (event as CustomEvent).detail;
    if (acceptsConversationEvent(event.type, detail, owner)) target.dispatchEvent(new CustomEvent(event.type, {detail}));
  };
  for (const event of conversationReplyEvents) source.addEventListener(event, forward);
  return () => { for (const event of conversationReplyEvents) source.removeEventListener(event, forward); };
}
