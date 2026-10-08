#!/usr/bin/env node

import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const benchRoot = join(here, '..');
const resultsDir = join(benchRoot, 'results');

function arg(flag, fallback = null) {
  const i = process.argv.indexOf(flag);
  return i > -1 ? process.argv[i + 1] : fallback;
}

const tool = arg('--tool', 'wba');
const input = arg('--input');
if (!input) {
  console.error('usage: node run-tool.mjs --tool <wba|sme|node> --input <file>');
  process.exit(2);
}
if (!existsSync(input)) {
  console.error(`input not found: ${input}`);
  process.exit(2);
}

const commands = {

  wba: {
    cmd: process.execPath,
    args: [
      'node_modules/webpack-bundle-analyzer/lib/bin/analyzer.js',
      input,
      '.',
      '-m',
      'static',
      '-r',

      join(benchRoot, 'fixtures', 'artifacts', 'wba-report.html'),
      '-O',
    ],
    version: 'webpack-bundle-analyzer',
  },

  sme: {
    cmd: process.execPath,
    args: ['node_modules/source-map-explorer/bin/source-map-explorer.js', input, '--no-border-checks'],
    version: 'source-map-explorer',
  },

  node: {
    cmd: process.execPath,
    args: [
      '-e',
      `const fs=require('fs');const s=fs.readFileSync(process.argv[1],'utf8');const o=JSON.parse(s);` +
        `console.log('modules',Array.isArray(o.modules)?o.modules.length:0);`,
      input,
    ],
    version: 'node readFileSync+JSON.parse',
  },
};

const spec = commands[tool];
if (!spec) {
  console.error(`unknown tool: ${tool}`);
  process.exit(2);
}

const t0 = process.hrtime.bigint();
const child = spawn(spec.cmd, spec.args, { stdio: ['ignore', 'pipe', 'pipe'] });
let stdout = '';
let stderr = '';
child.stdout.on('data', (d) => (stdout += d));
child.stderr.on('data', (d) => (stderr += d));

const peakRss = { mb: 0 };
const sampler = setInterval(() => {

  peakRss.mb = Math.round(process.memoryUsage().rss / 1048576);
}, 100);

child.on('close', (code) => {
  clearInterval(sampler);
  const wallMs = Math.round(Number(process.hrtime.bigint() - t0) / 1e6);
  const result = {
    fixture: input.split(/[\\/]/).pop(),
    fixture_class: arg('--class', 'synthetic'),
    tool: `${spec.version}@${arg('--tool-version', 'unknown')}`,
    input_bytes: Number(arg('--bytes', '0')),
    wall_ms: wallMs,
    peak_rss_mb: peakRss.mb,
    peak_rss_source: 'parent-process (child RSS requires the shell wrapper)',
    phase_ms: null,
    exit_code: code,
    notes: [stdout.trim().split('\n').slice(-1)[0], stderr.trim().split('\n').slice(-1)[0]]
      .filter(Boolean)
      .join(' | ')
      .slice(0, 300),
  };
  mkdirSync(resultsDir, { recursive: true });
  const outFile = join(resultsDir, `${arg('--label', tool)}-${Date.now()}.json`);
  writeFileSync(outFile, `${JSON.stringify(result, null, 2)}\n`);
  console.log(JSON.stringify(result, null, 2));
  process.exitCode = code === 0 ? 0 : 3;
});
