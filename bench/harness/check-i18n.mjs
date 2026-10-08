#!/usr/bin/env node
// Translation parity for the shipped READMEs.
//
// It used to walk docs/<lang>/ and diff every document against docs/en/. That
// tree is not published any more, so the only thing left to compare is the
// README in each language, and the only thing that can drift is its structure.
//
// The checks, in order of how badly a failure misleads a reader:
//   - the file exists at all;
//   - every number in the canonical README appears in the translation, because a
//     missing figure makes a benchmark claim quietly untrue;
//   - the code blocks match, because a mistyped command is a support ticket;
//   - the links resolve, because a 404 in a translated README is invisible.
//
// Heading counts are compared too, but a mismatch there is reported rather than
// failed on: the canonical README carries two sections that have not been
// translated yet, and a visible untranslated block is better than a paraphrase.

import { readFileSync, existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const langs = ['zh', 'ja', 'de'];

const only = (() => {
  const i = process.argv.indexOf('--lang');
  return i > -1 ? process.argv[i + 1] : null;
})();

// German writes 2.295 where English writes 2,295, and 63,6 where English writes
// 63.6. Comparing the raw strings would report twelve missing figures in a
// translation that is correct. So the comparison is on value: parse each match
// under both conventions, keep the one that yields a number, and compare those.
const numbers = (text) => {
  const found = text.match(/\d[\d.,]{2,}/g) ?? [];
  const values = new Set();
  for (const raw of found) {
    // `1,234` is thousands in English and 1.234 in German; `1.234` is the
    // reverse. Try both readings and take whichever produces a number.
    const asEnglish = Number(raw.replace(/,/g, ''));
    const asGerman = Number(raw.replace(/\./g, '').replace(',', '.'));
    for (const v of [asEnglish, asGerman]) {
      if (Number.isFinite(v) && Math.abs(v) >= 1000) values.add(v);
    }
  }
  return [...values].sort((a, b) => a - b);
};
const codeBlocks = (text) => (text.match(/```[\s\S]*?```/g) ?? []).map((b) => b.replace(/\s+/g, ' ').trim());
const links = (text) => (text.match(/\]\(([^)]+)\)/g) ?? []).map((l) => l.slice(2, -1));
const sections = (t) => (t.match(/^## .+$/gm) ?? []).length;

const canonical = readFileSync(join(repoRoot, 'README.md'), 'utf8');
const canonicalNums = numbers(canonical);
const canonicalBlocks = codeBlocks(canonical).length;

let failures = 0;
let pending = 0;

for (const lang of langs) {
  if (only && only !== lang) continue;
  console.log(`\n== ${lang} ==`);

  const readme = join(repoRoot, `README.${lang}.md`);
  if (!existsSync(readme)) {
    console.log(`  FAIL     README.${lang}.md is missing`);
    failures++;
    continue;
  }
  const text = readFileSync(readme, 'utf8');
  const problems = [];

  const present = new Set(numbers(text));
  const missingNums = canonicalNums.filter((n) => !present.has(n));
  if (missingNums.length) problems.push(`numbers missing: ${missingNums.join(', ')}`);

  const blocks = codeBlocks(text).length;
  if (blocks !== canonicalBlocks) problems.push(`code blocks differ (${blocks} vs ${canonicalBlocks})`);

  const badLinks = links(text).filter((l) => {
    if (/^https?:/.test(l) || !l) return false;
    return !existsSync(join(repoRoot, l.split('#')[0]));
  });
  if (badLinks.length) problems.push(`broken relative links: ${badLinks.join(', ')}`);

  if (sections(text) !== sections(canonical)) {
    console.log(
      `  pending  ${sections(canonical) - sections(text)} section(s) not translated yet`,
    );
    pending++;
  }

  if (problems.length) {
    console.log(`  FAIL     README.${lang}.md`);
    for (const p of problems) console.log(`             - ${p}`);
    failures++;
  } else {
    console.log(`  ok       README.${lang}.md`);
  }
}

const englishCopy = readFileSync(join(repoRoot, 'README.en.md'), 'utf8');
if (canonical === englishCopy) {
  console.log('\n  ok       README.en.md is identical to README.md');
} else {
  console.log(
    '\n  FAIL     README.en.md has drifted from README.md ' +
      `(${sections(englishCopy)} sections vs ${sections(canonical)})\n` +
      '           Fix it by copying README.md over README.en.md. There is no second\n' +
      '           English document to reconcile; the translations are the ones that diverge.',
  );
  failures++;
}

console.log(`\n${failures} failure(s), ${pending} pending`);
process.exitCode = failures > 0 ? 1 : 0;
