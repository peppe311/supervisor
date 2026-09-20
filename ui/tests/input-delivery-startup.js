// Shared timeline in the real hidden native WebView; no model or account calls.
try {
  const assert=(condition,message)=>{if(!condition)throw new Error(message);};
  const root=document.createElement('section');root.className='conversation-section';
  const log=document.createElement('div');log.className='chat-messages';root.append(log);document.body.append(root);
  const previousTheme=document.documentElement.dataset.theme;
  const controller=window.CentralAgentSvelte.createAgentTimelineController(log,{
    createMessageRow(message) {
      const row=document.createElement('article');row.className='chat-message user message';row.dataset.messageId=message.id;
      const text=document.createElement('div');text.className='chat-text';row.append(text);this.patchMessageRow(row,message);return row;
    },
    patchMessageRow(row,message) { row.querySelector('.chat-text').textContent=message.text; },
  });
  try {
    for (const theme of ['central','central_dark']) {
      document.documentElement.dataset.theme=theme;
      for (const width of [280,480,1920,2560,3840]) {
        root.style.width=`${width}px`;controller.render([]);
        const pending={id:'native-input:fixture',role:'user',kind:'message',provider:'codex_app_server',text:'My prompt appears immediately, before a native reply.',submissionStatus:'sending'};
        controller.render([pending],{active:true});
        const row=log.querySelector('.chat-message');
        const status=row.querySelector('.input-delivery-status');
        assert(row.textContent.includes(pending.text) && status?.textContent==='Sending…','Prompt or sending state waits for a native item');
        assert(row.getBoundingClientRect().width>0 && status.getBoundingClientRect().height>0,'Pending prompt has no visible layout');
        assert(row.scrollWidth<=row.clientWidth+1,'Pending prompt overflows at this width');
        controller.render([{...pending,submissionStatus:'accepted'}],{active:true});
        assert(log.querySelectorAll('.chat-message').length===1 && log.querySelector('.chat-message')===row,'Acceptance removed or duplicated the pending bubble');
        assert(!row.querySelector('.input-delivery-status'),'Acceptance retained a sending indicator');
        controller.render([{...pending,submissionStatus:undefined,nativeTurnId:'turn',runId:'native:thread:turn'}],{active:true});
        assert(log.querySelectorAll('.chat-message').length===1 && !log.querySelector('.input-delivery-status'),'Native echo duplicated the bubble');
        controller.render([]);
        assert(!log.childElementCount,'Rejected input left a ghost bubble');
      }
    }
  } finally {
    controller.destroy();root.remove();
    if(previousTheme)document.documentElement.dataset.theme=previousTheme;else delete document.documentElement.dataset.theme;
  }
} catch (error) { startupErrors.push(`Input delivery: ${error?.stack || String(error)}`); }
