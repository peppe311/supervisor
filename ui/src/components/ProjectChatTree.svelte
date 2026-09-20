<script lang="ts">
  import type {Snippet} from 'svelte';
  import {chatForest, chatRelationship, chatAge, type ProjectChat} from '../lib/chat-tree';
  import TaskVerificationStatus from './TaskVerificationStatus.svelte';
  type Options = {chats:ProjectChat[]; activeChatId:string|null; searching?:boolean; extra?:Snippet<[ProjectChat]>;
    onOpen:(chat:ProjectChat)=>void; onActions:(chat:ProjectChat,anchor:HTMLElement,label:HTMLElement)=>void};
  let {chats,activeChatId,searching=false,onOpen,onActions,extra}:Options=$props();
  let collapsed=$state(new Set<string>());
  const rows=$derived(chatForest(chats, searching ? new Set() : collapsed));
  export function update(options:Options):void {
    chats=options.chats;activeChatId=options.activeChatId;searching=!!options.searching;
    onOpen=options.onOpen;onActions=options.onActions;
  }
  function toggle(id:string):void {
    const next=new Set(collapsed); if(next.has(id))next.delete(id);else next.add(id);collapsed=next;
  }
</script>

<ul class="chat-tree" aria-label="Chats and forks">
  {#each rows as row (row.chat.id)}
    {@const chat=row.chat}
    {@const relation=chatRelationship(chat)}
    <li class="chat-entry">
      <div class="workspace-chat-row" class:active={chat.id===activeChatId} class:archived={chat.archived} class:working={chat.agentActive}
        data-chat-id={chat.id} data-chat-kind={chat.lineage?.kind || 'chat'} data-chat-depth={row.depth}
        style:padding-inline-start={`calc(var(--ca-space-3) * ${Math.min(row.depth,3)})`}>
        {#if row.children && !searching}
          <button class="branch-toggle" type="button" aria-label={`${collapsed.has(chat.id)?'Show':'Hide'} ${chat.lineage?.delegatedCount?'related conversations':'forks'} of ${chat.title || 'New chat'}`}
            aria-expanded={!collapsed.has(chat.id)} onclick={()=>toggle(chat.id)}>
            <svg viewBox="0 0 20 20" aria-hidden="true"><path d={collapsed.has(chat.id)?'m7 5 5 5-5 5':'m5 7 5 5 5-5'} /></svg>
          </button>
        {:else}
          <span class="branch-glyph" aria-hidden="true">
            {#if chat.lineage?.kind==='fork'||chat.lineage?.kind==='pending'}<svg viewBox="0 0 20 20"><path d="M5 3v7a3 3 0 0 0 3 3h8m-4-4 4 4-4 4" /></svg>{/if}
          </span>
        {/if}
        <button class="workspace-chat-open" type="button" disabled={chat.archived}
          aria-current={chat.id===activeChatId?'true':'false'} title={[chat.title||'New chat',relation].filter(Boolean).join(' · ')}
          aria-label={[chat.title||'New chat',relation,chat.needsAttention?'Needs attention':chat.agentActive?'Working':''].filter(Boolean).join(' · ')}
          onclick={()=>onOpen(chat)}>
          <span class="chat-copy"><span class="workspace-chat-title">{chat.title||'New chat'}</span>
            {#if relation}<span class="chat-relation">{relation}</span>{/if}
          </span>
          <span class="workspace-chat-status">
            {#if chat.verification}<TaskVerificationStatus owner={`chat:${chat.id}`} verification={chat.verification}/>
            {:else}<span class="workspace-chat-age">{chatAge(chat)}</span>{/if}
          </span>
        </button>
        <span class="workspace-chat-actions"><button class="workspace-row-menu" type="button"
          aria-label={`${chat.archived?'Archived chat actions':'Chat actions'} for ${chat.title||'New chat'}`} title="Chat actions"
          onclick={event=>{const anchor=event.currentTarget;const label=anchor.closest('li')?.querySelector<HTMLElement>('.workspace-chat-title');if(label)onActions(chat,anchor,label);}}>
          <svg class="workspace-menu-icon" viewBox="0 0 16 16" aria-hidden="true"><circle cx="4" cy="8" r="1"/><circle cx="8" cy="8" r="1"/><circle cx="12" cy="8" r="1"/></svg>
        </button></span>
      </div>
      {#if extra}<div class="chat-inline">{@render extra(chat)}</div>{/if}
    </li>
  {/each}
</ul>

<style>
  .chat-entry{display:grid;min-width:0;list-style:none}
  .chat-inline{min-width:0;list-style:none}.chat-inline:not(:has(:global(.agent-console))){display:none}
  .chat-tree .workspace-chat-row.working .workspace-chat-age{color:var(--ca-text);font-weight:600}
  .chat-tree .workspace-chat-row.working .workspace-chat-title{color:var(--ca-text)}
  .chat-tree {list-style:none;margin:0;padding:0;display:grid;gap:var(--chat-tree-gap,var(--ca-space-1));min-width:0;}
  .chat-tree .workspace-chat-row {display:flex;align-items:center;gap:var(--ca-space-1);min-width:0;background:transparent;}
  .branch-toggle,.branch-glyph {flex:0 0 var(--ca-space-4);width:var(--ca-space-4);min-width:0;display:grid;place-items:center;}
  .branch-toggle {min-height:var(--ca-control-compact);padding:0;border:0;border-radius:var(--ca-control-radius);background:transparent;color:var(--ca-muted);cursor:pointer;}
  svg {width:var(--ca-space-4);height:var(--ca-space-4);fill:none;stroke:currentColor;stroke-width:1.5;stroke-linecap:round;stroke-linejoin:round;}
  .branch-glyph {color:var(--ca-muted);}
  .chat-tree .workspace-chat-open {display:grid;flex:1;grid-template-columns:minmax(0,1fr) fit-content(35%);gap:0 var(--ca-space-2);align-items:center;min-width:0;min-height:var(--ca-control-compact);padding:var(--ca-space-1) 0;border:0;border-radius:var(--ca-control-radius);background:transparent;color:var(--ca-muted);text-align:left;font:450 var(--ca-type-navigation)/var(--ca-leading-body) var(--ca-font-navigation);}
  .chat-copy {display:contents;}
  .workspace-chat-title {grid-column:1;grid-row:1;overflow:hidden;font:450 var(--ca-type-navigation)/var(--ca-leading-body) var(--ca-font-navigation);text-overflow:ellipsis;white-space:nowrap;}
  .chat-tree .workspace-chat-row.active .workspace-chat-title {color:var(--ca-text);font-weight:600;}
  .chat-relation {grid-column:1 / -1;grid-row:2;min-width:0;color:var(--ca-muted);font:var(--ca-type-navigation)/var(--ca-leading-compact) var(--ca-font-navigation);white-space:normal;overflow-wrap:anywhere;}
  .workspace-chat-status {grid-column:2;grid-row:1;display:flex;min-width:0;align-items:center;justify-content:flex-end;gap:var(--ca-space-1);}
  .chat-tree .workspace-chat-age {min-width:0;max-width:none;white-space:normal;overflow-wrap:anywhere;text-align:right;color:var(--ca-muted);font:var(--ca-type-navigation)/var(--ca-leading-compact) var(--ca-font-navigation);}
  .chat-tree .workspace-chat-actions {display:grid;place-items:center;align-self:center;opacity:1;flex:0 0 auto;}
  .chat-tree .workspace-row-menu {display:grid;place-items:center;padding:0;line-height:1;}
  .workspace-menu-icon {width:var(--ca-space-3);height:var(--ca-space-3);fill:currentColor;stroke:none;}
  button:focus-visible {outline:none;box-shadow:var(--ca-focus-ring);}
  .branch-toggle:hover,.workspace-chat-open:hover {color:var(--ca-text);}
</style>
