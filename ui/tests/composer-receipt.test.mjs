import test from 'node:test';
import assert from 'node:assert/strict';
import { acceptsComposerReceipt } from '../src/lib/composer-receipt.ts';

test('acknowledgement is scoped to its draft conversation', () => {
  const receipt = { owner: 'chat:a', input: 'Continue' };
  assert.equal(acceptsComposerReceipt(receipt, 'chat:a', 'Continue'), true);
  assert.equal(acceptsComposerReceipt(receipt, 'chat:b', 'Continue'), false);
  assert.equal(acceptsComposerReceipt(receipt, 'graph:a', 'Continue'), false);
});

test('new edits survive acknowledgement of the previously submitted text', () => {
  const receipt = { owner: 'chat:a', input: 'Original prompt' };
  assert.equal(acceptsComposerReceipt(receipt, 'chat:a', 'New instruction'), false);
  assert.equal(acceptsComposerReceipt(receipt, 'chat:a', '  Original prompt\n'), true);
});

test('draft-to-chat transition cannot clear the newly loaded draft', () => {
  const receipt = { owner: 'draft:project', input: 'Hello' };
  assert.equal(acceptsComposerReceipt(receipt, 'draft:project', 'Hello'), true);
  assert.equal(acceptsComposerReceipt(receipt, 'chat:created', 'Hello'), false);
});

test('legacy, malformed and unbound acknowledgements fail closed', () => {
  for (const receipt of [null, undefined, 'Hello', {}, { input: 'Hello' }, { owner: 'chat:a', input: null }]) {
    assert.equal(acceptsComposerReceipt(receipt, 'chat:a', 'Hello'), false);
  }
  assert.equal(acceptsComposerReceipt({ owner: '', input: 'Hello' }, '', 'Hello'), false);
});
