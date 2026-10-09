# Nopeat

Bundle analysis in Rust. One binary, no Node runtime, no browser: give it a
`stats.json`, an esbuild `metafile.json`, or just a build folder, and every byte
comes back attributed to the source file that caused it.

```bash
nopeat ./dist/stats.json   # a bundler graph: webpack or rspack
nopeat ./dist/metafile.json  # an esbuild metafile
nopeat ./dist              # just the output folder, read recursively
```

Three shapes of input, one graph, one report. What each shape can and cannot
tell you is the single most useful thing to know before you start, so it is
worth a minute:

| input | sizes | per-source attribution | ghost code |
|---|---|---|---|
| `stats.json` / `metafile.json` | declared, then corrected from disk | only with maps next to the output | yes |
| a build folder with maps | measured on disk | yes | no (`NPT0051`) |

**Ghost code** is code the bundler declared and shipped that no source map
accounts for. **Hidden code** is bytes in the bundle that map back to no module -
minifier output, injected polyfills, the bundler's own runtime. Both are
first-class rows in the report rather than a footnote.

## Where to start

- [Install](install.md) - npm, a release binary, or from source.
- [Quick start](guides/quick-start.md) - a first run and how to read what it says.
- [Reading the report](guides/reading-the-report.md) - the summary line, the
  dimensions, the fusion line.

## Guides

- [webpack and rspack](guides/webpack-rspack.md)
- [esbuild](guides/esbuild.md)
- [Folder mode](guides/folder-mode.md) - Vite, Rollup, Parcel, tsup.
- [Size budgets](guides/budgets.md) - a limit that fails CI.
- [Nopeat in CI](guides/ci.md)

## Reference

- [CLI](reference/cli.md) - every flag, and what each exit code means.
- [Diagnostics](reference/diagnostics.md) - the `NPT` codes.
- [The payload](reference/payload.md) - what `--mode json` emits.
- [Glossary](reference/glossary.md) - `stat`, `parsed`, `gzip`, `attributed`.

## The longer view

- [Architecture](concepts/architecture.md) - how it is put together, and why.
- [Contracts](schema/unified-graph.md) - the frozen specifications: the graph,
  the CLI surface, the benchmark protocol.
- [Contributing](concepts/contributing.md) - the gates a change has to pass.

The source is at [github.com/Nopeat/Nopeat](https://github.com/Nopeat/Nopeat).
