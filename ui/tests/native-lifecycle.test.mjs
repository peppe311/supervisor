// Run actual component event handlers without mounting a browser or visual tests.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import ts from 'typescript';
import {graphWindowSelection} from '../src/lib/graph-conversations.ts';

function component(graph=false) {
  let script=fs.readFileSync(new URL('../src/components/NativeConversation.svelte',import.meta.url),'utf8').match(/<script lang="ts">([\s\S]*?)<\/script>/)[1];
  const parsed=ts.createSourceFile('component.ts',script,ts.ScriptTarget.Latest,true);
  for(const statement of [...parsed.statements].reverse()) if(ts.isImportDeclaration(statement))script=script.slice(0,statement.pos)+script.slice(statement.end);
  const window=new EventTarget(),target=new EventTarget(),mounts=[],sent=[];
  const dialog={open:false,showModal(){this.open=true;},close(){this.open=false;}};
  target.addEventListener('central-agent:app-server-conversation-control',event=>sent.push(event.detail));
  const ctx=vm.createContext({Event,CustomEvent,window,document:target,
    $state:value=>value,$props:()=>graph?{owner:'graph:a',eventTarget:target}:{},onMount:callback=>mounts.push(callback),fixtureDialog:dialog});
  vm.runInContext(ts.transpileModule(script,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.None}}).outputText,ctx);
  vm.runInContext('dialog=fixtureDialog;',ctx);
  mounts.forEach(callback=>callback());
  if(!graph)window.dispatchEvent(new CustomEvent('central-agent:conversation-key',{detail:'chat:a'}));
  const owner=graph?'graph:a':'chat:a';
  function update(patch={},eventOwner=owner) {
    (graph?target:window).dispatchEvent(new CustomEvent('central-agent:conversation-state',{detail:{owner:eventOwner,nativeConversation:{visible:true,connected:true,busy:false,observed:true,name:'Native name',directory:'C:\\fixture',binding:{threadId:'native-a',archived:false,deleted:false},...patch}}}));
  }
  update();
  return {window,target,dialog,sent,update,owner,run:code=>vm.runInContext(code,ctx)};
}

test('native operation failures survive background stream and metadata events',()=>{
  for(const graph of [false,true]) {
    const c=component(graph);
    const report=detail=>c.window.dispatchEvent(new CustomEvent('central-agent:app-server-conversation',{detail:{owner:c.owner,...detail}}));
    report({error:'The native reviewer rejected this request.'});
    report({change:'stream'});
    report({nativeConversation:{visible:true,connected:true,binding:{threadId:'native-a'}}});
    assert.equal(c.run('error'),'The native reviewer rejected this request.');
    c.window.dispatchEvent(new CustomEvent('central-agent:app-server-conversation',{detail:{owner:'other',change:'updated'}}));
    assert.equal(c.run('error'),'The native reviewer rejected this request.');
    report({change:'updated'});
    assert.equal(c.run('error'),'');
    assert.equal(c.sent.length,0);
  }
});

test('slash lifecycle shortcuts open existing confirmations without sending a mutation',()=>{
  for(const graph of [false,true])for(const [command,operation] of [['rename','rename'],['fork','new_fork'],['review','start_review'],['archive','archive'],['unarchive','unarchive']]){
    const c=component(graph);
    const event=new CustomEvent('central-agent:native-command',{cancelable:true,detail:{owner:c.owner,command}});
    c.window.dispatchEvent(event);assert.equal(event.defaultPrevented,true);assert.equal(c.dialog.open,true);assert.equal(c.sent.length,0);assert.equal(c.run('operation'),operation);
    c.run('confirm()');assert.equal(c.sent.length,1);
  }
  const c=component();
  for(const owner of ['graph:a','chat:other'])c.window.dispatchEvent(new CustomEvent('central-agent:native-command',{detail:{owner,command:'archive'}}));
  assert.equal(c.dialog.open,false);assert.equal(c.sent.length,0);
  c.update({busy:true});const blocked=new CustomEvent('central-agent:native-command',{cancelable:true,detail:{owner:c.owner,command:'archive'}});c.window.dispatchEvent(blocked);assert.equal(blocked.defaultPrevented,false);
});

