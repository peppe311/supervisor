import assert from 'node:assert/strict';
import test from 'node:test';
import {resolveSettingsId,settingsCategories,settingsGroups,searchSettings} from '../src/lib/settings-catalog.ts';

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

test('search distinguishes Codex execution access from Supervisor tool confirmations',()=>{
  assert(searchSettings('codex permissions').some(result=>result.section==='settings-agent' && result.selector==="#codex-workspace-access-select-picker-button"));
  assert(searchSettings('action approval').some(result=>result.section==='settings-agent' && result.selector==='#supervisor-permission-select-picker-button'));
});

test('search routes Codex Skills and Plugins to their dedicated category',()=>{
  const skills=searchSettings('codex skills').find(result=>result.title==='Codex Skills');
  const plugins=searchSettings('codex plugins').find(result=>result.title==='Plugins');
  assert.equal(skills?.section,'settings-codex-tools');
  assert.equal(plugins?.section,'settings-codex-tools');
});

test('search is accent-insensitive and indexes only useful controls',()=>{
  const titles=query=>searchSettings(query).map(result=>result.title);
  assert.deepEqual(titles('  CoDeX    PERMISSIONS  '),titles('permissions codex'));
  assert.deepEqual(titles('Mémory'),[]);
  assert.deepEqual(titles(''),[]);
  for(const query of ['device code','reload chats','repair codex windows','remote desktop','personality'])assert(searchSettings(query).length,query);
  for(const removed of ['protocol diagnostics','feature flags','account token activity','workspace messages','mcp servers'])assert.deepEqual(searchSettings(removed),[],removed);
});

test('every visible category has at least one searchable control',()=>{
  for(const category of settingsCategories)assert(searchSettings(category.title).some(result=>result.section===category.id),category.title);
});
