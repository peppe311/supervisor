// Download only public documentation; no API key, account, or inference request.
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile, rename } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const origin = 'https://developers.openai.com';
const guidePrefix = '/api/docs/guides/agents-api/';
const referencePrefix = '/api/reference/typescript/resources/beta/subresources/agents';
const streamingPath = '/api/reference/resources/beta/subresources/agents/streaming-events';
const repository = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const date = new Date().toISOString().slice(0, 10);
const destination = path.join(repository, 'target/reference-docs/agents-api', date);
const sha256 = (value) => createHash('sha256').update(value).digest('hex');
const entries = new Map();

async function get(url, markdown = true) {
  const source = new URL(url, origin);
  if (source.origin !== origin) throw new Error(`Unexpected documentation origin: ${source.origin}`);
  const response = await fetch(source, { redirect: 'error', signal: AbortSignal.timeout(30000) });
  if (!response.ok) throw new Error(`Documentation returned ${response.status}: ${source}`);
  const bytes = Buffer.from(await response.arrayBuffer());
  if (bytes.length < 40 || bytes.length > 8 * 1024 * 1024) throw new Error(`Invalid document size: ${source}`);
  const text = bytes.toString('utf8');
  if (markdown && (!/^# /m.test(text) && !/^## /m.test(text) || /^\s*<!doctype html/i.test(text))) {
    throw new Error(`Expected Markdown: ${source}`);
  }
  return { bytes, text, source: source.href, retrievedAtUtc: new Date().toISOString() };
}

function add(source, local, group) {
  const url = new URL(source, origin);
  if (url.origin !== origin) throw new Error('Unexpected source origin');
  const resolved = path.resolve(destination, local);
  if (!resolved.startsWith(destination + path.sep)) throw new Error('Document path escaped its destination');
  const existing = entries.get(url.href);
  if (existing && existing.local !== local) throw new Error(`Conflicting local path: ${source}`);
  if ([...entries.values()].some(e => e.local === local && e.source !== url.href)) throw new Error(`Duplicate destination: ${local}`);
  entries.set(url.href, { source: url.href, local, group });
}

async function save(local, bytes) {
  const target = path.resolve(destination, local);
  if (!target.startsWith(destination + path.sep)) throw new Error('Output path escaped its destination');
  await mkdir(path.dirname(target), { recursive: true });
  try { if ((await readFile(target)).equals(bytes)) return; } catch (error) { if (error.code !== 'ENOENT') throw error; }
  const temporary = `${target}.download-${process.pid}`;
  await writeFile(temporary, bytes, { flag: 'wx' });
  await rename(temporary, target);
}

const indexUrl = `${origin}/api/docs/llms.txt`;
const navigationUrl = `${origin}${referencePrefix}/methods/create`;
const discovery = await Promise.allSettled([get(indexUrl), get(navigationUrl, false)]);
for (const result of discovery) if (result.status === 'rejected') throw result.reason;
const [guideIndex, navigation] = discovery.map(result => result.value);

for (const match of guideIndex.text.matchAll(/\]\((https:\/\/developers\.openai\.com\/[^)]+)\)/g)) {
  const source = new URL(match[1]);
  if (source.pathname.startsWith(guidePrefix) && source.pathname.endsWith('.md')) {
    add(source.href, `guides/${source.pathname.slice(guidePrefix.length)}`, 'guide');
  }
}
for (const match of navigation.text.matchAll(/href="([^"]+)"/g)) {
  const source = new URL(match[1].replaceAll('&amp;', '&'), origin);
  if (source.origin !== origin) continue;
  const pathname = source.pathname.replace(/\/$/, '');
  if (pathname !== referencePrefix && !pathname.startsWith(referencePrefix + '/') && pathname !== streamingPath) continue;
  if (pathname.endsWith('/index.md')) continue;
  const stem = pathname.replace(/\.md$/, '');
  const suffix = stem === streamingPath ? '/streaming-events' : stem.slice(referencePrefix.length);
  const shortPath = suffix.replaceAll('/subresources/', '/').replaceAll('/methods/', '/');
  add(`${origin}${stem}.md`, `reference${shortPath || '/agents'}.md`, 'reference');
}

