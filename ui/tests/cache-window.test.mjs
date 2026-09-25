import test from 'node:test';
import assert from 'node:assert/strict';
import { usageSnapshot } from '../src/lib/native-usage.ts';
import { cacheWindowEstimate } from '../src/lib/cache-window.ts';

const observed = 1_800_000_000_000;
const payload = () => ({ owner:'chat:a', nativeUsage:{
  visible:true, connected:true, current:true, turnId:'turn-a', activeTurnId:'turn-a',
  model:'gpt-6-sol', cacheReportAtMs:observed,
  report:{ last:{cachedInputTokens:'1024',cacheWriteInputTokens:'0'}, total:{}, modelContextWindow:null },
} });

test('a fresh native cache report produces an explicitly estimated 30-minute window', () => {
  const view = usageSnapshot(payload(), 'chat:a');
  assert.equal(cacheWindowEstimate(view, observed)?.label, '30:00');
  assert.equal(cacheWindowEstimate(view, observed + 61_000)?.label, '28:59');
  assert.equal(cacheWindowEstimate(view, observed + 1_800_000)?.label, '30m+');
  assert.equal(cacheWindowEstimate(view, observed + 1_800_000)?.elapsed, true);
  assert.match(cacheWindowEstimate(view, observed)?.description ?? '', /does not report the cache expiry/);
});

test('cache-write activity qualifies, but reports with no cache activity do not', () => {
  const data = payload();
  data.nativeUsage.report.last.cachedInputTokens = '0';
  data.nativeUsage.report.last.cacheWriteInputTokens = '99999999999999999999';
  assert.equal(cacheWindowEstimate(usageSnapshot(data, 'chat:a'), observed)?.label, '30:00');
  data.nativeUsage.report.last.cacheWriteInputTokens = '0';
  assert.equal(cacheWindowEstimate(usageSnapshot(data, 'chat:a'), observed), null);
});

test('unknown retention, stale or another conversation never receive a countdown', () => {
  const data = payload();
  assert.equal(usageSnapshot(data, 'chat:b'), null);
  for (const model of ['gpt-5.5', 'gpt-5.2', 'o3', null]) {
    data.nativeUsage.model = model;
    assert.equal(cacheWindowEstimate(usageSnapshot(data, 'chat:a'), observed), null);
  }
  data.nativeUsage.model = 'gpt-5.6-sol';
  assert.equal(cacheWindowEstimate(usageSnapshot(data, 'chat:a'), observed)?.label, '30:00');
  data.nativeUsage.current = false;
  assert.equal(cacheWindowEstimate(usageSnapshot(data, 'chat:a'), observed), null);
  data.nativeUsage.current = true;
  data.nativeUsage.connected = false;
  assert.equal(cacheWindowEstimate(usageSnapshot(data, 'chat:a'), observed), null);
  data.nativeUsage.connected = true;
  data.nativeUsage.cacheReportAtMs = null;
  assert.equal(cacheWindowEstimate(usageSnapshot(data, 'chat:a'), observed), null);
  data.nativeUsage.cacheReportAtMs = observed;
  assert.equal(cacheWindowEstimate(usageSnapshot(data, 'chat:a'), observed - 1), null);
});
