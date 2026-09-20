import type {ThreadGoal} from '../../../protocol/app-server/0.153.4/typescript/v2/ThreadGoal';
import type {NativeConversationView} from './native-history';

export interface NativeGoalView {
  threadId:string; directory:string; viewId:string; goal:ThreadGoal|null;
  current:boolean; busy:boolean; writing:boolean; error:string|null;
}
export type GoalUserStatus='active'|'paused'|'blocked'|'complete';
export type GoalEdit=
  | {kind:'replace';objective:string;status:GoalUserStatus;token_budget:number|null}
  | {kind:'status';status:GoalUserStatus}
  | {kind:'budget';token_budget:number|null}
  | {kind:'clear'};
export interface GoalChoice {owner:string;thread:string;directory:string;viewId:string;edit:GoalEdit}
export function goalAvailable(view:NativeConversationView|null):boolean {
  return !!view?.visible && view.connected && view.nativeLoaded===true && !!view.directory && !!view.binding && !view.binding.deleted && !view.binding.archived;
}
export function goalEditable(view:NativeConversationView|null):boolean {
  return goalAvailable(view) && !!view?.goal?.current && !view.goal.busy && view.goal.threadId===view.binding?.threadId && view.goal.directory===view.directory;
}
export function goalBudget(limited:boolean,text:string):number|null {
  if(!limited)return null;
  const value=Number(text);
  if(!/^\d+$/.test(text.trim()) || !Number.isSafeInteger(value) || value<1)throw new Error('Enter a positive whole-number token budget, or leave it unlimited.');
  return value;
}
export function goalEditError(edit:GoalEdit):string|null {
  if(edit.kind==='replace' && (!edit.objective.trim() || [...edit.objective].length>4000 || edit.objective.includes('\0')))return 'Enter an objective of 1–4,000 characters.';
  if((edit.kind==='replace'||edit.kind==='budget') && edit.token_budget!==null && (!Number.isSafeInteger(edit.token_budget)||edit.token_budget<1))return 'The optional token budget must be a positive whole number.';
  return null;
}
export function goalChoiceValid(choice:GoalChoice,owner:string,view:NativeConversationView|null):boolean {
  return goalEditable(view) && choice.owner===owner && choice.thread===view?.binding?.threadId && choice.directory===view.directory && choice.viewId===view.goal?.viewId && !goalEditError(choice.edit);
}
export function goalStatus(status:ThreadGoal['status']):string {
  return {active:'Active',paused:'Paused',blocked:'Blocked',usageLimited:'Account usage limit reached',budgetLimited:'Goal budget reached',complete:'Complete'}[status];
}
