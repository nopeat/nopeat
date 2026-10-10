# webpack and rspack

Both emit `stats.json`, and rspack uses the same schema, so this page covers
the two.

## Getting a `stats.json`

webpack has no single flag that writes the file every tool expects, so pick
whichever of these your setup already does:

```bash
# from a config that builds stats for you
npx webpack --profile --json > stats.json

# or from code
#   const stats = await compiler.run();
#   require('fs').writeFileSync('stats.json',
#     JSON.stringify(stats.toJson({ all: false, modules: true, chunks: true })));
```

rspack writes the same document through its own `stats` option.

Then:

```bash
nopeat ./dist/stats.json
```

The graph is declared by the bundler, so this is the input where ghost code is
defined and detected.

## Source maps, if you want per-source attribution

```js
// webpack.config.js
module.exports = {
  devtool: 'source-map',
  // ...
};
```

Put the maps next to the emitted assets (`*.map` beside `*.js`) and run against
the folder:

```bash
nopeat ./dist
```

With a declared graph *and* maps, Nopeat can reconcile the two - that is the
[fusion](../reference/glossary.md) step, and it is the part neither
`webpack-bundle-analyzer` nor `source-map-explorer` does on its own.

Without maps you still get declared sizes corrected against the bytes on disk,
and the report says so in its summary line.

## What you can tune

```bash
nopeat ./dist/stats.json \
  --exclude '\.map$' \
  --exclude '^dist/runtime' \
  --min-size 1024
```

- `--exclude` and `--include` take a Rust regular expression, are repeatable,
  and match against asset and chunk names. An invalid pattern is an error, not a
  silent pass.
- `--min-size` drops modules under a byte threshold and records what it dropped
  as `NPT0060`.

They compose: `--include` narrows the set first, `--exclude` removes from what
is left.

## Checking it in CI

```bash
nopeat ./dist/stats.json --budget nopeat.config.json
```

Exit codes and the config format are on [size budgets](budgets.md) and
[Nopeat in CI](ci.md).

## Where the numbers come from

Every performance figure the project publishes - including the ones on the
repository front page - is a measurement against a recorded fixture, following
the [benchmark protocol](../schema/bench-spec.md), with raw results in
`bench/results/`.
