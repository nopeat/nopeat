import { existsSync, globSync, readFileSync } from 'node:fs';

let failures = 0;

const targets = process.argv.slice(2).flatMap((arg) =>
  globSync(arg, { cwd: process.cwd() }).map((p) => p.replace(/\\\\/g, '/')),
);
if (targets.length === 0) {
  console.error('usage: node check-treemap-svg.mjs <treemap.svg> ...');
  process.exit(2);
}

for (const path of targets) {
  const svg = readFileSync(path, 'utf8');
  const width = Number(/width="(\d+)"/.exec(svg)?.[1] ?? 0);
  const height = Number(/height="(\d+)"/.exec(svg)?.[1] ?? 0);
  const re = /<rect x="([\d.]+)" y="([\d.]+)" width="([\d.]+)" height="([\d.]+)"[^>]*><title>([^<]+)<\/title>/g;
  const rects = [...svg.matchAll(re)].map((m) => ({
    x: +m[1],
    y: +m[2],
    w: +m[3],
    h: +m[4],
    title: m[5],
  }));

  if (rects.length === 0) {
    console.error(`FAIL ${path}: no labelled rectangles found - is this a treemap image?`);
    failures++;
    continue;
  }

  let overlaps = 0;
  for (let i = 0; i < rects.length; i++) {
    for (let j = i + 1; j < rects.length; j++) {
      const a = rects[i];
      const b = rects[j];
      const dx = Math.min(a.x + a.w, b.x + b.w) - Math.max(a.x, b.x);
      const dy = Math.min(a.y + a.h, b.y + b.h) - Math.max(a.y, b.y);

      if (dx > 1 && dy > 1) overlaps++;
    }
  }

  const area = rects.reduce((s, r) => s + r.w * r.h, 0);
  const fill = area / (width * height);
  const outside = rects.filter((r) => r.x < -1 || r.y < -1 || r.x + r.w > width + 1 || r.y + r.h > height + 1);
  const labelled = rects.filter((r) => svg.includes(`>${r.title.split(':')[0].slice(0, 4)}`)).length;

  const ok = overlaps === 0 && fill > 0.97 && outside.length === 0;
  if (!ok) failures++;
  console.log(
    `${ok ? 'ok  ' : 'FAIL'} ${path}: ${rects.length} rects, ${(fill * 100).toFixed(1)}% of the canvas, ${overlaps} overlaps, ${outside.length} outside, ${labelled} labelled`,
  );
}

process.exit(failures === 0 ? 0 : 1);