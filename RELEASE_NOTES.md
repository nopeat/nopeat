# Nopeat 2.1.1

First published release to npm and crates.io. One binary, no Node runtime:
point it at a `stats.json`, an esbuild `metafile.json`, or a build folder and
every byte comes back attributed to the source that caused it.

## Install

- npm: `npm i -g nopeat` (downloads the release binary for your platform)
- Cargo: `cargo install nopeat-cli`
- or build from source: `cargo build --release`

## What changed since 2.1.0

Fixed:

- The npm installer asked for asset names the release never published, so
  installs failed on every platform. It now maps Node's `platform`/`arch` to the
  Rust target triple and unpacks the right archive.
- The cross builds did not install their targets, so three of the five release
  binaries failed to build.
- `blake3` was a dependency of a function nothing called, and it needs a C
  cross-compiler; dropped.
- The coverage floor read the wrong column of the llvm-cov summary and failed a
  run at 82.19%.

Changed:

- The README documented `nopeat ./dist --mode csv`, which exits 2. CSV is a
  separate `--csv <FILE>` flag.

See `CHANGELOG.md` for the full list.
