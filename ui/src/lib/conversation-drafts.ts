import { requestId } from "./request-id.ts";

type DraftRequest = {owner:string;request_id:string;action:{kind:"read"}|{kind:"write";revision:string;text:string}};
type Remote = {send:(request:DraftRequest)=>void;updated:(owner:string,error:string)=>void};
type State = {revision:string;loaded:boolean;dirty:boolean;pending?:{id:string;kind:"read"|"write";text?:string};error:string};
/** Native storage is optional; opaque WebViews always retain a local draft cache. */
export class ConversationDrafts {
  private readonly values = new Map<string, string>();
  private readonly storage: () => Pick<Storage, "getItem" | "setItem" | "removeItem">;
  private readonly prefix: string;
  private readonly remote?: Remote;
  private readonly states = new Map<string, State>();
  private readonly localEdits = new Set<string>();

  constructor(storage: () => Pick<Storage, "getItem" | "setItem" | "removeItem">, prefix: string, remote?:Remote) {
    this.storage = storage;
    this.prefix = prefix;
    this.remote = remote;
  }

  read(owner: string): string {
    if(this.remote && /^(chat:|graph:|draft:)/.test(owner) && !this.states.has(owner)) {
      const id=requestId();
      this.states.set(owner,{revision:"",loaded:false,dirty:this.localEdits.delete(owner),pending:{id,kind:"read"},error:""});
      this.remote.send({owner,request_id:id,action:{kind:"read"}});
    }
    return this.readLocal(owner);
  }

  readLocal(owner: string): string {
    if (this.values.has(owner)) return this.values.get(owner)!;
    let value = "";
    try { value = this.storage().getItem(`${this.prefix}:${owner}`) ?? ""; } catch { /* Local cache remains authoritative. */ }
    this.values.set(owner, value);
    return value;
  }

  writeLocal(owner: string, value: string): void {
    if (!owner || this.readLocal(owner) === value) return;
    this.cache(owner, value);
    const state = this.states.get(owner);
    if (state) state.dirty = true;
    else this.localEdits.add(owner);
  }

  synchronize(owner: string): string {
    const text = this.read(owner), state = this.states.get(owner);
    if (state) this.flush(owner, state);
    return text;
  }

  write(owner: string, value: string): void {
    if (!owner) return;
    const previous=this.read(owner);
    if(this.remote && previous===value)return;
    this.cache(owner,value);
    const state=this.states.get(owner);
    if(state){state.dirty=true;this.remote?.updated(owner,state.error);this.flush(owner,state);}
  }

  private cache(owner:string,value:string):void {
    this.values.set(owner, value);
    try {
      if (value) this.storage().setItem(`${this.prefix}:${owner}`, value);
      else this.storage().removeItem(`${this.prefix}:${owner}`);
    } catch { /* Navigation does not depend on storage availability. */ }
  }

  private flush(owner:string,state:State):void {
    if(!this.remote || !state.loaded || state.pending || !state.dirty)return;
    const text=this.values.get(owner)||"",id=requestId();
    state.pending={id,kind:"write",text};
    this.remote.updated(owner,state.error);
    this.remote.send({owner,request_id:id,action:{kind:"write",revision:state.revision,text}});
  }

  error(owner:string):string {return this.states.get(owner)?.error||"";}
  status(owner:string):string {
    const s=this.states.get(owner);
    return !s?"local":s.error?"error":!s.loaded?"loading":s.pending||s.dirty?"saving":"saved";
  }

  forget(owner:string,revision:string):void {
    this.cache(owner,"");
    this.localEdits.delete(owner);
    this.states.set(owner,{revision,loaded:true,dirty:false,error:""});
    this.remote?.updated(owner,"");
  }

  receive(detail:unknown):void {
    if(!detail || typeof detail!=="object")return;
    const d=detail as {owner?:unknown;requestId?:unknown;revision?:unknown;text?:unknown;error?:unknown};
    if(typeof d.owner!=="string")return;
    const state=this.states.get(d.owner),pending=state?.pending;
    if(!state || !pending || d.requestId!==pending.id)return;
    state.pending=undefined;
    if(typeof d.error==="string" || typeof d.revision!=="string" || pending.kind==="read" && typeof d.text!=="string") {
      state.error=typeof d.error==="string"?d.error:"The local draft response was incomplete. Copy your text before closing.";
      this.remote?.updated(d.owner,state.error);return;
    }
    state.revision=d.revision;state.loaded=true;state.error="";
    if(pending.kind==="read") {
      if(!state.dirty) {
        // Import a session-only draft only when no durable record ever existed.
        // A saved empty record is a tombstone, not permission to resurrect text.
        if(!d.revision && this.values.get(d.owner))state.dirty=true;
        else this.cache(d.owner,d.text as string);
      }
    } else state.dirty=this.values.get(d.owner)!==pending.text;
    this.remote?.updated(d.owner,"");
    this.flush(d.owner,state);
  }

  seed(owner: string, value: string): boolean {
    if (!owner || !value || (this.read(owner) && this.read(owner) !== value)) return false;
    this.write(owner, value);
    return true;
  }
}
