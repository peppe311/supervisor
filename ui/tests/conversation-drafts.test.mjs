import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { ConversationDrafts } from '../src/lib/conversation-drafts.ts';
import { acceptsComposerReceipt } from '../src/lib/composer-receipt.ts';

test('switching A to B and back keeps both drafts when WebView storage is denied', () => {
  const drafts = new ConversationDrafts(() => { throw new Error('SecurityError'); }, 'drafts');
  drafts.write('chat:a', 'Continue A');
  assert.equal(drafts.read('chat:b'), '');
  drafts.write('chat:b', 'Continue B');
  assert.equal(drafts.read('chat:a'), 'Continue A');
  assert.equal(drafts.read('chat:b'), 'Continue B');
});

test('a background acknowledgement neither clears the visible chat nor resurrects cleared drafts', () => {
  const records = new Map();
  const storage = { getItem: key => records.get(key), setItem: (key, value) => records.set(key, value), removeItem: key => records.delete(key) };
  const drafts = new ConversationDrafts(() => storage, 'drafts');
  drafts.write('chat:a', 'Continue');
  drafts.write('chat:b', 'Continue');
  const receipt = { owner: 'chat:a', input: 'Continue' };
  assert.equal(acceptsComposerReceipt(receipt, 'chat:b', drafts.read('chat:b')), false);
  assert.equal(drafts.read('chat:b'), 'Continue');
  if (acceptsComposerReceipt(receipt, 'chat:a', drafts.read('chat:a'))) drafts.write('chat:a', '');
  assert.equal(drafts.read('chat:a'), '');
  assert.equal(new ConversationDrafts(() => storage, 'drafts').read('chat:a'), '');
  assert.equal(new ConversationDrafts(() => storage, 'drafts').read('chat:b'), 'Continue');
});

test('write failures cannot replace a newer session draft with stale storage', () => {
  const drafts = new ConversationDrafts(() => ({ getItem: () => 'Old', setItem: () => { throw new Error('quota'); }, removeItem: () => { throw new Error('quota'); } }), 'drafts');
  assert.equal(drafts.read('chat:a'), 'Old');
  drafts.write('chat:a', 'New');
  assert.equal(drafts.read('chat:a'), 'New');
  drafts.write('chat:a', '');
  assert.equal(drafts.read('chat:a'), '');
});

test('branch prefills target their new owner without replacing another or a newer draft', () => {
  const drafts = new ConversationDrafts(() => { throw new Error('SecurityError'); }, 'drafts');
  drafts.write('chat:source', 'Keep my current draft');
  assert.equal(drafts.seed('chat:branch', 'Edited earlier prompt'), true);
  assert.equal(drafts.read('chat:source'), 'Keep my current draft');
  assert.equal(drafts.read('chat:branch'), 'Edited earlier prompt');
  assert.equal(drafts.seed('chat:branch', 'Edited earlier prompt'), true);
  drafts.write('chat:branch', 'Newer manual draft');
  assert.equal(drafts.seed('chat:branch', 'Edited earlier prompt'), false);
  assert.equal(drafts.read('chat:branch'), 'Newer manual draft');
  assert.equal(drafts.seed('', 'No destination'), false);
});

function nativeDrafts() {
  const sent=[],updates=[];
  const drafts=new ConversationDrafts(()=>{throw new Error('Opaque WebView');},'drafts',{
    send:request=>sent.push(request),updated:(owner,error)=>updates.push({owner,error}),
  });
  const reply=(request,detail)=>drafts.receive({owner:request.owner,requestId:request.request_id,...detail});
  return {drafts,sent,updates,reply};
}

test('an unassigned graph draft stays local and is saved when its agent is assigned', () => {
  const c = nativeDrafts(), owner = 'graph:new-node';
  assert.equal(c.drafts.readLocal(owner), '');
  c.drafts.writeLocal(owner, 'Prepared before assignment');
  assert.equal(c.sent.length, 0);
  assert.equal(c.drafts.status(owner), 'local');
  assert.equal(c.drafts.synchronize(owner), 'Prepared before assignment');
  assert.equal(c.sent.length, 1);
  c.reply(c.sent[0], {revision: '', text: ''});
  assert.deepEqual(c.sent[1].action, {kind: 'write', revision: '', text: 'Prepared before assignment'});
  c.reply(c.sent[1], {revision: 'assigned'});
  assert.equal(c.drafts.status(owner), 'saved');
  assert.equal(c.drafts.read('graph:other-node'), '');
  assert.equal(c.drafts.read(owner), 'Prepared before assignment');
});

test('new setup edits survive hydration while an untouched cache respects a durable clear', () => {
  const c = nativeDrafts();
  c.drafts.writeLocal('graph:edited', 'New setup text');
  c.drafts.synchronize('graph:edited');
  c.reply(c.sent[0], {revision: 'cleared', text: ''});
  assert.equal(c.drafts.read('graph:edited'), 'New setup text');
  assert.deepEqual(c.sent[1].action, {kind: 'write', revision: 'cleared', text: 'New setup text'});
  const sent = [];
  const drafts = new ConversationDrafts(() => ({getItem: () => 'Old cache', setItem() {}, removeItem() {}}), 'drafts', {send: r => sent.push(r), updated() {}});
  assert.equal(drafts.readLocal('graph:untouched'), 'Old cache');
  drafts.synchronize('graph:untouched');
  drafts.receive({owner: 'graph:untouched', requestId: sent[0].request_id, revision: 'cleared', text: ''});
  assert.equal(drafts.read('graph:untouched'), '');
  assert.equal(sent.length, 1);
});

