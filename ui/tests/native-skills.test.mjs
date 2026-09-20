import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import ts from "typescript";
import {requestId as newRequestId} from "../src/lib/request-id.ts";

function component(graph=false) {
  let source=fs.readFileSync(new URL("../src/components/NativeSkills.svelte",import.meta.url),"utf8").match(/<script lang="ts">([\s\S]*?)<\/script>/)[1];
  const ast=ts.createSourceFile("component.ts",source,ts.ScriptTarget.Latest,true);
  for(const statement of [...ast.statements].reverse())if(ts.isImportDeclaration(statement))source=source.slice(0,statement.pos)+source.slice(statement.end);
  const target=new EventTarget(),window=new EventTarget(),sent=[],mounts=[],effects=[],owner=graph?"graph:a":"chat:a";
  const dialog={open:false,showModal(){this.open=true;},close(){this.open=false;}};
  target.addEventListener("central-agent:app-server-skills-control",event=>sent.push(event.detail));
  const ctx=vm.createContext({CustomEvent,Event,window,document:target,newRequestId,
    onMount:callback=>mounts.push(callback),$effect:callback=>effects.push(callback),$state:value=>value,
    $props:()=>({owner,eventTarget:graph?target:undefined,conversation:{visible:true,connected:true,busy:false,directory:"C:\\fixture",binding:null}}),fixtureDialog:dialog});
  vm.runInContext(ts.transpileModule(source,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText,ctx);
  vm.runInContext("dialog=fixtureDialog",ctx);mounts.forEach(callback=>callback());
  const run=code=>vm.runInContext(code,ctx);
  function list(patch={},destination=owner,requestId=run("requestId")) {
    window.dispatchEvent(new CustomEvent("central-agent:app-server-skills",{detail:{owner:destination,writing:false,view:{requestId,viewId:"inventory",directory:"C:\\fixture",loading:false,current:true,error:null,errors:[],items:[{name:"fixture-skill",description:"Description",scope:"repo",enabled:true,path:"C:\\fixture\\skill\\SKILL.md",pluginId:null}],...patch}}}));
  }
  return {sent,run,list,dialog,window,effects,owner};
}

test("main and graph skill configuration requires confirmation and sends only the selected native path",()=>{
  for(const graph of [false,true]) {
    const c=component(graph);c.run("open()");assert.equal(c.sent[0].action.kind,"refresh");assert.equal(c.sent[0].action.expected_directory,"C:\\fixture");
    c.list();c.run("select(inventory.items[0])");assert.equal(c.sent.length,1);
    c.run("confirm();confirm()");assert.equal(c.sent.length,2);
    assert.deepEqual(JSON.parse(JSON.stringify(c.sent[1])),{owner:c.owner,action:{kind:"set_enabled",view_id:"inventory",path:"C:\\fixture\\skill\\SKILL.md",enabled:false}});
    assert.equal(c.run("writing"),true);
  }
});

test("Computer Use discovery does not imply a live desktop connection and recovery never sends input",()=>{
  for(const graph of [false,true]) {
    const c=component(graph);c.run("open()");c.list();
    const update=detail=>c.window.dispatchEvent(new CustomEvent("central-agent:app-server-skills",{detail:{owner:c.owner,...detail}}));
    update({computerUse:"available",browserUse:"available",computerUseConnection:"unchecked",computerUseCanReconnect:true});
    assert.match(c.run("computerUseMessage()"),/not been verified/);
    assert.match(c.run("browserUseMessage()"),/composer plugin selector/);
    update({computerUseConnection:"connection_error",computerUseCanReconnect:false});
    c.run("reconnectComputerUse()");assert.equal(c.sent.some(e=>e.action.kind==="reconnect"),false);
    update({computerUseConnection:"connection_error",computerUseCanReconnect:true});
    assert.match(c.run("computerUseMessage()"),/not been repeated/);
    c.run("reconnectComputerUse();reconnectComputerUse()");
    assert.equal(c.sent.filter(e=>e.action.kind==="reconnect").length,1);
    assert.equal(c.sent.some(e=>["submit","steer","set_enabled"].includes(e.action.kind)),false);
  }
});

test("skills shortcut opens only its native owner without enabling or selecting anything",()=>{
  for(const graph of [false,true]){
    const c=component(graph);
    const foreign=new CustomEvent('central-agent:native-command',{cancelable:true,detail:{owner:'other',command:'skills'}});c.window.dispatchEvent(foreign);assert.equal(foreign.defaultPrevented,false);assert.equal(c.sent.length,0);
    const event=new CustomEvent('central-agent:native-command',{cancelable:true,detail:{owner:c.owner,command:'skills'}});c.window.dispatchEvent(event);assert.equal(event.defaultPrevented,true);assert.deepEqual(c.sent.map(e=>e.action.kind),['refresh']);
  }
});

test("foreign and stale skill reads cannot replace the current inventory",()=>{
  const c=component();c.run("open()");c.list({},"graph:foreign");assert.equal(c.run("inventory"),null);
  c.list({},c.owner,"old-request");assert.equal(c.run("inventory"),null);
  c.list();c.run("select(inventory.items[0])");c.list({current:false});c.run("confirm()");assert.equal(c.sent.length,1);assert.equal(c.run("choice"),null);
});

test("skill confirmations cannot follow a changed directory, owner, state or active turn",()=>{
  for(const change of ['conversation.directory="C:\\\\other"','owner="graph:other"','conversation.busy=true','conversation.connected=false','inventory.items[0].enabled=false']) {
    const c=component();c.run("open()");c.list();c.run("select(inventory.items[0])");c.run(change);c.run("confirm()");assert.equal(c.sent.filter(e=>e.action.kind==="set_enabled").length,0,change);
  }
});

test("closing or disconnecting a skill dialog never cancels or replays its shared write",()=>{
  const c=component();c.run("open()");c.list();c.run("select(inventory.items[0]);confirm();close()");
  assert.deepEqual(c.sent.map(e=>e.action.kind),["refresh","set_enabled","close"]);
  c.run('conversation.connected=false');c.effects.forEach(effect=>effect());assert.equal(c.dialog.open,false);
  assert.equal(c.sent.filter(e=>e.action.kind==="set_enabled").length,1);
  assert.equal(c.sent.some(e=>e.action.kind==="cancel"),false);
});

test("switching a skill dialog owner closes only its original discovery request",()=>{
  const c=component();c.run("open()");const request=c.sent[0].action.request_id;
  c.run('owner="graph:other"');c.effects.forEach(effect=>effect());
  assert.equal(c.dialog.open,false);
  assert.deepEqual(JSON.parse(JSON.stringify(c.sent.find(e=>e.action.kind==="close"))),{owner:"chat:a",action:{kind:"close",request_id:request}});
});

test("native skill selection sends no prompt or configuration write, including during an active turn",()=>{
  for(const graph of [false,true]) {
    const c=component(graph);c.run("open()");c.list();c.run("conversation.busy=true;attach(inventory.items[0])");
    assert.deepEqual(JSON.parse(JSON.stringify(c.sent.at(-1))),{owner:c.owner,action:{kind:"attach",view_id:"inventory",path:"C:\\fixture\\skill\\SKILL.md"}});
    assert.equal(c.sent.some(e=>["set_enabled","submit","steer"].includes(e.action.kind)),false);
  }
});
test("selected skill chips update without an open dialog and stay owner scoped",()=>{
  const c=component();
  c.window.dispatchEvent(new CustomEvent("central-agent:app-server-skills",{detail:{owner:"graph:other",selected:[{id:"foreign"}]}}));
  assert.equal(c.run("selected.length"),0);
  c.window.dispatchEvent(new CustomEvent("central-agent:app-server-skills",{detail:{owner:c.owner,selected:[{id:"selected-a",name:"fixture",current:true}]}}));
  assert.equal(c.run("selected[0].id"),"selected-a");
  c.run('remove("selected-a")');assert.deepEqual(JSON.parse(JSON.stringify(c.sent.at(-1))),{owner:c.owner,action:{kind:"remove",id:"selected-a"}});
});
test("stale disabled foreign-directory or writing skill inventories cannot attach",()=>{
  for(const change of ['inventory.current=false','inventory.items[0].enabled=false','conversation.directory="C:\\\\other"','writing=true','conversation.connected=false']) {
    const c=component();c.run("open()");c.list();c.run(change);c.run("attach(inventory.items[0])");assert.equal(c.sent.some(e=>e.action.kind==="attach"),false,change);
  }
});

test("extra skill roots require an explicit process-scoped confirmation and enforce the UI count bound",()=>{
  const c=component();c.run("open()");c.list();c.run("prepareRoots()");
  assert.equal(c.run("confirmRoots"),true);assert.equal(c.sent.length,1);
  c.run('rootDraft="C:\\\\one\\nC:\\\\one\\nC:\\\\two";saveRoots()');
  assert.deepEqual(JSON.parse(JSON.stringify(c.sent.at(-1))),{owner:c.owner,action:{kind:'set_extra_roots',roots:['C:\\one','C:\\two']}});

  const tooMany=component();tooMany.run("open()");tooMany.list();tooMany.run("prepareRoots()");
  tooMany.run('rootDraft=Array.from({length:17},(_,index)=>`C:\\\\root-${index}`).join("\\n");saveRoots()');
  assert.equal(tooMany.sent.filter(event=>event.action.kind==='set_extra_roots').length,0);
  assert.match(tooMany.run('error'),/at most 16/);
});
