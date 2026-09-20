export const settingsCategories = [
  {id:"settings-general",title:"General",description:"Appearance and browser preferences."},
  {id:"settings-ai",title:"AI accounts",description:"Connect the providers you want to use."},
  {id:"settings-agent",title:"Agents & permissions",description:"Choose how your agents work and what they can access."},
  {id:"settings-codex-tools",title:"Codex tools",description:"Choose Skills and Plugins for the selected Codex conversation."},
  {id:"settings-workspace",title:"Projects",description:"Manage the folders Supervisor works in."},
  {id:"settings-servers",title:"Servers",description:"Manage SSH and remote desktop connections."},
  {id:"settings-system",title:"Windows access",description:"Control access to Windows applications."},
] as const;

export type SettingsId = typeof settingsCategories[number]["id"];
export interface SettingsResult {section:SettingsId;title:string;description:string;keywords:string;selector?:string;}

export const settingsGroups:ReadonlyArray<{id:string;title:string;categories:readonly SettingsId[]}>= [
  {id:"personal",title:"Personal",categories:["settings-general"]},
  {id:"ai",title:"AI",categories:["settings-ai","settings-agent","settings-codex-tools"]},
  {id:"workspace",title:"Workspace",categories:["settings-workspace"]},
  {id:"connections",title:"Connections",categories:["settings-servers","settings-system"]},
];

const aliases:Record<string,SettingsId>={"settings-access":"settings-agent"};
export function resolveSettingsId(value:string):SettingsId|null {
  if(aliases[value])return aliases[value];
  return settingsCategories.some(category=>category.id===value)?value as SettingsId:null;
}

const entries:SettingsResult[] = [
  {section:"settings-general",title:"Appearance",description:"Choose the light or dark app theme.",keywords:"appearance theme light dark colors contrast tema chiaro scuro",selector:"#appearance-theme-select-picker-button"},
  {section:"settings-general",title:"Search engine",description:"Choose the service used by the address bar.",keywords:"search engine google bing duckduckgo browser ricerca",selector:"#search-engine-select-picker-button"},
  {section:"settings-ai",title:"Codex account",description:"Sign in with ChatGPT and check the connection.",keywords:"login subscription account openai app server abbonamento",selector:"[data-central-agent-svelte='app-server-account']"},
  {section:"settings-ai",title:"Reload saved chats",description:"Refresh chats that already load automatically when the app opens.",keywords:"history cronologia reload restart riavvio",selector:"[data-native-chat-reload]"},
  {section:"settings-ai",title:"Subscription usage",description:"Check limits and reset times reported by Codex.",keywords:"usage rate limits token quota reset utilizzo",selector:"[data-native-rate-limits]"},
  {section:"settings-ai",title:"Other AI accounts",description:"Connect Claude Code, Cursor, Copilot, Google or OpenCode.",keywords:"claude cursor copilot google antigravity opencode provider login",selector:"[data-settings-other-providers]"},
  {section:"settings-ai",title:"Account options",description:"Reconnect, refresh, use a device code or sign out.",keywords:"login device code logout disconnect reconnect sign out",selector:"[data-settings-account-options]"},
  {section:"settings-ai",title:"Repair Codex on Windows",description:"Prepare the protected Codex environment if commands cannot run.",keywords:"repair troubleshoot fix sandbox windows install elevated setup commands",selector:"[data-native-sandbox-setup]"},
  {section:"settings-agent",title:"Personality",description:"Choose how the selected agent communicates.",keywords:"personality response style tono carattere",selector:"[data-config-section='response']"},
  {section:"settings-agent",title:"Codex workspace access",description:"Choose native execution access shared by every Codex agent.",keywords:"codex permissions execution sandbox read only workspace write full access persistent permessi",selector:"#codex-workspace-access-select-picker-button"},
  {section:"settings-agent",title:"Reasoning summaries",description:"Choose the detail shown while Codex works.",keywords:"reasoning summary concise detailed ragionamento",selector:"#reasoning-summary-select-picker-button"},
  {section:"settings-codex-tools",title:"Codex Skills",description:"Choose native instructions for the selected Codex conversation.",keywords:"codex tools skills instructions strumenti",selector:"[data-config-section='tools']"},
  {section:"settings-codex-tools",title:"Plugins",description:"Inspect the user-facing plugins installed in Codex. Choose a plugin from the message composer.",keywords:"codex tools apps plugins connectors strumenti",selector:"[data-config-section='tools'] .native-apps"},
  {section:"settings-agent",title:"Supervisor tool confirmations",description:"Choose when Supervisor's browser, terminal, SSH and project tools ask for confirmation.",keywords:"supervisor action approval confirmations authorize approve terminal ssh permission autorizzazione",selector:"#supervisor-permission-select-picker-button"},
  {section:"settings-workspace",title:"Project folders",description:"Add a folder and check where agents work.",keywords:"workspace directory files file cartella progetto",selector:".workspace-card"},
  {section:"settings-servers",title:"Add an SSH server",description:"Save the address, user and optional key path.",keywords:"vps ssh host ip key chiave port username connection",selector:"#ssh-profile-form"},
  {section:"settings-servers",title:"Remote desktop",description:"Open an RDP or VNC session through SSH.",keywords:"rdp vnc remote desktop linux ssh tunnel",selector:".remote-desktop-card"},
  {section:"settings-system",title:"Windows applications",description:"Share application windows with compatible agents.",keywords:"window awareness computer desktop schermo controllo",selector:".system-card"},
  {section:"settings-system",title:"Emergency stop",description:"Resume Windows control after the global emergency stop.",keywords:"emergency stop resume blocco",selector:".system-safety"},
];

const normalize=(value:string)=>value.normalize("NFD").replace(/[\u0300-\u036f]/g,"").toLowerCase();
export function searchSettings(query:string):SettingsResult[] {
  const words=normalize(query.trim()).split(/\s+/).filter(Boolean);
  if(!words.length)return [];
  return entries.filter(entry=>{
    const category=settingsCategories.find(category=>category.id===entry.section)!;
    const text=normalize(`${entry.title} ${entry.description} ${entry.keywords} ${category.title}`);
    return words.every(word=>text.includes(word));
  });
}
