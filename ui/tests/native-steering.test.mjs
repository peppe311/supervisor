// Execute the actual component script handlers, without mounting or visual tests.
// Runes are inert values here; this checks event/draft logic, not Svelte rendering.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import ts from 'typescript';
import { ConversationDrafts } from '../src/lib/conversation-drafts.ts';
import { acceptsComposerReceipt } from '../src/lib/composer-receipt.ts';
import * as conversations from '../src/lib/conversation-events.ts';
import { contextDraft, contextIds } from '../src/lib/graph-contexts.ts';
import { attachmentList } from '../src/lib/file-attachments.ts';
import { consumeNativeCommand } from '../src/lib/native-commands.ts';

function component(name, graph = false) {
  let script = fs.readFileSync(new URL(`../src/components/${name}.svelte`, import.meta.url),'utf8').match(/<script lang="ts">([\s\S]*?)<\/script>/)[1];
  const parsed = ts.createSourceFile('component.ts',script,ts.ScriptTarget.Latest,true);
  for (const statement of [...parsed.statements].reverse()) if(ts.isImportDeclaration(statement)) script = script.slice(0,statement.pos)+script.slice(statement.end);
  const target = new EventTarget(), window = new EventTarget();
  const storage = new Map();
  window.sessionStorage = {getItem:key=>storage.get(key)??null,setItem:(key,value)=>storage.set(key,value),removeItem:key=>storage.delete(key)};
  target.dataset = {agentActive:'true',supportsSteer:'true',nativeTurnId:'turn-1',canResume:'false'};
  target.querySelector = () => ({childElementCount:0});
  const input = {value:'Follow up',style:{},scrollHeight:40};
  const mounts = [], sent = [], motion = [];
  for(const type of ['central-agent:submit','central-agent:graph-submit','central-agent:native-steer','central-agent:supervisor-steer','central-agent:stop']) target.addEventListener(type,event=>sent.push({type,...event.detail}));
  const ctx = vm.createContext({Event,CustomEvent,window,document:new EventTarget(),queueMicrotask,Map,Set,Boolean,String,Number,
    $state:value=>value,$derived:value=>value,$props:()=>({eventTarget:target,owner:'graph:steering-test'}),
    onMount:callback=>mounts.push(callback),setContext(){},flushSync:callback=>callback(),tick:()=>Promise.resolve(),drafts:new ConversationDrafts(()=>window.sessionStorage,'fixture'),acceptsComposerReceipt,
    ...conversations,contextDraft,contextIds,attachmentList,consumeNativeCommand,
    armPromptSendMotion:(scope,text)=>motion.push({scope,text}),animateSendControl:()=>{},fixtureInput:input,fixtureTarget:target});
  const js = ts.transpileModule(script,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText;
  vm.runInContext(js,ctx);
  vm.runInContext(graph?'input = fixtureInput;':'inputElement = fixtureInput; composerElement = fixtureTarget;',ctx);
  mounts.forEach(callback=>callback());
  if(graph) { input.value='Follow up'; target.dispatchEvent(new CustomEvent('central-agent:graph-composer-state',{detail:{owner:'graph:steering-test',canSubmit:true,active:true,supportsSteer:true,nativeTurnId:'turn-1'}})); }
  else { window.dispatchEvent(new CustomEvent('central-agent:conversation-key',{detail:'chat:steering-test'})); input.value='Follow up'; }
  return {ctx,target,window,input,sent,motion,run:code=>vm.runInContext(code,ctx)};
}

test('main and graph delivery paths offer slash controls before starting, queueing or steering',()=>{
  for(const graph of [false,true]){
    const c=component(graph?'AgentGraphCard':'Composer',graph);
    c.input.value='/review';let intercepted=0;
    c.input.dispatchEvent=event=>{assert.equal(event.type,'central-agent:composer-command');intercepted++;return false;};
    c.run(graph?'deliver("start");deliver("queue");deliver("steer")':'submit();deliverActiveRequest("queue");deliverActiveRequest("steer");queueShortcut()');
    assert.equal(intercepted,graph?3:4);assert.equal(c.sent.length,0);assert.equal(c.input.value,'/review');
  }
});

test('main Send now retains the displayed turn and waits for acceptance', () => {
  const c=component('Composer');
  c.run('submit()'); assert.equal(c.sent.length,0);
  c.target.dataset.nativeTurnId='turn-2';
  c.run('deliverActiveRequest("steer")');
  assert.equal(c.sent.length,1); assert.equal(c.sent[0].type,'central-agent:native-steer');
  assert.equal(c.sent[0].expectedTurnId,'turn-1'); assert.equal(c.sent[0].owner,'chat:steering-test');
  assert.ok(Number.isFinite(c.sent[0].timing.clickedAtMs));
  assert.equal(c.input.value,'Follow up');
  c.input.value='New draft';
  c.window.dispatchEvent(new CustomEvent('central-agent:submission-accepted',{detail:{owner:'chat:steering-test',input:'Follow up'}}));
  assert.equal(c.input.value,'New draft');
});

test('main Queue stays explicit and a finished-turn choice never starts a replacement', () => {
  const c=component('Composer'); c.run('submit(); deliverActiveRequest("queue")');
  assert.equal(c.sent[0].delivery,'queue');
  assert.ok(Number.isFinite(c.sent[0].timing.clickedAtMs));
  c.sent.length=0; c.run('submit()');
  c.target.dataset.agentActive='false';
  c.run('syncAgentState(); deliverActiveRequest("steer")');
  assert.equal(c.sent.length,0); assert.equal(c.input.value,'Follow up');
});

test('main work control sends and stops once, while Resume requires an explicit click path', () => {
  const c=component('Composer');
  c.target.dataset.agentActive='false';
  c.run('syncAgentState(); syncPayloadState(); primaryWorkAction(); primaryWorkAction()');
  assert.equal(c.sent.length,1); assert.equal(c.sent[0].type,'central-agent:submit');
  assert.equal(c.sent[0].message,'Follow up'); assert.equal(c.sent[0].resume,false);

  c.target.dataset.agentActive='true';
  c.run('syncAgentState(); stopWork(); stopWork()');
  assert.equal(c.sent.length,2); assert.equal(c.sent[1].type,'central-agent:stop');

  c.target.dataset.agentActive='false'; c.target.dataset.canResume='true'; c.input.value='';
  c.run('syncAgentState(); syncPayloadState(); submit()');
  assert.equal(c.sent.length,2,'Empty Enter-style submission must not resume stopped work');
  c.run('dispatchSubmission("start", true); dispatchSubmission("start", true)');
  assert.equal(c.sent.length,3); assert.equal(c.sent[2].type,'central-agent:submit');
  assert.equal(c.sent[2].message,''); assert.equal(c.sent[2].resume,true); assert.equal(c.sent[2].delivery,'start');
});

test('graph steering keeps owner, turn and attachments; failed input is not queued', () => {
  const c=component('AgentGraphCard',true);
  c.run('deliver("start")'); assert.equal(c.sent.length,0);
  c.target.dispatchEvent(new CustomEvent('central-agent:graph-composer-state',{detail:{owner:'graph:steering-test',canSubmit:true,active:true,supportsSteer:true,nativeTurnId:'turn-2'}}));
  c.run('files=[{id:"unsupported"}]; deliver("steer")');
  assert.equal(c.sent.length,1); assert.equal(c.sent[0].type,'central-agent:native-steer');
  assert.equal(c.sent[0].owner,'graph:steering-test'); assert.equal(c.sent[0].expectedTurnId,'turn-1');
  assert.equal(c.sent[0].fileIds[0],'unsupported'); assert.equal(c.input.value,'Follow up');
  c.window.dispatchEvent(new CustomEvent('central-agent:conversation-error',{detail:{owner:'graph:steering-test',error:'Rejected'}}));
  assert.equal(c.run('pending'),false); assert.equal(c.input.value,'Follow up');
  assert.equal(c.sent.length,1);
  c.input.value='New graph draft';
  c.window.dispatchEvent(new CustomEvent('central-agent:submission-accepted',{detail:{owner:'graph:steering-test',input:'Follow up'}}));
  assert.equal(c.input.value,'New graph draft');
});

test('a rejected graph dispatch neither arms motion nor leaves the composer pending', () => {
  const c=component('AgentGraphCard',true);
  c.target.dispatchEvent(new CustomEvent('central-agent:graph-composer-state',{detail:{owner:'graph:steering-test',canSubmit:true,active:false,supportsSteer:false,nativeTurnId:''}}));
  c.target.addEventListener('central-agent:graph-submit',event=>event.preventDefault());
  c.run('deliver("start")');
  assert.equal(c.motion.length,0);
  assert.equal(c.run('pending'),false);
  assert.equal(c.input.value,'Follow up');
});

test('a linked Supervisor steers only the exact active observed turn', () => {
  const c=component('AgentGraphCard',true);
  c.target.dispatchEvent(new CustomEvent('central-agent:supervision-state',{detail:{
    owner:'graph:steering-test',targetOwner:'chat:worker',title:'Worker',run:{},
    active:true,supportsSteer:true,nativeTurnId:'turn-worker',pendingRequests:0,monitoring:true,hooks:{}
  }}));
  assert.equal(c.run('supervision.monitoring'),true);
  c.input.value='Run the failing test first';
  c.run('submit({preventDefault(){},submitter:null})');
  assert.equal(c.sent.length,1);
  assert.equal(c.sent[0].type,'central-agent:supervisor-steer');
  assert.equal(c.sent[0].owner,'graph:steering-test');
  assert.equal(c.sent[0].targetOwner,'chat:worker');
  assert.equal(c.sent[0].expectedTurnId,'turn-worker');
  assert.equal(c.sent[0].message,'Run the failing test first');

  c.window.dispatchEvent(new CustomEvent('central-agent:conversation-error',{detail:{owner:'graph:steering-test',error:'The turn completed'}}));
  assert.equal(c.run('pending'),false);
  assert.equal(c.input.value,'Run the failing test first');
});
