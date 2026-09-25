export type WorkMode='summary'|'compare'|'handoff';
export interface WorkEvent {
  role:string;kind:string;phase:string|null;text:string;category:string|null;status:string|null;
  detail:string;diff:string;attachments:{name:string;kind:string}[];
}
export interface WorkReport {
  owner:string;title:string;root:string;remote:boolean;busy:boolean;loaded:boolean;partial:boolean;
  status:string;revision:string;native:boolean;threadId:string|null;turnId:string|null;canHandoff:boolean;
  historyState?:string;historyError?:string|null;
  evidence:{objective:string;finalAnswer:string;events:WorkEvent[];eventsOmitted:number};
  commands:{command:string;status:string;exitCode:number|null;output:string}[];commandsOmitted?:number;
  files:{path:string;state:string;operation:string}[];filesTruncated?:boolean;
  diff:{additions:number;deletions:number;fileCount:number}|null;
  checks:{step:string;status:string}[];pendingRequests:number;
  assessment:{state:string;summary:string}|null;
}
export interface WorkCandidate {owner:string;title:string;busy:boolean;reviewer:boolean;threadId:string|null}
export function comparisonReady(a:WorkReport|null,b:WorkReport|null,reviewer:WorkCandidate|undefined):boolean {
  return !!a && !!b && a.owner!==b.owner && a.loaded && b.loaded && !a.busy && !b.busy
    && !a.partial && !b.partial && !a.remote && !b.remote && !a.pendingRequests && !b.pendingRequests && !!reviewer?.reviewer && !reviewer.busy
    && reviewer.owner!==a.owner && reviewer.owner!==b.owner && !!reviewer.threadId;
}
export function commandOutcome(command:WorkReport['commands'][number]):string {
  if(command.status==='succeeded' && command.exitCode===0)return 'Exit 0';
  if(command.exitCode!==null)return `Exit ${command.exitCode}`;
  return ({failed:'Failed',stopped:'Stopped',running:'Running',not_reported:'Exit not reported'} as Record<string,string>)[command.status]||'Exit not reported';
}
export function workStatus(report:WorkReport):string {
  if(report.busy)return 'Working';
  return ({completed:'Completed',inProgress:'Interrupted connection',failed:'Failed',interrupted:'Stopped'} as Record<string,string>)[report.status]||report.status;
}
