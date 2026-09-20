import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import ts from 'typescript';
import * as goals from '../src/lib/native-goals.ts';
import {commandReason} from '../src/lib/native-commands.ts';

function fixture() {
  return {visible:true,connected:true,busy:false,observed:true,nativeLoaded:true,directory:'C:\\fixture',binding:{threadId:'native-a',archived:false,deleted:false},goal:{threadId:'native-a',directory:'C:\\fixture',viewId:'view-a',goal:null,current:true,busy:false,writing:false,error:null}};
}
function component(graph=false) {
  let source=fs.readFileSync(new URL('../src/components/NativeGoal.svelte',import.meta.url),'utf8').match(/<script lang="ts">([\s\S]*?)<\/script>/)[1];
  const ast=ts.createSourceFile('component.ts',source,ts.ScriptTarget.Latest,true);
  for(const statement of [...ast.statements].reverse())if(ts.isImportDeclaration(statement))source=source.slice(0,statement.pos)+source.slice(statement.end);
  const target=new EventTarget(),window=new EventTarget(),sent=[],effects=[],mounts=[],owner=graph?'graph:a':'chat:a';
  target.addEventListener('central-agent:app-server-conversation-control',event=>sent.push(event.detail));
  const dialog={open:false,showModal(){this.open=true;},close(){this.open=false;}};
  const ctx=vm.createContext({...goals,CustomEvent,Event,window,document:target,onMount:fn=>mounts.push(fn),$effect:fn=>effects.push(fn),$state:v=>v,$props:()=>({owner,eventTarget:graph?target:undefined,conversation:fixture()}),fixtureDialog:dialog});
  vm.runInContext(ts.transpileModule(source,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText,ctx);
  const run=code=>vm.runInContext(code,ctx);run('dialog=fixtureDialog');mounts.forEach(fn=>fn());
  return {run,sent,effects,window,owner,dialog};
}
test('goal shortcut opens the exact loaded owner and reads only, including during work',()=>{
  for(const graph of [false,true]) {
    const c=component(graph);c.run('conversation.busy=true');
    const foreign=new CustomEvent('central-agent:native-command',{cancelable:true,detail:{owner:'other',command:'goal'}});c.window.dispatchEvent(foreign);assert.equal(foreign.defaultPrevented,false);
    const own=new CustomEvent('central-agent:native-command',{cancelable:true,detail:{owner:c.owner,command:'goal'}});c.window.dispatchEvent(own);assert.equal(own.defaultPrevented,true);
    assert.equal(c.dialog.open,true);assert.deepEqual(c.sent.map(e=>e.action.action.kind),['refresh']);
    assert.equal(c.sent[0].owner,c.owner);assert.equal(c.sent[0].action.expected_thread_id,'native-a');
  }
  const view=fixture();view.busy=true;assert.equal(commandReason('goal',view),null);view.nativeLoaded=false;assert.ok(commandReason('goal',view));
});
test('new objectives are paused and unlimited unless explicitly budgeted; confirm freezes the edit',()=>{
  for(const graph of [false,true]) {
    const c=component(graph);c.run('objective="Finish the project";reviewObjective()');assert.equal(c.sent.length,0);
    c.run('objective="Later draft";limited=true;budget="20";confirm();confirm()');
    assert.equal(c.sent.length,1);assert.deepEqual(JSON.parse(JSON.stringify(c.sent[0].action.action)),{kind:'change',view_id:'view-a',edit:{kind:'replace',objective:'Finish the project',status:'paused',token_budget:null}});
    c.run('close()');assert.equal(c.sent.length,1);
  }
});
test('scope changes, stale/native-busy states and owner changes reject a confirmation',()=>{
  for(const change of ['owner="graph:b"','conversation.directory="C:\\\\other"','conversation.binding.threadId="native-b"','conversation.goal.viewId="view-b"','conversation.goal.current=false','conversation.goal.busy=true','conversation.visible=false','conversation.connected=false','conversation.nativeLoaded=false','conversation.binding.archived=true']) {
    const c=component();c.run('objective="Do work";reviewObjective()');c.run(change);c.run('confirm()');assert.equal(c.sent.length,0,change);
  }
});
test('accounting updates retain edit authority and do not change the objective or chat draft',()=>{
  const c=component(true);c.run('conversation.goal.goal={objective:"Existing",status:"active",tokensUsed:0};review({kind:"status",status:"paused"});conversation.goal.goal.tokensUsed=20;confirm()');
  assert.deepEqual(JSON.parse(JSON.stringify(c.sent[0].action.action.edit)),{kind:'status',status:'paused'});
  assert.equal(c.sent.length,1);
});
test('budget validation has no silent zero, fraction, unsafe integer or implicit limit',()=>{
  assert.equal(goals.goalBudget(false,''),null);assert.equal(goals.goalBudget(true,'2000'),2000);
  for(const value of ['','0','-1','1.5','1e3','Infinity','9007199254740992'])assert.throws(()=>goals.goalBudget(true,value),value);
  assert.equal(goals.goalEditError({kind:'replace',objective:'😀'.repeat(4000),status:'paused',token_budget:null}),null);
  assert.ok(goals.goalEditError({kind:'replace',objective:'a'.repeat(4001),status:'paused',token_budget:null}));
});
test('native read/write lifecycle unlocks only after observed busy completion and disconnect closes without replay',()=>{
  const c=component();c.run('open();conversation.goal.busy=true');c.effects.forEach(fn=>fn());assert.equal(c.run('waiting'),true);
  c.run('conversation.goal.busy=false');c.effects.forEach(fn=>fn());assert.equal(c.run('waiting'),false);
  c.run('objective="Keep draft";reviewObjective();confirm();conversation.connected=false');c.effects.forEach(fn=>fn());
  assert.equal(c.dialog.open,false);assert.equal(c.sent.filter(e=>e.action.action.kind==='change').length,1);assert.equal(c.sent.length,2);
});

test('goal modal uses the shared visible dialog and does not render an opaque thread id',()=>{
  const source=fs.readFileSync(new URL('../src/components/NativeGoal.svelte',import.meta.url),'utf8');
  assert.match(source,/native-goal workspace-confirm-dialog/);
  assert.match(source,/currently loaded native Codex conversation/);
  assert.doesNotMatch(source,/Native conversation · \{conversation\?\.binding\?\.threadId\}/);
});
