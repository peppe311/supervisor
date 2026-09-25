import test from 'node:test';
import assert from 'node:assert/strict';
import {activityFiles,fileActivityLabel,fileWorkLabel,agentInProject,agentKey,projectChats,fileRows,sameLocalPath} from '../src/lib/project-board.ts';
import {boardLayout,readBoardLayout,writeBoardLayout} from '../src/lib/board-layout.ts';
import {comparisonReady,commandOutcome} from '../src/lib/work-results.ts';

test('work comparisons require complete idle reports and a separate native reviewer',()=>{
  const a={owner:'chat:a',loaded:true,busy:false,partial:false,remote:false,pendingRequests:0};
  const b={...a,owner:'chat:b'};
  const reviewer={owner:'graph:r',reviewer:true,busy:false,threadId:'native-reviewer'};
  assert.equal(comparisonReady(a,b,reviewer),true);
  for(const patch of [{loaded:false},{busy:true},{partial:true},{remote:true},{pendingRequests:1}]){
    assert.equal(comparisonReady({...a,...patch},b,reviewer),false);
    assert.equal(comparisonReady(a,{...b,...patch},reviewer),false);
  }
  assert.equal(comparisonReady(a,a,reviewer),false);
  for(const patch of [{owner:a.owner},{owner:b.owner},{busy:true},{reviewer:false},{threadId:null}]){
    assert.equal(comparisonReady(a,b,{...reviewer,...patch}),false);
  }
  assert.equal(comparisonReady(null,b,reviewer),false);
});

test('missing command outcomes are never presented as a successful check',()=>{
  assert.equal(commandOutcome({status:'succeeded',exitCode:0}),'Exit 0');
  assert.equal(commandOutcome({status:'completed',exitCode:1}),'Exit 1');
  assert.equal(commandOutcome({status:'completed',exitCode:null}),'Exit not reported');
  assert.equal(commandOutcome({status:'failed',exitCode:null}),'Failed');
  assert.equal(commandOutcome({status:'stopped',exitCode:null}),'Stopped');
});

test('board layout keeps bounded presentation state only',()=>{
  const restored=boardLayout({selectedChat:'a',openChats:['a','a','b',null],minimized:['chat:a'],projectsWidth:Infinity,filesWidth:9000,chatShare:-4,execute:'never',permissions:'full'});
  assert.deepEqual(restored.openChats,['a','b']);assert.deepEqual(restored.minimized,['chat:a']);
  assert.equal(restored.filesWidth,600);assert.equal(restored.projectsWidth,0);assert.equal(restored.chatShare,.25);
  assert.ok(!('active' in restored));assert.ok(!('permissions' in restored));assert.ok(!('objectives' in restored));
});

test('presentation state follows a project only during the current app session',()=>{
  const restored=readBoardLayout('fixture-session');
  restored.openChats=['original','fork'];restored.openAgents=['conversation:s'];restored.minimized=['chat:fork'];restored.projectsWidth=320;
  writeBoardLayout('fixture-session',restored);
  assert.deepEqual(readBoardLayout('fixture-session').openAgents,['conversation:s']);
  assert.equal(readBoardLayout('fixture-session').projectsWidth,320);
  assert.deepEqual(readBoardLayout('other-session').openChats,[]);
});

const local={id:'local',nodeKey:'entity:local',source:'local',path:'C:\\work\\app',name:'App'};
const remote={id:'remote',nodeKey:'entity:remote',source:'ssh',path:'/srv/app',sshProfileId:'server-a',name:'Remote'};

const file=(path,state='working',operation='read',activeOperations=state==='working'?1:0)=>({path,state,operation,activeOperations});
const task=(owner,files,state='working')=>({owner,label:owner,state,files});

test('parallel owners stay identifiable and a completed edit does not hide another active read',()=>{
  const work=[task('chat:a',[file('src/A.rs','done','edit')],'done'),task('graph:b',[file('src/a.rs'),file('src/b.rs','working','edit')])];
  let files=activityFiles(work);
  assert.equal(files.length,2);
  const shared=files.find(f=>f.path==='src/A.rs');
  assert.equal(shared.state,'working');assert.equal(shared.operation,'read');assert.equal(shared.activeOperations,1);
  assert.deepEqual(shared.owners.map(o=>o.owner),['chat:a','graph:b']);
  assert.equal(fileWorkLabel(shared),'Reading');
  assert.equal(fileActivityLabel(files,'src',true),'2 active');
  work[1].files[0]=file('src/a.rs','stopped');
  files=activityFiles(work);
  assert.equal(fileActivityLabel(files,'SRC/A.RS',false),'Stopped');
  assert.equal(fileActivityLabel(files,'src',true),'1 active');
  assert.equal(activityFiles([work[0]])[0].state,'done');
});

