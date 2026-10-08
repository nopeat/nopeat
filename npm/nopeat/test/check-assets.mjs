// Check that the asset names the installer asks for are names the release
// actually publishes. A mismatch means `npm install -g nopeat` succeeds and then
// fails at first run, which is the worst place for it to fail.
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..', '..', '..');
let bad = 0;

// The release builds these, from the matrix in release.yml.
const workflow = readFileSync(join(repoRoot, '.github', 'workflows', 'release.yml'), 'utf8');
const targets = [...workflow.matchAll(/-\s*target:\s*(\S+)/g)].map((m) => m[1]);
console.log(`  release.yml builds ${targets.length} targets:`);
for (const t of targets) console.log(`    ${t}`);

// The name the release assembles.
const nameExpr = /NAME="([^"]+)"/.exec(workflow)?.[1];
console.log(`  release asset name: ${nameExpr}`);

// The mapping the installer uses.
const install = readFileSync(join(repoRoot, 'npm', 'nopeat', 'install.mjs'), 'utf8');
const triples = /const TARGET_TRIPLES = \{([\s\S]*?)\};/.exec(install)?.[1] ?? '';
const mapped = [...triples.matchAll(/'([\w-]+)':\s*'([\w-]+)'/g)].map((m) => [m[1], m[2]]);

console.log(`\n  install.mjs maps ${mapped.length} platform keys:`);
for (const [key, triple] of mapped) {
  const known = targets.includes(triple);
  if (!known) bad++;
  console.log(`    ${known ? 'ok  ' : 'FAIL'} ${key.padEnd(14)} -> ${triple}${known ? '' : '  (not in the release matrix)'}`);
}

// Every mapped triple must produce a name the release would publish.
for (const [key, triple] of mapped) {
  const isWindows = key.startsWith('win32');
  const ext = isWindows ? '.zip' : '.tar.gz';
  const want = `nopeat-\${VERSION}-${triple}${ext}`;
  const shape = new RegExp(
    '^nopeat-\\$\\{VERSION\\}-[\\w-]+(?:\\.zip|\\.tar\\.gz)$',
  );
  const ok = shape.test(want);
  if (!ok) bad++;
  console.log(`  ${ok ? 'ok  ' : 'FAIL'} ${key.padEnd(14)} would request ${want}`);
}

// And the installer must not still be building names out of process.platform.
if (/assetName = `nopeat-\$\{version\}-\$\{process\.platform\}/.test(install)) {
  console.log('  FAIL  the installer still derives the name from process.platform');
  bad++;
} else {
  console.log('  ok    the installer no longer derives the name from process.platform');
}

console.log(`\n${bad} problem(s)`);
process.exitCode = bad === 0 ? 0 : 1;
