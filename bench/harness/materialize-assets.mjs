import fs from 'node:fs';
import path from 'node:path';

const arg = (f, d = null) => {
  const i = process.argv.indexOf(f);
  return i > -1 ? process.argv[i + 1] : d;
};

const statsPath = arg('--stats');
const outDir = arg('--out');
const mapFor = arg('--map');
const mapTargetName = arg('--map-asset');
if (!statsPath || !outDir) {
  console.error('usage: node materialize-assets.mjs --stats <stats.json> --out <dir> [--map <file.map> [--map-asset <name>]]');
  process.exit(2);
}
fs.mkdirSync(outDir, { recursive: true });


const HEAD = 8 * 1024 * 1024;
const fd = fs.openSync(statsPath, 'r');
const head = Buffer.alloc(HEAD);
const read = fs.readSync(fd, head, 0, HEAD, 0);
fs.closeSync(fd);
const text = head.subarray(0, read).toString('latin1');

const assets = [];
const re = /"name":"([^"]+\.(?:js|css|mjs))","size":(\d+)/g;
let m;
while ((m = re.exec(text)) !== null) {
  assets.push({ name: m[1], size: Number(m[2]) });
}
if (assets.length === 0) {
  console.error('materialize-assets: no assets found in the head of', statsPath);
  process.exit(3);
}


let seed = 0x9e3779b9;
const rnd = () => {
  seed ^= seed << 13; seed >>>= 0;
  seed ^= seed >> 17;
  seed ^= seed << 5; seed >>>= 0;
  return seed;
};

const WORD = 'abcdefghijklmnopqrstuvwxyz0123456789';
const CHUNK = 1 << 20;
let totalBytes = 0;
for (const a of assets) {
  const target = path.join(outDir, a.name);
  const out = fs.openSync(target, 'w');
  let left = a.size;
  while (left > 0) {
    const n = Math.min(left, CHUNK);
    const buf = Buffer.allocUnsafe(n);
    for (let i = 0; i < n; i += 16) {
      const r = rnd();
      for (let k = 0; k < 16 && i + k < n; k++) buf[i + k] = WORD.charCodeAt((r >>> (k % 4 * 8)) & 31);
    }
    fs.writeSync(out, buf, 0, n);
    left -= n;
  }
  fs.closeSync(out);
  totalBytes += a.size;
}

let mapNote = null;
if (mapFor) {
  const name = mapTargetName ?? assets[0].name;
  const dest = path.join(outDir, `${name}.map`);
  fs.copyFileSync(mapFor, dest);
  mapNote = { map: `${name}.map`, bytes: fs.statSync(dest).size, sources: null };
}

console.log(
  JSON.stringify(
    { assets: assets.length, total_bytes: totalBytes, out_dir: outDir, map: mapNote },
    null,
    2,
  ),
);
