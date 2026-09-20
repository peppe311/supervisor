import type { Model } from "../../../protocol/app-server/0.153.4/typescript/v2/Model";
import type { NativeMcpView } from "./app-server-mcp";
import type { AppServerP2View } from "./app-server-p2";

export type AccountAction = "connect" | "refresh" | "reload_chats" | "login_browser" | "login_device" | "open_login" | "cancel_login" | "logout_confirmed" | "setup_sandbox_elevated" | "setup_sandbox_unelevated";

export interface PermissionProfileView {
  id: string;
  description: string | null;
  allowed: boolean;
}

export interface RateLimitWindowView {
  usedPercent: number;
  windowDurationMins: number | null;
  resetsAt: number | null;
}

export interface RateLimitBucketView {
  id: string;
  name: string | null;
  primary: RateLimitWindowView | null;
  secondary: RateLimitWindowView | null;
}

export type RuntimeNoticeValue =
  | { kind: "warning"; message: string }
  | { kind: "config_warning"; summary: string; details: string | null; path: string | null; line: number | null; column: number | null }
  | { kind: "deprecation"; summary: string; details: string | null }
  | { kind: "windows_world_writable"; samplePaths: string[]; extraCount: number; failedScan: boolean };

export interface RuntimeNoticeView {
  sequence: number;
  current: boolean;
  value: RuntimeNoticeValue;
}

export interface AppServerAccountView {
  connected: boolean;
  connecting: boolean;
  refreshing: boolean;
  authBusy: boolean;
  version: string | null;
  account: { type: string; email: string | null; planType: string | null; supported: boolean; policy: string | null } | null;
  login: { url: string; userCode: string | null } | null;
  models: Pick<Model, "id" | "model" | "displayName" | "supportedReasoningEfforts" | "serviceTiers" | "supportsPersonality" | "upgrade" | "upgradeInfo">[];
  requirementsLoaded: boolean;
  permissionProfiles: { loading: boolean; loaded: boolean; current: boolean; requiresNamedProfile: boolean; entries: PermissionProfileView[] };
  rateLimits: { refreshing: boolean; loaded: boolean; current: boolean; buckets: RateLimitBucketView[]; availableResetCredits: string | null };
  providerCapabilities: {loaded:boolean;current:boolean;namespaceTools:boolean;imageGeneration:boolean;webSearch:boolean};
  accountUsage: {refreshing:boolean;loaded:boolean;current:boolean;summary:{lifetimeTokens:string|null;peakDailyTokens:string|null;longestRunningTurnSec:string|null;currentStreakDays:string|null;longestStreakDays:string|null}|null;dailyUsageBuckets:{startDate:string;tokens:string}[]|null};
  workspaceMessages: {refreshing:boolean;loaded:boolean;current:boolean;featureEnabled:boolean;messages:{messageId:string;messageType:"headline"|"announcement"|"unknown";messageBody:string;createdAt:number|null;archivedAt:number|null}[]};
  runtimeNotices: RuntimeNoticeView[];
  errors: Record<string, string>;
  chatReload: {loading:boolean;checked:boolean;total:number;loaded:number;unavailable:number;skipped:number};
  sandboxSetup?: {mode:"elevated"|"unelevated"|null;phase:string;detail:string};
  mcp?: NativeMcpView;
  p2?: AppServerP2View;
  diagnostics?: {total:number;entries:{fingerprint:string;method:string|null;count:number}[]};
}
export function emptyAppServerAccount(): AppServerAccountView {
  return {
    connected:false,
    connecting:false,
    refreshing:false,
    authBusy:false,
    version:null,
    account:null,
    login:null,
    models:[],
    requirementsLoaded:false,
    permissionProfiles:{loading:false,loaded:false,current:false,requiresNamedProfile:false,entries:[]},
    rateLimits:{refreshing:false,loaded:false,current:false,buckets:[],availableResetCredits:null},
    providerCapabilities:{loaded:false,current:false,namespaceTools:false,imageGeneration:false,webSearch:false},
    accountUsage:{refreshing:false,loaded:false,current:false,summary:null,dailyUsageBuckets:null},
    workspaceMessages:{refreshing:false,loaded:false,current:false,featureEnabled:false,messages:[]},
    runtimeNotices:[],
    errors:{},
    chatReload:{loading:false,checked:false,total:0,loaded:0,unavailable:0,skipped:0}
  };
}

export function accountLabel(type: string): string {
  if (type === "chatgpt") return "ChatGPT";
  if (type === "apiKey") return "API key";
  if (type === "amazonBedrock") return "Amazon Bedrock";
  return "Unknown account";
}

export function accountStatus(view: AppServerAccountView): string {
  if (view.connecting) return "Connecting…";
  if (!view.connected) return "Not connected";
  if (view.refreshing) return "Refreshing…";
  if (view.login || view.authBusy) return "Sign-in in progress";
  if (view.account?.supported && view.account.type === "chatgpt") return "ChatGPT connected";
  if (view.account) return `${accountLabel(view.account.type)} detected · unsupported`;
  return "Sign-in required";
}
export function canAuthenticate(view: AppServerAccountView): boolean {
  return view.connected && !view.connecting && !view.authBusy && !view.login && !view.refreshing;
}

function compactNumber(value: number): string {
  return value.toFixed(1).replace(/\.0$/, "");
}

export function rateLimitWindowText(window: RateLimitWindowView): string {
  const parts = [`${compactNumber(window.usedPercent)}% used`];
  if (window.windowDurationMins !== null) parts.push(`${window.windowDurationMins} min window`);
  if (window.resetsAt !== null) {
    const reset = new Date(window.resetsAt * 1000);
    parts.push(Number.isNaN(reset.getTime()) ? "reset time unavailable" : `resets ${reset.toISOString()}`);
  }
  return parts.join(" · ");
}

export function runtimeNoticeTitle(notice: RuntimeNoticeView): string {
  if (notice.value.kind === "config_warning") return "Configuration warning";
  if (notice.value.kind === "deprecation") return "Codex deprecation";
  if (notice.value.kind === "windows_world_writable") return "World-writable path warning";
  return "Codex runtime warning";
}

export function runtimeNoticeLines(notice: RuntimeNoticeView): string[] {
  const value = notice.value;
  if (value.kind === "warning") return [value.message];
  if (value.kind === "deprecation") return [value.summary, ...(value.details ? [value.details] : [])];
  if (value.kind === "config_warning") {
    const location = value.path
      ? `${value.path}${value.line === null ? "" : ` · ${value.line}:${value.column ?? 0}`}`
      : null;
    return [value.summary, ...(value.details ? [value.details] : []), ...(location ? [location] : [])];
  }
  return [
    value.failedScan ? "Codex could not finish the path security scan." : "Codex found paths writable by every local user.",
    ...value.samplePaths,
    ...(value.extraCount ? [`${value.extraCount} additional path${value.extraCount === 1 ? "" : "s"}`] : [])
  ];
}
