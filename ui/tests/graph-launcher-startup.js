// Runs inside --check-ui-startup's disposable graph WebView. This checks the
// actual renderer/DOM, not native window geometry or the Settings IPC path.
// Rust transition-order tests cover the native Settings path.
// No account, provider, project or external connection is used.
try {
  const logo = document.getElementById('knowledge-logo');
  const shell = document.getElementById('graph-shell');
  const registryHost = document.querySelector('[data-central-agent-svelte="project-registry"]');
  const flush = async () => { await Promise.resolve(); await Promise.resolve(); await Promise.resolve(); await new Promise(resolve=>requestAnimationFrame(resolve)); await Promise.resolve(); };
  const assert = (condition, message) => { if (!condition) throw new Error(message); };
  const projectNodes = ['first', 'second'].map(id => ({
    key: `entity:${id}`, recordType: 'entity', recordId: id,
    label: `Synthetic ${id}`, entityKind: 'project',
    sourceUri: 'supervisor-project://connected', filesystemPath: `C:\\Projects\\${id}`,
  }));
  const projects = projectNodes.map((node, index) => ({
    id: `C:\\Projects\\${index ? 'second' : 'first'}`,
    nodeKey: node.key,
    source: 'local',
    name: node.label,
    path: node.filesystemPath,
    active: index === 0,
    pinned: index === 1,
    lastOpenedAtMs: 10 - index,
    agentCount: index + 1,
    access: 'Full access',
    metadata: {
      gitRepository: true,
      gitBranch: 'main',
      gitModified: index,
      worktreeCount: 1,
      stack: ['Rust', 'Svelte'],
      instructionFiles: ['AGENTS.md'],
      commands: [{label: 'Build', command: 'cargo build'}],
    },
    scanning: false,
  }));
  const graph = { visualGraph: { nodes: projectNodes, edges: [] } };
  const projectRegistry = {
    projects,
    sshProfiles: [{id:'fixture-server',name:'Fixture server',target:'fixture@example.test',agentEnabled:true}],
    import: {active:false,error:false},
    dropActive: false,
  };
  const projectChats = [
    {id:'original',title:'Design review',projectRoot:projects[0].path,lineage:{kind:'source',childCount:1}},
    {id:'fork',title:'Alternative',projectRoot:projects[0].path,lineage:{kind:'fork',parentChatId:'original',parentTitle:'Design review',parentAvailable:true,childCount:0}},
    {id:'other',title:'Other project chat',projectRoot:projects[1].path},
  ];
  const workspace={root:projects[0].path,entries:[{path:'src/main.rs',kind:'file',iconKey:'rust'},{path:'README.md',kind:'file',iconKey:'markdown'}]};

  for (const theme of ['central', 'central_dark']) {
    const state = { theme, graph, projectRegistry, projectChats, workspace, orb: { x: 0, y: 0, size: 112 } };
    for (let repeat = 0; repeat < 2; repeat++) {
      window.renderKnowledgeSurfaceState({ ...state, expanded: true });
      await flush();
      assert(document.body.classList.contains('expanded') && !shell.hidden,
        `${theme}: graph did not reopen`);
      assert(getComputedStyle(logo).display === 'none' && getComputedStyle(shell).display !== 'none',
        `${theme}: expanded graph and launcher have conflicting visibility`);
      assert(!document.getElementById('graph-summary') && !document.querySelector('.board-header') && !document.getElementById('close-graph'),
        `${theme}: the retired project count or in-board Workspace row still reserves vertical space`);
      assert(!registryHost.textContent.includes('Reopen last project') && !registryHost.querySelector('.reopen-setting'),
        `${theme}: the fixed reopen-last behavior is still exposed as a setting`);
      const boardLanes=[...document.querySelectorAll('[data-board-lane]')];
      assert(boardLanes.map(lane=>lane.dataset.boardLane).join(',')==='projects,chats,supervisors,files', `${theme}: four-column order differs from the project mockup`);
      const boardBounds=document.querySelector('.board-columns').getBoundingClientRect();
      const headingTops=boardLanes.map(lane=>lane.querySelector('h2').getBoundingClientRect().top);
      assert(Math.abs(boardBounds.top)<.6&&headingTops.every(top=>Math.abs(top-10)<.6),`${theme}: project sections do not align with the upper edge of the return arrow: ${JSON.stringify({boardTop:boardBounds.top,headingTops})}`);
      assert(registryHost.textContent.includes('Synthetic first') && registryHost.textContent.includes('Synthetic second'),
        `${theme}: explicit local projects are absent from the registry`);
      const fileLane=document.querySelector('[data-board-lane="files"]');
      assert(!fileLane.querySelector('.root-path')&&!fileLane.querySelector('.project-footer')&&!fileLane.textContent.includes('Showing a bounded file list')&&!fileLane.textContent.includes('Rust · Svelte'), `${theme}: internal project path or metadata footer is still visible in Project files`);
      assert(document.querySelector('[data-chat-id="fork"]').dataset.chatDepth==='1', `${theme}: real fork ancestry is flattened`);
      assert(!document.querySelector('[data-chat-id="other"]'), `${theme}: another project's chat is visible`);
      const chatAdd=document.querySelector('[data-board-lane="chats"] .icon-button');
      const chatMenuButton=document.querySelector('[data-chat-id="original"] .workspace-row-menu');
      const center=element=>{const bounds=element.getBoundingClientRect();return{x:bounds.left+bounds.width/2,y:bounds.top+bounds.height/2};};
      const chatAddCenter=center(chatAdd),chatMenuCenter=center(chatMenuButton);
      assert(Math.abs(chatAddCenter.x-chatMenuCenter.x)<.6,`${theme}: chat add and overflow controls do not share one center line: ${JSON.stringify({chatAddCenter,chatMenuCenter,cardWidth:getComputedStyle(document.querySelector('[data-board-lane="chats"]')).getPropertyValue('--project-chat-card-width')})}`);
      for(const [control,icon,name] of [[chatAdd,chatAdd.querySelector('svg'),'chat add'],[chatMenuButton,chatMenuButton.querySelector('svg'),'chat overflow']]){
        const controlCenter=center(control),iconCenter=center(icon);
        assert(Math.abs(controlCenter.x-iconCenter.x)<.6&&Math.abs(controlCenter.y-iconCenter.y)<.6,`${theme}: ${name} glyph is not centered in its control`);
      }
      assert(document.querySelector('[aria-label="Expand src"]') && !document.querySelector('[aria-label="Open src/main.rs"]'), `${theme}: project files do not start as a collapsed tree`);
      document.querySelector('[aria-label="Expand src"]').click();await flush();
      assert(document.querySelector('[aria-label="Open src/main.rs"]'), `${theme}: expanding a project folder hides its files`);
      document.querySelector('[aria-label="Collapse src"]').click();await flush();
      for(const width of [1024,1920,2560,3840]) {
        shell.style.width=width+'px';await flush();
        const lanes=[...document.querySelectorAll('[data-board-lane]')].map(lane=>lane.getBoundingClientRect());
        assert(lanes.every(rect=>rect.width>=190&&rect.height>200), `${theme}/${width}: a lane collapses below usable dimensions`);
        assert(lanes.every((rect,index)=>!index||rect.left>=lanes[index-1].right), `${theme}/${width}: board columns overlap`);
        const columns=document.querySelector('.board-columns');
        assert(columns.scrollWidth>columns.clientWidth||width>1100, `${theme}/${width}: compact lanes cannot be reached by scrolling`);
      }
      shell.style.width='';
      assert(!registryHost.textContent.includes('Windows') && !registryHost.textContent.includes('Documents'),
        `${theme}: the removed computer filesystem projection is still visible`);
      const legacyExplorer = document.getElementById('explorer-content');
      assert(legacyExplorer.closest('[hidden]') && legacyExplorer.textContent.trim() === '' && document.getElementById('graph-canvas').getClientRects().length===0,
        `${theme}: a legacy filesystem explorer was rendered`);
      const addButton = [...registryHost.querySelectorAll('button')].find(button => button.textContent.includes('Add project'));
      const addProjectIcon=addButton.querySelector('.project-add-icon');
      const addProjectIconStyle=getComputedStyle(addProjectIcon);
      assert(addProjectIcon&&addProjectIcon.getBoundingClientRect().width>0&&addProjectIconStyle.stroke!=='none'&&addProjectIconStyle.stroke!==addProjectIconStyle.fill,`${theme}: Add project plus is not visibly stroked`);
      addButton.click(); await flush();
      assert(document.querySelector('[role="dialog"]')?.textContent.includes('Local folder')
          && document.querySelector('[role="dialog"]')?.textContent.includes('Clone repository')
          && document.querySelector('[role="dialog"]')?.textContent.includes('SSH directory'),
        `${theme}: unified Add project sources are incomplete`);
      document.querySelector('[role="dialog"] button[aria-label="Close"]').click(); await flush();

      window.renderKnowledgeSurfaceState({ ...state, expanded: false });
      await flush();
      assert(!document.body.classList.contains('expanded') && shell.hidden,
        `${theme}: stale expanded graph remains after collapse`);
      assert(getComputedStyle(shell).display === 'none' && shell.getClientRects().length === 0,
        `${theme}: graph heading can leak into the collapsed launcher`);
      assert(!logo.hidden && !logo.disabled && getComputedStyle(logo).display !== 'none',
        `${theme}: collapsed graph launcher is unavailable`);
      const bounds = logo.getBoundingClientRect();
      assert(bounds.width === 112 && bounds.height === 112,
        `${theme}: collapsed graph launcher lost its native size`);
      const launcherMark = logo.querySelector('[data-graph-launcher-icon]');
      const launcherStyle = getComputedStyle(launcherMark);
      const dimensionalLogo = launcherMark.querySelector('.supervisor-logo--dimensional');
      const depthLayers = launcherMark.querySelectorAll('.supervisor-logo-depth');
      assert(launcherStyle.transform !== 'none' && dimensionalLogo && depthLayers.length === 10,
        `${theme}: collapsed graph launcher lost its geometric extrusion`);
      assert(launcherMark.querySelectorAll('[data-supervisor-logo="true"]').length === 1,
        `${theme}: dimensional launcher duplicated its canonical front face`);
      assert(getComputedStyle(depthLayers[0]).transform !== 'none'
          && getComputedStyle(depthLayers[9]).transform !== 'none',
        `${theme}: dimensional launcher depth layers are flat`);
      assert(document.querySelectorAll('#knowledge-logo').length === 1,
        `${theme}: graph launcher was duplicated on reopen`);
      assert(document.documentElement.dataset.theme === theme,
        `${theme}: collapse changed the theme`);
    }
  }

  window.renderKnowledgeSurfaceState({
    theme: 'central_dark', expanded: true, orb: {x: 0, y: 0, size: 112},
    graph: {visualGraph: {nodes: [], edges: []}},
    projectRegistry: {...projectRegistry, projects: []},
  });
  await flush();
  assert(!document.body.textContent.includes('0 projects ·') && !document.getElementById('graph-summary'),
    'Empty project registry restored the retired numeric summary');
  assert(registryHost.textContent.includes('No projects yet'),
    'Empty project registry does not explain how to add a project');
  window.renderKnowledgeSurfaceState({expanded:false, graph:{visualGraph:{nodes:[],edges:[]}}, projectRegistry:{...projectRegistry,projects:[]}});
  await flush();
} catch (error) { startupErrors.push(String(error)); }

