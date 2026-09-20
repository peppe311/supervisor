import type { Thread } from "../../../protocol/app-server/0.153.4/typescript/v2/Thread";
import type { NativeMcpView } from "./app-server-mcp";
import type { NativeGoalView } from "./native-goals";
import type { ThreadSettings } from "../../../protocol/app-server/0.153.4/typescript/v2/ThreadSettings";
import type { NativeAppsView } from "./native-apps";
import type { NativeHooksView } from "./native-hooks";
export type NativeThreadSettings = Pick<ThreadSettings,"cwd"|"model"|"modelProvider"|"serviceTier"|"effort"|"summary"|"personality"|"approvalsReviewer"> & {approvalPolicy:string;sandboxKind:string};
export type NativeHistoryItem = Pick<Thread,"id"|"modelProvider"|"updatedAt"|"ephemeral"> & {projectLabel:string;forkedFromId?:string|null;status:{type:Thread["status"]["type"]}};
export interface CloudTaskSummary {filesChanged:number;linesAdded:number;linesRemoved:number}
export interface CloudTask {
  id:string;title:string;status:string;updatedAt:string;environmentLabel:string;
  summary:CloudTaskSummary;isReview:boolean;attemptTotal:number;
}
export interface CloudDiff {taskId:string;attempt:number;busy:boolean;content:string;error:string|null}
export interface CloudHistoryView {items:CloudTask[];busy:boolean;hasMore:boolean;error:string|null;diff:CloudDiff|null}
export interface NativeHistoryView {
  requestId:string;viewId:string;archived:boolean;search:string;items:NativeHistoryItem[];
  busy:boolean;hasMore:boolean;error:string|null;cloud:CloudHistoryView;
}
export interface NativeConversationView {
  visible:boolean;connected:boolean;busy:boolean;observed:boolean;name:string|null;
  directory?:string|null;importAvailable?:boolean;binding:{threadId:string;archived:boolean;deleted:boolean}|null;
  pendingFork?:string|null;lastBranch?:{owner:string;name:string}|null;unusedLink?:boolean;
  delegates?:{owner:string;threadId:string;name:string;relation:'child'|'parent';status:string;pendingRequests:number}[];
  completedTurns?:{id:string;status:string}[];
  readyForTurn?:boolean;canCompact?:boolean;compaction?:'sending'|'accepted'|'running'|'stop_requested'|null;
  historyMode?:Thread["historyMode"]|null;
  mcp?:NativeMcpView|null;
  apps?:NativeAppsView|null;
  hooks?:NativeHooksView|null;
  nativeLoaded?:boolean;goal?:NativeGoalView|null;
  threadSettings?:NativeThreadSettings|null;threadSettingsCurrent?:boolean;
}
export function canImportHistory(view:NativeConversationView|null):boolean {
  return !!view?.visible && view.connected && !view.busy && !view.binding && !!view.directory && view.importAvailable===true;
}
export function historyTitle(_item:NativeHistoryItem):string {
  return "Codex conversation";
}
export function historyProjectLabel(value:string|null|undefined):string {
  const trimmed=String(value||"").replace(/[\\/]+$/,"");
  return trimmed.split(/[\\/]/).at(-1)||"Local project";
}