const supplementary = [
  ['/api/docs/changelog.md', 'context/changelog.md'],
  ['/api/docs/guides/agents.md', 'context/agents-runtimes.md'],
  ['/api/docs/guides/responses-multi-agent.md', 'context/responses-multi-agent.md'],
  ['/api/docs/pricing.md', 'context/pricing.md'],
  ['/api/docs/guides/tools-programmatic-tool-calling.md', 'context/programmatic-tool-calling.md'],
  ['/api/docs/guides/tools-tool-search.md', 'context/tool-search.md'],
  ['/api/docs/guides/tools-skills.md', 'context/skills.md'],
  ['/api/docs/guides/your-data.md', 'context/data-controls.md'],
];
for (const [source, local] of supplementary) add(source, local, 'context');
const list = [...entries.values()];
if (list.filter(e => e.group === 'guide').length < 20 || list.filter(e => e.group === 'reference').length < 35 || list.length > 150) {
  throw new Error('Documentation discovery is incomplete or unexpectedly broad');
}
for (const local of ['guides/overview.md', 'guides/sessions/events.md', 'guides/multi-agent.md', 'reference/agents.md', 'reference/sessions/create.md', 'reference/sessions/events/create.md', 'reference/sessions/events/stream.md', 'reference/streaming-events.md']) {
  if (!list.some(e => e.local === local)) throw new Error(`Missing essential documentation: ${local}`);
}

const results = [];
for (let offset = 0; offset < list.length; offset += 4) {
  const batch = list.slice(offset, offset + 4);
  const downloads = await Promise.allSettled(batch.map(entry => get(entry.source)));
  const failures = downloads.filter(result => result.status === 'rejected');
  if (failures.length) throw new AggregateError(failures.map(result => result.reason), 'Documentation download failed');
  for (let i = 0; i < batch.length; i++) {
    const document = downloads[i].value;
    await save(batch[i].local, document.bytes);
    results.push({ ...batch[i], title: document.text.match(/^#{1,2}\s+(.+)$/m)?.[1] ?? batch[i].local, bytes: document.bytes.length, sha256: sha256(document.bytes), retrievedAtUtc: document.retrievedAtUtc });
  }
  console.log(`Documentation saved: ${results.length}/${list.length}`);
}
await save('sources/guides-index.md', guideIndex.bytes);
const manifest = {
  generatedAtUtc: new Date().toISOString(),
  scope: 'All Agents API guides in the official index, including sandbox providers; every Agents API endpoint linked by the official TypeScript reference navigation, its expanded schema page and streaming reference. Supporting comparison, changelog, pricing, shared tool and data-control pages included. Other SDK language renderings and image binaries are not duplicated.',
  discovery: [{ source: indexUrl, sha256: sha256(guideIndex.bytes) }, { source: navigationUrl, sha256: sha256(navigation.bytes) }],
  counts: Object.fromEntries(['guide', 'reference', 'context'].map(group => [group, results.filter(e => e.group === group).length])),
  documents: results.sort((a, b) => a.local.localeCompare(b.local)),
};
await save('manifest.json', Buffer.from(JSON.stringify(manifest, null, 2) + '\n'));
const lines = [
  '# OpenAI Agents API — documentazione ufficiale locale', '',
  `Snapshot: ${date}. Testi Markdown originali, scaricati senza riscriverli.`, '',
  'Fonte: documentazione ufficiale OpenAI. Ogni file conserva i collegamenti originali; immagini e collegamenti esterni richiedono la rete. I diritti rimangono dei rispettivi titolari.', '',
  'Copertura: tutte le guide Agents API indicizzate, inclusi i provider di sandbox; tutti gli endpoint nella navigazione ufficiale TypeScript, schemi espansi e streaming. Le rappresentazioni equivalenti negli altri linguaggi non sono duplicate.', '',
  'Il manifest registra URL, data di acquisizione, dimensione e SHA-256 di ogni documento. Lo snapshot descrive una beta e non certifica accesso, modelli abilitati o comportamento del proprio account.', '',
  'Aggiornamento dalla radice del repository: `node scripts/sync-openai-agents-docs.mjs`.', '',
];
for (const [group, title] of [['guide', 'Guide'], ['reference', 'Riferimento API e schemi'], ['context', 'Contesto e confronto']]) {
  lines.push(`## ${title}`, '', '| Documento locale | Fonte |', '| --- | --- |');
  for (const entry of manifest.documents.filter(e => e.group === group)) lines.push(`| [${entry.title.replaceAll('|', '\\|')}](${entry.local}) | [OpenAI](${entry.source}) |`);
  lines.push('');
}
await save('README.md', Buffer.from(lines.join('\n') + '\n'));
console.log(JSON.stringify({ destination, counts: manifest.counts, bytes: results.reduce((n, e) => n + e.bytes, 0) }));
