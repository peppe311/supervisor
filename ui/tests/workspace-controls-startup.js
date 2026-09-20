// Runs after the other agent-panel fixtures because it renders a synthetic
// workspace state. It performs no native action and never touches real data.
try {
  const closeEnough=(left,right)=>Math.abs(left-right)<.5;
  for(const theme of ['central','central_dark']){
    window.renderAgentPanelState({
      appearance:{theme},settingsOpen:false,
      workspace:{connected:true,root:'C:\\Fixture\\Project',projects:[{root:'C:\\Fixture\\Project',name:'Fixture project',active:true,archived:false}],entries:[]},
      projectChats:[],permission:{mode:'every_action',sessionAuthorized:false,authorizedScopes:[],pending:null},
      agent:{active:false,phase:'idle'},provider:{phase:'unavailable',authenticated:false},providers:{},
    });
    await Promise.resolve();await Promise.resolve();
    const add=document.querySelector('[data-workspace-control="add-chat"]');
    const menu=document.querySelector('[data-workspace-control="project-menu"]');
    if(!add||!menu)throw new Error(`${theme}: project row controls were not rendered`);
    const center=element=>{const bounds=element.getBoundingClientRect();return{x:bounds.left+bounds.width/2,y:bounds.top+bounds.height/2};};
    const addBox=add.getBoundingClientRect(),menuBox=menu.getBoundingClientRect();
    const addCenter=center(add),menuCenter=center(menu),addIconCenter=center(add.querySelector('svg')),menuIconCenter=center(menu.querySelector('svg'));
    if(!closeEnough(addBox.width,menuBox.width)||!closeEnough(addBox.height,menuBox.height)||!closeEnough(addCenter.y,menuCenter.y)){
      throw new Error(`${theme}: project add and overflow controls do not share one box and center line`);
    }
    if(![addCenter.x-addIconCenter.x,addCenter.y-addIconCenter.y,menuCenter.x-menuIconCenter.x,menuCenter.y-menuIconCenter.y].every(offset=>Math.abs(offset)<.5)){
      throw new Error(`${theme}: a project action glyph is not geometrically centered`);
    }
  }
}catch(error){startupErrors.push(`Workspace controls: ${String(error)}`);}
