import type { ToolRequestUserInputQuestion } from "../../../protocol/app-server/0.153.4/typescript/v2/ToolRequestUserInputQuestion";
import type { CommandExecutionApprovalKind } from "../../../protocol/app-server/0.153.4/typescript/v2/CommandExecutionApprovalKind";
import type { NetworkApprovalContext } from "../../../protocol/app-server/0.153.4/typescript/v2/NetworkApprovalContext";

export type NativeRequest = {
  ticket:string; owner:string; kind:"command"|"files"|"permissions"|"questions"|"mcp";
  threadId:string; turnId:string|null; responding:boolean;
  // Rust projects only supported decisions offered by the native request.
  choices?:{accept:boolean;decline:boolean;cancel:boolean;session:boolean;execPolicy:boolean;networkPolicies:number[]}|null;
  params: {
    reason?:string; command?:string; cwd?:string; grantRoot?:string; environmentId?:string;
    kind?:CommandExecutionApprovalKind; commandActions?:unknown[]; networkApprovalContext?:NetworkApprovalContext|null;
    proposedExecpolicyAmendment?:string[]; proposedNetworkPolicyAmendments?:unknown[];
    permissions?:unknown; questions?:ToolRequestUserInputQuestion[];
    mode?:string; serverName?:string; message?:string; url?:string; requestedSchema?:unknown;
  };
  item?: {changes?: {path:string;diff:string}[]};
};

export function requestPresentation(request:NativeRequest): {title:string; explanation:string; network:NetworkApprovalContext|null; commandContext:boolean} {
  const network=request.kind==="command" ? request.params.networkApprovalContext ?? null : null;
  if(network) return {
    title:"Allow network access?", network, commandContext:true,
    explanation:"Codex is requesting access to this network destination. This is not a general command approval and may cover multiple requests to this destination.",
  };
  if(request.kind==="command" && request.params.kind==="writeStdin") return {
    title:"Allow input to a running process?", network:null, commandContext:true,
    explanation:"Codex wants to send input to an already running process. This decision does not start a new shell command.",
  };
  const titles={command:"Allow this command?",files:"Allow these file changes?",permissions:"Grant these permissions?",questions:"Codex needs your input",mcp:"MCP request"};
  return {title:titles[request.kind],explanation:"",network:null,commandContext:false};
}

export function requestSnapshot(detail:unknown, owner:string): NativeRequest[]|undefined {
  if(!owner || !detail || typeof detail!=="object") return undefined;
  const d=detail as {owner?:unknown;nativeRequests?:unknown};
  if(d.owner!==owner || !Array.isArray(d.nativeRequests)) return undefined;
  return d.nativeRequests.filter((r):r is NativeRequest => r && r.owner===owner && typeof r.ticket==="string" && typeof r.kind==="string" && r.params && typeof r.params==="object");
}

type Primitive = string|number|boolean|string[];
export type FormField = {
  key:string;title:string;description:string;type:string;required:boolean;
  options:{value:string;label:string}[];min?:number;max?:number;minLength?:number;maxLength?:number;
  minItems?:number;maxItems?:number;format?:string;initial?:Primitive;
};
export function formFields(schema:unknown): FormField[]|null {
  if(!schema || typeof schema!=="object") return null;
  const s=schema as Record<string,unknown>;
  if(s.type!=="object" || !s.properties || typeof s.properties!=="object") return null;
  const required=Array.isArray(s.required)?s.required:[];
  const fields:FormField[]=[];
  for(const [key,raw] of Object.entries(s.properties)) {
    if(!raw || typeof raw!=="object") return null;
    const f=raw as Record<string,unknown>,type=String(f.type);
    if(!["string","number","integer","boolean","array"].includes(type)) return null;
    const choices=(type==="array" ? f.items : f) as Record<string,unknown>|undefined;
    const titled=choices?.oneOf ?? choices?.anyOf;
    const options=Array.isArray(titled) ? titled.map(o=>({value:String(o.const),label:String(o.title ?? o.const)}))
      : Array.isArray(choices?.enum) ? choices.enum.map((v,i)=>({value:String(v),label:String(Array.isArray(f.enumNames)?f.enumNames[i] ?? v:v)})) : [];
    if(type==="array" && !options.length) return null;
    fields.push({key,title:String(f.title ?? key),description:String(f.description ?? ""),type,required:required.includes(key),options,
      min:typeof f.minimum==="number"?f.minimum:undefined,max:typeof f.maximum==="number"?f.maximum:undefined,
      minLength:typeof f.minLength==="number"?f.minLength:undefined,maxLength:typeof f.maxLength==="number"?f.maxLength:undefined,
      minItems:typeof f.minItems==="number"?f.minItems:undefined,maxItems:typeof f.maxItems==="number"?f.maxItems:undefined,
      format:typeof f.format==="string"?f.format:undefined,initial:f.default as Primitive|undefined});
  }
  return fields;
}

export function formAnswer(fields:FormField[], data:FormData): Record<string,Primitive> {
  const result:Record<string,Primitive>=Object.create(null);
  for(const field of fields) {
    const text=String(data.get(field.key) ?? "");
    if(field.type==="array") {
      const selected=data.getAll(field.key).map(String);
      if(field.minItems!==undefined && selected.length<field.minItems || field.maxItems!==undefined && selected.length>field.maxItems) throw new Error(`Check the number of choices for ${field.title}.`);
      if(selected.length || field.required) result[field.key]=selected;
    } else if(text!=="" || field.required) {
      result[field.key]=field.type==="boolean" ? text==="true" : field.type==="number" || field.type==="integer" ? Number(text) : text;
    }
  }
  return result;
}

export function extendedFormMode(mode:unknown):boolean {
  return mode==="openai/form" || mode==="openaiForm";
}

function schemaInitial(schema:unknown,required=false):unknown {
  if(!schema || typeof schema!=="object")return null;
  const field=schema as Record<string,unknown>;
  if(Object.hasOwn(field,"default"))return field.default;
  if(field.type==="object" && field.properties && typeof field.properties==="object") {
    const requiredKeys=new Set(Array.isArray(field.required)?field.required.filter((key):key is string=>typeof key==="string"):[]);
    const result:Record<string,unknown>=Object.create(null);
    for(const [key,child] of Object.entries(field.properties)) {
      const value=schemaInitial(child,requiredKeys.has(key));
      if(requiredKeys.has(key) || (child && typeof child==="object" && Object.hasOwn(child,"default")))result[key]=value;
    }
    return result;
  }
  if(field.type==="array")return [];
  if(field.type==="boolean")return false;
  if(field.type==="number" || field.type==="integer")return typeof field.minimum==="number"?field.minimum:0;
  if(field.type==="null")return null;
  return required?"":undefined;
}

export function schemaFormTemplate(schema:unknown):string {
  const initial=schemaInitial(schema,true);
  return JSON.stringify(initial && typeof initial==="object"?initial:{},null,2);
}

export function schemaFormAnswer(text:string):Record<string,unknown> {
  let value:unknown;
  try {value=JSON.parse(text);} catch {throw new Error("Enter valid JSON for the requested form.");}
  if(!value || typeof value!=="object" || Array.isArray(value))throw new Error("The requested form response must be a JSON object.");
  return value as Record<string,unknown>;
}
