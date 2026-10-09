import { readFileSync, existsSync } from 'node:fs';
import { join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { load } from 'js-yaml';

const repoRoot = resolve(fileURLToPath(import.meta.url), '..', '..', '..');

const FILES = [
  '.github/workflows/ci.yml',
  '.github/workflows/pages.yml',
  '.github/workflows/release.yml',
  '.github/labeler.yml',
  '.github/stale.yml',
  '.github/dependabot.yml',
  '.github/ISSUE_TEMPLATE/config.yml',
];

const EXPECTED = [
  { file: '.github/workflows/ci.yml', key: 'on' },
  { file: '.github/workflows/pages.yml', key: 'on' },
  { file: '.github/workflows/release.yml', key: 'on' },
];

let failures = 0;

for (const rel of FILES) {
  const path = join(repoRoot, rel);
  if (!existsSync(path)) {
    console.error(`FAIL ${rel}: missing`);
    failures++;
    continue;
  }
  const text = readFileSync(path, 'utf8');

  if (/^\t/m.test(text)) {
    console.error(`FAIL ${rel}: a literal tab (YAML forbids it for indentation)`);
    failures++;
    continue;
  }
  for (const [i, line] of text.split('\n').entries()) {
    if (/^\s*-?\s*name:.*[^^]\S:\s/.test(line) && !/["']/.test(line.split('name:')[1] ?? '')) {
      console.error(`FAIL ${relative(repoRoot, path)}:${i + 1}: an unquoted ": " in a name`);
      failures++;
    }
  }

  let doc;
  try {
    doc = load(text);
  } catch (err) {
    console.error(`FAIL ${rel}: ${String(err.message).split('\n')[0]}`);
    failures++;
    continue;
  }

  const expected = EXPECTED.find((e) => e.file === rel);
  if (expected && !(expected.key in (doc ?? {})) && !('true' in (doc ?? {}))) {
    console.error(`FAIL ${rel}: no top-level \`${expected.key}\` key`);
    failures++;
    continue;
  }

  const jobs = Object.keys(doc?.jobs ?? {});

  if (rel.startsWith('.github/workflows/') && jobs.length === 0) {
    console.error(`FAIL ${rel}: no jobs`);
    failures++;
    continue;
  }
  console.log(
    jobs.length > 0
      ? `ok   ${rel} (jobs: ${jobs.join(', ')})`
      : `ok   ${rel} (${Object.keys(doc ?? {}).join(', ') || 'empty'})`,
  );
}

if (failures > 0) {
  console.error(`${failures} workflow problem(s)`);
  process.exit(1);
}
console.log('ok   all workflow files parse and declare jobs');