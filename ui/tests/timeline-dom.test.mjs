// Structural DOM fixture only: no browser, pixels, network or model calls.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import { createAgentTimelineController } from '../src/lib/agent-timeline.ts';

test('native terminal activity labels remain explicit in the main host', () => {
  const html=fs.readFileSync(new URL('../../assets/agent-panel.html',import.meta.url),'utf8');
  const fn=html.match(/function activityRoleLabel\(status, streaming\) \{[\s\S]*?\n      \}/)?.[0];
  assert.ok(fn);
  const label=vm.runInNewContext(`(${fn})`);
  assert.equal(label('stopped',false),'Stopped');
  assert.equal(label('completed',false),'Completed');
  assert.notEqual(label('error',false),label('completed',false));
});

class Element extends EventTarget {
  // Node's EventTarget does not normalize the browser boolean capture shorthand.
  addEventListener(type, listener, options) { super.addEventListener(type,listener,options===true?{capture:true}:options); }
  removeEventListener(type, listener, options) { super.removeEventListener(type,listener,options===true?{capture:true}:options); }
  constructor(tag='div') { super(); this.tag=tag; this.children=[]; this.parent=null; this.dataset={}; this.className=''; this.open=false; this.scrollTop=0; this.scrollHeight=1000; this.clientHeight=300; this.textContent=''; }
  get childElementCount() { return this.children.length; }
  get classList() { const element=this; return {add(...names){ element.className=[...new Set([...element.className.split(' '),...names])].join(' '); },remove(...names){ element.className=element.className.split(' ').filter(name=>!names.includes(name)).join(' '); },toggle(name,on){ if(on) this.add(name); else this.remove(name); }}; }
  setAttribute() {}
  removeAttribute() {}
  append(...children) { for(const child of children) this.insertBefore(child,null); }
  prepend(child) { this.insertBefore(child,this.children[0]||null); }
  insertBefore(child, before) { child.remove(); child.parent=this; const index=before?this.children.indexOf(before):this.children.length; this.children.splice(index,0,child); }
  remove() { if(this.parent) { this.parent.children.splice(this.parent.children.indexOf(this),1); this.parent=null; } }
  replaceWith(child) { const parent=this.parent, index=parent.children.indexOf(this); child.remove(); this.remove(); parent.children.splice(index,0,child); child.parent=parent; }
  replaceChildren(...children) { for(const child of [...this.children]) child.remove(); this.append(...children); }
  matches(selector) { const tag=selector.match(/^[a-z]+/)?.[0]; return (!tag || tag===this.tag) && [...selector.matchAll(/\.([\w-]+)/g)].every(match=>this.className.split(' ').includes(match[1])) && (!selector.includes('[data-message-id]') || this.dataset.messageId!==undefined); }
  querySelectorAll(selector) { return this.children.flatMap(child=>[...(child.matches(selector)?[child]:[]),...child.querySelectorAll(selector)]); }
  querySelector(selector) { return this.querySelectorAll(selector)[0] || null; }
  closest(selector) { return this.matches(selector)?this:this.parent?.closest(selector)||null; }
}

test('native user row timing updates preserve its user presentation', () => {
  const html=fs.readFileSync(new URL('../../assets/agent-panel.html',import.meta.url),'utf8');
  const fn=html.match(/function patchChatMessageRow\(row, update\) \{[\s\S]*?\n      \}/)?.[0];
  assert.ok(fn);
  const attributes=new Map();
  const row=new Element(); row.className='chat-message user message';
  row.setAttribute=(key,value)=>attributes.set(key,value);
  row.removeAttribute=key=>attributes.delete(key);
  const text=new Element(); text.className='chat-text';
  const role=new Element('span'); role.className='chat-role';
  row.append(text,role);
  const patch=vm.runInNewContext(`(${fn})`, {
    setRenderedMessageContent:(element,message)=>{element.textContent=message.text;},
    chatRoleLabel:(_kind,role)=>role,
    activityStatuses:[],
  });
  patch(row,{id:'native:review:user',role:'user',kind:'message',text:'Review this snippet',nativeTurnTiming:{durationMs:9000}});
  assert.equal(row.matches('.agent-response'),false);
  assert.equal(attributes.has('aria-label'),false);
  assert.equal(role.textContent,'user');
  assert.equal(text.textContent,'Review this snippet');
});

