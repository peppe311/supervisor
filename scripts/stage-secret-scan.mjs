// SPDX-License-Identifier: MPL-2.0
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { assertNoLinks, copyInputs, listInputs } from './publication-files.mjs';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const destination=path.resolve(process.argv[2] ?? '');
const sessions=path.join(root,'target/.supervisor-build/sessions')+path.sep;
if(!destination.startsWith(sessions))throw Error('Secret scan staging must use an owned build session');
assertNoLinks(root,destination);
const files=copyInputs(root,destination,listInputs(root));
console.log(`Scanning ${files.length} current publication inputs, including eligible untracked sources.`);
