#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(here, '..', '..');

const exe = process.platform === 'win32' ? 'nopeat.exe' : 'nopeat';

const candidates = [
  process.env.NOPEAT_BIN,
  join(here, '..', 'vendor', `${process.platform}-${process.arch}`, exe),
  join(repoRoot, 'target', 'release', exe),
  join(repoRoot, 'target', 'debug', exe),
].filter(Boolean);

const found = candidates.find((p) => existsSync(p));

if (!found) {
  process.stderr.write(
    [
      '',
      'nopeat: no binary found for this platform.',
      '',
      `  platform: ${process.platform}-${process.arch}`,
      `  looked in: ${candidates.join('\n            ')}`,
      '',
      '  Most often the install script did not run to completion: it downloads',
      '  the binary at install time, and it is skipped when npm is not allowed',
      '  to run it, or the download itself failed. Either way:',
      '',
      '    - let the install script run:  npm i -g @nathangzchow/nopeat --allow-scripts=@nathangzchow/nopeat',
      '    - or fetch it now:             node install.mjs',
      '    - or build from source:        cargo build --release --manifest-path crates/nopeat-cli/Cargo.toml',
      '    - or point at an existing one: NOPEAT_BIN=/path/to/nopeat',
      '',
    ].join('\n'),
  );
  process.exit(127);
}


const result = spawnSync(found, process.argv.slice(2), { stdio: 'inherit' });

if (result.error) {
  process.stderr.write(`nopeat: failed to run ${found}: ${result.error.message}\n`);
  process.exit(127);
}

process.exit(result.status ?? 1);