// Actual embedded controls; all native messages are handled by the startup sink.
try {
  const assert=(condition,message)=>{if(!condition)throw new Error(message);};
  const frames=()=>new Promise(resolve=>setTimeout(resolve,50));
  const launcher=document.getElementById('tetra-config-button');
  const host=document.getElementById('agent-configuration-host');
  const menu=document.getElementById('tetra-config-menu');
  const explorer=document.getElementById('workspace-explorer');
  const conversation=document.querySelector('.conversation-section');
  const home=document.getElementById('home-view');
  const pieces=[...launcher.querySelectorAll('[data-supervisor-logo] path')];
  const reduced=()=>window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  const settle=async()=>{
    const animations=[menu,...pieces].flatMap(element=>element.getAnimations());
    // Advance native animations deterministically even when the test window is
    // backgrounded; their real keyframes and opening/closing phases stay intact.
    animations.forEach(animation=>animation.finish());
    await Promise.all(animations.map(animation=>animation.finished.catch(()=>{})));
    await frames();
  };
  const geometry=()=>{
    const bounds=host.getBoundingClientRect(),area=conversation.getBoundingClientRect();
    assert(bounds.width>0 && bounds.width<=parseFloat(getComputedStyle(menu).getPropertyValue('--ca-profile-popup-width'))+1,'Profile popup is no longer compact');
    assert(bounds.left>=area.left && bounds.right<=Math.min(area.right,innerWidth)+1 && bounds.top>=0 && bounds.bottom<=innerHeight+1,'Profile popup leaves the available chat space');
    assert(getComputedStyle(explorer).visibility==='visible' && !explorer.inert,'Profile popup hides or disables Explorer');
    assert(menu.scrollWidth<=menu.clientWidth+1,'Profile content overflows horizontally');
    if(host.dataset.placement==='gutter') {
      const chat=conversation.querySelector('.chat-messages');
      assert(bounds.right<=chat.getBoundingClientRect().left+chat.clientLeft+parseFloat(getComputedStyle(chat).paddingLeft)+1,'Popup covers transcript content despite an available gutter');
    }
  };
  const previousTheme=document.documentElement.dataset.theme, previousWidth=home.style.width;
  const previousMedia=window.matchMedia;
  try {
    const settings=document.getElementById('agent-settings-host');
    assert(host.hidden && document.querySelectorAll('#context-select').length===1 && settings.contains(document.getElementById('context-select')) && settings.contains(document.getElementById('personality-select')),'Advanced profile preferences left their unique Settings controls');
    for(const theme of ['central','central_dark']) {
      document.documentElement.dataset.theme=theme;
      const composer=document.getElementById('composer');
      const workSlot=composer.querySelector('.composer-work-slot');
      const contextMonitor=document.getElementById('context-window-monitor');
      for(const width of [560,720,1920,2560,3840]) {
        home.style.width=`${width}px`;await frames();
        assert(composer.getBoundingClientRect().height>0 && document.getElementById('chat-input').getClientRects().length>0,'Prompt disappeared with its retired metadata');
        const composerBounds=composer.getBoundingClientRect(),workBounds=workSlot.getBoundingClientRect(),rightGap=composerBounds.right-workBounds.right,bottomGap=composerBounds.bottom-workBounds.bottom;
        assert(workBounds.width>=28 && workBounds.width<=36 && rightGap>=0 && bottomGap>=0 && rightGap<=12 && bottomGap<=12 && Math.abs(rightGap-bottomGap)<=2,'Main work action slot does not keep equal right and bottom composer insets');
        assert(!composer.querySelector('.chat-identity'),'Repeated chat title/Chat banner is still mounted beside the prompt');
        assert(contextMonitor.hidden && contextMonitor.inert && contextMonitor.getAttribute('aria-hidden')==='true' && !contextMonitor.getClientRects().length && [...contextMonitor.querySelectorAll('*')].every(element=>!element.getClientRects().length),'Hidden context report still occupies space or exposes controls');
        assert(!/Context · native report|Waiting for usage|Waiting for the first native token-usage event\./.test(composer.innerText),'Usage placeholders remain visible beside the prompt');
      }
      home.style.width=previousWidth;await frames();
      const explorerBefore=explorer.getBoundingClientRect(),chatBefore=conversation.getBoundingClientRect();
      launcher.disabled=false;launcher.click();
      assert(!menu.hidden && launcher.getAttribute('aria-expanded')==='true','Logo does not open the profile popup');
      if(!reduced())assert(menu.dataset.phase==='opening' && menu.getAnimations().length===1,'Opening popup animation is missing');
      await settle();geometry();
      assert(explorer.getBoundingClientRect().width===explorerBefore.width && conversation.getBoundingClientRect().left===chatBefore.left,'Opening the popup resized Explorer or moved the chat');
      assert(pieces.every(path=>reduced()?getComputedStyle(path).transform==='none':getComputedStyle(path).transform!=='none'),'The mark does not decompose, or ignores reduced motion');
      assert([...menu.querySelectorAll('[data-picker]')].map(field=>field.dataset.picker).join(',')==='provider,model,effort,speed','Popup lost its four profile selectors');
      for(const kind of ['provider','model','effort','speed']) {
        const button=document.getElementById(`${kind}-picker-button`);button.disabled=false;button.click();await frames();
        const options=document.getElementById(`${kind}-picker-menu`);
        assert(!options.hidden && getComputedStyle(options).position==='static','A profile selector does not expand inside the popup');
        geometry();
      }
      home.style.width='720px';await frames();geometry();
      home.style.width=previousWidth;await frames();geometry();
      menu.querySelector('[aria-label="Close agent configuration"]').click();
      assert(launcher.getAttribute('aria-expanded')==='false' && menu.inert,'Closing popup remains active for keyboard input');
      if(!reduced())assert(!menu.hidden && menu.dataset.phase==='closing','Popup is hidden before its closing animation');
      await settle();
      assert(menu.hidden && host.hidden && pieces.every(path=>getComputedStyle(path).transform==='none' && !path.getAnimations().length),'Closing did not recompose and release the original mark');
      assert(document.activeElement===launcher,'Close did not return keyboard focus to the logo');
      launcher.click();launcher.click();launcher.click();await settle();
      assert(!menu.hidden && launcher.getAttribute('aria-expanded')==='true','A stale animation completion closed the reopened popup');
      document.body.click();await settle();assert(menu.hidden,'Outside click did not close the popup');
      launcher.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowDown',bubbles:true}));await settle();
      assert(menu.contains(document.activeElement),'ArrowDown did not focus the popup');
      menu.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}));await settle();
      assert(menu.hidden && document.activeElement===launcher,'Escape did not close and restore focus');
    }
    window.matchMedia=query=>query==='(prefers-reduced-motion: reduce)'?{matches:true}:previousMedia.call(window,query);
    launcher.click();
    assert(menu.dataset.phase==='open' && !menu.getAnimations().length && pieces.every(path=>getComputedStyle(path).transform==='none'),'Reduced motion still decomposes or animates the mark');
    launcher.click();assert(menu.hidden,'Reduced-motion close is not immediate');
    window.matchMedia=previousMedia;
    window.centralAgentOpenSettings();window.CentralAgentSvelte.selectSettingsSection('settings-agent');await frames();
    for(const kind of ['personality','context']) {
      const button=document.getElementById(`${kind}-picker-button`);button.disabled=false;button.click();await frames();
      const options=document.getElementById(`${kind}-picker-menu`);
      assert(host.hidden && !options.hidden && getComputedStyle(options).position==='absolute','Moved preferences no longer open as an anchored Settings selector');
    }
    window.centralAgentCommitSettingsClose();
  } finally {
    window.matchMedia=previousMedia;home.style.width=previousWidth;
    if(previousTheme)document.documentElement.dataset.theme=previousTheme;else delete document.documentElement.dataset.theme;
    window.CentralAgentSvelte.setAgentConfigurationOpen(false,true);
  }
} catch(error) {startupErrors.push(`Profile popup: ${error?.stack || String(error)}`);}
