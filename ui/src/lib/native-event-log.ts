export interface NativeEventEntry {
  sequence:number;receivedAtMs:number;connectionEpoch:number;direction:string;method:string;
  owner:string|null;threadId:string|null;turnId:string|null;itemId:string|null;requestId:string|null;
  state:string|null;summaryIndex:number|null;deltaBytes:number|null;
}
export interface DeliveryTiming {
  messageId:string;mode:string;outcome:string;
  clientToHostMs:number|null;preparedMs:number|null;writtenMs:number|null;
  acknowledgedMs:number|null;nativeInputMs:number|null;firstUpdateMs:number|null;
}
export interface NativeEventLog {entries:NativeEventEntry[];retainedLimit:number;viewLimit:number;omitted:number;persistent:boolean;error:string|null;deliveryTimings?:DeliveryTiming[]}
export function deliveryTime(value:number|null):string {
  return value !== null && Number.isFinite(value) && value >= 0 ? `${value.toFixed(1)} ms` : 'Not observed';
}
export function deliveryMode(mode:string):string {
  return ({ready:'Session ready',preparing:'Joined session preparation',resume:'Session resumed at send',new:'New conversation',steer:'Follow-up to active work'} as Record<string,string>)[mode] || 'Prompt';
}
export function eventTime(entry:NativeEventEntry):string {
  const date=new Date(entry.receivedAtMs);
  return Number.isFinite(date.getTime())?date.toLocaleTimeString(undefined,{hour12:false,hour:'2-digit',minute:'2-digit',second:'2-digit',fractionalSecondDigits:3}):'Time unavailable';
}
export function eventMeaning(entry:NativeEventEntry):string {
  if(entry.method==='turn/start' && entry.direction==='dispatch')return 'Prompt submitted';
  if(entry.method==='turn/start' && entry.direction==='response')return entry.state==='accepted'?'Prompt acknowledged':entry.state==='delivery_unknown'?'Prompt delivery uncertain':'Prompt rejected';
  if(entry.method==='turn/started')return 'Work started';
  if(entry.method==='turn/completed')return 'Turn ended';
  if(entry.method.startsWith('item/reasoning/summary'))return 'Reasoning summary update';
  if(entry.direction==='request')return 'Input or approval requested';
  if(entry.method==='request/answer')return 'Reply to native request';
  if(entry.method==='serverRequest/resolved')return 'Native request resolved';
  if(entry.method==='turn/plan/updated')return 'Plan updated';
  if(entry.method==='item/started')return 'Activity started';
  if(entry.method==='item/completed')return 'Activity completed';
  if(entry.direction==='connection')return 'Connection closed';
  if(entry.direction==='dispatch')return 'Operation dispatched';
  if(entry.direction==='response')return 'Operation acknowledged';
  return 'Native update';
}
