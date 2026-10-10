import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { assetNameFor, checksumFor, findBinary } from '../install.mjs';

// The real shape of a `sha256sum`-style `checksums.txt`: hash first, two
// spaces, asset name. Nothing else in the line. Asset names come from
// `assetNameFor` so the fixture does not go stale on a version bump.
const PLATFORMS = [
  ['darwin', 'arm64'],
  ['darwin', 'x64'],
  ['linux', 'arm64'],
  ['linux', 'x64'],
  ['win32', 'x64'],
];

const HASHES = PLATFORMS.map(([platform, arch]) =>
  createHash('sha256').update(`${platform}-${arch}`).digest('hex'),
);

const CHECKSUMS = PLATFORMS.map(
  ([platform, arch], i) => `${HASHES[i]}  ${assetNameFor(platform, arch)}`,
).join('\n');

test('finds the hash for a published asset', () => {
  const name = assetNameFor('win32', 'x64');
  const hash = checksumFor(CHECKSUMS, name);
  assert.equal(hash, HASHES[PLATFORMS.findIndex(([p, a]) => p === 'win32' && a === 'x64')]);
  assert.match(hash, /^[0-9a-f]{64}$/);
});

test('returns the hash belonging to that asset, not another platform', () => {
  for (const [platform, arch] of PLATFORMS) {
    const name = assetNameFor(platform, arch);
    const expected = HASHES[PLATFORMS.findIndex(([p, a]) => p === platform && a === arch)];
    assert.equal(checksumFor(CHECKSUMS, name), expected, `wrong hash for ${name}`);
  }
});

test('tolerates CRLF line endings', () => {
  const name = assetNameFor('linux', 'x64');
  assert.equal(checksumFor(CHECKSUMS.replace(/\n/g, '\r\n'), name), HASHES[3]);
});

test('tolerates runs of spaces', () => {
  const name = assetNameFor('linux', 'x64');
  const spaced = CHECKSUMS.replace(
    `${HASHES[3]}  ${name}`,
    `${HASHES[3]}     ${name}`,
  );
  assert.equal(checksumFor(spaced, name), HASHES[3]);
});

test('returns null, not the wrong hash, for an asset that is not listed', () => {
  assert.equal(checksumFor(CHECKSUMS, 'nopeat-0.0.0-x86_64-pc-windows-msvc.zip'), null);
  assert.equal(checksumFor(CHECKSUMS, 'checksums.txt'), null);
});

test('returns null for empty and junk input', () => {
  assert.equal(checksumFor('', assetNameFor('win32', 'x64')), null);
  assert.equal(checksumFor('not a checksums file at all\n', 'nopeat.zip'), null);
  assert.equal(checksumFor('\n\n\n', 'nopeat.zip'), null);
});

// This is the regression that shipped: `split(/\s+/)` gives `[hash, name]`, and
// destructuring the first field as the name compared the hash against the asset
// name. Every platform failed to match, so the download was skipped on every
// install and the package landed with no binary behind it.
test('the hash is never mistaken for the asset name', () => {
  const name = assetNameFor('win32', 'x64');
  const hash = checksumFor(CHECKSUMS, name);
  assert.equal(hash.length, 64);
  assert.notEqual(hash, name);
});

// Same class of bug, one level up: an asset name the release never published
// makes the installer request a URL that 404s. The triples are what the
// release workflow publishes; only the version prefix moves.
test('asset names follow the release triple, not node platform strings', () => {
  const triples = {
    'linux-x64': 'x86_64-unknown-linux-gnu.tar.gz',
    'linux-arm64': 'aarch64-unknown-linux-gnu.tar.gz',
    'darwin-x64': 'x86_64-apple-darwin.tar.gz',
    'darwin-arm64': 'aarch64-apple-darwin.tar.gz',
    'win32-x64': 'x86_64-pc-windows-msvc.zip',
  };
  for (const [key, suffix] of Object.entries(triples)) {
    const [platform, arch] = key.split('-');
    assert.ok(
      assetNameFor(platform, arch).endsWith(suffix),
      `${key} should end with ${suffix}, got ${assetNameFor(platform, arch)}`,
    );
  }
});

test('windows is a zip, everything else a tar.gz', () => {
  assert.match(assetNameFor('win32', 'x64'), /\.zip$/);
  for (const [platform, arch] of PLATFORMS.filter(([p]) => p !== 'win32')) {
    assert.match(assetNameFor(platform, arch), /\.tar\.gz$/);
  }
});

test('an unsupported platform throws rather than naming a URL that cannot exist', () => {
  assert.throws(() => assetNameFor('linux', 'ia32'), /no published binary for linux-ia32/);
  assert.throws(() => assetNameFor('win32', 'arm64'), /no published binary for win32-arm64/);
});

// The two release archives really do have different layouts, which is why the
// lookup cannot assume a wrapping directory. release.yml zips the flat
// `nopeat-<version>-<triple>.exe` on Windows and tars a staged
// `nopeat-<version>-<triple>/` folder everywhere else.
function fixture(files) {
  const dir = mkdtempSync(join(tmpdir(), 'nopeat-find-'));
  for (const [rel, body] of Object.entries(files)) {
    const full = join(dir, rel);
    mkdirSync(join(full, '..'), { recursive: true });
    writeFileSync(full, body);
  }
  return dir;
}

test('finds the binary in the flat windows zip layout', () => {
  const dir = fixture({
    'nopeat-9.9.9-x86_64-pc-windows-msvc.exe': 'binary',
    'nopeat-9.9.9-x86_64-pc-windows-msvc.zip': 'archive',
  });
  try {
    assert.equal(
      findBinary(dir, 'nopeat.exe'),
      join(dir, 'nopeat-9.9.9-x86_64-pc-windows-msvc.exe'),
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('finds the binary in the unix tar.gz directory layout', () => {
  const dir = fixture({
    'nopeat-9.9.9-x86_64-unknown-linux-gnu/nopeat': 'binary',
    'nopeat-9.9.9-x86_64-unknown-linux-gnu.tar.gz': 'archive',
  });
  try {
    assert.equal(
      findBinary(dir, 'nopeat'),
      join(dir, 'nopeat-9.9.9-x86_64-unknown-linux-gnu', 'nopeat'),
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('never picks the archive sitting next to the binary', () => {
  const dir = fixture({
    'nopeat-9.9.9-x86_64-pc-windows-msvc.exe': 'binary',
    'nopeat-9.9.9-x86_64-pc-windows-msvc.zip': 'archive',
    'nopeat-9.9.9-x86_64-pc-windows-msvc.exe.licenses': 'licence',
  });
  try {
    assert.equal(findBinary(dir, 'nopeat.exe').endsWith('.zip'), false);
    assert.equal(findBinary(dir, 'nopeat.exe').endsWith('.licenses'), false);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('finds the binary in a nested directory', () => {
  const dir = fixture({
    'stage/nopeat-9.9.9-aarch64-apple-darwin/nopeat': 'binary',
    'stage/nopeat-9.9.9-aarch64-apple-darwin.tar.gz': 'archive',
  });
  try {
    assert.equal(
      findBinary(dir, 'nopeat'),
      join(dir, 'stage', 'nopeat-9.9.9-aarch64-apple-darwin', 'nopeat'),
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('returns null when the archive had no binary at all', () => {
  const dir = fixture({
    'nopeat-9.9.9-x86_64-pc-windows-msvc.zip': 'archive',
    'README.txt': 'docs',
  });
  try {
    assert.equal(findBinary(dir, 'nopeat.exe'), null);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
