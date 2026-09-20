export interface NativeAppTool {
  name:string;title:string|null;description:string;enabled:boolean;disabledReason:string|null;readOnly:boolean;
}
export interface NativeApp {
  id:string;name:string;description:string|null;iconUrl:string|null;accessible:boolean;configuredEnabled:boolean;
  installed:boolean;runtimeEnabled:boolean;callable:boolean;tools:NativeAppTool[];
}
export interface SelectedNativeApp {
  selectionId:string;appId:string;name:string;iconUrl:string|null;threadId:string;current:boolean;
}
export interface NativeAppsView {
  threadId:string;inventoryId:string|null;items:NativeApp[];selected:SelectedNativeApp[];
  current:boolean;busy:boolean;error:string|null;unavailableReason:string|null;notice:string|null;
}
export type AppsAction = {kind:'refresh'}
  | {kind:'attach';inventory_id:string;app_id:string}
  | {kind:'remove';selection_id:string};

export function appAvailability(app:NativeApp):string {
  if(!app.accessible)return 'Unavailable for this account or workspace';
  if(!app.configuredEnabled)return 'Disabled in Codex';
  if(!app.installed)return 'Not installed';
  if(!app.runtimeEnabled)return 'Disabled in the Codex runtime';
  if(!app.callable)return 'Unavailable for an explicit mention';
  return 'Available for an explicit mention';
}

export function appSelectable(app:NativeApp):boolean {
  return app.accessible && app.configuredEnabled && app.installed && app.runtimeEnabled && app.callable;
}

export function appMatches(app:NativeApp,query:string):boolean {
  const normalized=query.trim().toLocaleLowerCase();
  if(!normalized)return true;
  return `${app.name} ${app.description||''} ${appAvailability(app)} ${app.tools.map(tool=>`${tool.name} ${tool.title||''} ${tool.description||''}`).join(' ')}`
    .toLocaleLowerCase().includes(normalized);
}

export type NativeAppIconKind='browser'|'github'|'cloudflare'|'figma'|'linear'|'notion'|'generic';

export function appIconKind(appId:string,name:string):NativeAppIconKind {
  const normalized=`${appId} ${name}`.trim().toLocaleLowerCase();
  if(/(^|[^a-z])browser(?:[ -]use)?([^a-z]|$)/.test(normalized))return 'browser';
  if(/(^|[^a-z])github([^a-z]|$)/.test(normalized))return 'github';
  if(/(^|[^a-z])cloudflare([^a-z]|$)/.test(normalized))return 'cloudflare';
  if(/(^|[^a-z])figma([^a-z]|$)/.test(normalized))return 'figma';
  if(/(^|[^a-z])linear([^a-z]|$)/.test(normalized))return 'linear';
  if(/(^|[^a-z])notion([^a-z]|$)/.test(normalized))return 'notion';
  return 'generic';
}

export function appInitial(name:string):string {
  return Array.from(name.trim())[0]?.toLocaleUpperCase()||'?';
}