// Navigate real projects while cards are mounted: ownership and unsent drafts
// must survive a different project's render and background updates.
{
  const bridge=window.chrome.webview, originalPost=bridge.postMessage, sent=[];
  const flush=async()=>{await Promise.resolve();await Promise.resolve();await Promise.resolve();await new Promise(resolve=>requestAnimationFrame(resolve));await Promise.resolve();};
  const assert=(condition,message)=>{if(!condition)throw new Error(message);};
  try {
    bridge.postMessage=message=>sent.push(JSON.parse(message));
    const boardShell=document.getElementById('graph-shell');
    for(const theme of ['central','central_dark']) {
      const projects=['alpha','beta'].map(name=>({id:`C:/fixture/${name}`,path:`C:/fixture/${name}`,nodeKey:`entity:${name}`,source:'local',name,active:name==='alpha'}));
      const bindings=projects.map(project=>({recordType:'entity',recordId:project.name,conversationId:`card-${project.name}`,name:`Supervisor ${project.name}`,projectDirectory:project.path,provider:'codex_app_server',selection:{model:'fixture-model',effort:'low'}}));
      const provider={id:'codex_app_server',label:'Codex',provider:{phase:'ready',authenticated:true},selection:{model:'fixture-model',effort:'low',serviceTier:null},models:[{model:'fixture-model',displayName:'Fixture',defaultReasoningEffort:'low',supportedReasoningEfforts:[{reasoningEffort:'low'}],serviceTiers:[]}]};
      const state={theme,expanded:true,projectRegistry:{projects},
        providers:[provider],projectChatPreviews:[],
        projectChats:[{id:'alpha-chat',title:'Alpha task',projectRoot:projects[0].path,verification:{state:'checking',summary:'Reviewing the final worker checkpoint.'}},{id:'alpha-second',title:'Second task',projectRoot:projects[0].path}],
        graph:{visualGraph:{nodes:projects.map(project=>({key:project.nodeKey,recordType:'entity',recordId:project.name,label:project.name,entityKind:'project'})),edges:[]}},
        knowledgeAgents:{bindings,activeNodeKeys:[]},knowledgeRuns:[],workspace:{root:projects[0].path,entries:[{path:'alpha.rs',kind:'file'}]}};
      window.renderKnowledgeSurfaceState(state);await flush();
      const verificationButton=document.querySelector('[data-verification-owner="chat:alpha-chat"]'),verificationPanel=document.getElementById(verificationButton?.getAttribute('popovertarget'));
      assert(verificationButton?.dataset.taskVerification==='checking'&&verificationButton.textContent.trim()==='Checking',`${theme}: project delivery state is not visible beside its chat`);
      assert(!verificationButton.hasAttribute('title'),`${theme}: delivery state still exposes a hover guide`);
      verificationButton.click();await flush();
      assert(verificationPanel?.matches(':popover-open')&&verificationPanel.textContent.includes('Reviewing the final worker checkpoint.'),`${theme}: delivery details did not open from the compact status`);
      verificationButton.click();await flush();
      state.projectChats[0].verification={state:'blocked',summary:'A correction is required before delivery.'};window.renderKnowledgeSurfaceState(state);await flush();
      assert(document.querySelector('[data-verification-owner="chat:alpha-chat"]')?.dataset.taskVerification==='blocked',`${theme}: a blocked final review did not replace the checking state`);
      state.projectChats[0].verification={state:'checking',summary:'Reviewing the final worker checkpoint.'};window.renderKnowledgeSurfaceState(state);await flush();
      const supervisorRowBeforeOpen=document.querySelector('[data-agent-key="conversation:card-alpha"]'),supervisorWidthBeforeOpen=supervisorRowBeforeOpen.getBoundingClientRect().width;
      const supervisorMenuButton=supervisorRowBeforeOpen.parentElement.querySelector('[data-supervisor-menu-trigger]'),supervisorRowBounds=supervisorRowBeforeOpen.getBoundingClientRect(),supervisorMenuBounds=supervisorMenuButton.getBoundingClientRect();
      assert(Math.abs((supervisorMenuBounds.top+supervisorMenuBounds.height/2)-(supervisorRowBounds.top+supervisorRowBounds.height/2))<1,`${theme}: Supervisor actions are not vertically centered in their card`);
      supervisorRowBeforeOpen.click();await flush();
      const cardA=document.querySelector('.agent-console[data-node-key="conversation:card-alpha"]');
      const selectedAgentRow=document.querySelector('[data-agent-key="conversation:card-alpha"]');
      const cardStyle=getComputedStyle(cardA),selectedAgentStyle=getComputedStyle(selectedAgentRow);
      assert(parseFloat(cardStyle.borderTopWidth)===3&&cardStyle.borderTopColor===selectedAgentStyle.backgroundColor,`${theme}: supervisor card border is not three pixels in the selected-card color`);
      assert(Math.abs(selectedAgentRow.getBoundingClientRect().width-supervisorWidthBeforeOpen)<1,`${theme}: opening a Supervisor conversation shrank its selector card`);
      assert(Math.abs(cardA.getBoundingClientRect().width-selectedAgentRow.getBoundingClientRect().width)<1,`${theme}: supervisor conversation width does not match its selector card`);
      for(const width of [1024,1920,2560,3840]){
        boardShell.style.width=`${width}px`;await flush();
        const cardBounds=cardA.getBoundingClientRect(),rowBounds=selectedAgentRow.getBoundingClientRect();
        assert(Math.abs(cardBounds.width-rowBounds.width)<1,`${theme}/${width}: supervisor conversation is not symmetric with its selector card: ${JSON.stringify({card:cardBounds.width,row:rowBounds.width,cardWidth:getComputedStyle(document.querySelector('[data-board-lane="supervisors"]')).getPropertyValue('--supervisor-card-width')})}`);
        assert(Math.abs(cardBounds.height/cardBounds.width-16/9)<.02,`${theme}/${width}: supervisor conversation lost its responsive 9:16 size`);
      }
      boardShell.style.width='';await flush();
      const supervisorSettings=document.querySelector('[data-board-conversation-menu="graph:conversation:card-alpha"]');
      const settingsTrigger=supervisorSettings.querySelector('.menu-trigger'),settingsPanel=supervisorSettings.querySelector('[popover]'),association=supervisorSettings.querySelector('[data-settings-picker="board-agent-chat"]'),associationButton=association?.querySelector('.settings-picker-button');
      assert(!settingsPanel.matches(':popover-open')&&associationButton&&!associationButton.getClientRects().length,`${theme}: Supervisor options are visible before opening the compact menu`);
      const conversationBefore=cardA.getBoundingClientRect();
      settingsTrigger.click();await flush();
      assert(settingsPanel.matches(':popover-open')&&settingsTrigger.getAttribute('aria-expanded')==='true',`${theme}: compact Supervisor menu did not open`);
      assert(Math.abs(cardA.getBoundingClientRect().top-conversationBefore.top)<1&&Math.abs(cardA.getBoundingClientRect().width-conversationBefore.width)<1,`${theme}: opening Supervisor options moved or resized the conversation`);
      assert(association?.dataset.pickerPresentation==='workspace'&&associationButton&&Math.abs(associationButton.getBoundingClientRect().height-38)<.6,`${theme}: Associated conversation does not use the compact Supervisor picker`);
      associationButton.click();await flush();
      const associationMenu=association.querySelector('.settings-picker-menu');
      assert(!associationMenu.hidden&&associationMenu.querySelectorAll('[role="option"]').length===3&&getComputedStyle(associationMenu).boxShadow!=='none',`${theme}: Associated conversation menu does not match the floating Supervisor picker`);
      associationButton.click();await flush();
      settingsPanel.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true,cancelable:true}));await flush();
      assert(!settingsPanel.matches(':popover-open')&&document.activeElement===settingsTrigger,`${theme}: Escape did not close options and restore focus`);
      sent.length=0;supervisorMenuButton.click();await flush();
      const supervisorActions=document.querySelector('[role="menu"][aria-label="Actions for Supervisor alpha"]');
      assert(supervisorActions&&supervisorActions.querySelector('[data-supervisor-action="rename"]')&&supervisorActions.querySelector('[data-supervisor-action="delete"]'),`${theme}: Supervisor actions do not expose Rename and Delete`);
      supervisorActions.querySelector('[data-supervisor-action="rename"]').click();await flush();
      const renameDialog=document.querySelector('[role="dialog"][aria-labelledby="rename-supervisor-title"]'),renameInput=renameDialog?.querySelector('#supervisor-name');
      assert(renameInput&&renameInput.value==='Supervisor alpha',`${theme}: Rename did not preserve the current Supervisor name`);
      renameInput.value='Coordinator alpha';renameInput.dispatchEvent(new Event('input',{bubbles:true}));renameDialog.querySelector('form').requestSubmit();await flush();
      assert(sent.some(message=>message.message_type==='project_board'&&message.action.type==='rename_agent'&&message.action.node_key==='conversation:card-alpha'&&message.action.name==='Coordinator alpha'),`${theme}: Rename did not preserve the exact Supervisor identity and new name`);
      bindings[0].name='Coordinator alpha';window.renderKnowledgeSurfaceState(state);await flush();
      assert(document.querySelector('[data-agent-key="conversation:card-alpha"] .agent-copy strong')?.textContent==='Coordinator alpha',`${theme}: a saved Supervisor rename was not rendered`);
      const inputA=cardA.querySelector('[data-role="message"]');inputA.value='Keep alpha draft';inputA.dispatchEvent(new Event('input',{bubbles:true}));await flush();
      const supervisorMinimize=cardA.querySelector('[data-action="minimize"]'),supervisorExpandedHeight=cardA.getBoundingClientRect().height;
      sent.length=0;supervisorMinimize.click();await flush();window.renderKnowledgeSurfaceState(state);await flush();
      assert(cardA.dataset.cardMinimized==='true'&&cardA.getBoundingClientRect().height<supervisorExpandedHeight/2&&Math.abs(cardA.getBoundingClientRect().width-selectedAgentRow.getBoundingClientRect().width)<1,`${theme}: minimizing a Supervisor does not collapse only its body at the selector width`);
      assert(inputA.value==='Keep alpha draft'&&inputA.form.getClientRects().length===0&&supervisorMinimize.getAttribute('aria-label')==='Restore conversation',`${theme}: minimized Supervisor lost its draft or accessible restore control`);
      assert(!sent.some(message=>/stop_knowledge_agent|close_chat_preview/.test(message.message_type)),`${theme}: minimizing a Supervisor stopped or closed its work`);
      supervisorMinimize.click();await flush();
      assert(cardA.dataset.cardMinimized==='false'&&inputA.form.getClientRects().length&&inputA.value==='Keep alpha draft'&&Math.abs(cardA.getBoundingClientRect().height/cardA.getBoundingClientRect().width-16/9)<.02,`${theme}: restoring a Supervisor did not recover its unchanged portrait card`);
      window.CentralAgentSvelte.selectBoardProject('entity:beta');await flush();
      assert(sent.some(message=>message.message_type==='project_board'&&message.action.type==='select'&&message.action.id===projects[1].id),`${theme}: selecting a project did not reach Rust`);
      state.workspace={root:projects[1].path,entries:[{path:'beta.rs',kind:'file'}]};window.renderKnowledgeSurfaceState(state);await flush();
      assert(cardA.hidden&&!document.querySelector('[data-chat-id="alpha-chat"]')&&!document.querySelector('[aria-label="Open alpha.rs"]'),`${theme}: alpha scope leaked into beta`);
      document.querySelector('[data-agent-key="conversation:card-beta"]').click();await flush();
      const cardB=document.querySelector('.agent-console[data-node-key="conversation:card-beta"]');
      const inputB=cardB.querySelector('[data-role="message"]');inputB.value='Keep beta draft';inputB.dispatchEvent(new Event('input',{bubbles:true}));await flush();
      window.CentralAgentSvelte.selectBoardProject('entity:alpha');state.workspace={root:projects[0].path,entries:[{path:'alpha.rs',kind:'file'}]};state.projectChatPreviews=[];window.renderKnowledgeSurfaceState(state);await flush();
      assert(document.querySelector('.agent-console[data-node-key="conversation:card-alpha"]')===cardA&&!cardA.hidden&&cardB.hidden&&inputA.value==='Keep alpha draft'&&inputB.value==='Keep beta draft',`${theme}: switching projects lost card identity or a draft`);
      sent.length=0;document.querySelector('[data-chat-id="alpha-chat"] .workspace-row-menu').click();await flush();
      const projectChatActions=document.querySelector('[aria-label="Conversation actions"]'),renameProjectChat=projectChatActions?.querySelector('[data-chat-action="rename"]');
      assert(renameProjectChat,`${theme}: project chat actions do not expose Rename`);
      renameProjectChat.click();await flush();
      const renameProjectChatDialog=document.querySelector('[role="dialog"][aria-labelledby="rename-project-chat-title"]'),projectChatTitleInput=renameProjectChatDialog?.querySelector('#project-chat-title');
      assert(projectChatTitleInput?.value==='Alpha task',`${theme}: project chat Rename did not preserve the current title`);
      projectChatTitleInput.value=' ';projectChatTitleInput.dispatchEvent(new Event('input',{bubbles:true}));renameProjectChatDialog.querySelector('form').requestSubmit();await flush();
      assert(renameProjectChatDialog.querySelector('[role="alert"]')&&!sent.some(message=>message.message_type==='rename_project_chat'),`${theme}: an empty project chat title was accepted`);
      projectChatTitleInput.value='Alpha task renamed';projectChatTitleInput.dispatchEvent(new Event('input',{bubbles:true}));renameProjectChatDialog.querySelector('form').requestSubmit();await flush();
      assert(sent.some(message=>message.message_type==='rename_project_chat'&&message.chat_id==='alpha-chat'&&message.title==='Alpha task renamed'),`${theme}: project chat Rename did not preserve the exact chat identity and title`);
      state.projectChats[0].title='Alpha task renamed';window.renderKnowledgeSurfaceState(state);await flush();
      assert(document.querySelector('[data-chat-id="alpha-chat"] .workspace-chat-title')?.textContent==='Alpha task renamed',`${theme}: the persisted project chat title was not rendered`);
      sent.length=0;document.querySelector('[data-chat-id="alpha-chat"] .workspace-row-menu').click();await flush();
      const deleteProjectChat=document.querySelector('[data-chat-action="delete"]');
      assert(deleteProjectChat&&!deleteProjectChat.disabled,`${theme}: project chat actions do not expose Delete`);
      deleteProjectChat.click();await flush();
      const deleteProjectChatDialog=document.querySelector('[role="alertdialog"][aria-labelledby="delete-project-chat-title"]');
      assert(deleteProjectChatDialog?.textContent.includes('Project files and checkpoints remain unchanged')&&!sent.some(message=>message.message_type==='delete_project_chat'),`${theme}: project chat deletion was dispatched before confirmation`);
      deleteProjectChatDialog.querySelector('[data-chat-confirm-delete]').click();await flush();
      assert(sent.some(message=>message.message_type==='delete_project_chat'&&message.chat_id==='alpha-chat'),`${theme}: project chat Delete did not preserve its exact identity`);
      const projectRowBeforeOpen=document.querySelector('[data-chat-id="alpha-chat"]'),projectWidthBeforeOpen=projectRowBeforeOpen.getBoundingClientRect().width;
      sent.length=0;projectRowBeforeOpen.querySelector('.workspace-chat-open').click();await flush();
      assert(sent.some(message=>message.message_type==='project_board'&&message.action.type==='preview_chat'&&message.action.chat_id==='alpha-chat'),`${theme}: one click on a project chat did not request its in-board history`);
      assert(!sent.some(message=>message.message_type==='project_board'&&message.action.type==='open_chat'),`${theme}: one click on a project chat redirected to the main Agent chat`);
      const alphaPreview={id:'alpha-chat',title:'Alpha task',provider:'codex_app_server',selection:{model:'fixture-model',effort:'low',serviceTier:null},agent:{active:false,phase:'idle',status:'Waiting for a request',steps:[]},canResume:false,pendingApproval:null,conversationState:{owner:'chat:alpha-chat',active:false,busy:false,supportsSteer:false,queued:0,nativeMessages:[],nativeConversation:{visible:true,connected:true,busy:false,observed:true,directory:projects[0].path,binding:{threadId:'fixture-thread-alpha',archived:false,deleted:false},nativeLoaded:true,apps:null},fileAttachments:[],contextAttachments:{owner:'chat:alpha-chat',tabs:[],shells:[]},enabled:true},messages:[
        {id:1,role:'user',kind:'message',text:'Inspect alpha',timestampMs:1},
        {id:2,role:'assistant',kind:'message',text:'Alpha answer',renderedHtml:'<p>Alpha answer</p>',timestampMs:2}
      ]};state.projectChatPreviews=[alphaPreview];window.renderKnowledgeSurfaceState(state);await flush();
      const chatCard=document.querySelector('[data-project-chat-id="alpha-chat"]');
      assert(chatCard&&chatCard.dataset.centralAgentSvelte==='knowledge-agent-popup'&&chatCard.textContent.includes('Inspect alpha')&&chatCard.textContent.includes('Alpha answer'),`${theme}: the project conversation did not reuse the complete agent card`);
      const chatRows=[...document.querySelectorAll('[data-board-lane="chats"] .workspace-chat-row')];
      assert(Math.abs(chatRows[0].getBoundingClientRect().width-projectWidthBeforeOpen)<1,`${theme}: opening a project conversation shrank its chat card`);
      const projectChatShell=chatCard.closest('.chat-entry'),projectListGap=chatCard.getBoundingClientRect().top-projectRowBeforeOpen.getBoundingClientRect().bottom;
      assert(Math.abs(projectListGap)<.6&&chatCard.closest('.chat-inline')?.previousElementSibling===projectRowBeforeOpen&&getComputedStyle(projectChatShell).backgroundColor===getComputedStyle(chatCard).backgroundColor,`${theme}: the project chat row and conversation do not form one continuous surface`);
      for(const width of [1024,1920,2560,3840]){
        boardShell.style.width=`${width}px`;await flush();
        const cardBounds=chatCard.getBoundingClientRect(),rowBounds=chatRows[0].getBoundingClientRect();
        assert(Math.abs(cardBounds.width-rowBounds.width)<1,`${theme}/${width}: project conversation is not symmetric with its chat card`);
        assert(Math.abs(cardBounds.height/cardBounds.width-16/9)<.02,`${theme}/${width}: project conversation lost its responsive 9:16 size`);
      }
      boardShell.style.width='';await flush();
      assert(parseFloat(getComputedStyle(chatCard).borderTopWidth)===0&&getComputedStyle(projectChatShell).boxShadow==='none',`${theme}: the unified project conversation still has a residual frame`);
      assert(chatCard.querySelector('[data-role="message"]'),`${theme}: the reused project chat card is missing its composer`);
      assert(chatCard.querySelector('.plugin-picker-trigger'),`${theme}: the reused project chat card is missing its plugin picker`);
      assert(chatCard.querySelector('.agent-profile-toggle'),`${theme}: the reused project chat card is missing its profile control`);
      const alphaInput=chatCard.querySelector('[data-role="message"]');alphaInput.value='Send from the project card';alphaInput.dispatchEvent(new Event('input',{bubbles:true}));await flush();
      sent.length=0;chatCard.querySelector('[data-action="submit"]').click();await flush();
      assert(sent.some(message=>message.message_type==='project_board'&&message.action.type==='submit_chat'&&message.action.chat_id==='alpha-chat'&&message.action.message==='Send from the project card'),`${theme}: the reused card did not submit to its own project chat`);
      window.dispatchEvent(new CustomEvent('central-agent:submission-accepted',{detail:{owner:'chat:alpha-chat',input:'Send from the project card'}}));await flush();
      alphaInput.value='Keep this card draft';alphaInput.dispatchEvent(new Event('input',{bubbles:true}));await flush();
      sent.length=0;document.querySelector('[data-chat-id="alpha-second"] .workspace-chat-open').click();await flush();
      assert(sent.some(message=>message.message_type==='project_board'&&message.action.type==='preview_chat'&&message.action.chat_id==='alpha-second'),`${theme}: a second project conversation was not opened independently`);
      const secondPreview={...alphaPreview,id:'alpha-second',title:'Second task',conversationState:{...alphaPreview.conversationState,owner:'chat:alpha-second',nativeConversation:{...alphaPreview.conversationState.nativeConversation,binding:{threadId:'fixture-thread-second',archived:false,deleted:false}}},messages:[{id:3,role:'assistant',kind:'message',text:'Second answer',timestampMs:3}]};
      state.projectChatPreviews=[alphaPreview,secondPreview];window.renderKnowledgeSurfaceState(state);await flush();
      const chatLayer=document.getElementById('project-chat-window-layer'),secondCard=document.querySelector('[data-project-chat-id="alpha-second"]');
      assert(chatLayer.querySelectorAll('.agent-console[data-project-chat-id]').length===2&&getComputedStyle(chatLayer).flexDirection==='column'&&['auto','scroll'].includes(getComputedStyle(chatLayer).overflowY),`${theme}: multiple project conversations do not form a vertical scroll stack`);
      assert(chatLayer.scrollHeight>chatLayer.clientHeight&&getComputedStyle(chatLayer).scrollbarGutter.includes('stable'),`${theme}: stacked project conversations do not reserve a stable scrollbar gutter`);
      assert(Math.abs(chatCard.getBoundingClientRect().width-chatRows[0].getBoundingClientRect().width)<1,`${theme}: the project conversation changes width when its scrollbar appears`);
      const projectCardGap=secondCard.getBoundingClientRect().top-chatCard.getBoundingClientRect().bottom,sharedCardGap=parseFloat(getComputedStyle(chatCard.closest('.chat-tree')).gap),supervisorCardGap=parseFloat(getComputedStyle(document.getElementById('agent-window-layer')).gap);
      assert(sharedCardGap>0&&projectCardGap>0&&Math.abs(sharedCardGap-supervisorCardGap)<.1&&chatCard.closest('.chat-inline')?.previousElementSibling?.dataset.chatId==='alpha-chat'&&secondCard.closest('.chat-inline')?.previousElementSibling?.dataset.chatId==='alpha-second',`${theme}: conversations are not placed beneath their own rows with shared spacing`);
      assert(document.querySelector('[data-project-chat-id="alpha-chat"]')===chatCard&&alphaInput.value==='Keep this card draft'&&secondCard?.textContent.includes('Second answer'),`${theme}: opening a second conversation remounted the first card or lost its draft`);
      chatLayer.scrollTop=0;const expandedChatHeight=chatCard.getBoundingClientRect().height,expandedStackHeight=chatLayer.scrollHeight,chatMinimize=chatCard.querySelector('[data-action="minimize"]');
      sent.length=0;chatMinimize.click();await flush();window.renderKnowledgeSurfaceState(state);await flush();
      assert(chatCard.dataset.cardMinimized==='true'&&chatCard.getBoundingClientRect().height<expandedChatHeight/2&&chatLayer.scrollHeight<expandedStackHeight,`${theme}: minimizing a project chat did not release space in its stack`);
      assert(alphaInput.value==='Keep this card draft'&&alphaInput.form.getClientRects().length===0&&document.querySelector('[data-project-chat-id="alpha-second"]')===secondCard,`${theme}: minimizing a project chat remounted a sibling or lost its draft`);
      assert(!sent.some(message=>/stop_chat|close_chat_preview/.test(JSON.stringify(message))),`${theme}: minimizing a project chat stopped or closed it`);
      chatMinimize.click();await flush();
      assert(chatCard.dataset.cardMinimized==='false'&&alphaInput.form.getClientRects().length&&alphaInput.value==='Keep this card draft'&&Math.abs(chatCard.getBoundingClientRect().height/chatCard.getBoundingClientRect().width-16/9)<.02,`${theme}: restoring a project chat did not recover its unchanged card`);
      const regularWidth=chatCard.getBoundingClientRect().width;
      chatCard.querySelector('[data-action="focus"]').click();await flush();
      assert(chatCard.dataset.cardFocused==='true'&&chatCard.getBoundingClientRect().width>regularWidth&&secondCard.dataset.cardMinimized==='true',`${theme}: Focus did not expand its conversation and compact its sibling`);
      assert(alphaInput.value==='Keep this card draft'&&document.querySelector('[data-project-chat-id="alpha-chat"]')===chatCard,`${theme}: Focus remounted the conversation or dropped its draft`);
      window.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}));await flush();
      assert(chatCard.dataset.cardFocused==='false'&&secondCard.dataset.cardMinimized==='false',`${theme}: Escape did not restore the previous card arrangement`);
      const fileToggle=document.querySelector('[aria-label="Hide project files"]');fileToggle.click();await flush();
      assert(document.querySelector('.files-content').hidden,`${theme}: file column did not collapse`);
      document.querySelector('[aria-label="Show project files"]').click();await flush();
      const conversationResize=document.querySelector('[aria-label="Resize conversation columns"]'),chatLane=document.querySelector('[data-board-lane="chats"]'),supervisorLane=document.querySelector('[data-board-lane="supervisors"]');
      const projectResize=document.querySelector('[aria-label="Resize projects column"]'),fileResize=document.querySelector('[aria-label="Resize files column"]'),projectLane=document.querySelector('[data-board-lane="projects"]'),fileLane=document.querySelector('[data-board-lane="files"]'),conversationBounds=document.querySelector('.conversation-lanes').getBoundingClientRect(),projectDividerBounds=projectResize.getBoundingClientRect(),fileDividerBounds=fileResize.getBoundingClientRect();
      assert(Math.abs(projectDividerBounds.width-fileDividerBounds.width)<.6&&projectDividerBounds.left>=projectLane.getBoundingClientRect().right-.6&&projectDividerBounds.right<=conversationBounds.left+.6&&fileDividerBounds.left>=conversationBounds.right-.6&&fileDividerBounds.right<=fileLane.getBoundingClientRect().left+.6,`${theme}: the two outer board separators are not symmetric physical columns`);
      assert(getComputedStyle(projectResize).borderRadius===getComputedStyle(fileResize).borderRadius&&getComputedStyle(projectResize).backgroundColor===getComputedStyle(fileResize).backgroundColor,`${theme}: project/chat and Supervisor/file separators use different visual treatments`);
      const dividerBefore=conversationResize.getBoundingClientRect(),chatLaneBefore=chatLane.getBoundingClientRect(),supervisorLaneBefore=supervisorLane.getBoundingClientRect();
      assert(dividerBefore.left>=chatLaneBefore.right-.6&&dividerBefore.right<=supervisorLaneBefore.left+.6,`${theme}: conversation divider is not contained in the space between Project chats and Supervisors`);
      const dividerStart={x:dividerBefore.left+dividerBefore.width/2,y:dividerBefore.top+dividerBefore.height/2},pointerId=41;
      conversationResize.dispatchEvent(new PointerEvent('pointerdown',{pointerId,button:0,buttons:1,clientX:dividerStart.x,clientY:dividerStart.y,bubbles:true,cancelable:true}));
      window.dispatchEvent(new PointerEvent('pointermove',{pointerId,buttons:1,clientX:dividerStart.x+48,clientY:dividerStart.y,bubbles:true,cancelable:true}));
      window.dispatchEvent(new PointerEvent('pointerup',{pointerId,button:0,clientX:dividerStart.x+48,clientY:dividerStart.y,bubbles:true,cancelable:true}));await flush();
      const dividerAfter=conversationResize.getBoundingClientRect(),chatLaneAfter=chatLane.getBoundingClientRect(),supervisorLaneAfter=supervisorLane.getBoundingClientRect();
      assert(chatLaneAfter.width>chatLaneBefore.width+30&&supervisorLaneAfter.width<supervisorLaneBefore.width-30,`${theme}: pointer resizing did not redistribute Project chats and Supervisors: ${JSON.stringify({before:[chatLaneBefore.width,supervisorLaneBefore.width],after:[chatLaneAfter.width,supervisorLaneAfter.width]})}`);
      assert(dividerAfter.left>=chatLaneAfter.right-.6&&dividerAfter.right<=supervisorLaneAfter.left+.6,`${theme}: conversation divider drifted away from the resized columns`);
      const resizedSupervisorRow=document.querySelector('[data-agent-key="conversation:card-alpha"]');
      const resizedWidths={projectCard:chatCard.getBoundingClientRect().width,projectRow:chatRows[0].getBoundingClientRect().width,supervisorCard:cardA.getBoundingClientRect().width,supervisorRow:resizedSupervisorRow.getBoundingClientRect().width};
      assert(Math.abs(resizedWidths.projectCard-resizedWidths.projectRow)<1&&Math.abs(resizedWidths.supervisorCard-resizedWidths.supervisorRow)<1,`${theme}: resizing broke card-to-conversation width symmetry: ${JSON.stringify(resizedWidths)}`);
      const keyboardChatWidth=chatLaneAfter.width;conversationResize.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowLeft',bubbles:true,cancelable:true}));await flush();
      assert(chatLane.getBoundingClientRect().width<keyboardChatWidth-10,`${theme}: keyboard resizing did not move the conversation divider by a visible step`);
      const resize=document.querySelector('[aria-label="Resize projects column"]');resize.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowRight',bubbles:true}));await flush();
      assert(document.querySelector('.board-columns').style.getPropertyValue('--board-project-width'),`${theme}: keyboard resizing did not update the current board layout`);
      assert(!sent.some(message=>message.action?.type==='save_layout'),`${theme}: presentation-only layout leaked into native session persistence`);
      assert(!sent.some(message=>/stop_chat|close_chat_preview|submit_chat/.test(message.action?.type||'')),`${theme}: presentation controls changed a task runtime`);
      chatLayer.scrollTop=0;(chatCard.querySelector('.agent-console-header')||chatCard).dispatchEvent(new WheelEvent('wheel',{deltaY:180,bubbles:true,cancelable:true}));await flush();
      assert(chatLayer.scrollTop>0,`${theme}: the mouse wheel cannot move through stacked conversation cards`);
      const scrolledDown=chatLayer.scrollTop;(chatCard.querySelector('.agent-console-header')||chatCard).dispatchEvent(new WheelEvent('wheel',{deltaY:-180,bubbles:true,cancelable:true}));await flush();
      assert(chatLayer.scrollTop<scrolledDown,`${theme}: the mouse wheel cannot move back up through stacked conversation cards`);
      sent.length=0;secondCard.querySelector('[data-action="close"]').click();await flush();
      assert(sent.some(message=>message.message_type==='project_board'&&message.action.type==='close_chat_preview'&&message.action.chat_id==='alpha-second'),`${theme}: closing one project chat did not address that card`);
      state.projectChatPreviews=[alphaPreview];window.renderKnowledgeSurfaceState(state);await flush();
      assert(document.querySelector('[data-project-chat-id="alpha-chat"]')===chatCard&&!document.querySelector('[data-project-chat-id="alpha-second"]'),`${theme}: closing one chat removed or remounted the remaining card`);
      sent.length=0;document.querySelector('[aria-label="New supervisor"]').click();await flush();
      assert(sent.some(message=>message.message_type==='project_board'&&message.action.type==='new_agent'&&message.action.node_key==='entity:alpha'&&message.action.chat_id==='alpha-chat'),`${theme}: a supervisor was not associated with the selected project/chat`);
      assert(!sent.some(message=>/continue_knowledge_agent|run_knowledge_agent/.test(message.message_type)),`${theme}: creating an agent started an unsolicited turn`);
      sent.length=0;document.querySelector('[aria-label="Open alpha.rs"]').click();await flush();
      assert(sent.some(message=>message.message_type==='project_board'&&message.action.type==='open_file'&&message.action.root===projects[0].path&&message.action.path==='alpha.rs'),`${theme}: file navigation did not preserve its root`);
      cardA.querySelector('[data-action="close"]').click();await flush();
      assert(!sent.some(message=>message.message_type==='stop_knowledge_agent'),`${theme}: closing a card stopped its agent`);
      sent.length=0;document.querySelector('[data-agent-key="conversation:card-alpha"]').parentElement.querySelector('[data-supervisor-menu-trigger]').click();await flush();
      document.querySelector('[data-supervisor-action="delete"]').click();await flush();
      const deleteDialog=document.querySelector('[role="alertdialog"][aria-labelledby="delete-supervisor-title"]');
      assert(deleteDialog?.textContent.includes('Linked project conversations and project files remain unchanged')&&!sent.some(message=>message.action?.type==='delete_agent'),`${theme}: Supervisor deletion was dispatched before its bounded confirmation`);
      deleteDialog.querySelector('[data-supervisor-confirm-delete]').click();await flush();
      assert(sent.some(message=>message.message_type==='project_board'&&message.action.type==='delete_agent'&&message.action.node_key==='conversation:card-alpha'),`${theme}: Delete did not preserve the exact Supervisor identity`);
    }
  } catch(error){startupErrors.push(String(error));}
  finally{window.renderKnowledgeSurfaceState({expanded:false,graph:{visualGraph:{nodes:[],edges:[]}}});await flush();bridge.postMessage=originalPost;}
}

