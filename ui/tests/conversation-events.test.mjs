import test from 'node:test';
import assert from 'node:assert/strict';
import { bridgeConversationEvents, graphDraft, retainGraphDraft, acknowledgeGraphDraft, graphDeliveryIntent } from '../src/lib/conversation-events.ts';

test('provider-neutral graph replies reach only their owning card', () => {
  const source = new EventTarget(), a = new EventTarget(), b = new EventTarget();
  const receivedA = [], receivedB = [];
  a.addEventListener('central-agent:conversation-error', e => receivedA.push(e.detail));
  b.addEventListener('central-agent:conversation-error', e => receivedB.push(e.detail));
  const stopA = bridgeConversationEvents(source, a, 'graph:a');
  const stopB = bridgeConversationEvents(source, b, 'graph:b');
  source.dispatchEvent(new CustomEvent('central-agent:conversation-error', {detail:{owner:'graph:a',error:'Rejected'}}));
  assert.equal(receivedA.length, 1); assert.equal(receivedB.length, 0);
  stopA(); stopB();
  source.dispatchEvent(new CustomEvent('central-agent:conversation-error', {detail:{owner:'graph:a'}}));
  assert.equal(receivedA.length, 1);
});

test('acceptance preserves another card and newer draft text', () => {
  retainGraphDraft('graph:a', 'First request'); retainGraphDraft('graph:b', 'Other request');
  acknowledgeGraphDraft({owner:'graph:a',input:'Older request'});
  assert.equal(graphDraft('graph:a'),'First request');
  acknowledgeGraphDraft({owner:'graph:a',input:'First request'});
  assert.equal(graphDraft('graph:a'),''); assert.equal(graphDraft('graph:b'),'Other request');
});

test('remaining adapters queue followups and never advertise steering', () => {
  assert.equal(graphDeliveryIntent(true,false,'start'),'choose');
  assert.equal(graphDeliveryIntent(true,false,'steer'),'queue');
  assert.equal(graphDeliveryIntent(false,false,'start'),'start');
});

test('native App Server snapshots do not cross graph-card owners', () => {
  const source = new EventTarget(), a = new EventTarget(), b = new EventTarget();
  const seenA = [], seenB = [];
  a.addEventListener('central-agent:app-server-conversation', event => seenA.push(event.detail));
  b.addEventListener('central-agent:app-server-conversation', event => seenB.push(event.detail));
  const stopA = bridgeConversationEvents(source,a,'graph:a'), stopB = bridgeConversationEvents(source,b,'graph:b');
  source.dispatchEvent(new CustomEvent('central-agent:app-server-conversation', {detail:{owner:'graph:b',thread:{id:'native-b'}}}));
  source.dispatchEvent(new CustomEvent('central-agent:app-server-conversation', {detail:{owner:'chat:a',thread:{id:'main-thread'}}}));
  assert.equal(seenA.length,0); assert.equal(seenB.length,1);
  assert.equal(seenB[0].thread.id,'native-b');
  stopA(); stopB();
});