test('reported thread settings remain owner-scoped runtime data without becoming a setting',()=>{
  for(const graph of [false,true]) {
    const c=component(graph),report={cwd:'C:\\fixture',model:'native-model',modelProvider:'openai',effort:'high',serviceTier:null,summary:'concise',personality:null,approvalsReviewer:'user',approvalPolicy:'on-request',sandboxKind:'readOnly'};
    c.update({threadSettings:report,threadSettingsCurrent:true});
    assert.equal(c.run('view.threadSettings.model'),'native-model');
    c.update({threadSettings:{...report,model:'foreign'}},'other');
    assert.equal(c.run('view.threadSettings.model'),'native-model');
    c.update({threadSettings:report,threadSettingsCurrent:false,connected:false});
    assert.equal(c.run('view.threadSettingsCurrent'),false);
    assert.equal(c.run('view.threadSettings.model'),'native-model');
    assert.equal(c.sent.length,0);
    c.update({threadSettings:null,threadSettingsCurrent:false,binding:{threadId:'another',deleted:false,archived:false}});
    assert.equal(c.run('view.threadSettings'),null);
  }
  const source=fs.readFileSync(new URL('../src/components/NativeConversation.svelte',import.meta.url),'utf8');
  assert.doesNotMatch(source,/Reported native thread settings|Last observed settings; this report is stale|Saved defaults are not proof/);
});

test('successful reconnect clears a stale connection-required conversation alert',()=>{
  for(const graph of [false,true]) {
    const c=component(graph);
    c.update({connected:false});
    c.window.dispatchEvent(new CustomEvent('central-agent:app-server-conversation',{detail:{owner:c.owner,error:'Connect Codex in AI accounts first'}}));
    assert.match(c.run('error'),/Connect Codex/);
    c.update({connected:true});
    assert.equal(c.run('error'),'');

    c.window.dispatchEvent(new CustomEvent('central-agent:app-server-conversation',{detail:{owner:c.owner,error:'Native operation failed'}}));
    c.update({connected:true});
    assert.equal(c.run('error'),'Native operation failed');
  }
});

test('configuration host keeps native modals and loading rerenders visible',()=>{
  const source=fs.readFileSync(new URL('../../assets/agent-panel.html',import.meta.url),'utf8');
  assert.match(source,/picker\.host\.querySelector\("dialog\[open\]"\)/);
  assert.match(source,/event\.relatedTarget === null/);
  const sharedTheme=fs.readFileSync(new URL('../../assets/themes.css',import.meta.url),'utf8');
  assert.match(sharedTheme,/\.workspace-confirm-dialog::backdrop/);
  for(const host of ['agent-panel.html','agent-graph.html'])
    assert.ok(fs.readFileSync(new URL(`../../assets/${host}`,import.meta.url),'utf8').includes('/*__THEMES_CSS__*/'));
  const settings=fs.readFileSync(new URL('../src/components/SettingsShell.svelte',import.meta.url),'utf8');
  assert.match(settings,/id="settings-back".*aria-label="Back to workspace"/);
});

