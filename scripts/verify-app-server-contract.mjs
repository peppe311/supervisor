import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";
import Ajv from "../ui/node_modules/ajv/dist/ajv.js";

const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),"..");
const versions=JSON.parse(fs.readFileSync(path.join(root,"protocol/app-server/supported-versions.json"),"utf8"));
assert.ok(Array.isArray(versions) && versions.length > 0);
assert.equal(new Set(versions).size,versions.length);
for(const version of versions) assert.match(version,/^\d+\.\d+\.\d+(?:-[a-zA-Z0-9.-]+)?$/);
const version=process.argv[2];
if(!version) {
  for(const selected of versions) execFileSync(process.execPath,[fileURLToPath(import.meta.url),selected],{cwd:root,stdio:"inherit"});
  process.exit(0);
}
assert.ok(versions.includes(version),`Unsupported contract version: ${version}`);
const schemaPath=relative=>path.join(root,"protocol/app-server",version,"json",relative);
const schema=JSON.parse(fs.readFileSync(schemaPath("ClientRequest.json"),"utf8"));
function createValidator() {
const ajv=new Ajv({strict:false,allErrors:true});
for(const [name,min,max] of [["int64",Number.MIN_SAFE_INTEGER,Number.MAX_SAFE_INTEGER],["uint64",0,Number.MAX_SAFE_INTEGER],["uint",0,Number.MAX_SAFE_INTEGER],["int32",-2147483648,2147483647],["uint32",0,4294967295],["uint16",0,65535],["uint8",0,255]]) {
  ajv.addFormat(name,{type:"number",validate:value=>Number.isSafeInteger(value)&&value>=min&&value<=max});
}
for(const name of ["double","float"]) ajv.addFormat(name,{type:"number",validate:Number.isFinite});
return ajv;
}
const ajv=createValidator();
const calls=JSON.parse(execFileSync("cargo",["run","--quiet","--locked","-p","central-agent-codex-runtime","--example","contract"],{cwd:root,encoding:"utf8",maxBuffer:4*1024*1024}));
for(const call of calls) {
  const branch=schema.oneOf.find(branch=>branch.properties?.method?.enum?.includes(call.method));
  assert.ok(branch,`${call.method} is not in the generated stable contract`);
  const validate=ajv.compile({$schema:schema.$schema,definitions:schema.definitions,...branch});
  assert.ok(validate(call),`${call.method}: ${JSON.stringify(validate.errors)}`);
  assert.equal(call.jsonrpc,undefined);
}
console.log(`Official App Server contract verified: ${calls.length} serialized Rust calls against CLI ${version} generated schema.`);
const goalSamples=JSON.parse(fs.readFileSync(path.join(root,"crates/central-agent-codex-runtime/tests/native-goals.json"),"utf8"));
for(const sample of goalSamples) {
  const goalSchema=JSON.parse(fs.readFileSync(schemaPath(`v2/${sample.schema}.json`),"utf8"));
  const validate=createValidator().compile(goalSchema);
  assert.ok(validate(sample.params),`${sample.schema}: ${JSON.stringify(validate.errors)}`);
}
console.log(`Native goal contract verified: ${goalSamples.length} response and notification samples.`);
const patchSchema=JSON.parse(fs.readFileSync(schemaPath("v2/FileChangePatchUpdatedNotification.json"),"utf8"));
const validatePatch=createValidator().compile(patchSchema);
const patchSamples=JSON.parse(fs.readFileSync(path.join(root,"crates/central-agent-codex-runtime/tests/native-patch-updates.json"),"utf8"));
for(const sample of patchSamples) assert.ok(validatePatch(sample),`Native patch update: ${JSON.stringify(validatePatch.errors)}`);
console.log(`Native patch-update contract verified: ${patchSamples.length} replacement snapshots.`);
const progressSchema=JSON.parse(fs.readFileSync(schemaPath("v2/McpToolCallProgressNotification.json"),"utf8"));
const validateProgress=createValidator().compile(progressSchema);
const progressSamples=JSON.parse(fs.readFileSync(path.join(root,"crates/central-agent-codex-runtime/tests/native-mcp-progress.json"),"utf8"));
for(const sample of progressSamples) assert.ok(validateProgress(sample),`Native MCP progress: ${JSON.stringify(validateProgress.errors)}`);
console.log(`Native MCP progress contract verified: ${progressSamples.length} messages.`);
const server=JSON.parse(fs.readFileSync(schemaPath("ServerRequest.json"),"utf8"));
const replies=JSON.parse(execFileSync("cargo",["run","--quiet","--locked","-p","central-agent-codex-runtime","--example","request_contract"],{cwd:root,encoding:"utf8",maxBuffer:4*1024*1024}));
for(const reply of replies) {
  const branch=server.oneOf.find(branch=>branch.properties?.method?.enum?.includes(reply.request.method));
  assert.ok(branch,`${reply.request.method} is not in the selected server contract`);
  const validateRequest=ajv.compile({$schema:server.$schema,definitions:server.definitions,...branch});
  assert.ok(validateRequest(reply.request),`${reply.request.method}: ${JSON.stringify(validateRequest.errors)}`);
  const responseSchema=JSON.parse(fs.readFileSync(schemaPath(`${reply.schema}.json`),"utf8"));
  // Each standalone response schema may repeat its own definitions/$id.
  const validator=createValidator().compile(responseSchema);
  assert.ok(validator(reply.result),`${reply.schema}: ${JSON.stringify(validator.errors)}`);
}
console.log(`Native request contract verified: ${replies.length} serialized Rust decisions and matching server requests.`);
const usageSchema=JSON.parse(fs.readFileSync(schemaPath("v2/ThreadTokenUsageUpdatedNotification.json"),"utf8"));
const validateUsage=createValidator().compile(usageSchema);
const usageSamples=JSON.parse(readUsageFixtures());
function readUsageFixtures(){return fs.readFileSync(path.join(root,"crates/central-agent-codex-runtime/tests/token-usage.json"),"utf8");}
for(const sample of usageSamples) assert.ok(validateUsage(sample),`Native usage: ${JSON.stringify(validateUsage.errors)}`);
console.log(`Native token-usage contract verified: ${usageSamples.length} incoming report samples.`);
const lifecycleSamples=JSON.parse(fs.readFileSync(path.join(root,"crates/central-agent-codex-runtime/tests/thread-lifecycle.json"),"utf8"));
for(const sample of lifecycleSamples) {
  const lifecycleSchema=JSON.parse(fs.readFileSync(schemaPath(`v2/${sample.schema}.json`),"utf8"));
  const validate=createValidator().compile(lifecycleSchema);
  assert.ok(validate(sample.params),`${sample.method}: ${JSON.stringify(validate.errors)}`);
}
console.log(`Native lifecycle contract verified: ${lifecycleSamples.length} incoming notifications.`);
const mcpSamples=JSON.parse(fs.readFileSync(path.join(root,"crates/central-agent-codex-runtime/tests/native-mcp.json"),"utf8"));
for(const sample of mcpSamples) {
  const responseSchema=JSON.parse(fs.readFileSync(schemaPath(`v2/${sample.schema}.json`),"utf8"));
  const validate=createValidator().compile(responseSchema);
  assert.ok(validate(sample.params),`${sample.schema}: ${JSON.stringify(validate.errors)}`);
}
console.log(`Native MCP contract verified: ${mcpSamples.length} inventory/OAuth samples.`);
const historyThread=JSON.parse(fs.readFileSync(path.join(root,"crates/central-agent-codex-runtime/tests/native-history-thread.json"),"utf8"));
for(const [schemaName,value] of [
  ["ThreadListResponse",{data:[historyThread],nextCursor:"older",backwardsCursor:"newer"}],
  ["ThreadReadResponse",{thread:historyThread}],
  ["ThreadResumeResponse",{thread:historyThread,model:"fixture-model",modelProvider:"openai",serviceTier:null,cwd:historyThread.cwd,instructionSources:[],approvalPolicy:"untrusted",approvalsReviewer:"user",sandbox:{type:"readOnly",networkAccess:false},reasoningEffort:null,turnsBackwardsCursor:null,itemsBackwardsCursor:null}],
  ["ThreadForkResponse",{thread:{...historyThread,id:"fixture-fork",forkedFromId:historyThread.id},model:"fixture-model",modelProvider:"openai",serviceTier:null,cwd:historyThread.cwd,instructionSources:[],approvalPolicy:"untrusted",approvalsReviewer:"user",sandbox:{type:"readOnly",networkAccess:false},reasoningEffort:null}],
]) {
  const responseSchema=JSON.parse(fs.readFileSync(schemaPath(`v2/${schemaName}.json`),"utf8"));
  const validate=createValidator().compile(responseSchema);
  assert.ok(validate(value),`${schemaName}: ${JSON.stringify(validate.errors)}`);
}
console.log("Native history contract verified: list, read, review preparation/resume and fork response fixtures.");
const reviewSamples=JSON.parse(fs.readFileSync(path.join(root,"crates/central-agent-codex-runtime/tests/native-review.json"),"utf8"));
for(const sample of reviewSamples) {
  const responseSchema=JSON.parse(fs.readFileSync(schemaPath(`v2/${sample.schema}.json`),"utf8"));
  const validate=createValidator().compile(responseSchema);
  assert.ok(validate(sample.params),`${sample.schema}: ${JSON.stringify(validate.errors)}`);
}
console.log(`Native review contract verified: ${reviewSamples.length} response and notification samples.`);
const skillSamples=JSON.parse(fs.readFileSync(path.join(root,"crates/central-agent-codex-runtime/tests/native-skills.json"),"utf8"));
for(const sample of skillSamples) {
  const responseSchema=JSON.parse(fs.readFileSync(schemaPath(`v2/${sample.schema}.json`),"utf8"));
  const validate=createValidator().compile(responseSchema);
  assert.ok(validate(sample.params),`${sample.schema}: ${JSON.stringify(validate.errors)}`);
}
console.log(`Native skills contract verified: ${skillSamples.length} response and notification samples.`);
const configSamples=JSON.parse(fs.readFileSync(path.join(root,"crates/central-agent-codex-runtime/tests/native-configuration.json"),"utf8"));
const threadSettings=JSON.parse(fs.readFileSync(path.join(root,"crates/central-agent-codex-runtime/tests/thread-settings.json"),"utf8"));
const threadSettingsSchema=JSON.parse(fs.readFileSync(schemaPath("v2/ThreadSettingsUpdatedNotification.json"),"utf8"));
const validateThreadSettings=createValidator().compile(threadSettingsSchema);
assert.ok(validateThreadSettings(threadSettings),`Native thread settings: ${JSON.stringify(validateThreadSettings.errors)}`);
console.log("Native thread settings notification verified against the generated schema.");
for(const sample of configSamples) {
  const responseSchema=JSON.parse(fs.readFileSync(schemaPath(`v2/${sample.schema}.json`),"utf8"));
  const validate=createValidator().compile(responseSchema);
  assert.ok(validate(sample.params),`${sample.schema}: ${JSON.stringify(validate.errors)}`);
}
console.log(`Native configuration contract verified: ${configSamples.length} layered-read and write-outcome samples.`);
const activityItems=JSON.parse(fs.readFileSync(path.join(root,"crates/central-agent-codex-runtime/tests/native-activity-items.json"),"utf8"));
for(const [schemaName,timing] of [["ItemStartedNotification",{startedAtMs:1000}],["ItemCompletedNotification",{completedAtMs:2000}]]) {
  const itemSchema=JSON.parse(fs.readFileSync(schemaPath(`v2/${schemaName}.json`),"utf8"));
  const validate=createValidator().compile(itemSchema);
  for(const item of activityItems) {
    assert.ok(validate({threadId:"events-thread",turnId:"events-turn",item,...timing}),`${schemaName}/${item.type}: ${JSON.stringify(validate.errors)}`);
  }
}
console.log(`Native activity contract verified: ${activityItems.length} item variants in both lifecycle notifications.`);
const modelNoticeSamples=JSON.parse(fs.readFileSync(path.join(root,"crates/central-agent-codex-runtime/tests/native-model-notices.json"),"utf8"));
for(const sample of modelNoticeSamples) {
  const noticeSchema=JSON.parse(fs.readFileSync(schemaPath(`v2/${sample.schema}.json`),"utf8"));
  const validate=createValidator().compile(noticeSchema);
  assert.ok(validate(sample.params),`${sample.method}: ${JSON.stringify(validate.errors)}`);
}
console.log(`Native model-notice contract verified: ${modelNoticeSamples.length} service notifications.`);
const p0Samples=JSON.parse(fs.readFileSync(path.join(root,"crates/central-agent-codex-runtime/tests/p0-runtime-contract.json"),"utf8"));
for(const sample of p0Samples) {
  const p0Schema=JSON.parse(fs.readFileSync(schemaPath(`v2/${sample.schema}.json`),"utf8"));
  const validate=createValidator().compile(p0Schema);
  assert.ok(validate(sample.params),`${sample.method}: ${JSON.stringify(validate.errors)}`);
}
console.log(`Native P0 contract verified: ${p0Samples.length} notification, inventory and lifecycle samples.`);
const p1Samples=JSON.parse(fs.readFileSync(path.join(root,"crates/central-agent-codex-runtime/tests/p1-runtime-contract.json"),"utf8"));
for(const sample of p1Samples) {
  const p1Schema=JSON.parse(fs.readFileSync(schemaPath(`v2/${sample.schema}.json`),"utf8"));
  const validate=createValidator().compile(p1Schema);
  assert.ok(validate(sample.params),`${sample.method}: ${JSON.stringify(validate.errors)}`);
}
console.log(`Native P1 contract verified: ${p1Samples.length} account, Apps, hooks and direct MCP samples.`);
const p2Samples=JSON.parse(fs.readFileSync(path.join(root,"crates/central-agent-codex-runtime/tests/p2-runtime-contract.json"),"utf8"));
for(const sample of p2Samples) {
  const p2Schema=JSON.parse(fs.readFileSync(schemaPath(`v2/${sample.schema}.json`),"utf8"));
  const validate=createValidator().compile(p2Schema);
  assert.ok(validate(sample.params),`${sample.method}: ${JSON.stringify(validate.errors)}`);
}
console.log(`Native P2 contract verified: ${p2Samples.length} command, feature, import, account and feedback samples.`);

