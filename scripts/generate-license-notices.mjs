// SPDX-License-Identifier: MPL-2.0
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { selectLicense } from './license-policy.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const [allPath, windowsPath, mode] = process.argv.slice(2);
if (!allPath || !windowsPath || !['write', 'check'].includes(mode)) throw Error('Use update-licenses.ps1');
const read = f => fs.readFileSync(f, 'utf8').replace(/^\uFEFF/, '').replaceAll('\r\n', '\n');
const json = f => JSON.parse(read(f));
const hash = s => createHash('sha256').update(s).digest('hex');
const cargo = json(allPath), windows = json(windowsPath);
const active = new Set(windows.resolve.nodes.map(n => n.id));
const members = new Set(cargo.workspace_members);
const fallbacks = json(path.join(root, 'licenses/upstream.json'));
const texts = new Map(), packages = [];
const namePattern = /^(licen[cs]es?|copying|copyright|notice|unlicen[cs]e|authors|ofl)([._-]|$)/i;
const sort = (a, b) => a < b ? -1 : a > b ? 1 : 0;

function textId(file) {
  const text = read(file).trim() + '\n';
  if (text.length < 60 || text.includes('\0')) throw Error(`Invalid license text: ${file}`);
  const id = hash(text); texts.set(id, text); return id;
}
function legalFiles(directory, inLegalDirectory = false) {
  const files = [];
  for (const item of fs.readdirSync(directory, { withFileTypes: true })) {
    if (item.isSymbolicLink()) continue;
    const file = path.join(directory, item.name);
    if (item.isDirectory()) {
      if (!['.git', 'node_modules', 'target', 'testdata', 'tests', 'examples'].includes(item.name)) {
        files.push(...legalFiles(file, inLegalDirectory || namePattern.test(item.name)));
      }
    } else if (item.isFile() && (inLegalDirectory || namePattern.test(item.name)) && !/\.(rs|c|h|js|ts|json|png)$/i.test(item.name)) {
      if (fs.statSync(file).size >= 60) files.push(file);
    }
  }
  return files;
}
function fallbackTexts(key) {
  return (fallbacks[key] ?? []).map(entry => {
    const file = path.resolve(root, entry.file);
    if (!file.startsWith(path.join(root, 'licenses') + path.sep)) throw Error('Unsafe license fallback');
    if (hash(read(file).trim() + '\n') !== entry.sha256) throw Error(`Changed upstream notice: ${key}`);
    return textId(file);
  });
}
function add(record, files, extra = []) {
  record.selectedLicense = selectLicense(record.declaredLicense);
  record.notices = [...new Set([...files.map(textId), ...extra, ...fallbackTexts(record.id)])].sort(sort);
  if (record.required && !record.notices.length) throw Error(`Missing notice for ${record.id}`);
  record.coverage = record.notices.length ? 'notice-texts-collected' : 'metadata-only-not-shipped-on-windows';
  packages.push(record);
}

