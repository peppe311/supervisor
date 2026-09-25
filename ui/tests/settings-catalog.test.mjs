import assert from 'node:assert/strict';
import test from 'node:test';
import {resolveSettingsId,settingsCategories,settingsGroups} from '../src/lib/settings-catalog.ts';

test('settings expose the seven focused user-facing categories',()=>{
  assert.deepEqual(settingsCategories.map(category=>category.id),[
    'settings-general','settings-ai','settings-agent','settings-codex-tools',
    'settings-workspace','settings-servers','settings-system'
  ]);
  assert.equal(resolveSettingsId('settings-access'),'settings-agent');
  assert.equal(resolveSettingsId('settings-privacy'),null);
  assert.equal(resolveSettingsId('settings-advanced'),null);
  assert.deepEqual(settingsGroups.map(group=>[group.title,...group.categories]),[
    ['Personal','settings-general'],
    ['AI','settings-ai','settings-agent','settings-codex-tools'],
    ['Workspace','settings-workspace'],
    ['Connections','settings-servers','settings-system']
  ]);
  assert.deepEqual(settingsGroups.flatMap(group=>group.categories),settingsCategories.map(category=>category.id));
});
