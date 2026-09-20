<script lang="ts">
  import {onMount} from 'svelte';
  import {nativeCommands,commandQuery,commandReason,type NativeCommandName} from '../lib/native-commands';
  import type {NativeConversationView} from '../lib/native-history';
  let {inputId='chat-input',owner:fixedOwner,eventTarget}:{inputId?:string;owner?:string;eventTarget?:HTMLElement}=$props();
  let owner=$state(''),view=$state<NativeConversationView|null>(null),query=$state<string|null>(null);
  let selected=$state(0),dismissed=$state(false),focused=$state(false),notice=$state('');
  let input:HTMLTextAreaElement|null=null;
  function suggestions(){return query===null?[]:nativeCommands.filter(command=>command.name.startsWith(query!));}
  function menuVisible(){return Boolean(view?.visible && focused && !dismissed && query!==null && suggestions().length);}
  const listId=$derived(`${inputId}-native-commands`);

  function sync():void {
    query=input?commandQuery(input.value):null;selected=0;dismissed=false;notice='';
  }
  function choose(name:NativeCommandName):void {
    if(!input || !view?.visible)return;
    const reason=commandReason(name,view);
    if(reason){notice=reason;return;}
    if(!eventTarget && input.dispatchEvent(new CustomEvent('central-agent:slash-ui',{bubbles:true,cancelable:true,detail:{owner,action:'native_controls'}}))){
      notice='Open Settings → Agents & permissions to use this control. Your draft was kept.';return;
    }
    const original=input.value;
    const event=new CustomEvent('central-agent:native-command',{cancelable:true,detail:{owner,command:name}});
    window.dispatchEvent(event);
    if(!event.defaultPrevented){notice='This control is unavailable in the current conversation. Your draft was kept.';return;}
    // Only this shortcut text is consumed when its native control actually opens.
    // RPC acceptance, configuration consent, attachments and skill drafts are separate.
    if(input.value===original){input.value='';input.dispatchEvent(new Event('input',{bubbles:true}));}
    dismissed=true;query=null;
  }
  function submit(event:Event):void {
    if(event.target!==input || !view?.visible || !input)return;
    const parsed=commandQuery(input.value);
    if(parsed===null){
      if(/^\/[a-z-]+[ \t]+[^\r\n]+$/i.test(input.value)){
        event.preventDefault();notice='Open the slash command without arguments, then use its native dialog. Add a leading space to send this as ordinary text.';
      }
      return;
    }
    event.preventDefault();
    const command=nativeCommands.find(command=>command.name===parsed);
    if(command)choose(command.name);
    else notice='Choose a supported Codex control. This slash command was not sent. Add a leading space to send it as ordinary text.';
  }
  function keydown(event:KeyboardEvent):void {
    if(!menuVisible() || event.isComposing || event.ctrlKey || event.metaKey || event.altKey)return;
    const matches=suggestions();
    if(event.key==='Escape'){event.preventDefault();event.stopImmediatePropagation();dismissed=true;return;}
    if(event.key==='ArrowDown'||event.key==='ArrowUp'){
      event.preventDefault();event.stopImmediatePropagation();
      selected=(selected+(event.key==='ArrowDown'?1:matches.length-1))%matches.length;
      document.getElementById(`${listId}-${selected}`)?.scrollIntoView({block:'nearest'});return;
    }
    if(event.key==='Tab'){
      event.preventDefault();event.stopImmediatePropagation();
      if(input){input.value=`/${matches[selected].name}`;input.dispatchEvent(new Event('input',{bubbles:true}));}return;
    }
    if(event.key==='Enter'&&!event.shiftKey){event.preventDefault();event.stopImmediatePropagation();choose(matches[selected].name);}
  }
  onMount(()=>{
    owner=fixedOwner||'';
    input=document.getElementById(inputId) as HTMLTextAreaElement|null;
    const switchOwner=(event:Event)=>{if(!fixedOwner){owner=String((event as CustomEvent).detail||'');view=null;query=null;notice='';dismissed=true;}};
    const update=(event:Event)=>{
      const detail=(event as CustomEvent).detail;if(detail?.owner!==owner)return;
      if(detail.nativeConversation)view=detail.nativeConversation;
      if(!view?.visible){query=null;notice='';dismissed=true;}
    };
    const focus=()=>{focused=true;sync();};
    const blur=()=>{focused=false;};
    const target=eventTarget||window;
    input?.addEventListener('input',sync);
    input?.addEventListener('focus',focus);
    input?.addEventListener('blur',blur);
    input?.addEventListener('keydown',keydown,true);
    input?.addEventListener('central-agent:composer-command',submit);
    window.addEventListener('central-agent:conversation-key',switchOwner);
    window.addEventListener('central-agent:app-server-conversation',update);
    target.addEventListener('central-agent:conversation-state',update);
    return ()=>{
      input?.removeEventListener('input',sync);input?.removeEventListener('focus',focus);input?.removeEventListener('blur',blur);
      input?.removeEventListener('keydown',keydown,true);input?.removeEventListener('central-agent:composer-command',submit);
      window.removeEventListener('central-agent:conversation-key',switchOwner);window.removeEventListener('central-agent:app-server-conversation',update);
      target.removeEventListener('central-agent:conversation-state',update);
    };
  });
</script>

{#if menuVisible()}
  <div class="native-command-menu" role="group" aria-label="Codex command suggestions" id={listId}>
    <p>Codex controls · ↑ ↓ select · Tab complete · Enter open</p>
    {#each suggestions() as command,index (command.name)}
      <button type="button" id={`${listId}-${index}`} class:selected={selected===index}
        aria-current={selected===index?'true':undefined}
        aria-label={`/${command.name}: ${command.description}`} aria-disabled={Boolean(commandReason(command.name,view))}
        onpointerdown={event=>event.preventDefault()} onclick={()=>choose(command.name)}>
        <span>/{command.name}</span><span>{commandReason(command.name,view)||command.description}</span>
      </button>
    {/each}
  </div>
{/if}
{#if notice && view?.visible}<p class="native-command-notice" role="status">{notice}</p>{/if}

<style>
  .native-command-menu {flex:0 0 auto;min-width:0;max-height:var(--ca-command-menu-max-height);overflow:auto;background:var(--ca-app-background);color:var(--ca-text);border-radius:var(--ca-radius-medium);padding:var(--ca-space-2);}
  p {margin:var(--ca-space-2);font:var(--ca-type-caption)/var(--ca-leading-body) var(--ca-font-body);color:var(--ca-muted);}
  button {display:flex;flex-wrap:wrap;gap:var(--ca-space-3);width:100%;text-align:left;border:0;border-radius:var(--ca-control-radius);padding:var(--ca-space-2);background:var(--ca-app-background);color:var(--ca-text);font:var(--ca-type-label)/var(--ca-leading-body) var(--ca-font-body);}
  button span:first-child {font-family:var(--ca-font-mono);}
  button span:last-child {color:var(--ca-muted);}
  button.selected,button:hover {background:var(--ca-surface);}
  button:focus-visible {box-shadow:var(--ca-focus-ring);}
  .native-command-notice {overflow-wrap:anywhere;}
</style>