test('overlapping edits show operation counts; waiting and errors remain explicit',()=>{
  const work=[task('a',[file('a.rs'),file('b.rs','waiting')]),task('b',[file('a.rs','working','edit',2),file('c.rs','failed')])];
  const files=activityFiles(work);
  assert.equal(fileWorkLabel(files[0]),'Editing · 3 operations');
  assert.equal(fileWorkLabel(files[1]),'Waiting');
  assert.equal(fileWorkLabel(files[2]),'Failed');
  assert.equal(fileActivityLabel(files,'elsewhere',true),'');
});

test('remote case sensitivity and untrusted escaping activity paths preserve project boundaries',()=>{
  const work=[task('a',[file('Src/A.rs'),file('src/a.rs'),...['../secret','/etc/passwd','C:/other','src/../../secret'].map(p=>file(p))])];
  assert.equal(activityFiles(work,true).length,2);
  assert.equal(activityFiles(work,false).length,1);
  assert.equal(fileActivityLabel(activityFiles(work,true),'SRC/A.rs',false,true),'');
});

test('project selection preserves independent owners and excludes similarly named or remote paths',()=>{
  const original={recordType:'entity',recordId:'local',projectDirectory:local.path};
  const fork={...original,conversationId:'fork'};
  assert.notEqual(agentKey(original),agentKey(fork));
  assert.ok(agentInProject(original,local)&&agentInProject(fork,local));
  assert.ok(agentInProject({recordType:'entity',recordId:'legacy',projectDirectory:'c:/work/app/'},local));
  assert.ok(!agentInProject({recordType:'entity',recordId:'other',projectDirectory:'C:/work/application'},local));
  assert.ok(!agentInProject({recordType:'entity',recordId:'other',projectDirectory:local.path,sshProfileId:'server-a'},local));
  assert.ok(agentInProject({recordType:'entity',recordId:'legacy',projectDirectory:'/srv/app',sshProfileId:'server-a'},remote));
  assert.ok(!agentInProject({recordType:'entity',recordId:'legacy',projectDirectory:'/srv/app',sshProfileId:'server-b'},remote));
  assert.ok(!agentInProject({recordType:'entity',recordId:'legacy',projectDirectory:'/srv/App',sshProfileId:'server-a'},remote));
  assert.ok(sameLocalPath('\\\\?\\C:\\work\\app','c:/work/app/'));
});

test('only nonarchived same-project conversations are offered for association',()=>{
  const chats=[{id:'a',title:'A',projectRoot:'c:/work/app'}, {id:'b',title:'B',projectRoot:local.path,archived:true}, {id:'c',title:'C',projectRoot:'c:/work/other'}];
  assert.deepEqual(projectChats(chats,local).map(chat=>chat.id),['a']);
  assert.deepEqual(projectChats(chats,remote),[]);
  assert.deepEqual(projectChats(chats,null),[]);
});

test('bounded file tree rejects escaping paths, expands parents and searches collapsed branches',()=>{
  const entries=[{path:'src/lib/core.ts',kind:'file',iconKey:'typescript'},{path:'README.md',kind:'file'},
    ...['../secret','/etc/passwd','C:/secret','src/../../secret','./secret','src//file'].map(path=>({path,kind:'file'}))];
  assert.deepEqual(fileRows(entries,new Set()).map(row=>row.path),['src','README.md']);
  const expanded=fileRows(entries,new Set(['src','src/lib']));
  assert.deepEqual(expanded.map(row=>[row.path,row.depth]),[['src',0],['src/lib',1],['src/lib/core.ts',2],['README.md',0]]);
  assert.equal(expanded[2].iconKey,'typescript');
  assert.equal(fileRows(entries,new Set(),'CORE')[0].path,'src/lib/core.ts');
  assert.equal(fileRows(Array.from({length:1800},(_,i)=>({path:`file${i}`,kind:'file'})),new Set()).length,1500);
});
