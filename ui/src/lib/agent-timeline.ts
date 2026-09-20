import { animatePromptArrival, consumePromptSendMotion, transferPromptArrival } from "./prompt-send-motion.ts";

export interface AgentTimelineMessage {
  id?: number | string;
  role?: string;
  kind?: string;
  text?: string;
  renderedHtml?: string | null;
  streaming?: boolean;
  provider?: string;
  messagePhase?: string | null;
  submissionStatus?: "sending" | "accepted";
  nativeClientMessageId?: string;
  activityStatus?: string;
  activityCategory?: string;
  activityDetail?: string;
  activityContext?: string;
  activityAdditions?: number | null;
  activityDeletions?: number | null;
  activityDiff?: string | null;
  activityToolName?: string | null;
  activityFileCount?: number | null;
  nativeWorkHistory?: NativeWorkHistory | null;
  nativeQuestionReply?: { entries: { question:string; answer:string }[] } | null;
  runId?: number | string | null;
  timestampMs?: number | string;
  nativeTurnId?: string | null;
  nativeGoalPrompt?: boolean | null;
  nativeMedia?: unknown;
  nativeTurnTiming?: { startedAtMs?: number | null; completedAtMs?: number | null; durationMs?: number | null } | null;
  attachments?: unknown[];
  terminalAttachments?: unknown[];
  fileAttachments?: unknown[];
  artifacts?: unknown[];
  checkpoint?: unknown;
  [key: string]: unknown;
}

export interface NativeWorkHistory {
  owner: string;
  threadId: string;
  turnId: string;
  state: "unloaded" | "loading" | "loaded" | "error" | "live";
  error?: string | null;
}

export interface AgentTimelineRenderer {
  createMessageRow(message: AgentTimelineMessage): HTMLElement;
  patchMessageRow(row: HTMLElement, message: AgentTimelineMessage): void;
}

export function finalOutputIndex(output: AgentTimelineMessage[]): number {
  for (let index = output.length - 1; index >= 0; index -= 1) {
    const message = output[index];
    if (message.role === "assistant" && (message.kind || "message") === "message" && message.messagePhase !== "commentary") return index;
  }
  for (let index = output.length - 1; index >= 0; index -= 1) {
    if (!isNativeServiceNotice(output[index]) && !isNativeMedia(output[index]) && output[index].messagePhase !== "commentary" && !["activity", "reasoning"].includes(output[index].kind || "message")) return index;
  }
  return -1;
}

export function isNativeServiceNotice(message: AgentTimelineMessage): boolean {
  return message.provider === "codex_app_server" && message.kind === "service_notice";
}

export function isNativeMedia(message: AgentTimelineMessage): boolean {
  return message.provider === "codex_app_server" && message.kind === "native_media";
}

function isWorkEvent(message: AgentTimelineMessage): boolean {
  return message.provider === "codex_app_server" && message.kind === "activity" && message.activityCategory === "compaction";
}

function sameNativeTurn(first:AgentTimelineMessage, second:AgentTimelineMessage): boolean {
  return first.provider === "codex_app_server" && second?.provider === "codex_app_server"
    && Boolean(first.nativeTurnId) && first.nativeTurnId === second.nativeTurnId && first.runId === second.runId;
}

export interface AgentTimelineController {
  render(messages?: AgentTimelineMessage[], runState?: AgentTimelineRunState): void;
  update(update: AgentTimelineMessage): boolean;
  pauseFollowing(): void;
  destroy(): void;
}

export interface AgentTimelineRunState {
  active?: boolean;
  runId?: number | string | null;
  phase?: string;
}

type ChatZoomAction = "increase" | "decrease" | "reset";

declare global {
  interface Window {
    centralAgentApplyChatZoom?: (action: ChatZoomAction) => void;
    centralAgentSetChatZoom?: (value: number) => void;
  }
}

interface ActivityViewState {
  open: boolean;
  scrollTop: number;
  followEnd: boolean;
}

interface ActivitySummaryInput {
  category: string;
  status: string;
  streaming: boolean;
  toolName?: string | null;
  fileCount?: number | null;
}

interface ActivitySummary {
  title: string;
  meta: string;
  live: boolean;
  failed: number;
}

interface WorkGroupViewState {
  open: boolean;
  wasLive: boolean;
}

interface WorkGroupData {
  key: string;
  messages: AgentTimelineMessage[];
  live: boolean;
  startedAtMs: number;
  finishedAtMs: number;
  recordedDurationMs?: number | null;
  history?: NativeWorkHistory | null;
}

interface TimelineItem {
  type: "message" | "activity_group" | "work_group";
  key: string;
  signature: string;
  message?: AgentTimelineMessage;
  messages?: AgentTimelineMessage[];
  work?: WorkGroupData;
}