test('terminal panel accepts the shared Rust-owned UI owner envelope',()=>{
  const source=fs.readFileSync(new URL('../../src/browser.rs',import.meta.url),'utf8');
  const terminal=source.slice(source.indexOf('fn build_terminal_panel'),source.indexOf('fn create_tab'));
  assert.match(terminal,/main_runs::parse_control\(request\.body\(\)\)/);
  assert.match(terminal,/BrowserEvent::TerminalPanel\(message\)/);
  assert.doesNotMatch(terminal,/serde_json::from_str::<AgentPanelMessage>/);
});
test('native MCP administration stays out of user settings while runtime support remains',()=>{
  const conversation=fs.readFileSync(new URL('../src/components/NativeConversation.svelte',import.meta.url),'utf8');
  const account=fs.readFileSync(new URL('../src/components/AppServerAccount.svelte',import.meta.url),'utf8');
  assert.doesNotMatch(conversation,/NativeMcpSettings|mcp_status|mcp_control/);
  assert.doesNotMatch(account,/NativeMcpSettings|Codex MCP servers|data-native-protocol-diagnostics/);
  assert.ok(fs.existsSync(new URL('../../src/browser/app_server/mcp.rs',import.meta.url)));
});

test('a native fork-dependency refusal preserves the binding and requires explicit retry',()=>{
  for(const graph of [false,true]) {
    const c=component(graph);c.run('begin("delete");confirm()');
    const error='cannot delete thread native-a: forked history still references it';
    c.window.dispatchEvent(new CustomEvent('central-agent:app-server-conversation',{detail:{owner:c.owner,error}}));
    assert.equal(c.run('pending'),false);assert.equal(c.run('error'),error);
    assert.equal(c.run('view.binding.deleted'),false);assert.equal(c.sent.length,1);
    assert.equal(c.dialog.open,false);c.run('begin("delete")');assert.equal(c.sent.length,1);
    c.run('confirm()');assert.equal(c.sent.length,2);
  }
  const source=fs.readFileSync(new URL('../src/components/NativeConversation.svelte',import.meta.url),'utf8');
  assert.match(source,/fork still references this history/);
  assert.match(source,/forks are never deleted automatically/);
});

test('native deletion is explicit, owner-bound and cannot be double-submitted',()=>{
  const c=component();c.run('begin("delete")');assert.equal(c.sent.length,0);assert.equal(c.dialog.open,true);
  c.run('confirm();confirm()');assert.equal(c.sent.length,1);
  assert.equal(c.sent[0].owner,'chat:a');assert.equal(c.sent[0].action.kind,'delete');assert.equal(c.sent[0].action.expected_thread_id,'native-a');
  c.window.dispatchEvent(new CustomEvent('central-agent:app-server-conversation',{detail:{owner:'other',change:'updated'}}));
  assert.equal(c.run('pending'),true);
  c.window.dispatchEvent(new CustomEvent('central-agent:app-server-conversation',{detail:{owner:'chat:a',error:'Native rejection'}}));
  assert.equal(c.run('pending'),false);assert.equal(c.run('error'),'Native rejection');
});

test('native compaction requires an idle loaded history, preserves its target and never treats ACK as completion',()=>{
  for(const graph of [false,true]){
    const c=component(graph);c.run('begin("compact")');assert.equal(c.dialog.open,false);
    c.update({readyForTurn:true,canCompact:true});c.run('begin("compact")');assert.equal(c.dialog.open,true);assert.equal(c.sent.length,0);
    c.run('confirm();confirm()');assert.equal(c.sent.length,1);assert.deepEqual(JSON.parse(JSON.stringify(c.sent[0])),{owner:c.owner,action:{kind:'compact',expected_thread_id:'native-a'}});
    c.window.dispatchEvent(new CustomEvent('central-agent:app-server-conversation',{detail:{owner:c.owner,change:'compaction_accepted',nativeConversation:{visible:true,connected:true,busy:true,compaction:'accepted',binding:{threadId:'native-a'}}}}));
    assert.equal(c.run('allowed()'),false);assert.equal(c.run('view.compaction'),'accepted');
    c.update({canCompact:true});c.run('begin("compact")');c.update({canCompact:false});c.run('confirm()');assert.equal(c.sent.length,1);
  }
});

