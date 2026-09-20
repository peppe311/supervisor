export interface GraphBinding {
  recordType: string; recordId: string; conversationId?: string | null; name?: string;
}
export function graphConversationKey(binding: GraphBinding): string {
  return binding.conversationId ? `conversation:${binding.conversationId}` : `${binding.recordType}:${binding.recordId}`;
}
export function graphConversationTarget(key: string, bindings: GraphBinding[]) {
  const binding = bindings.find(binding => graphConversationKey(binding) === key);
  return binding ? { record_type:binding.recordType, id:binding.recordId,
    conversation_id:binding.conversationId || null, graphNodeKey:`${binding.recordType}:${binding.recordId}` } : null;
}
export function graphConversationOptions(graphNodeKey: string, bindings: GraphBinding[]) {
  return bindings.filter(binding => `${binding.recordType}:${binding.recordId}` === graphNodeKey)
    .map(binding => ({ key:graphConversationKey(binding), label:`${binding.name || "Agent"} · ${binding.conversationId ? `Chat ${binding.conversationId.slice(0, 8)}` : "Original"}` }));
}

export function graphWindowSelection(open: Iterable<string>, source: string, target: string, replaceCurrent: boolean): Set<string> {
  const next = new Set(open);
  if (replaceCurrent && source !== target) next.delete(source);
  next.add(target);
  return next;
}
