// SPDX-License-Identifier: MPL-2.0
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { assertNoLinks, copyInputs, listInputs, sha256 } from './publication-files.mjs';
import { assertPublicationPrivacy } from './publication-privacy.mjs';

const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const base=assertNoLinks(root,path.join(root,'target/publication'));
const destination=assertNoLinks(root,path.join(base,'source'));
const manifestPath=path.join(base,'source-manifest.json');
// Refuse private incoming content before touching an existing owned snapshot.
const inputs=listInputs(root);
for(const file of inputs)assertPublicationPrivacy(file,fs.readFileSync(assertNoLinks(root,path.join(root,file))));
fs.mkdirSync(base,{recursive:true});
if(fs.existsSync(destination)){
  // Re-sync only an owned snapshot with no independent source edits. Never
  // delete directories recursively; its independent .git stays untouched.
  if(!fs.existsSync(manifestPath))throw Error('Existing publication directory has no ownership manifest');
  const previous=JSON.parse(fs.readFileSync(manifestPath,'utf8'));
  if(previous.schema!=='supervisor.publication-snapshot')throw Error('Invalid publication ownership');
  const known=new Set(previous.files.map(f=>f.path));
  function inspect(dir){for(const entry of fs.readdirSync(dir,{withFileTypes:true})){
    const file=assertNoLinks(destination,path.join(dir,entry.name));
    if(dir===destination&&entry.name==='.git')continue;
    if(entry.isDirectory())inspect(file);
    else if(!known.has(path.relative(destination,file).replaceAll(path.sep,'/')))throw Error('Snapshot contains unowned files; preserve and review them');
  }}
  inspect(destination);
  for(const record of previous.files){
    const file=assertNoLinks(destination,path.join(destination,record.path));
    if(!fs.existsSync(file)||sha256(fs.readFileSync(file))!==record.sha256)throw Error(`Snapshot was edited independently: ${record.path}`);
  }
  for(const record of previous.files)fs.unlinkSync(assertNoLinks(destination,path.join(destination,record.path)));
}
const files=copyInputs(root,destination,inputs);
fs.writeFileSync(manifestPath,JSON.stringify({schema:'supervisor.publication-snapshot',files},null,2)+'\n');
console.log(`Publication snapshot: ${files.length} files, ${(files.reduce((sum,f)=>sum+f.bytes,0)/1024/1024).toFixed(1)} MiB. Development Git history is not copied.`);
