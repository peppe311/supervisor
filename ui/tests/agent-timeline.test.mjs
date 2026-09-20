import test from "node:test";
import assert from "node:assert/strict";
import { finalOutputIndex, nativeWorkDuration, activityGroupCopy } from "../src/lib/agent-timeline.ts";

test("work descriptions include native file changes, commands and named tools", () => {
  const item = (category, extra={}) => ({category,status:"completed",streaming:false,...extra});
  assert.equal(activityGroupCopy([item("command"),item("command")]).title,"Ha eseguito comandi");
  assert.equal(activityGroupCopy([item("command")]).title,"Comando eseguito");
  assert.equal(activityGroupCopy([item("command"),item("web_search")]).title,"Ha eseguito comandi e ha cercato sul web");
  assert.equal(activityGroupCopy([item("web_read")]).title,"Ha consultato pagine web");
  assert.equal(activityGroupCopy([item("file_change",{fileCount:1}),item("command"),item("tool",{toolName:"open_in_codex"})]).title,
    "Ha modificato un file, ha eseguito comandi e ha usato Open in Codex");
  assert.equal(activityGroupCopy([item("file_change",{fileCount:1,status:"error"})]).title,"Ha proposto modifiche ai file");
  assert.equal(activityGroupCopy([item("command",{status:"denied"})]).title,"Comando non autorizzato");
  assert.equal(activityGroupCopy([item("command",{status:"error"})]).title,"Comando non riuscito");
  assert.equal(activityGroupCopy([item("command",{status:"stopped"})]).title,"Comando interrotto");
  assert.equal(activityGroupCopy([item("command",{status:"running"})]).title,"Esecuzione di un comando…");
});

test("native duration is authoritative and missing values do not turn into import latency", () => {
  assert.equal(nativeWorkDuration(undefined), undefined);
  assert.equal(nativeWorkDuration({durationMs:2456,startedAtMs:100000,completedAtMs:102000}), 2456);
  assert.equal(nativeWorkDuration({durationMs:0}), 0);
  assert.equal(nativeWorkDuration({startedAtMs:1000,completedAtMs:5000}), 4000);
  assert.equal(nativeWorkDuration({durationMs:null,startedAtMs:null,completedAtMs:null}), null);
  assert.equal(nativeWorkDuration({durationMs:-1,startedAtMs:5000,completedAtMs:1000}), null);
});

test("commentary alone never masquerades as the final answer", () => {
  assert.equal(finalOutputIndex([{ role: "assistant", kind: "message", messagePhase: "commentary", text: "Reading files" }]), -1);
});
test("explicit final is distinct from reasoning, commands and trailing commentary", () => {
  assert.equal(finalOutputIndex([
    { role: "assistant", kind: "reasoning" }, { role: "assistant", kind: "activity" },
    { role: "assistant", kind: "message", messagePhase: "final_answer" },
    { role: "assistant", kind: "message", messagePhase: "commentary" },
  ]), 2);
});
test("legacy messages remain readable and failed turns can end in a system error", () => {
  assert.equal(finalOutputIndex([{ role: "assistant", text: "Old answer" }]), 0);
  assert.equal(finalOutputIndex([{ role: "assistant", kind: "reasoning" }, { role: "system", text: "Stopped" }]), 1);
});

test("native service notices never masquerade as a final answer", () => {
  const notice = {role:"system", kind:"service_notice", provider:"codex_app_server", text:"Account verification required"};
  assert.equal(finalOutputIndex([notice]), -1);
  assert.equal(finalOutputIndex([{role:"assistant", messagePhase:"commentary", text:"Working"},notice]), -1);
  assert.equal(finalOutputIndex([{role:"assistant", messagePhase:"final_answer", text:"Done"},notice]), 0);
});