test('switching main chat or native binding invalidates an open confirmation',()=>{
  const c=component();c.run('begin("archive")');
  c.window.dispatchEvent(new CustomEvent('central-agent:conversation-key',{detail:'chat:b'}));
  assert.equal(c.dialog.open,false);c.update({},'chat:b');c.run('confirm()');assert.equal(c.sent.length,0);
  c.window.dispatchEvent(new CustomEvent('central-agent:conversation-key',{detail:'chat:a'}));c.update();c.run('begin("delete")');
  c.update({binding:{threadId:'replacement',archived:false,deleted:false}});c.run('confirm()');assert.equal(c.sent.length,0);assert.equal(c.dialog.open,false);
});

test('graph controls preserve the node owner and handle disconnect, archive and deletion',()=>{
  const c=component(true);c.update({busy:true},'graph:other');c.run('begin("rename");name="Graph name";confirm()');
  assert.equal(c.sent[0].owner,'graph:a');assert.equal(c.sent[0].action.name,'Graph name');
  c.update({connected:false});assert.equal(c.run('pending'),false);
  c.update({observed:false,binding:{threadId:'native-a',archived:true,deleted:false}});
  c.run('begin("delete")');assert.equal(c.dialog.open,false);
  c.run('begin("unarchive");confirm()');assert.equal(c.sent[1].action.kind,'unarchive');
  c.update({binding:{threadId:'native-a',archived:false,deleted:true}});
  c.run('begin("delete");read()');assert.equal(c.sent.length,2);assert.equal(c.dialog.open,false);
});

test('new branches freeze native source, local directory and owner without submitting a prompt',()=>{
  for(const graph of [false,true]) {
    const c=component(graph);c.run('begin("new_fork")');assert.equal(c.dialog.open,true);
    assert.equal(c.run('name'),'Fork of Native name');assert.equal(c.sent.length,0);
    c.run('name="Independent branch";confirm();confirm()');
    assert.equal(c.sent.length,1);assert.equal(c.sent[0].owner,c.owner);
    assert.deepEqual(JSON.parse(JSON.stringify(c.sent[0].action)),{kind:'new_fork',expected_thread_id:'native-a',name:'Independent branch',expected_directory:'C:\\fixture',last_turn_id:null});
    c.window.dispatchEvent(new CustomEvent('central-agent:app-server-conversation',{detail:{owner:'foreign',change:'branched'}}));
    assert.equal(c.run('pending'),true);
    c.window.dispatchEvent(new CustomEvent('central-agent:app-server-conversation',{detail:{owner:c.owner,change:'branched'}}));
    assert.equal(c.run('pending'),false);
  }
});

test('changed project or disconnected owner invalidates a new branch confirmation',()=>{
  const c=component();c.run('begin("new_fork")');c.update({directory:'C:\\different'});c.run('confirm()');
  assert.equal(c.sent.length,0);assert.equal(c.dialog.open,false);assert.match(c.run('error'),/project changed/);
  c.update();c.run('begin("new_fork")');c.update({connected:false});c.run('confirm()');
  assert.equal(c.sent.length,0);assert.equal(c.dialog.open,false);
});

test('precise native branches freeze an explicitly selected completed turn',()=>{
  for(const graph of [false,true]){
    const c=component(graph);c.update({completedTurns:[{id:'turn-1',status:'completed'},{id:'turn-2',status:'completed'}]});
    c.run('begin("new_fork");name="Through first";forkTurnId="turn-1";confirm()');
    assert.equal(c.sent.length,1);assert.equal(c.sent[0].action.last_turn_id,'turn-1');
    assert.equal(c.sent[0].action.expected_directory,'C:\\fixture');
  }
});

