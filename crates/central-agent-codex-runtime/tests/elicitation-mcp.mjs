// Disposable native integration fixture. No network, file access or production installation.
import { createInterface } from 'node:readline';
const pending=new Map();
let serial=0;
const send=value=>process.stdout.write(JSON.stringify({jsonrpc:'2.0',...value})+'\n');
createInterface({input:process.stdin}).on('line',line=>{
  const message=JSON.parse(line);
  if (!message.method) {
    const call=pending.get(message.id);
    if (call===undefined) throw new Error('Unexpected fixture response');
    pending.delete(message.id);
    progress(call.token,2,'Fixture received decision');
    if(message.error) send({id:call.id,result:{content:[{type:'text',text:'Elicitation failed'}],isError:true}});
    else send({id:call.id,result:{content:[{type:'text',text:'Fixture decision received'}],structuredContent:{...message.result,fixtureProgressTokenPresent:call.token!==undefined},isError:false}});
    return;
  }
  if(message.id===undefined) return;
  const id=message.id;
  switch(message.method) {
    case 'initialize': send({id,result:{protocolVersion:message.params.protocolVersion,capabilities:{tools:{}},serverInfo:{name:'elicitation-fixture',version:'1.0.0'}}});break;
    case 'tools/list': send({id,result:{tools:[{name:'fixture_question',description:'Synthetic elicitation only; no effects',inputSchema:{type:'object',properties:{mode:{type:'string',enum:['form','url']}},required:['mode']}}]}});break;
    case 'resources/list': send({id,result:{resources:[]}});break;
    case 'resources/templates/list': send({id,result:{resourceTemplates:[]}});break;
    case 'ping': send({id,result:{}});break;
    case 'tools/call': {
      if(message.params.name!=='fixture_question'||!['form','url'].includes(message.params.arguments?.mode)) {
        send({id,error:{code:-32602,message:'Only the fixture question is supported'}});break;
      }
      const requestId=`fixture-${++serial}`;
      const token=message.params._meta?.progressToken;
      pending.set(requestId,{id,token});
      progress(token,1,'Fixture waiting for decision');
      const mode=message.params.arguments.mode;
      const params=mode==='url'
        ? {mode,message:'Synthetic external step; never open this URL',url:'https://example.invalid/native-fixture',elicitationId:requestId}
        : {mode,message:'Synthetic native form',requestedSchema:{type:'object',properties:{label:{type:'string',title:'Fixture label'},enabled:{type:'boolean',title:'Enabled'},count:{type:'integer',minimum:1,maximum:9}},required:['label','enabled','count']}};
      send({id:requestId,method:'elicitation/create',params});break;
    }
    default: send({id,error:{code:-32601,message:'Unsupported fixture method'}});
  }
});

function progress(token,value,message) {
  if(token!==undefined) send({method:'notifications/progress',params:{progressToken:token,progress:value,total:2,message}});
}
