#!/usr/bin/env node
// The three crate READMEs ship to crates.io, so a dead link in one is a dead link
// a visitor can click on the package page.
//
// ARCHITECTURE.md and CONTRIBUTING.md are here for a related reason: both are
// included into the documentation site as chapters, and to survive that they
// link into the repository with absolute URLs rather than relative paths - a
// relative path included two directories down resolves against the wrong place.
//
// `check-links.mjs` cannot catch any of this. It resolves *relative* paths, and
// these files link into the repository with absolute `https://github.com/...`
// URLs - which read as external links and are therefore skipped. That is
// exactly the shape a link takes when someone pastes a path from the browser,
// so it is a shape that will come back.
//
// So this resolves a repository URL against the working tree. It cannot tell
// whether the target exists on `main` today, which is the honest limit: a path
// that only exists in an unpushed commit will be reported as dead.

import { readFileSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const CRATES = ['nopeat-core', 'nopeat-cli', 'nopeat-wasm'];
const AT_ROOT = ['ARCHITECTURE.md', 'CONTRIBUTING.md'];
const FILES = [...CRATES.map((c) => `crates/${c}/README.md`), ...AT_ROOT];

const REPO = process.env.NOPEAT_REPO_URL ?? 'https://github.com/Nopeat/Nopeat';
const REPO_LINK = new RegExp(
  `${REPO.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}/(?:blob|tree)/main/([^\\s)\`"']+)`,
  'g',
);

let failures = 0;

for (const rel of FILES) {
  const path = join(repoRoot, rel);
  if (!existsSync(path)) {
    console.log(`  FAIL     ${rel} is missing`);
    failures++;
    continue;
  }

  const text = readFileSync(path, 'utf8');
  const dead = [];
  for (const m of text.matchAll(REPO_LINK)) {
    // A link to `tree/main/bench/results/` carries the trailing slash; the
    // filesystem does not care and neither should this.
    const target = m[1].replace(/\/+$/, '');
    if (!existsSync(join(repoRoot, target))) dead.push(m[1]);
  }

  if (dead.length) {
    console.log(`  FAIL     ${rel}`);
    for (const d of new Set(dead)) console.log(`             - ${d}`);
    failures++;
  } else {
    const count = [...text.matchAll(REPO_LINK)].length;
    console.log(`  ok       ${rel} (${count} repository link(s))`);
  }
}

console.log(`\n${failures} failure(s)`);
process.exitCode = failures > 0 ? 1 : 0;
