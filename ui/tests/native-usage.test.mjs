import test from 'node:test';
import assert from 'node:assert/strict';
import { usageSnapshot, usagePresentation, formatUsage } from '../src/lib/native-usage.ts';
const payload = (last='1200',total='95000',capacity='16000') => ({owner:'chat:a',nativeUsage:{visible:true,connected:true,current:true,turnId:'t1',activeTurnId:'t1',report:{last:{totalTokens:last,inputTokens:'1000',cachedInputTokens:'400',outputTokens:'200',reasoningOutputTokens:'50'},total:{totalTokens:total},modelContextWindow:capacity}}});

test('context ratio uses latest native total, not lifetime usage or summed subsets',()=>{
  const view=usageSnapshot(payload(),'chat:a');
  assert.equal(usagePresentation(view).percent,'7.50%');
  assert.equal(usagePresentation(view).meter,7.5);
  assert.equal(view.report.total.totalTokens,'95000');
  assert.equal(view.report.last.cacheWriteInputTokens,null);
  assert.equal(usagePresentation(usageSnapshot(payload('200','95200'),'chat:a')).percent,'1.25%');
});
test('unknown, zero, absent capacity and unsafe integer precision remain distinct',()=>{
  assert.equal(usageSnapshot(payload(),'graph:a'),null);
  assert.equal(usageSnapshot({owner:'chat:a'},'chat:a'),null);
  assert.equal(formatUsage(null),'Not reported');
  assert.equal(formatUsage('0'),'0');
  assert.equal(formatUsage('9007199254740993'),'9,007,199,254,740,993');
  assert.equal(usagePresentation(usageSnapshot(payload('0','0',null),'chat:a')).meter,null);
  assert.equal(usagePresentation(usageSnapshot(payload('0','0'),'chat:a')).meter,0);
  for(const invalid of [0,-5,1.5,'1.5','-1','NaN']) assert.equal(usageSnapshot(payload(invalid),'chat:a').report.last.totalTokens,null);
  assert.equal(usagePresentation(usageSnapshot(payload('20000','90000'),'chat:a')).percent,'125%');
  assert.equal(usagePresentation(usageSnapshot(payload('20000','90000'),'chat:a')).meter,100);
});
test('disconnect, reconnect and a new turn do not relabel old numbers as live',()=>{
  const data=payload(); data.nativeUsage.connected=false; data.nativeUsage.current=false;
  let shown=usagePresentation(usageSnapshot(data,'chat:a'));
  assert.equal(shown.stale,true); assert.match(shown.status,/Disconnected/);
  data.nativeUsage.connected=true;
  assert.match(usagePresentation(usageSnapshot(data,'chat:a')).status,/Previous connection/);
  data.nativeUsage.current=true; data.nativeUsage.activeTurnId='t2';
  assert.match(usagePresentation(usageSnapshot(data,'chat:a')).status,/Previous turn/);
  data.nativeUsage.turnId='t2';
  assert.equal(usagePresentation(usageSnapshot(data,'chat:a')).stale,false);
  data.nativeUsage.report=null;
  assert.match(usagePresentation(usageSnapshot(data,'chat:a')).summary,/Waiting/);
});
