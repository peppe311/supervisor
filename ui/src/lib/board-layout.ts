export interface BoardLayout {
  selectedChat: string | null; selectedAgent: string | null;
  openChats: string[]; openAgents:string[]; minimized: string[]; focus: string;
  projectsWidth: number; filesWidth: number; chatShare: number;
  filesHidden: boolean;
}
const limit = (value:unknown, fallback:number, min:number, max:number) =>
  typeof value==='number' && Number.isFinite(value) ? Math.min(max,Math.max(min,value)) : fallback;
const ids = (value:unknown):string[] => Array.isArray(value)
  ? [...new Set(value.filter((id):id is string=>typeof id==='string'&&id.length<512))].slice(0,128) : [];
export function boardLayout(value:unknown = {}):BoardLayout {
  const v=(value&&typeof value==='object'?value:{}) as Partial<BoardLayout>;
  return {
    selectedChat:typeof v.selectedChat==='string'?v.selectedChat:null,
    selectedAgent:typeof v.selectedAgent==='string'?v.selectedAgent:null,
    openChats:ids(v.openChats),openAgents:ids(v.openAgents),minimized:ids(v.minimized),focus:typeof v.focus==='string'?v.focus:'',
    projectsWidth:limit(v.projectsWidth,0,0,480),filesWidth:limit(v.filesWidth,0,0,600),
    chatShare:limit(v.chatShare,.45,.25,.75),filesHidden:v.filesHidden===true,
  };
}
const layouts=new Map<string,BoardLayout>();
export function readBoardLayout(key:string):BoardLayout {
  return boardLayout(layouts.get(key));
}
export function writeBoardLayout(key:string,layout:BoardLayout):void {
  if(!key)return;
  layouts.delete(key);layouts.set(key,boardLayout(layout));
  if(layouts.size>40)layouts.delete(layouts.keys().next().value!);
}
