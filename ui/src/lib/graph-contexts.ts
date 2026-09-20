export interface TabSnapshot {
  tabId: number; title: string; url: string; status: string; error?: string;
  previewDataUrl?: string; estimatedTokenCount: number; stale: boolean; truncated: boolean;
}
export interface ShellSnapshot {
  sessionId: number; label: string; cwd: string; outputPreview: string;
  followLive: boolean; available: boolean; estimatedTokenCount: number; truncated: boolean;
}
export interface ContextDraft { tabs: TabSnapshot[]; shells: ShellSnapshot[] }
export function contextDraft(value: unknown): ContextDraft {
  if (!value || typeof value !== "object") return {tabs:[],shells:[]};
  const v = value as Partial<ContextDraft>;
  return {tabs:Array.isArray(v.tabs) ? v.tabs : [],shells:Array.isArray(v.shells) ? v.shells : []};
}
export function contextIds(draft: ContextDraft): {tabIds:number[];shellIds:number[]} {
  return {tabIds:draft.tabs.map(tab=>tab.tabId),shellIds:draft.shells.map(shell=>shell.sessionId)};
}
export function droppedContext(transfer: Pick<DataTransfer,"getData">): {kind:"tab"|"shell";id:number}|null {
  for (const [kind, types, prefix] of [
    ["tab",["application/x-central-agent-tab"],"central-agent-tab:"],
    ["shell",["application/x-central-agent-terminal-context","application/x-central-agent-terminal"],"central-agent-shell:"]
  ] as const) {
    const text = transfer.getData("text/plain").trim();
    const value = types.map(type=>transfer.getData(type).trim()).find(Boolean) || (text.startsWith(prefix) ? text.slice(prefix.length) : "");
    if (/^\d+$/.test(value) && Number.isSafeInteger(Number(value)) && Number(value)>0) return {kind,id:Number(value)};
  }
  return null;
}
export function safeSnapshotImage(value?: string): string | undefined {
  return value && /^data:image\/jpeg;base64,[A-Za-z0-9+/]+={0,2}$/.test(value) ? value : undefined;
}