// Follow structured task activity in the real board, without a model or disk write.
{
  const bridge=window.chrome.webview, originalPost=bridge.postMessage, sent=[];
  const flush=async()=>{await Promise.resolve();await Promise.resolve();await Promise.resolve();};
  const assert=(condition,message)=>{if(!condition)throw new Error(message);};
  try {
    bridge.postMessage=message=>sent.push(JSON.parse(message));
    for(const theme of ['central','central_dark']) {
      const file=(path,state='working',operation='read')=>({path,state,operation,activeOperations:state==='working'?1:0});
      const task=(owner,label,files,extra={})=>({owner,chatId:owner.slice(5),label,state:'working',files,...extra});
      const projects=['work-a','work-b'].map(name=>({id:`C:/fixture/${name}`,path:`C:/fixture/${name}`,nodeKey:`entity:${name}`,source:'local',name,active:name==='work-a',activity:[]}));
      projects[0].activity=[task('chat:first','Read source',[file('src/main.rs'),file('src/done.rs','done')]),task('chat:second','Edit source',[file('src/main.rs','working','edit'),file('src/new.rs','working','edit')])];
      projects[0].activity.push(task('graph:active-supervisor','Review changes',[file('src/supervisor.rs')],{chatId:null,nodeKey:'conversation:active-supervisor'}));
      projects[1].activity=[task('chat:other','Other project',[file('other.rs')])];
      const state={theme,expanded:true,projectRegistry:{projects},
        projectChats:[{id:'first',title:'Read source',projectRoot:projects[0].path},{id:'second',title:'Edit source',projectRoot:projects[0].path,verification:{state:'working',summary:'The linked agent is working.'}},{id:'other',title:'Other project',projectRoot:projects[1].path}],
        graph:{visualGraph:{nodes:[],edges:[]}},knowledgeAgents:{bindings:[{recordType:'entity',recordId:'work-a',conversationId:'active-supervisor',name:'Active supervisor',projectDirectory:projects[0].path}],activeNodeKeys:[]},knowledgeRuns:[{nodeKey:'conversation:active-supervisor',agent:{active:true,phase:'working'},verification:{state:'working',summary:'The linked agent is working.'}}],
        workspace:{root:projects[0].path,entries:[{path:'src/main.rs',kind:'file'},{path:'src/done.rs',kind:'file'},{path:'src/supervisor.rs',kind:'file'}]}};
      window.renderKnowledgeSurfaceState(state);await flush();
      const following=document.querySelector('[data-work-owner="chat:second"]');
      assert(following?.closest('.project-card')?.textContent.includes('work-a'),`${theme}: working chat is not linked beneath its project`);
      assert(document.querySelector('[data-project-node="entity:work-a"] [data-collaboration-active]'),`${theme}: the active project does not show collaboration`);
      following.click();await flush();
      assert(document.querySelector('[data-chat-id="second"] [data-task-verification="working"]')&&!document.querySelector('[data-chat-id="second"] [data-collaboration-active]'),`${theme}: the active project chat does not use the single Working state`);
      assert(document.querySelector('[data-agent-key="conversation:active-supervisor"]')?.parentElement?.querySelector('[data-task-verification="working"]')&&!document.querySelector('[data-agent-key="conversation:active-supervisor"] [data-collaboration-active]'),`${theme}: the active Supervisor does not use the single Working state`);
      let panel=document.querySelector('.file-activity');
      assert(panel&&panel.querySelector('[data-file-activity="src/new.rs"]')&&!panel.querySelector('[data-file-activity="src/done.rs"]'),`${theme}: following a chat did not filter its files`);
      assert(panel.querySelector('[data-file-activity="src/main.rs"] [data-collaboration-active]'),`${theme}: the active file activity row does not show collaboration`);
      assert(document.querySelector('[data-chat-id="second"]')?.classList.contains('active'),`${theme}: following work did not select its conversation`);
      assert(!panel.querySelector('[data-file-activity="other.rs"]'),`${theme}: background project files leaked`);
      [...panel.querySelectorAll('.work-filters button')].find(button=>button.textContent.includes('All tasks')).click();await flush();
      assert(panel.querySelector('[data-file-activity="src/main.rs"]').textContent.includes('2 operations'),`${theme}: concurrent operations on a shared file are missing`);
      assert(document.querySelector('[aria-label="Expand src"]').textContent.includes('3 active'),`${theme}: collapsed folder hides its active files`);
      document.querySelector('[aria-label="Expand src"]').click();await flush();
      assert(document.querySelector('[aria-label="Open src/main.rs"] [data-collaboration-active]'),`${theme}: the exact active file row does not show collaboration`);
      for(const width of [1024,1920,2560,3840]) {
        document.getElementById('graph-shell').style.width=width+'px';await flush();
        const lane=document.querySelector('[data-board-lane="files"]').getBoundingClientRect();
        const bounds=panel.getBoundingClientRect();
        assert(bounds.width>150&&bounds.left>=lane.left&&bounds.right<=lane.right+1,`${theme}/${width}: activity escapes its file lane`);
        assert(getComputedStyle(panel).backgroundColor!==getComputedStyle(panel).color,`${theme}/${width}: activity text is invisible`);
      }
      document.getElementById('graph-shell').style.width='';
      [...panel.querySelectorAll('.work-filters button')].find(button=>button.textContent.includes('Edit source')).click();await flush();
      projects[0].activity[1].state='done';projects[0].activity[1].files=projects[0].activity[1].files.map(f=>({...f,state:'done',activeOperations:0}));
      state.projectChats[1].verification={state:'ready',summary:'The task passed its final review.'};
      window.renderKnowledgeSurfaceState(state);await flush();
      [...panel.querySelectorAll('.work-filters button')].find(button=>button.textContent.includes('All tasks')).click();await flush();
      assert(panel.querySelector('[data-file-activity="src/main.rs"]').textContent.includes('Reading'),`${theme}: completing an edit hid an independent active read`);
      assert(!document.querySelector('[data-work-owner="chat:second"]'),`${theme}: completed task stayed active in the project summary`);
      assert(!document.querySelector('[data-chat-id="second"] [data-collaboration-active]')&&!panel.querySelector('[data-file-activity="src/new.rs"] [data-collaboration-active]'),`${theme}: completed chat or file kept its collaboration indicator`);
      projects[0].activity[0].state='stopped';projects[0].activity[0].files[0]=file('src/main.rs','stopped');
      projects[0].activity[2].state='done';projects[0].activity[2].files=projects[0].activity[2].files.map(f=>({...f,state:'done',activeOperations:0}));
      state.knowledgeRuns[0].agent={active:false,phase:'idle'};
      state.knowledgeRuns[0].verification={state:'ready',summary:'The task passed its final review.'};
      window.renderKnowledgeSurfaceState(state);await flush();
      assert(!document.querySelector('[data-work-owner="chat:first"]')&&!panel.querySelector('[data-file-activity="src/main.rs"] [data-collaboration-active]'),`${theme}: stopping work left a live file/task marker`);
      assert(!document.querySelector('[data-project-node="entity:work-a"] [data-collaboration-active]')&&!document.querySelector('[data-agent-key="conversation:active-supervisor"] [data-collaboration-active]'),`${theme}: inactive project or Supervisor kept its collaboration indicator`);
      document.querySelector('[data-work-owner="chat:other"]').click();await flush();
      assert(document.querySelector('[data-file-activity="other.rs"]')&&!document.querySelector('[data-file-activity="src/main.rs"]'),`${theme}: task link failed to switch project scope`);
      assert(!sent.some(message=>/continue_knowledge_agent|run_knowledge_agent|app_server_turn/.test(message.message_type)),`${theme}: following activity submitted a turn`);

      const add=[...document.querySelectorAll('[data-central-agent-svelte="project-registry"] button')].find(button=>button.textContent.includes('Add project'));
      add.click();await flush();
      [...document.querySelectorAll('.add-dialog .add-choice')].find(button=>button.textContent.includes('New project')).click();await flush();
      const name=document.querySelector('.add-dialog input');name.value='New fixture';name.dispatchEvent(new Event('input',{bubbles:true}));await flush();
      sent.length=0;document.querySelector('.add-dialog form').requestSubmit();await flush();
      assert(sent.some(message=>message.message_type==='create_local_project'&&message.name==='New fixture'),`${theme}: New project did not reach native creation`);
      state.projectRegistry.import={kind:'create',error:true,message:'A folder named New fixture already exists.'};window.renderKnowledgeSurfaceState(state);await flush();
      assert(document.querySelector('.add-dialog [role="alert"]')&&name.value==='New fixture',`${theme}: creation failure hid its error or lost the name`);
      state.projectRegistry.import={active:false,error:false};window.renderKnowledgeSurfaceState(state);await flush();
      assert(document.querySelector('.add-dialog')&&name.value==='New fixture',`${theme}: cancelled folder selection lost the draft`);
      state.projectRegistry.import={kind:'create',error:false,message:'New fixture added. Detecting its metadata…'};window.renderKnowledgeSurfaceState(state);await flush();
      assert(!document.querySelector('.add-dialog'),`${theme}: successful creation left the dialog open`);
    }
  } catch(error){startupErrors.push(String(error));}
  finally{document.getElementById('graph-shell').style.width='';window.renderKnowledgeSurfaceState({expanded:false,graph:{visualGraph:{nodes:[],edges:[]}}});await flush();bridge.postMessage=originalPost;}
}

