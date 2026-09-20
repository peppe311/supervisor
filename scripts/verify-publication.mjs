// SPDX-License-Identifier: MPL-2.0
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';
import { isPublishable, assertNoLinks } from './publication-files.mjs';
import { assertPublicationPrivacy } from './publication-privacy.mjs';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const tracked=execFileSync('git',['-C',root,'ls-files','-z'],{maxBuffer:32*1024*1024}).toString().split('\0').filter(Boolean);
const required=['README.md','LICENSE','NOTICE','LICENSES.md','CONTRIBUTING.md','SECURITY.md','CODE_OF_CONDUCT.md','CHANGELOG.md',
  'licenses/dependencies.json','THIRD_PARTY_NOTICES.md','.github/workflows/ci.yml','.github/workflows/release.yml','.github/dependabot.yml'];
for(const file of required)if(!tracked.includes(file))throw Error(`Missing publication file: ${file}`);
for(const file of tracked){
  if(!isPublishable(file))throw Error(`Excluded file is tracked in the publication: ${file}`);
  const absolute=assertNoLinks(root,path.join(root,file));
  if(!fs.statSync(absolute).isFile())throw Error('Non-file publication entry');
  assertPublicationPrivacy(file,fs.readFileSync(absolute));
}
console.log(`Public source layout verified: ${tracked.length} files; no excluded publication paths.`);
