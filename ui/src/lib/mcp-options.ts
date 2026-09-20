import type { McpOptionKey } from "./app-server-mcp";
export interface McpOption {key:McpOptionKey;label:string;input:"text"|"json"|"number"|"boolean";transport?:"stdio"|"http";required?:boolean;help:string}
export const mcpOptions: readonly McpOption[] = [
  {key:"command",label:"Executable",input:"text",transport:"stdio",required:true,help:"The executable to run. Existing arguments and environment settings are retained; review them before enabling or reloading."},
  {key:"args",label:"Arguments",input:"json",transport:"stdio",help:'A JSON array of strings, for example ["--stdio"]. An empty array removes all arguments.'},
  {key:"cwd",label:"Working directory",input:"text",transport:"stdio",help:"An absolute local path. Clear saved option to use the native default."},
  {key:"env_vars",label:"Environment variable names",input:"json",transport:"stdio",help:'A JSON array of variable names, for example ["MCP_TOKEN"]. Values are read by Codex, never entered here.'},
  {key:"url",label:"MCP URL",input:"text",transport:"http",required:true,help:"An HTTP(S) endpoint without embedded credentials. Saved header/auth settings are retained and may be used with the new destination."},
  {key:"bearer_token_env_var",label:"Bearer token variable name",input:"text",transport:"http",help:"The environment variable name, not its secret value. Clearing this field does not sign out OAuth or remove other authorization sources."},
  {key:"env_http_headers",label:"HTTP header variable names",input:"json",transport:"http",help:'A JSON object mapping header names to environment variable names, for example {"X-Api-Key":"MCP_TOKEN"}. This replaces that map; static headers remain unchanged.'},
  {key:"startup_timeout_sec",label:"Server startup timeout (seconds)",input:"number",help:"A positive number. This is an explicitly chosen native MCP setting, not a Supervisor run timeout. Clear to restore the native default."},
  {key:"tool_timeout_sec",label:"Tool timeout (seconds)",input:"number",help:"A positive number for native MCP tools. No value is supplied automatically. Clear to restore the native default."},
  {key:"required",label:"Require successful server startup",input:"boolean",help:"When true, native startup fails if this enabled server cannot initialize."},
  {key:"enabled_tools",label:"Tool allow list",input:"json",help:"A JSON array of tool names. An empty array allows no tools; clearing removes this saved allow list."},
  {key:"disabled_tools",label:"Tool deny list",input:"json",help:"A JSON array of tool names. Codex applies it after the allow list. Clear removes this saved deny list."},
];
export function optionsFor(transport:string):readonly McpOption[] {
  return ["stdio","http"].includes(transport)?mcpOptions.filter(option=>!option.transport || option.transport===transport):[];
}
export function optionValue(key:McpOptionKey, text:string):unknown {
  const option=mcpOptions.find(option=>option.key===key);
  if(!option)throw new Error("Select a supported option.");
  if(!text.trim())throw new Error("Enter a value. Clearing a saved option is a separate action.");
  if(option.input==="json") {
    const value:unknown=JSON.parse(text);
    if(key==="env_http_headers") {
      if(!value || typeof value!=="object" || Array.isArray(value) || !Object.values(value).every(v=>typeof v==="string"))throw new Error("Use an object of header names and environment variable names.");
    } else if(!Array.isArray(value) || !value.every(v=>typeof v==="string"))throw new Error("Use a JSON array of strings.");
    return value;
  }
  if(option.input==="number") {const number=Number(text);if(!Number.isFinite(number)||number<=0)throw new Error("Enter a positive number of seconds.");return number;}
  if(option.input==="boolean") {if(text!=="true" && text!=="false")throw new Error("Select true or false.");return text==="true";}
  return text.trim();
}