// P3-A is backend-only. Reuse the versioned full Thread fixture rather than
// treating the minimal unit-test metadata objects as complete wire responses.
const p3Section={id:"00000000-0000-7000-8000-000000000001",name:"Fixture section",appearance:null};
const p3Thread={...historyThread,historyMode:"paginated",turns:[],section:p3Section,gitInfo:{sha:"a".repeat(40),branch:"fixture",originUrl:"https://example.test/owned/repo.git"}};
const p3Samples=[
  ["ThreadSectionListResponse",{data:[p3Section],nextCursor:"next"}],
  ["ThreadSectionListResponse",{data:[],nextCursor:null}],
  ["ThreadSectionCreateResponse",{section:p3Section}],
  ["ThreadSectionUpdateResponse",{section:{...p3Section,name:"Renamed"}}],
  ["ThreadSectionDeleteResponse",{}],
  ["ThreadSectionMoveResponse",{}],
  ["ThreadMetadataUpdateResponse",{thread:p3Thread}],
  ["ThreadRevertResponse",{thread:p3Thread,turnsBackwardsCursor:"retained-turn",itemsBackwardsCursor:null}],
  ["ThreadRevertedNotification",{threadId:p3Thread.id}],
  ["ThreadItemsListResponse",{data:[],nextCursor:null,backwardsCursor:null}],
];
for(const [name,value] of p3Samples) {
  const p3Schema=JSON.parse(fs.readFileSync(schemaPath(`v2/${name}.json`),"utf8"));
  const validate=createValidator().compile(p3Schema);
  assert.ok(validate(value),`${name}: ${JSON.stringify(validate.errors)}`);
}
console.log(`Native P3-A contract verified: ${p3Samples.length} Git, section, revert and disabled-item-list response/notification samples.`);
