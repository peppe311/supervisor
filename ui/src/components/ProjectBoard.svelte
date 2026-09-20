<script lang="ts">
  import {onMount,tick} from 'svelte';
  import ProjectRegistry from './ProjectRegistry.svelte';
  import ProjectChatTree from './ProjectChatTree.svelte';
  import SettingsPicker from './SettingsPicker.svelte';
  import BoardFiles from './BoardFiles.svelte';
  import BoardConversationMenu from './BoardConversationMenu.svelte';
  import ProjectFileActivity from './ProjectFileActivity.svelte';
  import TaskVerificationStatus from './TaskVerificationStatus.svelte';
  import {boardLayout,readBoardLayout,writeBoardLayout,type BoardLayout} from '../lib/board-layout';
  import {isWorking,agentInProject,agentKey,projectChats,sameLocalPath,type BoardState,type BoardProject,type BoardAgent} from '../lib/project-board';
  import type {ProjectChat} from '../lib/chat-tree';
  let {eventTarget}:{eventTarget:HTMLElement}=$props();
  let snapshot=$state<BoardState>({});
  let registry:ProjectRegistry;
  let selectedKey=$state('');
  let selectedChat=$state<string|null>(null);
  let selectedAgent=$state<string|null>(null);
  let workOwner=$state('');
  let chatQuery=$state('');
  let error=$state('');
  let chatMenu=$state<ProjectChat|null>(null);
  let menuElement=$state<HTMLDivElement>();
  type ChatRenameDialog={chatId:string;title:string};
  let chatRenameDialog=$state<ChatRenameDialog|null>(null);
  let chatDeleteDialog=$state<ProjectChat|null>(null);
  let chatTitle=$state('');
  let chatTitleError=$state('');
  let chatTitleInput=$state<HTMLInputElement>();
  type SupervisorDialog={kind:'rename'|'delete';nodeKey:string;name:string};
  let agentMenu=$state<BoardAgent|null>(null);
  let agentMenuElement=$state<HTMLDivElement>();
  let agentMenuPosition=$state({top:0,left:0});
  let supervisorDialog=$state<SupervisorDialog|null>(null);
  let supervisorName=$state('');
  let supervisorNameError=$state('');
  let supervisorNameInput=$state<HTMLInputElement>();
  let chatLane=$state<HTMLElement>();
  let chatScroll=$state<HTMLDivElement>();
  let chatWindowLayer=$state<HTMLDivElement>();
  let agentLane=$state<HTMLElement>();
  let agentList=$state<HTMLDivElement>();
  let agentWindowLayer=$state<HTMLDivElement>();
  let initialized=false;
  let worktreeRoot='';
  let layout=$state<BoardLayout>(boardLayout());
  let layoutKey='';
  let restored=false;
  let boardColumns:HTMLDivElement;
  let conversationLanes:HTMLDivElement;
  const slots=new Map<string,HTMLElement>();
  let parking:HTMLDivElement;
  let resizeCleanup:(()=>void)|undefined;
  let isolate=$state(false),isolateName=$state(''),isolateError=$state('');
  const projects=$derived(snapshot.projectRegistry?.projects||[]);
  const selected=$derived(projects.find(project=>project.nodeKey===selectedKey)||null);
  const work=$derived(selected?.activity||[]);
  const chats=$derived(projectChats(snapshot.projectChats||[],selected).map(chat=>({...chat,
    agentActive:chat.agentActive||work.some(task=>task.chatId===chat.id&&isWorking(task.state))})));
  const visibleChats=$derived(chats.filter(chat=>!chatQuery||chat.title.toLocaleLowerCase().includes(chatQuery.toLocaleLowerCase())));
  const detached=$derived((snapshot.knowledgeAgents?.bindings||[]).filter(agent=>!projects.some(project=>agentInProject(agent,project))));
  const agents=$derived(selectedKey==='saved-agents'?detached:(snapshot.knowledgeAgents?.bindings||[]).filter(agent=>selected&&agentInProject(agent,selected)));
  const remoteEntries=$derived(selected?.metadata?.files||[]);
  const entries=$derived(selected?.source==='ssh'?remoteEntries:selected&&snapshot.workspace?.root&&sameLocalPath(snapshot.workspace.root,selected.path)?snapshot.workspace.entries||[]:[]);
  const openChatIds=$derived((snapshot.projectChatPreviews||[]).map(chat=>chat.id));
  function saveLayout():void {
    if(!selectedKey)return;
    writeBoardLayout(selectedKey,boardLayout(layout));
  }
  function restoreLayout(key:string):void {
    layoutKey=key;layout=readBoardLayout(key);restored=false;
    selectedChat=layout.selectedChat;selectedAgent=layout.selectedAgent;
  }
  function cardSlot(element:HTMLElement,key:string):{update:(key:string)=>void;destroy:()=>void} {
    slots.set(key,element);void tick().then(arrangeCards);
    const release=()=>{for(const child of [...element.children])parking?.append(child);if(slots.get(key)===element)slots.delete(key);};
    return{update:(next:string)=>{if(next===key)return;release();key=next;slots.set(key,element);void tick().then(arrangeCards);},destroy:release};
  }
  function arrangeCards():void {
    if(!eventTarget.isConnected)return;
    for(const card of eventTarget.querySelectorAll<HTMLElement>('.agent-console[data-central-agent-svelte="knowledge-agent-popup"]')){
      const owner=card.dataset.projectChatId?`chat:${card.dataset.projectChatId}`:`graph:${card.dataset.nodeKey}`;
      const slot=slots.get(owner);
      if(slot&&card.parentElement!==slot)slot.append(card);
      const visible=Boolean(slot)&&!slot?.closest('[hidden]');
      card.hidden=!visible;
      card.dataset.boardCard='true';
      const focused=layout.focus===owner;
      const minimized=layout.minimized.includes(owner)||Boolean(layout.focus&&!focused);
      if(card.dataset.cardFocused!==String(focused)||card.dataset.cardMinimized!==String(minimized))card.dispatchEvent(new CustomEvent('central-agent:board-card-presentation',{detail:{owner,focused,minimized}}));
    }
    sizeConversationCards();
  }
  function toggleFocus(owner:string):void {
    layout.focus=layout.focus===owner?'':owner;
    if(layout.focus)layout.minimized=layout.minimized.filter(id=>id!==owner);
    saveLayout();void tick().then(()=>{arrangeCards();slots.get(owner)?.scrollIntoView({block:'nearest'});});
  }
  function resizeLane(event:PointerEvent,which:'projects'|'files'|'chats'):void {
    if(event.button!==0)return;event.preventDefault();resizeCleanup?.();
    const start=event.clientX;
    const projectWidth=boardColumns.querySelector<HTMLElement>('.project-lane')!.getBoundingClientRect().width;
    const fileWidth=boardColumns.querySelector<HTMLElement>('.file-lane')!.getBoundingClientRect().width;
    const chatWidth=chatLane?.getBoundingClientRect().width||0;
    const supervisorWidth=agentLane?.getBoundingClientRect().width||0;
    const share=layout.chatShare,total=Math.max(1,chatWidth+supervisorWidth||conversationLanes.getBoundingClientRect().width);
    const root=document.documentElement,previousCursor=root.style.cursor,previousSelection=root.style.userSelect;
    root.style.cursor='col-resize';root.style.userSelect='none';
    const move=(e:PointerEvent)=>{
      if(which==='projects')layout.projectsWidth=Math.min(480,Math.max(180,projectWidth+e.clientX-start));
      else if(which==='files')layout.filesWidth=Math.min(600,Math.max(230,fileWidth-e.clientX+start));
      else layout.chatShare=Math.min(.75,Math.max(.25,share+(e.clientX-start)/total));
      void tick().then(sizeConversationCards);
    };
    const end=()=>{window.removeEventListener('pointermove',move);window.removeEventListener('pointerup',end);window.removeEventListener('pointercancel',end);root.style.cursor=previousCursor;root.style.userSelect=previousSelection;resizeCleanup=undefined;saveLayout();void tick().then(sizeConversationCards);};
    window.addEventListener('pointermove',move);window.addEventListener('pointerup',end);window.addEventListener('pointercancel',end);resizeCleanup=end;
  }
  function resizeKey(event:KeyboardEvent,which:'projects'|'files'|'chats'):void {
    if(!['ArrowLeft','ArrowRight'].includes(event.key))return;event.preventDefault();
    const amount=event.key==='ArrowRight'?20:-20;
    if(which==='projects')layout.projectsWidth=Math.max(180,Math.min(480,(layout.projectsWidth||240)+amount));
    else if(which==='files')layout.filesWidth=Math.max(230,Math.min(600,(layout.filesWidth||320)-amount));
    else {
      const total=Math.max(1,(chatLane?.getBoundingClientRect().width||0)+(agentLane?.getBoundingClientRect().width||0)||conversationLanes.getBoundingClientRect().width);
      layout.chatShare=Math.max(.25,Math.min(.75,layout.chatShare+amount/total));
    }
    saveLayout();void tick().then(sizeConversationCards);
  }

  function dispatch(action:string,detail:Record<string,unknown>={}):void {
    eventTarget.dispatchEvent(new CustomEvent('central-agent:project-board',{bubbles:true,detail:{action,...detail}}));
  }
  function persist():void {
    try{localStorage.setItem('supervisor-project-board-v1',JSON.stringify({selectedKey,selectedChat,selectedAgent}));}catch{}
    layout.selectedChat=selectedChat;layout.selectedAgent=selectedAgent;saveLayout();
  }
  function requestChatPreview(chatId:string):void {
    if(!layout.openChats.includes(chatId)){layout.openChats=[...layout.openChats,chatId];saveLayout();}
    if(!openChatIds.includes(chatId))dispatch('preview-chat',{chatId});
    void tick().then(()=>eventTarget.querySelector<HTMLElement>(`[data-project-chat-id="${CSS.escape(chatId)}"]`)?.scrollIntoView({block:'nearest'}));
  }
  export function projectKey():string {return selectedKey;}
  export function agentVisible(key:string):boolean {
    return agents.some(agent=>agentKey(agent)===key)||key===selectedKey;
  }
  export function update(next:BoardState):void {
    const opening=next.expanded&&!snapshot.expanded;
    const previouslyOpen=(snapshot.projectChatPreviews||[]).map(chat=>chat.id);
    snapshot=next;
    const nextBindings=next.knowledgeAgents?.bindings||[];
    if(chatRenameDialog&&!(next.projectChats||[]).some(chat=>chat.id===chatRenameDialog?.chatId))chatRenameDialog=null;
    if(chatDeleteDialog&&!(next.projectChats||[]).some(chat=>chat.id===chatDeleteDialog?.id))chatDeleteDialog=null;
    const openAgentMenu=agentMenu;
    if(openAgentMenu&&!nextBindings.some(agent=>agentKey(agent)===agentKey(openAgentMenu)))agentMenu=null;
    if(supervisorDialog&&!nextBindings.some(agent=>agentKey(agent)===supervisorDialog?.nodeKey))supervisorDialog=null;
    if(selectedAgent&&!nextBindings.some(agent=>agentKey(agent)===selectedAgent)){selectedAgent=null;persist();}
    const nextOpen=(next.projectChatPreviews||[]).map(chat=>chat.id);
    if(selectedChat&&previouslyOpen.includes(selectedChat)&&!nextOpen.includes(selectedChat)){
      selectedChat=nextOpen.at(-1)||null;workOwner='';persist();
    }
    const projects=next.projectRegistry?.projects||[];
    if(!initialized){
      initialized=projects.length>0;
      try{const saved=JSON.parse(localStorage.getItem('supervisor-project-board-v1')||'{}');
        selectedKey=saved.selectedKey||'';selectedChat=saved.selectedChat||null;selectedAgent=saved.selectedAgent||null;}catch{}
      const savedProject=projects.find(project=>project.nodeKey===selectedKey);
      if(savedProject?.source==='local'&&!savedProject.active)selectedKey='';
    }
    if(opening&&projects.some(project=>project.nodeKey===selectedKey&&project.source==='local'&&!project.active))selectedKey='';
    if(selectedKey!=='saved-agents'&&!projects.some(project=>project.nodeKey===selectedKey)) {
      selectedKey=projects.find(project=>project.active)?.nodeKey||'';
      selectedChat=next.activeChatId||null;selectedAgent=null;
    }
    if(!projects.length&&next.knowledgeAgents?.bindings?.length)selectedKey='saved-agents';
    if(worktreeRoot){const created=projects.find(project=>sameLocalPath(project.path,worktreeRoot));if(created){selectedKey=created.nodeKey;worktreeRoot='';}}
    if(layoutKey!==selectedKey)restoreLayout(selectedKey);
    const scopedChats=projectChats(next.projectChats||[],projects.find(project=>project.nodeKey===selectedKey)||null);
    if(selectedChat&&!scopedChats.some(chat=>chat.id===selectedChat)){
      selectedChat=null;workOwner='';persist();
    }
    registry?.update(next.projectRegistry as never);
    if(next.expanded&&!restored&&selectedKey){
      restored=true;
      layout.openChats=layout.openChats.filter(id=>scopedChats.some(chat=>chat.id===id));
      for(const id of layout.openChats)if(!nextOpen.includes(id))dispatch('preview-chat',{chatId:id});
      for(const nodeKey of layout.openAgents)if(nextBindings.some(agent=>agentKey(agent)===nodeKey))dispatch('open-agent',{nodeKey});
    }else if(restored){
      const closed=previouslyOpen.filter(id=>!nextOpen.includes(id));
      layout.openChats=[...new Set([...layout.openChats.filter(id=>!closed.includes(id)),...nextOpen.filter(id=>scopedChats.some(chat=>chat.id===id))])];
    }
    saveLayout();void tick().then(arrangeCards);
    if(next.expanded&&selectedChat&&scopedChats.some(chat=>chat.id===selectedChat)&&!(next.projectChatPreviews||[]).some(chat=>chat.id===selectedChat))requestChatPreview(selectedChat);
  }
  export function selectProject(key:string):void {
    const project=projects.find(project=>project.nodeKey===key);
    if(!project)return;
    saveLayout();selectedKey=key;restoreLayout(key);chatMenu=null;agentMenu=null;workOwner='';
    chatQuery='';error='';persist();
    dispatch('select',{source:project.source,id:project.id,nodeKey:key});
  }
  export function revealAgent(key:string):void {
    const agent=(snapshot.knowledgeAgents?.bindings||[]).find(agent=>agentKey(agent)===key);
    const project=agent&&projects.find(project=>agentInProject(agent,project));
    if(project&&selectedKey!==project.nodeKey)selectProject(project.nodeKey);
    else if(agent&&!project)selectedKey='saved-agents';
    selectedAgent=key;layout.openAgents=[...new Set([...layout.openAgents,key])];persist();
  }
  function chooseChat(chat:ProjectChat):void {
    selectedChat=chat.id;selectedAgent=null;workOwner=work.find(task=>task.owner===`chat:${chat.id}`)?.owner||'';chatMenu=null;persist();
    requestChatPreview(chat.id);
  }
  function renameProjectChat():void {
    if(!chatMenu)return;
    chatRenameDialog={chatId:chatMenu.id,title:chatMenu.title||'New chat'};
    chatTitle=chatMenu.title||'New chat';chatTitleError='';chatMenu=null;
    void tick().then(()=>{chatTitleInput?.focus();chatTitleInput?.select();});
  }
  function closeChatRename():void {chatRenameDialog=null;chatTitleError='';}
  function deleteProjectChat():void {
    if(!chatMenu||chatMenu.mutationLocked)return;
    chatDeleteDialog=chatMenu;chatMenu=null;
  }
  function closeChatDelete():void {chatDeleteDialog=null;}
  function confirmDeleteProjectChat():void {
    if(!chatDeleteDialog)return;
    dispatch('delete-chat',{chatId:chatDeleteDialog.id});closeChatDelete();
  }
  function submitChatTitle():void {
    const title=chatTitle.trim().replace(/\s+/g,' ');
    if(!title){chatTitleError='Enter a title for this project chat.';return;}
    if([...title].length>120){chatTitleError='Use no more than 120 characters.';return;}
    if(!chatRenameDialog)return;
    dispatch('rename-chat',{chatId:chatRenameDialog.chatId,title});closeChatRename();
  }
  export function focusWork(key:string,owner:string):void {
    if(key!==selectedKey)selectProject(key);
    workOwner=owner;
    const task=projects.find(project=>project.nodeKey===key)?.activity?.find(task=>task.owner===owner);
    selectedChat=task?.chatId||null;selectedAgent=task?.nodeKey||null;persist();
    if(selectedChat)requestChatPreview(selectedChat);
    if(task?.nodeKey)dispatch('open-agent',{nodeKey:task.nodeKey});
    void tick().then(()=>eventTarget.querySelector<HTMLElement>(`[data-chat-id="${CSS.escape(selectedChat||'')}"]`)?.scrollIntoView({block:'nearest'}));
  }
  function openWorkFile(path:string):void {
    if(selected)dispatch(selected.source==='local'?'open-file':'remote-file',{root:selected.path,path,id:selected.id});
  }
  function openAgent(agent:BoardAgent):void {selectedAgent=agentKey(agent);persist();dispatch('open-agent',{nodeKey:selectedAgent});}
  function toggleAgentMenu(agent:BoardAgent,event:MouseEvent):void {
    event.stopPropagation();chatMenu=null;
    const key=agentKey(agent);
    if(agentMenu&&agentKey(agentMenu)===key){agentMenu=null;return;}
    const bounds=(event.currentTarget as HTMLElement).getBoundingClientRect();
    agentMenuPosition={top:Math.max(8,Math.min(bounds.bottom+6,window.innerHeight-132)),left:Math.max(8,Math.min(bounds.right-196,window.innerWidth-204))};
    agentMenu=agent;
    void tick().then(()=>agentMenuElement?.querySelector<HTMLButtonElement>('button')?.focus());
  }
  function renameSupervisor():void {
    if(!agentMenu)return;
    supervisorDialog={kind:'rename',nodeKey:agentKey(agentMenu),name:agentMenu.name||'Supervisor'};
    supervisorName=agentMenu.name||'Supervisor';supervisorNameError='';agentMenu=null;
    void tick().then(()=>{supervisorNameInput?.focus();supervisorNameInput?.select();});
  }
  function deleteSupervisor():void {
    if(!agentMenu)return;
    supervisorDialog={kind:'delete',nodeKey:agentKey(agentMenu),name:agentMenu.name||'Supervisor'};
    agentMenu=null;
  }
  function closeSupervisorDialog():void {supervisorDialog=null;supervisorNameError='';}
  function submitSupervisorName():void {
    const name=supervisorName.trim();
    if(!name){supervisorNameError='Enter a name for this supervisor.';return;}
    if(new TextEncoder().encode(name).length>80){supervisorNameError='Use no more than 80 characters.';return;}
    if(!supervisorDialog||supervisorDialog.kind!=='rename')return;
    dispatch('rename-agent',{nodeKey:supervisorDialog.nodeKey,name});closeSupervisorDialog();
  }
  function confirmDeleteSupervisor():void {
    if(!supervisorDialog||supervisorDialog.kind!=='delete')return;
    dispatch('delete-agent',{nodeKey:supervisorDialog.nodeKey});closeSupervisorDialog();
  }
  function boardKeydown(event:KeyboardEvent):void {
    if(event.key!=='Escape')return;
    if(chatRenameDialog){event.preventDefault();closeChatRename();}
    else if(chatDeleteDialog){event.preventDefault();closeChatDelete();}
    else if(supervisorDialog){event.preventDefault();closeSupervisorDialog();}
    else if(agentMenu){event.preventDefault();agentMenu=null;}
    else if(layout.focus){event.preventDefault();toggleFocus(layout.focus);}
  }
  function newAgent():void {if(selected)dispatch('new-agent',{nodeKey:selected.nodeKey,chatId:chats.some(chat=>chat.id===selectedChat)?selectedChat:null});}
  function agentVerification(agent:BoardAgent) {
    return snapshot.knowledgeRuns?.find(run=>run.nodeKey===agentKey(agent))?.verification||null;
  }
  function association(agent:BoardAgent):string {
    return agent.projectChatId ? chats.find(chat=>chat.id===agent.projectChatId)?.title||'Conversation unavailable' : 'Project supervisor';
  }
  function scrollConversationCards(event:WheelEvent):void {
    const stack=event.currentTarget;
    if(!(stack instanceof HTMLElement))return;
    if(!stack||!event.deltaY||stack.scrollHeight<=stack.clientHeight+1)return;
    const delta=event.deltaMode===1?event.deltaY*16:event.deltaMode===2?event.deltaY*stack.clientHeight:event.deltaY;
    for(const node of event.composedPath()){
      if(node===stack)break;
      if(!(node instanceof HTMLElement))continue;
      const overflow=getComputedStyle(node).overflowY;
      if(!['auto','scroll'].includes(overflow)||node.scrollHeight<=node.clientHeight+1)continue;
      const canContinue=delta<0?node.scrollTop>1:node.scrollTop+node.clientHeight<node.scrollHeight-1;
      if(canContinue)return;
    }
    event.preventDefault();
    stack.scrollTop=Math.max(0,Math.min(stack.scrollTop+delta,stack.scrollHeight-stack.clientHeight));
  }
  function usableInlineSize(element:HTMLElement):number {
    const style=getComputedStyle(element);
    return Math.max(0,element.clientWidth-(parseFloat(style.paddingLeft)||0)-(parseFloat(style.paddingRight)||0));
  }
  function setResponsiveCardWidth(lane:HTMLElement|undefined,list:HTMLElement|undefined,property:string):void {
    if(!lane||!list)return;
    const listWidth=usableInlineSize(list);
    if(listWidth<=0){lane.style.removeProperty(property);return;}
    lane.style.setProperty(property,`${Math.floor(listWidth*2)/2}px`);
  }
  function sizeConversationCards():void {
    setResponsiveCardWidth(chatLane,chatScroll,'--project-chat-card-width');
    setResponsiveCardWidth(agentLane,agentList,'--supervisor-card-width');
  }
  onMount(()=>{
    const cardLayout=(event:Event)=>{const d=(event as CustomEvent).detail;if(!d?.owner)return;layout.minimized=d.minimized?[...new Set([...layout.minimized,d.owner])]:layout.minimized.filter(id=>id!==d.owner);if(d.minimized&&layout.focus===d.owner)layout.focus='';saveLayout();void tick().then(arrangeCards);};
    const cardFocus=(event:Event)=>toggleFocus((event as CustomEvent).detail.owner);
    const closeCard=(event:Event)=>{if(event.target instanceof Element&&event.target.closest('[data-action=close]')){const card=event.target.closest<HTMLElement>('.agent-console');const owner=card?.dataset.projectChatId?`chat:${card.dataset.projectChatId}`:`graph:${card?.dataset.nodeKey}`;if(card?.dataset.nodeKey)layout.openAgents=layout.openAgents.filter(key=>key!==card.dataset.nodeKey);if(layout.focus===owner)layout.focus='';saveLayout();void tick().then(arrangeCards);}};
    eventTarget.addEventListener('click',closeCard,true);
    eventTarget.addEventListener('central-agent:board-card-layout',cardLayout);
    eventTarget.addEventListener('central-agent:board-card-focus',cardFocus);
    const cardsObserver=new MutationObserver(()=>void tick().then(arrangeCards));
    cardsObserver.observe(eventTarget,{childList:true,subtree:true});
    const worktreeReady=(event:Event)=>{worktreeRoot=(event as CustomEvent).detail?.root||'';};
    window.addEventListener('central-agent:board-worktree-ready',worktreeReady);
    const failure=(event:Event)=>{error=(event as CustomEvent).detail?.error||'';};
    const outside=(event:PointerEvent)=>{
      if(!(event.target instanceof Node))return;
      if(chatMenu&&!menuElement?.contains(event.target))chatMenu=null;
      const element=event.target instanceof Element?event.target:null;
      if(agentMenu&&!agentMenuElement?.contains(event.target)&&!element?.closest('[data-supervisor-menu-trigger]'))agentMenu=null;
    };
    const closeFloatingMenu=()=>{agentMenu=null;};
    window.addEventListener('central-agent:project-board-error',failure);
    window.addEventListener('resize',closeFloatingMenu);
    document.addEventListener('pointerdown',outside);
    document.addEventListener('scroll',closeFloatingMenu,true);
    chatWindowLayer?.addEventListener('wheel',scrollConversationCards,{passive:false});
    agentWindowLayer?.addEventListener('wheel',scrollConversationCards,{passive:false});
    const resizeObserver=new ResizeObserver(sizeConversationCards);
    for(const element of [chatLane,chatScroll,agentLane,agentList])if(element)resizeObserver.observe(element);
    sizeConversationCards();
    return()=>{eventTarget.removeEventListener('click',closeCard,true);window.removeEventListener('central-agent:board-worktree-ready',worktreeReady);resizeCleanup?.();cardsObserver.disconnect();eventTarget.removeEventListener('central-agent:board-card-layout',cardLayout);eventTarget.removeEventListener('central-agent:board-card-focus',cardFocus);window.removeEventListener('central-agent:project-board-error',failure);window.removeEventListener('resize',closeFloatingMenu);document.removeEventListener('pointerdown',outside);document.removeEventListener('scroll',closeFloatingMenu,true);chatWindowLayer?.removeEventListener('wheel',scrollConversationCards);agentWindowLayer?.removeEventListener('wheel',scrollConversationCards);resizeObserver.disconnect();};
  });
