export interface PopupRect { left:number; top:number; right:number; bottom:number; }

export function placeProfilePopup({anchor, conversation, viewport, contentLeft, width, height, gap}: {
  anchor:PopupRect; conversation:PopupRect; viewport:PopupRect;
  contentLeft:number; width:number; height:number; gap:number;
}) {
  const leftEdge=Math.max(viewport.left,conversation.left)+gap;
  const rightEdge=Math.max(leftEdge+1,Math.min(viewport.right,conversation.right)-gap);
  const topEdge=Math.max(viewport.top,conversation.top)+gap;
  const bottomEdge=Math.max(topEdge+1,Math.min(viewport.bottom,conversation.bottom)-gap);
  const popupWidth=Math.min(width,rightEdge-leftEdge);
  const gutterRight=Math.min(rightEdge,contentLeft-gap);
  const hasGutter=gutterRight-leftEdge>=popupWidth;
  const rightLimit=hasGutter?gutterRight:rightEdge;
  const left=Math.min(Math.max(anchor.left,leftEdge),rightLimit-popupWidth);
  const above=Math.max(0,Math.min(anchor.top-gap,bottomEdge)-topEdge);
  const below=Math.max(0,bottomEdge-Math.max(anchor.bottom+gap,topEdge));
  const side=above>=Math.min(height,bottomEdge-topEdge) || above>=below?"above":"below";
  const maxHeight=Math.max(1,side==="above"?above:below);
  const top=side==="above"?Math.max(topEdge,Math.min(anchor.top-gap,bottomEdge)-Math.min(height,maxHeight)):Math.max(topEdge,anchor.bottom+gap);
  return {left,top,width:popupWidth,maxHeight,side,hasGutter};
}
