# esbuild

esbuild writes two different things, and which one you hand Nopeat decides what
it can tell you.

## With a metafile: a declared graph

```bash
esbuild src/app.ts --bundle --outdir=dist \
  --sourcemap --metafile=dist/metafile.json

nopeat dist/metafile.json
```

A metafile lists inputs, outputs and the module graph between them, which makes
it a declared graph: ghost code is defined and detected against it. The tool is
sniffed from the document shape (`inputs` + `outputs` and no `modules`), so
naming the file `metafile.json` is not required - but it is what the auto-scan
in [folder mode](folder-mode.md) looks for.

```bash
nopeat dist           # also finds dist/metafile.json on its own
```

## Without a metafile: the folder only

esbuild can emit a directory with no metadata at all. Then:

```bash
nopeat dist
```

You get measured sizes - `parsed` and `gzip` from the bytes on disk - and
per-source attribution from `dist/*.map` if you built with `--sourcemap`. You do
not get ghost code, because there is no declared module graph to compare
against; the report says exactly that (`NPT0051`) instead of printing zero.

## Sizes

`stat` sizes come from the bundler's declarations. esbuild's metafile does not
carry a declared size for every module the way webpack's `stats.json` does, so
in practice you will be reading `parsed`, `gzip` and - with maps - `attributed`.
The summary line always names the dimension in use, and if you ask for one that
this input cannot produce, the tool tells you which it substituted and why
(`NPT0050`).

## Attribution and fusion

With `--sourcemap`, the maps sit next to the outputs and Nopeat folds each
source's byte share into the modules that produced it. What does not reconcile
becomes the two first-class rows in the report:

- **ghost code** - declared by the metafile, accounted for by no source map;
- **hidden code** - bytes in the output that map back to no module.

The `fusion:` line under the summary is where the coverage figure comes from:

```
fusion: 1 map(s) · coverage 47% · 0/22 modules attributed · 22 ghost (83 KB of declared) · 3 hidden source(s) (39 KB)
```

## In CI

```bash
esbuild src/app.ts --bundle --outdir=dist --sourcemap --metafile=dist/metafile.json
nopeat dist/metafile.json --budget nopeat.config.json
```

See [size budgets](budgets.md) for the config and [Nopeat in CI](ci.md) for the
exit codes and the machine-readable output.
