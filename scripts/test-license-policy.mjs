import test from 'node:test';
import assert from 'node:assert/strict';
import {selectLicense} from './license-policy.mjs';
test('license alternatives do not inherit rejected obligations',()=>{
  assert.equal(selectLicense('GPL-3.0-only OR MIT'),'MIT');
  assert.equal(selectLicense('MIT/Apache-2.0'),'MIT');
  assert.equal(selectLicense('(MIT OR Apache-2.0) AND Unicode-3.0'),'MIT AND Unicode-3.0');
  assert.equal(selectLicense('Apache-2.0 WITH LLVM-exception'),'Apache-2.0 WITH LLVM-exception');
});
test('unknown mandatory licenses, malformed expressions and empty metadata fail closed',()=>{
  for(const value of ['MIT AND GPL-3.0-only','UNLICENSED','MIT OR','MIT Apache-2.0','(MIT','MIT WITH unknown','','MIT; Apache-2.0'])assert.throws(()=>selectLicense(value),undefined,value);
});
