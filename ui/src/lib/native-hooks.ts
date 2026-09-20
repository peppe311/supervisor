export interface NativeHook {
  key:string;eventName:string;matcher:string|null;statusMessage:string|null;sourcePath:string;
  source:string;handlerType:string;enabled:boolean;managed:boolean;trustStatus:string;
}
export interface NativeHookRun {
  id:string;eventName:string;handlerType:string;executionMode:string;scope:string;source:string;
  status:string;statusMessage:string|null;startedAt:string;completedAt:string|null;durationMs:string|null;
  entries:{kind:string;text:string}[];
}
export interface NativeHooksView {
  threadId:string;directory:string;viewId:string|null;hooks:NativeHook[];warnings:string[];errors:string[];
  runs:NativeHookRun[];current:boolean;busy:boolean;error:string|null;notice:string|null;
}
