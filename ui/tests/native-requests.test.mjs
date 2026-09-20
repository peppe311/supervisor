import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import ts from "typescript";
import { compile } from "svelte/compiler";
import { render } from "svelte/server";
import { requestSnapshot,formFields,formAnswer,requestPresentation,schemaFormAnswer,schemaFormTemplate } from "../src/lib/native-requests.ts";

function requestsComponent(graph=false) {
  let source=fs.readFileSync(new URL('../src/components/NativeRequests.svelte',import.meta.url),'utf8').match(/<script lang="ts">([\s\S]*?)<\/script>/)[1];
  const ast=ts.createSourceFile('requests.ts',source,ts.ScriptTarget.Latest,true);
  for(const statement of [...ast.statements].reverse())if(ts.isImportDeclaration(statement))source=source.slice(0,statement.pos)+source.slice(statement.end);
  const window=new EventTarget(),target=new EventTarget(),mounts=[];
  const ctx=vm.createContext({Event,CustomEvent,window,document:target,requestSnapshot,
    $state:value=>value,$props:()=>graph?{owner:'graph:a',eventTarget:target}:{},onMount:callback=>mounts.push(callback)});
  vm.runInContext(ts.transpileModule(source,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText,ctx);
  mounts.forEach(callback=>callback());
  const owner=graph?'graph:a':'chat:a';
  if(!graph)window.dispatchEvent(new CustomEvent('central-agent:conversation-key',{detail:owner}));
  const state=connected=>(graph?target:window).dispatchEvent(new CustomEvent('central-agent:conversation-state',{detail:{owner,nativeConversation:{connected},nativeBinding:{threadId:'native-a',deleted:false},nativeLoaded:false}}));
  return {window,state,owner,run:code=>vm.runInContext(code,ctx)};
}

test("main and graph request snapshots preserve native-host arrival order and surviving tickets",()=>{
  for(const owner of ["chat:a","graph:b"]) {
    const cards=[2,10,11,100].map(serial=>({owner,ticket:`request:7:${serial}`,kind:"command",params:{command:"fixture"},responding:false}));
    const foreign={...cards[0],owner:"foreign"};
    assert.deepEqual(requestSnapshot({owner,nativeRequests:[...cards,foreign]},owner),cards);
    const sending=cards.map((card,index)=>index===1?{...card,responding:true}:card);
    assert.deepEqual(requestSnapshot({owner,nativeRequests:sending},owner),sending);
    const remaining=sending.filter((_,index)=>index!==1);
    assert.deepEqual(requestSnapshot({owner,nativeRequests:remaining},owner),remaining);
    assert.deepEqual(requestSnapshot({owner,nativeRequests:[]},owner),[]);
  }
});

test("main and graph decision surfaces clear connection-required errors after reconnect",()=>{
  for(const graph of [false,true]) {
    const c=requestsComponent(graph);
    c.state(false);
    c.window.dispatchEvent(new CustomEvent('central-agent:app-server-conversation',{detail:{owner:c.owner,error:'Connect Codex App Server in AI Settings first'}}));
    assert.match(c.run('error'),/Connect Codex App Server/);
    c.state(true);
    assert.equal(c.run('error'),'');

    c.window.dispatchEvent(new CustomEvent('central-agent:app-server-conversation',{detail:{owner:c.owner,error:'Native operation failed'}}));
    c.state(true);
    assert.equal(c.run('error'),'Native operation failed');
  }
});

test("compiled approval card renders only Rust-projected native decisions",async()=>{
  const source=fs.readFileSync(new URL('../src/components/NativeRequestCard.svelte',import.meta.url),'utf8');
  let code=compile(source,{filename:'NativeRequestCard.svelte',generate:'server'}).js.code;
  // Diff rendering is outside this command-card check; fail if it is invoked.
  code=code.replace(/import DiffViewer from ["']\.\/DiffViewer\.svelte["'];/,"const DiffViewer=()=>{throw new Error('Unexpected file diff in command fixture');};");
  code=code.replace(/from (["'])([^"']+)\1/g,(whole,quote,specifier)=>{
    const target=specifier==='../lib/native-requests'
      ? new URL('../src/lib/native-requests.ts',import.meta.url).href : import.meta.resolve(specifier);
    return `from ${JSON.stringify(target)}`;
  });
  const {default:Card}=await import(`data:text/javascript;base64,${Buffer.from(code).toString('base64')}`);
  const none={accept:false,decline:false,cancel:false,session:false,execPolicy:false,networkPolicies:[]};
  const draw=choices=>render(Card,{props:{request:{kind:'command',ticket:'fixture',owner:'chat:fixture',params:{command:'fixture',proposedExecpolicyAmendment:['cargo','test'],proposedNetworkPolicyAmendments:[{host:'one.test',action:'allow'},{host:'two.test',action:'deny'}]},choices},onaction(){throw new Error('Rendering granted authority');}}}).body;
  const restricted=draw({...none,accept:true,cancel:true,execPolicy:true});
  for(const text of ['Allow once','Cancel request','Accept proposed command policy']) assert.ok(restricted.includes(text));
  for(const text of ['>Decline<','Allow for session','Apply this network policy']) assert.ok(!restricted.includes(text));
  const network=draw({...none,networkPolicies:[1]});
  assert.ok(network.includes('two.test'));assert.ok(!network.includes('one.test'));
  assert.equal((network.match(/Apply this network policy/g)||[]).length,1);
  for(const choices of [none,null,undefined]) {
    const empty=draw(choices);assert.match(empty,/did not offer a supported decision/);
    assert.ok(!empty.includes('<button'));assert.ok(!empty.includes('Session and policy choices'));
  }
});

test("actual request card initializes form defaults once for each native ticket",()=>{
  let source=fs.readFileSync(new URL('../src/components/NativeRequestCard.svelte',import.meta.url),'utf8').match(/<script lang="ts">([\s\S]*?)<\/script>/)[1];
  const ast=ts.createSourceFile('card.ts',source,ts.ScriptTarget.Latest,true);
  for(const statement of [...ast.statements].reverse())if(ts.isImportDeclaration(statement))source=source.slice(0,statement.pos)+source.slice(statement.end);
  const make=(ticket,initial)=>{
    let calls=0;
    const request={ticket,kind:'mcp',params:{mode:'form',requestedSchema:{type:'object',properties:{label:{type:'string',default:initial}}}}};
    const ctx=vm.createContext({$props:()=>({request,onaction(){}}),$state:value=>value,$derived:value=>value,untrack:fn=>fn(),
      formFields:schema=>{calls++;return formFields(schema);},formAnswer,requestPresentation,schemaFormTemplate,extendedFormMode:()=>false,schemaFormAnswer});
    vm.runInContext(ts.transpileModule(source,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText,ctx);
    const fields=vm.runInContext('fields',ctx);
    vm.runInContext('request={...request,responding:true,params:JSON.parse(JSON.stringify(request.params))}',ctx);
    assert.equal(vm.runInContext('fields',ctx),fields);assert.equal(calls,1);
    return fields;
  };
  assert.equal(make('first','one')[0].initial,'one');
  assert.equal(make('second','two')[0].initial,'two');
});

test("native network approvals identify the destination rather than a shell command",()=>{
  for(const protocol of ["http","https","socks5Tcp","socks5Udp"]) {
    const network={host:"service.example:443",protocol};
    const request={kind:"command",params:{networkApprovalContext:network,command:"not the approval target"}};
    const view=requestPresentation(request);
    assert.equal(view.title,"Allow network access?");assert.deepEqual(view.network,network);
    assert.equal(view.commandContext,true);assert.match(view.explanation,/not a general command approval/);
    assert.deepEqual(request.params.networkApprovalContext,network);
  }
});

test("stdin callbacks are distinguished from new commands without inventing missing previews",()=>{
  const view=requestPresentation({kind:"command",params:{kind:"writeStdin"}});
  assert.equal(view.title,"Allow input to a running process?");assert.equal(view.commandContext,true);
  assert.equal(view.network,null);
  for(const params of [{},{kind:"command"},{networkApprovalContext:null}]) {
    const command=requestPresentation({kind:"command",params});
    assert.equal(command.title,"Allow this command?");assert.equal(command.commandContext,false);
  }
  // Native network context takes precedence even if the callback belongs to stdin.
  assert.equal(requestPresentation({kind:"command",params:{kind:"writeStdin",networkApprovalContext:{host:"example.test",protocol:"https"}}}).title,"Allow network access?");
  for(const kind of ["files","permissions","questions","mcp"]) {
    assert.equal(requestPresentation({kind,params:{kind:"writeStdin"}}).commandContext,false);
  }
});

test("request snapshots are owner scoped, including empty resolution",()=>{
  const item={ticket:"request:1:2",owner:"graph:a",kind:"command",params:{command:"pwd"}};
  assert.equal(requestSnapshot({owner:"graph:a",nativeRequests:[item]},"chat:a"),undefined);
  assert.deepEqual(requestSnapshot({owner:"graph:a",nativeRequests:[item,{...item,owner:"graph:b"}]},"graph:a"),[item]);
  assert.deepEqual(requestSnapshot({owner:"graph:a",nativeRequests:[]},"graph:a"),[]);
  assert.equal(requestSnapshot({owner:"graph:a"},"graph:a"),undefined);
});
test("MCP form preserves boolean false, numeric and multi-select values",()=>{
  const fields=formFields({type:"object",required:["enabled","count"],properties:{enabled:{type:"boolean"},count:{type:"integer",minimum:1},scopes:{type:"array",minItems:1,items:{enum:["read","write"]}},extra:{type:"string"}}});
  const data=new FormData();data.set("enabled","false");data.set("count","2");data.append("scopes","read");
  assert.deepEqual({...formAnswer(fields,data)},{enabled:false,count:2,scopes:["read"]});
  data.delete("scopes");assert.throws(()=>formAnswer(fields,data));
});
test("forms reject unsupported nested structures and use native enum labels",()=>{
  assert.equal(formFields({type:"object",properties:{nested:{type:"object"}}}),null);
  assert.equal(formFields({type:"array"}),null);
  const fields=formFields({type:"object",properties:{mode:{type:"string",oneOf:[{const:"read",title:"Read only"}]}}});
  assert.deepEqual(fields[0].options,[{value:"read",label:"Read only"}]);
});
test("extended MCP forms build a bounded structured template and require an object",()=>{
  const schema={type:"object",required:["profile"],properties:{profile:{type:"object",required:["name"],properties:{name:{type:"string"},enabled:{type:"boolean",default:true}}},tags:{type:"array",items:{type:"string"}}}};
  assert.deepEqual(schemaFormAnswer(schemaFormTemplate(schema)),{profile:{name:"",enabled:true}});
  assert.throws(()=>schemaFormAnswer("[1,2]"),/object/);
  assert.throws(()=>schemaFormAnswer("{"),/valid JSON/);
});
