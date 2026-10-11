# nopeat-cli

the `nopeat` binary. One analyzer for every bundler: `nopeat ./dist` analyses a stats file, its assets and its source maps in one pass.

## Install

```sh
npm install -g @nathangzchow/nopeat --allow-scripts=@nathangzchow/nopeat
nopeat ./dist
```

This package ships a wrapper, not a binary: a `postinstall` script downloads
the prebuilt `nopeat` for your platform from the
[GitHub releases](https://github.com/Nopeat/Nopeat/releases) and verifies it
against `checksums.txt`. The flag approves that script. npm 11 only warns for
an unapproved script and runs it anyway, so omitting the flag still installs
today, but npm is promoting that warning to an error and approving it keeps the
install working when it does. Either way, if the download never ran the
wrapper exits `127` with instructions the first time you run it.

The package name comes before the flag, because `--allow-scripts` takes the
value that follows it as its allow-list; written the other way round npm has no
package left to install and aborts reading a `package.json` from the current
directory.

If the download never happened, either re-run the installer:

```sh
node "$(npm root -g)/@nathangzchow/nopeat/install.mjs"
```

or let npm run it next time:

```sh
npm install -g @nathangzchow/nopeat --allow-scripts=@nathangzchow/nopeat
```

To persist it for future installs instead of typing the flag every time:

```sh
npm config set allow-scripts=@nathangzchow/nopeat --location=user
```

If you would rather not download anything at install time, build from source
and point the wrapper at it:

```sh
NOPEAT_BIN=/path/to/nopeat nopeat ./dist
```

## Documentation

These are the documents that matter, and they live in the repository rather than
being duplicated here:

- [Architecture](https://github.com/Nopeat/Nopeat/blob/main/ARCHITECTURE.md) - how
  it works, module by module, and why
- [Unified graph](https://github.com/Nopeat/Nopeat/blob/main/docs/schema/unified-graph.md)
  and [report schema](https://github.com/Nopeat/Nopeat/blob/main/docs/schema/report-schema.json)
- [CLI surface](https://github.com/Nopeat/Nopeat/blob/main/docs/schema/cli-surface.md)
- [Benchmark protocol](https://github.com/Nopeat/Nopeat/blob/main/docs/schema/bench-spec.md) -
  how a number has to be measured before it can be published
## Licence

MIT ([LICENSE-MIT](LICENSE-MIT)) or Apache-2.0 ([LICENSE-APACHE](LICENSE-APACHE)), at your option.