test('archived bound conversations cannot fork from controls or slash commands',()=>{
  for(const graph of [false,true]) {
    const c=component(graph);
    c.update({binding:{threadId:'native-a',archived:true,deleted:false}});
    assert.equal(c.run('begin("new_fork")'),false);
    const command=new CustomEvent('central-agent:native-command',{cancelable:true,detail:{owner:c.owner,command:'fork'}});
    c.window.dispatchEvent(command);
    assert.equal(command.defaultPrevented,false);
    assert.equal(c.dialog.open,false);assert.equal(c.sent.length,0);
    // Restoring remains a separate, explicitly confirmed native action.
    c.run('begin("unarchive");confirm()');
    assert.equal(c.sent.length,1);assert.equal(c.sent[0].action.kind,'unarchive');
  }
});

test('archive events close an open fork confirmation and reject stale submission',()=>{
  for(const graph of [false,true]) {
    const c=component(graph);c.run('begin("new_fork")');
    assert.equal(c.dialog.open,true);
    c.update({binding:{threadId:'native-a',archived:true,deleted:false}},'foreign');
    assert.equal(c.dialog.open,true);
    c.update({binding:{threadId:'native-a',archived:true,deleted:false}});
    assert.equal(c.dialog.open,false);
    c.run('confirm()');assert.equal(c.sent.length,0);
    c.update();assert.equal(c.run('begin("new_fork")'),true);
    c.run('confirm()');assert.equal(c.sent.length,1);
    assert.equal(c.sent[0].action.kind,'new_fork');
  }
});

test('native window events open a same-node graph branch while preserving other cards',()=>{
  const html=fs.readFileSync(new URL('../../assets/agent-graph.html',import.meta.url),'utf8');
  const start=html.indexOf('window.addEventListener("central-agent:graph-open-conversation"');
  assert.ok(start>=0,'Native events are dispatched on window, not document');
  const handler=html.slice(start,html.indexOf('      function renderAgentConsole',start));
  const revealed=[];
  const window=new EventTarget();window.CentralAgentSvelte={graphWindowSelection,revealBoardAgent:key=>revealed.push(key)};
  const graph={openAgentKeys:new Set(['source','sibling'])};let renders=0,focuses=0;
  const card={dataset:{nodeKey:'branch'},dispatchEvent(){},querySelector(){return {focus(){focuses++;}};}};
  const ctx=vm.createContext({window,graph,CustomEvent,currentState:{},
    nodeForAgentKey:key=>['source','branch','foreign'].includes(key)?{key:key==='foreign'?'other-node':'same-node'}:null,
    agentTarget:key=>key==='branch'?{conversation_id:'branch'}:null,
    saveAgentWindowPreferences(){},renderAgentConsole(){renders++;},drawGraph(){},
    agentWindowLayer:{querySelectorAll:()=>[card]},document:{querySelectorAll:()=>[card]},requestAnimationFrame:callback=>callback()});
  vm.runInContext(handler,ctx);
  for(const detail of [{owner:'chat:source',nodeKey:'branch'},{owner:'graph:source',nodeKey:'foreign'},{owner:'graph:missing',nodeKey:'branch'}]) {
    window.dispatchEvent(new CustomEvent('central-agent:graph-open-conversation',{detail}));
  }
  assert.equal(renders,0);
  window.dispatchEvent(new CustomEvent('central-agent:graph-open-conversation',{detail:{owner:'graph:source',nodeKey:'branch',replaceCurrent:false}}));
  assert.deepEqual([...graph.openAgentKeys],['source','sibling','branch']);assert.equal(renders,1);assert.equal(focuses,1);
  assert.deepEqual(revealed,['branch']);
});

test('native reviews send exact typed targets, preserve the owner and require confirmation',()=>{
  for(const [kind,value,target] of [
    ['uncommittedChanges','',{type:'uncommittedChanges'}],
    ['baseBranch','main',{type:'baseBranch',branch:'main'}],
    ['commit','abc123',{type:'commit',sha:'abc123',title:null}],
    ['custom','Check empty inputs',{type:'custom',instructions:'Check empty inputs'}],
  ]) {
    const c=component(true);c.run(`begin('start_review');reviewKind=${JSON.stringify(kind)};reviewValue=${JSON.stringify(value)}`);
    assert.equal(c.sent.length,0);c.run('confirm();confirm()');assert.equal(c.sent.length,1);
    assert.deepEqual(JSON.parse(JSON.stringify(c.sent[0])),{owner:'graph:a',action:{kind:'start_review',expected_thread_id:'native-a',expected_directory:'C:\\fixture',target,delivery:'inline',destination_name:null}});
  }
});

