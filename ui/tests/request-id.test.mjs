import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import ts from 'typescript';

const source=fs.readFileSync(new URL('../src/lib/request-id.ts',import.meta.url),'utf8');
const compiled=ts.transpileModule(source,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.CommonJS}}).outputText;
test('correlation UUIDs work in the non-secure WebView host without randomUUID',()=>{
  let calls=0;
  const ctx=vm.createContext({exports:{},crypto:{getRandomValues(bytes){calls++;return crypto.getRandomValues(bytes);}}});
  vm.runInContext(compiled,ctx);
  const ids=Array.from({length:1000},()=>ctx.exports.requestId());
  assert.equal(calls,1000);assert.equal(new Set(ids).size,1000);
  for(const id of ids)assert.match(id,/^[\da-f]{8}-[\da-f]{4}-4[\da-f]{3}-[89ab][\da-f]{3}-[\da-f]{12}$/);
});
test('missing cryptographic entropy fails instead of falling back to Math.random',()=>{
  const ctx=vm.createContext({exports:{},crypto:{}});vm.runInContext(compiled,ctx);
  assert.throws(()=>ctx.exports.requestId(),/getRandomValues/);
});
