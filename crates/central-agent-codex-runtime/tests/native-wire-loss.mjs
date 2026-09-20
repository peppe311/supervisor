// Opt-in test relay, never shipped or used as a production Codex transport.
// Forward the official server byte-for-byte until the second turn/start. Then
// lose either that request or its entire return stream, including its wire ACK.
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { writeFileSync } from 'node:fs';
import { isAbsolute } from 'node:path';

const [executable, marker, mode, lossOrdinalText = '2'] = process.argv.slice(2);
const lossOrdinal = Number(lossOrdinalText);
if (!isAbsolute(executable) || !isAbsolute(marker) || !['before', 'after'].includes(mode)
    || !Number.isSafeInteger(lossOrdinal) || lossOrdinal < 1) {
  throw new Error('Invalid owned wire-loss fixture arguments');
}
const native = spawn(executable, ['app-server', '--listen', 'stdio://'], {
  stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true,
});
let starts = 0;
let lostId;
let lostTurn;
let acknowledged = false;
let suppress = false;
function mark(phase) {
  // Fixed evidence only: never store prompts, headers, credentials or history.
  writeFileSync(marker, JSON.stringify({ phase, starts, acknowledged, nativePid: native.pid }));
}
native.stderr.resume();
native.on('error', () => { mark('spawn-error'); process.exit(1); });
native.stdin.on('error', () => { mark('input-error'); process.exit(1); });
native.on('exit', () => { process.stdout.end(); process.exitCode = 0; });
createInterface({ input: process.stdin }).on('line', line => {
  const value = JSON.parse(line);
  if (value.method === 'turn/start' && ++starts === lossOrdinal) {
    suppress = true;
    lostId = value.id;
    if (mode === 'before') { mark('request-dropped'); return; }
  }
  native.stdin.write(line + '\n');
}).on('close', () => native.stdin.end());
createInterface({ input: native.stdout }).on('line', line => {
  if (!suppress) { process.stdout.write(line + '\n'); return; }
  const value = JSON.parse(line);
  if (value.id === lostId) {
    acknowledged = !!value.result?.turn?.id;
    lostTurn = value.result?.turn?.id;
    if (!acknowledged) mark('native-rejected');
  }
  if (value.method === 'turn/completed' && value.params?.turn?.id === lostTurn) {
    mark(value.params.turn.status === 'completed' ? 'completed-with-ack-lost' : 'native-failed');
  }
});
