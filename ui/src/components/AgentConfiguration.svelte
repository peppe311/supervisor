<script lang="ts">
  import { onMount } from "svelte";
  import ModelPicker from "./ModelPicker.svelte";
  import { placeProfilePopup } from "../lib/profile-popup";

  let menu:HTMLDivElement;
  let open=false, revision=0, frame=0;
  let popupMotion:Animation|undefined;
  let markMotion:Animation[]=[];
  const reduced=()=>window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const button=()=>document.getElementById("tetra-config-button")!;
  const host=()=>menu.parentElement!;
  const paths=()=>[...button().querySelectorAll<SVGPathElement>("[data-supervisor-logo] path")];

  function position():void {
    if (!open) return;
    const anchor=button().getBoundingClientRect();
    const conversation=document.querySelector<HTMLElement>(".conversation-section")!;
    if (!anchor.width || !anchor.height || !conversation.getBoundingClientRect().width) { setOpen(false,true); return; }
    const chat=conversation.querySelector<HTMLElement>(".chat-messages")!;
    const style=getComputedStyle(menu), view=window.visualViewport;
    const viewport={left:view?.offsetLeft||0,top:view?.offsetTop||0,right:(view?.offsetLeft||0)+(view?.width||innerWidth),bottom:(view?.offsetTop||0)+(view?.height||innerHeight)};
    if(anchor.right<=viewport.left || anchor.left>=viewport.right || anchor.bottom<=viewport.top || anchor.top>=viewport.bottom){setOpen(false,true);return;}
    const geometry=placeProfilePopup({anchor,conversation:conversation.getBoundingClientRect(),viewport,
      contentLeft:chat.getBoundingClientRect().left+chat.clientLeft+parseFloat(getComputedStyle(chat).paddingLeft),
      width:parseFloat(style.getPropertyValue("--ca-profile-popup-width")),height:menu.scrollHeight,
      gap:parseFloat(style.getPropertyValue("--ca-space-2"))});
    Object.assign(host().style,{left:`${geometry.left}px`,top:`${geometry.top}px`,width:`${geometry.width}px`});
    menu.style.maxHeight=`${geometry.maxHeight}px`;
    menu.style.transformOrigin=`${anchor.left+anchor.width/2-geometry.left}px ${anchor.top+anchor.height/2-geometry.top}px`;
    host().dataset.placement=geometry.hasGutter?"gutter":"chat-inset";
  }
  function schedulePosition():void {
    if (open && !frame) frame=requestAnimationFrame(()=>{frame=0;position();});
  }
  export function setOpen(next:boolean, immediate=false):void {
    if (next===open && !immediate) return;
    const wasVisible=!menu.hidden, current=getComputedStyle(menu);
    const from={opacity:wasVisible?current.opacity:"0",transform:wasVisible?current.transform:`scale(${current.getPropertyValue("--ca-profile-popup-scale").trim()})`};
    const pieces=paths(), transforms=pieces.map(path=>getComputedStyle(path).transform);
    const generation=++revision;
    open=next;
    popupMotion?.cancel();markMotion.forEach(animation=>animation.cancel());markMotion=[];
    host().hidden=false;menu.hidden=false;menu.inert=!next;
    button().setAttribute("aria-expanded",String(next));
    menu.setAttribute("aria-hidden",String(!next));
    menu.dataset.phase=next?"opening":"closing";
    // Measure the fixed host at its new width before placing its bottom edge.
    if(next){position();position();if(!open)return;}
    const style=getComputedStyle(menu);
    const duration=immediate||reduced()?0:parseFloat(style.getPropertyValue("--ca-profile-motion-duration"));
    const finish=()=>{
      if(generation!==revision)return;
      menu.dataset.phase=next?"open":"closed";
      if(!next){menu.hidden=true;host().hidden=true;}
      popupMotion?.cancel();popupMotion=undefined;
      if(!next){markMotion.forEach(animation=>animation.cancel());markMotion=[];}
    };
    if(!duration){finish();return;}
    const split=parseFloat(style.getPropertyValue("--ca-profile-mark-split"));
    const angle=parseFloat(style.getPropertyValue("--ca-profile-mark-angle"));
    const easing=style.getPropertyValue("--ca-ease-emphasized").trim();
    markMotion=pieces.map((path,index)=>{
      const direction=index===0?1:-1;
      return path.animate([{transform:transforms[index]},{transform:next?`translate(${direction*split}px, ${-direction*split}px) rotate(${direction*angle}deg)`:"none"}],{duration,easing,fill:"forwards"});
    });
    popupMotion=menu.animate([from,{opacity:next?"1":"0",transform:next?"none":`scale(${style.getPropertyValue("--ca-profile-popup-scale").trim()})`}],{duration,easing,fill:"forwards"});
    void popupMotion.finished.then(finish,()=>{});
  }
  onMount(()=>{
    const observer=new ResizeObserver(schedulePosition);
    observer.observe(menu);observer.observe(document.querySelector(".conversation-section")!);observer.observe(button());
    observer.observe(document.getElementById("composer")!);
    const preference=window.matchMedia("(prefers-reduced-motion: reduce)");
    const motionChanged=()=>{if(preference.matches)setOpen(open,true);};
    preference.addEventListener("change",motionChanged);
    window.addEventListener("resize",schedulePosition);
    window.addEventListener("scroll",schedulePosition,true);
    window.visualViewport?.addEventListener("resize",schedulePosition);
    window.visualViewport?.addEventListener("scroll",schedulePosition);
    return ()=>{
      revision++;observer.disconnect();cancelAnimationFrame(frame);popupMotion?.cancel();markMotion.forEach(animation=>animation.cancel());
      preference.removeEventListener("change",motionChanged);window.removeEventListener("resize",schedulePosition);window.removeEventListener("scroll",schedulePosition,true);
      window.visualViewport?.removeEventListener("resize",schedulePosition);window.visualViewport?.removeEventListener("scroll",schedulePosition);
    };
  });

  const providers = [
    { value: "codex_app_server", label: "Codex" },
    { value: "claude_code", label: "Claude Code" },
    { value: "cursor", label: "Cursor" },
    { value: "github_copilot", label: "GitHub Copilot" },
    { value: "google_antigravity", label: "Google AI Pro / Ultra" },
    { value: "opencode_go", label: "OpenCode Go" },
  ];
  function close(event: MouseEvent): void {
    (event.currentTarget as HTMLElement).dispatchEvent(new CustomEvent("central-agent:close-configuration", { bubbles: true }));
  }
