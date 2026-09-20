import test from "node:test";
import assert from "node:assert/strict";
import {createComputerUseTimer, marker} from "../crates/central-agent-codex-runtime/examples/fixtures/computer_use_timing.mjs";

test("public timing preserves the result and does not retain screen, window or typed content", async () => {
  const output = [], result = {window:{title:"private title"},screenshots:[{url:"private pixels"}],accessibility:{tree:"private document"}};
  const calls=[];let time=0;
  const sky={get_window_state:async value=>{calls.push(value);return result;}};
  const measure=createComputerUseTimer(sky, value=>output.push(value),()=>time+=5);
  const args={window:{id:42},include_text:true};
  assert.equal(await measure("get_window_state",args),result);
  assert.equal(calls[0],args);
  assert.deepEqual(JSON.parse(output[0].trim().slice(marker.length)),{sequence:1,method:"get_window_state",phase:"observation",screenshotRequested:true,textRequested:true,completed:true,imageReturned:true,textReturned:true,durationMs:5});
  assert.ok(output[0].startsWith("\n") && output[0].endsWith("\n"));
  assert.doesNotMatch(output.join(""),/private|title|pixels|document|42/);
});

test("input errors are recorded once and propagated without retrying or leaking their message", async () => {
  let count=0;const output=[],failure=new Error("private input and path");
  const measure=createComputerUseTimer({type_text:async()=>{count++;throw failure;}},value=>output.push(value));
  await assert.rejects(measure("type_text",{text:"private text"}),error=>error===failure);
  assert.equal(count,1);assert.equal(output.length,1);
  assert.doesNotMatch(output[0],/private|path/);
  assert.equal(JSON.parse(output[0].trim().slice(marker.length)).completed,false);
});

test("the probe is bounded and does not invoke unknown APIs",async()=>{
  let count=0;const measure=createComputerUseTimer({list_windows:async()=>{count++;return [];}},()=>{});
  await assert.rejects(measure("private_transport",{}),/Unsupported/);
  assert.equal(count,0);
  for(let i=0;i<256;i++)await measure("list_windows");
  await assert.rejects(measure("list_windows"),/limit/);
  assert.equal(count,256);
});
