import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import {compile} from "svelte/compiler";
import {render} from "svelte/server";
import {appAvailability,appIconKind,appInitial,appMatches,appSelectable} from "../src/lib/native-apps.ts";

const command={name:'list_issues',title:'List issues',description:'Read repository issues',enabled:true,disabledReason:null,readOnly:true};
const app={id:'github',name:'GitHub',description:'Repositories and issues',iconUrl:'https://files.openai.com/github.png',accessible:true,configuredEnabled:true,installed:true,runtimeEnabled:true,callable:true,tools:[command]};

test("plugin availability and selection require every native policy and runtime gate",()=>{
  assert.equal(appAvailability(app),'Available for an explicit mention');
  assert.equal(appSelectable(app),true);
  for(const [field,label] of [
    ['accessible','Unavailable for this account or workspace'],['configuredEnabled','Disabled in Codex'],
    ['installed','Not installed'],['runtimeEnabled','Disabled in the Codex runtime'],
    ['callable','Unavailable for an explicit mention'],
  ]) {
    const unavailable={...app,[field]:false};
    assert.equal(appAvailability(unavailable),label);
    assert.equal(appSelectable(unavailable),false);
  }
});

test("plugin search includes plugin names and descriptions",()=>{
  assert.equal(appMatches(app,'github'),true);
  assert.equal(appMatches(app,'repositories'),true);
  assert.equal(appMatches(app,'calendar'),false);
});

test("plugin identity uses local product marks and a deterministic fallback",()=>{
  assert.equal(appIconKind('browser@openai-bundled','Browser'),'browser');
  assert.equal(appIconKind('github','GitHub'),'github');
  assert.equal(appIconKind('other','GitHub Issues'),'github');
  assert.equal(appIconKind('cloudflare@openai-curated-remote','Cloudflare'),'cloudflare');
  assert.equal(appIconKind('figma','Figma'),'figma');
  assert.equal(appIconKind('linear','Linear'),'linear');
  assert.equal(appIconKind('notion','Notion'),'notion');
  assert.equal(appIconKind('calendar','Calendar'),'generic');
  assert.equal(appInitial(' Calendar'),'C');
  assert.equal(appInitial(''),'?');
});

async function renderNativeApps(view){
  const iconSource=fs.readFileSync(new URL('../src/components/PluginIcon.svelte',import.meta.url),'utf8');
  let iconCode=compile(iconSource,{filename:'PluginIcon.svelte',generate:'server'}).js.code;
  iconCode=iconCode.replace(/from (["'])([^"']+)\1/g,(whole,quote,specifier)=>`from ${JSON.stringify(specifier==='../lib/native-apps'?new URL('../src/lib/native-apps.ts',import.meta.url).href:import.meta.resolve(specifier))}`);
  const iconModule=`data:text/javascript;base64,${Buffer.from(iconCode).toString('base64')}`;
  const source=fs.readFileSync(new URL('../src/components/NativeApps.svelte',import.meta.url),'utf8');
  let code=compile(source,{filename:'NativeApps.svelte',generate:'server'}).js.code;
  code=code.replace(/from (["'])([^"']+)\1/g,(whole,quote,specifier)=>`from ${JSON.stringify(specifier==='../lib/native-apps'?new URL('../src/lib/native-apps.ts',import.meta.url).href:specifier==='./PluginIcon.svelte'?iconModule:import.meta.resolve(specifier))}`);
  const {default:Component}=await import(`data:text/javascript;base64,${Buffer.from(code).toString('base64')}`);
  return render(Component,{props:{view,connected:true,onAction:()=>{}}}).body;
}

test("Settings inspect installed user-facing plugins without duplicating composer selection",async()=>{
  const view={threadId:'thread-a',inventoryId:'inventory-a',items:[app],selected:[{selectionId:'selection-a',appId:'github',name:'GitHub',threadId:'thread-a',current:true}],current:true,busy:false,error:null,unavailableReason:null,notice:null};
  const html=await renderNativeApps(view);
  assert.match(html,/>Plugins</);
  assert.match(html,/user-facing plugins installed in Codex/);
  assert.match(html,/data-plugin-icon="github"/);
  assert.doesNotMatch(html,/Commands \(1\)|Read only|May write/);
  assert.doesNotMatch(html,/Mention next/);
  assert.doesNotMatch(html,/\$github/);
});

test("composer plugin picker is shared by main chat and graph without entering the profile popup",()=>{
  const picker=fs.readFileSync(new URL('../src/components/PluginPicker.svelte',import.meta.url),'utf8');
  const icon=fs.readFileSync(new URL('../src/components/PluginIcon.svelte',import.meta.url),'utf8');
  const composer=fs.readFileSync(new URL('../src/components/Composer.svelte',import.meta.url),'utf8');
  const graph=fs.readFileSync(new URL('../src/components/AgentGraphCard.svelte',import.meta.url),'utf8');
  const profile=fs.readFileSync(new URL('../src/components/AgentConfiguration.svelte',import.meta.url),'utf8');
  assert.doesNotThrow(()=>compile(picker,{filename:'PluginPicker.svelte',generate:'server'}));
  assert.doesNotThrow(()=>compile(icon,{filename:'PluginIcon.svelte',generate:'server'}));
  assert.match(picker,/selected-plugin-chip/);
  assert.match(picker,/selected-plugin-remove/);
  assert.match(picker,/<PluginIcon/);
  assert.doesNotMatch(picker,/data-plugin-command|Read only|May write|app\.description|appAvailability/);
  assert.match(icon,/data-plugin-icon/);
  assert.match(icon,/kind==="browser"/);
  assert.match(icon,/kind==="github"/);
  assert.match(icon,/kind==="cloudflare"/);
  assert.match(icon,/kind==="figma"/);
  assert.match(icon,/kind==="linear"/);
  assert.match(icon,/kind==="notion"/);
  assert.match(icon,/data-plugin-icon-source/);
  assert.match(icon,/<img src=/);
  assert.match(picker,/kind:\s*["']apps_control["']/);
  assert.match(picker,/attach_official_browser/);
  assert.match(composer,/<PluginPicker\s*\/>/);
  assert.match(graph,/<PluginPicker \{eventTarget\} \{owner\} compact \/>/);
  assert.doesNotMatch(profile,/PluginPicker/);
});

test("plugin settings expose a text filter without rendering native thread IDs",()=>{
  const source=fs.readFileSync(new URL('../src/components/NativeApps.svelte',import.meta.url),'utf8');
  const picker=fs.readFileSync(new URL('../src/components/PluginPicker.svelte',import.meta.url),'utf8');
  assert.match(source,/Filter plugins/);
  assert.doesNotMatch(picker,/type="search"|Find a plugin|Find a command/);
  assert.doesNotMatch(source,/Native thread:/);
  assert.doesNotMatch(picker,/Native thread:/);
});

test("plugin access denial is a terminal availability state without stale refresh copy",async()=>{
  const reason='Codex plugins are unavailable for this ChatGPT account or workspace.';
  const view={threadId:'thread-a',inventoryId:null,items:[],selected:[],current:false,busy:false,error:null,unavailableReason:reason,notice:null};
  const html=await renderNativeApps(view);
  assert.match(html,/Codex plugins are unavailable/);
  assert.doesNotMatch(html,/Load plugins after resuming/);
  assert.doesNotMatch(html,/snapshot will be verified/);
  assert.doesNotMatch(html,/No plugins were reported/);
});
