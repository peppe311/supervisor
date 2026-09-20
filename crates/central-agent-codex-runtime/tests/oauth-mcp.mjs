// Isolated loopback OAuth/MCP integration fixture. Never ship as a real service.
import http from 'node:http';
import {createHash} from 'node:crypto';
import {createInterface} from 'node:readline';
import {appendFileSync} from 'node:fs';
const trace=process.argv[2];
if(!trace)throw new Error('An owned temporary trace path is required');
let origin;
let grant;
const server=http.createServer(async(req,res)=>{
  const url=new URL(req.url,origin);
  appendFileSync(trace,`${req.method} ${url.pathname}\n`);
  let body='';for await(const chunk of req)body+=chunk;
  const reply=(value,status=200)=>{res.writeHead(status,{'Content-Type':'application/json'});res.end(JSON.stringify(value));};
  if(url.pathname.startsWith('/.well-known/oauth-protected-resource'))return reply({resource:`${origin}/mcp`,authorization_servers:[origin],scopes_supported:['fixture.read']});
  if(url.pathname.startsWith('/.well-known/oauth-authorization-server'))return reply({issuer:origin,authorization_endpoint:`${origin}/authorize`,token_endpoint:`${origin}/token`,registration_endpoint:`${origin}/register`,response_types_supported:['code'],grant_types_supported:['authorization_code','refresh_token'],code_challenge_methods_supported:['S256'],token_endpoint_auth_methods_supported:['none'],scopes_supported:['fixture.read']});
  if(url.pathname==='/register'){
    const data=JSON.parse(body);return reply({...data,client_id:'fixture-client',token_endpoint_auth_method:'none'},201);
  }
  if(url.pathname==='/authorize'){
    const redirect=new URL(url.searchParams.get('redirect_uri'));
    if(redirect.protocol!=='http:' || !['localhost','127.0.0.1','[::1]'].includes(redirect.hostname) || !redirect.pathname.startsWith('/callback') || url.searchParams.get('client_id')!=='fixture-client' || !url.searchParams.get('code_challenge'))return reply({error:'invalid_request'},400);
    grant={redirect:redirect.href,challenge:url.searchParams.get('code_challenge')};
    redirect.searchParams.set('code','fixture-code');redirect.searchParams.set('state',url.searchParams.get('state'));
    res.writeHead(302,{Location:redirect.href});res.end();return;
  }
  if(url.pathname==='/token'){
    const params=new URLSearchParams(body);
    if(!grant || params.get('code')!=='fixture-code' || params.get('redirect_uri')!==grant.redirect || createHash('sha256').update(params.get('code_verifier')||'').digest('base64url')!==grant.challenge)return reply({error:'invalid_grant'},400);
    return reply({access_token:'fixture-access',token_type:'Bearer',expires_in:3600,scope:'fixture.read'});
  }
  if(url.pathname==='/mcp'){
    if(req.headers.authorization!=='Bearer fixture-access'){
      res.writeHead(401,{'WWW-Authenticate':`Bearer resource_metadata="${origin}/.well-known/oauth-protected-resource"`});res.end();return;
    }
    if(req.method!=='POST'){res.writeHead(405);res.end();return;}
    const rpc=JSON.parse(body);appendFileSync(trace,`MCP ${rpc.method}\n`);
    if(rpc.id===undefined){res.writeHead(202);res.end();return;}
    let result;
    switch(rpc.method){
      case 'initialize':result={protocolVersion:rpc.params.protocolVersion,capabilities:{tools:{},resources:{}},serverInfo:{name:'oauth-fixture',version:'1.0.0'}};break;
      case 'tools/list':result={tools:[{name:'fixture_echo',description:'OAuth inventory-only fixture',inputSchema:{type:'object',properties:{}}}]};break;
      case 'resources/list':result={resources:[]};break;
      case 'resources/templates/list':result={resourceTemplates:[]};break;
      case 'ping':result={};break;
      default:return reply({jsonrpc:'2.0',id:rpc.id,error:{code:-32601,message:'Inventory only'}});
    }
    return reply({jsonrpc:'2.0',id:rpc.id,result});
  }
  reply({error:'not_found'},404);
});
server.listen(0,'127.0.0.1',()=>{origin=`http://127.0.0.1:${server.address().port}`;process.stdout.write(origin+'\n');});
createInterface({input:process.stdin}).on('line',async line=>{
  try{
    const {authorize}=JSON.parse(line);const url=new URL(authorize);
    if(url.origin!==origin || url.pathname!=='/authorize')throw new Error('Foreign authorization URL');
    const response=await fetch(url,{redirect:'manual'});
    if(response.status!==302)throw new Error('Fixture authorization was rejected');
    const callback=new URL(response.headers.get('location'));
    if(callback.protocol!=='http:' || !['localhost','127.0.0.1','[::1]'].includes(callback.hostname) || !callback.pathname.startsWith('/callback'))throw new Error('Non-loopback callback rejected');
    const finished=await fetch(callback,{redirect:'manual'});
    process.stdout.write(JSON.stringify({authorized:finished.ok})+'\n');
  }catch{process.stdout.write(JSON.stringify({authorized:false})+'\n');}
});
