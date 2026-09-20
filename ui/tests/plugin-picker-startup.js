// Runs only in --check-ui-startup with a synthetic conversation. Plugin actions
// are intercepted before the native host so this probe never contacts a service.
try {
  const flushPluginPicker = async () => { await Promise.resolve(); await Promise.resolve(); };
  const requirePluginPicker = (condition, message) => { if (!condition) throw new Error(message); };
  const controls = [];
  const skillControls = [];
  const interceptPluginControl = event => {
    if (event.detail?.action?.kind !== 'apps_control') return;
    controls.push(event.detail);
    event.preventDefault();
    event.stopImmediatePropagation();
  };
  const interceptSkillControl = event => {
    skillControls.push(event.detail);
    event.preventDefault();
    event.stopImmediatePropagation();
  };
  window.addEventListener('central-agent:app-server-conversation-control', interceptPluginControl, true);
  window.addEventListener('central-agent:app-server-skills-control', interceptSkillControl, true);

  const owner = 'startup-plugin-picker';
  const baseApp = {
    id: 'github', name: 'GitHub', description: 'Repositories and issues',
    iconUrl: null,
    accessible: true, configuredEnabled: true, installed: true, runtimeEnabled: true, callable: true,
    tools: [
      {name:'list_issues',title:'List issues',description:'Read repository issues',enabled:true,disabledReason:null,readOnly:true},
      {name:'create_issue',title:'Create issue',description:'Create a repository issue',enabled:true,disabledReason:null,readOnly:false},
    ],
  };
  const plugins = [
    baseApp,
    {...baseApp,id:'cloudflare@openai-curated-remote',name:'Cloudflare',description:'Cloud infrastructure',tools:[]},
    {...baseApp,id:'figma',name:'Figma',description:'Product design',tools:[]},
    {...baseApp,id:'linear',name:'Linear',description:'Issues and projects',tools:[]},
    {...baseApp,id:'notion',name:'Notion',description:'Workspace documentation',tools:[]},
    {...baseApp,id:'airtable@fixture',name:'Airtable',description:'Structured data',iconUrl:'data:image/svg+xml,%3Csvg xmlns=%22http://www.w3.org/2000/svg%22/%3E',tools:[]},
  ];
  const conversation = selected => ({
    visible: true, connected: true, busy: false, observed: true, name: 'Plugin picker fixture', directory: 'C:\\fixture',
    binding: {threadId:'thread-plugin-picker',archived:false,deleted:false}, nativeLoaded: true,
    apps: {threadId:'thread-plugin-picker',inventoryId:'inventory-plugin-picker',items:plugins,selected,current:true,busy:false,error:null,unavailableReason:null,notice:null},
  });
  const draftConversation = {
    ...conversation([]),
    binding: null,
    nativeLoaded: false,
    apps: {threadId:'',inventoryId:'inventory-plugin-picker-draft',items:plugins,selected:[],current:true,busy:false,error:null,unavailableReason:null,notice:null},
  };
  window.dispatchEvent(new CustomEvent('central-agent:conversation-key', {detail:owner}));
  window.dispatchEvent(new CustomEvent('central-agent:app-server-conversation', {detail:{owner,nativeConversation:draftConversation,change:'snapshot'}}));
  window.dispatchEvent(new CustomEvent('central-agent:app-server-skills', {detail:{owner,selected:[],browserUse:'available',writing:false}}));
  await flushPluginPicker();

  const button = document.getElementById('plugin-picker-main-button');
  requirePluginPicker(button && !button.disabled, 'Plugin selector is disabled before the card first creates a native conversation');
  button.click();
  await flushPluginPicker();
  const draftPopover = document.getElementById('plugin-picker-main-popover');
  draftPopover?.querySelector('[data-plugin-id="github"]')?.click();
  await flushPluginPicker();
  requirePluginPicker(controls.at(-1)?.action?.action?.kind === 'attach', 'A new card cannot select a plugin for its first prompt');
  requirePluginPicker(controls.at(-1)?.action?.expected_thread_id === null, 'Draft plugin selection claimed a native thread before the first prompt');
  requirePluginPicker(controls.at(-1)?.action?.action?.inventory_id === 'inventory-plugin-picker-draft', 'Draft plugin selection lost its owner inventory');

  window.dispatchEvent(new CustomEvent('central-agent:app-server-conversation', {detail:{owner,nativeConversation:conversation([]),change:'snapshot'}}));
  await flushPluginPicker();

  requirePluginPicker(button && button.getAttribute('aria-expanded') === 'false', 'Composer plugin selector is missing or not closed initially');
  button.click();
  await flushPluginPicker();
  const popover = document.getElementById('plugin-picker-main-popover');
  requirePluginPicker(popover && button.getAttribute('aria-expanded') === 'true', 'Composer plugin selector did not open');
  const browserChoice = popover.querySelector('[data-plugin-id="browser@openai-bundled"]');
  requirePluginPicker(browserChoice?.querySelector('[data-plugin-icon="browser"][data-plugin-icon-source="local"]') && browserChoice.textContent.trim() === 'Browser', 'Official Browser is missing its local icon-and-name identity');
  const githubChoice = popover.querySelector('[data-plugin-id="github"]');
  requirePluginPicker(githubChoice?.querySelector('[data-plugin-icon="github"]') && githubChoice.textContent.trim() === 'GitHub', 'Plugin row is not reduced to the GitHub icon and name');
  for (const [id,name,kind] of [['cloudflare@openai-curated-remote','Cloudflare','cloudflare'],['figma','Figma','figma'],['linear','Linear','linear'],['notion','Notion','notion']]) {
    const choice = popover.querySelector(`[data-plugin-id="${id}"]`);
    requirePluginPicker(choice?.querySelector(`[data-plugin-icon="${kind}"][data-plugin-icon-source="local"]`) && choice.textContent.trim() === name, `${name} does not use its local icon-and-name identity`);
  }
  const catalogChoice=popover.querySelector('[data-plugin-id="airtable@fixture"]');
  requirePluginPicker(catalogChoice?.querySelector('[data-plugin-icon-source="catalog"] img') && catalogChoice.textContent.trim()==='Airtable','A catalog plugin does not use its official composer icon');
  requirePluginPicker(!popover.querySelector('details,input,[data-plugin-command]') && !/Repositories|Read only|May write|Enabled/.test(popover.textContent), 'Plugin menu still exposes descriptions, commands or suggestions');

  browserChoice.click();
  await flushPluginPicker();
  requirePluginPicker(skillControls.at(-1)?.action?.kind === 'attach_official_browser', 'Browser selection did not attach the verified official skill');
  requirePluginPicker(!document.getElementById('plugin-picker-main-popover'), 'Plugin menu did not close after Browser selection');
  const selectedBrowser = [{id:'selection-browser',name:'browser:control-in-app-browser',path:'C:\\official\\browser\\SKILL.md',directory:'C:\\fixture',current:true}];
  window.dispatchEvent(new CustomEvent('central-agent:app-server-skills', {detail:{owner,selected:selectedBrowser,browserUse:'available',writing:false}}));
  await flushPluginPicker();
  const browserChip = document.querySelector('[data-plugin-picker="main"] .selected-plugin-chip[data-plugin-id="browser@openai-bundled"]');
  requirePluginPicker(browserChip?.querySelector('[data-plugin-icon="browser"]') && browserChip.textContent.trim() === 'Browser', 'Selected Browser does not appear as its icon-and-name card');
  const browserRemove = browserChip.querySelector('.selected-plugin-remove');
  requirePluginPicker(browserRemove?.getAttribute('aria-label') === 'Remove Browser from the next prompt', 'Selected Browser has no dedicated X control');
  const skillsBeforeCardClick = skillControls.length;
  browserChip.click();
  await flushPluginPicker();
  requirePluginPicker(skillControls.length === skillsBeforeCardClick, 'Clicking the Browser card removed it without its X control');
  browserRemove.click();
  await flushPluginPicker();
  requirePluginPicker(skillControls.at(-1)?.action?.kind === 'remove' && skillControls.at(-1)?.action?.id === 'selection-browser', 'Browser cannot be removed from the next prompt');
  window.dispatchEvent(new CustomEvent('central-agent:app-server-skills', {detail:{owner,selected:[],browserUse:'available',writing:false}}));
  button.click();
  await flushPluginPicker();
  const pluginPopover = document.getElementById('plugin-picker-main-popover');
  const refreshedGithubChoice = pluginPopover.querySelector('[data-plugin-id="github"]');

  refreshedGithubChoice.click();
  await flushPluginPicker();
  requirePluginPicker(controls.at(-1)?.action?.action?.kind === 'attach', 'Plugin selection did not use the native Apps control');
  requirePluginPicker(controls.at(-1)?.action?.action?.inventory_id === 'inventory-plugin-picker', 'Plugin selection lost its native inventory identity');
  requirePluginPicker(!document.getElementById('plugin-picker-main-popover'), 'Plugin menu did not close after selection');

  const selected = [{selectionId:'selection-github',appId:'github',name:'GitHub',iconUrl:null,threadId:'thread-plugin-picker',current:true}];
  window.dispatchEvent(new CustomEvent('central-agent:app-server-conversation', {detail:{owner,nativeConversation:conversation(selected),change:'apps'}}));
  await flushPluginPicker();
  const chip = document.querySelector('[data-plugin-picker="main"] .selected-plugin-chip[data-plugin-id="github"]');
  requirePluginPicker(chip?.querySelector('[data-plugin-icon="github"]') && chip.textContent.trim() === 'GitHub', 'Selected plugin does not appear in the composer as its icon and name');
  requirePluginPicker(!chip.textContent.includes('$github'), 'Selected plugin chip exposes its protocol identifier');
  const removeButton = chip.querySelector('.selected-plugin-remove');
  requirePluginPicker(removeButton && removeButton.getAttribute('aria-label') === 'Remove GitHub from the next prompt', 'Selected plugin card has no dedicated remove control');
  requirePluginPicker(button.getAttribute('aria-label').includes('1 plugin selected'), 'Plugin trigger does not report selection count');

  const controlsBeforeCardClick = controls.length;
  chip.click();
  await flushPluginPicker();
  requirePluginPicker(controls.length === controlsBeforeCardClick, 'Clicking the selected plugin card removed it without its X control');
  removeButton.click();
  await flushPluginPicker();
  requirePluginPicker(controls.at(-1)?.action?.action?.kind === 'remove', 'Selected plugin cannot be removed from the next prompt');
  button.click();
  await flushPluginPicker();
  window.dispatchEvent(new KeyboardEvent('keydown', {key:'Escape',bubbles:true}));
  await flushPluginPicker();
  requirePluginPicker(button.getAttribute('aria-expanded') === 'false' && document.activeElement === button, 'Escape did not close the plugin selector and return focus');
  window.dispatchEvent(new CustomEvent('central-agent:app-server-conversation', {detail:{owner,nativeConversation:{...conversation([]),visible:false,connected:false,binding:null,nativeLoaded:false,apps:null},change:'provider'}}));
  await flushPluginPicker();
  window.removeEventListener('central-agent:app-server-conversation-control', interceptPluginControl, true);
  window.removeEventListener('central-agent:app-server-skills-control', interceptSkillControl, true);
} catch (error) {
  startupErrors.push(`Plugin picker: ${error?.stack || String(error)}`);
}