for (const p of cargo.packages) {
  if (members.has(p.id)) continue;
  const dir = path.dirname(p.manifest_path);
  const files = legalFiles(dir);
  if (p.license_file) files.push(path.resolve(dir, p.license_file));
  // Upstream selectors identifies MPL-2.0 in source headers without bundling
  // the standard text. Retain the complete standard text already in LICENSE.
  if (p.name === 'selectors' && p.license === 'MPL-2.0') files.push(path.join(root, 'LICENSE'));
  add({ id: `cargo:${p.name}@${p.version}`, name: p.name, version: p.version,
    ecosystem: 'cargo', declaredLicense: p.license, repository: p.repository,
    source: p.source ?? 'vendored', required: active.has(p.id), scope: active.has(p.id) ? 'windows-build' : 'other-cargo-target' }, files);
}
const lock = json(path.join(root, 'ui/package-lock.json'));
for (const [relative, p] of Object.entries(lock.packages)) {
  if (!relative) continue;
  const name = relative.slice(relative.lastIndexOf('node_modules/') + 'node_modules/'.length);
  const supported = (!p.os || p.os.includes('win32')) && (!p.cpu || p.cpu.includes('x64'));
  const dir = path.join(root, 'ui', relative);
  let files = [];
  if (supported) {
    if (!fs.existsSync(dir)) throw Error(`Run npm ci: missing ${name}`);
    if (json(path.join(dir, 'package.json')).version !== p.version) throw Error(`Installed version mismatch: ${name}`);
    files = legalFiles(dir);
    if (['is-reference', 'locate-character'].includes(name)) files.push(path.join(root, 'licenses/standard-notices.txt'));
    if (name === '@rolldown/binding-win32-x64-msvc') {
      const parent = json(path.join(root, 'ui/node_modules/rolldown/package.json'));
      if (parent.version !== p.version) throw Error('Review rolldown binding notices');
      files.push(...legalFiles(path.join(root, 'ui/node_modules/rolldown')));
    }
  } else if (!p.optional || !p.dev) throw Error(`Unexpected unavailable npm runtime: ${name}`);
  add({ id: `npm:${name}@${p.version}`, name, version: p.version, ecosystem: 'npm',
    declaredLicense: p.license, source: p.resolved, integrity: p.integrity,
    required: supported, scope: !supported ? 'optional-other-platform-build-tool' : p.dev ? 'frontend-build' : 'frontend-runtime' }, files);
}
for (const font of ['inter', 'manrope']) add({id:`asset:font-${font}`, name:font, ecosystem:'asset', declaredLicense:'OFL-1.1', required:true, scope:'bundled-font'}, [path.join(root, `assets/fonts/${font}/OFL.txt`)]);
add({id:'asset:codex-app-server-contracts', name:'OpenAI Codex generated contracts', ecosystem:'asset', declaredLicense:'Apache-2.0', required:true, scope:'generated-contracts', source:'https://github.com/openai/codex'}, legalFiles(path.join(root, 'protocol/app-server')));
add({id:'asset:simple-icons@16.21.0', name:'Simple Icons (16.21.0 and 12.4.0 paths)', ecosystem:'asset', declaredLicense:'CC0-1.0', required:true, scope:'bundled-icons', source:'https://github.com/simple-icons/simple-icons'}, []);
packages.sort((a,b)=>sort(a.id,b.id));
const inventory = {
  schema:'supervisor.license-inventory', schemaVersion:1,
  target:'x86_64-pc-windows-msvc', npmPlatform:'win32-x64',
  lockHashes: { cargo:hash(read(path.join(root,'Cargo.lock'))), npm:hash(read(path.join(root,'ui/package-lock.json'))) },
  limitation:'Collected notices and metadata, not a legal certification. Other-platform dependencies are not Windows distribution inputs.',
  packages,
};
const table = packages.map(p=>`| ${p.id} | ${p.declaredLicense} | ${p.scope} | ${p.notices.map(id=>`[${id.slice(0,12)}](#notice-${id})`).join(', ') || 'metadata only'} |`).join('\n');
const appendix = [...texts].sort(([a],[b])=>sort(a,b)).map(([id,text])=>`<a id="notice-${id}"></a>\n\n## Notice ${id.slice(0,12)}\n\n\`\`\`text\n${text}\`\`\`\n`).join('\n');
const markdown = `# Third-party notices\n\nGenerated by scripts/update-licenses.ps1 from the locked dependencies.\nDo not edit the generated sections by hand. See LICENSES.md and licenses/dependencies.json.\n\nSupervisor is independent of OpenAI and other integration providers. Third-party\ntrademarks belong to their owners. SF Pro, provider runtimes and official desktop\nplugins are not bundled. Fonts retain OFL-1.1; Simple Icons retains CC0-1.0.\n\nOriginal Supervisor source is MPL-2.0. Corresponding source and vendored patches:\nhttps://github.com/peppe311/supervisor (use the release's matching tag/commit).\nMPL-covered dependency sources are available at the exact versions through\nhttps://crates.io and https://registry.npmjs.org, as recorded in the inventory.\n\nThe table includes build tools and other-platform dependencies for transparency.\nOnly the supported Windows dependency set is required for this notice coverage\ncheck. Text collection does not replace redistribution review or source offers.\n\n| Component | Declared license | Scope | Notice text |\n| --- | --- | --- | --- |\n${table}\n\n${appendix}`;
for (const [relative,content] of [['licenses/dependencies.json',JSON.stringify(inventory,null,2)+'\n'],['THIRD_PARTY_NOTICES.md',markdown]]) {
  const file=path.join(root,relative);
  if(mode==='check') { if(!fs.existsSync(file)||read(file)!==content) throw Error(`Notices are stale: ${relative}; run scripts/update-licenses.ps1`); }
  else fs.writeFileSync(file,content);
}
console.log(`License coverage: ${packages.length} components, ${packages.filter(p=>p.required).length} required, ${texts.size} distinct notices; ${mode} passed.`);
