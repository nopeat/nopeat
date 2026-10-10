# Nopeat in CI

Three things a CI job needs from a size tool: a number it can store, a limit it
can fail on, and an exit code that means one specific thing.

## The command

```bash
nopeat ./dist/stats.json --budget nopeat.config.json --mode json > sizes.json
```

Everything needed is one invocation. See [size budgets](budgets.md) for the
config.

## Exit codes

| code | meaning | what CI should do |
|---|---|---|
| `0` | analysed, every budget held | pass |
| `1` | analysed, a budget was breached (`NPT0040`) | fail: this is a size regression |
| `2` | bad command line (clap) | fail: fix the step, this is not a regression |
| `3` | input unreadable, or the budget config is wrong | fail: fix the build or the config |

`1` and `3` are deliberately different. `1` says "your build got bigger"; `3`
says "we could not read your build". A broken build must never look like a size
regression, or the first person to hit it will spend an hour bisecting the wrong
thing.

## Machine-readable output

```bash
nopeat ./dist --mode json > sizes.json   # the payload, sorted keys, to stdout
nopeat ./dist --csv sizes.csv            # one row per module
```

`--mode json` is the shape described in [the payload](../reference/payload.md).
`--csv` writes nine columns:

```
module_id,name,package,chunks,stat,parsed,gzip,attributed,delta
```

`chunks` is a `|`-separated list, `attributed` is empty when no source map
covered that module, and `delta` is the attribution correction applied to the
module. Fields containing a comma, a quote or a newline are quoted per
RFC 4180.

## A GitHub Actions job

```yaml
- uses: actions/checkout@v4
- uses: actions/download-artifact@v4
  with:
    name: dist
    path: dist

- name: size budget
  shell: bash
  run: |
    curl -L -o nopeat.tar.gz \
      https://github.com/nopeat/nopeat/releases/download/v2.1.1/nopeat-2.1.1-x86_64-unknown-linux-gnu.tar.gz
    tar -xzf nopeat.tar.gz
    nopeat ./dist/stats.json --budget nopeat.config.json --csv sizes.csv

- uses: actions/upload-artifact@v4
  if: always()
  with:
    name: sizes
    path: sizes.csv
```

`shell: bash` matters on a Windows runner, where the default is pwsh and
`VAR=value cmd` is not a syntax it has.

## Storing the number over time

`--mode json` gives you `totals.total_size` and the per-dimension totals. Keep
one file per run rather than a rolling average: the
[benchmark protocol](../schema/bench-spec.md) exists because a number without
its machine, its fixture and its run count is not a measurement.

If a figure you published stops reproducing, the project's rule is to open an
issue before changing the number. See
[contributing](../concepts/contributing.md#honest-numbers).

## What not to do

- **Do not grep the human summary line.** It is written for a person and it
  changes when a dimension is substituted (`NPT0050`).
- **Do not parse the HTML report.** It is a rendering of the payload; the
  payload is the contract.
- **Do not treat exit `3` as a regression.** See the table above.
