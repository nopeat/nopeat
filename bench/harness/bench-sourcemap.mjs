import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, '..', '..');
const exeSuffix = process.platform === 'win32' ? '.exe' : '';
const binary = process.env.OB_BINARY ?? join(repoRoot, 'target', 'release', `nopeat${exeSuffix}`);
const resultsDir = join(repoRoot, 'bench', 'results');

const inputs = process.argv.slice(2);
if (inputs.length === 0) {
  console.error('usage: node bench-sourcemap.mjs <map.json> […]');
  process.exit(2);
}
if (!existsSync(binary)) {
  console.error(`binary not found: ${binary} (cargo build --release first)`);
  process.exit(2);
}

mkdirSync(resultsDir, { recursive: true });
const rows = [];

for (const input of inputs) {
  const size = statSync(input).size;
  const sources = (() => {
    try {
      return JSON.parse(readFileSync(input, 'utf8')).sources?.length ?? null;
    } catch {
      return null;
    }
  })();

  const walls = [];
  let payload = null;
  for (let i = 0; i < 3; i++) {
    const t0 = Date.now();
    const r = spawnSync(binary, [input, '--bench-map'], { encoding: 'utf8' });
    walls.push(Date.now() - t0);
    if (i === 0 && r.status !== 0) {
      console.error(`run failed for ${input}: ${r.stderr}`);
      process.exit(3);
    }
    payload = r.stdout.trim();
  }
  walls.sort((a, b) => a - b);
  const median = walls[1];
  const detail = JSON.parse(payload || '{}');

  rows.push({
    fixture: input.split(/[\\/]/).pop(),
    fixture_class: input.includes('artifacts') ? 'real' : 'synthetic',
    tool: 'nopeat@0.1.0 --bench-map',
    input_bytes: size,
    sources,
    mappings: detail.mappings ?? null,
    attributed_total: detail.attributed_total ?? null,
    wall_ms_runs: walls,
    wall_ms_median: median,
  });
}

const out = join(resultsDir, `ws3-sourcemap-${Date.now()}.json`);
writeFileSync(out, `${JSON.stringify({ measurement: 'source maps source map ingest', rows }, null, 2)}\n`);
for (const r of rows) {
  console.log(
    `${r.fixture}: ${r.sources} sources, ${r.mappings} mappings, median ${r.wall_ms_median} ms`,
  );
}
console.log(`\nwritten: ${out}`);
