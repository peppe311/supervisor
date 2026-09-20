// Executes actual Svelte script handlers against event fixtures. No visual tests.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import ts from 'typescript';
import {nativeCommands,commandQuery,commandReason,consumeNativeCommand} from '../src/lib/native-commands.ts';

function component(graph=false) {
  let source=fs.readFileSync(new URL('../src/components/NativeCommands.svelte',import.meta.url),'utf8').match(/<script lang="ts">([\s\S]*?)<\/script>/)[1];
  const ast=ts.createSourceFile('component.ts',source,ts.ScriptTarget.Latest,true);
  for(const statement of [...ast.statements].reverse())if(ts.isImportDeclaration(statement))source=source.slice(0,statement.pos)+source.slice(statement.end);
  class Target extends EventTarget {
    addEventListener(type,listener,options){super.addEventListener(type,listener,options===true?{capture:true}:options);}
    removeEventListener(type,listener,options){super.removeEventListener(type,listener,options===true?{capture:true}:options);}
  }
  const input=new Target(),window=new Target(),target=new Target(),mounts=[],commands=[];
  input.value='';const owner=graph?'graph:a':'chat:a';
  input.addEventListener('central-agent:slash-ui',event=>{if(event.detail.owner===owner && event.detail.action==='native_controls')event.preventDefault();});
  const document={getElementById:id=>id==='fixture-input'?input:{scrollIntoView(){}}};
  const ctx=vm.createContext({Event,CustomEvent,window,document,nativeCommands,commandQuery,commandReason,
    $props:()=>({inputId:'fixture-input',owner:graph?owner:undefined,eventTarget:graph?target:undefined}),
    $state:value=>value,$derived:value=>value,onMount:callback=>mounts.push(callback)});
  vm.runInContext(ts.transpileModule(source,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText,ctx);
  const cleanups=mounts.map(callback=>callback());
  if(!graph)window.dispatchEvent(new CustomEvent('central-agent:conversation-key',{detail:owner}));
  let acknowledge=true;
  window.addEventListener('central-agent:native-command',event=>{commands.push(event.detail);if(acknowledge)event.preventDefault();});
  function update(patch={},destination=owner){(graph?target:window).dispatchEvent(new CustomEvent('central-agent:conversation-state',{detail:{owner:destination,nativeConversation:{visible:true,connected:true,busy:false,observed:true,directory:'C:\\fixture',binding:{threadId:'native-a',archived:false,deleted:false},...patch}}}));}
  function type(text){input.value=text;input.dispatchEvent(new Event('focus'));input.dispatchEvent(new Event('input'));}
  function key(key,properties={}){const event=new Event('keydown',{cancelable:true});Object.assign(event,{key,...properties});input.dispatchEvent(event);return event;}
  update();
  return {input,window,target,owner,commands,type,key,update,cleanup:()=>cleanups.forEach(f=>f()),ack:value=>acknowledge=value,run:code=>vm.runInContext(code,ctx)};
}

test('native command syntax preserves literal paths, multiline prompts and escaped slash text',()=>{
  for(const value of ['hello',' /review','/home/project','/review\n','/review\nexplain','C:\\project'])assert.equal(commandQuery(value),null,value);
  assert.equal(commandQuery('/re'),'re');assert.equal(commandQuery('/REVIEW  '),'review');assert.equal(commandQuery('/'),'');
});

test('main and graph slash commands dispatch only to their owner and consume only acknowledged shortcuts',()=>{
  for(const graph of [false,true]){
    const c=component(graph);c.type('/review');assert.equal(consumeNativeCommand(c.input),true);
    assert.deepEqual(JSON.parse(JSON.stringify(c.commands)),[{owner:c.owner,command:'review'}]);assert.equal(c.input.value,'');
    c.ack(false);c.type('/fork');assert.equal(consumeNativeCommand(c.input),true);assert.equal(c.input.value,'/fork');assert.match(c.run('notice'),/unavailable/);
  }
});

test('busy, disconnected, unobserved and changed-provider states never execute or queue a shortcut',()=>{
  const c=component();
  for(const patch of [{busy:true},{connected:false},{observed:false},{binding:null}]){
    c.update(patch);c.type('/fork');assert.equal(consumeNativeCommand(c.input),true);assert.equal(c.commands.length,0);assert.equal(c.input.value,'/fork');
  }
  c.update();c.update({busy:true},'graph:foreign');c.type('/fork');consumeNativeCommand(c.input);assert.equal(c.commands.length,1);
  c.update({visible:false});c.type('/review');assert.equal(consumeNativeCommand(c.input),false);assert.equal(c.input.value,'/review');
});

test('unsupported commands and arguments stay in the draft; literal input still reaches normal submission',()=>{
  const c=component();
  for(const value of ['/clear','/rename New name','/unknown argument']){
    c.type(value);assert.equal(consumeNativeCommand(c.input),true);assert.equal(c.input.value,value);assert.equal(c.commands.length,0);assert.ok(c.run('notice'));
  }
  for(const value of [' /clear','/home/file','/review\nThen explain']){c.type(value);assert.equal(consumeNativeCommand(c.input),false);}
});

test('keyboard suggestions filter, wrap, complete and open without turning IME or multiline input into a command',()=>{
  const c=component(true);c.type('/re');assert.deepEqual(Array.from(c.run('suggestions().map(c=>c.name)')),['resume','rename','review']);
  c.key('ArrowUp');assert.equal(c.run('selected'),2);
  c.key('Tab');assert.equal(c.input.value,'/review');assert.equal(c.commands.length,0);
  assert.equal(c.key('Enter',{isComposing:true}).defaultPrevented,false);
  assert.equal(c.key('Enter',{shiftKey:true}).defaultPrevented,false);
  c.key('Escape');assert.equal(c.run('menuVisible()'),false);assert.equal(c.input.value,'/review');
  c.type('/review');assert.equal(c.key('Enter').defaultPrevented,true);assert.equal(c.commands[0].command,'review');
});

test('changing main owner and unmounting remove shortcut authority and preserve the draft',()=>{
  const c=component();c.type('/review');c.window.dispatchEvent(new CustomEvent('central-agent:conversation-key',{detail:'chat:b'}));
  assert.equal(c.run('menuVisible()'),false);assert.equal(consumeNativeCommand(c.input),false);assert.equal(c.commands.length,0);
  c.update({},'chat:b');c.cleanup();c.type('/review');assert.equal(consumeNativeCommand(c.input),false);
});

test('available controls disclose native state requirements without inventing a bound thread',()=>{
  const view={visible:true,connected:true,busy:true,directory:'C:\\fixture',binding:null};
  for(const name of ['resume','cloud','skills','config'])assert.equal(commandReason(name,view),null);
  assert.match(commandReason('review',view),/no available/);
  assert.match(commandReason('unarchive',{...view,busy:false,binding:{archived:false}}),/not archived/);
});

test('main command bridge prepares Settings dialogs only for the current owner without opening the profile menu',()=>{
  const html=fs.readFileSync(new URL('../../assets/agent-panel.html',import.meta.url),'utf8');
  const start=html.indexOf('composer.addEventListener("central-agent:slash-ui"');
  const handler=html.slice(start,html.indexOf('composer.addEventListener("central-agent:native-steer"',start));
  assert.ok(start>=0);
  const composer=new EventTarget(),tetraConfig={menu:{hidden:true}};let opened=0;
  vm.runInNewContext(handler,{composer,tetraConfig,currentConversationOwner:'chat:a',window:{CentralAgentSvelte:{prepareAgentSettingsDialogs:()=>{opened++;return true;}}},closeAllCompoundPickers:()=>{tetraConfig.menu.hidden=true;}});
  const foreign=new CustomEvent('central-agent:slash-ui',{cancelable:true,detail:{owner:'chat:b',action:'native_controls'}});composer.dispatchEvent(foreign);assert.equal(foreign.defaultPrevented,false);assert.equal(opened,0);
  const local=new CustomEvent('central-agent:slash-ui',{cancelable:true,detail:{owner:'chat:a',action:'native_controls'}});composer.dispatchEvent(local);assert.equal(local.defaultPrevented,true);assert.equal(opened,1);assert.equal(tetraConfig.menu.hidden,true);
});
