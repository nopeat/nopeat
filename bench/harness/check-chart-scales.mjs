import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const data = JSON.parse(
  readFileSync(join(repoRoot, 'docs', 'assets', 'charts-data.json'), 'utf8'),
);

let failures = 0;
const fail = (msg) => {
  console.error(`  FAIL ${msg}`);
  failures++;
};


function barsOf(file, trackColour) {
  const svg = readFileSync(join(repoRoot, 'docs', 'assets', file), 'utf8');
  return [...svg.matchAll(/<rect x="[\d.]+" y="[\d.]+" width="([\d.]+)" height="([\d.]+)" fill="(#\w+)"/g)]
    .map((m) => ({ w: +m[1], h: +m[2], fill: m[3] }))
    .filter((b) => b.fill !== trackColour && b.h > 15);
}


const rows = Object.values(data.stats_pipeline);
const TRACK_LIGHT = '#f1f5f9';
for (const [variant, track] of [['light', TRACK_LIGHT], ['dark', '#182236']]) {
  const bars = barsOf(`chart-pipeline-${variant}.svg`, track);
  const expected = [
    ['wall clock ref', rows[0].ref_s],
    ['wall clock ours', rows[0].ob_s],
    ['wall clock ref', rows[1].ref_s],
    ['wall clock ours', rows[1].ob_s],
    ['memory ref', rows[0].ref_mb],
    ['memory ours', rows[0].ob_mb],
    ['memory ref', rows[1].ref_mb],
    ['memory ours', rows[1].ob_mb],
  ];
  if (bars.length !== expected.length) {
    fail(`pipeline-${variant}: expected ${expected.length} bars, found ${bars.length}`);
    continue;
  }


  const panel = (from, to, label) => {
    const units = [];
    for (let i = from; i < to; i++) units.push(bars[i].w / expected[i][1]);
    const spread = Math.max(...units) / Math.min(...units);

    if (spread - 1 > 0.02) {
      fail(
        `pipeline-${variant}: ${label} panel is not linear, px/unit varies by ${((spread - 1) * 100).toFixed(1)}%`,
      );
    }
  };
  panel(0, 4, 'wall clock');
  panel(4, 8, 'memory');


  const pairs = [
    [0, 1, rows[0].ref_s, rows[0].ob_s, 'wall clock'],
    [4, 5, rows[0].ref_mb, rows[0].ob_mb, 'memory'],
  ];
  for (const [a, b, va, vb, label] of pairs) {
    const drawn = bars[a].w / bars[b].w;
    const actual = va / vb;
    const off = Math.abs(drawn - actual) / actual;
    if (off > 0.05) {
      fail(
        `pipeline-${variant}: ${label} bar ratio drawn ${drawn.toFixed(2)}x, data says ${actual.toFixed(2)}x`,
      );
    }
  }
  console.log(
    `  ok   pipeline-${variant}: ${bars.length} bars, lengths proportional`,
  );
}


for (const file of [
  'chart-pipeline-light.svg',
  'chart-pipeline-dark.svg',
  'chart-source-maps-light.svg',
  'chart-source-maps-dark.svg',
  'chart-memory-budget-light.svg',
  'chart-memory-budget-dark.svg',
]) {
  const svg = readFileSync(join(repoRoot, 'docs', 'assets', file), 'utf8');
  const texts = (svg.match(/<text[\s>]/g) || []).length;
  if (texts < 10) fail(`${file}: only ${texts} text elements`);
  if (!svg.includes('<title')) fail(`${file}: no <title>`);
  if (!svg.includes('role="img"')) fail(`${file}: no role="img"`);
  console.log(`  ok   ${file}: ${texts} text elements, titled`);
}


for (const file of ['chart-source-maps-light.svg', 'chart-memory-budget-light.svg']) {
  const svg = readFileSync(join(repoRoot, 'docs', 'assets', file), 'utf8');
  if (!/log/i.test(svg)) fail(`${file}: log scale is not labelled`);
  console.log(`  ok   ${file}: scale labelled`);
}

if (failures) {
  console.error(`\n${failures} failure(s)`);
  process.exit(1);
}
console.log('\nok');