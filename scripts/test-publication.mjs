import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { isPublishable, assertNoLinks } from './publication-files.mjs';
import { privacyFindings, assertPublicationPrivacy } from './publication-privacy.mjs';
test('publication preserves source, compiled frontend and upstream notices',()=>{
  for(const file of ['src/main.rs','ui/dist/central-agent-ui.js','.github/workflows/ci.yml','licenses/dependencies.json','assets/fonts/inter/OFL.txt','protocol/app-server/0.155.1/typescript/ClientRequest.ts','vendor/ironrdp-client/.cargo_vcs_info.json'])assert.ok(isPublishable(file),file);
});
test('publication excludes account data, build outputs and copied official docs even if tracked',()=>{
  for(const file of ['target/a.rs','outputs/Supervisor/Supervisor.exe','ui/node_modules/x/README.md','src/.env','src/.env.production','src/auth.json','docs/session.json','docs/cache.sqlite3-wal','assets/private.key','error.log','docs/vendor/openai/CODEX_APP_SERVER_OFFICIAL.md','docs/vendor/openai/agents-api/a.md','docs/brand/kit.zip','vendor/ironrdp-client/.cargo-ok'])assert.equal(isPublishable(file),false,file);
});
test('publication rejects traversal and platform path aliases',()=>{
  for(const file of ['../README.md','src/../../auth.json','/src/main.rs','C:/src/main.rs','src\\main.rs','src//main.rs','src/./main.rs','src/a\0b'])assert.equal(isPublishable(file),false,file);
});
test('publication rejects transcript, database and browser account exports',()=>{
  for(const file of ['docs/chat.jsonl','docs/cache.db-wal','assets/cookies.json','docs/sessions/a.json','docs/archived_sessions/a.json'])assert.equal(isPublishable(file),false,file);
});
test('privacy scan catches identifiers and escaped paths without exposing values',()=>{
  const id=['12345678','1234','7000','8000','123456789012'].join('-');
  const profile=['C:','Users','sample-person','Documents'].join('\\');
  for(const text of [`# Import\n${id}`,profile,JSON.stringify({path:profile})]) {
    assert.ok(privacyFindings(Buffer.from(text)).length);
    assert.throws(()=>assertPublicationPrivacy('docs/a.md',Buffer.from(text)),error=>{
      assert.ok(!error.message.includes(id));
      assert.ok(!error.message.includes('sample-person'));
      return /Publication privacy check failed/.test(error.message);
    });
  }
});
test('privacy scan allows explicit synthetic fixtures and generic documentation',()=>{
  const text='00000000-0000-7000-8000-000000000001\n%LOCALAPPDATA%\nC:/Users/<user>/Documents\nC:/Users/fixture/project';
  assert.deepEqual(privacyFindings(Buffer.from(text)),[]);
});
test('publication refuses junctions and paths outside its root',()=>{
  const temporary=fs.mkdtempSync(path.join(os.tmpdir(),'publication-test-'));
  const base=path.join(temporary,'source'),outside=path.join(temporary,'outside');
  fs.mkdirSync(base);fs.mkdirSync(outside);fs.writeFileSync(path.join(outside,'secret'),'synthetic');
  const link=path.join(base,'linked');
  try{fs.symlinkSync(outside,link,process.platform==='win32'?'junction':'dir');assert.throws(()=>assertNoLinks(base,path.join(link,'secret')),/Linked/);assert.throws(()=>assertNoLinks(base,path.join(outside,'secret')),/escapes/);}
  finally{if(fs.existsSync(link))fs.unlinkSync(link);fs.unlinkSync(path.join(outside,'secret'));fs.rmdirSync(outside);fs.rmdirSync(base);fs.rmdirSync(temporary);}
});
