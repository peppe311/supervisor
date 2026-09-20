// Production Svelte in the embedded WebView. Synthetic owner events only;
// the startup IPC sink does not dispatch any native operation.
try {
  const flush=async()=>{await Promise.resolve();await Promise.resolve();await Promise.resolve();};
  const assert=(condition,message)=>{if(!condition)throw new Error(message);};
  for(const font of ['450 14px Inter','650 12px Inter']) {
    const faces=await document.fonts.load(font,'Supervisor · lucidità è precisione 0123456789');
    assert(faces.length>0 && faces.every(face=>face.status==='loaded'),`Embedded interface font failed: ${font}`);
  }
  const graphSurface=document.documentElement.dataset.configurationProbeSurface==='graph';
  const popup=graphSurface?document.createElement('div'):null;
  if(popup){
    popup.dataset.nodeKey='configuration-probe';popup.className='agent-console';document.body.append(popup);window.CentralAgentSvelte.mountKnowledgeAgentPopup(popup);
    popup.dispatchEvent(new CustomEvent('central-agent:conversation-state',{detail:{owner:'graph:configuration-probe',nativeConversation:{visible:true,connected:true,busy:false,observed:true,binding:null,nativeLoaded:false,apps:null}}}));
  }
  await flush();
  assert(document.title.startsWith('Supervisor'),'The native host still shows the retired product name');
  const brandHost=document.querySelector(graphSurface?'[data-graph-launcher-icon]':'#tetra-config-visual');
  const brand=brandHost?.querySelector('[data-supervisor-logo]');
  assert(brand && brand.querySelectorAll('path').length===2,'The Supervisor launcher logo is missing');
  const activityBrand=window.CentralAgentSvelte.createSupervisorMark();
  assert(activityBrand.outerHTML===brand.outerHTML,'App and activity controls use different logo geometry');
  assert(document.querySelectorAll('[data-tetra-piece]').length===0,'The retired tetrahedron is still mounted');
  const owner=popup?'graph:configuration-probe':'chat:configuration-probe';
  const target=popup||window,root=popup||document.getElementById('agent-settings-host');
  if(popup)assert(popup.querySelector('[data-plugin-picker="graph-configuration-probe"] .plugin-picker-trigger'),'Graph plugin selector missed the conversation state emitted immediately after mounting');
  const profileRoot=document.getElementById('tetra-config-menu');
  const launcher=document.getElementById('tetra-config-button');
  if(!popup){window.dispatchEvent(new CustomEvent('central-agent:conversation-key',{detail:owner}));launcher.disabled=false;if(profileRoot.hidden)launcher.click();}
  const view={visible:true,connected:true,busy:false,observed:true,name:'Central Agent',directory:'C:\\fixture',binding:{threadId:'configuration-probe',archived:false,deleted:false},nativeLoaded:true,canCompact:true,readyForTurn:true,goal:{busy:false,current:true,viewId:'goal-probe',goal:null},completedTurns:[]};
  const accessOptions=[{value:'readOnly',label:'Read only',allowed:true},{value:'workspaceWrite',label:'Project access',allowed:true},{value:'fullAccess',label:'Full access',allowed:true}];
  const emit=(patch={},accessPatch={},usagePatch={})=>target.dispatchEvent(new CustomEvent('central-agent:conversation-state',{detail:{owner,nativeUsage:{visible:true,connected:true,current:true,report:null,...usagePatch},nativeAccess:{visible:true,selected:'readOnly',summary:'auto',disabled:false,permissionsDisabled:false,options:accessOptions,...accessPatch},nativeConversation:{...view,...patch}}}));
  emit();await flush();
  const groups=[...root.querySelectorAll('[data-config-section]')];
  const toggles=groups.map(group=>group.querySelector('.section-toggle')).filter(Boolean);
  const contents=groups.map(group=>group.querySelector('.configuration-content')).filter(Boolean);
  if(popup) {
    const oldTheme=document.documentElement.dataset.theme;
    popup.querySelector('.agent-console-name').textContent='Central Agent';
    popup.querySelector('[data-role="name"]').value='Central Agent';
    const phase=popup.querySelector('.agent-console-phase');phase.textContent='Setup';phase.className='agent-console-phase';
    const profileToggle=popup.querySelector('.agent-profile-toggle');
    const minimizeButton=popup.querySelector('[data-action="minimize"]');
    const profilePanel=popup.querySelector('.agent-console-profile-controls');
    const profileMark=profileToggle.querySelector('[data-supervisor-logo]');
    const input=popup.querySelector('[data-role="message"]');
    const composerState=(canSubmit,patch={})=>popup.dispatchEvent(new CustomEvent('central-agent:graph-composer-state',{detail:{owner,canSubmit,canAttachFiles:canSubmit,...patch}}));
    const attach=popup.querySelector('.attach-file');
    const sendButton=popup.querySelector('[data-action="submit"]'),stopButton=popup.querySelector('[data-action="stop"]');
    const profile=[...popup.querySelectorAll('.graph-profile-fields select')];
    const compatibility=popup.querySelector('.agent-console-compatibility');
    assert(compatibility?.hidden && compatibility.contains(popup.querySelector('[data-action="save"]')) && compatibility.contains(popup.querySelector('[data-role="mission"]')),'Assignment compatibility hooks are missing or mixed with live controls');
    profile.forEach(select=>{select.add(new Option('Selected profile value',select.dataset.role));select.value=select.dataset.role;});
    assert(minimizeButton?.nextElementSibling===profileToggle&&profileToggle.nextElementSibling===popup.querySelector('[data-action="close"]'),'Minimize, profile and Close controls are not ordered consistently');
    assert(profileMark?.querySelectorAll('path').length===2,'Graph profile control does not use the Supervisor mark');
    assert(profileToggle.getAttribute('aria-expanded')==='false' && profilePanel.hidden,'Profile menu does not start closed');
    const compactUsage=popup.querySelector('[data-native-usage="compact"]');
    assert(compactUsage?.getClientRects().length && compactUsage.textContent.replace(/\s/g,'')==='Context—','Graph context monitor does not start as a quiet compact signal');
    assert(!/Waiting for usage|Waiting for the first native token-usage event\./.test(popup.innerText),'Compact context monitor exposes native waiting prose');
    profileToggle.click();await flush();
    const openingMotions=[...profilePanel.getAnimations(),...[...profileMark.querySelectorAll('path')].flatMap(path=>path.getAnimations())];
    if(!matchMedia('(prefers-reduced-motion: reduce)').matches)assert(openingMotions.length>=3,'Graph profile mark and panel do not animate together');
    profileToggle.click();await flush();
    assert(profilePanel.inert && profilePanel.getAttribute('aria-hidden')==='true','Closing profile remains interactive during its exit');
    for(const animation of [...profilePanel.getAnimations(),...[...profileMark.querySelectorAll('path')].flatMap(path=>path.getAnimations())])animation.finish();
    await flush();
    assert(profilePanel.hidden,'Graph profile exit did not hide after recomposing the mark');
    popup.style.setProperty('--ca-profile-motion-duration','0ms');
    const pluginControls=[];
    const pluginControl=event=>{if(event.detail?.action?.kind==='apps_control'){pluginControls.push(event.detail);event.preventDefault();event.stopImmediatePropagation();}};
    popup.addEventListener('central-agent:app-server-conversation-control',pluginControl,true);
    const pluginApp={id:'github',name:'GitHub',description:'Repositories',iconUrl:null,accessible:true,configuredEnabled:true,installed:true,runtimeEnabled:true,callable:true,tools:[]};
    const pluginApps=selected=>({threadId:'configuration-probe',inventoryId:'graph-plugin-inventory',items:[pluginApp],selected,current:true,busy:false,error:null,unavailableReason:null,notice:null});
    emit({apps:pluginApps([])});await flush();
    const pluginButton=popup.querySelector('[data-plugin-picker="graph-configuration-probe"] .plugin-picker-trigger');
    assert(pluginButton && !pluginButton.disabled,'Graph composer does not expose its owner-scoped plugin picker');
    pluginButton.click();await flush();popup.querySelector('.plugin-choice[data-plugin-id="github"]').click();await flush();
    assert(pluginControls.at(-1)?.owner===owner && pluginControls.at(-1)?.action?.action?.kind==='attach' && pluginControls.at(-1)?.action?.action?.inventory_id==='graph-plugin-inventory','Graph plugin selection was not routed to this card');
    const selectedPlugin={selectionId:'graph-github',appId:'github',name:'GitHub',iconUrl:null,threadId:'configuration-probe',current:true};
    emit({apps:pluginApps([selectedPlugin])});await flush();
    const pluginChip=popup.querySelector('.selected-plugin-chip[data-plugin-id="github"]'),removePlugin=pluginChip?.querySelector('.selected-plugin-remove');
    const controlsBeforeChip=pluginControls.length;pluginChip.click();await flush();assert(pluginControls.length===controlsBeforeChip,'Clicking a graph plugin chip removed it without its X');
    removePlugin.click();await flush();assert(pluginControls.at(-1)?.action?.action?.kind==='remove','Graph plugin X did not remove the next-prompt selection');
    popup.removeEventListener('central-agent:app-server-conversation-control',pluginControl,true);
    emit();await flush();
    input.value='Draft retained while minimized';input.dispatchEvent(new Event('input',{bubbles:true}));await flush();
    const expandedCardHeight=popup.getBoundingClientRect().height;
    minimizeButton.click();await flush();emit();composerState(true,{active:true,canStop:true});await flush();
    assert(popup.dataset.cardMinimized==='true'&&minimizeButton.getAttribute('aria-label')==='Restore conversation'&&minimizeButton.getAttribute('aria-pressed')==='true','Conversation card did not enter an accessible minimized state');
    assert(popup.querySelector('.agent-console-header').getClientRects().length&&input.form.getClientRects().length===0&&popup.getBoundingClientRect().height<expandedCardHeight/2,'Minimized conversation still reserves its portrait body');
    assert(input.value==='Draft retained while minimized'&&popup.querySelector('[data-collaboration-active]'),'Minimizing lost the draft or hid active-work state from the header');
    minimizeButton.click();await flush();
    assert(popup.dataset.cardMinimized==='false'&&input.form.getClientRects().length&&input.value==='Draft retained while minimized'&&!stopButton.hidden,'Restoring a minimized conversation lost its draft or active controls');
    input.value='';input.dispatchEvent(new Event('input',{bubbles:true}));composerState(false);await flush();
    for(const theme of ['central','central_dark']) {
      document.documentElement.dataset.theme=theme;
      for(const [width,height] of [[480,720],[720,480],[1920,1080],[2560,1440],[3840,2160]]) {
        popup.style.setProperty('--agent-console-available-width',`${width}px`);
        popup.style.setProperty('--agent-console-available-height',`${height}px`);emit();composerState(false);await flush();
        const bounds=popup.getBoundingClientRect(),promptBounds=input.form.getBoundingClientRect();
        assert(Math.abs(bounds.height/bounds.width-16/9)<.005 && bounds.height<=height+1 && bounds.width<=width+1,'Agent card does not fit its available space in 9:16 portrait');
        assert(Math.abs(bounds.bottom-promptBounds.bottom)<=2,'Prompt is not anchored at the bottom of its card');
        assert(profile.every(select=>!select.getClientRects().length),'Closed profile selectors remain visible');
        assert(!/Setup|Connect|Profile changes apply to your next message\./.test(popup.innerText),'Removed card chrome is still visible');
        const field=input.closest('.graph-prompt-field').getBoundingClientRect(),attachBounds=attach.getBoundingClientRect();
        assert(attachBounds.width>0 && attachBounds.left>=field.left && attachBounds.right<=field.right && attachBounds.top>=field.top && attachBounds.bottom<=field.bottom,'Attachment plus is not inside the prompt field');
        const checkWorkButton=button=>{const bounds=button.getBoundingClientRect(),rightGap=field.right-bounds.right,bottomGap=field.bottom-bounds.bottom;assert(bounds.width>=28 && bounds.width<=36 && rightGap>=0 && bottomGap>=0 && rightGap<=12 && bottomGap<=12 && Math.abs(rightGap-bottomGap)<=2 && bounds.left>attachBounds.right,'Work button is not compact with equal right and bottom prompt insets');assert(getComputedStyle(button).color!==getComputedStyle(button).backgroundColor,'Work icon has no theme contrast');};
        checkWorkButton(sendButton);assert(sendButton.disabled && stopButton.hidden,'Unavailable composer shows an enabled action');
        composerState(true,{active:true,canStop:true});await flush();checkWorkButton(stopButton);assert(sendButton.hidden && !stopButton.disabled,'Running work does not expose Stop');
        assert(popup.querySelector('.agent-console-header [data-collaboration-active]'),'An active conversation card does not expose collaboration');
        composerState(true,{canResume:true});await flush();checkWorkButton(sendButton);assert(sendButton.dataset.workAction==='resume' && !sendButton.disabled && stopButton.hidden,'Stopped work does not expose Resume');
        assert(!popup.querySelector('.agent-console-header [data-collaboration-active]'),'A stopped conversation card kept its collaboration indicator');
        composerState(false);await flush();
        assert(attach.disabled,'Disconnected or unavailable profile can select files');
        const logTop=popup.querySelector('.graph-timeline').getBoundingClientRect().top;
        profileToggle.click();await flush();
        const visibleSelects=[...popup.querySelectorAll('select')].filter(select=>select.getClientRects().length);
        assert(visibleSelects.map(select=>select.dataset.role).join(',')==='provider,model,effort,speed','Agent card exposes more than its four profile choices');
        assert(!popup.innerText.includes('Central Agent'),'Agent card repeats the old name or assignment metadata');
        const monitor=popup.querySelector('[data-native-usage="compact"]');
        assert(monitor?.getClientRects().length && monitor.textContent.replace(/\s/g,'')==='Context—' && !monitor.querySelector('details,table'),'Agent card context signal is missing, verbose or not compact');
        emit({}, {}, {turnId:'turn-usage',activeTurnId:'turn-usage',report:{last:{totalTokens:'25'},total:{totalTokens:'125'},modelContextWindow:'100'}});await flush();
        assert(monitor.textContent.includes('25%') && monitor.querySelector('[role="progressbar"]')?.getAttribute('aria-valuenow')==='25','Agent card context signal does not use the latest report and capacity');
        assert(popup.querySelector('[data-role="context-monitor"]').getClientRects().length===0 && !/Context · native report|Waiting for usage|Waiting for the first native token-usage event\.|Token details|Attach tab or shell/.test(popup.innerText),'Agent card exposes legacy context chrome, detailed counters or the tab/shell attachment menu');
        assert(toggles.length===0,'Agent card still exposes the advanced configuration menus');
        assert(compatibility.getClientRects().length===0 && [...compatibility.querySelectorAll('input,select,button')].every(control=>!control.getClientRects().length),'Retired controls still reserve layout or enter the visible profile');
        assert(profile.every(select=>select.value===select.dataset.role && select.closest('.graph-profile-fields').scrollWidth<=select.closest('.graph-profile-fields').clientWidth+1),'A native update loses a profile choice or clips the compact profile');
        emit();await flush();assert(!profilePanel.hidden && Math.abs(logTop-popup.querySelector('.graph-timeline').getBoundingClientRect().top)<1,'Opening or updating the profile moved the transcript or closed the menu');
        profile[0].dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true,cancelable:true}));await flush();
        assert(profilePanel.hidden && document.activeElement===profileToggle,'Escape did not close the profile and return keyboard focus');
        profileToggle.click();await flush();document.body.dispatchEvent(new PointerEvent('pointerdown',{bubbles:true}));await flush();assert(profilePanel.hidden,'An outside click did not close the profile');
      }
    }
    popup.dispatchEvent(new CustomEvent('central-agent:slash-ui',{detail:{owner,action:'model'}}));await flush();
    assert(!profilePanel.hidden && document.activeElement===profile[1],'Model shortcut focused a hidden selector');
    profileToggle.click();await flush();
    const submissions=[];const capture=event=>submissions.push(event.detail);
    popup.addEventListener('central-agent:graph-submit',capture);
    input.value='Keyboard submission probe';input.dispatchEvent(new Event('input',{bubbles:true}));
    const enter=(fields={})=>input.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true,cancelable:true,...fields}));
    enter();await flush();assert(!submissions.length && input.value==='Keyboard submission probe','Unavailable profile submitted or lost the draft');
    composerState(true);await flush();enter({shiftKey:true});enter({isComposing:true});enter({repeat:true});await flush();assert(!submissions.length,'Editing or repeated Enter submitted a prompt');
    enter();await flush();enter();await flush();assert(submissions.length===1 && submissions[0].owner===owner && submissions[0].input===input.value,'Keyboard submission was lost, duplicated or misrouted');
    popup.dispatchEvent(new CustomEvent('central-agent:submission-accepted',{detail:{owner,input:input.value}}));await flush();assert(input.value==='','Accepted keyboard submission did not clear its exact draft');
    enter();await flush();assert(submissions.length===1,'Empty Enter started agent work');
    const fileActions=[];const captureFile=event=>{fileActions.push(event.detail);event.stopPropagation();};
    popup.addEventListener('central-agent:graph-files',captureFile);
    attach.click();await flush();assert(fileActions.length===1 && fileActions[0].owner===owner && fileActions[0].action.kind==='select' && submissions.length===1,'Plus did not request only this card file picker');
    const selectedFile={id:'selected-file',name:'note.txt',kind:'text',byteCount:32};
    const fileResult=(files,scope=owner)=>popup.dispatchEvent(new CustomEvent('central-agent:graph-files-result',{detail:{owner:scope,files}}));
    fileResult([selectedFile]);await flush();assert(popup.querySelector('.graph-file-drafts').textContent.includes('note.txt'),'Selected file preview is missing');
    fileResult([],'graph:another-card');await flush();assert(popup.querySelector('.graph-file-drafts').textContent.includes('note.txt'),'Another owner cleared the selected file');
    fileResult([selectedFile]);await flush();assert(popup.querySelector('.graph-file-drafts').textContent.includes('note.txt'),'Cancelling a picker lost an existing attachment');
    fileResult(Array.from({length:12},(_,index)=>({...selectedFile,id:`selected-${index}`})));await flush();assert(attach.disabled,'File limit does not match the main composer');
    fileResult([selectedFile]);await flush();popup.querySelector('.graph-file-drafts button').click();await flush();assert(fileActions.at(-1).action.kind==='remove' && fileActions.at(-1).action.id===selectedFile.id,'Remove did not target the selected snapshot');
    fileResult([]);await flush();assert(!popup.querySelector('.graph-file-drafts').textContent.trim(),'Removed snapshot is still displayed');fileResult([selectedFile]);await flush();
    enter();await flush();assert(submissions.length===2 && submissions[1].input==='' && submissions[1].fileIds.join(',')===selectedFile.id && attach.disabled,'Attachment-only submission lost its snapshot or bypassed pending acceptance');
    popup.dispatchEvent(new CustomEvent('central-agent:conversation-error',{detail:{owner,error:'Rejected fixture'}}));await flush();assert(!attach.disabled && popup.querySelector('.graph-file-drafts').textContent.includes('note.txt'),'Rejected submission lost its attachment');
    fileResult([]);await flush();popup.removeEventListener('central-agent:graph-files',captureFile);
    const stops=[];const captureStop=event=>stops.push(event.detail);popup.addEventListener('central-agent:graph-stop',captureStop);
    const type=text=>{input.value=text;input.dispatchEvent(new Event('input',{bubbles:true}));};
    type('Button submission probe');await flush();sendButton.click();sendButton.click();await flush();
    assert(submissions.length===3 && submissions[2].input==='Button submission probe' && sendButton.disabled,'Send button lost the prompt or allowed a duplicate');
    composerState(true,{active:true,canStop:true});await flush();
    popup.dispatchEvent(new CustomEvent('central-agent:submission-accepted',{detail:{owner,input:'Button submission probe'}}));await flush();
    stopButton.click();stopButton.click();await flush();assert(stops.length===1 && stops[0].owner===owner && stopButton.disabled && sendButton.hidden,'Stop was duplicated or claimed premature completion');
    composerState(true,{active:true,canStop:true});await flush();assert(stopButton.disabled && sendButton.hidden,'An active update incorrectly completed Stop');
    composerState(true,{owner:'graph:another-card',canResume:true});await flush();assert(stopButton.disabled && sendButton.hidden,'Another owner completed Stop');
    composerState(true,{canResume:true});await flush();assert(!sendButton.disabled && sendButton.dataset.workAction==='resume' && stopButton.hidden,'Confirmed interruption did not enable Resume');
    enter();await flush();assert(submissions.length===3,'Empty Enter resumed work without a click');
    sendButton.click();sendButton.click();await flush();assert(submissions.length===4 && submissions[3].resume===true && submissions[3].owner===owner && submissions[3].delivery==='start' && !submissions[3].fileIds.length && input.value==='','Resume did not send exactly one continuation to the same owner');
    type('Keep this newer draft');await flush();popup.dispatchEvent(new CustomEvent('central-agent:submission-accepted',{detail:{owner,input:submissions[3].input}}));await flush();assert(input.value==='Keep this newer draft' && sendButton.dataset.workAction==='send','Resume acceptance overwrote later typing');
    composerState(true,{active:true,canStop:true});await flush();stopButton.click();await flush();
    popup.dispatchEvent(new CustomEvent('central-agent:conversation-error',{detail:{owner,error:'Stop rejected fixture'}}));await flush();assert(!stopButton.disabled && sendButton.hidden && input.value==='Keep this newer draft','Rejected Stop hid the active work or lost a draft');
    composerState(true,{canResume:true});await flush();assert(sendButton.dataset.workAction==='send','A stopped conversation replaced a draft with a continuation');
    type('');await flush();composerState(true);await flush();assert(sendButton.dataset.workAction==='send' && sendButton.disabled,'Completed work still offers Resume');
    composerState(true,{canResume:true});await flush();assert(sendButton.dataset.workAction==='resume','Reloaded stopped state needs an earlier local Stop click');
    popup.removeEventListener('central-agent:graph-stop',captureStop);
    popup.removeEventListener('central-agent:graph-submit',capture);
    if(oldTheme)document.documentElement.dataset.theme=oldTheme;else delete document.documentElement.dataset.theme;
    popup.style.removeProperty('--ca-profile-motion-duration');
    popup.style.removeProperty('--agent-console-available-width');popup.style.removeProperty('--agent-console-available-height');
  } else {
  assert([...profileRoot.querySelectorAll('[data-picker]')].map(field=>field.dataset.picker).join(',')==='provider,model,effort,speed','Main profile menu must contain exactly the four profile choices');
  assert(!profileRoot.querySelector('[data-config-section],.native-configuration,dialog'),'Advanced menus or dialogs remain in the main profile picker');
  assert(document.querySelectorAll('#personality-select').length===1 && document.querySelectorAll('#context-select').length===1,'Moving preferences duplicated their stable controls');
  assert(root.closest('#settings-agent') && document.querySelector('[data-settings-target="settings-agent"]'),'Agent controls are missing from Settings');
  assert(document.querySelector('[data-settings-target="settings-codex-tools"]'),'The separate Codex tools category is missing');
  assert(groups.map(group=>group.dataset.configSection).join(',')==='response,access,tools','The stable behavior, access and tool controllers are missing');
  assert(toggles.length===0,'Fixed Settings sections still look like collapsible controls');
  assert(contents.length===3 && new Set(contents.map(content=>content.id)).size===3,'Configuration section IDs are missing or duplicated');
  assert(contents.every(content=>content.dataset.collapsed==='false'),'A fixed Settings section starts collapsed');
  const profileIds=['provider','model','effort','speed','personality','context'];
  const controls=profileIds.map(kind=>document.getElementById(`${kind}-select`));
  const values=controls.map(control=>control.value);
  window.centralAgentOpenSettings();window.CentralAgentSvelte.selectSettingsSection('settings-agent');await flush();
  assert(profileRoot.hidden,'Opening Settings left the main profile menu open');
  assert(root.querySelector('.agent-settings-primary').getClientRects().length && !root.querySelector('[data-config-section="tools"]').getClientRects().length,'Agents & permissions still includes Codex Skills and Apps');
  window.CentralAgentSvelte.selectSettingsSection('settings-codex-tools');await flush();
  const toolsCategory=root.querySelector('[data-config-section="tools"]'),toolsContent=toolsCategory.querySelector('.configuration-content');
  assert(!root.querySelector('.agent-settings-primary').getClientRects().length && toolsCategory.getClientRects().length,`Codex tools did not isolate Skills and Apps (page hidden=${root.closest('#settings-agent').hidden}, presentation=${root.closest('#settings-agent').dataset.settingsPresentation}, section display=${getComputedStyle(toolsCategory).display})`);
  assert(toolsContent.dataset.collapsed==='false',`Codex tools did not expose its content (collapsed=${toolsContent.dataset.collapsed})`);
  window.CentralAgentSvelte.selectSettingsSection('settings-agent');await flush();
  const oldTheme=document.documentElement.dataset.theme,shell=document.querySelector('.settings-shell'),oldWidth=shell.style.width;
  const accessPanel=root.querySelector('.native-workspace-access'),accessSelect=accessPanel.querySelector('select');
  const summaryPicker=root.querySelector('#reasoning-summary-select-picker-button');
  const summaryMenu=root.querySelector('#reasoning-summary-select-picker-menu');
  assert(summaryPicker?.textContent.trim()==='Automatic' && summaryPicker.getAttribute('aria-describedby')?.includes('reasoning-summary-select-picker-description'),'Reasoning summaries does not expose a clear selected value and explanation');
  summaryPicker.click();await flush();
  assert(!summaryMenu.hidden && summaryMenu.querySelectorAll('[role="option"]').length===5,'Reasoning summaries did not open the shared picker');
  summaryMenu.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true,cancelable:true}));await flush();
  assert(summaryMenu.hidden && document.activeElement===summaryPicker,'Reasoning summaries did not close cleanly with Escape');
  for(const theme of ['central','central_dark']) {
    document.documentElement.dataset.theme=theme;
    for(const width of [720,1920,2560,3840]) {
      shell.style.width=`${width}px`;await flush();
      const preferences=root.querySelector('.response-settings');
      assert(preferences.clientWidth>0 && preferences.scrollWidth<=preferences.clientWidth+1,`${theme}/${width}: moved preferences overflow Settings`);
      for(const selected of ['readOnly','workspaceWrite','fullAccess']) {
        emit({},{selected});await flush();
        assert(accessSelect.value===selected && !accessSelect.disabled,`${theme}/${width}: saved shared permissions are not displayed`);
        assert(accessPanel.clientWidth>0 && accessPanel.scrollWidth<=accessPanel.clientWidth+1,`${theme}/${width}: shared permission text overflows`);
        assert(accessPanel.textContent.includes('shared by every Codex agent') && accessPanel.textContent.includes('after restarting Supervisor'), 'Permissions do not explain their persistent shared scope');
        const summaryCopy=root.querySelector('[data-settings-picker="reasoning-summary-select"] .settings-picker-copy').getBoundingClientRect();
        const summaryButton=summaryPicker.getBoundingClientRect();
        if(width>760)assert(summaryCopy.right<summaryButton.left,`${theme}/${width}: Reasoning summaries label and selected value run together`);
      }
      emit({},{permissionsDisabled:true,permissionsBusy:true});await flush();
      assert(accessSelect.disabled && !root.querySelector('#reasoning-summary-select').disabled,'Other Codex work does not block shared permissions independently of summaries');
      emit({},{selected:'fullAccess',options:accessOptions.map(option=>({...option,allowed:option.value!=='fullAccess'}))});await flush();
      assert(accessPanel.textContent.includes('saved choice is unavailable') && accessSelect.selectedOptions[0].disabled,'A managed restriction hides the saved choice or loses its explanation');
      emit();await flush();accessSelect.value='fullAccess';accessSelect.dispatchEvent(new Event('change',{bubbles:true}));await flush();
      const consent=root.querySelector('dialog[aria-label="Allow full Codex access?"]');
      assert(consent.open && consent.textContent.includes('All Codex agents in Supervisor') && consent.textContent.includes('remains active after restarting'), 'Full access confirmation omits its global persistent scope');
      assert(consent.scrollWidth<=consent.clientWidth+1,'Permission confirmation overflows');
      consent.close();await flush();
    }
  }
  emit();
  shell.style.width=oldWidth;
  if(oldTheme)document.documentElement.dataset.theme=oldTheme;else delete document.documentElement.dataset.theme;
  window.centralAgentCommitSettingsClose();await flush();launcher.click();await flush();
  assert(!profileRoot.hidden && controls.every((control,index)=>control===document.getElementById(`${profileIds[index]}-select`) && control.value===values[index]),'Settings navigation replaced profile controls or reset their values');
  window.centralAgentOpenSettings();await flush();
  await flush();
  assert(getComputedStyle(root.querySelector('.response-settings')).display!=='none','Fixed response preferences are hidden');
  }
  assert(![...root.querySelectorAll('button')].some(button=>button.getClientRects().length && ['Rename','Archive','Delete conversation','Browse Codex history…','Codex defaults…','Goal…'].includes(button.textContent.trim())),'Conversation management leaked into Settings or an agent card');
  emit();await flush();
  if(!popup){window.CentralAgentSvelte.selectSettingsSection('settings-codex-tools');await flush();}
  window.dispatchEvent(new CustomEvent('central-agent:app-server-skills',{detail:{owner,selected:[{id:'configuration-skill',name:'Selected probe',directory:view.directory,current:true}]}}));await flush();
  const selectedSkills=root.querySelector('[data-config-persistent]');
  assert(popup?!selectedSkills?.getClientRects().length:selectedSkills?.getBoundingClientRect().height>0,popup?'Agent card displays skill configuration':'Closing tools hid selected skill evidence/removal');
  window.dispatchEvent(new CustomEvent('central-agent:app-server-conversation',{detail:{owner,error:'Synthetic operation failed'}}));await flush();
  const error=root.querySelector('.native-configuration > [role="alert"]');
  assert(error?.textContent==='Synthetic operation failed' && error.getBoundingClientRect().height>0,'Closing conversation hid an actionable error');
  window.dispatchEvent(new CustomEvent('central-agent:app-server-conversation',{detail:{owner,change:'read'}}));
  // Slash commands use the mounted Settings controller. No confirmed mutation
  // is sent: opening history/skills/defaults/goal only requests a read.
  if(!popup)window.centralAgentCommitSettingsClose();await flush();
  for(const [command,label] of [['goal','Native Codex goal'],['fork','Manage Codex conversation'],['review','Manage Codex conversation'],['resume','Native Codex history'],['skills','Native Codex skills'],['config','Native Codex defaults']]){
    if(!popup)assert(window.CentralAgentSvelte.prepareAgentSettingsDialogs(),'Settings dialog host is unavailable');
    window.dispatchEvent(new CustomEvent('central-agent:native-command',{cancelable:true,detail:{owner,command}}));await flush();
    const dialog=root.querySelector(`dialog[aria-label="${label}"]`);
    assert(dialog?.open,`${command} shortcut did not open its owned dialog`);
    assert(dialog.getBoundingClientRect().width>0 && dialog.getBoundingClientRect().height>0,`${command} shortcut left an invisible modal in a closed group`);
    assert(getComputedStyle(dialog).visibility==='visible' && !dialog.closest('[hidden]'),`${command} modal inherited hidden Settings presentation`);
    const action=[...dialog.querySelectorAll('button:not(:disabled)')].find(button=>{
      const bounds=button.getBoundingClientRect();return bounds.width>0 && bounds.height>0 && bounds.top>=0 && bounds.bottom<=window.innerHeight;
    });
    assert(action && getComputedStyle(action).visibility==='visible' && getComputedStyle(action).pointerEvents!=='none',`${command} dialog controls cannot receive interaction`);
    const actionBounds=action.getBoundingClientRect();
    assert(action.contains(document.elementFromPoint(actionBounds.left+actionBounds.width/2,actionBounds.top+actionBounds.height/2)),`${command} dialog action is covered by another surface`);
    assert(!root.hidden,`${command} hid the configuration host`);
    if(!popup)assert(profileRoot.hidden && document.getElementById('settings-view').hidden && root.parentElement===document.body,'A shortcut exposed advanced menus or reopened Settings');
    emit();await flush();assert(dialog.open,`${command} dialog closed on a background refresh`);
    dialog.close();await flush();
    if(!popup)assert(root.parentElement===document.getElementById('settings-agent') && !root.dataset.dialogOnly,'Closing the modal did not return its original mounted controls to Settings');
  }
  if(!popup)window.centralAgentOpenSettings();await flush();
  await flush();emit({connected:false});await flush();
  emit({binding:null,observed:false});await flush();
  assert(![...root.querySelectorAll('button')].some(button=>button.getClientRects().length && button.textContent==='Browse Codex history…'),'History browsing leaked back into Settings');
  emit({visible:false});await flush();assert(root.querySelector('.native-configuration').hidden,'Other providers display Codex configuration');
  if(popup){window.CentralAgentSvelte.unmountKnowledgeAgentPopup(popup);popup.remove();}
  else {window.centralAgentCommitSettingsClose();}
} catch(error) { startupErrors.push(`Agent configuration: ${String(error)}`); }
