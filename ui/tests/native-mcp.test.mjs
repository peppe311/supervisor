import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import ts from "typescript";
import {emptyNativeMcp,mcpAuthLabel,mcpCanLogin,mcpRuntimeLabel} from "../src/lib/app-server-mcp.ts";

test("unknown native connection/auth states are not inferred as connected",()=>{
  assert.equal(emptyNativeMcp().current,false);
  assert.equal(emptyNativeMcp().inventoryId,null);
  assert.equal(mcpRuntimeLabel(null),"Not reported for this scope");
  assert.equal(mcpRuntimeLabel("failed"),"Failed");
  assert.equal(mcpAuthLabel("unknown"),"Not reported");
  assert.equal(mcpCanLogin({authStatus:"unsupported",runtimeStatus:null}),false);
  assert.equal(mcpCanLogin({authStatus:"notLoggedIn",runtimeStatus:null}),true);
  assert.equal(mcpCanLogin({authStatus:"oAuth",runtimeStatus:"authenticationRequired"}),true);
});
function component(){
  const source=fs.readFileSync(new URL("../src/components/NativeMcpSettings.svelte",import.meta.url),"utf8").match(/<script lang="ts">([\s\S]*?)<\/script>/)[1];
  const ast=ts.createSourceFile("component.ts",source,ts.ScriptTarget.Latest,true);
  const functions=ast.statements.filter(ts.isFunctionDeclaration).map(node=>node.getText(ast)).join("\n");
  const sent=[];
  const ctx={connected:true,threadScope:false,mcpCanLogin,view:{...emptyNativeMcp(),inventoryId:"observed",current:true},confirmedInventory:null,
    dialog:{open:false,showModal(){this.open=true;},close(){this.open=false;}},toolDialog:{open:false,showModal(){this.open=true;},close(){this.open=false;}},
    toolServer:'',toolName:'',toolSchema:'{}',toolArguments:'{}',toolError:'',toolInventory:null,onAction:action=>sent.push(action)};
  Object.defineProperty(ctx,"blocked",{get:()=>!ctx.connected||ctx.view.busy||!!ctx.view.login});
  const context=vm.createContext(ctx);
  vm.runInContext(ts.transpileModule(functions,{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText,context);
  return {ctx,sent,run:code=>vm.runInContext(code,context)};
}
test("MCP reload needs confirmation bound to the observed inventory",()=>{
  const c=component();c.run("reload()");assert.equal(c.sent.length,0);assert.equal(c.ctx.dialog.open,true);
  c.run("confirmReload();confirmReload()");assert.equal(c.sent.length,1);
  assert.equal(c.sent[0].kind,"reload");assert.equal(c.sent[0].inventory_id,"observed");
  c.run("reload()");c.ctx.view.inventoryId="new";c.run("confirmReload()");assert.equal(c.sent.length,1);
});
test("disconnection or pending OAuth invalidates a reload confirmation",()=>{
  for(const change of [c=>c.connected=false,c=>c.view.login={attemptId:"pending"},c=>c.view.current=false,c=>c.view.busy=true]){
    const c=component();c.run("reload()");change(c.ctx);c.run("confirmReload()");assert.equal(c.sent.length,0);
  }
});
test("conversation inventory cannot open or submit a shared reload",()=>{
  const c=component();c.ctx.threadScope=true;c.run("reload();confirmReload()");
  assert.equal(c.ctx.dialog.open,false);assert.equal(c.sent.length,0);
  c.ctx.threadScope=false;c.run("reload()");c.ctx.threadScope=true;c.run("confirmReload()");assert.equal(c.sent.length,0);
});
test("OAuth intent requires the current inventory and supported exact server, including thread scope",()=>{
  for(const threadScope of [false,true]){
    const c=component();c.ctx.threadScope=threadScope;
    c.ctx.view.servers=[{name:'fixture',authStatus:'notLoggedIn',runtimeStatus:null}];
    c.run('authorize("foreign")');assert.equal(c.sent.length,0);
    c.run('authorize("fixture")');assert.deepEqual(JSON.parse(JSON.stringify(c.sent[0])),{kind:'login',inventory_id:'observed',name:'fixture'});
    c.ctx.view.current=false;c.run('authorize("fixture")');assert.equal(c.sent.length,1);
    c.ctx.view.current=true;c.ctx.view.login={attemptId:'pending'};c.run('authorize("fixture")');assert.equal(c.sent.length,1);
  }
});

test("a delayed MCP reload close preserves a new explicit confirmation",()=>{
  const c=component();c.run('reload();dialog.close()');c.ctx.view.inventoryId="new";
  c.run('reload();closed()');assert.equal(c.ctx.confirmedInventory,"new");assert.equal(c.sent.length,0);
  c.run('confirmReload()');assert.equal(c.sent.length,1);assert.equal(c.sent[0].inventory_id,"new");
  c.run('reload();dialog.close();closed()');assert.equal(c.ctx.confirmedInventory,null);
});

test("direct MCP resources use opaque entry IDs and direct tools require a fresh explicit confirmation",()=>{
  const c=component();c.ctx.threadScope=true;
  c.ctx.view.servers=[{name:'fixture',authStatus:'notLoggedIn',runtimeStatus:'connected',resources:[{entryId:'opaque-resource'}],tools:{lookup:{name:'lookup',inputSchema:'{"type":"object"}'}}}];
  c.run("readResource('fixture',view.servers[0].resources[0])");
  assert.deepEqual(JSON.parse(JSON.stringify(c.sent[0])),{kind:'read_resource',inventory_id:'observed',server:'fixture',resource_id:'opaque-resource'});
  c.run("prepareTool('fixture',view.servers[0].tools.lookup)");
  assert.equal(c.ctx.toolDialog.open,true);assert.equal(c.ctx.toolInventory,'observed');
  c.ctx.toolArguments='{"query":"exact"}';c.run('callTool()');
  assert.deepEqual(JSON.parse(JSON.stringify(c.sent[1])),{kind:'call_tool',inventory_id:'observed',server:'fixture',tool:'lookup',arguments:{query:'exact'}});

  c.run("prepareTool('fixture',view.servers[0].tools.lookup)");c.ctx.view.inventoryId='changed';c.run('callTool()');
  assert.equal(c.sent.length,2);
});

test("direct MCP tool UI rejects invalid JSON and non-object arguments before dispatch",()=>{
  for(const value of ['not-json','[]','null']){
    const c=component();c.ctx.threadScope=true;c.ctx.view.servers=[{name:'fixture',tools:{lookup:{name:'lookup',inputSchema:'{}'}},resources:[]}];
    c.run("prepareTool('fixture',view.servers[0].tools.lookup)");c.ctx.toolArguments=value;c.run('callTool()');
    assert.equal(c.sent.length,0);assert.ok(c.ctx.toolError);
  }
});

test("large MCP inventories are filterable and do not display opaque thread identity",()=>{
  const source=fs.readFileSync(new URL("../src/components/NativeMcpSettings.svelte",import.meta.url),"utf8");
  assert.match(source,/Filter servers, tools and resources/);
  assert.match(source,/No MCP server, tool or resource matches this filter/);
  assert.match(source,/\.tool-call-dialog \{[^}]*box-sizing:border-box[^}]*inline-size:min\(430px,calc\(100vw - 32px\)\)[^}]*overflow-x:hidden/);
  assert.match(source,/\.tool-call-dialog form > \* \{[^}]*max-inline-size:100%[^}]*min-inline-size:0/);
  assert.match(source,/\.tool-call-dialog h2 \{[^}]*overflow-wrap:anywhere[^}]*word-break:break-all/);
  assert.doesNotMatch(source,/Native thread:/);
});
