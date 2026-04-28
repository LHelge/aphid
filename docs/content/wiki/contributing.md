---
title: Contributing
category: Development
tags:
  - reference
---

Contributions are welcome. The project is dual licensed under MIT and Apache 2.0 — see the [About](/about/) page for details.

# Bug reports and feature requests

Open an issue on [GitHub](https://github.com/LHelge/aphid/issues). For bugs, include the `aphid --version` output, the command you ran, and what you expected to happen versus what did.

# Pull requests

1. Fork the repository and create a branch from `main`.
2. Make your changes. Keep commits focused — one logical change per commit.
3. Run the full check suite before opening the PR:

```sh
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

4. Use [Conventional Commits](https://www.conventionalcommits.org/) for commit messages: `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`, etc.
5. Open the PR against `main`. The CI workflow runs the same checks automatically.

# Development setup

```sh
git clone https://github.com/LHelge/aphid
cd aphid
cargo build
cargo test
```

Run the binary against the docs site to exercise a real build:

```sh
cargo run -- --config docs/aphid.toml build
```

Serve it locally:

```sh
cargo run -- --config docs/aphid.toml serve
```

# Project structure

The source follows the module layout described in the repository's `CLAUDE.md`. Key areas:

| Path | Purpose |
|------|---------|
| `src/content/` | Pass 1 — frontmatter parsing, slug indexing, backlink graph |
| `src/markdown/` | Pass 2 — pulldown-cmark pipeline: wiki-links, anchors, highlighting |
| `src/render/` | Tera renderer and theme loading |
| `src/output.rs` | Output directory management and page writing |
| `src/serve/` | Axum dev server, file watcher, live reload |
| `src/lib.rs` | Top-level `build()` and `serve()` entry points |
| `tests/` | Integration tests against fixture sites |
| `docs/` | This documentation site |
| `default-theme/` | The embedded default theme |
