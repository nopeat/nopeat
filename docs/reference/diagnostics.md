# Diagnostics

Every time the tool degrades, stops or corrects something, it says so with a
code. Codes are stable strings: grep for `NPT0042` across the repository and you
find the line that emits it.

A diagnostic is a `Diagnostic` with five fields:

| field | meaning |
|---|---|
| `severity` | `info`, `warning` or `error` |
| `code` | `NPT` followed by four digits |
| `message` | prose, written for a person |
| `subject` | the module, asset or chunk it is about, when there is one |
| `data` | machine-readable context - counts, bytes, limits |

They surface in three places: the `diagnostics` array of [the
payload](payload.md), the diagnostics section of the HTML report, and, for
budgets, on stderr as `nopeat: <code> <message>`.

## The eight codes

This is the complete set the code can emit today.

| code | severity | condition |
|---|---|---|
| `NPT0001` | warning | the stats file has no `modules` array, which is a dev-server export rather than a build |
| `NPT0002` | info | an asset declared by the metadata was not found next to it, so its bytes on disk could not be measured |
| `NPT0040` | error | a budget rule was breached; the command exits 1 |
| `NPT0042` | error | the size invariant was violated: module sizes do not sum to the total |
| `NPT0050` | warning | the size dimension asked for is not what this run can report, and another one was used instead |
| `NPT0051` | info | no bundler metadata in the folder, so ghost code cannot be detected. It needs a declared graph to compare against |
| `NPT0052` | info | modules with no name were skipped. webpack emits one for its runtime, and a module with no path cannot be attributed to a file |
| `NPT0060` | info | modules below `--min-size` were filtered out. The count and the bytes are exact; the names listed are a sample |

## What each one means in practice

**`NPT0001` - the stats file is not a build.** `stats.toJson()` from a
dev server has no `modules` array. The graph is built from whatever is there
and the report is mostly empty; the warning is the reason why.

**`NPT0002` - an asset is missing from disk.** The metadata declared an asset
that is not where it was expected, so `parsed` and `gzip` for it stay at zero
and it is left on its declared `stat` size. The message lists up to five names.

**`NPT0040` - a budget was breached.** Emitted by `--budget`, with `scope`,
`dimension`, `actual` and `max` in `data`. The command exits `1`.

**`NPT0042` - the numbers stopped adding up.** When the report is on the
`attributed` dimension, the sum of module sizes is checked against the total
within 1,000 ppm (0.1%). A violation means the join keys were wrong, so it is
an error rather than a rounding note. It is emitted after fusion, only on that
dimension.

**`NPT0050` - the dimension changed.** Printed to the terminal with both
dimensions and the reason, and it is the one diagnostic you will actually read
on a normal run:

```
showing `parsed` instead of `attributed`: that dimension is not measurable for this input (NPT0050)
```

It also fires the other way, when maps cover the whole build and the tool
upgrades to `attributed`.

**`NPT0051` - ghost code cannot be detected here.** Folder mode with no
metadata. The honest answer, rather than a zero.

**`NPT0052` - some modules had no name.** webpack's runtime and a few synthetic
entries. They are skipped because a module with no path cannot be attributed to
a file; `data.modules` is the count.

**`NPT0060` - a filter ran.** `--min-size` dropped modules;
`data` carries `modules`, `bytes` and `min_size`, and the message samples at
most five names.

## How severity interacts with the exit code

It does not, except for budgets.

Exit codes are decided in one place: `0` for a clean run, `1` for a breached
budget, `2` for a rejected command line, `3` when the input or the budget
config could not be read. An `error`-severity diagnostic such as `NPT0042` does
not by itself change the exit code today, so a script that needs to notice it
must read `diagnostics` out of `--mode json` rather than check `$?`.

`docs/schema/cli-surface.md` and `docs/schema/unified-graph.md` both describe
`NPT0042` as paired with a non-zero exit. That is a disagreement between the
contracts and the code, tracked as an issue like the other ones below.

## Codes in the contracts that nothing emits

`docs/schema/unified-graph.md` section 5 also lists `NPT0010`, `NPT0011`,
`NPT0020`, `NPT0021` and `NPT0030` - source map version, missing
`sourcesContent`, ghost module, hidden bytes, gzip level. No code in the
workspace emits them; ghost and hidden are reported through `fusion` instead,
and a gzip level difference is not checked.

That file is a frozen contract, so correcting it needs an ADR and a
`schema_version` bump rather than an edit. Until then, the table above is what
the binary does and that section is what it was specified to do.

## Finding one in the code

```bash
grep -rn "NPT0042" crates/
```

Each code appears exactly once as an emitter, plus tests asserting it. See
[architecture](../concepts/architecture.md#diagnostics) for the same table in
the context of the pipeline.
