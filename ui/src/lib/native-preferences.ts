import type {Config} from "../../../protocol/app-server/0.153.4/typescript/v2/Config";
export type PreferenceKey=Extract<keyof Config,string>;
export interface ConfigLayer {kind:string;location:string|null;profile:string|null;version:string;disabledReason:string|null;}
export interface NativePreference {key:PreferenceKey;effective:string|null;userValue:string|null;origin:ConfigLayer|null;}
export interface NativePreferencesView {
  requestId:string;viewId:string;directory:string;loading:boolean;current:boolean;error:string|null;
  snapshot:{preferences:NativePreference[];layers:ConfigLayer[];target:{file:string;version:string}|null}|null;
}
export const preferenceLabels:Record<string,string>={model:"Default model",review_model:"Review model",model_reasoning_effort:"Default effort",model_reasoning_summary:"Reasoning summaries",model_verbosity:"Response detail",service_tier:"Service tier",web_search:"Web search",model_context_window:"Context-window override",model_auto_compact_token_limit:"Automatic compaction threshold",model_auto_compact_token_limit_scope:"Automatic compaction counting"};
export const preferenceChoices:Record<string,string[]>={model_reasoning_summary:["auto","concise","detailed","none"],model_verbosity:["low","medium","high"],web_search:["disabled","cached","live"],model_auto_compact_token_limit_scope:["total","body_after_prefix"] satisfies NonNullable<Config["model_auto_compact_token_limit_scope"]>[]};
