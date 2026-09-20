import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';
import { graphTimelineMessages } from '../src/lib/graph-history.ts';

test('compact graph assignment preserves its owner, saved mission and selected profile without reading retired controls', () => {
  const host = readFileSync(new URL('../../assets/agent-graph.html', import.meta.url), 'utf8');
  const source = host.slice(host.indexOf('function popupDraft(card)'), host.indexOf('function renderConsoleHistory(log, run)'));
  const sent = [];
  const fields = {name:'Saved agent',mission:'Keep the saved mission',provider:'codex_app_server',model:'selected-model',effort:'low',speed:''};
  const card = {dataset:{nodeKey:'conversation:branch'},querySelector(selector) {
    const role = /^\[data-role="([^"]+)"\]$/.exec(selector)?.[1];
    assert.ok(Object.hasOwn(fields,role), `Unexpected retired control read: ${selector}`);
    return {value:fields[role]};
  }};
  const target = {record_type:'entity',id:'node',conversation_id:'branch'};
  const api = runInNewContext(`${source}; ({draft:popupDraft,save:savePopupAssignment})`, {
    nodeForAgentKey:key=>key===card.dataset.nodeKey?{label:'Node'}:null,
    nodeVisibleLabel:node=>node.label,defaultAgentName:()=> 'Default agent',
    agentTarget:key=>{assert.equal(key,card.dataset.nodeKey);return target;},
    send:(kind,payload)=>sent.push({kind,payload:JSON.parse(JSON.stringify(payload))}),
  });
  assert.deepEqual(JSON.parse(JSON.stringify(api.draft(card))),fields);
  assert.equal(sent.length,0);
  fields.model='new-model';fields.effort='high';fields.speed='priority';
  assert.equal(api.save(card),true);
  assert.deepEqual(sent,[{kind:'set_knowledge_agent',payload:{...target,name:fields.name,mission:fields.mission,provider:fields.provider,model:fields.model,effort:fields.effort,service_tier:'priority',context_window:null}}]);
  assert.equal(api.save({...card,dataset:{nodeKey:'missing'}}),false);
  assert.equal(sent.length,1);
});

test('actual graph host invalidates native deltas without requiring a runtime phase change', () => {
  const host = readFileSync(new URL('../../assets/agent-graph.html', import.meta.url), 'utf8');
  const source = host.slice(host.indexOf('function consoleHistorySignature(run)'), host.indexOf('function popupSelection(card)'));
  const signature = runInNewContext(`(${source.trim()})`);
  const run = {agent:{phase:'running',status:'Working',active:true},conversationState:{nativeMessages:[
    {id:'native:a:t:i',role:'assistant',text:'One',streaming:true}
  ]}};
  const initial = signature(run);
  run.conversationState.nativeMessages[0].text += ' two';
  assert.notEqual(signature(run), initial);
  const delta = signature(run);
  run.conversationState.nativeMessages[0].streaming = false;
  assert.notEqual(signature(run), delta);
  const completed = signature(run);
  run.conversationState.pendingRequests = [{id:'unrelated-approval'}];
  assert.equal(signature(run), completed, 'non-timeline updates do not remount the log');
  assert.equal(signature({}), signature({conversationState:{nativeMessages:null}}));
});

test('new App Server messages retain native IDs beside unchanged other-provider history', () => {
  const messages=[{id:'native:thread:turn:user',role:'user',text:'Native request'},
    {id:'native:thread:turn:answer',role:'assistant',text:'Stream',streaming:true}];
  const rows=graphTimelineMessages({messages:[{id:2,text:'Claude history'}],conversationState:{nativeMessages:messages}});
  assert.deepEqual(rows.map(r=>r.id),[2,...messages.map(r=>r.id)]);
  assert.equal(rows[2].streaming,true);
  assert.equal(graphTimelineMessages({conversationState:{nativeMessages:messages}}).length,2);
});

test('graph history keeps native activity, phases, duration and artifacts without duplicate step summaries', () => {
  const native = [
    {id:1,role:'user',text:'Inspect',nativeTurnId:'turn-1',nativeTurnTiming:{durationMs:9876},fileAttachments:[{id:'file-1',name:'Cargo.toml',kind:'text',iconKey:'cargo'}],attachments:[{tabId:7,title:'Captured page',previewDataUrl:'data:image/jpeg;base64,YWJj'}],terminalAttachments:[{sessionId:9,outputPreview:'Frozen output'}]},
    {id:2,role:'assistant',kind:'reasoning',text:'Read-only inspection'},
    {id:3,role:'assistant',kind:'activity',text:'Read',activityCategory:'read',activityDetail:'src/main.rs',activityContext:'C:/project',activityStatus:'success',activityDiff:'+line',activityAdditions:1},
    {id:4,role:'assistant',text:'Done',messagePhase:'final_answer',artifacts:[{id:'image'}]},
  ];
  const result = graphTimelineMessages({history:[{runId:8,request:'Inspect',finishedAtMs:20,messages:native,steps:[{label:'Read'}]}]});
  assert.deepEqual(result.map(row => row.id), [1,2,3,4]);
  assert.equal(result[0].nativeTurnTiming.durationMs, 9876);
  assert.deepEqual(result[0].fileAttachments, native[0].fileAttachments);
  assert.deepEqual(result[0].attachments, native[0].attachments);
  assert.deepEqual(result[0].terminalAttachments, native[0].terminalAttachments);
  assert.equal(result[2].activityDiff, '+line');
  assert.deepEqual(result[3].artifacts, [{id:'image'}]);
  assert.ok(result.every(row => row.runId === 8));
});

test('live graph messages replace the unfinished saved snapshot and do not duplicate a retired run', () => {
  const active = {runId:2,finishedAtMs:null,messages:[{id:2,text:'stale'}]};
  const result = graphTimelineMessages({agent:{runId:2,active:true},history:[{runId:1,finishedAtMs:2,messages:[{id:1,text:'Earlier'}]},active],messages:[{id:2,runId:2,text:'Live',streaming:true}]});
  assert.deepEqual(result.map(row => row.text), ['Earlier','Live']);
  assert.equal(graphTimelineMessages({history:[{runId:2,finishedAtMs:3,messages:[{id:2,text:'Done'}]}],messages:[{id:2,text:'Done'}]}).length, 1);
});

test('legacy graph sessions retain requests, steps, errors and checkpoints without inventing native timing', () => {
  const rows = graphTimelineMessages({history:[{runId:9,request:'Legacy',finishedAtMs:50,steps:[{label:'Command',status:'error',detail:'stderr'}],messages:[{id:3,role:'assistant',text:'Partial'}],checkpoint:{id:'checkpoint'}}]});
  assert.deepEqual(rows.map(row => row.kind || 'message'), ['message','activity','message','message']);
  assert.equal(rows[0].text,'Legacy'); assert.equal(rows[0].nativeTurnTiming, undefined);
  assert.equal(rows[1].activityDetail, 'stderr'); assert.equal(rows[1].activityStatus, 'error');
  assert.equal(rows[3].checkpoint.id,'checkpoint');
});

test('every loaded graph turn is readable; an older-page prepend retains stable message identities', () => {
  const history = Array.from({length:63}, (_,index) => ({runId:index+1,finishedAtMs:100,messages:[{id:index+1,text:`Turn ${index+1}`,role:'assistant'}]}));
  const all = graphTimelineMessages({history});
  assert.equal(all.length,63);
  assert.deepEqual(all.slice(43), graphTimelineMessages({history:history.slice(43)}));
});