test('shared main/graph controller preserves row, diff state and disclosures across new actions and completion', () => {
  const original={window:globalThis.window,document:globalThis.document,requestAnimationFrame:globalThis.requestAnimationFrame,cancelAnimationFrame:globalThis.cancelAnimationFrame};
  globalThis.window=new EventTarget(); window.localStorage={getItem:()=>null,setItem(){}};
  globalThis.document={documentElement:{style:{setProperty(){}},dataset:{}},createElement:tag=>new Element(tag)};
  globalThis.requestAnimationFrame=()=>1; globalThis.cancelAnimationFrame=()=>{};
  const root=new Element(), keys=new Element();
  const renderer={createMessageRow(message){ const row=new Element('article'); row.className=`chat-message ${message.kind||'message'}`; row.dataset.messageId=String(message.id); row.dataset.activityCategory=message.activityCategory||'tool'; row.dataset.activityStatus=message.activityStatus||'success'; row.textContent=message.text; return row; },patchMessageRow(row,message){row.textContent=message.text;row.dataset.activityStatus=message.activityStatus||'success';}};
  const controller=createAgentTimelineController(root,renderer,{keyboardTarget:keys});
  try {
    const user={id:1,role:'user',text:'Task',runId:8,timestampMs:1000};
    const reasoning={id:'r',role:'assistant',kind:'reasoning',text:'Checking',runId:8,streaming:true};
    const progress={id:'p',role:'assistant',kind:'message',messagePhase:'commentary',text:'Reading files',runId:8};
    const command={id:2,role:'assistant',kind:'activity',activityCategory:'command',activityStatus:'running',text:'Compile',runId:8};
    controller.render([user,reasoning,progress],{active:true,runId:8});
    const reasoningRow=root.querySelectorAll('[data-message-id]').find(row=>row.dataset.messageId==='r');
    const progressRow=root.querySelectorAll('[data-message-id]').find(row=>row.dataset.messageId==='p');
    assert.equal(root.querySelector('.agent-work-group').open,true);
    controller.pauseFollowing();root.scrollTop=80;
    controller.render([user,{...reasoning,text:'Checking files'},progress,command],{active:true,runId:8});
    assert.equal(root.querySelectorAll('[data-message-id]').find(row=>row.dataset.messageId==='r'),reasoningRow);
    assert.equal(root.querySelectorAll('[data-message-id]').find(row=>row.dataset.messageId==='p'),progressRow);
    assert.equal(reasoningRow.textContent,'Checking files');
    assert.equal(root.scrollTop,80);
    assert.equal(root.querySelector('.agent-work-group').open,true);
    controller.render([user,{...reasoning,streaming:false},progress,{...command,activityStatus:'completed'},{id:'final',role:'assistant',text:'Done',messagePhase:'final_answer',runId:8}],{active:false,runId:8});
    assert.equal(root.querySelector('.agent-work-group').open,false);
    assert.equal(root.children.at(-1).dataset.messageId,'final');
    assert.equal(root.querySelectorAll('[data-message-id]').find(row=>row.dataset.messageId==='r'),reasoningRow);
    controller.render([user,command],{active:true,runId:8});
    const row=root.querySelectorAll('[data-message-id]').find(row=>row.dataset.messageId==='2');
    row.editorState={search:'cargo',selection:5};
    root.querySelector('.activity-group').open=true;
    controller.pauseFollowing(); root.scrollTop=120;
    controller.render([user,{...command,text:'Compiled',activityStatus:'success'},{id:3,role:'assistant',kind:'activity',text:'Check',runId:8}],{active:true,runId:8});
    const retained=root.querySelectorAll('[data-message-id]').find(row=>row.dataset.messageId==='2');
    assert.equal(retained,row); assert.equal(retained.textContent,'Compiled'); assert.equal(retained.editorState.search,'cargo');
    assert.equal(root.querySelector('.activity-group').open,true); assert.equal(root.scrollTop,120);
    controller.render([user,{...command,activityStatus:'success'},{id:4,role:'assistant',text:'Done',runId:8,messagePhase:'final_answer'}],{active:false,runId:8});
    assert.equal(root.querySelector('.agent-work-group').open,false);
    assert.equal(root.querySelectorAll('[data-message-id]').find(row=>row.dataset.messageId==='2'),row);
    assert.equal(root.children.at(-1).dataset.messageId,'4');
    const notice={id:'notice',role:'system',kind:'service_notice',provider:'codex_app_server',text:'Account verification required',runId:8};
    controller.render([user,command,notice],{active:true,runId:8});
    const noticeRow=root.children.at(-1);
    assert.equal(noticeRow.dataset.messageId,'notice');
    assert.equal(noticeRow.parent,root);
    controller.render([user,{...command,activityStatus:'success'},notice],{active:false,runId:8});
    assert.equal(root.children.at(-1),noticeRow);
    assert.equal(root.querySelector('.agent-work-group').open,false);
    controller.render([user,{...command,activityStatus:'success'},{id:4,role:'assistant',text:'Done',runId:8,messagePhase:'final_answer'},{...notice,text:'Previously reported verification'}],{active:false,runId:8});
    assert.equal(root.children.at(-1),noticeRow);
    assert.equal(root.children.at(-2).dataset.messageId,'4');
    assert.equal(noticeRow.textContent,'Previously reported verification');
    const image={id:'generated',role:'assistant',kind:'native_media',provider:'codex_app_server',nativeMedia:{state:'ready'},runId:8};
    controller.render([user,command,image],{active:true,runId:8});
    const imageRow=root.children.at(-1);
    assert.equal(imageRow.dataset.messageId,'generated');
    assert.equal(imageRow.parent,root);
    imageRow.previewOpen=true;
    controller.pauseFollowing();root.scrollTop=140;
    controller.render([user,{...command,activityStatus:'completed'},image,{id:4,role:'assistant',text:'Done',runId:8,messagePhase:'final_answer'}],{active:false,runId:8});
    assert.equal(root.children.at(-2),imageRow);
    assert.equal(root.children.at(-1).dataset.messageId,'4');
    assert.equal(imageRow.previewOpen,true);
    assert.equal(root.querySelector('.agent-work-group').open,false);
    assert.equal(root.scrollTop,140);
    const recorded=[{id:'recorded-user',role:'user',provider:'codex_app_server',runId:'recorded',nativeTurnTiming:{durationMs:1649000},nativeWorkHistory:{owner:'chat:recorded',threadId:'thread',turnId:'recorded',state:'loaded'},text:'Recorded task'},
      {id:'recorded-progress',role:'assistant',messagePhase:'commentary',runId:'recorded',text:'Checking'},
      {id:'recorded-final',role:'assistant',messagePhase:'final_answer',runId:'recorded',text:'Done'}];
    const savedNow=Date.now;
    try {
      Date.now=()=>100000;controller.render(recorded,{active:false});
      const stable=root.querySelector('.agent-work-group');stable.open=true;
      Date.now=()=>200000;controller.render(recorded,{active:false});
      assert.equal(root.querySelector('.agent-work-group'),stable,'A local clock tick replaced an unchanged native disclosure');
      assert.equal(stable.open,true);
    } finally { Date.now=savedNow; }
    const key=new Event('keydown',{cancelable:true}); Object.assign(key,{ctrlKey:true,key:'+'}); keys.dispatchEvent(key);
    assert.equal(document.documentElement.dataset.chatZoom,'1');
    controller.destroy(); keys.dispatchEvent(key); assert.equal(document.documentElement.dataset.chatZoom,'1');
  } finally { controller.destroy(); Object.assign(globalThis,original); }
});
