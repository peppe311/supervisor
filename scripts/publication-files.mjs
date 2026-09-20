// SPDX-License-Identifier: MPL-2.0
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { assertPublicationPrivacy } from './publication-privacy.mjs';

const roots = new Set(['src','crates','ui','assets','protocol','vendor','scripts','docs','licenses','.github']);
const files = new Set(['.gitignore','.gitattributes','.gitleaks.toml','.gitleaksignore','AGENTS.md','DESIGN.md','CODEX_PERSONALIZATION.md',
  'Cargo.toml','Cargo.lock','rust-toolchain.toml','build.rs','LICENSE','LICENSES.md','NOTICE','REUSE.toml',
  'README.md','CONTRIBUTING.md','SECURITY.md','CODE_OF_CONDUCT.md','CHANGELOG.md','THIRD_PARTY_NOTICES.md']);
const excludedSegments = new Set(['.git','target','node_modules','outputs','output','artifacts','.ssh','.codex','browser-profile','ui-profile','sessions','archived_sessions']);
const excludedName = /(?:^\.env(?:\.|$)|^\.npmrc$|^\.netrc$|^(?:auth|credentials|cookies|session|ssh-auto-connect)\.json$|\.(?:jsonl|log|har|dmp|exe|msi|msix|appx|pdb|pem|pfx|p12|key|zip)$|\.(?:sqlite(?:3)?|db)(?:-(?:wal|shm|journal))?$|^\.cargo-ok$)/i;
export function isPublishable(relative) {
  if (typeof relative !== 'string' || relative.includes('\\') || relative.includes('\0') || /^[A-Za-z]:/.test(relative)) return false;
  const parts = relative.split('/');
  if (parts.some(p=>!p || p==='.' || p==='..' || excludedSegments.has(p.toLowerCase()))) return false;
  if (parts.some(p=>excludedName.test(p))) return false;
  if (relative.startsWith('docs/vendor/openai/') && relative !== 'docs/vendor/openai/README.md') return false;
  if (relative.startsWith('docs/brand/') && relative !== 'docs/brand/README.md') return false;
  return parts.length === 1 ? files.has(relative) : roots.has(parts[0]);
}
export function listInputs(root) {
  const raw=execFileSync('git',['-C',root,'ls-files','--cached','--others','--exclude-standard','-z'],{maxBuffer:32*1024*1024});
  return [...new Set(raw.toString('utf8').split('\0').filter(Boolean))].filter(isPublishable).filter(p=>fs.existsSync(path.join(root,p))).sort();
}
export function assertNoLinks(root, file) {
  const base=path.resolve(root),full=path.resolve(file);
  if(full!==base&&!full.startsWith(base+path.sep))throw Error('Path escapes publication boundary');
  let current=full;
  while(true){if(fs.existsSync(current)&&fs.lstatSync(current).isSymbolicLink())throw Error(`Linked publication path: ${current}`);if(current===base)break;current=path.dirname(current);}
  return full;
}
export const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
export function copyInputs(root, destination, inputs) {
  assertNoLinks(root,root);
  fs.mkdirSync(destination,{recursive:true});
  const records=[];
  for(const relative of inputs){
    if(!isPublishable(relative))throw Error('Excluded publication input');
    const source=assertNoLinks(root,path.join(root,relative));
    if(!fs.statSync(source).isFile())throw Error('Publication inputs must be files');
    const content=fs.readFileSync(source);
    if(content.length>20*1024*1024)throw Error(`Oversized publication input: ${relative}`);
    assertPublicationPrivacy(relative,content);
    const target=assertNoLinks(destination,path.join(destination,relative));
    fs.mkdirSync(path.dirname(target),{recursive:true});
    fs.writeFileSync(target,content,{flag:'wx'});
    records.push({path:relative,bytes:content.length,sha256:sha256(content)});
  }
  return records;
}
