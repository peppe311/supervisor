// Hidden disposable WebView; synthetic IPC only, no provider inference or user data.
{
  const bridge=window.chrome.webview,post=bridge.postMessage,sent=[];
  const flush=async()=>{await Promise.resolve();await Promise.resolve();await new Promise(resolve=>requestAnimationFrame(resolve));await Promise.resolve();};
  const assert=(condition,message)=>{if(!condition)throw new Error(message);};
  const work=kind=>sent.filter(message=>message.message_type==='project_board'&&message.action?.type==='work'&&message.action.action.kind===kind);
  const reply=(request,result,error)=>window.dispatchEvent(new CustomEvent('central-agent:work-results',{detail:{requestId:request.action.request_id,result,error}}));
  const notify=detail=>window.dispatchEvent(new CustomEvent('central-agent:app-server-conversation',{detail}));
  try {
    bridge.postMessage=raw=>sent.push(JSON.parse(raw));
    for(const theme of ['central','central_dark']) {
      const root=`C:/fixture/results-${theme}`,nodeKey=`entity:results-${theme}`,chatId=`result-a-${theme}`,owner=`chat:${chatId}`,other=`chat:result-b-${theme}`,reviewer=`graph:conversation:result-reviewer-${theme}`;
      const state={theme,expanded:true,graph:{visualGraph:{nodes:[],edges:[]}},projectRegistry:{projects:[{id:root,path:root,nodeKey,source:'local',name:'Work results',active:true}]},
        projectChats:[{id:chatId,title:'Attempt A',projectRoot:root},{id:other.slice(5),title:'Attempt B',projectRoot:root}],projectChatPreviews:[],knowledgeAgents:{bindings:[],activeNodeKeys:[]},knowledgeRuns:[],workspace:{root,entries:[]}};
      window.renderKnowledgeSurfaceState(state);await flush();
      const report={owner,title:'Attempt A',root,remote:false,busy:false,loaded:true,partial:false,status:'completed',revision:'revision-a',native:true,threadId:'thread-a',turnId:'turn-a',canHandoff:true,
        evidence:{objective:'Build search',finalAnswer:'Search works; one test still fails.',events:[{role:'assistant',kind:'activity',phase:null,text:'Changed search',category:'file',status:'completed',detail:'',diff:'-old\n+new',attachments:[{name:'design.png',kind:'image'}]}],eventsOmitted:2},
        commands:[{command:'test search',status:'failed',exitCode:1,output:'One assertion failed'},{command:'unknown outcome',status:'not_reported',exitCode:null,output:''}],files:[{path:'src/search.ts',state:'done',operation:'edit'}],diff:{additions:12,deletions:4,fileCount:1},checks:[{step:'Verify search',status:'pending'}],pendingRequests:0,assessment:null};
      const candidates=[{owner:other,title:'Attempt B',busy:false,reviewer:false,threadId:'thread-b'},{owner:reviewer,title:'Reviewer',busy:false,reviewer:true,threadId:'thread-reviewer'}];
      const open=async (mode,answer=true)=>{
        document.querySelector(`[data-chat-id="${chatId}"] .workspace-row-menu`).click();await flush();
        document.querySelector(`[data-work-action="${mode}"]`).click();await flush();
        const request=work('inspect').at(-1);assert(request?.action.action.owner===owner,`${theme}: work menu lost its source owner`);
        if(answer){reply(request,{report,candidates});await flush();}
        return document.querySelector('dialog.work-dialog');
      };
      sent.length=0;let dialog=await open('summary',false);
      assert(dialog.textContent.includes('Loading recorded work'),`${theme}: pending work is not identified`);
      reply(work('inspect').at(-1),null,'Recorded work is temporarily unavailable.');await flush();
      assert(dialog.querySelector('[role="alert"]')?.textContent.includes('temporarily unavailable')&&!dialog.textContent.includes('Loading recorded work'),`${theme}: a failed inspection remained stuck on Loading`);
      dialog.querySelector('footer button').click();await flush();
      reply(work('inspect').at(-1),{report,candidates});await flush();
      assert(dialog.open&&dialog.textContent.includes('One assertion failed')&&dialog.textContent.includes('Exit not reported')&&dialog.textContent.includes('design.png'),`${theme}: report lost failure, missing outcome or attachment context`);
      assert(!work('compare').length&&!work('handoff').length,`${theme}: opening evidence dispatched an inference or fork`);
      for(const width of [1024,1920,2560,3840]){
        document.getElementById('graph-shell').style.width=`${width}px`;window.dispatchEvent(new Event('resize'));await flush();
        const bounds=dialog.getBoundingClientRect();
        assert(bounds.left>=0&&bounds.top>=0&&bounds.right<=innerWidth+1&&bounds.bottom<=innerHeight+1&&dialog.scrollWidth<=dialog.clientWidth+1,`${theme}/${width}: work summary escaped its viewport`);
      }
      document.getElementById('graph-shell').style.width='';dialog.close();await flush();
      dialog=await open('compare');
      document.getElementById('work-other-picker-button').click();await flush();
      document.querySelectorAll('#work-other-picker-menu [role="option"]')[1].click();await flush();
      const requestB=work('inspect').at(-1);assert(requestB.action.action.owner===other,`${theme}: comparison loaded the wrong attempt`);
      reply(requestB,{report:{...report,owner:other,title:'Attempt B',revision:'revision-b'},candidates:[]});await flush();
      document.getElementById('work-reviewer-picker-button').click();await flush();
      [...document.querySelectorAll('#work-reviewer-picker-menu [role="option"]')].find(option=>option.textContent.includes('Reviewer')).click();await flush();
      const ask=dialog.querySelector('[aria-label="Ask Supervisor to compare attempts"]');
      assert(!ask.disabled&&dialog.querySelectorAll('[data-work-evidence]').length===2,`${theme}: loaded comparison is unavailable`);
      sent.length=0;ask.click();ask.click();await flush();
      const comparisons=work('compare');assert(comparisons.length===1,`${theme}: double click submitted more than one comparison`);
      const comparison=comparisons[0].action.action;
      assert(comparison.reviewer===reviewer&&comparison.reviewer_thread==='thread-reviewer'&&comparison.revision==='revision-a'&&comparison.other_revision==='revision-b',`${theme}: compare lost the confirmed revisions or reviewer identity`);
      reply(comparisons[0],null,'The work changed. Refresh the summary and confirm again.');await flush();
      assert(dialog.querySelector('[role="alert"]')?.textContent.includes('work changed')&&work('compare').length===1,`${theme}: rejected comparison disappeared or retried itself`);
      dialog.close();await flush();dialog=await open('handoff');
      const create=dialog.querySelector('[aria-label="Create handoff"]');
      sent.length=0;create.click();create.click();await flush();
      const handoffs=work('handoff');assert(handoffs.length===1&&handoffs[0].action.action.revision==='revision-a'&&handoffs[0].action.action.instruction.includes('original objective'),`${theme}: handoff duplicated or lost its continuation`);
      const destination=`chat:new-${theme}`;reply(handoffs[0],{handoff:true,destination,source:owner});await flush();
      const openNew=[...dialog.querySelectorAll('button')].find(button=>button.textContent==='Open new agent');
      assert(openNew.disabled,`${theme}: handoff opened before native acknowledgement`);
      notify({owner:destination,change:'forked'});await flush();
      assert(!openNew.disabled,`${theme}: native acknowledgement did not enable the continuation`);
      openNew.click();await flush();
      assert(work('open').at(-1)?.action.action.owner===destination&&!dialog.open,`${theme}: handoff did not open its exact destination in the board`);
      dialog=await open('summary');
      const stale=work('inspect').at(-1);
      window.renderKnowledgeSurfaceState({...state,expanded:false});await flush();reply(stale,{report,candidates});await flush();
      assert(!dialog.open,`${theme}: a late report reopened a closed project board`);
    }
  }catch(error){startupErrors.push(`Work results: ${String(error)}`);}
  finally{document.querySelector('dialog.work-dialog')?.close();document.getElementById('graph-shell').style.width='';window.renderKnowledgeSurfaceState({expanded:false,graph:{visualGraph:{nodes:[],edges:[]}}});await flush();bridge.postMessage=post;}
}
