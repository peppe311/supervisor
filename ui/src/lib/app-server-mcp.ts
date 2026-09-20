import type { McpServerStatus } from "../../../protocol/app-server/0.153.4/typescript/v2/McpServerStatus";

type Entry = { name: string; description: string | null; entryId?:string; inputSchema?:string|null };
export type NativeMcpServer = Pick<McpServerStatus, "name" | "authStatus" | "runtimeStatus"> & {
  tools: Record<string, Entry>; resources: Entry[]; resourceTemplates: Entry[];
};
export interface NativeMcpView {
  configuration?: McpConfiguration | null;
  threadId?: string | null;
  inventoryId: string | null;
  current: boolean;
  busy: boolean;
  servers: NativeMcpServer[];
  login: { attemptId: string; name: string; origin: string | null; canOpen: boolean } | null;
  error: string | null;
  notice: string | null;
  directResult?: {operation:'resource'|'tool';server:string;name:string;contents:{kind:string;mimeType:string|null;text:string}[];structuredContent:string|null;isError:boolean}|null;
}
export interface McpConfiguration {
  viewId: string; current: boolean;
  snapshot: {target:{file:string;version:string}|null;servers:McpConfiguredServer[]};
}
export interface McpConfiguredServer {name:string;transport:string;userTransport:string;effectiveEnabled:boolean|null;inUserConfig:boolean;canEdit:boolean;savedOptions:McpOptionKey[]}
export type McpOptionKey = "command"|"args"|"cwd"|"env_vars"|"url"|"bearer_token_env_var"|"env_http_headers"|"startup_timeout_sec"|"tool_timeout_sec"|"required"|"enabled_tools"|"disabled_tools";
export type McpConfigurationEdit = {kind:"add";name:string;transport:
  {kind:"stdio";command:string;args:string[];cwd:string|null;env_vars:string[]} |
  {kind:"http";url:string;bearer_token_env_var:string|null}}
  | {kind:"set_enabled";name:string;enabled:boolean} | {kind:"remove";name:string}
  | {kind:"set_option";name:string;key:McpOptionKey;value:unknown} | {kind:"clear_option";name:string;key:McpOptionKey};
export type McpAction = {kind:"inspect_configuration"} | {kind:"change_configuration";view_id:string;edit:McpConfigurationEdit}
  | {kind:"refresh"} | {kind:"reload";inventory_id:string}
  | {kind:"login";inventory_id:string;name:string} | {kind:"open_login";attempt_id:string}
  | {kind:"read_resource";inventory_id:string;server:string;resource_id:string}
  | {kind:"call_tool";inventory_id:string;server:string;tool:string;arguments:Record<string,unknown>};
export function emptyNativeMcp(): NativeMcpView {
  return {inventoryId:null,current:false,busy:false,servers:[],login:null,error:null,notice:null};
}
export function mcpCanLogin(server: NativeMcpServer): boolean {
  return server.authStatus === "notLoggedIn" || server.authStatus === "oAuth" || server.runtimeStatus === "authenticationRequired";
}
export function mcpRuntimeLabel(status: NativeMcpServer["runtimeStatus"]): string {
  const labels = {notStarted:"Not started",starting:"Starting",connected:"Connected",authenticationRequired:"Authentication required",failed:"Failed",cancelled:"Cancelled",disabled:"Disabled"};
  return status ? labels[status] || "Not reported" : "Not reported for this scope";
}
export function mcpAuthLabel(status: NativeMcpServer["authStatus"]): string {
  return {unknown:"Not reported",unsupported:"OAuth not supported",notLoggedIn:"Sign-in required",bearerToken:"Bearer token configured",oAuth:"OAuth authorized"}[status] || "Not reported";
}
