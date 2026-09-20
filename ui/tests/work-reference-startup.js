// Redacted version of the user's 13m 29s reference, projected by the actual
// Rust mirror/presentation and rendered by the real main or graph message view.
try {
  const assert=(condition,message)=>{if(!condition)throw new Error(message);};
  const flush=()=>new Promise(resolve=>setTimeout(resolve,35));
  const root=document.createElement('section');root.className='conversation-section';document.body.append(root);
  const isMain=typeof window.createChatMessageRow==='function';
  let log,controller,popup;
  const owner='graph:work-reference-root';
  const hooks={decorate:()=>{},artifacts:()=>document.createElement('div'),checkpoint:()=>document.createElement('div')};
  const actions=[];
  const capture=event=>{if(event.detail?.owner==='chat:work-reference'){actions.push(event.detail);event.stopImmediatePropagation();}};
  document.addEventListener('central-agent:app-server-conversation-control',capture,true);
  const oldTheme=document.documentElement.dataset.theme;
  try {
    if(isMain) {
      log=document.createElement('div');log.className='chat-messages';root.append(log);
      controller=window.CentralAgentSvelte.createAgentTimelineController(log,{createMessageRow:window.createChatMessageRow,patchMessageRow:window.patchChatMessageRow});
    } else {
      popup=document.createElement('div');popup.dataset.nodeKey='work-reference-root';root.append(popup);
      window.CentralAgentSvelte.mountKnowledgeAgentPopup(popup);await flush();
      log=popup.querySelector('.graph-timeline');assert(log,'Real graph timeline did not mount');
    }
    const render=(messages,active=false)=>{
      if(controller)controller.render(messages,{active,runId:'local-wrapper-run'});
      else popup.dispatchEvent(new CustomEvent('central-agent:graph-timeline',{detail:{owner,hooks,run:{conversationState:{nativeMessages:messages},agent:{active,runId:'local-wrapper-run'}}}}));
    };
    const byId=new Map(workReferenceMessages.map(message=>[String(message.id),message]));
    const contentEdges=element=>{
      const bounds=element.getBoundingClientRect(),style=getComputedStyle(element);
      const left=bounds.left+element.clientLeft+parseFloat(style.paddingLeft);
      const right=bounds.left+element.clientLeft+element.clientWidth-parseFloat(style.paddingRight);
      return {left,right,width:right-left};
    };
    const luminance=color=>{
      const channels=color.match(/[\d.]+/g).slice(0,3).map(value=>{
        const channel=Number(value)/255;return channel<=0.04045?channel/12.92:((channel+0.055)/1.055)**2.4;
      });
      return channels[0]*0.2126+channels[1]*0.7152+channels[2]*0.0722;
    };
    const assertBubble=(row,label)=>{
      assert(row,`${label}: user message is missing`);
      const bounds=row.getBoundingClientRect(),edges=contentEdges(row.parentElement),style=getComputedStyle(row);
      assert(bounds.width>0 && bounds.width<=edges.width*0.8+1,`${label}: prompt expanded across the transcript (${bounds.width}/${edges.width})`);
      assert(Math.abs(bounds.right-edges.right)<=1 && bounds.left>edges.left+1,`${label}: prompt is not aligned to the right`);
      assert(parseFloat(style.borderRadius)>0 && parseFloat(style.paddingLeft)>0,`${label}: user bubble lost its shape`);
      assert(style.backgroundColor!=='rgba(0, 0, 0, 0)' && row.scrollWidth<=row.clientWidth+1,`${label}: bubble is transparent or overflows`);
      const foreground=luminance(style.color),background=luminance(style.backgroundColor);
      assert((Math.max(foreground,background)+0.05)/(Math.min(foreground,background)+0.05)>=4.5,`${label}: insufficient text contrast`);
    };
    const assertAssistant=(row,user)=>{
      const bounds=row.getBoundingClientRect(),edges=contentEdges(row.parentElement),style=getComputedStyle(row);
      assert(Math.abs(bounds.left-edges.left)<=1 && Math.abs(bounds.width-edges.width)<=1,'Agent prose no longer starts at the left transcript edge');
      assert(style.backgroundColor!==getComputedStyle(user).backgroundColor && parseFloat(style.borderWidth)===0,'Agent prose uses the same bubble as the user');
    };
    for(const theme of ['central','central_dark']) {
      document.documentElement.dataset.theme=theme;
      for(const width of [320,384,480,887,1920,2560,3840]) {
        root.style.width=`${width}px`;render([]);render(workReferenceMessages);await flush();
        const prompt=log.querySelector('.chat-message.user.message');
        assertBubble(prompt,`${theme}/${width} prompt`);
        assert(log.querySelectorAll('.agent-work-group').length===1,'The answer to a question split one native turn into multiple duration sections');
        const group=log.querySelector('.agent-work-group');
        assert(group.querySelector('summary').textContent.includes('Durata lavoro: 13m 29s'),'Duration rounded up instead of showing elapsed whole seconds');
        group.querySelector('summary').click();await flush();
        const body=group.querySelector('.agent-work-group-body');
        const outline=[...body.children].map(row=>row.matches('.activity-group')?'actions':row.matches('.native-question-reply')?'reply':byId.get(row.dataset.messageId)?.text);
        assert(JSON.stringify(outline)===JSON.stringify([
          'Aggiornamento 1: sto verificando i file.','actions','Conservare i file selezionati?',
          'Aggiornamento 2: sto verificando i file.','actions','reply',
          'Aggiornamento 3: sto verificando i file.','actions',
          'Aggiornamento 4: sto verificando i file.','Aggiornamento 5: sto verificando i file.','actions',
        ]),'Reference outline changed: '+JSON.stringify(outline));
        const groups=[...body.querySelectorAll('.activity-group')];
        assert(groups.length===4 && groups.every(activity=>!activity.open),'Technical summaries fragmented the four compact activity groups');
        assert(groups[0].querySelector('.activity-group-title').textContent==='Ha eseguito comandi','Initial commands were not grouped');
        assert(groups[1].querySelector('.activity-group-title').textContent==='Comando eseguito','Standalone command has the wrong label');
        assert(groups[3].querySelector('.activity-group-title').textContent==='Ha eseguito comandi e ha cercato sul web','Web search was reduced to an unnamed tool');
        assert(groups.every(activity=>activity.querySelector('.activity-icon')),'Activity identity icon is missing');
        const reasoning=[...body.querySelectorAll('.chat-message.reasoning')];
        assert(reasoning.length===workReferenceMessages.filter(message=>message.kind==='reasoning').length && reasoning.every(row=>row.closest('.activity-group')),'Public summaries were discarded or remain expanded in the narrative');
        const reply=body.querySelector('.native-question-reply');
        assert(reply?.querySelector('.question').textContent==='Conservare i file selezionati?' && reply.querySelector('.answer').textContent==='Conserva i file','Answered question is not readable as a quoted reply');
        const replyBounds=reply.getBoundingClientRect(),bodyBounds=body.getBoundingClientRect(),replyStyle=getComputedStyle(reply);
        assert(replyBounds.width < bodyBounds.width && replyBounds.right<=bodyBounds.right+1,`Quoted reply does not fit its conversation: reply=${replyBounds.left}/${replyBounds.right}/${replyBounds.width}, body=${bodyBounds.left}/${bodyBounds.right}/${bodyBounds.width}, css=${replyStyle.width}/${replyStyle.maxWidth}`);
        assertBubble(reply,`${theme}/${width} quoted answer`);
        assert(getComputedStyle(reply).backgroundColor===getComputedStyle(prompt).backgroundColor,'Quoted answer does not share the user prompt treatment');
        assert(!log.textContent.includes('send_user_message_question_reply') && !log.textContent.includes('PRIVATE_FIXTURE_REASONING'),'Raw envelope or private reasoning reached the reference conversation');
        assert(log.lastElementChild.dataset.messageId===workReferenceMessages.at(-1).id && !group.contains(log.lastElementChild),'The real final answer moved inside the work details');
        assertAssistant(log.lastElementChild,prompt);
        assert(group.scrollWidth<=group.clientWidth+1,'Reference conversation overflows its width');
        groups[0].querySelector('summary').click();await flush();
        const stableReply=reply;
        render(workReferenceMessages);await flush();
        assert(log.querySelector('.agent-work-group')===group && group.querySelector('.activity-group').open && log.querySelector('.native-question-reply')===stableReply,'Unchanged real history replaced the open group or quoted reply');
        const live=workReferenceMessages.filter(message=>message.messagePhase!=='final_answer').map(message=>message.nativeWorkHistory?{...message,nativeWorkHistory:{...message.nativeWorkHistory,state:'live'}}:message);
        render(live,true);await flush();
        assert(log.querySelector('.chat-message.user.message')===prompt,'Streaming replaced the user prompt');
        assertBubble(prompt,`${theme}/${width} live prompt`);
        assert(log.querySelectorAll('.agent-work-group').length===1 && log.querySelector('.agent-work-group').dataset.live==='true' && getComputedStyle(log.querySelector('.agent-work-group > summary')).display==='none','An in-turn user reply or local graph run ID ended live presentation early');
        render(workReferenceMessages);await flush();
        assert(!log.querySelector('.agent-work-group').open,'Native turn completion did not collapse its entire work');
        const variants=[
          {id:'bubble-short',role:'user',kind:'message',text:'Procedi'},
          {id:'bubble-long',role:'user',kind:'message',text:`Controlla questo riferimento: ${'riferimento'.repeat(24)}\nMantieni anche questa seconda riga.`},
          {id:'bubble-files',role:'user',kind:'message',text:'',fileAttachments:[{id:'fixture-file',name:`${'documentazione_'.repeat(16)}.md`,kind:'text',byteCount:1024}]},
          {id:'bubble-assistant',role:'assistant',kind:'message',messagePhase:'final_answer',text:'Sto verificando i file.',streaming:true},
        ];
        render(variants,true);await flush();
        const variantRows=[...log.querySelectorAll('.chat-message.user.message')];
        assert(variantRows.length===3,'A short, long or attachment-only prompt is missing');
        variantRows.forEach(row=>assertBubble(row,`${theme}/${width} ${row.dataset.messageId}`));
        assert(variantRows[1].querySelector('.chat-text').textContent.includes('\n'),'Multiline prompt lost its line break');
        assert(variantRows[2].textContent.includes(variants[2].fileAttachments[0].name),'Attachment-only prompt lost its file');
        assert(variantRows[0].getBoundingClientRect().width<variantRows[1].getBoundingClientRect().width,'A short prompt is stretched to the long prompt width');
        assertAssistant(log.querySelector('.chat-message.assistant.message'),variantRows[0]);
        render(variants.map(message=>({...message,streaming:false})));await flush();
        variantRows.forEach(row=>{
          assert(log.contains(row),'Finishing work replaced an existing user row');
          assertBubble(row,`${theme}/${width} restored prompt`);
        });
      }
    }
    assert(actions.length===0,'Reference rendering invoked an unexpected native action');
  } finally {
    controller?.destroy();if(popup)window.CentralAgentSvelte.unmountKnowledgeAgentPopup(popup);
    root.remove();document.removeEventListener('central-agent:app-server-conversation-control',capture,true);
    if(oldTheme)document.documentElement.dataset.theme=oldTheme;else delete document.documentElement.dataset.theme;
  }
} catch(error) {startupErrors.push(`Work reference: ${error?.stack || String(error)}`);}
