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
