import type { AgentTimelineMessage, AgentTimelineRunState } from "./agent-timeline";
export interface GraphRenderHooks {
  decorate: (element:HTMLElement)=>void;
  artifacts: (items:unknown[])=>HTMLElement;
  checkpoint: (value:unknown)=>HTMLElement;
}

export interface GraphTurn {
  runId: number; request?: string; provider?: string; startedAtMs?: number;
  finishedAtMs?: number | null; phase?: string; status?: string;
  messages?: AgentTimelineMessage[]; checkpoint?: unknown;
  steps?: {label?: string; kind?: string; status?: string; detail?: string}[];
}
export interface GraphHistory {
  conversationState?: {nativeMessages?: AgentTimelineMessage[]};
  history?: GraphTurn[]; historyOffset?: number; historyTotal?: number;
  messages?: AgentTimelineMessage[]; agent?: AgentTimelineRunState;
}

export function graphTimelineMessages(run: GraphHistory): AgentTimelineMessage[] {
  const result: AgentTimelineMessage[] = [];
  for (const turn of run.history || []) {
    if (turn.finishedAtMs == null && String(turn.runId) === String(run.agent?.runId)) continue;
    const messages = (turn.messages || []).map(message => ({...message, runId:turn.runId, provider:message.provider || turn.provider}));
    if (!messages.some(message => message.role === "user") && turn.request) {
      messages.unshift({id:`request:${turn.runId}`, role:"user", kind:"message", text:turn.request, runId:turn.runId, provider:turn.provider, timestampMs:turn.startedAtMs});
    }
    // Legacy summaries have no native activity rows. Do not duplicate steps when
    // the authoritative item stream already includes them.
    if (!messages.some(message => message.kind === "activity")) {
      const legacy = (turn.steps || []).map((step, index) => ({id:`step:${turn.runId}:${index}`, runId:turn.runId, role:"assistant", kind:"activity", text:step.label || "Action", activityStatus:step.status, activityDetail:step.detail, activityCategory:"tool", timestampMs:turn.startedAtMs, provider:turn.provider}));
      let answer = -1;
      messages.forEach((message, index) => { if (message.role === "assistant" && (message.kind || "message") === "message" && message.messagePhase !== "commentary") answer = index; });
      messages.splice(answer < 0 ? messages.length : answer, 0, ...legacy);
    }
    if (turn.status && !messages.some(message => message.role !== "user")) {
      messages.push({id:`status:${turn.runId}`, runId:turn.runId, role:"system", text:turn.status, provider:turn.provider, timestampMs:turn.finishedAtMs ?? turn.startedAtMs});
    }
    result.push(...messages);
    if (turn.checkpoint && !messages.some(message => message.checkpoint)) {
      result.push({id:`checkpoint:${turn.runId}`, role:"system", text:"", checkpoint:turn.checkpoint});
    }
  }
  const seen = new Set(result.map(message => String(message.id)));
  for (const message of [...(run.messages || []), ...(run.conversationState?.nativeMessages || [])]) {
    if (!seen.has(String(message.id))) { result.push(message); seen.add(String(message.id)); }
  }
  return result;
}
