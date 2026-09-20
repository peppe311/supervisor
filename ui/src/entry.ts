import { mount, unmount, flushSync, type ComponentProps } from "svelte";

import ChatSurface from "./components/ChatSurface.svelte";
import SupervisorLogo from "./components/SupervisorLogo.svelte";
import { createSupervisorMark } from "./lib/supervisor-brand";
import AgentConfiguration from "./components/AgentConfiguration.svelte";
import MainAgentSettings from "./components/MainAgentSettings.svelte";
import BrowserPanelToggle from "./components/BrowserPanelToggle.svelte";
import AgentGraphCard from "./components/AgentGraphCard.svelte";
import ProjectBoard from "./components/ProjectBoard.svelte";
import SettingsShell from "./components/SettingsShell.svelte";
import AppServerAccount from "./components/AppServerAccount.svelte";
import type { AppServerAccountView } from "./lib/app-server-account";
import WorkspaceExplorer from "./components/WorkspaceExplorer.svelte";
import ProjectRegistry from "./components/ProjectRegistry.svelte";
import ProjectChatTree from "./components/ProjectChatTree.svelte";
import { chatRelationship } from "./lib/chat-tree";
import { createWorkTimeline as createAgentTimelineController } from "./lib/work-disclosure";
import { updateFileEditor } from "./lib/file-editor";
import { renderDiff } from "./lib/diff-host";
import { renderNativeMedia } from "./lib/native-media-host";
import { acknowledgeGraphDraft, prefillGraphDraft } from "./lib/conversation-events";
import { graphConversationKey, graphConversationTarget, graphConversationOptions, graphWindowSelection } from "./lib/graph-conversations";
import "./styles/hosts.css";

function claim(target: HTMLElement): boolean {
  if (target.dataset.centralAgentSvelteMounted === "true") return false;
  target.dataset.centralAgentSvelteMounted = "true";
  return true;
}

let browserPanelToggle: { update: (minimized: boolean, disabled: boolean) => void } | undefined;
let appServerAccount: { update: (view: AppServerAccountView) => void } | undefined;
let mainAgentSettings: { prepareDialogs: () => void } | undefined;
let agentConfiguration: { setOpen: (open:boolean, immediate?:boolean) => void } | undefined;
let settingsShell: {select:(sectionId:string,selector?:string)=>boolean;visibilityChanged:(open:boolean)=>void} | undefined;
let projectRegistry: { update: (state: any) => void } | undefined;
let projectBoard: { update: (state: any) => void; selectProject: (key: string) => void; focusWork:(key:string,owner:string)=>void; revealAgent: (key: string) => void; projectKey: () => string; agentVisible: (key:string) => boolean } | undefined;
type BoardFileDragUpdate = {
  phase: "enter" | "over" | "leave" | "drop";
  x?: number;
  y?: number;
  fileCount?: number;
  directoryCount?: number;
};
let boardFileDragTarget: HTMLElement | null = null;
let boardFileDragCounts = {fileCount:0,directoryCount:0};
function updateProjectBoard(state: any): void { projectBoard?.update(state); flushSync(); }
function selectBoardProject(key: string): void { projectBoard?.selectProject(key); flushSync(); }
function focusBoardWork(key:string,owner:string):void {projectBoard?.focusWork(key,owner);flushSync();}
function revealBoardAgent(key: string): void { projectBoard?.revealAgent(key); flushSync(); }
function boardAgentVisible(key:string):boolean { return projectBoard?.agentVisible(key) ?? true; }
function boardProjectKey(): string { return projectBoard?.projectKey() || ''; }
function routeBoardFileDrag(update:BoardFileDragUpdate):string|null {
  if(Number.isFinite(update.fileCount))boardFileDragCounts.fileCount=Math.max(0,Number(update.fileCount));
  if(Number.isFinite(update.directoryCount))boardFileDragCounts.directoryCount=Math.max(0,Number(update.directoryCount));
  const phase=update.phase;
  let target:HTMLElement|null=null;
  if(phase!=="leave"&&Number.isFinite(update.x)&&Number.isFinite(update.y)){
    const scale=window.devicePixelRatio||1;
    target=document.elementFromPoint(Number(update.x)/scale,Number(update.y)/scale)?.closest<HTMLElement>('.agent-console[data-conversation-owner]')||null;
  }
  const detail={phase,fileCount:boardFileDragCounts.fileCount,directoryCount:boardFileDragCounts.directoryCount};
  if(boardFileDragTarget&&boardFileDragTarget!==target)boardFileDragTarget.dispatchEvent(new CustomEvent("central-agent:native-file-drag",{detail:{...detail,phase:"leave"}}));
  if(target)target.dispatchEvent(new CustomEvent("central-agent:native-file-drag",{detail}));
  const owner=phase==="drop" ? target?.dataset.conversationOwner||null : null;
  boardFileDragTarget=phase==="leave"||phase==="drop"?null:target;
  if(phase==="leave"||phase==="drop")boardFileDragCounts={fileCount:0,directoryCount:0};
  return owner;
}
function selectSettingsSection(sectionId:string,selector?:string):boolean {return settingsShell?.select(sectionId,selector)??false;}
function setSettingsVisibility(open:boolean):void {settingsShell?.visibilityChanged(open);}
function setAgentConfigurationOpen(open:boolean, immediate=false):void { agentConfiguration?.setOpen(open,immediate); }
function prepareAgentSettingsDialogs():boolean { if (!mainAgentSettings) return false; mainAgentSettings.prepareDialogs(); return true; }
function updateAppServerAccount(view: AppServerAccountView): void { if (view) appServerAccount?.update(view); }

