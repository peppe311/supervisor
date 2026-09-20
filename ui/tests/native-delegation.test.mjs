import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import ts from 'typescript';
import {chatForest,chatRelationship} from '../src/lib/chat-tree.ts';
import {eventMeaning,deliveryTime,deliveryMode} from '../src/lib/native-event-log.ts';

function component(file,graph=false) {
  let script=fs.readFileSync(new URL(`../src/components/${file}.svelte`,import.meta.url),'utf8').match(/<script lang="ts">([\s\S]*?)<\/script>/)[1];
  const ast=ts.createSourceFile('component.ts',script,ts.ScriptTarget.Latest,true);
  for(const statement of [...ast.statements].reverse())if(ts.isImportDeclaration(statement))script=script.slice(0,statement.pos)+script.slice(statement.end);
  const window=new EventTarget(),target=new EventTarget(),mounts=[],sent=[];
  const owner=graph?'graph:parent':'chat:parent';
  const props={owner,eventTarget:graph?target:undefined,conversation:{binding:{threadId:'parent'},connected:false,busy:true,delegates:[{owner:graph?'graph:child':'chat:child',threadId:'child',relation:'child',name:'Child'}]}};
  target.addEventListener('central-agent:app-server-conversation-control',event=>sent.push(event.detail));
  const ctx=vm.createContext({Event,CustomEvent,window,document:target,$state:value=>value,$props:()=>props,onMount:callback=>mounts.push(callback)});
  vm.runInContext(ts.transpileModule(script,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText,ctx);
  mounts.forEach(fn=>fn());
  return {owner,window,target,sent,props,run:code=>vm.runInContext(code,ctx)};
}
test('delegated children and independent forks retain distinct labels and nesting',()=>{
  const root={id:'parent',projectRoot:'p',lineage:{kind:'source',childCount:1,delegatedCount:1}};
  const child={id:'child',projectRoot:'p',lineage:{kind:'delegated',parentChatId:'parent',parentTitle:'Parent',childCount:0}};
  const fork={id:'fork',projectRoot:'p',lineage:{kind:'fork',parentChatId:'parent',parentTitle:'Parent',childCount:0}};
  assert.deepEqual(chatForest([child,fork,root]).map(row=>[row.chat.id,row.depth]),[['parent',0],['child',1],['fork',1]]);
  assert.equal(chatRelationship(root),'Supervisor · 1 fork · 1 delegated');
  assert.equal(chatRelationship(child),'Delegated · Parent');
  assert.equal(chatRelationship(fork),'Fork · Parent');
});
test('opening a related agent is owner-scoped, permitted while busy and sends no prompt',()=>{
  for(const graph of [false,true]) {
    const c=component('NativeDelegates',graph);
    c.run(`open('${graph?'graph':'chat'}:foreign')`);
    assert.equal(c.sent.length,0);
    c.run(`open('${graph?'graph':'chat'}:child')`);
    assert.deepEqual(JSON.parse(JSON.stringify(c.sent)),[{owner:c.owner,action:{kind:'open_delegate',expected_thread_id:'parent',destination:graph?'graph:child':'chat:child'}}]);
  }
});
test('diagnostic reads work offline and never accept another conversation or a stale binding',()=>{
  for(const graph of [false,true]) {
    const c=component('NativeEventLog',graph);
    c.run('refresh()');
    assert.equal(c.sent[0].action.kind,'event_log');
    const target=graph?c.target:c.window;
    const update=detail=>target.dispatchEvent(new CustomEvent('central-agent:native-event-log',{detail}));
    update({owner:'chat:other',threadId:'parent',log:{entries:['wrong']}});
    update({owner:c.owner,threadId:'old',log:{entries:['stale']}});
    assert.equal(c.run('log'),null);
    update({owner:c.owner,threadId:'parent',log:{entries:[{sequence:1}]}});
    assert.equal(c.run('log.entries[0].sequence'),1);
    assert.equal(c.sent.length,1);
  }
});
test('acknowledgements, reasoning summaries and turn completion have different meanings',()=>{
  assert.equal(eventMeaning({method:'turn/start',direction:'response',state:'accepted'}),'Prompt acknowledged');
  assert.equal(eventMeaning({method:'turn/start',direction:'response',state:'delivery_unknown'}),'Prompt delivery uncertain');
  assert.equal(eventMeaning({method:'turn/completed',direction:'notification'}),'Turn ended');
  assert.equal(eventMeaning({method:'item/reasoning/summaryTextDelta',direction:'notification'}),'Reasoning summary update');
});

test('delivery diagnostics distinguish missing observations, estimates and session modes',()=>{
  assert.equal(deliveryTime(null),'Not observed');
  assert.equal(deliveryTime(NaN),'Not observed');
  assert.equal(deliveryTime(-1),'Not observed');
  assert.equal(deliveryTime(0),'0.0 ms');
  assert.equal(deliveryTime(12.35),'12.3 ms');
  assert.equal(deliveryMode('preparing'),'Joined session preparation');
  assert.equal(deliveryMode('steer'),'Follow-up to active work');
});
