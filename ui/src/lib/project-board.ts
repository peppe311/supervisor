import { graphConversationKey, type GraphBinding } from './graph-conversations.ts';
import type { ProjectChat } from './chat-tree';
import type {TaskVerification} from './task-verification';

export interface BoardProject {
  id: string; nodeKey: string; name: string; path: string; source: 'local' | 'ssh';
  active?: boolean; pinned?: boolean; sshProfileId?: string | null; sshProfileName?: string | null;
  agentCount?: number; access?: string; scanning?: boolean;
  metadata?: {gitBranch?:string|null;gitModified?:number;stack?:string[];instructionFiles?:string[];
    commands?:{label:string;command:string}[];error?:string|null;files?:BoardFile[];filesTruncated?:boolean} | null;
  activity?: ProjectWork[];
}
export type WorkState = 'working' | 'waiting' | 'done' | 'stopped' | 'failed';
export interface FileWork {path:string;state:WorkState;operation:'read'|'edit';activeOperations:number}
export interface ProjectWork {owner:string;chatId?:string|null;nodeKey?:string|null;label:string;state:WorkState;files:FileWork[];truncated?:boolean}
export interface ActiveFile extends FileWork {owners:{owner:string;label:string;state:WorkState;operation:'read'|'edit'}[]}
export function isWorking(state:WorkState):boolean {return state==='working'||state==='waiting';}
export function workLabel(state:WorkState):string {return ({working:'Working',waiting:'Waiting',done:'Done',stopped:'Stopped',failed:'Failed'})[state];}
export function fileWorkLabel(file:FileWork):string {
  if(file.state==='working')return `${file.operation==='edit'?'Editing':'Reading'}${file.activeOperations>1?` · ${file.activeOperations} operations`:''}`;
  return workLabel(file.state);
}
/** Group shared files without losing parallel owners or treating a finished read as an edit. */
export function activityFiles(work:ProjectWork[],remote=false):ActiveFile[] {
  const files=new Map<string,ActiveFile>();
  const priority=(state:WorkState)=>state==='working'?4:state==='waiting'?3:state==='failed'?2:state==='stopped'?1:0;
  for(const task of work)for(const file of task.files||[]) {
    if(!file.path||file.path.startsWith('/')||/^[a-z]:/i.test(file.path)||file.path.split(/[\\/]/).some(p=>p==='..'||p==='.'||!p))continue;
    const path=file.path.replaceAll('\\','/'),key=remote?path:path.toLowerCase();
    const owner={owner:task.owner,label:task.label,state:file.state,operation:file.operation};
    const old=files.get(key);
    if(!old)files.set(key,{...file,path,owners:[owner]});
    else {
      old.owners.push(owner);old.activeOperations+=file.activeOperations;
      if(priority(file.state)>priority(old.state)){old.state=file.state;old.operation=file.operation;}
      else if(file.state==='working'&&old.state==='working'&&file.operation==='edit')old.operation='edit';
    }
  }
  return [...files.values()].sort((a,b)=>priority(b.state)-priority(a.state)||a.path.localeCompare(b.path));
}
export function fileActivityLabel(files:ActiveFile[],path:string,directory:boolean,remote=false):string {
  const normalize=(value:string)=>remote?value:value.toLowerCase();
  const target=normalize(path);
  const matches=files.filter(file=>normalize(file.path)===target||directory&&normalize(file.path).startsWith(target+'/'));
  const active=matches.filter(file=>isWorking(file.state));
  if(directory)return active.length?`${active.length} active`:matches.length?`${matches.length} worked on`:'';
  return matches.length?fileWorkLabel(matches[0]):'';
}
export interface BoardAgent extends GraphBinding {
  projectDirectory?: string | null; sshProfileId?: string | null; projectChatId?: string | null;
  provider?: string; selection?: {model?:string};
}
export interface BoardFile {path:string;kind:string;iconKey?:string;bytes?:number|null}
export interface FileRow extends BoardFile {name:string;depth:number;expanded:boolean}
export interface BoardState {
  expanded?: boolean;
  projectRegistry?: {projects:BoardProject[];[key:string]:unknown};
  projectChats?: ProjectChat[];activeChatId?:string|null;
  projectChatPreviews?: {id:string;title:string;provider:string;selection:{model?:string;effort?:string;serviceTier?:string|null};agent?:{active?:boolean;phase?:string;status?:string};canResume?:boolean;pendingApproval?:unknown;conversationState?:Record<string,unknown>;messages?:unknown[]}[];
  workspace?: {root?:string|null;entries?:BoardFile[];truncated?:boolean;explorerError?:string|null};
  knowledgeAgents?: {bindings?:BoardAgent[];activeNodeKeys?:string[];links?:{sourceNodeKey:string;targetNodeKey:string}[]};
  knowledgeRuns?: {nodeKey:string;agent?:{active?:boolean;phase?:string;status?:string};conversationState?:{projectDiff?:unknown;nativeRequests?:unknown[]};verification?:TaskVerification|null}[];
}
export function sameLocalPath(a:string,b:string):boolean {
  const normalize=(path:string)=>path.replace(/^\\\\\?\\/,'').replaceAll('\\','/').replace(/\/+$/,'').toLocaleLowerCase();
  return normalize(a)===normalize(b);
}
export function agentInProject(agent:BoardAgent,project:BoardProject):boolean {
  if (`${agent.recordType}:${agent.recordId}` === project.nodeKey) return true;
  if (!agent.projectDirectory) return false;
  return project.source==='ssh'
    ? agent.sshProfileId===project.sshProfileId && agent.projectDirectory.replace(/\/+$/,'')===project.path.replace(/\/+$/,'')
    : !agent.sshProfileId && sameLocalPath(agent.projectDirectory,project.path);
}
export function agentKey(agent:BoardAgent):string {return graphConversationKey(agent);}
export function projectChats(chats:ProjectChat[],project:BoardProject|null):ProjectChat[] {
  return project?.source==='local' ? chats.filter(chat=>!chat.archived&&sameLocalPath(chat.projectRoot,project.path)) : [];
}
/** A bounded project-relative tree; never accept host paths as navigation roots. */
export function fileRows(entries:BoardFile[],expanded:ReadonlySet<string>,query=''):FileRow[] {
  const nodes=new Map<string,BoardFile>();
  for (const entry of entries.slice(0,1500)) {
    const path=entry.path.replaceAll('\\','/');
    if(!path||path.startsWith('/')||/^[a-z]:/i.test(path)||path.split('/').some(p=>p==='..'||p==='.'||!p))continue;
    nodes.set(path,{...entry,path});
    const parts=path.split('/');
    for(let end=1;end<parts.length;end++) {
      const parent=parts.slice(0,end).join('/');
      if(!nodes.has(parent))nodes.set(parent,{path:parent,kind:'directory'});
    }
  }
  const normalized=query.trim().toLocaleLowerCase();
  const children=new Map<string,BoardFile[]>();
  for(const node of nodes.values()) {
    const parent=node.path.includes('/')?node.path.slice(0,node.path.lastIndexOf('/')):'';
    const group=children.get(parent)||[];group.push(node);children.set(parent,group);
  }
  const sort=(a:BoardFile,b:BoardFile)=>Number(b.kind==='directory')-Number(a.kind==='directory')||a.path.localeCompare(b.path);
  if(normalized)return [...nodes.values()].filter(n=>n.path.toLocaleLowerCase().includes(normalized)).sort(sort)
    .map(n=>({...n,name:n.path,depth:0,expanded:expanded.has(n.path)}));
  const rows:FileRow[]=[];
  const visit=(parent:string,depth:number)=>{
    for(const node of (children.get(parent)||[]).sort(sort)) {
      rows.push({...node,name:node.path.split('/').pop()!,depth,expanded:expanded.has(node.path)});
      if(node.kind==='directory'&&expanded.has(node.path))visit(node.path,depth+1);
    }
  };
  visit('',0);return rows;
}
