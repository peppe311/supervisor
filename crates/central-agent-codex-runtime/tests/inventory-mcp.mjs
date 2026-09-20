// Local no-inference integration fixture, never a production MCP installation.
import { createInterface } from "node:readline";
import { appendFileSync } from "node:fs";
const trace = process.argv[2];
if (!trace) throw new Error("An owned temporary trace path is required");
createInterface({input:process.stdin}).on("line", line => {
  const request = JSON.parse(line);
  appendFileSync(trace, `${request.method}\n`);
  if (request.id === undefined) return;
  let result;
  switch (request.method) {
    case "initialize": result = {protocolVersion:request.params.protocolVersion,capabilities:{tools:{},resources:{}},serverInfo:{name:"inventory-fixture",version:"1.0.0"}}; break;
    case "tools/list": result = {tools:[{name:"fixture_echo",description:"Inventory-only test tool",inputSchema:{type:"object",properties:{}}}]}; break;
    case "resources/list": result = {resources:[{uri:"fixture://overview",name:"Fixture overview",mimeType:"text/plain"}]}; break;
    case "resources/templates/list": result = {resourceTemplates:[]}; break;
    case "ping": result = {}; break;
    default: process.stdout.write(JSON.stringify({jsonrpc:"2.0",id:request.id,error:{code:-32601,message:"This fixture exposes inventory only"}})+"\n"); return;
  }
  process.stdout.write(JSON.stringify({jsonrpc:"2.0",id:request.id,result})+"\n");
});
