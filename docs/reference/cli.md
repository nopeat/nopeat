# CLI

The whole surface, as the binary actually implements it. Every flag below is
accepted by `nopeat` today; the defaults are what `--help` prints.

```text
nopeat <PATH> [OPTIONS]
```

`<PATH>` is required and is one of:

- a **file** - `stats.json`, `metafile.json` or a `*.map`, sniffed from its
  contents;
- a **directory** - auto-scanned as described in
  [folder mode](../guides/folder-mode.md).

A path that is neither exits `3` with `does not exist`.

## Options

| flag | values | default | what it does |
|---|---|---|---|
| `-m`, `--mode` | `static` \| `json` \| `server` | `static` | write the HTML report, print the payload, or serve a live one |
| `-r`, `--report` | path | `nopeat-report.html` | where the HTML report goes |
| `-d`, `--default-sizes` | `stat` \| `parsed` \| `gzip` \| `attributed` | `parsed` | the dimension to ask for |
| `-e`, `--exclude` | regex, repeatable | - | drop matching assets and chunks |
| `-i`, `--include` | regex, repeatable | - | keep only matching assets and chunks |
| `--min-size` | bytes | - | drop modules smaller than this (`NPT0060`) |
| `--csv` | path | - | also write a per-module CSV |
| `--budget` | path | - | evaluate a [budget config](../guides/budgets.md) |
| `--dims` | `package` \| `source` \| `chunk` \| `ext`, comma-separated | `package,source,chunk` | which treemap dimensions the payload carries |
| `--port` | port | `8888` | the port `--mode server` binds |
| `--bench` | flag | off | print one line of timings as JSON and stop before the report |
| `--bench-map` | flag | off | parse and attribute one source map, timed, and stop |

`--include` is applied before `--exclude`. Both are Rust regular expressions
against asset and chunk names; an invalid pattern is an error rather than a
pattern that silently matches nothing.

## Modes

**`static`** (the default) writes one self-contained HTML file. Payload inlined,
no web fonts, no network requests. It never opens a browser.

**`json`** writes the payload described in [the payload](payload.md) to stdout,
pretty-printed, and writes nothing else. This is the mode CI should use.

**`server`** binds `127.0.0.1:<port>` and re-renders the report whenever the
input's modification time changes:

```
nopeat: watching ./dist — live report at http://127.0.0.1:8888/ (Ctrl-C to stop)
```

The report is cached between requests and regenerated only when the stamp
changes; `/detail.js` serves the companion payload and `/__stamp` serves the
stamp itself, for anything polling it. It is a development convenience, not a
service: it listens on the loopback interface only.

## The two bench flags

```bash
nopeat ./dist --bench
nopeat ./map.js.map --bench-map
```

Both print a single JSON object and exit `0` without writing a report.

- `--bench` reports `ingest_ms`, `total_ms`, the counts and the total size.
- `--bench-map` reports `parse_ms`, `attribute_ms`, `total_ms`, `sources`,
  `mappings`, `attributed_files` and `attributed_total`.

They exist so a benchmark harness can time a phase without scraping a human
summary line.

## Exit codes

| code | meaning |
|---|---|
| `0` | success, no budget breach |
| `1` | a budget rule was breached (`NPT0040`) |
| `2` | usage error - clap rejected the command line |
| `3` | input unreadable, or the budget config was wrong |

Exit `3` covers three distinct failures, all of them "we could not read your
build": the path does not exist, the document could not be parsed, or the
budget config had no `limits`, matched nothing, or named a dimension the input
does not have. It is never a size regression; `1` is.

## Flags that exist but do nothing yet

`--json` and `--include-sources` are accepted and currently have no effect.
`--json` was meant as a shortcut for `--mode json`; use that instead.
`--include-sources` was meant to embed `sourcesContent` in the HTML for
drill-down. Both are listed here rather than in the table above because a flag
that parses and does nothing is worse than one that does not exist: a CI step
can be written against it and never know.

`--min-size` also prints `REGEX` as its value name in `--help`, which is a
copy-paste mistake - it takes a number of bytes.

## The contract document

[`docs/schema/cli-surface.md`](../schema/cli-surface.md) is the frozen
specification for this interface, and it does not currently agree with the
binary: it documents flags that were never built (`--no-fusion`,
`--cache-dir`, `--no-open`, `--log-level`), omits flags that were
(`--csv`, `--min-size`, `--include`, `--port`, `--bench`, `--bench-map`),
lists `--mode` as two values where there are three, and omits `ext` from
`--dims`. Until that is reconciled, this page is what the code does and that
page is what the code was specified to do. The discrepancy is a tracked issue;
reconciling it needs an ADR, not an edit, because the contract file is frozen
for Phase 1.

## Related

- [Diagnostics](diagnostics.md) - the codes a run can emit.
- [Size budgets](../guides/budgets.md) - the config `--budget` reads.
- [The payload](payload.md) - what `--mode json` prints.
