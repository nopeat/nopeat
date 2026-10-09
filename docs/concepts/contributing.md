> The contributing guide from the repository root, included verbatim rather
> than copied. If you are reading this to work on Nopeat, start here: the
> project's bar is that every change ships with a measurement, or with an
> argument for why it cannot have one.

{{#include ../../CONTRIBUTING.md}}

## The documentation site

This book is the documentation site. Its sources live in `docs/` and it is
built by mdBook:

```bash
cargo install mdbook --locked
mdbook build          # writes book/
mdbook serve          # http://localhost:3000
```

`mdbook build` fails on a `SUMMARY.md` entry with no file behind it, because
`create-missing` is off in `book.toml`. CI builds the book on every pull
request, and the `pages` workflow publishes it to GitHub Pages on every push to
`main`.

Two things are worth knowing before editing:

- `docs/good-first-issues.md` is intentionally not in `SUMMARY.md` and is
  gitignored, so it exists locally and not in CI. mdBook ignores a Markdown
  file that no chapter references; nothing breaks.
- `ARCHITECTURE.md` and `CONTRIBUTING.md` are not chapters themselves. They are
  included into the two pages under [Concepts](architecture.md) by mdBook's
  include preprocessor, which is why their links are absolute repository URLs:
  a relative link in a file included two directories down would resolve
  against the wrong place.
