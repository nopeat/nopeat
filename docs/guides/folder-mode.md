# Folder mode

Vite, Rollup, Parcel and tsup emit a directory and nothing else. Point Nopeat
at the directory and it works out what it can:

```bash
nopeat ./dist
```

## What it looks for, in this order

If the path is a directory, Nopeat takes the first of these that exists as a
file inside it:

1. `stats.json`
2. `stats.stats.json`
3. `metafile.json`
4. `manifest.json`
5. `.vite/manifest.json`
6. `build-manifest.json`
7. `app-build-manifest.json`

The winner is ingested as a declared graph, and the tool is sniffed from its
shape: `manifest.json` is Vite, a `*build-manifest.json` is Next, a document
with `inputs` and `outputs` and no `modules` is esbuild, otherwise webpack.
A webpack or esbuild winner means the module graph is present, which is what
makes ghost code detectable.

If none of them is there, the folder is read as output only: every `.js`,
`.mjs`, `.cjs`, `.css` and `.html` file within twelve directory levels becomes
an asset, measured from its own size on disk.

If neither produces anything - no metadata, no build output - the command exits
`3` and names what it looked for.

## Source maps are found too

Every `*.map` within twelve directory levels is parsed and fused against the
graph, whether the graph came from metadata or from the folder. A map that
cannot be opened is reported as `could not read <name>: <reason>` rather than
dropped silently, because a silently dropped map shows up as a lower coverage
figure that reads like a build that shipped no maps.

Two consequences worth knowing:

- **Sizes are measured.** With no declared `stat` sizes to fall back on, the
  report is on `parsed` bytes, which are exact for "how big is the file".
- **Ghost code is undetectable.** Ghost is *declared by the bundler and mapped
  by nothing*. A folder has no declaration, so you get `NPT0051` - "ghost code
  cannot be detected" - instead of zero.

## What the report file does not analyse itself

A previous run left `nopeat-report.html` (and, when the detail was too large to
inline, `nopeat-report.html.data.js`) in the directory. Both are removed from
the graph before anything is counted, so re-running in place does not grow the
report by its own weight.

## Filtering

```bash
nopeat ./dist \
  --include '\.js$' \
  --exclude '\.map$' \
  --min-size 4096
```

`--include` narrows first, `--exclude` removes from what is left, `--min-size`
drops small modules and records the count and the bytes as `NPT0060`. All three
recompute totals afterwards, so a filtered graph still satisfies the size
invariant.

## Attribution without a graph

With maps but no metadata, Nopeat runs the sources-only path: each source in
each map becomes a module, byte shares are folded in, and whatever maps back to
nothing is reported as **hidden code**. This is the mode where the report's
`fusion:` line is most often the interesting one, because it is the only place
the difference between "the bundler said" and "the bytes say" is visible.
