# Glossary

Terms as this project uses them. The rule the code follows is that these are
the only words for these things: the report, the CSV export and the diagnostics
all say `attributed`, so nothing here invents a second name for it.

## Size dimensions

The four values `-d` accepts and `sizes` carries. This is the set people
arrive confused about, because which ones you have depends on the input.

| term | what it measures | where it comes from |
|---|---|---|
| `stat` | the size the bundler declared, uncompressed, before minification | the bundler's own numbers |
| `parsed` | the bytes actually on disk | measured, always exact for "how big is the file" |
| `gzip` | those bytes at gzip level 6 | measured; the level is part of the contract |
| `attributed` | how many bytes each original source is responsible for | source map mappings; ground truth per source |

`stat` disagrees with `parsed` sometimes by a lot, and the disagreement is
usually the interesting part: `stat` is uncompressed source weight, `parsed`
includes minification and the bundler's own runtime.

Which ones you have:

| input | `stat` | `parsed` | `gzip` | `attributed` |
|---|---|---|---|---|
| `stats.json` / `metafile.json` alone | yes | no | no | no |
| metadata plus assets on disk | yes | yes | yes | only with maps |
| a build folder with maps | no | yes | yes | yes |
| a build folder without maps | no | yes | yes | no - and `NPT0050` says so |

Resolution order is `attributed > parsed > stat`, and `totals.size_dimension`
records which one the report used.

## Input

**bundler graph** - a document in which the bundler declared what it put in the
bundle: webpack or rspack `stats.json`, esbuild `metafile.json`. The only shape
where ghost code can be defined.

**declared graph** - same thing, said when the point is that something can be
compared against it.

**folder mode** - a directory read as output only, or as output plus whatever
metadata is inside it. See the [folder mode guide](../guides/folder-mode.md).

**auto-scan** - the ordered list of file names folder mode looks for. See the
[folder mode guide](../guides/folder-mode.md#what-it-looks-for-in-this-order).

## The join

**module** - one file as the bundler sees it. The unit budgets, the CSV export
and `--min-size` work on.

**asset** - one emitted file, measured on disk.

**chunk** - a group of assets the bundler decided to load together. Named, and
a chunk may carry more than one name.

**package** - an npm package, derived from a `node_modules/` path. Modules
without one are not counted in `package_count`.

**source** - an original file as it appears in a source map's `sources` list.

**fusion** - the step that reconciles a bundler graph with source maps: each
source's byte share is folded into the modules that produced it, and whatever
does not reconcile becomes a diagnostic. It is the part neither
`webpack-bundle-analyzer` nor `source-map-explorer` does.

**attribution** - matching a source to a module. A longest-suffix path match on
a normalised path, never a content hash: the key has to survive a rebuild with
no source change, or every report would show everything as new.

**ghost code** - declared by the bundler, accounted for by no source map. Code
that ships to production and that nobody has traced back to a file. Only
definable against a declared graph.

**hidden code** - bytes in the output that map back to no module: minifier
output, injected polyfills, the bundler's own runtime.

**coverage** - the share of total bytes a source map accounts for. At or above
99% the report is `attributed`; below that it falls back to `parsed`, because a
report that looks smaller than reality is the worst failure mode this tool has.

**attribution delta** - how much a module's size moved after attribution, in
bytes, signed. `attribution_delta` in the CSV and in the payload;
`significant_corrections` counts the ones above 1% of declared size.

## Reporting

**size dimension** - which of the four sizes a particular number is on. See
`totals.size_dimension` in [the payload](payload.md).

**bounded sample** - a list in the payload capped at a fixed size while its
count stays exact: at most 500 ghost modules and 500 hidden sources, at most
256 children per treemap node and 64 assets per group. The cap is always
reported (`*_truncated`, `dropped`) rather than hidden, because a truncated list
presented as a complete one is a lie that looks like data.

**detail file** - `nopeat-report.html.data.js`, the companion payload the HTML
report loads on demand when the inline copy would exceed 2 MB. `fetch()` cannot
read a sibling file from `file://`, which is why it is a script tag and not a
fetch.

**budget** - a ceiling in `nopeat.config.json` that makes the command exit 1.
See the [size budgets guide](../guides/budgets.md).

**diagnostic** - a machine-readable note about something the tool degraded,
stopped or corrected. See [diagnostics](diagnostics.md).

## Process

**contract** - a frozen specification under `docs/schema/`. Changing one needs
an ADR and a `schema_version` bump, not just a good commit message.

**ADR** - architecture decision record. The write-up that lets a later reader
find out why something is the way it is.

**parity** - the gate that diffs Nopeat's numbers against
`webpack-bundle-analyzer`'s on real builds, at 0.1% on assets and modules. A
wrong number is worse than no number.

**fixture** - a build the benchmarks run on. Real fixtures (small, famous,
permissively licensed projects) and synthetic fixtures (generated) are recorded
in separate tables and never mixed.

**unverified** - a target with no measurement. It stays that way; nobody fills
it in from a previous number, a different machine or an impression.