function updateBrowserPanelToggle(minimized: boolean, disabled: boolean): void {
  browserPanelToggle?.update(minimized, disabled);
}
function updateProjectRegistry(state: any): void { projectRegistry?.update(state); }

function mountStaticSurfaces(): void {
  for (const logo of document.querySelectorAll<HTMLElement>('[data-central-agent-svelte="supervisor-logo"]')) {
    if (claim(logo)) mount(SupervisorLogo, {
      target: logo,
      props: { dimensional: logo.dataset.logoVariant === "dimensional" },
    });
  }
  const browserToggle = document.querySelector<HTMLElement>('[data-central-agent-svelte="browser-panel-toggle"]');
  if (browserToggle && claim(browserToggle)) {
    browserPanelToggle = mount(BrowserPanelToggle, {
      target: browserToggle,
      props: { onToggle: () => browserToggle.dispatchEvent(new CustomEvent("central-agent-toggle-web-panel", { bubbles: true })) },
    });
  }
  const explorer = document.querySelector<HTMLElement>('[data-central-agent-svelte="workspace-explorer"]');
  if (explorer && claim(explorer)) mount(WorkspaceExplorer, { target: explorer });

  const board = document.querySelector<HTMLElement>('[data-central-agent-svelte="project-board"]');
  if (board && claim(board)) projectBoard = mount(ProjectBoard, {target: board, props: {eventTarget: board}});
  const registry = document.querySelector<HTMLElement>('[data-central-agent-svelte="project-registry"]');
  if (!board && registry && claim(registry)) projectRegistry = mount(ProjectRegistry, { target: registry, props: { eventTarget: registry } });

  const configuration = document.querySelector<HTMLElement>('[data-central-agent-svelte="agent-configuration"]');
  if (configuration && claim(configuration)) agentConfiguration=mount(AgentConfiguration, { target:configuration });
  const agentSettings = document.querySelector<HTMLElement>('[data-central-agent-svelte="agent-settings"]');
  if (agentSettings && claim(agentSettings)) mainAgentSettings = mount(MainAgentSettings, { target:agentSettings, props:{host:agentSettings} });

  const chat = document.querySelector<HTMLElement>('[data-central-agent-svelte="chat-surface"]');
  if (chat && claim(chat)) mount(ChatSurface, { target: chat });

  const settings = document.querySelector<HTMLElement>('[data-central-agent-svelte="settings-shell"]');
  const settingsSource = document.getElementById('settings-sections-source');
  if (settings && settingsSource && claim(settings)) settingsShell=mount(SettingsShell, {target:settings,props:{source:settingsSource,sections:[...settingsSource.querySelectorAll<HTMLElement>('[data-settings-section]')]}});
  const account = document.querySelector<HTMLElement>('[data-central-agent-svelte="app-server-account"]');
  if (account && claim(account)) appServerAccount = mount(AppServerAccount, { target: account,
    props: {
      onAction: action => account.dispatchEvent(new CustomEvent("central-agent:app-server-account", {bubbles:true, detail:{action}})),
    } });
}

