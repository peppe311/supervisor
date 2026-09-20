import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import ts from "typescript";
import { optionsFor, optionValue } from "../src/lib/mcp-options.ts";

test("MCP option inputs distinguish lists, numbers, booleans and explicit clearing",()=>{
  assert.deepEqual(optionValue("args",'["--stdio", "with spaces"]'),["--stdio","with spaces"]);
  assert.deepEqual(optionValue("enabled_tools","[]"),[]);
  assert.deepEqual(optionValue("env_http_headers",'{"X-Key":"TOKEN"}'),{"X-Key":"TOKEN"});
  assert.equal(optionValue("required","false"),false);
  assert.equal(optionValue("startup_timeout_sec","0.5"),0.5);
  for(const [key,value] of [["args","{}"],["args","null"],["env_http_headers","[]"],["env_http_headers",'{"X-Key":1}'],["tool_timeout_sec",""],["tool_timeout_sec","0"],["tool_timeout_sec","Infinity"],["required",""],["command",""]])assert.throws(()=>optionValue(key,value));
  assert.ok(!optionsFor("stdio").some(o=>o.key==="url"));
  assert.ok(!optionsFor("http").some(o=>o.key==="command"));
  assert.deepEqual(optionsFor("unknown"),[]);
});

function component(){
  const source=fs.readFileSync(new URL("../src/components/NativeMcpOptions.svelte",import.meta.url),"utf8").match(/<script lang="ts">([\s\S]*?)<\/script>/)[1];
  const ast=ts.createSourceFile("component.ts",source,ts.ScriptTarget.Latest,true);
  const functions=ast.statements.filter(ts.isFunctionDeclaration).map(node=>node.getText(ast)).join("\n");
  const sent=[];
  const ctx={Error,server:{name:"fixture",canEdit:true,userTransport:"stdio",savedOptions:["args"]},viewId:"v1",editable:true,openedView:"",editing:false,key:"command",value:"",error:"",optionValue,onReview:edit=>sent.push(edit)};
  Object.defineProperty(ctx,"options",{get:()=>optionsFor(ctx.server.userTransport)});
  Object.defineProperty(ctx,"selected",{get:()=>ctx.options.find(o=>o.key===ctx.key)});
  Object.defineProperty(ctx,"current",{get:()=>ctx.editable && ctx.openedView===ctx.viewId && ctx.server.canEdit});
  const context=vm.createContext(ctx);
  vm.runInContext(ts.transpileModule(functions,{compilerOptions:{target:ts.ScriptTarget.ES2022}}).outputText,context);
  return {ctx,sent,run:code=>vm.runInContext(code,context)};
}

test("MCP options reach the existing confirmation only with the original editable view",()=>{
  for(const invalidate of [c=>c.editable=false,c=>c.viewId="v2",c=>c.server.canEdit=false,c=>c.server.userTransport="http"]){
    const c=component();c.run("begin()");c.ctx.key="args";c.ctx.value='["--new"]';invalidate(c.ctx);c.run("review(false);review(true)");assert.equal(c.sent.length,0);
  }
  const c=component();c.run("begin()");c.ctx.key="args";c.ctx.value='["--new"]';c.run("review(false)");
  assert.deepEqual(JSON.parse(JSON.stringify(c.sent[0])),{kind:"set_option",name:"fixture",key:"args",value:["--new"]});
  c.run("review(true)");assert.equal(c.sent[1].kind,"clear_option");
  c.ctx.key="command";c.run("review(true)");c.ctx.key="cwd";c.run("review(true)");assert.equal(c.sent.length,2);
  c.ctx.value="";c.run("review(false)");assert.equal(c.sent.length,2);assert.match(c.ctx.error,/Enter a value/);
});
