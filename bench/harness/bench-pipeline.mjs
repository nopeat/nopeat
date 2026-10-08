import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, '..', '..');
const exeSuffix = process.platform === 'win32' ? '.exe' : '';
const binary = process.env.OB_BINARY ?? join(repoRoot, 'target', 'release', `nopeat${exeSuffix}`);
const resultsDir = join(repoRoot, 'bench', 'results');

const arg = (flag, fallback = null) => {
  const i = process.argv.indexOf(flag);
  return i > -1 ? process.argv[i + 1] : fallback;
};
const has = (flag) => process.argv.includes(flag);

const stats = arg('--stats');
const dir = arg('--dir');
const withMaps = has('--maps');
const runs = Number(arg('--runs', '3'));

if (!stats || !existsSync(binary)) {
  console.error('usage: node bench-pipeline.mjs --stats <stats.json> [--dir <dir>] [--maps] [--runs 3]');
  process.exit(2);
}

function runOnce() {
  const args = [dir ?? stats, '--mode', 'static', '--report', resultsDir + '/bench-report.html'];
  if (dir) {

  } else {
    args[0] = stats;
    args.push('--bench');
  }
  const t0 = Date.now();
  const r = spawnSync(binary, args, { encoding: 'utf8' });
  return { wall_ms: Date.now() - t0, stdout: r.stdout ?? '', status: r.status };
}

const samples = [];
for (let i = 0; i < runs; i++) {
  const s = runOnce();
  samples.push(s.wall_ms);
  if (i === 0) {
    const line = s.stdout
      .split('\n')
      .map((l) => l.trim())
      .filter(Boolean)
      .find((l) => l.startsWith('{') || l.includes('modules')) ?? s.stdout;
    console.log(line);
  }
}
samples.sort((a, b) => a - b);
const median = samples[Math.floor(samples.length / 2)];

const out = {
  measurement: 'sizes full pipeline',
  input: { stats, dir: dir ?? null, maps_fused: withMaps, bytes: statSync(stats).size },
  runs: samples,
  wall_ms_median: median,
  phase_breakdown:
    dir === null
      ? { scan: null, parse: 'reported in the CLI line', attribute: null, fuse: null, report: null }
      : { note: 'the CLI reports ingest and total; attribute/fuse/report are inside the total' },
  targets: {
    B3: dir === null ? 'not applicable (no bundle directory: ingest only)' : '<= 5000 ms, <= 200 MB at 381 MB',
    B4: dir === null ? 'not applicable' : '<= 15000 ms, <= 400 MB at 1 GB',
    B8: withMaps ? '< 500 MB peak on 1 GB stats + 50 MB map' : 'maps not included in this run',
  },
  honest_gaps: [
    dir === null ? 'no bundle directory, so asset measurement and fusion did not run' : null,
    'peak RSS is not sampled by this script; use measure-rss.sh for that',
  ].filter(Boolean),
};

mkdirSync(resultsDir, { recursive: true });
const file = join(resultsDir, `ws2-pipeline-${Date.now()}.json`);
writeFileSync(file, `${JSON.stringify(out, null, 2)}\n`);
console.log(`\nmedian ${median} ms — written to ${file}`);
