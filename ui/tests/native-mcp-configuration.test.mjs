import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import ts from "typescript";

function component(){
  const source=fs.readFileSync(new URL("../src/components/NativeMcpConfiguration.svelte",import.meta.url),"utf8").match(/<script lang="ts">([\s\S]*?)<\/script>/)[1];
  const ast=ts.createSourceFile("component.ts",source,ts.ScriptTarget.Latest,true);
  const functions=ast.statements.filter(ts.isFunctionDeclaration).map(node=>node.getText(ast)).join("\n");
  const sent=[];
  const ctx={configuration:{viewId:"v1",current:true,snapshot:{target:{file:"fixture-config",version:"r1"},servers:[]}},blocked:false,pending:null,
    name:" fixture ",transport:"stdio",command:"server",args:'["--stdio","argument with spaces"]',cwd:"",envVars:"TOKEN, SECOND_TOKEN",url:"https://example.invalid/mcp",tokenEnv:"TOKEN",error:"",structuredClone,
    dialog:{open:false,showModal(){this.open=true;},close(){this.open=false;}},onAction:action=>sent.push(action)};
  Object.defineProperty(ctx,"editable",{get:()=>!ctx.blocked && !!ctx.configuration?.current && !!ctx.configuration.snapshot.target});
  const context=vm.createContext(ctx);
  vm.runInContext(ts.transpileModule(functions,{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText,context);
  return {ctx,sent,run:code=>vm.runInContext(code,context)};
}
test("MCP creation freezes the complete explicit input before shared-config confirmation",()=>{
  const c=component();c.run("add({preventDefault(){}})");assert.equal(c.sent.length,0);assert.equal(c.ctx.dialog.open,true);
  c.ctx.command="changed";c.ctx.args="[]";c.run("confirm();confirm()");
  assert.equal(c.sent.length,1);
  assert.deepEqual(JSON.parse(JSON.stringify(c.sent[0])),{kind:"change_configuration",view_id:"v1",edit:{kind:"add",name:"fixture",transport:{kind:"stdio",command:"server",args:["--stdio","argument with spaces"],cwd:null,env_vars:["TOKEN","SECOND_TOKEN"]}}});
});
test("MCP confirmation cannot survive disconnect, stale revision, busy or missing target",()=>{
  for(const invalidate of [c=>c.blocked=true,c=>c.configuration.current=false,c=>c.configuration.viewId="v2",c=>c.configuration.snapshot.target=null]){
    const c=component();c.run('review({kind:"remove",name:"fixture"})');invalidate(c.ctx);c.run("confirm()");assert.equal(c.sent.length,0);
  }
});
test("MCP HTTP uses an environment-name reference and malformed arguments send nothing",()=>{
  const c=component();c.ctx.args='{"TOKEN":"secret"}';c.run("add({preventDefault(){}})");assert.equal(c.ctx.pending,null);assert.match(c.ctx.error,/array/);
  c.ctx.transport="http";c.run("add({preventDefault(){}});confirm()");
  assert.equal(c.sent[0].edit.transport.kind,"http");assert.equal(c.sent[0].edit.transport.bearer_token_env_var,"TOKEN");
});

test("MCP option confirmation freezes replacement maps and sends a single native intent",()=>{
  const c=component();c.run('review({kind:"set_option",name:"fixture",key:"env_http_headers",value:{"X-Key":"TOKEN"}})');
  assert.equal(c.sent.length,0);assert.equal(c.ctx.dialog.open,true);
  c.run("confirm();confirm()");assert.equal(c.sent.length,1);
  assert.deepEqual(JSON.parse(JSON.stringify(c.sent[0])),{kind:"change_configuration",view_id:"v1",edit:{kind:"set_option",name:"fixture",key:"env_http_headers",value:{"X-Key":"TOKEN"}}});
});

test("a queued close cannot erase a newly opened MCP change confirmation",()=>{
  const c=component();
  c.run('review({kind:"remove",name:"old"});dialog.close();review({kind:"remove",name:"new"});closed()');
  assert.equal(c.ctx.dialog.open,true);assert.equal(c.ctx.pending.edit.name,"new");
  assert.equal(c.sent.length,0);c.run("confirm()");
  assert.equal(c.sent.length,1);assert.equal(c.sent[0].edit.name,"new");
  c.run('review({kind:"remove",name:"cancelled"});dialog.close();closed()');
  assert.equal(c.ctx.pending,null);assert.equal(c.sent.length,1);
});
