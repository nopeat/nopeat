import { createHash } from 'node:crypto';
import { chmodSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { get as httpsGet } from 'node:https';

const here = dirname(fileURLToPath(import.meta.url));
const vendorDir = resolve(here, 'vendor', `${process.platform}-${process.arch}`);
const exe = process.platform === 'win32' ? 'nopeat.exe' : 'nopeat';
const target = join(vendorDir, exe);

const manifest = JSON.parse(readFileSync(resolve(here, 'package.json'), 'utf8'));
const version = process.env.NOPEAT_VERSION ?? manifest.version;


const slugFromManifest = String(manifest.repository?.url ?? '')
  .replace(/^git\+/, '')
  .replace(/\.git$/, '')
  .replace(/^https?:\/\/github\.com\//, '');
const repo = process.env.NOPEAT_REPO ?? slugFromManifest;
const base = `https://github.com/${repo}/releases/download/v${version}`;


const unfilled = () => /\{\{[A-Z_]+\}\}/.test(repo);

const alreadyUsable = () => {
  if (process.env.NOPEAT_BIN && existsSync(process.env.NOPEAT_BIN)) return true;
  const local = [
    join(here, '..', '..', 'target', 'release', exe),
    join(here, '..', '..', 'target', 'debug', exe),
  ].find(existsSync);
  return Boolean(local);
};

if (alreadyUsable()) {
  process.exit(0);
}

const assetName = `nopeat-${version}-${process.platform}-${process.arch}${process.platform === 'win32' ? '.exe' : '.tar.gz'}`;

function fetch(url, redirectsLeft = 5) {
  return new Promise((res, rej) => {
    httpsGet(url, { headers: { 'user-agent': 'nopeat-install' } }, (r) => {
      if (r.statusCode >= 300 && r.statusCode < 400 && r.headers.location) {
        if (redirectsLeft === 0) return rej(new Error('too many redirects'));
        r.resume();
        return res(fetch(r.headers.location, redirectsLeft - 1));
      }
      if (r.statusCode !== 200) {
        r.resume();
        return rej(new Error(`HTTP ${r.statusCode} for ${url}`));
      }
      const chunks = [];
      r.on('data', (c) => chunks.push(c));
      r.on('end', () => res(Buffer.concat(chunks)));
      r.on('error', rej);
    }).on('error', rej);
  });
}

async function main() {
  if (unfilled()) {

    throw new Error(
      'no repository to download from: the repository URL in package.json is still a placeholder.\n' +
        'Fill it in via repo-links.json, or set NOPEAT_REPO=owner/name.',
    );
  }
  try {
    process.stdout.write(`nopeat: fetching ${assetName}\n`);
    const payload = await fetch(`${base}/${assetName}`);


    const checksums = await fetch(`${base}/checksums.txt`).catch(() => null);
    if (!checksums) {
      throw new Error(`no checksums.txt published for v${version}`);
    }
    const expected = checksums
      .toString('utf8')
      .split('\n')
      .map((line) => line.trim().split(/\s+/))
      .find(([name]) => name === assetName)?.[0];
    if (!expected) {
      throw new Error(`checksums.txt has no entry for ${assetName}`);
    }
    const actual = createHash('sha256').update(payload).digest('hex');
    if (actual !== expected) {
      throw new Error(
        `checksum mismatch for ${assetName}\n  expected ${expected}\n  actual   ${actual}`,
      );
    }

    mkdirSync(vendorDir, { recursive: true });
    if (assetName.endsWith('.tar.gz')) {

      const { execFileSync } = await import('node:child_process');
      const tmp = join(vendorDir, '..', `extract-${process.pid}`);
      mkdirSync(tmp, { recursive: true });
      const tgz = join(tmp, assetName);
      writeFileSync(tgz, payload);
      execFileSync('tar', ['-xzf', tgz, '-C', tmp], { stdio: 'ignore' });
      const { readdirSync } = await import('node:fs');
      const [dir] = readdirSync(tmp).filter((n) => n.startsWith('nopeat-'));
      writeFileSync(target, readFileSync(join(tmp, dir, exe)));
    } else {
      writeFileSync(target, payload);
    }
    if (process.platform !== 'win32') chmodSync(target, 0o755);
    process.stdout.write(`nopeat: installed ${target}\n`);
  } catch (err) {

    process.stdout.write(
      [
        `nopeat: skipped the binary download (${err.message}).`,
        'The package is installed, but it will need a binary at first run.',
        '  - build from source: cargo build --release',
        '  - or re-run with the network: node install.mjs',
        '  - or point at one: NOPEAT_BIN=/path/to/nopeat',
        '',
      ].join('\n'),
    );
  }
}

await main();


void pathToFileURL;