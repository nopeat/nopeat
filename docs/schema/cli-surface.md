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
| `-m, --mode` | `static` \| `json` \| `server` | `static` | same flag; wba defaults to `server`, we default to `static` because a CI run wants a file, not a listener |
| `--host` | host name | `127.0.0.1` | same flag (server mode binds it) |
| `-p, --port` | number \| `auto` | `8888` | same flag; `auto` binds an OS-assigned port and prints the real URL |
| `-r, --report` | path | `nopeat-report.html` | same flag; different default file name. In `json` mode a report path writes the payload there instead of stdout |
| `-t, --title` | string | the input label | same flag; wba defaults to the current date |
| `-s, --default-sizes` | `stat` \| `parsed` \| `gzip` \| `brotli` \| `zstd` \| `attributed` | `parsed` | same flag plus `attributed`; wba 5.x default is also `parsed` |
| `--compression-algorithm` | `gzip` \| `brotli` \| `zstd` | `gzip` | same flag, same values; the compressed size column and the `gzip` size slot hold the bytes of the chosen algorithm |
| `-O, --no-open` | flag | off | accepted for compatibility; nopeat never opens a browser, so it is a no-op by construction |
| `-e, --exclude` | regex, repeatable | – | same flag |
| `-l, --log-level` | `debug` \| `info` \| `warn` \| `error` \| `silent` | `info` | same flag and levels; `silent` prints nothing, exit codes still carry the verdict |

Nopeat extensions, named so they cannot collide with a wba flag:

| flag | values | default | purpose |
|---|---|---|---|
| `--budget` | path to `nopeat.config.json` | – | CI gate: breaches emit `NPT0040` diagnostics and exit `1` (§3) |
| `--csv` | path | – | module-level export, one row per module |
| `--dims` | `package` \| `source` \| `chunk` \| `ext` (repeatable) | all four | which treemap dimensions to build |
| `--include` | regex, repeatable | – | whitelist counterpart to `--exclude` |
| `--json` | flag | off | shortcut for `--mode json` to stdout; it wins over a conflicting `--mode` |
| `--min-size` | bytes | – | drop modules below the threshold, reported as `NPT0060` |

The `-h` short flag stays bound to help, as it does in wba itself; `--host`
is long-only for the same reason wba made its host value optional
(webpack-bundle-analyzer#239).

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
