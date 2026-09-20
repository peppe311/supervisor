try {
  const flush = () => new Promise(resolve => setTimeout(resolve, 35));
  const assert = (condition, message) => { if (!condition) throw new Error(message); };
  const root = document.createElement('section'); root.className = 'conversation-section';
  const log = document.createElement('div'); log.className = 'chat-messages'; root.append(log); document.body.append(root);
  const actions = [];
  const capture = event => { if (event.detail?.owner === 'chat:work-fixture') { actions.push(event.detail); event.stopImmediatePropagation(); } };
  document.addEventListener('central-agent:app-server-conversation-control', capture, true);
  const history = {owner:'chat:work-fixture',threadId:'work-thread',turnId:'work-turn',state:'unloaded'};
  const user = {id:'work-user',role:'user',kind:'message',text:'Verify the fork',provider:'codex_app_server',runId:'work-turn',nativeTurnId:'work-turn',nativeTurnTiming:{durationMs:1649000},nativeWorkHistory:history};
  const answer = {id:'work-answer',role:'assistant',kind:'message',messagePhase:'final_answer',runId:'work-turn',text:'The final answer remains visible.'};
  const activity = (id, category, extra={}) => ({id,role:'assistant',kind:'activity',runId:'work-turn',text:id,activityCategory:category,activityStatus:'completed',...extra});
  const checkpoint = (id,text) => ({id,role:'assistant',kind:'message',messagePhase:'commentary',runId:'work-turn',text});
  const controller = window.CentralAgentSvelte.createAgentTimelineController(log, {
    createMessageRow(message) { const row=document.createElement('article'); this.patchMessageRow(row,message); return row; },
    patchMessageRow(row,message) { row.className=`chat-message ${message.kind||'message'} ${message.role||'assistant'}`;row.dataset.messageId=String(message.id);row.dataset.activityCategory=message.activityCategory||'tool';row.dataset.activityStatus=message.activityStatus||'completed';row.textContent=message.text||''; },
  });
  const oldTheme=document.documentElement.dataset.theme;
  try {
    for (const theme of ['central','central_dark']) {
      document.documentElement.dataset.theme=theme;
      for (const width of [480,1920,2560,3840]) {
        root.style.width=`${width}px`;
        controller.render([]); actions.length=0;
        const liveUser={...user,nativeWorkHistory:{...history,state:'live'}};
        const first=checkpoint('live-first','Aggiungo l’apertura di Durata lavoro.');
        const second=checkpoint('live-second','Ho trovato il punto mancante.');
        const third=checkpoint('live-third','I controlli del progetto sono passati.');
        let live=[liveUser,first,activity('live-command-1','command',{activityStatus:'running'})];
        controller.render(live,{active:true,runId:'work-turn'});await flush();
        let runningGroup=log.querySelector('.agent-work-group');
        let workBody=runningGroup.querySelector('.agent-work-group-body');
        const firstRow=workBody.firstElementChild;
        assert(runningGroup.open && getComputedStyle(runningGroup.querySelector('summary')).display==='none','Live commentary is hidden inside a duration disclosure');
        assert(firstRow.textContent===first.text && firstRow.classList.contains('work-progress'),'Live commentary is not ordinary progress text');
        assert(!workBody.querySelector('.activity-group').open,'Live command details opened automatically');
        assert(workBody.querySelector('.activity-group-title').textContent==='Esecuzione di un comando…','Running command claims completion');
        controller.update({id:'live-command-1',activityStatus:'completed'});
        assert(workBody.querySelector('.activity-group-title').textContent==='Comando eseguito','Command completion did not update its own group');
        const openAction=workBody.querySelector('.activity-group');
        openAction.querySelector('summary').focus();openAction.querySelector('summary').click();await flush();
        assert(getComputedStyle(openAction.querySelector('.opened')).display!=='none' && getComputedStyle(openAction.querySelector('.activity-group-chevron')).transform==='none','Open activity arrow is missing or rotated twice');
        const compact=activity('live-compact','compaction',{provider:'codex_app_server',activityStatus:'running',text:'Ottimizzazione della conversazione…'});
        live=[liveUser,first,activity('live-command-1','command'),activity('live-command-2','command'),second,activity('live-file','file_change',{activityFileCount:2}),activity('live-command-3','command'),compact,activity('live-command-4','command'),third];
        controller.pauseFollowing();
        controller.render(live,{active:true,runId:'work-turn'});await flush();
        runningGroup=log.querySelector('.agent-work-group');workBody=runningGroup.querySelector('.agent-work-group-body');
        assert(workBody.firstElementChild===firstRow,'Streaming replaced an existing commentary row');
        assert(workBody.querySelector('.activity-group').open && document.activeElement===workBody.querySelector('.activity-group > summary'),'New activity lost the open group or keyboard focus');
        assert([...workBody.children].map(child=>child.matches('.activity-group')?'actions':child.dataset.messageId).join(',')==='live-first,actions,live-second,actions,live-compact,actions,live-third','Live updates, actions and compaction are out of order');
        assert(workBody.querySelectorAll('.activity-group-title')[0].textContent==='Ha eseguito comandi','Consecutive commands lost their compact summary');
        assert(workBody.querySelectorAll('.activity-group-title')[1].textContent==='Ha modificato file e ha eseguito comandi','File changes are missing from the live action summary');
        const compactRow=workBody.querySelector('.work-event');
        controller.update({...compact,activityStatus:'completed',text:'Conversazione ottimizzata'});
        assert(workBody.querySelector('.work-event')===compactRow && compactRow.textContent==='Conversazione ottimizzata' && !compactRow.closest('.activity-group'),'Compaction is grouped as a tool or duplicated on completion');
        assert(actions.length===0,'Live display triggered history loading or a model action');
        controller.render([{...user,nativeWorkHistory:{...history,state:'loaded'}},...live.slice(1).map(message=>message.id===compact.id?{...compact,activityStatus:'completed',text:'Conversazione ottimizzata'}:message),answer],{active:false});await flush();
        runningGroup=log.querySelector('.agent-work-group');
        assert(!runningGroup.open && getComputedStyle(runningGroup.querySelector('summary')).display!=='none','Completed work did not collapse into duration');
        assert(log.lastElementChild.dataset.messageId==='work-answer','Live-to-completed transition swallowed the final answer');
        controller.render([]); actions.length=0;
        controller.render([user,answer],{active:false});await flush();
        let group=log.querySelector('.agent-work-group');
        assert(group && !group.open,'Imported summary did not create a closed duration disclosure');
        assert(group.querySelector('summary').textContent.includes('Durata lavoro: 27m 29s'),'Recorded duration label changed');
        assert(getComputedStyle(group.querySelector('.closed')).display!=='none' && getComputedStyle(group.querySelector('.opened')).display==='none','Closed arrow state is wrong');
        assert(actions.length===0,'Closed work history fetched details');
        group.querySelector('summary').focus(); group.querySelector('summary').click();await flush();
        assert(group.open && actions.length===1 && actions[0].action.kind==='read_work','Opening did not request exactly this turn');
        assert(actions[0].action.expected_thread_id==='work-thread' && actions[0].action.turn_id==='work-turn','Work scope was lost');
        controller.render([{...user,nativeWorkHistory:{...history,state:'loading'}},answer],{active:false});await flush();
        group=log.querySelector('.agent-work-group');
        assert(group.open && group.querySelector('[role=status]').textContent.includes('Caricamento'),'Loading closed the duration disclosure');
        const full=[{...user,nativeWorkHistory:{...history,state:'loaded'}},activity('command-1','command'),checkpoint('checkpoint-1','La bozza del fork è rimasta intatta.'),activity('command-2','command'),checkpoint('checkpoint-2','La correzione è stata applicata.'),activity('file','file_change',{activityFileCount:1}),activity('command-3','command'),activity('tool','tool',{activityToolName:'open_in_codex'}),answer];
        controller.pauseFollowing();log.scrollTop=0;
        controller.render(full,{active:false});await flush();group=log.querySelector('.agent-work-group');
        assert(group.open,'Loaded details collapsed the user selection');
        assert(getComputedStyle(group.querySelector('.opened')).display!=='none' && getComputedStyle(group.querySelector('.closed')).display==='none','Open arrow state is wrong');
        const body=group.querySelector('.agent-work-group-body');
        assert([...body.children].map(child=>child.matches('.activity-group')?'actions':child.dataset.messageId).join(',')==='actions,checkpoint-1,actions,checkpoint-2,actions','Narrative checkpoints and action groups are out of order');
        assert(body.lastElementChild.querySelector('summary').textContent.includes('Ha modificato un file, ha eseguito comandi e ha usato Open in Codex'),'Native file changes or named tools are missing');
        assert(log.lastElementChild.dataset.messageId==='work-answer' && !group.contains(log.lastElementChild),'Final answer moved inside the disclosure');
        body.firstElementChild.open=true;controller.render(full,{active:false});await flush();
        assert(log.querySelector('.agent-work-group')===group,'Unchanged recorded work replaced its disclosure during a rerender');
        assert(group.querySelector('.activity-group').open,'Nested disclosure state was lost');
        group.querySelector('summary').click();await flush();group.querySelector('summary').click();await flush();
        assert(actions.length===1,'Reopening loaded details repeated the native request');
        assert(group.scrollWidth<=group.clientWidth+1,'Duration disclosure overflows its layout');
        controller.render([{...user,nativeWorkHistory:{...history,state:'error',error:'Connessione interrotta.'}},answer],{active:false});await flush();
        group=log.querySelector('.agent-work-group');
        assert(group.open && group.querySelector('button')?.textContent==='Riprova','Missing inline retry after failed history read');
        group.querySelector('button').click();await flush();assert(actions.length===2,'Retry did not request this same work history');
        controller.render([{...user,nativeWorkHistory:{...history,state:'loaded'}},answer],{active:false});await flush();
        assert(getComputedStyle(log.querySelector('.work-history-empty')).display!=='none','Empty history is not explained');
      }
    }
  } finally {
    controller.destroy();root.remove();document.removeEventListener('central-agent:app-server-conversation-control',capture,true);
    if(oldTheme)document.documentElement.dataset.theme=oldTheme;else delete document.documentElement.dataset.theme;
  }
} catch (error) { startupErrors.push(`Work duration: ${error?.stack || String(error)}`); }
