// Deterministic protocol fixture only. No model, credentials, file reads or network.
import readline from "node:readline";
const mode = process.argv[2];
let initialized = false;
let hold;
let approval;
const threads = new Map();
const send = (frame) => process.stdout.write(JSON.stringify(frame) + "\n");
readline.createInterface({ input: process.stdin }).on("line", line => {
  const frame = JSON.parse(line);
  if (frame.method === "initialize") {
    if (mode === "reject-initialize") { send({id:frame.id,error:{code:-32600,message:"Fixture rejected initialization"}}); return; }
    // Fill more than a normal pipe buffer; the client must drain stderr.
    process.stderr.write("diagnostic fixture\n".repeat(10000), () => {
      const reply = JSON.stringify({id:frame.id,result:{userAgent:"fixture",platformOs:"windows"}}) + "\n";
      process.stdout.write(reply.slice(0, 9));
      setTimeout(() => process.stdout.write(reply.slice(9)), 5);
    });
  } else if (frame.method === "initialized") initialized = true;
  else if (!initialized) send({id:frame.id,error:{code:-32600,message:"Not initialized"}});
  else if (["conversations","steering"].includes(mode) && frame.method.startsWith("thread/")) {
    if (["history", "dynamicTools", "developerInstructions", "baseInstructions"].some(key => key in frame.params)) throw new Error("The thin client must not reconstruct Codex");
    if (frame.method === "thread/start") {
      const id = `native-${threads.size + 1}`;
      const thread = {id, sessionId:`session-${id}`,status:{type:"idle"},turns:[]};
      threads.set(id,thread);
      send({id:frame.id,result:{thread}});
    } else {
      const thread = threads.get(frame.params.threadId);
      if (!thread) send({id:frame.id,error:{code:-32602,message:"Unknown thread"}});
      else send({id:frame.id,result:{thread}});
    }
  } else if (["conversations","steering"].includes(mode) && frame.method === "turn/start") {
    const thread = threads.get(frame.params.threadId);
    const turn = {id:`turn-${thread.id}`,status:"inProgress",items:[]};
    send({method:"turn/started",params:{threadId:thread.id,turn}});
    if(mode === "steering") { thread.turns.push(turn); send({id:frame.id,result:{turn}}); return; }
    const item = {id:`answer-${thread.id}`,type:"agentMessage",phase:"final_answer",text:""};
    send({method:"item/started",params:{threadId:thread.id,turnId:turn.id,item}});
    send({method:"item/agentMessage/delta",params:{threadId:thread.id,turnId:turn.id,itemId:item.id,delta:`Answer for ${thread.id}`}});
    item.text = `Authoritative ${thread.id}`;
    send({method:"item/completed",params:{threadId:thread.id,turnId:turn.id,item}});
    const completed = {...turn,status:"completed",items:[{id:`input-${thread.id}`,clientId:frame.params.clientUserMessageId,type:"userMessage",content:frame.params.input},item]};
    thread.turns.push(completed);
    send({method:"turn/completed",params:{threadId:thread.id,turn:{...completed,items:[]}}});
    // Native completion can reach the UI before its async request worker reply.
    send({id:frame.id,result:{turn}});
  } else if(mode === "steering" && frame.method === "turn/steer") {
    if(Object.keys(frame.params).some(key=>!["threadId","expectedTurnId","clientUserMessageId","input"].includes(key))) throw new Error("Steer cannot override configuration");
    if(typeof frame.params.clientUserMessageId !== "string" || !frame.params.clientUserMessageId) throw new Error("Steer must retain its client message identity");
    const thread = threads.get(frame.params.threadId), turn = thread?.turns.at(-1);
    if(!turn || turn.status !== "inProgress" || turn.id !== frame.params.expectedTurnId) { send({id:frame.id,error:{code:-32602,message:"Wrong active turn"}}); return; }
    const item = {id:`followup-${thread.id}`,type:"userMessage",clientId:frame.params.clientUserMessageId,content:frame.params.input};
    turn.items.push(item);
    send({method:"item/completed",params:{threadId:thread.id,turnId:turn.id,item}});
    send({id:frame.id,result:{turnId:turn.id}});
  }
  else if (frame.method === "fixture/unknownNotification") {
    send({method:"future/notification",params:{opaque:"private fixture payload",threadId:"not-owned"}});
    send({id:frame.id,result:{ok:true}});
  }
  else if (frame.method === "fixture/requestApproval") {
    approval=frame.id;
    send({id:99,method:"item/commandExecution/requestApproval",params:{threadId:"a",turnId:"turn-a",itemId:"item-a",command:"fixture"}});
  } else if (frame.id === 99 && "result" in frame) send({id:approval,result:frame.result});
  else if (frame.method === "fixture/hold") hold=frame;
  else if (frame.method === "fixture/reverse") {
    send({id:frame.id,result:frame.params});
    send({id:hold.id,result:hold.params});
  } else if (frame.method === "fixture/exit") process.exit(0);
  else send({id:frame.id,result:frame.params});
});
