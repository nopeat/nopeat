<div align="center">

<img src="assets/logo.svg" alt="Nopeat logo" width="76" />

# Nopeat

Just a simple bundle analyzer that reduces memory usage and saves your time —
29× faster, 3.8× less memory.

**Bundle analysis in Rust. On a 1 GB `stats.json` with 445,602 modules it takes
6.1 s and 376 MB, where webpack-bundle-analyzer takes 176 s and 1.4 GB.**

One binary, no Node runtime, no browser. Give it a `stats.json`, an esbuild
`metafile.json`, or just a build folder, and every byte comes back attributed to
the source file that caused it.

[![CI](https://github.com/Nopeat/Nopeat/actions/workflows/ci.yml/badge.svg)](https://github.com/Nopeat/Nopeat/actions/workflows/ci.yml)
[![npm](https://img.shields.io/npm/v/@nathangzchow/nopeat)](https://www.npmjs.com/package/@nathangzchow/nopeat)
[![crates.io](https://img.shields.io/crates/v/nopeat-core)](https://crates.io/crates/nopeat-core)
[![rust](https://img.shields.io/badge/rust-1.90%2B-000?logo=rust&logoColor=white)](https://doc.rust-lang.org/stable/notes.html)
[![license](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)

**[English](README.en.md) · [中文](README.zh.md) · [日本語](README.ja.md) · [Deutsch](README.de.md)**

</div>

## Install

Install the `nopeat` command from npm — the package downloads the release
binary for your platform and verifies it against `checksums.txt`. The package
lives at [npmjs.com/package/@nathangzchow/nopeat](https://www.npmjs.com/package/@nathangzchow/nopeat):

```bash
npm install -g @nathangzchow/nopeat --allow-scripts=@nathangzchow/nopeat
```

Then check it:

```bash
nopeat --version   # nopeat 2.1.3
```

Two things about that command.

The package name comes **first**. `--allow-scripts` takes the value that
follows it as its allow-list, so `npm install -g --allow-scripts=@nathangzchow/nopeat`
— flag first — leaves npm with no package to install at all, and it aborts
trying to read a `package.json` out of the current directory. It has to be
`install -g <package> --allow-scripts=<package>`.

The flag approves the `postinstall` that fetches the binary. npm 11 only
**warns** for an unapproved script today — it prints `npm warn allow-scripts`
and runs it anyway — so dropping the flag still installs. npm is promoting that
warning to an error, so approving it now keeps the install working when it
does. To approve it once instead of typing it every time:

```bash
npm config set allow-scripts=@nathangzchow/nopeat --location=user
npm install -g @nathangzchow/nopeat
```

Run once without installing:

```bash
npx --yes --allow-scripts=@nathangzchow/nopeat @nathangzchow/nopeat ./dist
```

**Platforms:** prebuilt binaries ship for Windows x64, macOS x64 and Apple
silicon, and Linux x64 / arm64; anything else builds from source with `cargo`.

**Release binaries** — five targets per release (x86-64 and aarch64 for Linux,
macOS and Windows), each listed with a sha256 in `checksums.txt`. Verify before
running anything:

```bash
# Linux and macOS
curl -L -o nopeat.tar.gz \
  https://github.com/nopeat/nopeat/releases/download/v2.1.3/nopeat-2.1.3-x86_64-unknown-linux-gnu.tar.gz
tar -xzf nopeat.tar.gz
sha256sum -c checksums.txt

# Windows
curl -L -o nopeat.zip \
  https://github.com/nopeat/nopeat/releases/download/v2.1.3/nopeat-2.1.3-x86_64-pc-windows-msvc.zip
Expand-Archive nopeat.zip -DestinationPath .
```

**From source** — a Rust toolchain is the only requirement:

```bash
git clone https://github.com/nopeat/nopeat.git
cd nopeat
cargo build --release
./target/release/nopeat ./dist
```

**As a Rust library** — the analysis engine is on
[crates.io](https://crates.io/crates/nopeat-core):

```toml
[dependencies]
nopeat-core = "2.1.3"
```

`cargo install nopeat-cli` does not work yet: the CLI crate is not published,
so that command fails today. The npm package and the release binaries above are
the two working routes; the crate comes back in the commit that publishes it.

## Why it is fast

Three decisions, each with a number behind it. The full table is in
[`CHANGELOG.md`](CHANGELOG.md); the measurements are reproducible from
[`bench/results/`](bench/results/).

**It streams instead of building a parse tree.** The obvious implementation
deserialises `stats.json` into a `serde_json::Value` and then walks it. That value
is a tree of boxed maps and owned strings, and on the 1 GB fixture it cost about
550 MB before any analysis started. Modules enter the graph one at a time instead,
so the document is never resident. Peak stays at 376 MB against a 400 MB ceiling.

**It builds an index instead of scanning.** Matching each module to its source was
O(modules × sources): 2.5 billion comparisons, 73.8 seconds on the fused
benchmark. A suffix index built once per asset turns the lookup into a hash, and
the same benchmark drops to 4.94 seconds. A property test asserts the index and the
original scan agree on every awkward case, so it is the same rule with a lookup
table rather than a second implementation.

**It shares what repeats.** A large build has hundreds of thousands of modules and
a few hundred packages. Package references are interned, so every module in a
package points at one allocation. On the 1 GB fixture that was worth about 70 MB.

There is no `unsafe` in any of it — `#![forbid(unsafe_code)]` is enforced by the
compiler, so there is no hand-written SIMD and no unchecked indexing. The
bottleneck was never arithmetic. It was allocation and IO.

## It finds code nothing else can account for

The join between a bundler's module graph and a source map is the part no existing
tool does. `webpack-bundle-analyzer` reads a graph, `source-map-explorer` reads a
map, and neither reconciles them.

Nopeat does, and whatever fails to reconcile is reported rather than hidden:

- **Ghost code** — declared by the bundler, attributed to no source file. Code that
  will ship to production and that no one on the team has traced back.
- **Hidden code** — bytes in the bundle that map back to no module. Minifier
  output, injected polyfills, the bundler's own runtime.

Both show up in the report as first-class rows, not as a footnote. Ghost code
needs a declared graph to be defined against, so with a bare build folder it is
reported as undetectable (`NPT0051`) instead of as a reassuring zero.

## Supported bundlers

| bundler | what it reads | needs `dist/` + `*.map` | ghost code |
|---|---|---|---|
| **webpack** 4 / 5 | `stats.json` | no | yes |
| **rspack** | `stats.json`, same schema | no | yes |
| **esbuild** | `metafile.json`, or the folder | with a metafile, no | with a metafile, yes |
| **Vite** · **Rollup** · **Parcel** · **tsup** | the folder and its maps | yes | needs a `stats.json` |
| **Angular** · **Next.js** · **Nuxt** · **SvelteKit** | whatever they emit, which is webpack or Vite output | depends | depends |

Two input shapes. A **bundler graph** (`stats.json`, `metafile.json`) carries the
declared module structure, and is the only shape where ghost code can be detected.
Just the **output folder** carries measured file sizes and source maps, which is all
Vite, Rollup, Parcel and tsup give you by default. Sizes are measured rather than
estimated there, so they are exact.

```bash
nopeat ./dist/stats.json   # a bundler graph
nopeat ./dist              # just the output folder, read recursively
```

## The problem

`webpack-bundle-analyzer` holds the whole `stats.json` in the JS heap: **63.6 s and
2.3 GB** on a 363 MB build, to answer "how big is this?" It cannot tell you which
source file is responsible, either.

## Measurements

Full pipeline: parse, measure every asset on disk, fuse the maps, render. Synthetic
fixtures, median of three runs, on the reference machine.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/chart-pipeline-dark.svg">
  <img alt="Bar charts comparing Nopeat with webpack-bundle-analyzer. Wall clock 1.87 s against 63.6 s on 363 MB of stats, 6.14 s against 176.3 s on 1 GB. Peak memory 127 MB against 2,295 MB, and 376 MB against 1,437 MB." src="docs/assets/chart-pipeline-light.svg" width="100%">
</picture>

| input | Nopeat | webpack-bundle-analyzer | ratio |
|---|---|---|---|
| 363 MB `stats.json`, 154,379 modules | **1.87 s / 127 MB** | 63.6 s / 2,295 MB | 34× faster, 18× smaller |
| 1 GB `stats.json`, 445,602 modules | **6.14 s / 376 MB** | 176.3 s / 1,437 MB | 29× faster, 3.8× smaller |

Source map attribution, against the number of sources in the map. Five times the
sources costs `source-map-explorer` thirty times the time; this stays linear.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/chart-source-maps-dark.svg">
  <img alt="Log-log charts of source map attribution time and memory against source count. source-map-explorer rises from 0.25 s on a 12-source bundle to 562 s at 50,000 sources; Nopeat from 7 ms to 209 ms. Memory 642 MB against 67 MB at 50,000 sources." src="docs/assets/chart-source-maps-light.svg" width="100%">
</picture>

Every figure is generated from [`charts-data.json`](docs/assets/charts-data.json),
which records where each number came from.

## Ghost code and hidden code

The part neither reference tool can do.

- **Ghost code** — a module the bundler declared and shipped that no source map
  accounts for. Tree-shaking missed it, or the asset was built without a map.
- **Hidden code** — generated bytes that map back to no module. An inlined snippet,
  an `eval`, an injected polyfill.

Each source's byte share is folded into the modules that produced it, and whatever
does not reconcile becomes a diagnostic:

```
$ nopeat ./dist
dist  ·  22 modules  ·  2 assets  ·  11 packages  ·  ingest 4 ms  ·  total 6 ms  ·  dimension parsed
fusion: 1 map(s) · coverage 47% · 0/22 modules attributed · 22 ghost (83 KB of declared) · 3 hidden source(s) (39 KB)
```

`dimension parsed`, not `attributed`: half the build is unmapped, so the ground-truth
dimension was withdrawn and the report says so on its first line.

Attribution is a longest-suffix path match on the documented join key
([`unified-graph.md`](docs/schema/unified-graph.md)), never a content hash, and
the corrected sizes are checked against the asset total by an invariant that fails
loudly (`NPT0042`) rather than rounding. A map covering only part of the build
downgrades the report from `attributed` to `parsed`. In folder mode, where there is
no declared graph, ghost detection reports itself as undetectable rather than zero.

## The report

<p align="center">
  <img alt="The Nopeat HTML report: a squarified treemap of a build grouped by package, showing the 40 largest of 400, with a searchable module list, three grouping dimensions and light/dark themes." src="docs/assets/treemap-large.svg" width="100%">
</p>

<sub>The 40 largest packages of a synthetic 400-package build: 1,500 assets, 8,041
modules, in the sizes the bundler declared. Drawn from the tool's own graph payload
by `bench/harness/render-treemap.py` rather than screenshotted, so the labels are
the tool's output; CI regenerates it and fails on a diff. The HTML report adds
search, three grouping dimensions, per-module drill-down, light/dark, and makes no
network requests.</sub>

## Use

```bash
nopeat ./dist                      # a folder: stats + assets + *.map
nopeat ./dist/stats.json           # a stats file
nopeat ./dist/metafile.json        # an esbuild metafile

nopeat ./dist --budget nopeat.config.json   # exits 1 on a breach
nopeat ./dist --mode json > sizes.json               # for CI or BI
nopeat ./dist --csv sizes.csv                        # a column per size dimension
nopeat ./dist/map.js.map --bench-map                 # attribution only, timed
```

```jsonc
// nopeat.config.json
{
  "limits": [
    { "scope": "total",   "max": 1500000 },
    { "scope": "chunk",   "match": "vendor", "max": 800000 },
    { "scope": "package", "match": "moment", "max": 250000, "dimension": "gzip" }
  ]
}
```

<details>
<summary>Exit codes</summary>

| code | meaning |
|---|---|
| 0 | analysed, every budget held |
| 1 | analysed, a budget was breached (`NPT0040`) |
| 2 | bad command line |
| 3 | input unreadable, or a budget rule matching nothing |

A rule that matches nothing is an error rather than a no-op, so a typo cannot
quietly pass CI. A config that omits `limits` entirely is refused for the same
reason.

</details>

## Status

Pre-1.0. Anything unmeasured says so.

| area | state |
|---|---|
| ingest, attribution, source maps, fusion, report, budgets, JSON/CSV | implemented, measured, gated |
| 1 GB ingest wall clock | **misses**: 4.40 s against a 3 s target, at 350 MB |
| report first paint / 30 fps | **unverified** — no browser in CI; measured instead as 1.56 MB and 1.27 s at 154,379 modules |
| WASM build, WebGL renderer | not started |
| npm (`@nathangzchow/nopeat`) | published |
| crates.io | `nopeat-core` published; `nopeat-cli` **not published**, so `cargo install nopeat-cli` fails |
| GitHub release | v2.1.3 published, 5 targets, with sha256 checksums |
| tests | 82 Rust, 4 npm, 1,018 generated layout cases |
| coverage | 82.19% of lines, against a 70% floor |
| Linux and macOS runners | not exercised; CI runs on Windows only |

The one miss is `serde_json`'s DOM cursor over a 445,602-element module array, stated
in full in the [changelog](CHANGELOG.md).

## Why

| decision | evidence |
|---|---|
| streaming JSON, not `simd-json` | `simd-json` needs the whole document in memory, which is the ceiling being removed |
| our own HTML report, no vendored viewer | a viewer we do not control cannot show fusion data without a fork |
| rayon for size and gzip work | 25,600 assets: 2,177 ms serial → **342 ms**. The parse was never the bottleneck |
| a suffix index for the join | scanning every source per module is 2.5 billion comparisons; the index took it from 73.8 s to 4.9 s |
| exact counts, bounded lists | one entry per unmapped module cost 47 MB for a field documented as a *summary* |
| property tests over fixtures | found two defects no fixture could see: sizes never scaled **down**, and an out-of-range source index made attribution vanish silently |
| not vendoring WBA's viewer | then we own a UI we do not control |
| no WebGL treemap yet | 10k nodes is fine on Canvas 2D; the swap is Phase 2 behind a stable payload |

## Documentation

- [Documentation site](https://nopeat.github.io/Nopeat) — install, guides, CLI
  reference, diagnostics, glossary and the contracts below, as a book
- [Architecture](ARCHITECTURE.md) — how it works, module by module, and why
- [Payload schema](docs/schema/unified-graph.md) and [report schema](docs/schema/report-schema.json)
- [Benchmark protocol](docs/schema/bench-spec.md) — how every number above was measured
- [Contributing](CONTRIBUTING.md)

## Contributing

Every change ships with a measurement, or an argument for why it cannot have one.
See [CONTRIBUTING.md](CONTRIBUTING.md). Gates: `cargo test`, `clippy -D warnings`
with pedantic lints, `cargo fmt`, a 70% coverage floor, and parity against
`webpack-bundle-analyzer`.
[Code of conduct](CODE_OF_CONDUCT.md) · [Security](SECURITY.md)

## Licence

MIT ([LICENSE-MIT](LICENSE-MIT)) or Apache-2.0 ([LICENSE-APACHE](LICENSE-APACHE)),
at your option.
