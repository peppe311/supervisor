// Execute the shared component's actual event handlers, without a browser.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import ts from 'typescript';

test('summary choices remain owner-scoped, acknowledgement-driven and blocked while busy',()=>{
  for(const graph of [false,true]) {
    let script=fs.readFileSync(new URL('../src/components/NativeAccess.svelte',import.meta.url),'utf8').match(/<script lang="ts">([\s\S]*?)<\/script>/)[1];
    const parsed=ts.createSourceFile('component.ts',script,ts.ScriptTarget.Latest,true);
    for(const statement of [...parsed.statements].reverse()) if(ts.isImportDeclaration(statement))script=script.slice(0,statement.pos)+script.slice(statement.end);
    const window=new EventTarget(),target=new EventTarget(),mounts=[],sent=[];
    const owner=graph?'graph:a':'chat:a';
    target.addEventListener('central-agent:app-server-access-control',event=>sent.push(event.detail));
    const ctx=vm.createContext({Event,CustomEvent,window,document:target,$state:v=>v,$props:()=>graph?{owner,eventTarget:target}:{},onMount:cb=>mounts.push(cb)});
    vm.runInContext(ts.transpileModule(script,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText,ctx);
    vm.runInContext('confirmation={close(){},showModal(){}};',ctx);
    const cleanup=mounts.map(cb=>cb());
    const emit=(where,name,detail)=>where.dispatchEvent(new CustomEvent(name,{detail}));
    if(!graph)emit(window,'central-agent:conversation-key',owner);
    const view={visible:true,selected:'readOnly',summary:'auto',disabled:false,options:[]};
    emit(graph?target:window,'central-agent:conversation-state',{owner,nativeAccess:view});
    vm.runInContext('selectSummary("detailed")',ctx);
    assert.deepEqual(JSON.parse(JSON.stringify(sent.pop())),{owner,action:{kind:'set_summary',value:'detailed'}});
    assert.equal(vm.runInContext('view.summary',ctx),'auto'); // No optimistic setting.
    emit(window,'central-agent:app-server-access',{owner,nativeAccess:{...view,summary:'detailed'}});
    assert.equal(vm.runInContext('view.summary',ctx),'detailed');
    emit(window,'central-agent:app-server-access',{owner:'foreign',nativeAccess:{...view,summary:'none'}});
    assert.equal(vm.runInContext('view.summary',ctx),'detailed');
    for(const [choice,value] of [['inherit',null],['none','none'],['concise','concise'],['auto','auto']]) {
      vm.runInContext(`selectSummary(${JSON.stringify(choice)})`,ctx);
      assert.equal(sent.pop().action.value,value);
    }
    vm.runInContext('selectSummary("private")',ctx);assert.equal(sent.length,0);
    emit(graph?target:window,'central-agent:conversation-state',{owner,nativeAccess:{...view,disabled:true}});
    vm.runInContext('selectSummary("auto")',ctx);assert.equal(sent.length,0);
    cleanup.forEach(fn=>fn());
  }
});

test('shared permission consent waits for acknowledgement and rejects busy, restricted or stale confirmations',()=>{
  let script=fs.readFileSync(new URL('../src/components/NativeAccess.svelte',import.meta.url),'utf8').match(/<script lang="ts">([\s\S]*?)<\/script>/)[1];
  const parsed=ts.createSourceFile('component.ts',script,ts.ScriptTarget.Latest,true);
  for(const statement of [...parsed.statements].reverse()) if(ts.isImportDeclaration(statement))script=script.slice(0,statement.pos)+script.slice(statement.end);
  const window=new EventTarget(),document=new EventTarget(),mounts=[],sent=[];
  let modal=false;
  document.addEventListener('central-agent:app-server-access-control',event=>sent.push(event.detail));
  const ctx=vm.createContext({Event,CustomEvent,window,document,$state:v=>v,$props:()=>({}),onMount:cb=>mounts.push(cb),dialog:{get open(){return modal;},close(){modal=false;},showModal(){modal=true;}}});
  vm.runInContext(ts.transpileModule(script,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText,ctx);
  vm.runInContext('confirmation=dialog;',ctx);
  const cleanup=mounts.map(cb=>cb());
  const emit=(name,detail)=>window.dispatchEvent(new CustomEvent(name,{detail}));
  const view={visible:true,selected:'workspaceWrite',summary:'auto',disabled:false,permissionsDisabled:false,options:['readOnly','workspaceWrite','fullAccess'].map(value=>({value,label:value,allowed:true}))};
  const update=(patch={})=>emit('central-agent:conversation-state',{owner:'chat:main',nativeAccess:{...view,...patch}});
  emit('central-agent:conversation-key','chat:main');update();
  vm.runInContext('choose("fullAccess")',ctx);
  assert.equal(modal,true);assert.equal(sent.length,0);
  assert.equal(vm.runInContext('view.selected',ctx),'workspaceWrite');
  vm.runInContext('confirm()',ctx);
  assert.equal(sent.pop().action.kind,'confirm_full_access');
  assert.equal(vm.runInContext('view.selected',ctx),'workspaceWrite');
  emit('central-agent:app-server-access',{owner:'chat:main',nativeAccess:{...view,selected:'fullAccess'}});
  assert.equal(vm.runInContext('view.selected',ctx),'fullAccess');
  vm.runInContext('choose("readOnly")',ctx);assert.equal(sent.pop().action.kind,'read_only');
  vm.runInContext('choose("workspaceWrite")',ctx);assert.equal(sent.pop().action.kind,'workspace_write');
  vm.runInContext('choose("fullAccess");confirmation.close();confirm()',ctx);assert.equal(sent.length,0);
  update();vm.runInContext('choose("fullAccess")',ctx);assert.equal(modal,true);
  update({permissionsDisabled:true,permissionsBusy:true});assert.equal(modal,false);
  vm.runInContext('confirm();choose("readOnly");choose("workspaceWrite");choose("fullAccess")',ctx);
  assert.equal(sent.length,0);
  // Another agent's work blocks shared permissions, not this idle chat's summaries.
  vm.runInContext('selectSummary("concise")',ctx);assert.equal(sent.pop().action.kind,'set_summary');
  update({options:view.options.map(option=>({...option,allowed:option.value!=='fullAccess'}))});
  vm.runInContext('choose("fullAccess");confirm()',ctx);assert.equal(sent.length,0);
  update();vm.runInContext('choose("fullAccess")',ctx);
  emit('central-agent:conversation-key','chat:other');assert.equal(modal,false);
  vm.runInContext('confirm()',ctx);assert.equal(sent.length,0);
  cleanup.forEach(fn=>fn());
});
