// Runs inside --check-ui-startup with synthetic events only. Delivery telemetry
// remains available to the native runtime, but must not leak into agent cards or
// the user-facing settings surface.
{
  const card=document.createElement('article');
  card.dataset.nodeKey='latency-fixture';
  document.body.append(card);
  const flush=async()=>{await Promise.resolve();await Promise.resolve();};
  const assert=(condition,message)=>{if(!condition)throw new Error(message);};
  try {
    window.CentralAgentSvelte.mountKnowledgeAgentPopup(card);await flush();
    const owner='graph:latency-fixture',threadId='latency-thread';
    card.dispatchEvent(new CustomEvent('central-agent:conversation-state',{detail:{owner,nativeConversation:{visible:true,connected:false,busy:false,observed:true,binding:{threadId,archived:false,deleted:false},name:'Fixture'}}}));
    card.dispatchEvent(new CustomEvent('central-agent:native-event-log',{detail:{owner,threadId,log:{entries:[],retainedLimit:2048,persistent:false,error:null,deliveryTimings:[{mode:'ready',outcome:'accepted',clientToHostMs:2,preparedMs:8,writtenMs:10,acknowledgedMs:30,nativeInputMs:25,firstUpdateMs:null}]}}}));
    await flush();
    assert(!card.querySelector('.native-event-log'),'Protocol diagnostics leaked into an agent card');
    assert(!card.textContent.includes('Prompt delivery timings') && !card.textContent.includes('Not observed'),'Delivery telemetry leaked into user-facing copy');
  } catch(error) {startupErrors.push(`Delivery telemetry boundary: ${error?.stack||String(error)}`);}
  finally {window.CentralAgentSvelte.unmountKnowledgeAgentPopup(card);await flush();card.remove();}
}