</script>

<div bind:this={menu} id="tetra-config-menu" class="tetra-config-menu" role="dialog" aria-label="Agent configuration" data-phase="closed" hidden>
  <div class="configuration-heading">
    <span>Agent profile</span>
    <button type="button" aria-label="Close agent configuration" title="Close profile" onclick={close}>×</button>
  </div>
  <div class="tetra-config-part" data-config-part="subscription">
    <ModelPicker kind="provider" label="Provider" initialValue="Claude Code" options={providers} />
  </div>
  <div class="tetra-config-part" data-config-part="model">
    <ModelPicker kind="model" label="Model" initialValue="Loading…" />
  </div>
  <div class="tetra-config-part" data-config-part="effort">
    <ModelPicker kind="effort" label="Effort" initialValue="—" />
  </div>
  <div class="tetra-config-part" data-config-part="speed">
    <ModelPicker kind="speed" label="Speed" initialValue="Standard" />
  </div>
  <p id="tetra-config-summary" hidden></p>
</div>

<style>
  :global(#agent-configuration-host) { position:fixed; min-width:0; min-height:0; z-index:95; }
  :global(#agent-configuration-host[hidden]) { display: none; }
  .tetra-config-menu { position:relative; display:grid; width:100%; min-width:0; min-height:0; grid-template-columns:repeat(2,minmax(0,1fr)); align-content:start; gap:var(--ca-space-3); padding:var(--ca-space-4); overflow:auto; overscroll-behavior:contain; border:0; border-radius:var(--ca-radius-large); background:var(--ca-surface-2); color:var(--ca-text); box-shadow:var(--ca-shadow-float); }
  .tetra-config-menu[hidden] { display:none; }
  .configuration-heading { grid-column:1/-1; display:flex; align-items:center; gap:var(--ca-space-2); color:var(--ca-text); font-size:var(--ca-type-label); font-weight:650; }
  .configuration-heading span { flex: 1; min-width: 0; }
  .configuration-heading button { padding: var(--ca-space-2); border: 0; border-radius: var(--ca-radius-small); background: transparent; color: var(--ca-muted); font: inherit; cursor: pointer; }
  .configuration-heading button:hover { background: var(--ca-surface-3); }
  .configuration-heading button:focus-visible { outline: 1px solid var(--ca-border-strong); }
  .tetra-config-part { min-width:0; }
  .tetra-config-part[data-config-part="subscription"], .tetra-config-part[data-config-part="model"] { grid-column:1/-1; }
  .tetra-config-menu :global(.model-label) { margin-bottom: var(--ca-space-1); font: var(--ca-type-caption)/var(--ca-leading-body) var(--ca-font-body); letter-spacing: normal; text-transform: none; }
  .tetra-config-menu :global(.model-picker-button) { width:100%; min-height:var(--ca-control-default); height:auto; padding:var(--ca-space-2); border:0; background:var(--ca-surface-3); color:var(--ca-text); box-shadow:none; }
  .tetra-config-menu :global(.model-picker-value) { white-space: normal; overflow-wrap: anywhere; text-align: left; }
  #tetra-config-menu :global(.model-picker-menu) { position:static; inset:auto; width:100%; min-width:0; max-height:var(--ca-command-menu-max-height); margin-block:var(--ca-space-1); border:0; box-shadow:none; overflow:auto; transform:none; animation:none; background:var(--ca-surface-2); }
  .tetra-config-menu :global(.model-picker-option-title) { overflow: visible; text-overflow: clip; white-space: normal; overflow-wrap: anywhere; }
  .tetra-config-menu :global(.model-picker-option-description) { display: block; overflow: visible; white-space: normal; overflow-wrap: anywhere; -webkit-line-clamp: unset; line-clamp: unset; }
  :global(#tetra-config-visual svg) { overflow:visible; }
  :global(#tetra-config-visual path) { transform-box:fill-box; transform-origin:center; }
</style>
