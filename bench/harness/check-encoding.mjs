import { readFileSync, readdirSync, statSync } from 'node:fs';
import { extname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = resolve(fileURLToPath(import.meta.url), '..', '..', '..');

const SKIP_DIRS = new Set([
  '.git', 'target', 'node_modules', 'artifacts', 'vendor', 'coverage', 'repos',
]);

const SKIP_PREFIXES = ['book/book'];

const ALLOW = new Set(['bench/harness/check-encoding.mjs']);
const EXTENSIONS = new Set([
  '.rs', '.md', '.mjs', '.js', '.json', '.toml', '.yml', '.yaml', '.html',
  '.css', '.sh', '.ps1', '.txt', '.svg', '.gitignore', '.gitattributes',
]);


const SUSPECT = [
  '\u00EF\u00BF\u00BD',
  '\u00E2\u20AC\u201C',
  '\u00E2\u20AC',
  '\u00C3\u00A4',
  '\u00E3\u20AC',
  '\u00E5\u00AE',
  '\u00E7\u0161',
  '\u00E9\u0192',
];

function* walk(dir) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (entry.name.startsWith('.') && entry.name !== '.github') continue;
    const full = join(dir, entry.name);
    if (entry.isDirectory()) {
      if (SKIP_DIRS.has(entry.name)) continue;
      yield* walk(full);
    } else if (EXTENSIONS.has(extname(entry.name)) || entry.name.startsWith('.')) {
      if (statSync(full).size > 2 * 1024 * 1024) continue;
      yield full;
    }
  }
}

const problems = [];
let scanned = 0;

for (const file of walk(repoRoot)) {
  const rel = relative(repoRoot, file).replace(/\\/g, '/');
  if (ALLOW.has(rel)) continue;
  if (SKIP_PREFIXES.some((prefix) => rel.startsWith(prefix))) continue;
  scanned++;
  const bytes = readFileSync(file);
  const text = bytes.toString('utf8');

  if (bytes[0] === 0xef && bytes[1] === 0xbb && bytes[2] === 0xbf) {
    problems.push(`${rel}: has a UTF-8 BOM`);
  }
  if (text.includes('\uFFFD')) {
    const line = text.slice(0, text.indexOf('\uFFFD')).split('\n').length;
    problems.push(`${rel}:${line}: U+FFFD replacement character`);
  }
  for (let i = 0; i < text.length; i++) {
    const code = text.charCodeAt(i);

    if (code >= 0x80 && code <= 0x9f) {
      problems.push(`${rel}: C1 control U+${code.toString(16).padStart(4, '0')}`);
      break;
    }
  }
  for (let i = 0; i < text.length; i++) {
    const code = text.charCodeAt(i);
    if (code >= 0x20 || code === 0x09 || code === 0x0a || code === 0x0d) continue;
    const line = text.slice(0, i).split('\n').length;
    problems.push(
      `${rel}:${line}: control character U+${code.toString(16).padStart(4, '0')} ` +
        `(escaped: ${JSON.stringify(text.slice(Math.max(0, i - 30), i + 10))})`,
    );
    break;
  }
  for (const suspect of SUSPECT) {
    if (text.includes(suspect)) {
      const line = text.slice(0, text.indexOf(suspect)).split('\n').length;
      problems.push(`${rel}:${line}: mojibake fragment ${JSON.stringify(suspect)}`);
    }
  }
}

if (problems.length === 0) {
  console.log(`ok  ${scanned} files, no encoding damage`);
  process.exit(0);
}
console.error(`${problems.length} encoding problem(s):`);
for (const p of problems.slice(0, 40)) console.error(`  ${p}`);
process.exit(1);