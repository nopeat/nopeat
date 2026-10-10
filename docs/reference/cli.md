# CLI

The whole surface, as the binary actually implements it. Every flag below is
accepted by `nopeat` today; the defaults are what `--help` prints. The flags of
webpack-bundle-analyzer keep their spelling and their short forms, so an
existing `wba` invocation is a drop-in swap.

```text
nopeat <PATH> [BUNDLE_DIR] [OPTIONS]
```

`<PATH>` is required and is one of:

- a **file** - `stats.json`, `metafile.json` or a `*.map`, sniffed from its
  contents;
- a **directory** - auto-scanned as described in
  [folder mode](../guides/folder-mode.md).

`[BUNDLE_DIR]` is the directory that holds the emitted assets. It matters when
`<PATH>` is a metadata file: without it the file's own directory is used, which
is what webpack-bundle-analyzer does too. With a directory as `<PATH>` the
argument is redundant and rejected, because the two would disagree about where
the bytes live.

A path that is neither exits `3` with `does not exist`.

## Options

| flag | values | default | what it does |
|---|---|---|---|
| `-m`, `--mode` | `static` \| `json` \| `server` | `static` | write the HTML report, print the payload, or serve a live one |
| `--host` | host name | `127.0.0.1` | what `--mode server` binds. Long-only, so `-h` stays help - the same trade-off webpack-bundle-analyzer made |
| `-p`, `--port` | number \| `auto` | `8888` | what `--mode server` binds; `auto` takes an OS-assigned port and prints the real URL |
| `-r`, `--report` | path | `nopeat-report.html` | where the HTML report goes; in `json` mode it names the file the payload is written to |
| `-t`, `--title` | string | the input's label | the `<title>` of the HTML report, HTML-escaped |
| `-s`, `--default-sizes` | `stat` \| `parsed` \| `gzip` \| `brotli` \| `zstd` \| `attributed` | `parsed` | the dimension to ask for |
| `--compression-algorithm` | `gzip` \| `brotli` \| `zstd` | `gzip` | what measures the compressed size column; the report labels it accordingly |
| `-O`, `--no-open` | flag | off | accepted so a wba command line parses unchanged; nopeat never opens a browser, so there is nothing to switch off |
| `-e`, `--exclude` | regex, repeatable | - | drop matching assets and chunks |
| `-i`, `--include` | regex, repeatable | - | keep only matching assets and chunks |
| `--min-size` | bytes | - | drop modules smaller than this (`NPT0060`) |
| `--csv` | path | - | also write a per-module CSV |
| `--budget` | path | - | evaluate a [budget config](../guides/budgets.md) |
| `--dims` | `package` \| `source` \| `chunk` \| `ext`, comma-separated | all four | which treemap dimensions the payload carries |
| `--json` | flag | off | shortcut for `--mode json` to stdout; it wins over a conflicting `--mode` |
| `-l`, `--log-level` | `debug` \| `info` \| `warn` \| `error` \| `silent` | `info` | how much the terminal hears; `silent` prints nothing and the exit code carries the verdict |
| `--bench` | flag | off | print one line of timings as JSON and stop before the report |
| `--bench-map` | flag | off | parse and attribute one source map, timed, and stop |

`--include` is applied before `--exclude`. Both are Rust regular expressions
against asset and chunk names; an invalid pattern is an error rather than a
pattern that silently matches nothing.

## Modes

**`static`** (the default) writes one self-contained HTML file. Payload inlined,
no web fonts, no network requests. It never opens a browser, which is why
`-O` is a no-op here.

**`json`** writes the payload described in [the payload](payload.md),
pretty-printed, to stdout - or to the `--report` file when one is given, which
is what webpack-bundle-analyzer's json mode does. This is the mode CI should
use.

**`server`** binds `--host`:`--port` and re-renders the report whenever the
input's modification time changes:

```
nopeat: watching ./dist — live report at http://127.0.0.1:8888/ (Ctrl-C to stop)
```

The report is cached between requests and regenerated only when the stamp
changes; `/detail.js` serves the companion payload and `/__stamp` serves the
stamp itself, for anything polling it. It is a development convenience, not a
service: the default host is loopback, and binding anything else is an explicit
`--host` away.

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

## Related

- [The contract](../schema/cli-surface.md) - the frozen specification this
  surface implements.
- [Diagnostics](diagnostics.md) - the codes a run can emit.
- [Size budgets](../guides/budgets.md) - the config `--budget` reads.
- [The payload](payload.md) - what `--mode json` prints.
