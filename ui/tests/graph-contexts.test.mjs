import test from 'node:test';
import assert from 'node:assert/strict';
import { contextDraft, contextIds, droppedContext, safeSnapshotImage } from '../src/lib/graph-contexts.ts';
import { acceptsConversationEvent, conversationReplyEvents } from '../src/lib/conversation-events.ts';
test('graph drafts submit exact tab and shell IDs and allow attachment-only input',()=>{
  const draft=contextDraft({tabs:[{tabId:7}],shells:[{sessionId:9}]});
  assert.deepEqual(contextIds(draft),{tabIds:[7],shellIds:[9]});
  assert.deepEqual(contextIds(contextDraft(null)),{tabIds:[],shellIds:[]});
});
test('drag payloads accept only typed positive safe IDs and reject arbitrary URLs, commands and paths',()=>{
  const parse=values=>droppedContext({getData:key=>values[key] || ''});
  assert.deepEqual(parse({'application/x-central-agent-tab':'7'}),{kind:'tab',id:7});
  assert.deepEqual(parse({'application/x-central-agent-terminal-context':'9'}),{kind:'shell',id:9});
  assert.deepEqual(parse({'text/plain':'central-agent-shell:9'}),{kind:'shell',id:9});
  for(const value of ['https://example.com','C:/secret.txt','central-agent-tab:1;whoami','central-agent-tab:-1','central-agent-tab:0','central-agent-tab:9007199254740992']) assert.equal(parse({'text/plain':value}),null);
});
test('snapshot replies are graph-owner scoped and page previews never fetch a remote URL',()=>{
  assert.ok(conversationReplyEvents.includes('central-agent:graph-contexts-result'));
  assert.equal(acceptsConversationEvent('central-agent:graph-contexts-result',{owner:'graph:a'},'graph:b'),false);
  assert.equal(acceptsConversationEvent('central-agent:graph-contexts-result',{owner:'graph:a'},'graph:a'),true);
  assert.equal(safeSnapshotImage('data:image/jpeg;base64,YWJj'),'data:image/jpeg;base64,YWJj');
  for(const value of ['https://example.com/a.jpg','file:///secret','data:image/svg+xml,<svg/>','javascript:alert(1)']) assert.equal(safeSnapshotImage(value),undefined);
});
