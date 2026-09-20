import { ConversationDrafts } from "./conversation-drafts.ts";

function create(prefix:string):ConversationDrafts {
  const drafts=new ConversationDrafts(()=>window.sessionStorage,prefix,
    typeof window==="undefined"?undefined:{
      send:detail=>document.dispatchEvent(new CustomEvent("central-agent:composer-draft",{detail})),
      updated:(owner,error)=>window.dispatchEvent(new CustomEvent("central-agent:composer-draft-updated",{detail:{owner,error}})),
    });
  if(typeof window!=="undefined")window.addEventListener("central-agent:composer-draft-result",event=>drafts.receive((event as CustomEvent).detail));
  if(typeof window!=="undefined")window.addEventListener("central-agent:composer-draft-forgotten",event=>{
    const detail=(event as CustomEvent).detail;
    if(typeof detail?.owner==="string" && typeof detail.revision==="string")drafts.forget(detail.owner,detail.revision);
  });
  return drafts;
}
// One cache per trusted WebView, preserved through component unmount/remount.
export const mainDrafts=create("central-agent:composer-draft-v1");
export const graphDrafts=create("central-agent:graph-draft-v1");
