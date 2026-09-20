import { test } from 'node:test';
import assert from 'node:assert/strict';
import { groupedGitEntries, matchesProjectDiffReply } from '../src/lib/project-diff.ts';

test('all paths remain grouped by Git state without counting a file twice in the source list', () => {
  const entries = [{path:'src/main.rs',sections:['staged','unstaged'],iconKey:'rust',blocked:null}, {path:'.env',sections:['untracked'],iconKey:'file',blocked:'Secret'}];
  const groups = groupedGitEntries(entries);
  assert.deepEqual(groups.map(group => group.entries.length), [1,1,1]);
  assert.equal(groups[2].entries[0].blocked, 'Secret');
  assert.deepEqual(groupedGitEntries(entries,'MAIN').map(group => group.entries.length), [1,1,0]);
  assert.equal(entries.length, 2);
  const many = Array.from({length:2500}, (_, i) => ({path:`file-${i}.rs`,sections:['unstaged']}));
  assert.equal(groupedGitEntries(many)[1].entries.length, 2500);
});

test('late diff reads cannot replace another file, project view, or conversation', () => {
  const reply = {owner:'chat:a',viewId:'diff:1',requestId:'query:1'};
  assert.ok(matchesProjectDiffReply(reply,'chat:a','diff:1','query:1',true));
  for (const args of [['chat:b','diff:1','query:1',true],['chat:a','diff:2','query:1',true],['chat:a','diff:1','query:2',true],['chat:a','diff:1','query:1',false],['chat:a','diff:1','',true]]) {
    assert.equal(matchesProjectDiffReply(reply,...args),false);
  }
});

test('graph diff responses cannot target the main chat, another branch or a reopened dialog', () => {
  const reply = {owner:'graph:conversation:one',viewId:'graph-input-one:diff:uuid',requestId:'query:2'};
  assert.equal(matchesProjectDiffReply(reply,reply.owner,reply.viewId,reply.requestId,true),true);
  for (const owner of ['chat:main','graph:entity:original','graph:conversation:two']) {
    assert.equal(matchesProjectDiffReply(reply,owner,reply.viewId,reply.requestId,true),false);
  }
  assert.equal(matchesProjectDiffReply(reply,reply.owner,'reopened',reply.requestId,true),false);
  assert.equal(matchesProjectDiffReply(reply,reply.owner,reply.viewId,'earlier',true),false);
  assert.equal(matchesProjectDiffReply(reply,reply.owner,reply.viewId,reply.requestId,false),false);
});
