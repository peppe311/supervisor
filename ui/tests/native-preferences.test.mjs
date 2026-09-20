import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import ts from "typescript";
import {requestId as newRequestId} from "../src/lib/request-id.ts";

function component(graph=false) {
  let source=fs.readFileSync(new URL("../src/components/NativePreferences.svelte",import.meta.url),"utf8").match(/<script lang="ts">([\s\S]*?)<\/script>/)[1];
  const ast=ts.createSourceFile("component.ts",source,ts.ScriptTarget.Latest,true);
  for(const statement of [...ast.statements].reverse())if(ts.isImportDeclaration(statement))source=source.slice(0,statement.pos)+source.slice(statement.end);
  const target=new EventTarget(),window=new EventTarget(),sent=[],mounts=[],effects=[],owner=graph?"graph:a":"chat:a";
  const dialog={open:false,showModal(){this.open=true;},close(){this.open=false;}};
  target.addEventListener("central-agent:app-server-preferences-control",event=>sent.push(event.detail));
  const ctx=vm.createContext({CustomEvent,Event,window,document:target,newRequestId,
    onMount:callback=>mounts.push(callback),$effect:callback=>effects.push(callback),$state:value=>value,
    $props:()=>({owner,eventTarget:graph?target:undefined,conversation:{visible:true,connected:true,busy:false,directory:"C:\\fixture",binding:null}}),fixtureDialog:dialog});
  vm.runInContext(ts.transpileModule(source,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText,ctx);
  vm.runInContext("dialog=fixtureDialog",ctx);mounts.forEach(callback=>callback());
  const run=code=>vm.runInContext(code,ctx);
  function list(patch={},destination=owner,requestId=run("requestId"),writing=false) {
    window.dispatchEvent(new CustomEvent("central-agent:app-server-preferences",{detail:{owner:destination,writing,view:{requestId,viewId:"view-a",directory:"C:\\fixture",loading:false,current:true,error:null,snapshot:{preferences:[{key:"model_verbosity",effective:"high",userValue:"medium",origin:null}],layers:[],target:{file:"C:\\fixture\\config.toml",version:"native-v1"}},...patch}}}));
  }
  return {sent,run,list,dialog,window,effects,owner};
}

test("config shortcut opens only its owner and never writes a preference",()=>{
  for(const graph of [false,true]){
    const c=component(graph);
    const foreign=new CustomEvent('central-agent:native-command',{cancelable:true,detail:{owner:'other',command:'config'}});c.window.dispatchEvent(foreign);assert.equal(foreign.defaultPrevented,false);assert.equal(c.sent.length,0);
    const event=new CustomEvent('central-agent:native-command',{cancelable:true,detail:{owner:c.owner,command:'config'}});c.window.dispatchEvent(event);assert.equal(event.defaultPrevented,true);assert.equal(c.dialog.open,true);assert.deepEqual(c.sent.map(e=>e.action.kind),['refresh']);
  }
});

test("main and graph native preferences save only after a frozen confirmation",()=>{
  for(const graph of [false,true]) {
    const c=component(graph);c.run("open()");assert.equal(c.sent[0].action.kind,"refresh");assert.equal(c.sent[0].action.expected_directory,"C:\\fixture");
    c.list();c.run('select(inventory.snapshot.preferences[0]);value="low";review()');assert.equal(c.sent.length,1);
    c.run('value="high";confirm();confirm()');assert.equal(c.sent.length,2);
    assert.deepEqual(JSON.parse(JSON.stringify(c.sent[1])),{owner:c.owner,action:{kind:"save",view_id:"view-a",key:"model_verbosity",value:"low"}});
    assert.equal(c.run("writing"),true);
  }
});
test("foreign and superseded preference reads cannot replace the current inventory",()=>{
  const c=component();c.run("open()");c.list({},"graph:other");assert.equal(c.run("inventory"),null);
  c.list({},c.owner,"old-request");assert.equal(c.run("inventory"),null);
  c.list();c.run('select(inventory.snapshot.preferences[0]);value="low";review()');c.list({current:false});c.run("confirm()");assert.equal(c.sent.length,1);assert.equal(c.run("choice"),null);
});

test("main and graph compaction counting saves either native enum and clears only the saved override",()=>{
  for(const graph of [false,true])for(const value of ["total","body_after_prefix"]) {
    const c=component(graph);c.run("open()");
    c.list();c.run('inventory.snapshot.preferences=[{key:"model_auto_compact_token_limit_scope",effective:"total",userValue:"body_after_prefix",origin:null}]');
    c.run(`select(inventory.snapshot.preferences[0]);value=${JSON.stringify(value)};review()`);
    assert.equal(c.sent.length,1);
    c.run('value="later edit";confirm();confirm()');
    assert.deepEqual(JSON.parse(JSON.stringify(c.sent[1])),{owner:c.owner,action:{kind:"save",view_id:"view-a",key:"model_auto_compact_token_limit_scope",value}});
    c.list();c.run('inventory.snapshot.preferences=[{key:"model_auto_compact_token_limit_scope",effective:"total",userValue:"body_after_prefix",origin:null}];reviewClear(inventory.snapshot.preferences[0]);confirm()');
    assert.deepEqual(JSON.parse(JSON.stringify(c.sent[2])),{owner:c.owner,action:{kind:"clear",view_id:"view-a",key:"model_auto_compact_token_limit_scope"}});
    assert.equal(c.sent.some(e=>e.action.kind==="reload"||e.action.kind==="start"),false);
  }
});
test("preference confirmation expires on owner directory revision target or active-work changes",()=>{
  for(const change of ['conversation.directory="C:\\\\other"','owner="graph:other"','conversation.busy=true','conversation.connected=false','inventory.snapshot.target.version="v2"','inventory.snapshot.target.file="C:\\\\other.toml"','inventory.viewId="new-view"','inventory.snapshot.target=null','inventory.snapshot.preferences=[]']) {
    const c=component();c.run("open()");c.list();c.run('select(inventory.snapshot.preferences[0]);value="low";review()');c.run(change);c.run("confirm()");assert.equal(c.sent.filter(e=>e.action.kind==="save").length,0,change);
  }
});
test("closing a native preferences dialog preserves a sent write without cancel or replay",()=>{
  const c=component();c.run("open()");c.list();c.run('select(inventory.snapshot.preferences[0]);value="low";review();confirm();close()');
  assert.deepEqual(c.sent.map(e=>e.action.kind),["refresh","save","close"]);
  c.run('conversation.connected=false');c.effects.forEach(effect=>effect());assert.equal(c.dialog.open,false);
  assert.equal(c.sent.filter(e=>e.action.kind==="save").length,1);
  assert.equal(c.sent.some(e=>e.action.kind==="cancel"),false);
});
test("shared skill writes disable preference edits and unlock after the native result",()=>{
  const c=component();c.run("open()");c.list({},c.owner,c.run("requestId"),true);
  c.run('select(inventory.snapshot.preferences[0]);review();confirm();refresh()');assert.equal(c.sent.length,1);
  c.list({current:false});assert.equal(c.run("writing"),false);c.run("refresh()");assert.equal(c.sent.at(-1).action.kind,"refresh");
});
test("preference project switches close the original view without affecting another owner",()=>{
  const c=component();c.run("open()");const request=c.sent[0].action.request_id;
  c.run('owner="graph:other"');c.effects.forEach(effect=>effect());assert.equal(c.dialog.open,false);
  assert.deepEqual(JSON.parse(JSON.stringify(c.sent.at(-1))),{owner:"chat:a",action:{kind:"close",request_id:request}});
});

test("main and graph clear only an observed saved value after explicit confirmation",()=>{
  for(const graph of [false,true]) {
    const c=component(graph);c.run("open()");c.list();
    c.run("reviewClear(inventory.snapshot.preferences[0])");assert.equal(c.sent.length,1);
    assert.equal(c.run("choice.operation"),"clear");assert.equal(c.run("choice.value"),"medium");
    c.run("confirm();confirm();close()");
    assert.deepEqual(JSON.parse(JSON.stringify(c.sent[1])),{owner:c.owner,action:{kind:"clear",view_id:"view-a",key:"model_verbosity"}});
    assert.deepEqual(c.sent.map(e=>e.action.kind),["refresh","clear","close"]);
    assert.equal(c.run("writing"),true);
  }
});
test("clear confirmations expire on changed owner target revision saved value or busy work",()=>{
  for(const change of ['owner="graph:other"','conversation.directory="C:\\\\other"','conversation.connected=false','conversation.busy=true','inventory.current=false','inventory.snapshot.target=null','inventory.snapshot.target.version="v2"','inventory.snapshot.preferences[0].userValue="new"','inventory.snapshot.preferences=[]']) {
    const c=component();c.run("open()");c.list();c.run("reviewClear(inventory.snapshot.preferences[0])");c.run(change);c.run("confirm()");
    assert.equal(c.sent.filter(e=>e.action.kind==="clear").length,0,change);
  }
});
test("clearing absent or stale saved values is not an implicit reset",()=>{
  for(const change of ['inventory.snapshot.preferences[0].userValue=null','inventory.current=false','writing=true']) {
    const c=component();c.run("open()");c.list();c.run(change);c.run("reviewClear(inventory.snapshot.preferences[0]);confirm()");assert.equal(c.sent.length,1,change);
  }
  const c=component();c.run("open()");c.list();c.run('select(inventory.snapshot.preferences[0]);value="";review();confirm()');assert.equal(c.sent.length,1);
});
