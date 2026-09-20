import test from 'node:test';
import assert from 'node:assert/strict';
import {chatForest,chatRelationship,chatAge} from '../src/lib/chat-tree.ts';

const chat=(id,parent=null,patch={})=>({id,title:id,projectRoot:'local',lineage:parent?{
  kind:'fork',parentChatId:parent,parentTitle:parent,parentAvailable:true,childCount:0}:null,...patch});
test('forks follow their real parent even when recency puts children first',()=>{
  const input=[chat('grandchild','child'),chat('child','source'),chat('unrelated'),chat('source')];
  const original=JSON.stringify(input);
  assert.deepEqual(chatForest(input).map(row=>[row.chat.id,row.depth]),[['unrelated',0],['source',0],['child',1],['grandchild',2]]);
  assert.equal(JSON.stringify(input),original);
});
test('same names and same project never imply ancestry',()=>{
  const rows=chatForest([chat('one',null,{title:'Original'}),chat('two',null,{title:'Original'})]);
  assert.ok(rows.every(row=>row.depth===0));assert.equal(chatRelationship(rows[1].chat),'');
});
test('missing, filtered and cross-project parents retain a labelled fork',()=>{
  for(const input of [[chat('child','missing')],[chat('child','source'),chat('source',null,{projectRoot:'other'})]]) {
    assert.equal(chatForest(input)[0].chat.id,'child');assert.equal(chatForest(input)[0].depth,0);
    assert.match(chatRelationship(input[0]),/^Fork ·/);
  }
});
test('collapse affects only descendants, not another root or source data',()=>{
  const input=[chat('child','source'),chat('other'),chat('source')];
  assert.deepEqual(chatForest(input,new Set(['source'])).map(row=>row.chat.id),['other','source']);
  assert.equal(chatForest(input).length,3);
});
test('cycles and self-links cannot lose chats or loop',()=>{
  const rows=chatForest([chat('a','b'),chat('b','a'),chat('self','self'),chat('child','a')]);
  assert.equal(rows.length,4);assert.equal(new Set(rows.map(row=>row.chat.id)).size,4);
});
test('pending forks cannot be labelled completed and a fork may own forks',()=>{
  assert.match(chatRelationship(chat('pending','source',{lineage:{kind:'pending',parentTitle:'Source',childCount:0}})),/unconfirmed/);
  assert.equal(chatRelationship(chat('a','source',{lineage:{kind:'fork',parentTitle:'Source',childCount:2}})),'Fork · Source · 2 forks');
  assert.equal(chatRelationship(chat('source',null,{lineage:{kind:'source',childCount:1}})),'Original · 1 fork');
});
test('working and attention remain explicit rather than becoming a timestamp',()=>{
  assert.equal(chatAge(chat('a',null,{agentActive:true})),'Working');
  assert.equal(chatAge(chat('a',null,{agentActive:true,needsAttention:true})),'Needs attention');
});