test('review confirmation rejects empty targets, changed roots and archived history',()=>{
  const c=component();c.run('begin("start_review");reviewKind="custom";reviewValue=" ";confirm()');assert.equal(c.sent.length,0);
  c.run('reviewValue="Inspect errors"');c.update({directory:'C:\\different'});c.run('confirm()');assert.equal(c.sent.length,0);
  c.update({binding:{threadId:'native-a',archived:true,deleted:false}});c.run('begin("start_review")');assert.equal(c.dialog.open,false);
});

test('review history format explains native detached limits without substituting another action',()=>{
  for(const graph of [false,true]) {
    const c=component(graph);
    c.update({historyMode:'paginated'});
    assert.match(c.run('separateReviewNotice()'),/does not support separate review delivery for paginated histories/);
    c.run('begin("start_review");reviewDelivery="detached";reviewName="Blocked";confirm()');assert.equal(c.sent.length,0);
    c.update({historyMode:'legacy'});assert.match(c.run('separateReviewNotice()'),/reviewThreadId returned by Codex/);
    c.run('begin("start_review");reviewDelivery="detached";reviewName="Separate review";confirm()');
    assert.equal(c.sent.length,1);assert.equal(c.sent[0].action.kind,'start_review');
    assert.equal(c.sent[0].action.delivery,'detached');assert.equal(c.sent[0].action.destination_name,'Separate review');
    for(const historyMode of [null,undefined]) {
      c.update({historyMode});assert.match(c.run('separateReviewNotice()'),/did not report a compatible history mode/);
    }
  }
});

test('unused native links reset only after exact-owner confirmation, without a native delete or prompt',()=>{
  for(const graph of [false,true]) {
    const c=component(graph);c.update({observed:false,unusedLink:true});
    c.run('begin("reset_unused")');assert.equal(c.dialog.open,true);assert.equal(c.sent.length,0);
    c.run('confirm();confirm()');assert.equal(c.sent.length,1);
    assert.deepEqual(JSON.parse(JSON.stringify(c.sent[0])),{owner:c.owner,action:{kind:'reset_unused',expected_thread_id:'native-a'}});
    c.window.dispatchEvent(new CustomEvent('central-agent:app-server-conversation',{detail:{owner:c.owner,change:'unused_link_reset',nativeConversation:{visible:true,connected:true,busy:false,observed:false,binding:null}}}));
    assert.equal(c.run('pending'),false);assert.equal(c.dialog.open,false);
  }
});

test('reset cannot unlink used, changed, busy, disconnected or foreign conversations',()=>{
  for(const patch of [{unusedLink:false},{busy:true},{connected:false},{binding:{threadId:'replacement',archived:false,deleted:false}},{visible:false}]) {
    const c=component();c.update({observed:false,unusedLink:true});c.run('begin("reset_unused")');
    c.update({observed:false,unusedLink:true,...patch});c.run('confirm()');assert.equal(c.sent.length,0);
  }
  const c=component();c.update({observed:false,unusedLink:false});c.run('begin("reset_unused")');assert.equal(c.dialog.open,false);
  c.update({observed:false,unusedLink:true},'chat:foreign');c.run('begin("reset_unused")');assert.equal(c.dialog.open,false);
  c.update({observed:false,unusedLink:true});c.run('begin("reset_unused")');
  c.window.dispatchEvent(new CustomEvent('central-agent:conversation-key',{detail:'chat:b'}));c.run('confirm()');assert.equal(c.sent.length,0);
});