const renderFields = [
  "id",
  "role",
  "kind",
  "text",
  "renderedHtml",
  "streaming",
  "provider",
  "activityStatus",
  "activityCategory",
  "activityDetail",
  "activityContext",
  "activityAdditions",
  "activityDeletions",
  "activityDiff",
  "runId",
  "messagePhase",
  "submissionStatus",
  "timestampMs",
  "nativeTurnId",
  "nativeGoalPrompt",
  "nativeMedia",
  "nativeTurnTiming",
  "nativeWorkHistory",
  "nativeQuestionReply",
  "activityToolName",
  "activityFileCount",
  "attachments",
  "terminalAttachments",
  "fileAttachments",
  "artifacts",
  "checkpoint",
] as const;

const CHAT_ZOOM_STORAGE_KEY = "central-agent-chat-zoom";
const CHAT_ZOOM_MIN = -2;
const CHAT_ZOOM_MAX = 8;

function nearScrollEnd(element: HTMLElement | null, threshold = 48): boolean {
  return !element || element.scrollHeight - element.scrollTop - element.clientHeight <= threshold;
}

function activityIsLive(status: string, streaming: boolean): boolean {
  return streaming || status === "queued" || status === "awaiting_approval" || status === "running";
}

export function activityGroupCopy(items: ActivitySummaryInput[]): ActivitySummary {
  if (!items.length) return {title:"Riepiloghi del ragionamento",meta:"",live:false,failed:0};
  const hasCommands = items.some((item) => item.category === "command");
  const files = items.filter((item) => item.category === "file" || item.category === "file_change");
  const hasFiles = files.length > 0;
  const hasReads = items.some((item) => item.category === "read");
  const live = items.some((item) => activityIsLive(item.status, item.streaming));

  let title: string;
  if (hasReads && hasFiles && hasCommands) {
    title = live
      ? "Sta leggendo e modificando file ed eseguendo comandi"
      : "Ha letto e modificato file e ha eseguito comandi";
  } else if (hasReads && hasCommands) {
    title = live
      ? "Sta leggendo file ed eseguendo comandi"
      : "Ha letto i file e ha eseguito comandi";
  } else if (hasFiles && hasCommands) {
    title = live
      ? "Sta modificando file ed eseguendo comandi"
      : "Ha modificato file e ha eseguito comandi";
  } else if (hasReads && hasFiles) {
    title = live ? "Sta leggendo e modificando file" : "Ha letto e modificato file";
  } else if (hasFiles) {
    title = live ? "Sta modificando file" : "Ha modificato file";
  } else if (hasReads) {
    title = live ? "Sta leggendo i file" : "Ha letto i file";
  } else if (hasCommands) {
    title = live ? "Sta eseguendo comandi" : "Ha eseguito comandi";
  } else {
    title = live ? "Sta usando gli strumenti" : "Ha usato gli strumenti";
  }

  // Derive a compact description from actual native actions, including file
  // changes and named integrations that used to disappear behind "commands".
  if (!live) {
    const parts: string[] = [];
    if (hasReads) parts.push("ha letto file");
    if (hasFiles) {
      const completed = files.some(item => item.status === "completed" || item.status === "success");
      const single = files.length === 1 && files[0].fileCount === 1;
      parts.push(completed ? `ha modificato ${single ? "un file" : "file"}` : "ha proposto modifiche ai file");
    }
    if (hasCommands) parts.push(items.filter(item => item.category === "command").every(item => item.status === "denied")
      ? "ha richiesto comandi non autorizzati" : "ha eseguito comandi");
    if (items.some(item => item.category === "web_search")) parts.push("ha cercato sul web");
    if (items.some(item => item.category === "web_read")) parts.push("ha consultato pagine web");
    const tools = [...new Set(items.filter(item => item.toolName).map(item => {
      const name = String(item.toolName).split("__").at(-1) || "";
      return name === "open_in_codex" ? "Open in Codex" : name.replaceAll("_", " ").replace(/[\u0000-\u001f]/g, "").slice(0, 100);
    }).filter(Boolean))];
    if (tools.length) parts.push(`ha usato ${tools.slice(0, 3).join(", ")}${tools.length > 3 ? " e altri strumenti" : ""}`);
    else if (items.some(item => !["command", "read", "file", "file_change", "web_search", "web_read"].includes(item.category))) parts.push("ha usato strumenti");
    if (parts.length) {
      title = parts.length > 1 ? `${parts.slice(0, -1).join(", ")} e ${parts.at(-1)}` : parts[0];
      title = title[0].toUpperCase() + title.slice(1);
    }
  }

  const failed = items.filter((item) => item.status === "error" || item.status === "denied").length;
  const count = items.length;
  if (count === 1 && hasCommands) {
    const status = items[0].status;
    title = status === "awaiting_approval" ? "Comando in attesa di approvazione"
      : status === "denied" ? "Comando non autorizzato"
      : status === "error" ? "Comando non riuscito"
      : status === "stopped" ? "Comando interrotto"
      : live ? "Esecuzione di un comando…" : "Comando eseguito";
  }
  const meta = `${count} ${count === 1 ? "azione" : "azioni"}${
    live ? " · in corso" : failed ? ` · ${failed} non riuscite` : ""
  }`;
  return { title, meta, live, failed };
}