test('main and graph composers keep draft diagnostics and errors without a saving announcement',()=>{
  for(const name of ['Composer','AgentGraphCard']) {
    const source=readFileSync(new URL(`../src/components/${name}.svelte`,import.meta.url),'utf8');
    const markup=source.slice(source.indexOf('</script>')+'</script>'.length,source.indexOf('<style>'));
    assert.doesNotMatch(markup,/Saving draft/i,name);
    assert.match(markup,/\{#if draftSaveError\}<p[^>]*role="alert">Draft not saved: \{draftSaveError\}<\/p>\{\/if\}/,name);
    assert.match(markup,/<textarea\s+data-draft-status=\{draftSaveStatus\}/,name);
    assert.equal(markup.match(/draftSaveStatus/g)?.length,1,`${name}: saving state belongs only in the diagnostic data attribute`);
  }
});

test('silent draft saving still persists main and graph text through delayed acknowledgements and failures',()=>{
  for(const owner of ['chat:a','graph:a']) {
    const c=nativeDrafts();c.drafts.read(owner);
    c.reply(c.sent[0],{revision:'initial',text:''});
    assert.equal(c.drafts.status(owner),'saved');
    c.drafts.write(owner,'First draft');
    assert.equal(c.drafts.status(owner),'saving');
    c.drafts.write(owner,'Newer draft');
    assert.equal(c.sent.length,2);
    c.reply(c.sent[1],{revision:'first'});
    assert.equal(c.drafts.status(owner),'saving');
    assert.deepEqual(c.sent[2].action,{kind:'write',revision:'first',text:'Newer draft'});
    c.reply(c.sent[2],{revision:'second'});
    assert.equal(c.drafts.status(owner),'saved');
    assert.equal(c.drafts.read(owner),'Newer draft');
    c.drafts.write(owner,'Preserve on failure');
    c.reply(c.sent[3],{error:'Could not save this draft'});
    assert.equal(c.drafts.status(owner),'error');
    assert.equal(c.drafts.error(owner),'Could not save this draft');
    assert.equal(c.drafts.read(owner),'Preserve on failure');
    assert.equal(c.updates.at(-1).owner,owner);
    assert.equal(c.updates.at(-1).error,'Could not save this draft');
  }
});

test('native draft hydration restores exact unsent text without a submission',()=>{
  const c=nativeDrafts();assert.equal(c.drafts.read('chat:a'),'');
  assert.equal(c.sent.length,1);assert.equal(c.sent[0].action.kind,'read');
  c.reply(c.sent[0],{revision:'saved',text:'Prima riga\nperché 🦀'});
  assert.equal(c.drafts.read('chat:a'),'Prima riga\nperché 🦀');
  assert.equal(c.sent.length,1);assert.equal(c.updates[0].owner,'chat:a');
});

test('typing before hydration wins and writes serialize without losing later keystrokes',()=>{
  const c=nativeDrafts();c.drafts.write('graph:a','New draft');
  const read=c.sent[0];c.reply(read,{revision:'old',text:'Older saved draft'});
  assert.equal(c.drafts.read('graph:a'),'New draft');
  assert.deepEqual(c.sent[1].action,{kind:'write',revision:'old',text:'New draft'});
  c.drafts.write('graph:a','Newer typing');assert.equal(c.sent.length,2);
  c.reply(c.sent[1],{revision:'one'});
  assert.deepEqual(c.sent[2].action,{kind:'write',revision:'one',text:'Newer typing'});
  c.reply(c.sent[2],{revision:'two'});assert.equal(c.sent.length,3);
  c.reply(read,{revision:'stale',text:'Stale'});assert.equal(c.drafts.read('graph:a'),'Newer typing');
});

test('foreign replies and failures cannot overwrite or clear local text',()=>{
  const c=nativeDrafts();c.drafts.write('chat:a','Keep');
  const read=c.sent[0];c.drafts.receive({owner:'chat:b',requestId:read.request_id,revision:'x',text:'Foreign'});
  c.reply(read,{revision:'one',text:''});const write=c.sent[1];
  c.reply(write,{error:'Changed in another window'});
  assert.equal(c.drafts.read('chat:a'),'Keep');assert.equal(c.sent.length,2);
  assert.match(c.drafts.error('chat:a'),/another window/);
  c.drafts.write('chat:a','Keep newer');assert.equal(c.sent[2].action.revision,'one');
});

test('a durable clear prevents session fallback resurrection and forgotten replies are ignored',()=>{
  const sent=[];
  const drafts=new ConversationDrafts(()=>({getItem:()=> 'Old session text',setItem(){},removeItem(){}}),'drafts',{send:r=>sent.push(r),updated(){}});
  drafts.read('chat:a');drafts.receive({owner:'chat:a',requestId:sent[0].request_id,revision:'cleared',text:''});
  assert.equal(drafts.read('chat:a'),'');assert.equal(sent.length,1);
  drafts.write('chat:a','Pending');const pending=sent[1];
  drafts.forget('chat:a','deleted');drafts.receive({owner:'chat:a',requestId:pending.request_id,revision:'late'});
  assert.equal(drafts.read('chat:a'),'');assert.equal(sent.length,2);
  drafts.write('chat:a','Recreated card');assert.equal(sent[2].action.revision,'deleted');
});
