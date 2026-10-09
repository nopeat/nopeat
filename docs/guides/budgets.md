# Size budgets

A budget is a ceiling that fails the build. It is the difference between a
report somebody reads and a rule nobody can forget.

```bash
nopeat ./dist --budget nopeat.config.json
```

Exit `0` if every limit holds, `1` if one is breached, `3` if the config itself
is wrong.

## The config

```jsonc
// nopeat.config.json
{
  "limits": [
    { "scope": "total",   "max": 1_500_000 },
    { "scope": "chunk",   "match": "vendor", "max": 800_000 },
    { "scope": "package", "match": "moment", "max": 250_000, "dimension": "gzip" }
  ]
}
```

Unknown fields are rejected - the config is parsed with `deny_unknown_fields`,
so a typo in a key is an error rather than a limit that never runs.

| field | required | meaning |
|---|---|---|
| `scope` | yes | `total`, `chunk` or `package` |
| `max` | yes | the ceiling, in bytes |
| `match` | for `chunk` and `package` | the chunk name, or the package name |
| `dimension` | no | `stat`, `parsed`, `gzip` or `attributed`; defaults to whatever the report is showing |

## Scopes

- **`total`** - the whole graph. With no `dimension`, this is
  `totals.total_size`.
- **`chunk`** - `match` is compared against every name in the chunk, exactly.
  The sizes of all matching chunks are summed. A chunk may carry several names;
  all of them count.
- **`package`** - `match` is compared against the module's package name, and
  against `<name>/...` so a rule on `react` also catches `react/jsx-runtime`.
  Every matching module's size is summed.

## Two rules that are errors, not warnings

1. **A rule that matches nothing fails the run.** A typo in `match` must not
   quietly pass CI, so `no chunk matches \`vendor\`` and `no package matches
   \`moment\`` are errors, and the command exits `3`.
2. **A config with no `limits` fails the run**, for the same reason: nothing
   would be checked.

A dimension that is not available for the input - `attributed` with no source
map covering the build - is likewise an error rather than a silently skipped
rule.

## What a breach looks like

```
nopeat: NPT0040 package `moment` on gzip is 42 KB over its 250 KB limit
```

The breach is also pushed into the graph as a diagnostic, so it appears in the
HTML report and in `--mode json` with its `code`, `severity`, `message` and
`data`. The command exits `1`.

The `data` object carries `scope`, `dimension`, `actual` and `max`, which is
what a CI annotation wants rather than a parsed message.

## Dimensions in one sentence each

See the [glossary](../reference/glossary.md) for the full definition; the short
version is that `stat` is what the bundler declared, `parsed` is the bytes on
disk, `gzip` is those bytes at level 6, and `attributed` is ground truth per
original source. Pick the one you would defend in a review - for a shipped
bundle, `gzip` or `attributed`.
