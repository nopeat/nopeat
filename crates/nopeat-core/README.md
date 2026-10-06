# nopeat-core

the portable engine: streaming stats ingest, size attribution, source map parsing, the fusion join, and the report payload. No IO, no async runtime - which is what keeps the WASM target a packaging change rather than a rewrite.

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