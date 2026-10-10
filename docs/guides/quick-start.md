# Quick start

Install as described in [Install](../install.md), point Nopeat at a build, and
read two lines.

## A first run

```bash
nopeat ./dist
```

Two things come back. The summary line:

```
dist  ·  22 modules  ·  2 assets  ·  11 packages  ·  ingest 4 ms  ·  total 6 ms  ·  dimension parsed
```

and the path of the report it wrote, `nopeat-report.html` by default. Open it;
it is one self-contained file with no web fonts and no network requests, so it
works from `file://` and it can be handed to someone who has never heard of the
tool.

| field | what it is |
|---|---|
| `dist` | the target: the file name of the input, or the folder name |
| `22 modules` | modules in the graph. A module is one file as the bundler sees it |
| `2 assets` | emitted files, measured on disk |
| `11 packages` | distinct npm packages the modules belong to |
| `ingest 4 ms` | reading and building the graph, before any analysis |
| `total 6 ms` | the whole run |
| `dimension parsed` | which of the four size dimensions the report is showing |

That last field is the one to check first. See [the
glossary](../reference/glossary.md) for what the four dimensions are, and
[reading the report](reading-the-report.md) for what happens when the one you
asked for is not measurable.

## Choosing the input

```bash
nopeat ./dist/stats.json   # webpack or rspack: declared graph
nopeat ./dist/metafile.json  # esbuild: declared graph
nopeat ./dist              # any build folder, read recursively
```

A declared graph is what makes [ghost code](../reference/glossary.md)
detectable.
A folder on its own gives measured sizes and source-map attribution, and reports
ghost code as undetectable (`NPT0051`) rather than as a reassuring zero.

## Asking for machine-readable output

```bash
nopeat ./dist --mode json > sizes.json   # the payload, to stdout
nopeat ./dist --csv sizes.csv            # one row per module
```

`--mode json` is the shape CI and BI consume; it is described in [the
payload](../reference/payload.md).

## Failing the build when a size regresses

```bash
nopeat ./dist --budget nopeat.config.json
```

Exit `0` when every limit holds, `1` when one is breached. The config format is
in [size budgets](budgets.md).

## What to read next

- Different bundler? [webpack and rspack](webpack-rspack.md),
  [esbuild](esbuild.md), or [folder mode](folder-mode.md).
- Want the whole surface? [CLI](../reference/cli.md).
- Want to know what the tool is doing and why it is fast?
  [Architecture](../concepts/architecture.md).
