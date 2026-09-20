// Real embedded Svelte/DOM, synthetic navigation only. No account or native mutation.
try {
  const flush=async()=>{await Promise.resolve();await Promise.resolve();};
  const assert=(ok,message)=>{if(!ok)throw new Error(message);};
  const source={id:'tree-source',title:'Same title',projectRoot:'fixture',lineage:{kind:'source',childCount:1}};
  const child={id:'tree-child',title:'Same title',projectRoot:'fixture',lineage:{kind:'fork',parentChatId:source.id,parentTitle:source.title,parentAvailable:true,childCount:0}};
  let opened='',menus=0;
  const options={chats:[child,source],activeChatId:child.id,onOpen:chat=>{opened=chat.id;},onActions:()=>{menus++;}};
  const previous=document.documentElement.dataset.theme;
  for(const theme of ['central','central_dark']) {
    document.documentElement.dataset.theme=theme;
    const tree=window.CentralAgentSvelte.renderProjectChatTree('startup-tree',options);
    tree.style.width='240px';document.body.append(tree);await flush();
    const rows=[...tree.querySelectorAll('.workspace-chat-row')];
    assert(rows.map(row=>row.dataset.chatId).join(',')==='tree-source,tree-child','Fork tree order lost source identity');
    assert(rows[1].dataset.chatDepth==='1' && rows[1].textContent.includes('Fork'),'Fork relationship is missing');
    const button=rows[1].querySelector('.workspace-chat-open');button.click();await flush();
    assert(opened===child.id,'Opening fork targeted another local chat');
    const menu=rows[1].querySelector('.workspace-row-menu');
    const menuIcon=menu.querySelector('.workspace-menu-icon');
    assert(menuIcon?.querySelectorAll('circle').length===3,'Chat actions did not use the compact three-dot icon');
    const menuBox=menu.getBoundingClientRect(),iconBox=menuIcon.getBoundingClientRect(),rowBox=rows[1].getBoundingClientRect();
    assert(iconBox.width<menuBox.width && iconBox.height<menuBox.height,'Three-dot icon is not smaller than its click target');
    assert(Math.abs((menuBox.top+menuBox.height/2)-(rowBox.top+rowBox.height/2))<1,'Chat actions are not vertically centered with the row');
    const titleStyle=getComputedStyle(rows[1].querySelector('.workspace-chat-title'));
    const buttonStyle=getComputedStyle(button),rootStyle=getComputedStyle(document.documentElement);
    assert(titleStyle.fontFamily.includes('SF Pro Text') && titleStyle.fontFamily.includes('Inter') && parseFloat(titleStyle.fontSize)===13,`Chat title typography did not match the file-name scale and platform face: title=${titleStyle.fontFamily}/${titleStyle.fontSize}, button=${buttonStyle.font}/${rootStyle.getPropertyValue('--ca-type-navigation')}`);
    menu.click();await flush();assert(menus>0,'Lifecycle menu is missing');
    tree.querySelector('.branch-toggle').click();await flush();
    assert(tree.querySelectorAll('.workspace-chat-row').length===1,'Collapsing source lost or retained the wrong rows');
    window.CentralAgentSvelte.renderProjectChatTree('startup-tree',{...options,chats:[{...child,updatedAtMs:5},source]});await flush();
    assert(tree.querySelectorAll('.workspace-chat-row').length===1,'Background refresh reopened a collapsed branch');
    tree.querySelector('.branch-toggle').click();await flush();
    assert(tree.querySelectorAll('.workspace-chat-row').length===2,'Expanding source failed');
    for(const width of [170,240]) {
      tree.style.width=`${width}px`;await flush();
      assert(![...tree.querySelectorAll('.workspace-chat-row')].some(row=>row.scrollWidth>row.clientWidth+1),'Chat tree overflows a narrow explorer');
    }
    tree.remove();window.CentralAgentSvelte.pruneProjectChatTrees();
  }
  document.documentElement.dataset.theme=previous;
} catch(error) { startupErrors.push(`Chat tree: ${String(error)}`); }