function numericTimestamp(value: unknown, fallback: number): number {
  const timestamp = Number(value);
  return Number.isFinite(timestamp) && timestamp > 0 ? timestamp : fallback;
}

function formatWorkDuration(startedAtMs: number, finishedAtMs: number): string {
  const totalSeconds = Math.max(0, Math.floor((finishedAtMs - startedAtMs) / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  return [hours ? `${hours}h` : "", minutes ? `${minutes}m` : "", `${seconds}s`]
    .filter(Boolean)
    .join(" ");
}

export function nativeWorkDuration(timing: AgentTimelineMessage["nativeTurnTiming"]): number | null | undefined {
  if (!timing) return undefined;
  const valid = (value: unknown): value is number => typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
  if (valid(timing.durationMs)) return timing.durationMs;
  if (valid(timing.startedAtMs) && valid(timing.completedAtMs) && timing.completedAtMs >= timing.startedAtMs) return timing.completedAtMs - timing.startedAtMs;
  return null;
}

function activityGroupKey(messages: AgentTimelineMessage[]): string {
  return `activity-${String(messages[0]?.id ?? "empty")}`;
}

function chatMessageRenderValue(message: AgentTimelineMessage): Record<string, unknown> {
  return Object.fromEntries(renderFields.map((field) => [field, message[field]]));
}

function chatMessageRenderSignature(message: AgentTimelineMessage): string {
  const source = JSON.stringify(chatMessageRenderValue(message)) || "";
  let first = 2166136261;
  let second = 5381;
  for (let index = 0; index < source.length; index += 1) {
    const code = source.charCodeAt(index);
    first = Math.imul(first ^ code, 16777619);
    second = Math.imul(second, 33) ^ code;
  }
  return `${source.length}:${first >>> 0}:${second >>> 0}`;
}

function chatMessageRenderKey(message: AgentTimelineMessage, fallbackIndex: number): string {
  if (message.id !== undefined && message.id !== null) return `message:${String(message.id)}`;
  return `message:fallback:${fallbackIndex}:${message.role || "unknown"}:${message.kind || "message"}`;
}

function activityGroupRenderSignature(messages: AgentTimelineMessage[]): string {
  return chatMessageRenderSignature({
    id: activityGroupKey(messages),
    kind: "activity_group",
    attachments: messages.map(chatMessageRenderValue),
  });
}

function workGroupRenderSignature(work: WorkGroupData): string {
  return chatMessageRenderSignature({
    id: work.key,
    kind: "work_group",
    streaming: work.live,
    // Native duration is authoritative; render time must not recreate an
    // unchanged disclosure or steal focus when old history lacks timestamps.
    timestampMs: !work.live && work.recordedDurationMs === undefined ? work.finishedAtMs : undefined,
    nativeTurnTiming: work.recordedDurationMs === undefined ? undefined : {durationMs:work.recordedDurationMs},
    nativeWorkHistory: work.history,
    attachments: work.messages.map(chatMessageRenderValue),
  });
}

export function createAgentTimelineController(
  chatMessages: HTMLElement,
  renderer: AgentTimelineRenderer,
  options: {
    keyboardTarget?: HTMLElement;
    promptMotionScope?: string | (() => string);
    mountWorkDisclosure?: (group:HTMLDetailsElement, title:string, history?:NativeWorkHistory|null) => () => void;
    mountActivityDisclosure?: (group:HTMLDetailsElement, title:string, meta:string) => () => void;
  } = {},
): AgentTimelineController {
  const workComponents = new Map<HTMLDetailsElement, () => void>();
  const pruneWorkComponents = (all = false): void => {
    for (const [group, dispose] of workComponents) if (all || !chatMessages.contains(group)) { dispose(); workComponents.delete(group); }
  };
  const activityGroupViewState = new Map<string, ActivityViewState>();
  const workGroupViewState = new Map<string, WorkGroupViewState>();
  const workCompletionTimes = new Map<string, number>();
  const messageByRow = new WeakMap<HTMLElement, AgentTimelineMessage>();
  const messagesByGroup = new WeakMap<HTMLDetailsElement, AgentTimelineMessage[]>();
  let chatFollowsEnd = true;
  let chatZoom = 0;
  let viewportResizeFrame = 0;
  let reusableRows = new Map<string, HTMLElement>();
  function messageRow(message: AgentTimelineMessage): HTMLElement {
    const key = String(message.id);
    const previous = reusableRows.get(key);
    const old = previous && messageByRow.get(previous);
    const structure = (value: AgentTimelineMessage) => JSON.stringify([value.role, value.kind, value.provider, value.attachments, value.terminalAttachments, value.fileAttachments, value.artifacts, value.checkpoint, value.nativeTurnId, value.nativeGoalPrompt, Boolean(value.nativeQuestionReply)]);
    if (previous && old && structure(old) === structure(message)) {
      reusableRows.delete(key);
      if (chatMessageRenderSignature(old) !== chatMessageRenderSignature(message)) renderer.patchMessageRow(previous, message);
      previous.classList.remove("work-progress");
      messageByRow.set(previous, {...message});
      return previous;
    }
    return renderer.createMessageRow(message);
  }

  function queueEndAnchor(): void {
    if (viewportResizeFrame) return;
    viewportResizeFrame = requestAnimationFrame(() => {
      viewportResizeFrame = 0;
      chatMessages.scrollTop = chatMessages.scrollHeight;
      chatFollowsEnd = true;
    });
  }

  function setChatZoom(value: number, persist = false): void {
    const next = Math.max(
      CHAT_ZOOM_MIN,
      Math.min(CHAT_ZOOM_MAX, Number.isFinite(value) ? Math.round(value) : 0),
    );
    const keepEndVisible = chatFollowsEnd || nearScrollEnd(chatMessages);
    chatZoom = next;
    document.documentElement.style.setProperty("--ca-chat-font-offset", `${next}px`);
    document.documentElement.dataset.chatZoom = String(next);
    if (persist) {
      try {
        window.localStorage.setItem(CHAT_ZOOM_STORAGE_KEY, String(next));
      } catch {
        // Local persistence is optional; zoom still applies to the current window.
      }
    }
    if (keepEndVisible) queueEndAnchor();
  }

  function restoreChatZoom(): void {
    let storedZoom = 0;
    try {
      storedZoom = Number(window.localStorage.getItem(CHAT_ZOOM_STORAGE_KEY));
    } catch {
      storedZoom = 0;
    }
    setChatZoom(storedZoom);
  }

  function handleChatZoomKeydown(event: KeyboardEvent): void {
    if ((!event.ctrlKey && !event.metaKey) || event.altKey) return;
    let action: ChatZoomAction | null = null;
    if (event.key === "+" || event.key === "=" || event.code === "NumpadAdd") {
      action = "increase";
    } else if (event.key === "-" || event.code === "NumpadSubtract") {
      action = "decrease";
    } else if (event.key === "0" || event.code === "Numpad0") {
      action = "reset";
    }
    if (action === null) return;
    event.preventDefault();
    event.stopPropagation();
    applyChatZoom(action);
  }

  function applyChatZoom(action: ChatZoomAction): void {
    if (options.keyboardTarget) chatZoom = Number(document.documentElement.dataset.chatZoom) || 0;
    if (action === "increase") setChatZoom(chatZoom + 1, true);
    else if (action === "decrease") setChatZoom(chatZoom - 1, true);
    else setChatZoom(0, true);
  }

  const viewportResizeObserver =
    typeof ResizeObserver === "undefined"
      ? null
      : new ResizeObserver(() => {
          if (!chatFollowsEnd || viewportResizeFrame) return;
          queueEndAnchor();
        });
  viewportResizeObserver?.observe(chatMessages);
  // WebView2 routes keyboard input to whichever control currently owns focus. Listen at
  // window capture level so the shortcut also works while the composer textarea is active.
  const keyboardTarget = options.keyboardTarget || window;
  keyboardTarget.addEventListener("keydown", handleChatZoomKeydown as EventListener, true);
  if (!options.keyboardTarget) {
    window.centralAgentApplyChatZoom = applyChatZoom;
    window.centralAgentSetChatZoom = (value) => setChatZoom(value, true);
  }
  restoreChatZoom();

  function rememberActivityGroupViewState(group: HTMLDetailsElement | null): void {
    const key = group?.dataset.activityGroupKey;
    const body = group?.querySelector<HTMLElement>(".activity-group-body");
    if (!key || !body || !group) return;
    const previous = activityGroupViewState.get(key);
    const canMeasureScroll = group.open && body.clientHeight > 0;
    activityGroupViewState.set(key, {
      open: group.open,
      scrollTop: body.scrollTop,
      followEnd: canMeasureScroll ? nearScrollEnd(body, 2) : (previous?.followEnd ?? true),
    });
  }

  function captureActivityGroupViewState(): void {
    chatMessages
      .querySelectorAll<HTMLDetailsElement>(".activity-group")
      .forEach(rememberActivityGroupViewState);
  }

  function rememberWorkGroupViewState(group: HTMLDetailsElement | null): void {
    const key = group?.dataset.workGroupKey;
    if (!key || !group) return;
    workGroupViewState.set(key, {
      open: group.open,
      wasLive: group.dataset.live === "true",
    });
  }

  function captureWorkGroupViewState(): void {
    chatMessages
      .querySelectorAll<HTMLDetailsElement>(".agent-work-group")
      .forEach(rememberWorkGroupViewState);
  }

  function restoreActivityGroupViewState(group: HTMLDetailsElement): void {
    const key = group.dataset.activityGroupKey;
    const body = group.querySelector<HTMLElement>(".activity-group-body");
    if (!key || !body) return;
    const view = activityGroupViewState.get(key);
    if (!view) {
      if (group.open) body.scrollTop = body.scrollHeight;
      rememberActivityGroupViewState(group);
      return;
    }
    group.open = view.open;
    if (view.followEnd) body.scrollTop = body.scrollHeight;
    else body.scrollTop = Math.min(view.scrollTop, Math.max(0, body.scrollHeight - body.clientHeight));
  }

  function refreshActivityGroup(group: HTMLDetailsElement): ActivitySummary {
    const items = [...group.querySelectorAll<HTMLElement>(".chat-message.activity")].map((row) => ({
      category: row.dataset.activityCategory || "tool",
      status: row.dataset.activityStatus || "running",
      streaming: row.dataset.streaming === "true",
      toolName: messageByRow.get(row)?.activityToolName,
      fileCount: messageByRow.get(row)?.activityFileCount,
    }));
    const state = activityGroupCopy(items);
    const title = group.querySelector<HTMLElement>(".activity-group-title");
    const meta = group.querySelector<HTMLElement>(".activity-group-meta");
    if (title) title.textContent = state.title;
    if (meta) meta.textContent = state.meta;
    group.dataset.live = String(state.live);
    group.dataset.activityIcon = items.some(item => item.category === "command") ? "command" : items.some(item => item.category.startsWith("web_")) ? "web" : "other";
    group.classList.toggle("has-errors", state.failed > 0);
    return state;
  }

  function createActivityGroup(messages: AgentTimelineMessage[]): HTMLDetailsElement {
    const group = document.createElement("details");
    group.className = "activity-group";
    const key = activityGroupKey(messages);
    group.dataset.activityGroupKey = key;

    const summary = document.createElement("summary");
    const chevron = document.createElement("span");
    chevron.className = "activity-group-chevron";
    chevron.textContent = "›";
    chevron.setAttribute("aria-hidden", "true");
    const title = document.createElement("span");
    title.className = "activity-group-title";
    const meta = document.createElement("span");
    meta.className = "activity-group-meta";
    summary.append(title, meta, chevron);

    const body = document.createElement("div");
    body.className = "activity-group-body";
    for (const message of messages) {
      const row = messageRow(message);
      messageByRow.set(row, { ...message });
      body.append(row);
    }
    const initial = activityGroupCopy(messages.filter(message => message.kind === "activity").map(message => ({
      category:message.activityCategory || "tool", status:message.activityStatus || "running",
      streaming:Boolean(message.streaming), toolName:message.activityToolName, fileCount:message.activityFileCount,
    })));
    if (options.mountActivityDisclosure) workComponents.set(group, options.mountActivityDisclosure(group, initial.title, initial.meta));
    else group.append(summary);
    group.append(body);
    messagesByGroup.set(group, messages.map((message) => ({ ...message })));

    refreshActivityGroup(group);
    const savedView = activityGroupViewState.get(key);
    group.open = savedView?.open ?? false;
    group.addEventListener("toggle", () => rememberActivityGroupViewState(group));
    body.addEventListener(
      "wheel",
      (event) => {
        if (event.deltaY >= 0) return;
        const view = activityGroupViewState.get(key) || {
          open: group.open,
          scrollTop: body.scrollTop,
          followEnd: true,
        };
        activityGroupViewState.set(key, {
          ...view,
          open: group.open,
          scrollTop: body.scrollTop,
          followEnd: false,
        });
      },
      { passive: true },
    );
    body.addEventListener("scroll", () => rememberActivityGroupViewState(group), { passive: true });
    return group;
  }

  function appendWorkMessages(
    body: HTMLElement,
    messages: AgentTimelineMessage[],
  ): HTMLDetailsElement[] {
    const activityGroups: HTMLDetailsElement[] = [];
    let activities: AgentTimelineMessage[] = [];
    let deferredReasoning: AgentTimelineMessage[] = [];
    const flushActivities = (): void => {
      if (!activities.length) return;
      if (!activities.some(message => message.kind === "activity")) {
        deferredReasoning.push(...activities); activities=[]; return;
      }
      activities = [...deferredReasoning, ...activities]; deferredReasoning=[];
      const group = createActivityGroup(activities);
      activityGroups.push(group);
      body.append(group);
      activities = [];
    };

    for (const message of messages) {
      if (((message.kind || "message") === "activity" && !isWorkEvent(message)) || message.kind === "reasoning") {
        activities.push(message);
        continue;
      }
      flushActivities();
      const row = messageRow(message);
      decorateWorkRow(row, message);
      messageByRow.set(row, { ...message });
      body.append(row);
    }
    flushActivities();
    if (deferredReasoning.length) {
      const lastGroup = activityGroups.at(-1);
      if (lastGroup) {
        const target = lastGroup.querySelector<HTMLElement>(".activity-group-body")!;
        for (const message of deferredReasoning) { const row=messageRow(message); messageByRow.set(row,{...message}); target.append(row); }
        messagesByGroup.set(lastGroup,[...(messagesByGroup.get(lastGroup)||[]),...deferredReasoning]);
      } else {
        const group=createActivityGroup(deferredReasoning);activityGroups.push(group);body.append(group);
      }
    }
    return activityGroups;
  }

  function decorateWorkRow(row:HTMLElement, message:AgentTimelineMessage): void {
    row.classList.toggle("work-event", isWorkEvent(message));
    if (message.role === "assistant" && (message.kind || "message") === "message") {
      row.classList.add("work-progress");
      row.setAttribute("aria-label", "Aggiornamento del lavoro dell’agente");
    }
  }

  function createWorkGroup(work: WorkGroupData): HTMLDetailsElement {
    const group = document.createElement("details");
    group.className = "agent-work-group";
    group.dataset.workGroupKey = work.key;
    group.dataset.live = String(work.live);

    const summary = document.createElement("summary");
    const title = document.createElement("span");
    title.className = "agent-work-group-title";
    title.textContent = work.recordedDurationMs === null ? "Durata lavoro: non disponibile"
      : `Durata lavoro: ${work.recordedDurationMs === 0 ? "0s" : typeof work.recordedDurationMs === "number" ? formatWorkDuration(0, work.recordedDurationMs) : formatWorkDuration(work.startedAtMs, work.finishedAtMs)}`;
    const chevron = document.createElement("span");
    chevron.className = "agent-work-group-chevron";
    chevron.textContent = "›";
    chevron.setAttribute("aria-hidden", "true");
    summary.append(title, chevron);

    const body = document.createElement("div");
    body.className = "agent-work-group-body";
    appendWorkMessages(body, work.messages);
    if (options.mountWorkDisclosure) workComponents.set(group, options.mountWorkDisclosure(group, title.textContent, work.history));
    else group.append(summary);
    group.append(body);

    const saved = workGroupViewState.get(work.key);
    if (work.live) group.open = true;
    else if (saved?.wasLive) group.open = false;
    else group.open = saved?.open ?? false;
    workGroupViewState.set(work.key, { open: group.open, wasLive: work.live });
    group.addEventListener("toggle", () => rememberWorkGroupViewState(group));
    return group;
  }

  function render(
    messages: AgentTimelineMessage[] = [],
    runState: AgentTimelineRunState = {},
  ): void {
    const promptMotionScope = typeof options.promptMotionScope === "function"
      ? options.promptMotionScope()
      : options.promptMotionScope;
    const followChatEnd = !chatMessages.childElementCount || chatFollowsEnd;
    const previousChatScrollTop = chatMessages.scrollTop;
    const focusedActivity = document.activeElement?.closest<HTMLDetailsElement>(".activity-group");
    const focusedActivityKey = document.activeElement === focusedActivity?.querySelector("summary") ? focusedActivity?.dataset.activityGroupKey : undefined;
    captureActivityGroupViewState();
    captureWorkGroupViewState();
    reusableRows = new Map([...chatMessages.querySelectorAll<HTMLElement>(".chat-message[data-message-id]")].map(row => [row.dataset.messageId || "", row]));
    if (!messages.length) {
      chatMessages.replaceChildren();
      pruneWorkComponents(true);
      activityGroupViewState.clear();
      workGroupViewState.clear();
      workCompletionTimes.clear();
      chatFollowsEnd = true;
      return;
    }

    const items: TimelineItem[] = [];
    let fallbackIndex = 0;
    let activities: AgentTimelineMessage[] = [];
    const flushActivities = (): void => {
      if (!activities.length) return;
      items.push({
        type: "activity_group",
        key: `group:${activityGroupKey(activities)}`,
        signature: activityGroupRenderSignature(activities),
        messages: activities,
      });
      activities = [];
    };
    const appendLooseMessage = (message: AgentTimelineMessage): void => {
      if ((message.kind || "message") === "activity" && !isWorkEvent(message)) {
        activities.push(message);
        return;
      }
      flushActivities();
      items.push({
        type: "message",
        key: chatMessageRenderKey(message, fallbackIndex),
        signature: chatMessageRenderSignature(message),
        message,
      });
      fallbackIndex += 1;
    };

    const lastUserIndex = messages.reduce(
      (last, message, index) =>
        message.role === "user" && (message.kind || "message") === "message" ? index : last,
      -1,
    );
    const currentRunId =
      runState.runId === undefined || runState.runId === null ? null : String(runState.runId);
    const renderStartedAt = Date.now();

    for (let index = 0; index < messages.length; ) {
      const userMessage = messages[index];
      const beginsRun =
        userMessage.role === "user" && (userMessage.kind || "message") === "message";
      if (!beginsRun) {
        appendLooseMessage(userMessage);
        index += 1;
        continue;
      }

      appendLooseMessage(userMessage);
      flushActivities();
      let end = index + 1;
      while (
        end < messages.length &&
        (!(messages[end].role === "user" && (messages[end].kind || "message") === "message") || sameNativeTurn(userMessage,messages[end]))
      ) {
        end += 1;
      }
      const segment = messages.slice(index + 1, end);
      const notices = segment.filter(isNativeServiceNotice);
      const media = segment.filter(isNativeMedia);
      const output = segment.filter(message => !isNativeServiceNotice(message) && !isNativeMedia(message));
      const userRunId =
        userMessage.runId === undefined || userMessage.runId === null
          ? null
          : String(userMessage.runId);
      const live = userMessage.provider === "codex_app_server" && userMessage.nativeWorkHistory
        ? userMessage.nativeWorkHistory.state === "live"
        : Boolean(runState.active &&
          (currentRunId !== null && userRunId !== null
            ? currentRunId === userRunId
            : index === lastUserIndex || sameNativeTurn(userMessage,messages[lastUserIndex])),
      );
      const workKey = `run-${userRunId ?? "legacy"}-${String(userMessage.id ?? index)}`;

      const finalIndex = live ? -1 : finalOutputIndex(output);

      const workMessages = live
        ? output
        : finalIndex >= 0
          ? output.slice(0, finalIndex)
          : output;
      if (workMessages.length || (!live && userMessage.nativeWorkHistory)) {
        const saved = workGroupViewState.get(workKey);
        if (!live && saved?.wasLive && !workCompletionTimes.has(workKey)) {
          workCompletionTimes.set(workKey, renderStartedAt);
        }
        const startedAtMs = numericTimestamp(userMessage.timestampMs, renderStartedAt);
        const lastRecordedAtMs = output.reduce(
          (latest, message) => Math.max(latest, numericTimestamp(message.timestampMs, latest)),
          startedAtMs,
        );
        const finishedAtMs = live
          ? lastRecordedAtMs
          : (workCompletionTimes.get(workKey) ?? lastRecordedAtMs);
        const work: WorkGroupData = {
          key: workKey,
          messages: workMessages,
          live,
          startedAtMs,
          finishedAtMs,
          recordedDurationMs: live ? undefined : nativeWorkDuration(userMessage.nativeTurnTiming),
          history: userMessage.nativeWorkHistory,
        };
        items.push({
          type: "work_group",
          key: `work:${workKey}`,
          signature: workGroupRenderSignature(work),
          work,
        });
      }

      // Completed native images remain usable while running and after the work
      // disclosure collapses; they are not invented final assistant messages.
      for (const image of media) appendLooseMessage(image);
      if (!live && finalIndex >= 0) {
        for (const message of output.slice(finalIndex)) appendLooseMessage(message);
      }
      // Service verification/wait notices remain visible, never hidden in the
      // completed work disclosure or mistaken for the model's final answer.
      for (const notice of notices) appendLooseMessage(notice);
      index = end;
    }
    flushActivities();

    const activeWorkKeys = new Set(
      items.filter((item) => item.type === "work_group").map((item) => item.work?.key || ""),
    );
    for (const key of workGroupViewState.keys()) {
      if (!activeWorkKeys.has(key)) {
        workGroupViewState.delete(key);
        workCompletionTimes.delete(key);
      }
    }

    const existingByKey = new Map<string, HTMLElement>();
    for (const node of chatMessages.children) {
      const element = node as HTMLElement;
      const key = element.dataset.renderKey;
      if (key && !existingByKey.has(key)) existingByKey.set(key, element);
    }

    const retained = new Set<HTMLElement>();
    const groupsNeedingRestore: HTMLDetailsElement[] = [];
    const promptsNeedingMotion: HTMLElement[] = [];
    const promptMotionsToTransfer: Array<[HTMLElement, HTMLElement]> = [];
    items.forEach((item, index) => {
      let node = existingByKey.get(item.key);
      const wasMissing = !node;
      const expectedType =
        item.type === "activity_group"
          ? node?.matches("details.activity-group")
          : item.type === "work_group"
            ? node?.matches("details.agent-work-group")
            : node?.matches(".chat-message");

      if (!node || !expectedType || node.dataset.renderSignature !== item.signature) {
        const restoreSummaryFocus = item.type === "work_group" && document.activeElement != null
          && node?.querySelector("summary") === document.activeElement;
        const replacement =
          item.type === "activity_group"
            ? createActivityGroup(item.messages || [])
            : item.type === "work_group"
              ? createWorkGroup(item.work as WorkGroupData)
              : messageRow(item.message || {});
        replacement.dataset.renderKey = item.key;
        replacement.dataset.renderSignature = item.signature;
        if (item.type === "activity_group") {
          groupsNeedingRestore.push(replacement as HTMLDetailsElement);
        } else if (item.type === "work_group") {
          replacement
            .querySelectorAll<HTMLDetailsElement>(".activity-group")
            .forEach((group) => groupsNeedingRestore.push(group));
        } else {
          messageByRow.set(replacement, { ...(item.message || {}) });
          if (item.message && isWorkEvent(item.message)) decorateWorkRow(replacement, item.message);
          if (
            wasMissing
            && promptMotionScope
            && item.message
            && consumePromptSendMotion(promptMotionScope, item.message)
          ) promptsNeedingMotion.push(replacement);
        }
        if (node && node !== replacement) {
          promptMotionsToTransfer.push([node, replacement]);
          node.replaceWith(replacement);
        }
        node = replacement;
        if (restoreSummaryFocus) node.querySelector<HTMLElement>("summary")?.focus({preventScroll:true});
      } else if (item.type === "activity_group") {
        messagesByGroup.set(node as HTMLDetailsElement, (item.messages || []).map((message) => ({ ...message })));
      } else if (item.type === "message") {
        messageByRow.set(node, { ...(item.message || {}) });
      }

      retained.add(node);
      const nodeAtIndex = chatMessages.children[index];
      if (nodeAtIndex !== node) chatMessages.insertBefore(node, nodeAtIndex || null);
    });

    for (const node of [...chatMessages.children] as HTMLElement[]) {
      if (!retained.has(node)) node.remove();
    }
    groupsNeedingRestore.forEach(restoreActivityGroupViewState);
    if (focusedActivityKey) {
      [...chatMessages.querySelectorAll<HTMLDetailsElement>(".activity-group")]
        .find(group => group.dataset.activityGroupKey === focusedActivityKey)?.querySelector<HTMLElement>("summary")?.focus({preventScroll:true});
    }
    pruneWorkComponents();
    if (followChatEnd) chatMessages.scrollTop = chatMessages.scrollHeight;
    else {
      chatMessages.scrollTop = Math.min(
        previousChatScrollTop,
        Math.max(0, chatMessages.scrollHeight - chatMessages.clientHeight),
      );
    }
    chatFollowsEnd = followChatEnd || nearScrollEnd(chatMessages, 2);
    promptMotionsToTransfer.forEach(([previous, replacement]) => {
      transferPromptArrival(previous, replacement);
    });
    promptsNeedingMotion.forEach(animatePromptArrival);
  }

  function update(updateValue: AgentTimelineMessage): boolean {
    const row = [...chatMessages.querySelectorAll<HTMLElement>("[data-message-id]")].find(
      (candidate) => candidate.dataset.messageId === String(updateValue.id),
    );
    if (!row) return false;

    const followChatEnd = chatFollowsEnd;
    const previousChatScrollTop = chatMessages.scrollTop;
    const activityGroup = row.closest<HTMLDetailsElement>(".activity-group");
    const workGroup = row.closest<HTMLDetailsElement>(".agent-work-group");
    if (activityGroup) rememberActivityGroupViewState(activityGroup);
    if (workGroup) rememberWorkGroupViewState(workGroup);

    const cachedMessages = activityGroup ? messagesByGroup.get(activityGroup) || [] : [];
    const cachedGroupMessage = cachedMessages.find(
      (message) => String(message.id) === String(updateValue.id),
    );
    const previousMessage = messageByRow.get(row) || cachedGroupMessage || {};
    const updatedMessage = { ...previousMessage, ...updateValue };
    if (!updatedMessage.role) updatedMessage.role = "assistant";

    renderer.patchMessageRow(row, updatedMessage);
    if (workGroup || isWorkEvent(updatedMessage)) decorateWorkRow(row, updatedMessage);
    messageByRow.set(row, updatedMessage);
    row.dataset.renderSignature = chatMessageRenderSignature(updatedMessage);

    if (activityGroup) {
      const messageIndex = cachedMessages.findIndex(
        (message) => String(message.id) === String(updateValue.id),
      );
      if (messageIndex >= 0) cachedMessages[messageIndex] = updatedMessage;
      messagesByGroup.set(activityGroup, cachedMessages);
      activityGroup.dataset.renderSignature =
        messageIndex >= 0 ? activityGroupRenderSignature(cachedMessages) : "";
      refreshActivityGroup(activityGroup);
      restoreActivityGroupViewState(activityGroup);
    }
    if (workGroup) workGroup.dataset.renderSignature = "";

    if (followChatEnd) chatMessages.scrollTop = chatMessages.scrollHeight;
    else {
      chatMessages.scrollTop = Math.min(
        previousChatScrollTop,
        Math.max(0, chatMessages.scrollHeight - chatMessages.clientHeight),
      );
    }
    chatFollowsEnd = followChatEnd || nearScrollEnd(chatMessages, 2);
    return true;
  }

  const handleWheel = (event: WheelEvent): void => {
    if (event.deltaY < 0) chatFollowsEnd = false;
  };
  const handleScroll = (): void => {
    chatFollowsEnd = nearScrollEnd(chatMessages, 2);
  };
  chatMessages.addEventListener("wheel", handleWheel, { passive: true });
  chatMessages.addEventListener("scroll", handleScroll, { passive: true });

  return {
    render,
    update,
    pauseFollowing(): void { chatFollowsEnd = false; if (viewportResizeFrame) { cancelAnimationFrame(viewportResizeFrame); viewportResizeFrame = 0; } },
    destroy(): void {
      pruneWorkComponents(true);
      chatMessages.removeEventListener("wheel", handleWheel);
      chatMessages.removeEventListener("scroll", handleScroll);
      keyboardTarget.removeEventListener("keydown", handleChatZoomKeydown as EventListener, true);
      if (window.centralAgentApplyChatZoom === applyChatZoom) {
        delete window.centralAgentApplyChatZoom;
      }
      if (!options.keyboardTarget) delete window.centralAgentSetChatZoom;
      viewportResizeObserver?.disconnect();
      if (viewportResizeFrame) cancelAnimationFrame(viewportResizeFrame);
      activityGroupViewState.clear();
      workGroupViewState.clear();
      workCompletionTimes.clear();
    },
  };
}