const graphCards = new WeakMap<HTMLElement, ReturnType<typeof mount>>();
type ChatTreeOptions = ComponentProps<typeof ProjectChatTree>;
const chatTrees = new Map<string,{target:HTMLElement;update:(options:ChatTreeOptions)=>void;dispose:()=>void}>();
function renderProjectChatTree(key:string, options:ChatTreeOptions):HTMLElement {
  let tree=chatTrees.get(key);
  if(!tree) {
    const target=document.createElement('div');
    target.dataset.centralAgentSvelte='project-chat-tree';
    const component=mount(ProjectChatTree,{target,props:options});
    tree={target,update:next=>component.update(next),dispose:()=>{void unmount(component);}};
    chatTrees.set(key,tree);
  } else tree.update(options);
  flushSync();
  return tree.target;
}
function pruneProjectChatTrees():void {
  for(const [key,tree] of chatTrees) if(!tree.target.isConnected){tree.dispose();chatTrees.delete(key);}
}
function unmountAgentGraphCard(target: HTMLElement): void {
  const card = graphCards.get(target);
  if (card) { void unmount(card); graphCards.delete(target); delete target.dataset.centralAgentSvelteMounted; }
}
function mountAgentGraphCard(target: HTMLElement): unknown {
  target.dataset.centralAgentSvelte = "knowledge-agent-popup";
  if (!claim(target)) return undefined;
  const owner=target.dataset.conversationOwner||`graph:${target.dataset.nodeKey}`;
  target.dataset.conversationOwner=owner;
  const card = mount(AgentGraphCard, { target, props: { eventTarget:target, owner, title:target.dataset.conversationTitle||"" } });
  graphCards.set(target, card);
  return card;
}

// Compatibility aliases preserve the established host API while new code uses
// Agent Graph terminology.
const mountKnowledgeAgentPopup = mountAgentGraphCard;
const unmountKnowledgeAgentPopup = unmountAgentGraphCard;

declare global {
  interface Window {
    CentralAgentSvelte?: {
      createSupervisorMark: typeof createSupervisorMark;
      createAgentTimelineController: typeof createAgentTimelineController;
      mountKnowledgeAgentPopup: typeof mountKnowledgeAgentPopup;
      unmountKnowledgeAgentPopup: typeof unmountKnowledgeAgentPopup;
      mountAgentGraphCard: typeof mountAgentGraphCard;
      unmountAgentGraphCard: typeof unmountAgentGraphCard;
      updateBrowserPanelToggle: typeof updateBrowserPanelToggle;
      updateProjectRegistry: typeof updateProjectRegistry;
      updateProjectBoard: typeof updateProjectBoard;
      selectBoardProject: typeof selectBoardProject;
      focusBoardWork: typeof focusBoardWork;
      revealBoardAgent: typeof revealBoardAgent;
      boardAgentVisible: typeof boardAgentVisible;
      boardProjectKey: typeof boardProjectKey;
      routeBoardFileDrag: typeof routeBoardFileDrag;
      updateAppServerAccount: typeof updateAppServerAccount;
      prepareAgentSettingsDialogs: typeof prepareAgentSettingsDialogs;
      setAgentConfigurationOpen: typeof setAgentConfigurationOpen;
      selectSettingsSection: typeof selectSettingsSection;
      setSettingsVisibility: typeof setSettingsVisibility;
      updateFileEditor: typeof updateFileEditor;
      renderDiff: typeof renderDiff;
      renderNativeMedia: typeof renderNativeMedia;
      renderProjectChatTree: typeof renderProjectChatTree;
      pruneProjectChatTrees: typeof pruneProjectChatTrees;
      chatRelationship: typeof chatRelationship;
      graphConversationKey: typeof graphConversationKey;
      graphConversationTarget: typeof graphConversationTarget;
      graphConversationOptions: typeof graphConversationOptions;
      graphWindowSelection: typeof graphWindowSelection;
      prefillGraphDraft: typeof prefillGraphDraft;
    };
  }
}

window.CentralAgentSvelte = { createSupervisorMark, createAgentTimelineController, mountKnowledgeAgentPopup, unmountKnowledgeAgentPopup, mountAgentGraphCard, unmountAgentGraphCard, updateBrowserPanelToggle, updateProjectRegistry, updateProjectBoard, selectBoardProject, focusBoardWork, revealBoardAgent, boardAgentVisible, boardProjectKey, routeBoardFileDrag, updateAppServerAccount, prepareAgentSettingsDialogs, setAgentConfigurationOpen, selectSettingsSection, setSettingsVisibility, updateFileEditor, renderDiff, renderNativeMedia, renderProjectChatTree, pruneProjectChatTrees, chatRelationship, graphConversationKey, graphConversationTarget, graphConversationOptions, graphWindowSelection, prefillGraphDraft };
window.addEventListener("central-agent:submission-accepted", event => acknowledgeGraphDraft((event as CustomEvent).detail || {}));
mountStaticSurfaces();
flushSync();
