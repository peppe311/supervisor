import test from 'node:test';
import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';

test('isolated MCP fixture sends valid ordered progress with the exact requested token', () => {
  const fixture=fileURLToPath(new URL('../../crates/central-agent-codex-runtime/tests/elicitation-mcp.mjs',import.meta.url));
  for(const token of [0,'native-token',undefined]) {
    const input=[
      {jsonrpc:'2.0',id:1,method:'tools/call',params:{name:'fixture_question',arguments:{mode:'form'},...(token===undefined?{}:{_meta:{progressToken:token}})}},
      {jsonrpc:'2.0',id:'fixture-1',result:{action:'accept',content:{label:'test',enabled:false,count:3}}},
    ].map(value=>JSON.stringify(value)).join('\n')+'\n';
    const output=execFileSync(process.execPath,[fixture],{input,encoding:'utf8',timeout:10_000});
    const frames=output.trim().split('\n').map(line=>JSON.parse(line));
    const progress=frames.filter(frame=>frame.method==='notifications/progress');
    assert.equal(progress.length,token===undefined?0:2);
    for(const [index,frame] of progress.entries()) {
      assert.equal(frame.jsonrpc,'2.0');
      assert.equal(frame.params.progressToken,token);
      assert.equal(frame.params.progress,index+1);
      assert.equal(frame.params.total,2);
      assert.equal(frame.params.message,index?'Fixture received decision':'Fixture waiting for decision');
    }
    assert.equal(frames.at(-1).id,1);
    assert.equal(frames.at(-1).result.structuredContent.fixtureProgressTokenPresent,token!==undefined);
    assert.equal(frames.at(-1).result.structuredContent.action,'accept');
  }
});
