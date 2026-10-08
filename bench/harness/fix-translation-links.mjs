import { readFileSync, writeFileSync, readdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = resolve(fileURLToPath(import.meta.url), '..', '..', '..');
const dir = join(repoRoot, 'docs', 'en');

const PATTERN =
  /\[ZH\]\(\.\.\/zh\/[0-9a-z-]+\.md\) ·\r?\n?\[JA\]\(\.\.\/ja\/[0-9a-z-]+\.md\) · \[DE\]\(\.\.\/de\/[0-9a-z-]+\.md\)/g;
const REPLACEMENT =
  'Translations: per-document translations are still in progress. ' +
  '[ZH](../zh/README.md) · [JA](../ja/README.md) · [DE](../de/README.md)';

let changed = 0;
for (const name of readdirSync(dir)) {
  if (!name.endsWith('.md') || name === 'GLOSSARY.md') continue;
  const path = join(dir, name);
  const before = readFileSync(path, 'utf8');
  const after = before.replace(PATTERN, REPLACEMENT);
  if (after !== before) {
    writeFileSync(path, after, 'utf8');
    console.log(`updated docs/en/${name}`);
    changed++;
  }
}
console.log(`${changed} file(s) updated`);
