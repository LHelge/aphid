# aphid

A small static site generator that produces a **blog** and a **wiki** from a flat directory of markdown files, with `[[wiki-links]]` resolving across both.

Designed to be driven from a GitHub Action for Pages deployment and from `aphid serve` for local writing with file watching and live reload.

> Status: early — public API is unstable and most features are not yet implemented.

## Install

```sh
cargo install aphid --locked   # from crates.io
cargo install --path .         # from a local checkout
```

## Commands

```sh
aphid                 # run a dev server on :3000 with file watching + live reload (default command)
aphid serve           # same as `aphid`
aphid serve --port 8080
aphid build           # render the site into ./dist (use this in CI)
aphid build -o public # render into ./public instead
aphid --config path/to/aphid.toml ...
```

## Source layout

```
.
├── aphid.toml           # site config (title, base_url, …)
├── content/
│   ├── blog/*.md        # dated posts
│   ├── wiki/*.md        # reference pages
│   └── pages/*.md       # standalone pages (about, contact, …) at the site root
├── theme/               # optional — overrides the embedded default theme
│   ├── theme.toml
│   ├── templates/*.html # Tera templates
│   └── static/          # theme-owned assets
└── static/              # site-owned assets, copied through verbatim
```

The `content/blog`, `content/wiki`, and `content/pages` directories are flat — no nested sub-directories. A file's name (without the `.md` extension) is its slug. Omit the `theme/` directory and aphid renders with its built-in default theme.

## Frontmatter

YAML, delimited by `---`:

```markdown
---
title: Getting started
date: 2026-03-14
tags: [meta, notes]
---

Body in markdown here.
```

## Wiki-links

Write `[[slug]]` — where `slug` is the filename stem of the target — to link anywhere within the site:

```markdown
See the [[glossary]] for terminology.
Or with a display label: [[glossary|our shared glossary]].
```

Wiki-links work in both blog posts and wiki pages. The renderer resolves them in a first pass that builds an index of every file's slug; the second pass renders each page (markdown → HTML → template) in parallel via `rayon`.

- **In `aphid build`**, an unresolved `[[link]]` fails the build.
- **In `aphid serve`**, it's logged as a warning and rendered as a "missing" link so you can keep writing.

## Output

Clean URLs, e.g. `content/wiki/glossary.md` is served at `/wiki/glossary/` (backed by `dist/wiki/glossary/index.html`).

## License

Dual-licensed under MIT or Apache-2.0, at your option.
