import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = resolve(fileURLToPath(import.meta.url), '..', '..', '..');
const SKIP_DIRS = new Set(['.git', 'target', 'node_modules', 'repos', 'artifacts', 'book']);
const SKIP_FILES = new Set(['docs/en/GLOSSARY.md', 'docs/zh/GLOSSARY.md', 'docs/ja/GLOSSARY.md', 'docs/de/GLOSSARY.md']);

function* markdownFiles(dir) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (entry.name.startsWith('.') && entry.name !== '.github') continue;
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (SKIP_DIRS.has(entry.name)) continue;
      yield* markdownFiles(full);
    } else if (entry.name.endsWith('.md')) {
      if (statSync(full).size > 4 * 1024 * 1024) continue;
      yield full;
    }
  }
}


const LINK = /!?\[[^\]]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)/g;
const IMG_SRC = /<img[^>]+src="([^"]+)"/g;
const SUMMARY = /\[[^\]]*\]\(([^)]+)\)/g;

let broken = 0;
let checked = 0;

for (const file of markdownFiles(repoRoot)) {
  const rel = relative(repoRoot, file).replace(/\\/g, '/');
  if (SKIP_FILES.has(rel)) continue;
  const text = readFileSync(file, 'utf8');
  const base = dirname(file);

  const targets = [
    ...[...text.matchAll(LINK)].map((m) => m[1]),
    ...[...text.matchAll(IMG_SRC)].map((m) => m[1]),

    ...(rel === 'docs/SUMMARY.md' ? [...text.matchAll(SUMMARY)].map((m) => m[1]) : []),
  ];

  for (const raw of targets) {
    if (!raw) continue;
    if (/^(https?:|mailto:|#)/.test(raw)) continue;

    if (raw.includes('{{')) continue;
    const target = raw.split('#')[0];
    if (!target) continue;
    checked++;
    const resolvedPath = target.startsWith('/')
      ? join(repoRoot, target)
      : resolve(base, target);
    if (!existsSync(resolvedPath)) {
      const line = text.slice(0, text.indexOf(raw)).split('\n').length;
      console.error(`${rel}:${line}: ${raw} -> does not exist`);
      broken++;
    }
  }
}

if (broken > 0) {
  console.error(`\n${broken} broken link(s) out of ${checked} relative links`);
  process.exit(1);
}
console.log(`ok   ${checked} relative links resolve`);
