// Production Svelte in a hidden WebView. All account data is synthetic and the
// startup sink records intent without contacting a provider.
try {
  const assert=(condition,message)=>{if(!condition)throw new Error(message);};
  const flush=async()=>{for(let i=0;i<6;i++)await Promise.resolve();};
  const api=window.CentralAgentSvelte,view=document.getElementById('settings-view');
  const content=document.getElementById('settings-content'),search=document.getElementById('settings-search');
  const source=document.getElementById('settings-sections-source');
  const pages=[...document.querySelectorAll('[data-settings-section]')];
  const nav=[...document.querySelectorAll('[data-settings-target]')];
  const navGroups=[...document.querySelectorAll('[data-settings-nav-group]')];
  const shell=view.querySelector('.settings-shell'),topbar=view.querySelector('.settings-topbar');
  const primaryTabs=view.querySelector('.settings-primary-tabs');
  const oldWidth=view.style.width,oldTheme=document.documentElement.dataset.theme;
  const oldContentHeight=content.style.height,oldContentMaxHeight=content.style.maxHeight;
  const field=document.getElementById('ssh-profile-name'),fieldValue=field.value;
  const originalControls=[...document.querySelectorAll('input[id],select[id]')];
  const details=[...document.querySelectorAll('details')].map(element=>[element,element.open]);
  const actions=[];
  const accountHost=view.querySelector('[data-central-agent-svelte="app-server-account"]');
  const sessionAccessModal=document.getElementById('full-access-modal');
  const sessionAccessWasHidden=sessionAccessModal.hidden;
  const capture=event=>{if(event.detail?.action)actions.push(event.detail.action);};
  const account={connected:false,connecting:false,refreshing:false,authBusy:false,version:null,account:null,login:null,models:[],requirementsLoaded:false,
    permissionProfiles:{loading:false,loaded:false,current:false,requiresNamedProfile:false,entries:[]},
    rateLimits:{refreshing:false,loaded:false,current:false,buckets:[],availableResetCredits:null},
    providerCapabilities:{loaded:false,current:false,namespaceTools:false,imageGeneration:false,webSearch:false},
    accountUsage:{refreshing:false,loaded:false,current:false,summary:null,dailyUsageBuckets:null},
    workspaceMessages:{refreshing:false,loaded:false,current:false,featureEnabled:false,messages:[]},
    runtimeNotices:[],errors:{},chatReload:{loading:false,checked:false,total:0,loaded:0,unavailable:0,skipped:0}};
  const setQuery=async value=>{search.value=value;search.dispatchEvent(new Event('input',{bubbles:true}));await flush();};
  const select=async id=>{assert(api.selectSettingsSection(id),`Cannot select ${id}`);await flush();};
  try {
    accountHost.addEventListener('central-agent:app-server-account',capture);
    window.centralAgentOpenSettings();document.body.classList.remove('settings-preparing');await flush();

    sessionAccessModal.hidden=false;await flush();
    const sessionAccessCard=sessionAccessModal.querySelector('.modal-card'),sessionAccessBox=sessionAccessCard.getBoundingClientRect();
    assert(sessionAccessBox.width<=560.5 && sessionAccessBox.width>=480,'Session access confirmation is not a compact dialog');
    assert(sessionAccessBox.left>=17 && innerWidth-sessionAccessBox.right>=17,'Session access confirmation is not centered inside the viewport');
    assert(sessionAccessBox.height<=innerHeight-35,'Session access confirmation exceeds the available viewport height');
    assert(parseFloat(getComputedStyle(sessionAccessCard).borderTopWidth)===0 && [...sessionAccessModal.querySelectorAll('.modal-icon,.action')].every(element=>parseFloat(getComputedStyle(element).borderTopWidth)===0),'A Settings confirmation still has a resting border');
    sessionAccessModal.hidden=true;

    assert(nav.length===7,'Settings must expose exactly seven categories');
    assert(topbar?.querySelector('h1')?.textContent.trim()==='Settings' && topbar.contains(search),'Settings title or search is missing from the top bar');
    const back=topbar?.querySelector('#settings-back');
    assert(back?.getAttribute('aria-label')==='Back to workspace' && back.querySelector('svg[aria-hidden="true"]') && !back.querySelector('.settings-back-icon') && !back.textContent.trim(),'The workspace return action is not a bare accessible arrow');
    assert(navGroups.map(group=>group.textContent.trim()).join('|')==='Personal|AI|Workspace|Connections','Settings navigation groups are missing or out of order');
    assert(primaryTabs && view.querySelectorAll('.settings-primary-tab').length===4,'The four primary settings groups are not presented as one clear selector');
    assert(nav.every(button=>button.querySelector('svg[aria-hidden="true"]')),'A settings category is missing its navigation icon');
    assert(pages.length===7,'The seven remaining settings sections must be adopted');
    assert(!source.querySelector('[data-settings-section]'),'A visible settings section remained outside its Svelte owner');
    assert(source.querySelector('#settings-privacy') && source.querySelector('#settings-advanced'),'Compatibility diagnostics were removed instead of kept outside the user settings');
    assert(source.hidden && source.querySelector('#settings-privacy').closest('[hidden]')===source,'Removed information is not hidden');
    await select('settings-general');
    const appearanceSelect=document.getElementById('appearance-theme-select');
    const supervisorPermissionSelect=document.getElementById('supervisor-permission-select');
    const appearancePicker=document.getElementById('appearance-theme-select-picker-button');
    const appearanceMenu=document.getElementById('appearance-theme-select-picker-menu');
    const supervisorPermissionPicker=document.getElementById('supervisor-permission-select-picker-button');
    assert(appearanceSelect instanceof HTMLSelectElement && appearanceSelect.options.length===2,'Appearance is not a closed two-option selector');
    assert(supervisorPermissionSelect instanceof HTMLSelectElement && supervisorPermissionSelect.options.length===3,'Supervisor confirmations are not a closed selector');
    assert(appearanceSelect.getClientRects().length===0 && appearancePicker?.getClientRects().length>0,'Appearance did not replace the native field with a model-style picker');
    assert(appearanceMenu.hidden && appearancePicker.getAttribute('aria-expanded')==='false','Appearance options are visible before opening the picker');
    appearancePicker.click();await flush();
    assert(!appearanceMenu.hidden && appearancePicker.getAttribute('aria-expanded')==='true' && appearanceMenu.querySelectorAll('[role="option"]').length===2,'Appearance picker did not open its option menu');
    appearanceMenu.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true,cancelable:true}));await flush();
    assert(appearanceMenu.hidden && document.activeElement===appearancePicker,'Escape did not close the appearance picker and restore focus');
    assert(!view.querySelector('.appearance-choice-button,.mode-card'),'An exclusive setting still renders every option as a visible card');
    const pageChoiceSelects=[...view.querySelectorAll('.settings-pages select')].filter(select=>!select.closest('dialog,.compatibility-setting'));
    assert(pageChoiceSelects.length>=7 && pageChoiceSelects.every(select=>select.classList.contains('model-select') && select.closest('.model-field')?.querySelector('.model-picker-button')),'A user-facing Settings choice still uses a plain native selector');

    field.value='Unsubmitted server draft';
    for(const theme of ['central','central_dark']) {
      document.documentElement.dataset.theme=theme;
      for(const width of [560,720,1920]) {
        view.style.width=`${width}px`;
        await flush();
        const topbarBox=topbar.getBoundingClientRect(),navBox=document.getElementById('settings-nav').getBoundingClientRect(),contentBox=content.getBoundingClientRect();
        assert(topbarBox.bottom<=navBox.top+1 && navBox.bottom<=contentBox.bottom,`${theme}/${width}: the top bar, settings groups and content are not ordered vertically`);
        assert(parseFloat(getComputedStyle(topbar).borderBottomWidth)===0 && parseFloat(getComputedStyle(document.getElementById('settings-nav')).borderTopWidth)===0,`${theme}/${width}: a divider still separates settings search from content`);
        assert(navGroups.every(group=>group.getClientRects().length),`${theme}/${width}: a primary settings group is hidden`);
        const activePrimary=view.querySelector('.settings-primary-tab.active'),themeStyle=getComputedStyle(document.documentElement);
        assert(activePrimary && activePrimary.getAttribute('aria-selected')==='true' && themeStyle.getPropertyValue('--ca-settings-selected-background').trim()!==themeStyle.getPropertyValue('--ca-settings-nav-background').trim(),`${theme}/${width}: the selected primary group has insufficient visual distinction`);
        for(const button of nav) {
          button.click();await flush();
          const active=pages.filter(page=>!page.hidden);
          const expected=button.dataset.settingsTarget==='settings-agent'?2:1;
          assert(active.length===expected,`${theme}/${width}: category contains the wrong number of sections`);
          const expectedGroup=button.dataset.settingsTarget==='settings-codex-tools'?'settings-agent':button.dataset.settingsTarget;
          assert(active.every(page=>(page.dataset.settingsGroup||page.id)===expectedGroup),`${theme}/${width}: category navigation selected unrelated content`);
          assert(button.getAttribute('aria-current')==='page',`${theme}/${width}: active category is not announced`);
          const navHeight=button.getBoundingClientRect().height;
          assert(navHeight>=42 && navHeight<=60,`${theme}/${width}: navigation rows do not have a clear target size`);
          if(button.dataset.settingsTarget==='settings-agent') {
            const selector=view.querySelector('#supervisor-permission-select-picker-button');
            const writtenHeading=view.querySelector('.settings-plain-section-head');
            const selectorStyle=getComputedStyle(selector),headingStyle=getComputedStyle(writtenHeading);
            assert(parseFloat(selectorStyle.borderTopWidth)===0,`${theme}/${width}: a selector still has a resting border`);
            assert(selectorStyle.backgroundColor!==headingStyle.backgroundColor && selectorStyle.cursor==='pointer' && headingStyle.cursor!=='pointer',`${theme}/${width}: selectable and written surfaces are not clearly distinguished`);
            const personalityPanel=view.querySelector('#settings-agent .response-settings > .settings-option-panel');
            const workspacePanel=view.querySelector('#settings-agent .native-workspace-access');
            const confirmationPanel=view.querySelector('#settings-access .supervisor-permission-content');
            const accessHeading=view.querySelector('[data-config-section="access"] .section-heading');
            assert(personalityPanel?.contains(view.querySelector('#personality-picker-button')) && workspacePanel?.contains(view.querySelector('#codex-workspace-access-select-picker-button')) && confirmationPanel?.contains(selector),`${theme}/${width}: setting names and selectors are not grouped in their own panels`);
            assert(accessHeading && !workspacePanel.contains(accessHeading) && workspacePanel.querySelector('.access-note') && workspacePanel.querySelector('.settings-note'),`${theme}/${width}: Workspace access heading or permission explanations are outside their intended roles`);
            for(const panel of [personalityPanel,workspacePanel,confirmationPanel]) {
              const panelStyle=getComputedStyle(panel),control=panel.querySelector('.model-picker-button'),controlStyle=getComputedStyle(control);
              assert(parseFloat(panelStyle.borderTopWidth)===0 && panelStyle.backgroundColor!==getComputedStyle(shell).backgroundColor && panelStyle.cursor!=='pointer',`${theme}/${width}: a written setting panel has no distinct borderless surface`);
              assert(controlStyle.backgroundColor!==panelStyle.backgroundColor && controlStyle.cursor==='pointer',`${theme}/${width}: the inset selector is not visibly distinct from written setting content`);
              assert(panel.getBoundingClientRect().width<=content.clientWidth+1,`${theme}/${width}: a setting panel overflows the reading column`);
            }
            const title=view.querySelector('#settings-title'),subtitle=view.querySelector('.settings-page-title p'),section=view.querySelector('.settings-card-title'),label=view.querySelector('#supervisor-permission-select-picker-label'),help=view.querySelector('#supervisor-permission-select-picker-help');
            const size=element=>parseFloat(getComputedStyle(element).fontSize),weight=element=>parseFloat(getComputedStyle(element).fontWeight);
            assert(size(title)>size(section) && size(section)>size(label) && size(label)>size(help),`${theme}/${width}: page, section, control and help type sizes have no clear order`);
            assert(weight(title)>weight(section) && weight(section)>weight(label) && weight(label)>weight(help) && weight(subtitle)<weight(section),`${theme}/${width}: settings headings and explanations have no clear weight order`);
            assert(help.getBoundingClientRect().top>=label.getBoundingClientRect().bottom,`${theme}/${width}: the setting explanation still shares the name's baseline`);
          }
          const borderless=[search,back,primaryTabs,...view.querySelectorAll('.settings-primary-tab,.settings-secondary-tabs,.settings-secondary-tab,.settings-secondary-collapse,.settings-picker-button')];
          assert(borderless.every(control=>parseFloat(getComputedStyle(control).borderTopWidth)===0),`${theme}/${width}: a settings cylinder or selector still has a resting border`);
          assert(content.scrollWidth<=content.clientWidth+1,`${theme}/${width}: settings content overflows`);
        }
      }
    }

    await select('settings-ai');
    for(const patch of [
      {},{connecting:true},{connected:true},
      {connected:true,login:{url:'https://example.invalid/synthetic',userCode:'TEST-CODE'}},
      {connected:true,requirementsLoaded:true,account:{type:'chatgpt',email:'fixture@example.invalid',planType:'Pro',supported:true,policy:null}},
      {errors:{connection:'Synthetic connection failed. Retry when the service is available.'}}
    ]) {
      api.updateAppServerAccount({...account,...patch});await flush();
      assert(accountHost.scrollWidth<=accountHost.clientWidth+1,'An account state overflows Settings');
      if(patch.login)assert(accountHost.querySelector('.login-instructions').getClientRects().length,'Pending sign-in instructions are hidden');
      if(patch.errors)assert(accountHost.querySelector('[role="alert"]').getClientRects().length,'An account failure is hidden');
    }

    api.updateAppServerAccount({...account,connected:true,requirementsLoaded:true,account:{type:'chatgpt',email:null,planType:'pro',supported:true,policy:null}});await flush();
    const accountText=accountHost.textContent;
    for(const removed of ['Advanced Codex settings','Account token activity','Workspace messages','Protocol diagnostics','Managed permission profiles','Codex MCP servers'])assert(!accountText.includes(removed),`${removed} is still visible in user settings`);
    for(const retained of ['Subscription usage & limits','Saved chats','Account options'])assert(accountText.includes(retained),`${retained} was removed`);

    await setQuery('repair codex windows');
    let result=[...view.querySelectorAll('.settings-result')].find(button=>button.textContent.includes('Repair Codex on Windows'));
    assert(result,'Search missed the recovery action');result.click();await flush();
    const repair=accountHost.querySelector('[data-native-sandbox-setup]');
    const options=accountHost.querySelector('[data-settings-account-options]');
    assert(!search.value && options.open && repair.getClientRects().length,'Search did not reveal the repair action inside Account options');

    for(const removed of ['protocol diagnostics','feature flags','account token activity']) {
      await setQuery(removed);
      assert(!view.querySelector('.settings-result'),`${removed} remains searchable`);
    }
    search.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true,cancelable:true}));await flush();
    assert(!search.value && document.activeElement===search,'Escape did not clear settings search');

    await select('settings-access');
    assert(!document.getElementById('settings-agent').hidden && !document.getElementById('settings-access').hidden,'The compatibility permission route did not open the merged category');
    const permissionHeading=view.querySelector('[data-config-section="access"] .section-heading');
    assert(permissionHeading && view.querySelector('[data-config-section="access"] .configuration-content').dataset.collapsed==='false','Codex workspace access is not open for scanning');
    assert(permissionHeading.textContent.includes('Codex workspace access') && permissionHeading.textContent.includes('All Codex agents'),'Codex workspace access does not expose its global scope');
    assert(!view.querySelector('[data-settings-target="settings-access"]'),'Action permissions still has a separate navigation item');
    const supervisorPermissions=view.querySelector('.supervisor-permission-settings');
    assert(supervisorPermissions && supervisorPermissions.querySelector('.settings-plain-section-head') && supervisorPermissions.getClientRects().length,'Supervisor tool confirmations are not open for scanning');
    assert(view.textContent.includes("Supervisor's own tools use the confirmation policy below"),'The two permission scopes are not explained');
    assert(!view.querySelector('[data-config-section="tools"]').getClientRects().length,'Codex Skills and Apps still appear inside Agents & permissions');
    await select('settings-codex-tools');
    const toolsSection=view.querySelector('[data-config-section="tools"]');
    assert(view.querySelector('.codex-tools-copy').getClientRects().length,'The Codex tools category has no selected-conversation guidance');
    if(toolsSection?.getClientRects().length)assert(toolsSection.querySelector('.configuration-content').dataset.collapsed==='false','Available Codex Skills and Apps did not open in their category');
    assert(!view.querySelector('.agent-settings-primary').getClientRects().length,'Agent behavior or permissions leaked into Codex tools');
    await select('settings-agent');
    await flush();
    assert(supervisorPermissionSelect.getClientRects().length===0 && supervisorPermissionPicker?.getClientRects().length>0,'Supervisor confirmations did not use the shared model-style picker');
    supervisorPermissionPicker.click();await flush();
    assert(!document.getElementById('supervisor-permission-select-picker-menu').hidden && supervisorPermissionPicker.getAttribute('aria-expanded')==='true','Supervisor confirmation choices did not open on demand');
    supervisorPermissionPicker.click();await flush();

    await select('settings-servers');
    assert(field===document.getElementById(field.id) && field.value==='Unsubmitted server draft','Navigation replaced a control or discarded its draft');
    content.style.height='320px';content.style.maxHeight='320px';await flush();
    content.scrollTop=220;const savedScroll=content.scrollTop;
    assert(savedScroll>0,'Server page does not provide a scroll restoration fixture');
    await select('settings-general');await select('settings-servers');
    assert(Math.abs(content.scrollTop-savedScroll)<2,'Returning to a category lost its reading position');
    content.style.height=oldContentHeight;content.style.maxHeight=oldContentMaxHeight;

    await select('settings-ai');
    const dialog=accountHost.querySelector('dialog[aria-label="Repair Codex on Windows"]');dialog.showModal();
    assert(!api.selectSettingsSection('settings-general') && dialog.open,'Navigation hid an open repair confirmation');
    window.centralAgentCommitSettingsClose();await flush();
    assert(!dialog.open,'Closing Settings stranded a modal');
    assert(originalControls.every(control=>document.getElementById(control.id)===control),'Settings navigation remounted a stable form control');
    assert(!api.selectSettingsSection('settings-privacy') && !api.selectSettingsSection('settings-advanced') && !api.selectSettingsSection('unknown-section'),'A removed or invalid category remains selectable');
    assert(actions.length===0,'Settings navigation or search dispatched an account operation');
  } finally {
    accountHost.removeEventListener('central-agent:app-server-account',capture);
    api.updateAppServerAccount(account);field.value=fieldValue;view.style.width=oldWidth;content.style.height=oldContentHeight;content.style.maxHeight=oldContentMaxHeight;sessionAccessModal.hidden=sessionAccessWasHidden;
    for(const [element,open] of details)element.open=open;
    if(oldTheme)document.documentElement.dataset.theme=oldTheme;else delete document.documentElement.dataset.theme;
    window.centralAgentCommitSettingsClose();
  }
} catch(error) {startupErrors.push(`Settings: ${error?.stack||String(error)}`);}
