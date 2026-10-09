# Reading the report

Nopeat prints two or three lines and writes one file. This page explains each
of them.

## The summary line

```
dist  ·  22 modules  ·  2 assets  ·  11 packages  ·  ingest 4 ms  ·  total 6 ms  ·  dimension parsed
```

- **`dist`** - the target, from the file name of the input or the folder name.
- **`22 modules`** - one per file as the bundler saw it. Modules are what
  `--csv` exports and what a `--budget` `package` rule sums over.
- **`2 assets`** - emitted files. Their sizes come from disk, always.
- **`11 packages`** - distinct npm packages, derived from `node_modules/`
  paths. Modules with no package are not counted here.
- **`ingest 4 ms`** - reading the input and building the graph. Nothing else.
- **`total 6 ms`** - the whole run, including writing the report.
- **`dimension parsed`** - which of the four size dimensions the totals are on.

When gzip was measurable for every asset, a `gzip` figure is inserted after the
target name:

```
dist  ·  gzip 42 B  ·  0 modules  ·  1 assets  ·  0 packages  ·  ingest 1 ms  ·  total 5 ms  ·  dimension parsed
```

If only some assets were measured it says `gzip (partial 2/3)` instead, because
a partial total presented as a total is the kind of number this project exists
to avoid.

## When the dimension changes

If you asked for a dimension this input cannot produce, the tool says so, with
the code, before the summary line:

```
showing `parsed` instead of `attributed`: that dimension is not measurable for this input (NPT0050)
```

The other direction is worth noticing: when source maps cover the whole build,
the tool upgrades to `attributed` even if you did not ask for it, because that
one is ground truth:

```
showing `attributed` instead of `parsed`: source maps cover the build, so this is ground truth (NPT0050)
```

The report's payload carries the dimension actually used in `sizeDimension`, so
a consumer never has to infer it.

## The fusion line

Only printed when source maps were found:

```
fusion: 1 map(s) · coverage 47% · 0/22 modules attributed · 22 ghost (83 KB of declared) · 3 hidden source(s) (39 KB)
```

| field | meaning |
|---|---|
| `1 map(s)` | maps parsed and fused |
| `coverage 47%` | share of total bytes that a map accounts for |
| `0/22 modules attributed` | modules whose size was corrected from a map |
| `22 ghost (83 KB of declared)` | declared by the bundler, mapped by nothing |
| `3 hidden source(s) (39 KB)` | bytes in the output that map back to no module |

Coverage is what decides the dimension: at or above 99% the report is
`attributed`, below it the report falls back to `parsed`, because a report that
looks smaller than reality is the worst failure mode this tool has.

With no maps at all, you get neither line - and in folder mode, a `NPT0051`
explanation instead of a ghost count.

## The written file

```
wrote nopeat-report.html (0.0 MB), detail inlined
```

or, when the detail payload exceeds 2 MB:

```
wrote nopeat-report.html (1.5 MB), detail in a companion script (loaded on demand)
```

In the second case a sibling `nopeat-report.html.data.js` is written and loaded
on demand. `fetch()` cannot read a sibling file from `file://`, which is why
the report does not fetch it.

`--report` moves the file; `--csv` adds a `csv <path>` clause to the same line.
Both the report and its companion are excluded from the graph on a later run in
the same directory, so re-running does not grow the report by its own weight.

## Inside the HTML report

A squarified treemap on Canvas 2D, with:

- three grouping dimensions - package, source file, chunk - plus extension in
  the payload;
- fuzzy search over module names;
- per-module drill-down into the sources that produced it, where the maps
  provide them;
- light, dark and auto themes;
- no web fonts and no network requests, ever.

The treemap caps what it draws rather than truncating quietly: at most 256
children per node, with the rest folded into an `other` node, and at most 64
assets in a group, folded into `(+N more assets)`. Both report what they
dropped. The same rule applies upstream: the payload samples at most 500 ghost
modules and 500 hidden sources, with exact counts alongside, because one entry
per unmapped module once cost 47 MB for a field documented as a summary. These
caps are in the [glossary](../reference/glossary.md) under *bounded sample*.
