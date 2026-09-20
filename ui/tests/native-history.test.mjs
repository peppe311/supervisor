import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import ts from "typescript";
import {requestId as newRequestId} from "../src/lib/request-id.ts";
import {canImportHistory,historyProjectLabel,historyTitle} from "../src/lib/native-history.ts";

const ready=()=>({visible:true,connected:true,busy:false,observed:false,name:null,directory:"C:\\project",importAvailable:true,binding:null});
test("native history import requires an unbound persistent local destination",()=>{
  const v=ready();assert.equal(canImportHistory(v),true);
  for(const patch of [{visible:false},{connected:false},{busy:true},{directory:null},{importAvailable:false},{binding:{threadId:"existing"}}])assert.equal(canImportHistory({...v,...patch}),false);
  assert.equal(historyTitle({name:"Native name",preview:"First message"}),"Codex conversation");
  assert.equal(historyProjectLabel("C:\\Users\\fixture\\Documents\\Central Agent"),"Central Agent");
});
function component(){
  let source=fs.readFileSync(new URL("../src/components/NativeHistory.svelte",import.meta.url),"utf8").match(/<script lang="ts">([\s\S]*?)<\/script>/)[1];
  const ast=ts.createSourceFile("component.ts",source,ts.ScriptTarget.Latest,true);
  for(const statement of [...ast.statements].reverse())if(ts.isImportDeclaration(statement))source=source.slice(0,statement.pos)+source.slice(statement.end);
  const target=new EventTarget(),window=new EventTarget(),sent=[],mounts=[],effects=[];
  const dialog={open:false,showModal(){this.open=true;},close(){this.open=false;}};
  target.addEventListener("central-agent:app-server-history-control",event=>sent.push(event.detail));
  const ctx=vm.createContext({canImportHistory,historyTitle,CustomEvent,Event,window,document:target,newRequestId,
    onMount:callback=>mounts.push(callback),tick:()=>Promise.resolve(),$effect:callback=>effects.push(callback),$state:value=>value,
    $props:()=>({owner:"chat:a",eventTarget:target,conversation:ready()}),fixtureDialog:dialog});
  vm.runInContext(ts.transpileModule(source,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText,ctx);
  vm.runInContext("dialog=fixtureDialog",ctx);mounts.forEach(callback=>callback());
  const run=code=>vm.runInContext(code,ctx);
  function list(owner="chat:a",requestId=run("requestId")){
    window.dispatchEvent(new CustomEvent("central-agent:app-server-history",{detail:{owner,view:{requestId,viewId:"snapshot",busy:false,error:null,hasMore:false,archived:false,items:[{id:"native-a",projectLabel:"source",modelProvider:"openai",updatedAt:1,ephemeral:false,status:{type:"notLoaded"}}],cloud:{items:[{id:"task_e_cloudfixture",title:"Cloud task",status:"ready",updatedAt:"2026-09-17T10:00:00Z",environmentLabel:"Central Agent",summary:{filesChanged:2,linesAdded:4,linesRemoved:1},isReview:false,attemptTotal:2}],busy:false,hasMore:false,error:null,diff:null}}}}));
  }
  return {sent,run,list,dialog,window,effects};
}
test("resume shortcut browses history without importing or starting a turn",()=>{
  const c=component();
  const foreign=new CustomEvent('central-agent:native-command',{cancelable:true,detail:{owner:'other',command:'resume'}});c.window.dispatchEvent(foreign);assert.equal(foreign.defaultPrevented,false);assert.equal(c.sent.length,0);
  const event=new CustomEvent('central-agent:native-command',{cancelable:true,detail:{owner:'chat:a',command:'resume'}});c.window.dispatchEvent(event);assert.equal(event.defaultPrevented,true);assert.equal(c.dialog.open,true);assert.deepEqual(c.sent.map(e=>e.action.kind),['open']);
});
test("cloud shortcut opens the subscription source and still sends no prompt",()=>{
  const c=component();const event=new CustomEvent('central-agent:native-command',{cancelable:true,detail:{owner:'chat:a',command:'cloud'}});c.window.dispatchEvent(event);
  assert.equal(event.defaultPrevented,true);assert.equal(c.dialog.open,true);assert.equal(c.run('source'),'cloud');assert.deepEqual(c.sent.map(e=>e.action.kind),['open']);
});

test("browsing and selecting do not import until an explicit confirmation",()=>{
  const c=component();c.run("open()");assert.equal(c.sent[0].action.kind,"open");c.list();
  c.run("select(history.items[0],true)");assert.equal(c.sent.length,1);
  c.run("confirm();confirm()");assert.equal(c.sent.length,2);
  assert.equal(c.sent[1].owner,"chat:a");assert.equal(c.sent[1].action.fork,true);assert.equal(c.sent[1].action.thread_id,"native-a");
  assert.equal(c.sent[1].action.view_id,"snapshot");
  assert.equal(c.run("dialog.scrollTop"),0);
});

test("history UI never renders native prompts, previews or absolute source paths",()=>{
  const source=fs.readFileSync(new URL("../src/components/NativeHistory.svelte",import.meta.url),"utf8");
  assert.doesNotMatch(source,/item\.(?:name|preview|cwd)/);
  assert.match(source,/Project: \{item\.projectLabel\}/);
  assert.match(source,/{#if choice}[\s\S]*{:else}[\s\S]*class="results"/);
});
test("cloud history uses observed subscription tasks without projecting a local thread",()=>{
  const c=component();c.run("open()");c.list();
  c.run("cloudRefresh();openCloud(history.cloud.items[0]);cloudDiff(history.cloud.items[0]);newCloud() ");
  assert.deepEqual(c.sent.map(entry=>entry.action.kind),["open","cloud_refresh","cloud_open","cloud_diff","cloud_new"]);
  assert.equal(c.sent[2].action.task_id,"task_e_cloudfixture");
  assert.equal(c.sent[3].action.attempt,2);
  const source=fs.readFileSync(new URL("../src/components/NativeHistory.svelte",import.meta.url),"utf8");
  assert.match(source,/ChatGPT subscription/);assert.match(source,/class="cloud-icon"/);
  assert.doesNotMatch(source,/Link cloud|Fork cloud/);
});
test("archived history may be linked but cannot silently be restored for a fork",()=>{
  const c=component();c.run("open()");c.list();
  c.run("history={...history,archived:true};select(history.items[0],true)");
  assert.equal(c.run("choice"),null);assert.equal(c.sent.length,1);
  c.run("select(history.items[0],false);confirm()");
  assert.equal(c.sent.at(-1).action.fork,false);
  const changed=component();changed.run("open()");changed.list();
  changed.run("select(history.items[0],true);history={...history,archived:true};confirm()");
  assert.equal(changed.run("choice"),null);assert.equal(changed.sent.length,1);
});

test("foreign/stale results and changed directories cannot redirect imports",()=>{
  const c=component();c.run("open()");c.list("chat:other");assert.equal(c.run("history"),null);
  c.list("chat:a","stale");assert.equal(c.run("history"),null);c.list();c.run("select(history.items[0],false)");
  c.run('conversation={...conversation,directory:"C:\\\\different"};confirm()');
  assert.equal(c.sent.length,1);assert.equal(c.run("choice"),null);
  c.run("owner='graph:new'");c.effects.forEach(effect=>effect());assert.equal(c.dialog.open,false);
  assert.equal(c.sent.at(-1).owner,"chat:a");assert.equal(c.sent.at(-1).action.kind,"close");
});
test("native acknowledgement closes only its own import; rejection keeps history",()=>{
  const c=component();c.run("open()");c.list();c.run("select(history.items[0],false);confirm()");
  c.window.dispatchEvent(new CustomEvent("central-agent:app-server-conversation",{detail:{owner:"other",binding:{threadId:"x"},change:"updated"}}));
  assert.equal(c.dialog.open,true);assert.equal(c.run("submitting"),true);
  c.window.dispatchEvent(new CustomEvent("central-agent:app-server-conversation",{detail:{owner:"chat:a",error:"Native rejection"}}));
  assert.equal(c.run("submitting"),false);assert.equal(c.run("error"),"Native rejection");assert.equal(c.dialog.open,true);
  c.run("confirm()");
  c.window.dispatchEvent(new CustomEvent("central-agent:app-server-conversation",{detail:{owner:"chat:a",binding:{threadId:"native-a"},change:"updated"}}));
  assert.equal(c.dialog.open,false);
});

test("uncertain fork recovery only links the original source's fork and revalidates confirmation",()=>{
  const c=component();c.run("open()");c.list();
  c.run('conversation={...conversation,pendingFork:"source"};select(history.items[0],false)');
  assert.equal(c.run('choice'),null);
  c.run('history.items[0].forkedFromId="source";select(history.items[0],true)');
  assert.equal(c.run('choice'),null);
  c.run('select(history.items[0],false)');assert.equal(c.run('choice.item.id'),'native-a');
  c.run('conversation={...conversation,pendingFork:"changed-source"};confirm()');
  assert.equal(c.run('choice'),null);assert.equal(c.sent.length,1);
  c.run('conversation={...conversation,pendingFork:"source"};select(history.items[0],false);confirm()');
  assert.equal(c.sent.length,2);assert.equal(c.sent[1].action.fork,false);
});
