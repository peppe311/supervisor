// SPDX-License-Identifier: MPL-2.0
export const allowed = new Set([
  'MIT', 'Apache-2.0', 'Apache-2.0 WITH LLVM-exception', 'MPL-2.0',
  'Unicode-3.0', 'ISC', 'Zlib', 'BSD-1-Clause', 'BSD-2-Clause', 'BSD-3-Clause',
  '0BSD', 'CC0-1.0', 'Unlicense', 'BSL-1.0', 'MIT-0', 'OFL-1.1',
]);

// Resolve OR as a choice, AND as cumulative obligations. Unknown identifiers
// fail closed; legacy Cargo's slash separator means OR, not AND.
export function selectLicense(expression) {
  if (typeof expression !== 'string' || !expression.trim()) throw Error('Missing license');
  const tokens = expression.replaceAll('/', ' OR ').match(/\(|\)|[A-Za-z0-9.+-]+/g) ?? [];
  if (tokens.join('').replaceAll('OR', '') === '') throw Error('Invalid license');
  if (expression.replaceAll('/', ' OR ').replace(/\(|\)|[A-Za-z0-9.+-]+|\s/g, '')) throw Error('Invalid license syntax');
  let i = 0;
  function atom() {
    if (tokens[i] === '(') {
      i++; const value = or();
      if (tokens[i++] !== ')') throw Error('Unbalanced license expression');
      return value;
    }
    let id = tokens[i++];
    if (!id || ['OR', 'AND', 'WITH', ')'].includes(id)) throw Error('Invalid license identifier');
    if (tokens[i] === 'WITH') { i++; const exception = tokens[i++]; if (!exception) throw Error('Missing exception'); id += ` WITH ${exception}`; }
    return allowed.has(id) ? [id] : null;
  }
  function and() {
    let result = atom();
    while (tokens[i] === 'AND') { i++; const right = atom(); result = result && right ? [...result, ...right] : null; }
    return result;
  }
  function or() {
    let result = and();
    while (tokens[i] === 'OR') { i++; const right = and(); result = result ?? right; }
    return result;
  }
  const result = or();
  if (i !== tokens.length || !result) throw Error(`License needs review: ${expression}`);
  return [...new Set(result)].join(' AND ');
}
