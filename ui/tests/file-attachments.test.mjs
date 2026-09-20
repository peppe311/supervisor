import test from 'node:test';
import assert from 'node:assert/strict';
import { attachmentList, attachmentPreview } from '../src/lib/file-attachments.ts';

test('attachment previews never load remote files or executable URI content', () => {
  for (const value of [undefined, null, '', 'https://example.test/image.png','file:///C:/secret','javascript:alert(1)','data:image/svg+xml;base64,AAAA','data:text/html;base64,AAAA']) assert.equal(attachmentPreview(value), undefined);
  assert.equal(attachmentPreview('data:image/png;base64,AAAA'), 'data:image/png;base64,AAAA');
  assert.equal(attachmentPreview('data:image/jpeg;base64,AAAA'), 'data:image/jpeg;base64,AAAA');
});

test('stored file metadata retains all valid identities while legacy invalid entries stay noninteractive', () => {
  const a = {id:'a',name:'Cargo.toml',kind:'text',iconKey:'cargo'}, b = {id:'b',name:'reference.png',kind:'image'}, c = {id:'c',name:'sample.wav',kind:'audio'};
  assert.deepEqual(attachmentList([a, null, {}, {id:'foreign',name:'x',kind:'script'}, b, c]), [a,b,c]);
  assert.deepEqual(attachmentList(null), []);
  assert.deepEqual(attachmentList({files:[a]}), []);
});
