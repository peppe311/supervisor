import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import {compile} from "svelte/compiler";
import {render} from "svelte/server";

test("native hooks remain a read-only, escaped inventory and activity view",async()=>{
  const source=fs.readFileSync(new URL('../src/components/NativeHooks.svelte',import.meta.url),'utf8');
  let code=compile(source,{filename:'NativeHooks.svelte',generate:'server'}).js.code;
  code=code.replace(/from (["'])([^"']+)\1/g,(whole,quote,specifier)=>`from ${JSON.stringify(import.meta.resolve(specifier))}`);
  const {default:Component}=await import(`data:text/javascript;base64,${Buffer.from(code).toString('base64')}`);
  const view={threadId:'thread-a',directory:'C:\\fixture',viewId:'view-a',current:true,busy:false,error:null,notice:null,warnings:[],errors:[],
    hooks:[{key:'hook-a',eventName:'preToolUse',matcher:null,statusMessage:'Checking',sourcePath:'C:\\fixture\\hook.toml',source:'project',handlerType:'command',enabled:true,managed:false,trustStatus:'trusted'}],
    runs:[{id:'run-a',eventName:'preToolUse',handlerType:'command',executionMode:'sync',scope:'turn',source:'project',status:'completed',statusMessage:null,startedAt:'1',completedAt:'2',durationMs:'1',entries:[{kind:'feedback',text:'<script>unsafe()</script>'}]}]};
  const html=render(Component,{props:{view,connected:true,onRefresh:()=>{}}}).body;
  assert.match(html,/Refresh hooks/);assert.match(html,/preToolUse/);assert.match(html,/&lt;script>unsafe\(\)&lt;\/script>/);
  assert.doesNotMatch(html,/Run hook|Retry hook|Edit hook|fixture-command|currentHash/);
});
