# Install

Nopeat ships as a single binary. There is no Node runtime to install and no
browser to open; Node is only ever involved if you choose the npm wrapper, which
downloads the binary for you.

Pick whichever of the three is least friction where you are running it.

## From npm

```bash
npm i -g @nathangzchow/nopeat
```

The package publishes no binary of its own. On install it downloads the release
asset that matches your platform, verifies it against `checksums.txt`, and puts
`nopeat` on your `PATH`. Node 18 or newer.

## From a release binary

Every release publishes five targets - x86-64 and aarch64 for Linux, macOS and
Windows - each with a sha256 in `checksums.txt`. Verify before running anything:

```bash
# Linux and macOS
curl -L -o nopeat.tar.gz \
  https://github.com/nopeat/nopeat/releases/download/v2.1.1/nopeat-2.1.1-x86_64-unknown-linux-gnu.tar.gz
tar -xzf nopeat.tar.gz
sha256sum -c checksums.txt

# Windows
curl -L -o nopeat.zip \
  https://github.com/nopeat/nopeat/releases/download/v2.1.1/nopeat-2.1.1-x86_64-pc-windows-msvc.zip
```

## From source

Rust 1.90 or newer (see `rust-toolchain.toml`). The build needs no Node.

```bash
git clone https://github.com/nopeat/nopeat.git
cd nopeat
cargo build --release
./target/release/nopeat ./dist
```

The binary is at `target/release/nopeat`.

## `cargo install nopeat-cli` does not work yet

`nopeat-core` is on crates.io; the CLI crate is not published, so
`cargo install nopeat-cli` fails with a 404 today. Build from source instead,
or use one of the two routes above. The install section of the repository
README carries the same statement, and it will change in the commit that
publishes the crate.

## Check it

```bash
nopeat --version
```

Then go to the [quick start](guides/quick-start.md).
