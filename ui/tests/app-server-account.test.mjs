import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import {compile} from "svelte/compiler";
import {render} from "svelte/server";
import {accountStatus,canAuthenticate,emptyAppServerAccount,rateLimitWindowText,runtimeNoticeLines,runtimeNoticeTitle} from "../src/lib/app-server-account.ts";

async function renderAccount(view) {
  const source=fs.readFileSync(new URL('../src/components/AppServerAccount.svelte',import.meta.url),'utf8');
  let code=compile(source.replace('$state(emptyAppServerAccount())',`$state(${JSON.stringify(view)})`),{filename:'AppServerAccount.svelte',generate:'server'}).js.code;
  code=code.replace(/from (["'])([^"']+)\1/g,(whole,quote,specifier)=>`from ${JSON.stringify(specifier==='../lib/app-server-account'?new URL('../src/lib/app-server-account.ts',import.meta.url).href:import.meta.resolve(specifier))}`);
  const {default:Account}=await import(`data:text/javascript;base64,${Buffer.from(code).toString('base64')}`);
  const deny=()=>{throw new Error('Rendering account state must not send intent');};
  return render(Account,{props:{onAction:deny}}).body;
}

test("connection does not imply authentication or inferred subscription access",()=>{
  const view=emptyAppServerAccount();
  assert.equal(accountStatus(view),"Not connected");
  assert.equal(canAuthenticate(view),false);
  view.connected=true;
  assert.equal(accountStatus(view),"Sign-in required");
  view.account={type:"apiKey",email:null,planType:null,supported:false,policy:"Subscription only"};
  assert.equal(accountStatus(view),"API key detected · unsupported");
  view.account={type:"chatgpt",email:null,planType:"pro",supported:true,policy:null};
  assert.equal(accountStatus(view),"ChatGPT connected");
});

test("pending login and refresh cannot launch duplicate auth flows",()=>{
  const view={...emptyAppServerAccount(),connected:true};
  assert.equal(canAuthenticate(view),true);
  for(const field of ["authBusy","refreshing","connecting"])assert.equal(canAuthenticate({...view,[field]:true}),false);
  assert.equal(canAuthenticate({...view,login:{url:"https://auth.openai.com/codex/device",userCode:"TEST"}}),false);
  assert.equal(accountStatus({...view,refreshing:true}),"Refreshing…");
});

test("saved chat reload remains optional and reports partial recovery honestly",async()=>{
  const fresh=emptyAppServerAccount();
  const idle=await renderAccount(fresh);
  assert.match(idle,/Saved chats load automatically/);
  assert.match(idle,/<button[^>]*>Reload chats/);
  const loading=await renderAccount({...fresh,chatReload:{loading:true,checked:true,total:3,loaded:1,unavailable:0,skipped:0}});
  assert.match(loading,/<button[^>]*disabled[^>]*>Loading chats/);
  assert.match(loading,/Loading saved chats… 1 \/ 3/);
  const partial=await renderAccount({...fresh,chatReload:{loading:false,checked:true,total:3,loaded:1,unavailable:1,skipped:1}});
  assert.match(partial,/1 chat loaded\./);
  assert.match(partial,/1 history unavailable\. Existing links were kept\./);
  assert.match(partial,/1 conversation left unchanged\./);
});

test("the account surface keeps only user-facing controls and repair guidance",async()=>{
  const view={...emptyAppServerAccount(),connected:true,requirementsLoaded:true,account:{type:'chatgpt',email:'person@example.test',planType:'pro',supported:true,policy:null},rateLimits:{refreshing:false,loaded:true,current:true,availableResetCredits:null,buckets:[{id:'codex',name:'Codex',primary:{usedPercent:31.5,windowDurationMins:15,resetsAt:0},secondary:null}]}};
  const html=await renderAccount(view);
  for(const expected of ['person@example.test · pro','Subscription usage &amp; limits','Saved chats','Account options','Repair Codex on Windows','31.5% used · 15 min window'])assert.match(html,new RegExp(expected));
  for(const removed of ['Advanced Codex settings','Account token activity','Workspace messages','Available model profiles','Provider capabilities','Managed permission profiles','Protocol diagnostics','Codex MCP servers','Optional Codex features'])assert.doesNotMatch(html,new RegExp(removed));
});

test("errors and public runtime notices remain visible and escaped",async()=>{
  const view={...emptyAppServerAccount(),errors:{connection:'Codex is temporarily unavailable'},runtimeNotices:[
    {sequence:1,current:true,value:{kind:'config_warning',summary:'Unsafe <script>',details:null,path:'C:\\config.toml',line:null,column:null,raw:'SECRET_NOTICE'}},
    {sequence:2,current:false,value:{kind:'windows_world_writable',samplePaths:['C:\\shared'],extraCount:2,failedScan:false}}
  ]};
  const html=await renderAccount(view);
  assert.match(html,/Codex is temporarily unavailable/);
  assert.match(html,/Unsafe &lt;script>/);
  assert.doesNotMatch(html,/<script>|SECRET_NOTICE/);
  assert.match(html,/from the previous connection/);
});

test("unsupported account mode renders its subscription policy",async()=>{
  const view={...emptyAppServerAccount(),connected:true,account:{type:'apiKey',email:null,planType:null,supported:false,policy:'ChatGPT subscription accounts only; no API key is read.'}};
  const html=await renderAccount(view);
  assert.match(html,/API key detected · unsupported/);
  assert.match(html,/ChatGPT subscription accounts only; no API key is read\./);
  assert.match(html,/Usage limits are available only/);
});

test("rate-limit and runtime-notice helpers expose deliberate public text",()=>{
  assert.equal(rateLimitWindowText({usedPercent:31.5,windowDurationMins:15,resetsAt:0}),"31.5% used · 15 min window · resets 1970-01-01T00:00:00.000Z");
  const notice={sequence:1,current:true,value:{kind:"config_warning",summary:"Invalid setting",details:"Use a supported value",path:"C:\\config.toml",line:3,column:4}};
  assert.equal(runtimeNoticeTitle(notice),"Configuration warning");
  assert.deepEqual(runtimeNoticeLines(notice),["Invalid setting","Use a supported value","C:\\config.toml · 3:4"]);
});
