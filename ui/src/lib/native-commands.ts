import type {NativeConversationView} from './native-history';

/** UI shortcuts to implemented native controls, not a CLI command interpreter. */
export const nativeCommands = [
  {name:'resume', description:'Browse local and cloud Codex history', scope:'connected'},
  {name:'cloud', description:'Browse ChatGPT subscription cloud chats', scope:'connected'},
  {name:'skills', description:'Choose or configure native skills', scope:'directory'},
  {name:'config', description:'Inspect and edit native Codex defaults', scope:'directory'},
  {name:'history', description:'Read this conversation from Codex', scope:'bound'},
  {name:'goal', description:'Manage this native conversation’s goal…', scope:'goal'},
  {name:'compact', description:'Ask Codex to compact this context…', scope:'observed'},
  {name:'rename', description:'Rename this native conversation…', scope:'observed'},
  {name:'fork', description:'Create an independent conversation branch…', scope:'project'},
  {name:'review', description:'Choose a native code review target…', scope:'project'},
  {name:'archive', description:'Archive this native conversation…', scope:'observed'},
  {name:'unarchive', description:'Restore this archived conversation…', scope:'bound'},
] as const;
export type NativeCommandName = typeof nativeCommands[number]['name'];

export function commandQuery(text:string):string|null {
  if(/[\r\n]/.test(text))return null;
  // Leading whitespace, multiline instructions and filesystem paths are literal input.
  const match = /^\/([a-z-]*)(?:[ \t]*)$/i.exec(text);
  return match ? match[1].toLowerCase() : null;
}
export function commandReason(name:NativeCommandName,view:NativeConversationView|null):string|null {
  if(!view?.visible || !view.connected)return 'Connect Codex in AI accounts first.';
  const scope=nativeCommands.find(command=>command.name===name)!.scope;
  if(scope==='connected')return null;
  if(scope==='directory')return view.directory ? null : 'Select a local project directory first.';
  if(!view.binding || view.binding.deleted)return 'This chat has no available native Codex conversation.';
  if(scope==='goal')return !view.binding.archived && view.nativeLoaded && view.directory ? null : 'Resume a local native conversation before managing its goal.';
  if(view.busy)return 'Wait for this conversation’s current operation to finish.';
  if(name==='unarchive')return view.binding.archived ? null : 'This conversation is not archived.';
  if(scope==='bound')return null;
  if(!view.observed)return 'Read native history after reconnecting first: /history.';
  if(name==='compact' && !view.canCompact)return 'Resume the native connection with existing history before compacting.';
  if(scope==='project' && !view.directory)return 'Select a local project directory first.';
  if(name==='review' && view.binding.archived)return 'Restore the conversation before reviewing code.';
  return null;
}
export function consumeNativeCommand(input:HTMLTextAreaElement):boolean {
  if(!input.value.startsWith('/'))return false;
  return !input.dispatchEvent(new CustomEvent('central-agent:composer-command', {bubbles:true,cancelable:true}));
}
