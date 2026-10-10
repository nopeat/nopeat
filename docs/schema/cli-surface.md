# Contract: CLI surface

Status: **frozen for Phase 1**. Owner: maintainers. The Rust types live in
`crates/nopeat-cli/src/main.rs`; this file is the normative spec.

Design rule: the flags of the two tools we replace keep working, so migrating
is a one-line change in a CI config.

## 1. Invocation

```
nopeat <PATH> [BUNDLE_DIR] [OPTIONS]

  <PATH>                 a dist folder, a stats.json, a metafile.json, or a
                         *.map file. A folder is auto-scanned (see §4).
  [BUNDLE_DIR]           directory holding the emitted assets. Used when
                         <PATH> is a metadata file; defaults to its parent,
                         matching webpack-bundle-analyzer. Giving a folder as
                         <PATH> already implies it.
```

Exit codes:

| code | meaning |
|---|---|
| `0` | success, no budget breach |
| `1` | budget breached (`--budget`), or an `NPT0042` invariant violation |
| `2` | usage error (clap) |
| `3` | input unreadable / unsupported schema |

`1` is the CI gate. `3` is reserved for "we could not read your build" and is
deliberately distinct from `1`, so a broken build never looks like a size
regression.

## 2. Options

Every webpack-bundle-analyzer CLI flag parses with the same spelling and the
same short form; the last column says what to expect when it does.

| flag | values | default | wba compatibility |
|---|---|---|---|
| `-m, --mode` | `static` \| `json` | `static` | `webpack-bundle-analyzer -m` |
| `-r, --report` | path | `nopeat-report.html` | `webpack-bundle-analyzer -r` |
| `-s, --default-sizes` | `stat` \| `parsed` \| `gzip` \| `attributed` | `parsed` | `webpack-bundle-analyzer -s` |
| `-e, --exclude` | regex, repeatable | – | `webpack-bundle-analyzer -e` |
| `-O, --no-open` | flag | off (static never opens a browser) | `webpack-bundle-analyzer -O` |
| `--budget` | path to `nopeat.config.json` | – | new |
| `--json` | flag (shortcut for `--mode json` to stdout) | off | new |
| `--dims` | `source` \| `package` \| `chunk` \| `ext` (repeatable) | all three of `source`, `package`, `chunk` | new |
| `--include-sources` | flag | off | new: embed `sourcesContent` in the HTML for drill-down |
| `--bench` | flag | off | new: print one line of JSON timings and counts (`ingest_ms`, `total_ms`, modules, assets, packages, total size, dimension) instead of a report, then exit `0` |
| `--bench-map` | flag | off | new: benchmark decoding one `*.map` file and print one line of JSON (`parse_ms`, `attribute_ms`, sources, mappings, attributed files and bytes), then exit |
| `-l, --log-level` | `error` \| `warn` \| `info` \| `debug` | `warn` | `webpack-bundle-analyzer -l` |

Deliberate non-goals for Phase 1: no `--serve` (a static single file is the
whole promise; see ADR-0002), no GUI, no plugin system.

## 3. Budget config

```jsonc
{
  // No JSON schema is published, so an editor cannot validate this. The two
  // rules a config must satisfy are: `limits` is required, and a rule that
  // matches nothing is an error rather than a silent pass.
  "limits": [
    { "scope": "total",    "max": 750000 },              // bytes, gzip dimension
    { "scope": "chunk",    "match": "framework", "max": 300000 },
    { "scope": "package",  "match": "react-dom", "max": 130000 }
  ]
}
```

- `max` is in bytes on the dimension reported by the run (printed in the
  summary line so a breach is never ambiguous).
- a `match` that matches nothing is an **error**, not a no-op: a typo in a
  budget rule must not silently pass CI.
- breaches emit `NPT0040` diagnostics and exit `1`.

## 4. Auto-scan rules (zero-config promise)

Given a directory, Nopeat looks for, in this order:

1. `stats.json` / `*.stats.json` (webpack, rspack) — enables the dependency graph
2. `*.map` next to the emitted assets — enables ground-truth attribution
3. `metafile.json` (esbuild) — enables the module graph without source maps
4. `visualizer.json` / `stats-visualizer.json` (rollup-plugin-visualizer)

Ambiguity is resolved by size (largest wins) and reported as a diagnostic, never
silently. A folder with no recognised artifact exits `3` with a message naming
what it looked for.

## 5. Output determinism

- `--mode json` writes the payload from `report-schema.json` to stdout, sorted
  keys, trailing newline; with `-r <file>` it writes the same payload to that
  file instead, matching wba's json mode.
- `--mode static` writes one self-contained HTML file: payload inlined, no
  network requests, no external assets.
- both honour `--dims` ordering; the summary line always names the size
  dimension used.

## 6. Budget scope for future flags

`--watch` (Phase 2), `--baseline <file>` (Phase 3, trend diffing) and
`--format sarif` (Phase 3) are explicitly reserved names so plugins and CI
configs written against Nopeat 1.x keep parsing.

## 7. Diagnostic codes

`NPT0040` (budget breach, §3) and `NPT0042` (size invariant, §1) are described
above. The remaining codes are the same ones documented in
[`ARCHITECTURE.md`](../../ARCHITECTURE.md); each is a stable string you can grep for.

| code | severity | condition |
|---|---|---|
| `NPT0001` | warning | the stats file has no `modules` array, which is a dev-server export rather than a build |
| `NPT0002` | error | no asset was found next to the metadata, so there is nothing to attribute |
| `NPT0050` | warning | no source maps sit next to the output, so only file sizes are known. Build with `--sourcemap` for per-source attribution |
| `NPT0051` | info | no bundler metadata in the folder, so ghost code cannot be detected. It needs a declared graph to compare against |
| `NPT0052` | info | modules with no name were skipped. webpack emits one for its runtime, and a module with no path cannot be attributed to a file |
| `NPT0060` | info | modules below `--min-size` were filtered out. The count and the bytes are exact; the names listed are a sample |
