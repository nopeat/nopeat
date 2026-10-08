import fs from 'node:fs';
import path from 'node:path';

const HERE = path.dirname(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1'));
fs.mkdirSync(path.join(HERE, '..', 'fixtures', 'artifacts'), { recursive: true });

const target = Number(process.argv[2] || 381_000_000);

const out = process.argv[3] || path.join(HERE, '..', 'fixtures', 'artifacts', 'stats-synthetic.json');

const argOf = (flag, dflt) => {
  const i = process.argv.indexOf(flag);
  return i > -1 ? Number(process.argv[i + 1]) : dflt;
};
const N_ASSETS = argOf('--assets', 1500);
const N_CHUNKS = Math.min(200, N_ASSETS);
const ASSET_NAME = process.argv.includes('--asset-name')
  ? process.argv[process.argv.indexOf('--asset-name') + 1]
  : null;
const ASSET_BYTES = argOf('--asset-bytes', 0);

let SOURCE_NAMES = null;
if (process.argv.includes('--sources-from')) {
  const mapPath = process.argv[process.argv.indexOf('--sources-from') + 1];
  const map = JSON.parse(fs.readFileSync(mapPath, 'utf8'));

  SOURCE_NAMES = map.sources.map((s) => {
    const clean = s.replace(/^webpack:\/\/[^/]*\/?/, '');
    return clean.startsWith('.') ? clean : `./${clean.replace(/^\.?\//, '')}`;
  });
  console.error(`naming modules after ${SOURCE_NAMES.length} sources from ${map.file}`);
}

const fd = fs.openSync(out, 'w');
let written = 0;

function w(str) {
  const buf = Buffer.from(str);
  let off = 0;
  while (off < buf.length) off += fs.writeSync(fd, buf, off, buf.length - off);
  written += buf.length;
}

w(`{"version":"5.90.0","hash":"deadbeef","time":100,"builtAt":0,"publicPath":"/","outputPath":"/dist","assets":[`);
for (let i = 0; i < N_ASSETS; i++) {
  w(
    (i ? ',' : '') +
      JSON.stringify({
        type: 'asset',
        name: ASSET_NAME ?? `chunk.${i}.js`,
        size: ASSET_BYTES || 20000 + i * 37,
        chunks: [i % N_CHUNKS],
        chunkNames: [`chunk-${i % N_CHUNKS}`],
        emitted: true,
        info: { javascriptModule: false },
        auxiliaryFiles: [],
      }),
  );
}
w('],"chunks":[');
for (let i = 0; i < N_CHUNKS; i++) {
  const files = [];
  for (let a = 0; a < N_ASSETS; a++) if (a % N_CHUNKS === i) files.push(`chunk.${a}.js`);
  w(
    (i ? ',' : '') +
      JSON.stringify({
        id: i,
        names: [`chunk-${i}`],
        files,
        size: 20000 * files.length,
        entry: false,
        rendered: true,
        initial: true,
        modules: [],
        parents: [],
        siblings: [],
        children: [],
        origins: [],
      }),
  );
}
w('],"modules":[');

const sourceUnit = 'function f(a,b){return a+b*2-1;} '.repeat(60);

const moduleName = (n) =>
  SOURCE_NAMES
    ? SOURCE_NAMES[(n - 1) % SOURCE_NAMES.length]
    : `./node_modules/pkg${n % 400}/dist/index-${n}.js`;
let first = true;
let id = 1;
while (written < target) {
  const chunk = id % N_CHUNKS;
  w(
    (first ? '' : ',') +
      JSON.stringify({
        id,

        identifier: moduleName(id),
        name: moduleName(id),
        index: id,
        size: 1700 + (id % 900),
        cacheable: true,
        built: true,
        optional: false,
        prefetched: false,
        chunks: [chunk],
        issuer: null,
        issuerId: null,
        issuerName: null,
        failed: false,
        errors: 0,
        warnings: 0,
        assets: [],
        reasons: [],
        usedExports: true,
        providedExports: [],
        optimizationBailout: [],
        depth: 1,
        moduleType: 'javascript/auto',
        profile: { total: 100 },
        source: sourceUnit,
      }),
  );
  first = false;
  id++;
}

w('],"entrypoints":{},"errors":[],"warnings":[]}');
fs.closeSync(fd);

const result = {
  file: out,
  bytes: written,
  MB: +(written / 1048576).toFixed(2),
  modules: id - 1,
  assets: N_ASSETS,
  chunks: N_CHUNKS,
};
console.log(JSON.stringify(result));
