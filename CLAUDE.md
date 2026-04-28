# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this project is

`aphid` is a CLI static site generator that builds a **blog**, a **wiki**, and standalone **pages** from markdown files, with `[[wiki-links]]` resolving across all three. Two entry points:

- `aphid` / `aphid serve` — axum dev server on port 3000 with file watching and WebSocket-driven live reload, for local writing. This is the default command.
- `aphid build` — renders into `./dist` (or `--output <dir>`), intended for CI (GitHub Actions → Pages).

Both modes share the same rendering pipeline; they differ only in what they do with the output and in broken-wiki-link handling (see below).

## Architecture

A **two-pass pipeline** is the load-bearing idea. Rendering a single file cannot be done in isolation because `[[wiki-links]]` may point at any other file in the site.

1. **Pass 1 — index** (sequential, I/O-bound): walk `content/blog/`, `content/wiki/`, and `content/pages/`, parse frontmatter, record each file's slug (its filename stem), destination URL, tags, and backlink targets. Produces a `Site` value holding every `Page`'s metadata and a slug → URL map.

2. **Pass 2 — render** (parallel, CPU-bound): for each page, run markdown through `pulldown-cmark`, rewrite `[[wiki-links]]` against the slug index, inject heading anchors, resolve syntax-highlighted code blocks, apply the Tera template, and write HTML.

Backlinks fall out of pass 1 cheaply: while you're already walking every page's links, invert the map.

### Wiki-link semantics

- Target is the **filename stem** — Obsidian-style. `[[glossary]]` resolves to `content/blog/glossary.md`, `content/wiki/glossary.md`, or `content/pages/glossary.md`, whichever exists.
- Pipe-alias form `[[slug|Display Text]]` is supported.
- Collision between any two files (blog, wiki, or page) with the same stem is a hard error in both `build` and `serve`.
- Unresolved link policy differs by mode: `build` fails, `serve` warns and renders the link in a `class="wikilink broken"` span so writing can continue.

### Serve / live reload

- Inject a small `<script>` into served HTML that opens a WebSocket to axum and reloads on file change.
- **The injection must never reach `dist/`.** It is a serve-mode-only transformation, applied after rendering, not baked into the templates or the Page HTML.
- File watching uses `notify` + `notify-debouncer-mini`.

### Clean URLs

`content/wiki/foo.md` → `dist/wiki/foo/index.html`, served at `/wiki/foo/`. Standalone pages are root-level: `content/pages/about.md` → `dist/about/index.html`, served at `/about/`. The renderer owns this mapping; both wiki-link rewriting and the serve router depend on it.

### Heading levels in Markdown

The markdown pipeline shifts all heading levels up by one (`HEADING_LEVEL_OFFSET = 1` in `src/markdown/anchors.rs`). The page title is rendered as `<h1>` by the template, so body headings start at `<h2>`. This means:

- `#` in a content file → `<h2>` in HTML
- `##` → `<h3>`, and so on

**Always write content files with `#` for top-level sections, `##` for subsections.** Never use `#` for the page title — that comes from frontmatter.

### Config

Site configuration lives in `aphid.toml` at the project root (title, `base_url`, etc.), parsed via `serde` + the `toml` crate. Path is overridable with `--config`. Frontmatter on individual pages is YAML via `serde_yml`.

## Common commands

Run each before committing (non-negotiable — see the style rules below):

```sh
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

Running the binary locally:

```sh
cargo run -- build
cargo run -- build --output public
cargo run -- serve --port 3000
RUST_LOG=debug cargo run -- serve
```

Run a single test:

```sh
cargo test <test_name_substring>
cargo test -p aphid <module>::tests::<name> -- --exact
```

Dependencies — **always via `cargo add`**, never by hand-editing `Cargo.toml`:

```sh
cargo add <crate> --features <feat1>,<feat2>
cargo rm <crate>
```

## Code style rules (project-specific)

- **Behaviour lives on types.** Prefer `impl` methods on structs/traits over free functions. Reach for a free function only when there is no reasonable owning type.
- **Modules pair logic with the data it manipulates** — e.g. `Site` and its loader sit together; wiki-link rewriting is a method on the markdown pipeline, not a loose helper.
- **Errors via `thiserror`.** One crate-level `Error` enum in `src/error.rs` with `#[from]` conversions for upstream error types. No `anyhow` in library code.
- **Logging via `tracing`.** `info!` for lifecycle events, `warn!` for recoverable issues (missing wiki-links in serve mode), `debug!` for verbose trace.
- **Async via `tokio`** — used for the dev server (axum, file watcher, WebSocket hub) and as the runtime for `aphid::build` / `aphid::serve`.
- **Parallelism via `rayon`.** Pass 2 (the per-page markdown render and per-page Tera template render) uses `rayon::par_iter`. Tokio is for I/O concurrency; the render pipeline is pure CPU and stays sync. Critically, this means a serve-mode rebuild offloads to the rayon pool and doesn't block tokio worker threads serving HTTP/WS requests.

## Planned module tree

Not all of these exist yet; create them as each slice lands.

```
src/
  main.rs              clap entry, tracing init, dispatch to lib
  lib.rs               public surface: build() / serve()
  error.rs             crate Error enum
  config.rs            aphid.toml → Config
  content/
    mod.rs             Site, Page types
    frontmatter.rs     YAML parsing via serde_yml
    loader.rs          pass 1: walk fs, build slug index + backlink graph
  markdown/
    mod.rs             pulldown-cmark event pipeline
    wikilinks.rs       [[slug]] rewriting against the index
    anchors.rs         heading id generation + TOC collection
    highlight.rs       syntect-driven code block highlighting
  render/
    mod.rs             Tera renderer; pass 2 orchestration
  build.rs             full build: pass 1 → pass 2 → write dist
  serve/
    mod.rs             axum app, route handlers
    livereload.rs      WebSocket hub + <script> injection
    watcher.rs         notify + debouncer bridge to the hub
```

## v1 scope (locked)

- Tags on blog posts, with tag index pages.
- Standalone pages (About, Contact, etc.) with nav integration.
- Syntax highlighting for fenced code blocks (`syntect`).
- Auto heading anchors + a TOC accessor for templates.
- Backlinks on wiki pages.
- Drafts, RSS/Atom — **not** v1; revisit after.

## Git workflow

- **Conventional Commits** for every message (`feat:`, `fix:`, `chore:`, `docs:`, `refactor:`, `test:`, `ci:`, …).
- Work happens on feature branches; landing on `main` is always through a pull request — never direct pushes.
- Merge strategy: **rebase** onto `main`, not merge-commits. Keep history linear. Resolve conflicts locally before opening the PR for review, or during the rebase before merge.

## Documentation

The wiki at `docs/content/wiki/` is the user-facing reference for aphid. **Keep it up to date whenever a feature is added or changed.** In particular:

- New config fields → [[configuration]]
- New frontmatter fields → [[frontmatter]]
- Changes to the markdown pipeline → [[markdown]]
- New template variables or templates → [[themes]]
- Changes to the install or build process → [[installation]]

If a change doesn't fit an existing page, add a new one.
