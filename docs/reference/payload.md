# The payload

`--mode json` prints one JSON object to stdout and writes nothing else. It is
the same object the HTML report embeds, so anything the report shows can be
read from it.

```bash
nopeat ./dist --mode json > sizes.json
```

Twelve top-level keys:

```text
assets  chunks  compression  diagnostics  fusion  inputs  modules
schema_version  sizeDimension  target  totals  trees
```

`schema_version` is `1` and is a contract: a change that is not backwards
compatible bumps it.

## Which keys are always there

| key | type | notes |
|---|---|---|
| `schema_version` | integer | `1` |
| `target` | string | the input's label - file name, or folder name |
| `sizeDimension` | string | **the dimension you asked for** (see below) |
| `compression` | string | the algorithm that measured the compressed slot: `gzip` (default), `brotli` or `zstd` via `--compression-algorithm` |
| `totals` | object | `total_size`, `module_count`, `asset_count`, `package_count`, `size_dimension`, `module_size_sum` |
| `inputs` | array | what was read, in merge order |
| `diagnostics` | array | see [diagnostics](diagnostics.md) |
| `assets` | array | `name`, `size`, `chunks`, `sizes` |
| `chunks` | array | `id`, `names`, `initial`, `assets`, `size` |
| `trees` | array | one entry per requested dimension: `{dimension, tree}` |
| `fusion` | object or `null` | present when source maps were found |

`modules` is the one conditional top-level key: it
is present in `--mode json` and absent from the HTML report's own payload,
because there it lives in the companion detail file instead. An object keyed by
module id, each value carrying `name`, `package`, `chunks`, `reasons`,
`sources`, `sizes` and `attribution_delta`.

## `sizeDimension` is the request; `totals.size_dimension` is the answer

```bash
nopeat ./dist --mode json -d attributed
```

On an input with no source maps this reports:

```json
{ "sizeDimension": "attributed", "totals": { "size_dimension": "parsed" } }
```

The first is what `-d` asked for. The second is what the run could actually
produce, and it is the one to use - the report prints `NPT0050` when they
differ. The HTML report rewrites its `sizeDimension` to the value actually
used, so the two modes disagree about this key: read `totals.size_dimension`
and you are correct in both.

## Sizes

Every size-bearing object carries a `sizes` object rather than a bare number:

```json
{ "stat": 10240, "parsed": 4318, "gzip": 1402, "attributed": null }
```

`attributed` is `null` until a source map covers that object. `stat` is the
bundler's declaration and is `0` in folder mode, where nothing was declared.

## `inputs`

Tagged by `kind`:

| `kind` | extra fields | from |
|---|---|---|
| `stats` | `tool` | webpack / rspack `stats.json` |
| `esbuild_metafile` | - | esbuild `metafile.json` |
| `vite_manifest` | - | Vite `manifest.json` |
| `next_manifest` | - | Next `build-manifest.json` |
| `visualizer_stats` | `tool` | rollup-plugin-visualizer output |
| `source_map` | `name` | a `.map` |
| `dist_folder` | - | a directory with no metadata |

Later entries correct earlier ones: a folder's measured bytes override a
graph's declared ones.

## `trees`

```json
{ "dimension": "package", "tree": { "name": "...", "size": 0, "children": [] } }
```

`dimension` is one of `package`, `source_file`, `chunk`, `extension`. The
default set is the first three; `--dims ext` adds `extension`. Each node carries
`name`, `size`, `module_count`, `children` and `dropped` - the last being what
was folded away into a summary node rather than left out silently.

## `fusion`

`null` when no source maps were found. Otherwise:

| field | meaning |
|---|---|
| `ghost_modules` | declared by the bundler, mapped by nothing. Sampled to 500 entries |
| `ghost_modules_truncated` | how many were left out of the sample; the counts stay exact |
| `hidden_sources` | bytes that map back to no module. Sampled to 500 |
| `hidden_sources_truncated` | same, for hidden sources |
| `total_attribution_delta` | net correction applied across all modules, signed |
| `significant_corrections` | modules moved by more than 1% of their declared size |

Each ghost carries a `reason`: `unmapped`, `empty_attribution` or
`missing_asset`.

There is no coverage percentage in the payload. The `coverage` figure the
terminal prints is computed while fusing and is not stored, so a consumer that
needs it has to derive it from `totals.total_size` and the mapped bytes - or
read it off the `fusion:` line. Neither is there a `ghost_detectable` field: a
folder input reports that fact through `NPT0051` instead.

## Determinism

The same input gives byte-identical JSON: collections reaching the output are
`BTreeMap` or sorted `Vec`, there are no timestamps, no absolute paths and no
hash-map iteration order in it. That is what makes a CI diff of two runs
meaningful.

## The schema, and where it lags

[`docs/schema/report-schema.json`](../schema/report-schema.json) is the
contract. It currently declares eight properties against the eleven above, and
declares `additionalProperties: false`, so a consumer that validates strictly
will reject correct output:

- missing: `sizeDimension`, `target`, `trees`;
- `inputs` allows four of the seven `kind` values above, missing `dist_folder`,
  `vite_manifest` and `next_manifest`.

Reconciling it is a tracked good-first issue: the judgement call is whether the
schema or the CLI is wrong, and the answer changes what gets edited. Until it
is settled, validate against the list at the top of this page.

## Related

- [Reading the report](../guides/reading-the-report.md) - the human output that
  accompanies this.
- [The unified graph](../schema/unified-graph.md) - the model behind it.
- [Nopeat in CI](../guides/ci.md) - storing and comparing runs.