// Exercise the real host/card boundary. Standalone composer-state fixtures do
// not cover initial mounting, profile population or the native submit bridge.
{
  const nativeBridge = window.chrome.webview;
  const postMessage = nativeBridge.postMessage;
  const messages = [];
  const flush = async () => { await Promise.resolve(); await Promise.resolve(); };
  const assert = (condition, message) => { if (!condition) throw new Error(message); };
  const profile = {
    id: 'codex_app_server', label: 'Codex',
    provider: {phase: 'ready', authenticated: true},
    selection: {model: 'fixture-model', effort: 'low', serviceTier: null},
    models: [{model: 'fixture-model', displayName: 'Fixture', defaultReasoningEffort: 'low',
      supportedReasoningEfforts: [{reasoningEffort: 'low'}], serviceTiers: []}],
  };
  try {
    const capture = message => messages.push(JSON.parse(message));
    nativeBridge.postMessage = capture;
    assert(nativeBridge.postMessage === capture, 'Startup IPC capture was not installed');
    for (const theme of ['central', 'central_dark']) {
      const id = `submit-${theme}`;
      const nodeKey = `entity:${id}`, owner = `graph:${nodeKey}`;
      const signedOut = {...profile, provider:{phase:'unavailable',authenticated:false,detail:'Sign in to Codex in Supervisor Settings → AI provider.'}};
      const state = {theme, expanded: true, providers: [signedOut],
        graph: {visualGraph: {nodes: [{key: nodeKey,
          recordType: 'entity', recordId: id, label: 'Submit fixture', entityKind: 'project'}], edges: []}},
        projectRegistry:{projects:[{id:'C:/fixture',path:'C:/fixture',name:'Submit fixture',source:'local',nodeKey,active:true}]},
        knowledgeAgents: {bindings: [], activeNodeKeys: [nodeKey]}, knowledgeRuns: []};
      window.renderKnowledgeSurfaceState(state);
      await flush();
      const card = document.querySelector(`.agent-console[data-node-key="${nodeKey}"]`);
      assert(card, `${theme}: host did not open the agent card`);
      const input = card.querySelector('[data-role="message"]');
      const button = card.querySelector('[data-action="submit"]');
      const type = text => { input.value = text; input.dispatchEvent(new Event('input', {bubbles:true})); };
      const sends = () => messages.filter(message => ['set_knowledge_agent', 'continue_knowledge_agent'].includes(message.message_type));
      const receipt = () => window.dispatchEvent(new CustomEvent('central-agent:submission-accepted', {detail:{owner,input:input.value}}));
      type('Host button submission'); await flush();
      messages.length = 0;
      assert(button.disabled && card.querySelector('.graph-availability')?.textContent === signedOut.provider.detail && button.title === signedOut.provider.detail, `${theme}: signed-out card hides the reason Send is disabled`);
      input.dispatchEvent(new KeyboardEvent('keydown', {key:'Enter', bubbles:true, cancelable:true})); button.click(); await flush();
      assert(!sends().length && input.value === 'Host button submission', `${theme}: signed-out submission bypassed authentication or lost its draft`);
      state.providers = [{...profile, provider:{phase:'checking',authenticated:false,detail:'Checking the saved sign-in…'}}];
      window.renderKnowledgeSurfaceState(state); await flush();
      assert(button.disabled && card.querySelector('.graph-availability')?.textContent === state.providers[0].provider.detail, `${theme}: pending connection claimed it was ready`);
      state.providers = [profile]; window.renderKnowledgeSurfaceState(state); await flush();
      assert(document.querySelector(`.agent-console[data-node-key="${nodeKey}"]`) === card && input.value === 'Host button submission' && !card.querySelector('.graph-availability'), `${theme}: successful sign-in remounted the card, lost its draft or left stale guidance`);
      assert(!button.disabled, `${theme}: host left Send disabled with a ready profile and a prompt`);
      button.scrollIntoView({block:'nearest'});await flush();
      const bounds = button.getBoundingClientRect();
      const hit = document.elementFromPoint(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
      assert(button.contains(hit), `${theme}: another graph element covers Send: ${JSON.stringify({button:bounds.toJSON(),card:card.getBoundingClientRect().toJSON(),viewport:[innerWidth,innerHeight],hit:hit?.outerHTML.slice(0,240),layer:card.parentElement.getBoundingClientRect().toJSON()})}`);
      messages.length = 0; input.dispatchEvent(new FocusEvent('focus')); await flush();
      assert(messages.some(message=>message.message_type==='app_server_prepare' && message.owner===`graph:${nodeKey}`), `${theme}: focused card did not prepare its own conversation`);
      messages.length = 0; button.click(); button.click(); await flush();
      assert(sends().length === 2 && sends()[0].message_type === 'set_knowledge_agent' && sends()[1].message_type === 'continue_knowledge_agent', `${theme}: Send did not assign then submit exactly once: ${JSON.stringify(messages)}`);
      assert(sends().every(message => message.id === id && message.record_type === 'entity' && message.conversation_id === null) && sends()[1].message === input.value, `${theme}: host submitted the wrong owner or prompt`);
      assert(Number.isFinite(sends()[1].timing?.clickedAtMs), `${theme}: graph bridge dropped click timing`);
      receipt(); await flush(); assert(input.value === '', `${theme}: host receipt did not clear the accepted prompt`);
      type('Host keyboard submission'); await flush();
      state.providers = [signedOut]; window.renderKnowledgeSurfaceState(state); await flush();
      assert(button.disabled && input.value === 'Host keyboard submission', `${theme}: losing authentication enabled Send or discarded later typing`);
      state.providers = [profile]; window.renderKnowledgeSurfaceState(state); await flush(); messages.length = 0;
      input.dispatchEvent(new KeyboardEvent('keydown', {key:'Enter', bubbles:true, cancelable:true})); await flush();
      assert(sends().length === 2 && sends()[1].message === input.value, `${theme}: Enter did not reach the host submission bridge`);
      receipt(); await flush();
    }
  } catch (error) { startupErrors.push(String(error)); }
  finally {
    window.renderKnowledgeSurfaceState({expanded:false, graph:{visualGraph:{nodes:[],edges:[]}}});
    await flush();
    nativeBridge.postMessage = postMessage;
  }
}

// The retained board workflows use the real components and host IPC with synthetic replies.
{
  const bridge=window.chrome.webview,post=bridge.postMessage,sent=[];
  const flush=async()=>{await Promise.resolve();await Promise.resolve();await new Promise(resolve=>requestAnimationFrame(resolve));await Promise.resolve();};
  const assert=(condition,message)=>{if(!condition)throw new Error(message);};
  try {
    bridge.postMessage=raw=>sent.push(JSON.parse(raw));
    for(const theme of ['central','central_dark']) {
      const nodeKey=`entity:workflow-${theme}`,agentKey=`conversation:reviewer-${theme}`,chatId=`worker-${theme}`,root=`C:/fixture/workflow-${theme}`;
      const provider={id:'codex_app_server',label:'Codex',provider:{phase:'ready',authenticated:true},selection:{model:'fixture',effort:'low',serviceTier:null},models:[{model:'fixture',displayName:'Fixture',defaultReasoningEffort:'low',supportedReasoningEfforts:[{reasoningEffort:'low'}],serviceTiers:[]}]};
      const state={theme,expanded:true,providers:[provider],graph:{visualGraph:{nodes:[{key:nodeKey,recordType:'entity',recordId:`workflow-${theme}`,entityKind:'project',label:'Workflow'}],edges:[]}},projectRegistry:{projects:[{id:root,path:root,nodeKey,source:'local',name:'Workflow',active:true,activity:[{owner:`chat:${chatId}`,chatId,label:'Worker',state:'done',files:[{path:'src/main.rs',state:'done',operation:'edit',activeOperations:0}]}]}]},projectChats:[{id:chatId,title:'Worker',projectRoot:root}],projectChatPreviews:[{id:chatId,title:'Worker',provider:'codex_app_server',selection:provider.selection,agent:{active:false,phase:'idle',steps:[]},messages:[],conversationState:{owner:`chat:${chatId}`,active:false,busy:false,nativeMessages:[],fileAttachments:[],contextAttachments:{owner:`chat:${chatId}`,tabs:[],shells:[]}}}],knowledgeAgents:{bindings:[{recordType:'entity',recordId:`workflow-${theme}`,conversationId:`reviewer-${theme}`,name:'Reviewer',projectDirectory:root,projectChatId:chatId,provider:'codex_app_server',selection:provider.selection}],activeNodeKeys:[]},knowledgeRuns:[{nodeKey:agentKey,agent:{active:false,phase:'idle'}}],workspace:{root,entries:[{path:'src/main.rs',kind:'file',iconKey:'rust'}]}};
      window.renderKnowledgeSurfaceState(state);await flush();await flush();
      const chatRow=document.querySelector(`[data-chat-id="${chatId}"]`),card=document.querySelector(`[data-project-chat-id="${chatId}"]`);
      assert(card&&card.closest('.chat-inline')?.previousElementSibling===chatRow,`${theme}: the retained project chat is not mounted directly below its row`);
      document.querySelector(`[data-agent-key="${agentKey}"]`).click();await flush();
      const supervisorSettings=document.querySelector(`[data-board-conversation-menu="graph:${agentKey}"]`),supervisorPanel=supervisorSettings?.querySelector('[popover]');
      assert(supervisorSettings&&supervisorPanel&&!supervisorPanel.matches(':popover-open'),`${theme}: Supervisor association did not remain in a compact menu`);
      supervisorSettings.querySelector('.menu-trigger').click();await flush();
      const association=supervisorSettings.querySelector('[data-settings-picker="board-agent-chat"]'),associationButton=association?.querySelector('.settings-picker-button');
      assert(associationButton&&supervisorPanel.matches(':popover-open'),`${theme}: the compact supervised-conversation selector did not open`);
      sent.length=0;associationButton.click();await flush();
      [...association.querySelectorAll('[role="option"]')].find(option=>option.textContent.includes('Whole project')).click();await flush();
      assert(sent.some(message=>message.action?.type==='link_chat'&&message.action.node_key===agentKey&&message.action.chat_id===null),`${theme}: the compact selector lost its exact Supervisor owner`);
      for(const width of [1024,1920,2560,3840]){
        document.getElementById('graph-shell').style.width=`${width}px`;window.dispatchEvent(new Event('resize'));await flush();
        const bounds=supervisorPanel.getBoundingClientRect();
        assert(bounds.left>=0&&bounds.right<=innerWidth+1&&bounds.top>=0&&bounds.bottom<=innerHeight+1&&supervisorPanel.scrollWidth<=supervisorPanel.clientWidth+1,`${theme}/${width}: the compact menu escaped the viewport`);
      }
      document.getElementById('graph-shell').style.width='';window.dispatchEvent(new Event('resize'));await flush();
      supervisorPanel.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true,cancelable:true}));await flush();
      assert(!supervisorPanel.matches(':popover-open'),`${theme}: Escape did not close the compact Supervisor menu`);
      sent.length=0;document.querySelector(`[data-chat-id="${chatId}"] .workspace-row-menu`).click();await flush();document.querySelector('[data-chat-action="worktree"]').click();await flush();
      const worktree=document.querySelector('[aria-labelledby="worktree-title"]'),name=worktree.querySelector('input');name.value='Isolated login';name.dispatchEvent(new Event('input',{bubbles:true}));worktree.querySelector('form').requestSubmit();await flush();
      assert(sent.some(message=>message.action?.type==='create_worktree'&&message.action.root===root&&message.action.name==='Isolated login'),`${theme}: isolated task did not reach native worktree creation with its project root`);
      window.renderKnowledgeSurfaceState({...state,expanded:false});await flush();
      assert(!supervisorPanel.matches(':popover-open'),`${theme}: closing the board left its compact menu floating`);
    }
  }catch(error){startupErrors.push(String(error));}
  finally {bridge.postMessage=post;window.renderKnowledgeSurfaceState({expanded:false,graph:{visualGraph:{nodes:[],edges:[]}}});await flush();}
}
