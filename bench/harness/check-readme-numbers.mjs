import { readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = resolve(fileURLToPath(import.meta.url), '..', '..', '..');
const data = JSON.parse(readFileSync(join(repoRoot, 'docs/assets/charts-data.json'), 'utf8'));

function parseNumber(text) {
  if (/^\d{1,3}(\.\d{3})+$/.test(text)) return Number(text.replace(/\./g, ''));
  if (/^\d{1,3}(,\d{3})+$/.test(text)) return Number(text.replace(/,/g, ''));
  if (/^\d+,\d+$/.test(text)) return Number(text.replace(',', '.'));
  return Number(text);
}

function parseQuantity(text) {
  const match = /([\d.,]+)\s*(ms|s|min|MB|GB)/.exec(text);
  if (!match) return null;
  const value = parseNumber(match[1]);
  if (!Number.isFinite(value)) return null;
  switch (match[2]) {
    case 'ms':
      return { unit: 's', value: value / 1000 };
    case 's':
      return { unit: 's', value };
    case 'min':
      return { unit: 's', value: value * 60 };
    case 'MB':
      return { unit: 'mb', value };
    case 'GB':
      return { unit: 'mb', value: value * 1024 };
    default:
      return null;
  }
}

function matchesRounded(printed, actual, tolerance) {
  return Math.abs(printed - actual) <= tolerance;
}

let failures = 0;
const checked = [];

for (const name of ['README.md', 'README.en.md', 'README.zh.md', 'README.ja.md', 'README.de.md']) {
  const text = readFileSync(join(repoRoot, name), 'utf8');
  const lines = text.split('\n');

  let inComparisonTable = false;

  for (const [index, line] of lines.entries()) {
    if (!line.trim().startsWith('|')) {
      inComparisonTable = false;
      continue;
    }

    const cells = line.split('|').map((c) => c.trim()).slice(1, -1);

    if (/webpack-bundle-analyzer/i.test(line) && /nopeat/i.test(line)) {
      inComparisonTable = true;
      continue;
    }
    if (!inComparisonTable || cells.length < 4) continue;

    const [label, ours, theirs, ratioCell] = cells;
    if (!/\d/.test(label) || !/[\d]/.test(ours) || !/[\d]/.test(theirs)) continue;

    const key = Object.keys(data.stats_pipeline).find((k) =>
      label.includes(k.split(' ').slice(0, 2).join(' ')),
    );
    if (!key) continue;
    const record = data.stats_pipeline[key];

    const ourCells = ours.match(/[\d.,]+\s*(?:ms|s|min|MB|GB)/g) ?? [];
    const theirCells = theirs.match(/[\d.,]+\s*(?:ms|s|min|MB|GB)/g) ?? [];
    if (ourCells.length < 2 || theirCells.length < 2) {
      console.error(`${name}:${index + 1}: expected a time and a memory in both cells`);
      failures++;
      continue;
    }

    const pairs = [
      ['our time', ourCells[0], { unit: 's', value: record.ob_s }],
      ['our memory', ourCells[1], { unit: 'mb', value: record.ob_mb }],
      ['reference time', theirCells[0], { unit: 's', value: record.ref_s }],
      ['reference memory', theirCells[1], { unit: 'mb', value: record.ref_mb }],
    ];
    for (const [what, printedText, expected] of pairs) {
      const printed = parseQuantity(printedText);
      if (!printed) {
        console.error(`${name}:${index + 1}: cannot read ${what} from ${JSON.stringify(printedText)}`);
        failures++;
        continue;
      }
      if (printed.unit !== expected.unit) {
        console.error(
          `${name}:${index + 1}: ${what} is in ${printed.unit} but the record is in ${expected.unit}`,
        );
        failures++;
        continue;
      }

      const tolerance = Math.max(expected.value * 0.005, expected.unit === 's' ? 0.01 : 1);
      if (!matchesRounded(printed.value, expected.value, tolerance)) {
        console.error(
          `${name}:${index + 1}: ${what} reads ${printed.value}${expected.unit} but the ` +
            `record says ${expected.value}${expected.unit} (${key})`,
        );
        failures++;
      }
    }

    const ratioMatch = /([\d.,]+)\s*[x×倍倍]/.exec(ratioCell ?? '');
    if (ratioMatch) {
      const stated = Number(ratioMatch[1].replace(',', '.'));
      const timeRatio = record.ref_s / record.ob_s;
      const memRatio = record.ref_mb / record.ob_mb;
      const ok =
        Math.abs(stated - timeRatio) <= 1.1 ||
        Math.abs(stated - memRatio) <= 1.1 ||
        Math.abs(stated - timeRatio) / timeRatio <= 0.05 ||
        Math.abs(stated - memRatio) / memRatio <= 0.05;
      if (!ok) {
        console.error(
          `${name}:${index + 1}: ratio reads ${stated}x but the record gives ` +
            `${timeRatio.toFixed(1)}x on time and ${memRatio.toFixed(1)}x on memory`,
        );
        failures++;
      }
    }

    checked.push(`${name}:${index + 1} (${key})`);
  }
}

if (failures > 0) {
  console.error(`\n${failures} headline figure(s) disagree with docs/assets/charts-data.json`);
  process.exit(1);
}
if (checked.length === 0) {
  console.error('no comparison tables found - the check is not looking at anything');
  process.exit(1);
}
console.log(`ok   ${checked.length} comparison rows across ${new Set(checked.map((c) => c.split(' ')[0])).size} READMEs match charts-data.json`);
