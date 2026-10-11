import { createHash } from 'node:crypto';
import {
  chmodSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
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

// A `checksums.txt` line is `<sha256>  <asset name>`, so the name is the second
// field. Taking the first one compares the hash against the asset name, which
// never matches: the download is skipped on every platform, and the package
// installs with no binary behind it.
export function checksumFor(text, assetName) {
  const rows = text
    .split('\n')
    .map((line) => line.trim().split(/\s+/))
    .filter((row) => row.length >= 2);
  return rows.find(([, name]) => name === assetName)?.[0] ?? null;
}

const alreadyUsable = () => {
  if (process.env.NOPEAT_BIN && existsSync(process.env.NOPEAT_BIN)) return true;
  const local = [
    join(here, '..', '..', 'target', 'release', exe),
    join(here, '..', '..', 'target', 'debug', exe),
  ].find(existsSync);
  return Boolean(local);
};

// The release publishes assets named after the Rust target triple, and the
// Windows one is a zip. Node's platform/arch do not line up with a Rust triple on
// their own, so the mapping is explicit: guessing produced a request for
// `nopeat-2.1.0-linux-x64.tar.gz` against a published
// `nopeat-2.1.0-x86_64-unknown-linux-gnu.tar.gz`, and a bare `.exe` request
// against a `.zip`.
const TARGET_TRIPLES = {
  'linux-x64': 'x86_64-unknown-linux-gnu',
  'linux-arm64': 'aarch64-unknown-linux-gnu',
  'darwin-x64': 'x86_64-apple-darwin',
  'darwin-arm64': 'aarch64-apple-darwin',
  'win32-x64': 'x86_64-pc-windows-msvc',
};

// Resolved on demand rather than at import, so requiring this module to reach
// `checksumFor` neither exits nor throws on a platform it cannot serve.
export function assetNameFor(platform = process.platform, arch = process.arch) {
  const targetTriple = TARGET_TRIPLES[`${platform}-${arch}`];
  if (!targetTriple) {
    throw new Error(
      `no published binary for ${platform}-${arch}; build from source with ` +
        '`cargo build --release`, or set NOPEAT_BIN.',
    );
  }
  return `nopeat-${version}-${targetTriple}${platform === 'win32' ? '.zip' : '.tar.gz'}`;
}

// Locate the released binary wherever the archive put it. On Windows the zip is
// flat and the file is `nopeat-<version>-<triple>.exe`; elsewhere it is `nopeat`
// inside a directory of that same name. Archives and licence files are skipped
// so the archive sitting in the same folder cannot be picked up by mistake.
export function findBinary(dir, name = exe) {
  const wanted =
    name.endsWith('.exe')
      ? (n) => n === name || /^nopeat-[^/]+\.exe$/.test(n)
      : (n) => n === name;
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (entry.isDirectory()) {
      const hit = findBinary(join(dir, entry.name), name);
      if (hit) return hit;
    } else if (wanted(entry.name)) {
      return join(dir, entry.name);
    }
  }
  return null;
}

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
  // A checkout that already has a built binary has nothing to download, and
  // running this as a side effect of `npm install` in the monorepo would
  // clobber it.
  if (alreadyUsable()) {
    process.exit(0);
  }
  if (unfilled()) {

    throw new Error(
      'no repository to download from: the repository URL in package.json is still a placeholder.\n' +
        'Fill it in via repo-links.json, or set NOPEAT_REPO=owner/name.',
    );
  }
  const assetName = assetNameFor();
  try {
    process.stdout.write(`nopeat: fetching ${assetName}\n`);
    const payload = await fetch(`${base}/${assetName}`);


    const checksums = await fetch(`${base}/checksums.txt`).catch(() => null);
    if (!checksums) {
      throw new Error(`no checksums.txt published for v${version}`);
    }
    const expected = checksumFor(checksums.toString('utf8'), assetName);
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
    // The two archives do not have the same layout. The Windows zip is a flat
    // `nopeat-<version>-<triple>.exe` with no directory around it, because
    // `Compress-Archive` was given the file. The Unix tar.gz does have a
    // `nopeat-<version>-<triple>/` directory, because it was built with `tar -czf`
    // on a staged folder. Searching for a wrapping directory therefore finds
    // nothing on Windows, and the archive we just wrote into the same folder
    // matches any `startsWith('nopeat-')` filter, so it can also be mistaken for
    // the binary. Walk the tree and match the real name instead.
    const tmp = join(vendorDir, '..', `extract-${process.pid}`);
    mkdirSync(tmp, { recursive: true });
    const archive = join(tmp, assetName);
    writeFileSync(archive, payload);

    if (assetName.endsWith('.zip')) {
      const { execFileSync } = await import('node:child_process');
      execFileSync(
        'powershell.exe',
        [
          '-NoProfile',
          '-Command',
          `Expand-Archive -LiteralPath '${archive}' -DestinationPath '${tmp}' -Force`,
        ],
        { stdio: 'ignore' },
      );
    } else {
      const { execFileSync } = await import('node:child_process');
      execFileSync('tar', ['-xzf', archive, '-C', tmp], { stdio: 'ignore' });
    }

    const extracted = findBinary(tmp);
    if (!extracted) {
      throw new Error(`the archive did not contain ${exe}`);
    }
    writeFileSync(target, readFileSync(extracted));
    rmSync(tmp, { recursive: true, force: true });
    if (process.platform !== 'win32') chmodSync(target, 0o755);
    process.stdout.write(`nopeat: installed ${target}\n`);
  } catch (err) {

    process.stdout.write(
      [
        `nopeat: skipped the binary download (${err.message}).`,
        'The package is installed, but it will need a binary at first run.',
        '  - re-run the installer: node install.mjs',
        '  - or build from source: cargo build --release',
        '  - or point at one: NOPEAT_BIN=/path/to/nopeat',
        '  - or let it run at install time: npm i -g @nathangzchow/nopeat --allow-scripts=@nathangzchow/nopeat',
        '',
      ].join('\n'),
    );
  }
}

// Importable so the checksum parser can be tested against a real
// `checksums.txt` without hitting the network; run directly it still installs.
const invokedDirectly =
  process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href;
if (invokedDirectly) {
  await main();
}
