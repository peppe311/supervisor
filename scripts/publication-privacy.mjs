// SPDX-License-Identifier: MPL-2.0
// Secret scanners do not detect conversation metadata or personal machine paths.
// These checks supplement manual review; arbitrary prose still needs review.
export function privacyFindings(bytes) {
  if (bytes.includes(0)) return [];
  const text = bytes.toString('utf8');
  const findings = [];
  const record = (kind, index) => findings.push({ kind, line: text.slice(0, index).split('\n').length });
  for (const match of text.matchAll(/\b(?!00000000-)[a-f0-9]{8}-[a-f0-9]{4}-7[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}\b/gi)) {
    record('Non-synthetic native conversation identifier', match.index);
  }
  for (const match of text.matchAll(/[a-z]:[\\/]+Users[\\/]+([^\\/"'\s<>]+)/gi)) {
    if (!['fixture', 'example', 'you', 'public', 'default'].includes(match[1].toLowerCase())) {
      record('Personal Windows profile path', match.index);
    }
  }
  return findings;
}

export function assertPublicationPrivacy(file, bytes) {
  const findings = privacyFindings(bytes);
  if (findings.length) {
    // Do not echo the matched private text in CI output.
    throw Error(`Publication privacy check failed: ${file}: ${findings.map(f => `${f.kind} at line ${f.line}`).join('; ')}`);
  }
}