</script>

<svelte:window onkeydown={boardKeydown}/>
{#snippet chatCard(chat:ProjectChat)}
  <div class="inline-card-slot" use:cardSlot={`chat:${chat.id}`}></div>
{/snippet}
{#if isolate}
  <div class="board-dialog-backdrop" role="presentation"><div class="board-dialog" role="dialog" aria-modal="true" aria-labelledby="worktree-title"><form onsubmit={event=>{event.preventDefault();if(!isolateName.trim()){isolateError='Enter a task name.';return;}dispatch('backend',{backendAction:{type:'create_worktree',root:selected?.path,name:isolateName.trim()}});isolate=false;}}>
    <h2 id="worktree-title">New isolated task</h2><p>Create a branch and a linked working folder from the current HEAD. Your existing uncommitted changes stay in this project. Choose the destination in the next step.</p>
    <label for="worktree-name">Task name</label><input id="worktree-name" bind:value={isolateName} maxlength="60" placeholder="For example, improve search"/>
    <p>The worktree appears as its own project with a new conversation. Git history is shared; file changes remain separate.</p>
    {#if isolateError}<p role="alert">{isolateError}</p>{/if}<div class="dialog-actions"><button type="button" onclick={()=>isolate=false}>Cancel</button><button class="dialog-primary" type="submit">Choose destination</button></div>
  </form></div></div>
{/if}

<section class="board" class:focused={!!layout.focus} class:files-hidden={layout.filesHidden} aria-label="Projects, conversations, supervisors and files">
  {#if error}<div class="board-error" role="status">{error}<button type="button" aria-label="Dismiss message" onclick={()=>error=''}>×</button></div>{/if}
  <div bind:this={boardColumns} class="board-columns" style:--board-project-width={layout.projectsWidth?`${layout.projectsWidth}px`:undefined} style:--board-file-width={layout.filesHidden?'44px':layout.filesWidth?`${layout.filesWidth}px`:undefined}>

    <aside class="project-lane lane" aria-label="Projects" data-board-lane="projects">
      <div data-central-agent-svelte="project-registry"><ProjectRegistry bind:this={registry} {eventTarget} compact selectedNodeKey={selectedKey}/></div>
      {#if detached.length}<button class="saved-agents" type="button" onclick={()=>{selectedKey='saved-agents';selectedAgent=null;selectedChat=null;persist();dispatch('scope-changed');}}>Other saved agents · {detached.length}</button>{/if}
    </aside>
    <button class="lane-resizer board-divider" type="button" aria-label="Resize projects column" onpointerdown={event=>resizeLane(event,'projects')} onkeydown={event=>resizeKey(event,'projects')}></button>
    <div bind:this={conversationLanes} class="conversation-lanes" style:grid-template-columns={`minmax(210px,${layout.focus?(layout.focus.startsWith('chat:')?3:1):layout.chatShare}fr) var(--board-section-spacing) minmax(210px,${layout.focus?(layout.focus.startsWith('graph:')?3:1):1-layout.chatShare}fr)`}>
      <button class="lane-resizer between-chats" type="button" aria-label="Resize conversation columns" onpointerdown={event=>resizeLane(event,'chats')} onkeydown={event=>resizeKey(event,'chats')}></button>
      <section bind:this={chatLane} class="chat-lane lane" class:previewing={openChatIds.length>0} aria-label="Project chats" data-board-lane="chats">
        <header class="lane-heading"><div><h2>Project chats</h2><p>{selected ? `↳ ${selected.name}` : 'Conversations & forks'}</p></div><button type="button" class="icon-button" aria-label="New project chat" disabled={!selected||selected.source==='ssh'} onclick={()=>selected&&dispatch('new-chat',{root:selected.path})}><svg class="plus-icon" viewBox="0 0 16 16" aria-hidden="true"><path d="M8 3v10M3 8h10"/></svg></button></header>
        <label class="search"><svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="8" cy="8" r="5"/><path d="m12 12 5 5"/></svg><input type="search" bind:value={chatQuery} placeholder="Find a conversation" aria-label="Find a project conversation"/></label>
        <div bind:this={chatWindowLayer} id="project-chat-window-layer" class="project-chat-window-layer" aria-label="Open project conversation cards">
        <div bind:this={chatScroll} class="chat-scroll">
          {#if visibleChats.length}<ProjectChatTree chats={visibleChats} activeChatId={selectedChat} searching={!!chatQuery} extra={chatCard} onOpen={chooseChat} onActions={chat=>{chatMenu=chat;selectedChat=chat.id;void tick().then(()=>menuElement?.querySelector('button')?.focus());}}/>
          {:else}<div class="empty-state"><span class="empty-symbol" aria-hidden="true">↳</span><strong>{chatQuery?'No matching conversations':selected?.source==='ssh'?'SSH conversations':'A place for your work'}</strong><p>{selected?.source==='ssh'?'Open a supervisor to work in this remote project.':selected?'Start a conversation. Its forks stay grouped here.':'Choose a project to see its conversations.'}</p></div>{/if}
        </div>
        </div>
        {#if chatMenu}<div class="chat-actions" bind:this={menuElement} role="group" aria-label="Conversation actions"><strong>{chatMenu.title}</strong><button type="button" onclick={()=>{if(chatMenu)chooseChat(chatMenu);}}>Open conversation</button><button type="button" data-chat-action="rename" onclick={renameProjectChat}>Rename</button><button type="button" onclick={()=>{newAgent();chatMenu=null;}}>Add supervisor to this chat</button><button type="button" data-chat-action="worktree" disabled={selected?.source!=='local'} onclick={()=>{isolate=true;isolateName='';isolateError='';chatMenu=null;}}>New task in a worktree</button><button type="button" onclick={()=>{dispatch('pin-chat',{chatId:chatMenu?.id,pinned:!chatMenu?.pinned});chatMenu=null;}}>{chatMenu.pinned?'Unpin conversation':'Pin conversation'}</button><button type="button" data-chat-action="delete" disabled={chatMenu.mutationLocked} onclick={deleteProjectChat}>Delete conversation</button><button type="button" onclick={()=>chatMenu=null}>Close</button></div>{/if}
      </section>
      <section bind:this={agentLane} class="agent-lane lane" aria-label="Supervisor agents" data-board-lane="supervisors">
        <header class="lane-heading"><div><h2>Supervisors</h2><p>Agents in this project</p></div><button type="button" class="icon-button" aria-label="New supervisor" disabled={!selected} onclick={newAgent}><svg class="plus-icon" viewBox="0 0 16 16" aria-hidden="true"><path d="M8 3v10M3 8h10"/></svg></button></header>
        <div bind:this={agentWindowLayer} id="agent-window-layer" class="agent-window-layer" aria-label="Saved agent cards"><div bind:this={agentList} class="agent-list">
          {#each agents as agent (agentKey(agent))}
          {@const verification=agentVerification(agent)}
          <div class="agent-row-shell"><div class="agent-row-head" class:verified={!!verification}>
            <button type="button" class="agent-row" class:selected={selectedAgent===agentKey(agent)} data-agent-key={agentKey(agent)} onclick={()=>openAgent(agent)}>
              <span class="agent-glyph" aria-hidden="true"><svg viewBox="0 0 24 24"><path d="M4 7h10v10H4zM10 3h10v10M7 12h4"/></svg></span>
              <span class="agent-copy"><strong>{agent.name||'Supervisor'}</strong><small>{association(agent)}</small></span>
            </button>
            {#if verification}<span class="agent-verification"><TaskVerificationStatus owner={`supervisor-row:${agentKey(agent)}`} {verification}/></span>{/if}
            <button type="button" class="agent-row-menu" data-supervisor-menu-trigger aria-label={`Actions for ${agent.name||'Supervisor'}`} aria-haspopup="menu" aria-expanded={agentMenu&&agentKey(agentMenu)===agentKey(agent)} onclick={(event)=>toggleAgentMenu(agent,event)}><svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="4" cy="10" r="1"/><circle cx="10" cy="10" r="1"/><circle cx="16" cy="10" r="1"/></svg></button>
          </div>
            {#if selectedAgent===agentKey(agent)&&selected?.source==='local'}
              <BoardConversationMenu owner={`graph:${agentKey(agent)}`} label="Supervisor settings" active={snapshot.expanded!==false}>
                <SettingsPicker id="board-agent-chat" label="Supervised conversation" presentation="workspace" contained value={agent.projectChatId||''} options={[{value:'',label:'Whole project'},...chats.map(chat=>({value:chat.id,label:chat.title}))]} onSelect={value=>dispatch('link-chat',{nodeKey:agentKey(agent),chatId:value||null})}/>
              </BoardConversationMenu>
            {/if}
            <div class="inline-card-slot" use:cardSlot={`graph:${agentKey(agent)}`}></div>
          </div>{/each}
          {#if selectedKey&&!agents.some(agent=>agentKey(agent)===selectedKey)}<div class="inline-card-slot" use:cardSlot={`graph:${selectedKey}`}></div>{/if}
          {#if !agents.length}<div class="empty-state"><span class="empty-symbol" aria-hidden="true">⌘</span><strong>Your supervisor, here</strong><p>{selected?'Add an agent to coordinate and verify work in this project.':'Choose a project to see its agents.'}</p>{#if selected}<button class="primary" type="button" onclick={newAgent}>Add supervisor</button>{/if}</div>{/if}
        </div>
        {#if agentMenu}<div class="agent-actions" bind:this={agentMenuElement} role="menu" aria-label={`Actions for ${agentMenu.name||'Supervisor'}`} style:top={`${agentMenuPosition.top}px`} style:left={`${agentMenuPosition.left}px`}><strong>{agentMenu.name||'Supervisor'}</strong><button type="button" role="menuitem" data-supervisor-action="rename" onclick={renameSupervisor}>Rename</button><button type="button" role="menuitem" data-supervisor-action="delete" onclick={deleteSupervisor}>Delete</button></div>{/if}
        </div>
      </section>
    </div>
    <button class="lane-resizer board-divider" type="button" aria-label="Resize files column" onpointerdown={event=>resizeLane(event,'files')} onkeydown={event=>resizeKey(event,'files')}></button>
    <section class="file-lane lane" aria-label="Project files" data-board-lane="files">
      <header class="lane-heading"><div hidden={layout.filesHidden}><h2>Project files</h2></div><button type="button" class="icon-button" aria-label={layout.filesHidden?'Show project files':'Hide project files'} title={layout.filesHidden?'Show project files':'Hide project files'} onclick={()=>{layout.filesHidden=!layout.filesHidden;saveLayout();}}>{layout.filesHidden?'‹':'›'}</button></header>
      <div class="files-content" hidden={layout.filesHidden}>
        <ProjectFileActivity {work} remote={selected?.source==='ssh'} owner={workOwner} onSelect={owner=>workOwner=owner} onOpen={openWorkFile}/>
        <BoardFiles project={selected} {entries} {work} onOpen={openWorkFile}/>
      </div>
    </section>
  </div>
  <div bind:this={parking} class="card-parking" hidden></div>
</section>

{#if chatRenameDialog}
  <div class="board-dialog-backdrop" role="presentation">
    <div class="board-dialog" role="dialog" aria-modal="true" aria-labelledby="rename-project-chat-title">
      <form onsubmit={(event)=>{event.preventDefault();submitChatTitle();}}>
        <h2 id="rename-project-chat-title">Rename project chat</h2>
        <p>Choose the title shown in this project, its conversation card and linked Supervisor selectors.</p>
        <label for="project-chat-title">Title</label>
        <input bind:this={chatTitleInput} id="project-chat-title" bind:value={chatTitle} maxlength="120" autocomplete="off" aria-describedby={chatTitleError?'project-chat-title-error':undefined}/>
        {#if chatTitleError}<p id="project-chat-title-error" class="dialog-error" role="alert">{chatTitleError}</p>{/if}
        <div class="dialog-actions"><button type="button" onclick={closeChatRename}>Cancel</button><button class="dialog-primary" type="submit">Save title</button></div>
      </form>
    </div>
  </div>
{/if}

{#if chatDeleteDialog}
  <div class="board-dialog-backdrop" role="presentation">
    <div class="board-dialog" role="alertdialog" aria-modal="true" aria-labelledby="delete-project-chat-title" aria-describedby="delete-project-chat-copy">
      <h2 id="delete-project-chat-title">Delete conversation?</h2>
      <p id="delete-project-chat-copy"><strong>{chatDeleteDialog.title||'New chat'}</strong> will be permanently removed from this project's local history. Project files and checkpoints remain unchanged.</p>
      <div class="dialog-actions"><button type="button" onclick={closeChatDelete}>Cancel</button><button class="dialog-primary" type="button" data-chat-confirm-delete onclick={confirmDeleteProjectChat}>Delete conversation</button></div>
    </div>
  </div>
{/if}

{#if supervisorDialog}
  <div class="board-dialog-backdrop" role="presentation">
    {#if supervisorDialog.kind==='rename'}
      <div class="board-dialog" role="dialog" aria-modal="true" aria-labelledby="rename-supervisor-title">
        <form onsubmit={(event)=>{event.preventDefault();submitSupervisorName();}}>
          <h2 id="rename-supervisor-title">Rename supervisor</h2>
          <p>Choose the name shown on this saved supervisor card.</p>
          <label for="supervisor-name">Name</label>
          <input bind:this={supervisorNameInput} id="supervisor-name" bind:value={supervisorName} maxlength="80" autocomplete="off" aria-describedby={supervisorNameError?'supervisor-name-error':undefined}/>
          {#if supervisorNameError}<p id="supervisor-name-error" class="dialog-error" role="alert">{supervisorNameError}</p>{/if}
          <div class="dialog-actions"><button type="button" onclick={closeSupervisorDialog}>Cancel</button><button class="dialog-primary" type="submit">Save name</button></div>
        </form>
      </div>
    {:else}
      <div class="board-dialog" role="alertdialog" aria-modal="true" aria-labelledby="delete-supervisor-title" aria-describedby="delete-supervisor-copy">
        <h2 id="delete-supervisor-title">Delete supervisor?</h2>
        <p id="delete-supervisor-copy"><strong>{supervisorDialog.name}</strong>, its saved card and its local association will be removed. Linked project conversations and project files remain unchanged.</p>
        <div class="dialog-actions"><button type="button" onclick={closeSupervisorDialog}>Cancel</button><button class="dialog-primary" type="button" data-supervisor-confirm-delete onclick={confirmDeleteSupervisor}>Delete supervisor</button></div>
      </div>
    {/if}
  </div>
{/if}

<style>
  .file-lane>.lane-heading{align-items:flex-start}
  .files-content{display:flex;flex:1;min-height:0;flex-direction:column}.files-content[hidden]{display:none}.card-parking{display:none}
  .inline-card-slot{min-width:0}.inline-card-slot:empty{display:none}.agent-row-head{position:relative}
  .lane-resizer{position:relative;z-index:6;min-width:0;padding:0;border-radius:var(--ca-radius-none);cursor:col-resize;background:transparent;touch-action:none}
  .lane-resizer::before{position:absolute;inset-block-start:50%;inset-inline-start:50%;width:var(--ca-space-1);height:var(--ca-space-12);border-radius:var(--ca-radius-pill);background:var(--ca-border-strong);content:"";pointer-events:none;transform:translate(-50%,-50%);transition:background-color var(--ca-duration-fast) var(--ca-ease-standard)}
  .lane-resizer:hover,.lane-resizer:focus-visible{background:transparent;box-shadow:none}.lane-resizer:hover::before{background:var(--ca-muted)}.lane-resizer:focus-visible::before{background:var(--ca-muted);box-shadow:var(--ca-focus-ring)}.lane-resizer:active::before{background:var(--ca-icon)}
  .board-divider{width:100%;height:100%}.conversation-lanes .between-chats{grid-column:2;grid-row:1;width:100%;height:100%}
  .inline-card-slot :global(.agent-console-title){display:none}.board.focused .inline-card-slot :global(.agent-console[data-card-focused="true"]){aspect-ratio:auto;height:var(--ca-board-focus-height);min-height:400px}.focused .inline-card-slot :global(.agent-console[data-card-focused="true"] .agent-console-title){display:block}
  .board{--board-section-spacing:var(--ca-space-4);--board-top-inset:10px;--board-edge-content-inset:var(--ca-space-4);--conversation-card-gap:var(--ca-space-4);--project-chat-surface:var(--ca-surface-2);display:flex;height:100%;min-height:0;flex-direction:column;background:var(--ca-app-background);color:var(--ca-text);font:var(--ca-type-body)/var(--ca-leading-body) var(--ca-font-body)}
  h2,p{margin:0}
  button,input{font:inherit}button{border:0;border-radius:var(--ca-control-radius);background:var(--ca-surface-2);color:var(--ca-text);padding:var(--ca-space-2) var(--ca-space-3);cursor:pointer}button:hover{background:var(--ca-surface-3)}button:disabled{opacity:.45;cursor:default}button:focus-visible,input:focus-visible{outline:none;box-shadow:var(--ca-focus-ring)}
  svg{width:var(--ca-space-5);height:var(--ca-space-5);fill:none;stroke:currentColor;stroke-width:1.6;stroke-linecap:round;stroke-linejoin:round;flex-shrink:0}
  .board-columns{display:grid;box-sizing:border-box;inline-size:100%;grid-template-columns:var(--board-project-width,minmax(210px,.85fr)) var(--board-section-spacing) minmax(560px,2.25fr) var(--board-section-spacing) var(--board-file-width,minmax(270px,1.3fr));gap:0;flex:1;min-height:0;padding:var(--board-top-inset) var(--board-section-spacing) var(--board-section-spacing);overflow:auto;scrollbar-gutter:stable both-edges}
  .lane{min-width:0;min-height:0;display:flex;flex-direction:column;position:relative;border-radius:var(--ca-radius-large)}
  .project-lane{--project-registry-padding:0 var(--board-edge-content-inset) var(--board-edge-content-inset);background:var(--ca-surface-2)}.project-lane>div{flex:1;min-height:0}.file-lane{box-sizing:border-box;padding-inline-end:var(--board-edge-content-inset)}.saved-agents{margin:var(--ca-space-3);text-align:left;font-size:var(--ca-type-caption)}
  .conversation-lanes{position:relative;display:grid;grid-column:3;grid-template-columns:minmax(210px,1fr) var(--board-section-spacing) minmax(210px,1fr);gap:0;min-width:0;min-height:0}.project-lane{grid-column:1}.board-divider[aria-label="Resize projects column"]{grid-column:2}.board-divider[aria-label="Resize files column"]{grid-column:4}.file-lane{grid-column:5}.chat-lane{grid-column:1;grid-row:1}.agent-lane{grid-column:3;grid-row:1}
  .lane-heading{display:flex;align-items:center;justify-content:space-between;gap:var(--ca-space-2);padding:0 var(--ca-space-2) var(--ca-space-4)}
  h2{font-size:var(--ca-type-title);font-weight:600;letter-spacing:-.015em}.lane-heading p{color:var(--ca-muted);font-size:var(--ca-type-caption);margin-top:var(--ca-space-1)}
  .icon-button{display:grid;place-items:center;flex-shrink:0;width:var(--ca-control-compact);height:var(--ca-control-compact);padding:0;font-size:var(--ca-type-title);line-height:1}.icon-button .plus-icon{width:var(--ca-space-3);height:var(--ca-space-3);stroke-width:2}
  .search{display:flex;align-items:center;gap:var(--ca-space-2);min-height:var(--ca-control-default);border-radius:var(--ca-radius-medium);background:var(--ca-surface-2);padding:0 var(--ca-space-3);color:var(--ca-muted);margin-bottom:var(--ca-space-3)}
  .search input{width:100%;min-width:0;border:0;outline:none;background:transparent;color:var(--ca-text);font-size:var(--ca-type-label)}
  .chat-scroll{flex:1;overflow:auto;min-height:0;scrollbar-width:thin;scrollbar-color:var(--ca-scrollbar-thumb) var(--ca-scrollbar-track)}
  .chat-scroll,.agent-list,.project-chat-window-layer,.agent-window-layer{box-sizing:border-box;width:100%;scrollbar-gutter:stable}
  .chat-lane>.lane-heading,.chat-lane>.search,.chat-scroll :global(.chat-tree){box-sizing:border-box;width:var(--project-chat-card-width,100%);max-width:100%}.chat-scroll :global(.chat-tree){--chat-tree-gap:var(--conversation-card-gap)}
  .agent-lane>.lane-heading,.agent-list :global(.agent-row-shell){box-sizing:border-box;width:var(--supervisor-card-width,100%);max-width:100%}
  .chat-scroll,.chat-lane.previewing .chat-scroll{flex:none;min-height:0;max-height:none;overflow:visible;scrollbar-gutter:auto}
  .chat-scroll :global(.workspace-chat-row){padding:var(--ca-space-2);border-radius:var(--ca-radius-medium);margin-bottom:0;background:var(--ca-surface-2)}
  .chat-scroll :global(.workspace-chat-row.active){background:var(--ca-accent-soft)}
  .chat-scroll :global(.chat-entry:has(.agent-console)){overflow:hidden;border-radius:var(--ca-radius-large);background:var(--project-chat-surface);box-shadow:none}
  .chat-scroll :global(.chat-entry:has(.agent-console) .workspace-chat-row),.chat-scroll :global(.chat-entry:has(.agent-console) .workspace-chat-row.active){border-radius:var(--ca-radius-none);background:transparent}
  .chat-scroll :global(.workspace-chat-title){font-size:var(--ca-type-body)}
  .chat-scroll :global(.workspace-row-menu){border:0;border-radius:var(--ca-control-radius);width:var(--ca-control-compact);height:var(--ca-control-compact);color:var(--ca-muted);background:transparent;cursor:pointer}
  .agent-window-layer :global(.agent-console-header){cursor:default;touch-action:auto}
  .chat-scroll :global(.chat-relation),.chat-scroll :global(.workspace-chat-age){font-size:var(--ca-type-caption)}
  .empty-state{display:grid;justify-items:start;align-content:center;gap:var(--ca-space-2);padding:var(--ca-space-6) var(--ca-space-3);color:var(--ca-muted)}.empty-state strong{font-weight:550;color:var(--ca-text)}.empty-state p{font-size:var(--ca-type-label);max-width:32ch}.empty-symbol{font-size:var(--ca-type-display);color:var(--ca-faint)}
  .primary{background:var(--ca-accent);color:var(--ca-on-accent)}.primary:hover{background:var(--ca-accent-hover)}
  .project-chat-window-layer{--conversation-scrollbar-track:var(--project-chat-surface);display:flex;flex:1;min-height:0;flex-direction:column;align-items:stretch;gap:var(--conversation-card-gap);overflow-y:auto;overflow-x:hidden;margin-top:var(--ca-space-2);padding:0 0 var(--ca-space-2);scrollbar-width:thin;scrollbar-color:var(--ca-input) var(--conversation-scrollbar-track);overscroll-behavior:contain;isolation:isolate}
  .project-chat-window-layer:empty{display:none}
  .project-chat-window-layer :global(.chat-entry .agent-console[data-central-agent-svelte="knowledge-agent-popup"]){--agent-card-background:var(--project-chat-surface);position:relative;left:auto;top:auto;box-sizing:border-box;width:100%;max-width:100%;height:auto;min-height:0;max-height:none;aspect-ratio:var(--agent-card-aspect-ratio,9/16);flex:0 0 auto;border:0;border-radius:var(--ca-radius-none);box-shadow:none}
  .project-chat-window-layer :global(.agent-console-header){cursor:default;touch-action:auto}
  .agent-list{display:grid;gap:var(--conversation-card-gap);flex-shrink:0;max-height:none;overflow:visible;scrollbar-gutter:auto}.agent-list:has(.empty-state){max-height:none}
  .agent-row-shell{position:relative}.agent-row{display:flex;width:100%;align-items:center;text-align:left;gap:var(--ca-space-2);min-height:var(--ca-control-prominent);padding:var(--ca-space-3);padding-inline-end:calc(var(--ca-control-compact) + var(--ca-space-3))}.agent-row.selected{background:var(--ca-accent-soft)}
  .agent-row-head.verified .agent-row{padding-inline-end:calc(var(--ca-control-compact) * 4)}.agent-verification{position:absolute;z-index:3;inset-block-start:50%;inset-inline-end:calc(var(--ca-control-compact) + var(--ca-space-3));transform:translateY(-50%)}
  .agent-row-menu{position:absolute;z-index:2;inset-block-start:50%;inset-inline-end:var(--ca-space-2);display:grid;width:var(--ca-control-compact);height:var(--ca-control-compact);place-items:center;padding:0;transform:translateY(-50%);background:transparent;color:var(--ca-muted)}.agent-row-menu:hover,.agent-row-menu[aria-expanded="true"]{background:var(--ca-surface-3);color:var(--ca-text)}.agent-row-menu svg{width:var(--ca-space-4);height:var(--ca-space-4);fill:currentColor;stroke:none}
  .agent-copy{display:grid;min-width:0;flex:1;gap:var(--ca-space-1)}.agent-copy strong{font-weight:550;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}.agent-copy small{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;color:var(--ca-muted);font-size:var(--ca-type-caption)}.agent-glyph{display:grid;place-items:center;color:var(--ca-icon)}
  .agent-window-layer{--conversation-scrollbar-track:var(--ca-app-background);display:flex;flex-direction:column;align-items:stretch;gap:var(--conversation-card-gap);overflow-y:auto;overflow-x:hidden;min-height:0;flex:1;padding:var(--ca-space-3) 0;scrollbar-width:thin;scrollbar-color:var(--ca-input) var(--conversation-scrollbar-track);overscroll-behavior:contain;isolation:isolate}
  .project-chat-window-layer::-webkit-scrollbar,.agent-window-layer::-webkit-scrollbar{background:var(--conversation-scrollbar-track)}
  .project-chat-window-layer::-webkit-scrollbar-track,.project-chat-window-layer::-webkit-scrollbar-corner,.agent-window-layer::-webkit-scrollbar-track,.agent-window-layer::-webkit-scrollbar-corner{background:var(--conversation-scrollbar-track)}
  .project-chat-window-layer::-webkit-scrollbar-thumb,.project-chat-window-layer::-webkit-scrollbar-thumb:hover,.agent-window-layer::-webkit-scrollbar-thumb,.agent-window-layer::-webkit-scrollbar-thumb:hover{border-color:var(--conversation-scrollbar-track);background:var(--ca-input)}
  .agent-window-layer:empty{display:none}.agent-window-layer :global(.agent-console[data-central-agent-svelte="knowledge-agent-popup"]){position:relative;left:auto;top:auto;box-sizing:border-box;width:var(--supervisor-card-width,100%);max-width:100%;height:auto;min-height:0;max-height:none;aspect-ratio:var(--agent-card-aspect-ratio,9/16);flex:0 0 auto;border:3px solid var(--ca-accent-soft);border-radius:var(--ca-radius-large);box-shadow:none}
  .chat-actions{position:absolute;inset:auto var(--ca-space-2) var(--ca-space-2);z-index:8;display:grid;gap:var(--ca-space-1);padding:var(--ca-space-3);border-radius:var(--ca-radius-large);box-shadow:var(--ca-shadow-float);background:var(--ca-surface)}.chat-actions strong{padding:var(--ca-space-2);font-weight:550;overflow-wrap:anywhere}.chat-actions button{text-align:left}
  .agent-actions{position:fixed;z-index:90;display:grid;width:196px;gap:var(--ca-space-1);padding:var(--ca-space-2);border-radius:var(--ca-radius-large);background:var(--ca-surface);box-shadow:var(--ca-shadow-float)}.agent-actions strong{padding:var(--ca-space-2);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;font-weight:550}.agent-actions button{text-align:left;background:transparent}.agent-actions button:hover,.agent-actions button:focus-visible{background:var(--ca-surface-3)}
  .board-dialog-backdrop{position:fixed;z-index:100;inset:0;display:grid;place-items:center;padding:var(--ca-space-5);background:color-mix(in srgb,var(--ca-app-background) 72%,transparent);backdrop-filter:blur(6px)}.board-dialog{display:grid;width:min(420px,calc(100vw - 40px));gap:var(--ca-space-3);padding:var(--ca-space-5);border:0;border-radius:var(--ca-radius-large);background:var(--ca-surface);box-shadow:var(--ca-shadow-float);animation:board-dialog-in var(--ca-duration-deliberate) var(--ca-ease-emphasized)}.board-dialog form{display:grid;gap:var(--ca-space-3)}.board-dialog h2{font:680 var(--ca-type-settings-sidebar-title-compact)/1.2 var(--ca-font-display);letter-spacing:-.015em}.board-dialog p{color:var(--ca-muted);font-size:var(--ca-type-body);line-height:var(--ca-leading-body)}.board-dialog p strong{color:var(--ca-text)}.board-dialog label{font-weight:640}.board-dialog input{width:100%;box-sizing:border-box;min-height:42px;padding:0 var(--ca-space-3);border:0;border-radius:var(--ca-radius-medium);outline:0;background:var(--ca-surface-2);color:var(--ca-text)}.board-dialog input:focus{box-shadow:var(--ca-focus-ring)}.board-dialog .dialog-error{color:var(--ca-text);box-shadow:inset 3px 0 var(--ca-danger);padding-inline-start:var(--ca-space-3);font-size:var(--ca-type-caption)}.dialog-actions{display:flex;justify-content:flex-end;gap:var(--ca-space-2);margin-top:var(--ca-space-2)}.dialog-actions button{min-height:38px;padding:0 var(--ca-space-4);font-weight:640}.dialog-actions .dialog-primary{background:var(--ca-text);color:var(--ca-app-background)}
  @keyframes board-dialog-in{from{opacity:0;transform:translateY(8px) scale(.98)}to{opacity:1;transform:none}}
  .board-error{display:flex;align-items:center;justify-content:space-between;gap:var(--ca-space-3);margin:0 var(--ca-space-6) var(--ca-space-2);padding:var(--ca-space-2) var(--ca-space-3);background:var(--ca-surface-3);border-radius:var(--ca-radius-medium);font-size:var(--ca-type-label)}
  @media(max-width:1100px){.board{--board-section-spacing:var(--ca-space-3)}.board-columns{grid-template-columns:var(--board-project-width,210px) var(--board-section-spacing) minmax(540px,1fr) var(--board-section-spacing) var(--board-file-width,270px)}.board-columns>.lane,.conversation-lanes{min-height:480px}}
  @media(prefers-reduced-motion:reduce){.board{scroll-behavior:auto}.board-dialog{animation:none}}
</style>
