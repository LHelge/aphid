---
title: Dev server
category: Getting Started
tags:
  - reference
---

`aphid serve` runs a local development server with file watching and live reload — the recommended way to write content. Bare `aphid` is shorthand for `aphid serve --port 3000`.

# Starting it

```sh
aphid                     # default — serve on :3000
aphid serve               # explicit form, same as above
aphid serve --port 8080   # custom port
```

The server binds on `0.0.0.0`, so you can preview from another device on your LAN at `http://<your-machine-ip>:3000`.

# What it does

On startup, `aphid serve` loads `aphid.toml`, runs the same render pipeline as `aphid build`, and keeps the result in memory. Pages are served from that in-memory map; no `dist/` directory is touched. A request for `/wiki/foo/` returns the rendered HTML directly — no per-request rebuild.

# Live reload

Every served HTML page has a small `<script>` injected before `</body>` that opens a WebSocket back to the dev server. When the watcher detects a file change, the server rebuilds and the browser reloads automatically. The injection is a serve-mode-only transformation — `aphid build` never adds it, so it can't leak into a deployed site.

If the connection drops (e.g. you restart the server), the script retries every second so the page reconnects on its own once the server is back.

# File watching

The watcher rebuilds whenever any of these change:

- Files under `source_dir/` — your blog posts, wiki pages, and standalone pages.
- Files under `theme_dir/` — templates, theme static files, `theme.toml`.
- The config file (`aphid.toml`) itself.

Files in the user `static_dir/` are served directly without going through the render pipeline, so changes to them don't trigger a rebuild — refresh the browser to pick up new versions. The `Cache-Control: no-store` header (below) ensures the refresh actually fetches the new file.

# Broken wiki-links

`aphid serve` is permissive about broken `[[wiki-links]]`: a missing target is logged as a warning and rendered as a `<span class="wikilink broken">`, so half-written cross-links don't stop you from writing. `aphid build` treats the same broken link as a hard error.

# Cache headers

All responses get `Cache-Control: no-store` so edits to templates or static files always show up on the next request — no need for browser hard-reloads while iterating.

# Stopping

`Ctrl-C` triggers a graceful shutdown: the server drains in-flight requests, the file watcher stops cleanly, and any open WebSocket connections close.

See also: [[cli]], [[wiki-links]], [[themes]].
