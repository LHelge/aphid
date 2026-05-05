---
title: Deployment
category: Getting Started
tags:
  - reference
---

`aphid build` writes a self-contained directory of HTML and static files (default: `dist/`). Pass `--output <dir>` to change the location. Any static host can serve it.

# GitHub Actions + GitHub Pages

The recommended workflow: content in a git repository, built by GitHub Actions, hosted on GitHub Pages.

## 1. Enable GitHub Pages

In the repository settings, set the Pages source to **GitHub Actions**.

## 2. Add the workflow

The recommended approach uses the official `LHelge/aphid` action, which downloads a pre-built binary — no Rust toolchain needed:

```yaml
name: Deploy to GitHub Pages

on:
  push:
    branches: [main]

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: pages
  cancel-in-progress: true

jobs:
  deploy:
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - uses: actions/checkout@v4

      - uses: LHelge/aphid@main
        with:
          output: _site

      - uses: actions/upload-pages-artifact@v3
        with:
          path: _site

      - uses: actions/deploy-pages@v4
        id: deployment
```

The action accepts these inputs:

| Input | Default | Description |
|-------|---------|-------------|
| `version` | `latest` | aphid version to download (e.g. `v0.1.4`) |
| `config` | `aphid.toml` | Path to the config file |
| `output` | `dist` | Output directory for the built site |

If your `aphid.toml` is not at the repository root, pass the path explicitly:

```yaml
- uses: LHelge/aphid@main
  with:
    config: path/to/aphid.toml
    output: _site
```

## Alternative: install from crates.io

If you prefer to build from source (longer CI times but no dependency on GitHub releases):

```yaml
name: Deploy to GitHub Pages

on:
  push:
    branches: [main]

permissions:
  contents: read
  pages: write
  id-token: write

concurrency:
  group: pages
  cancel-in-progress: true

jobs:
  deploy:
    runs-on: ubuntu-latest
    environment:
      name: github-pages
      url: ${{ steps.deployment.outputs.page_url }}
    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable

      - uses: Swatinem/rust-cache@v2

      - name: Build site
        run: cargo install aphid --locked && aphid build

      - uses: actions/upload-pages-artifact@v3
        with:
          path: dist/

      - uses: actions/deploy-pages@v4
        id: deployment
```

## Building from source in CI

For a repository that contains the aphid source (like this one), build the binary from source instead of installing from crates.io:

```yaml
- name: Build site
  run: cargo run --release -- --config docs/aphid.toml build
```

## base_url

Set `base_url` in `aphid.toml` to your Pages URL so that any absolute URL generation is correct:

```toml
base_url = "https://username.github.io/repository"
```

For a site hosted at a custom domain:

```toml
base_url = "https://example.com"
```

# Other platforms

The output is plain HTML — any host that can serve static files works.

| Platform | Notes |
|----------|-------|
| GitLab Pages | Add a `.gitlab-ci.yml` that runs `aphid build` and uploads the `dist/` artifact |
| Codeberg Pages | Push the built `dist/` contents to a `pages` branch |
| Netlify / Vercel | Set the build command to `cargo install aphid && aphid build` and the publish directory to `dist` |
| Self-hosted | Copy `dist/` to any web server; no server-side logic required |
