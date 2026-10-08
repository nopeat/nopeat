#!/usr/bin/env node
// The three crate READMEs ship to crates.io, so a dead link in one is a dead link
// a visitor can click on the package page.
//
// `check-links.mjs` cannot catch these. It resolves *relative* paths, and these
// files link into the repository with absolute `https://github.com/...` URLs -
// which read as external links and are therefore skipped. That is exactly the
// shape a link takes when someone pastes a path from the browser, so it is a
// shape that will come back.
//
// So this resolves a repository URL against the working tree. It cannot tell
// whether the target exists on `main` today, which is the honest limit: a path
// that only exists in an unpushed commit will be reported as dead.

import { readFileSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const CRATES = ['nopeat-core', 'nopeat-cli', 'nopeat-wasm'];

const REPO = process.env.NOPEAT_REPO_URL ?? 'https://github.com/Nopeat/Nopeat';
const REPO_LINK = new RegExp(
  `${REPO.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}/(?:blob|tree)/main/([^\\s)\`"']+)`,
  'g',
);

let failures = 0;

for (const crate of CRATES) {
  const readme = join(repoRoot, 'crates', crate, 'README.md');
  if (!existsSync(readme)) {
    console.log(`  FAIL     crates/${crate}/README.md is missing`);
    failures++;
    continue;
  }

  const text = readFileSync(readme, 'utf8');
  const dead = [];
  for (const m of text.matchAll(REPO_LINK)) {
    if (!existsSync(join(repoRoot, m[1]))) dead.push(m[1]);
  }

  if (dead.length) {
    console.log(`  FAIL     crates/${crate}/README.md`);
    for (const d of new Set(dead)) console.log(`             - ${d}`);
    failures++;
  } else {
    const count = [...text.matchAll(REPO_LINK)].length;
    console.log(`  ok       crates/${crate}/README.md (${count} repository link(s))`);
  }
}

console.log(`\n${failures} failure(s)`);
process.exitCode = failures > 0 ? 1 : 0;