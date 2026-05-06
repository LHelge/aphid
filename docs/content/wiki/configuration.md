---
title: Configuration
category: Getting Started
tags:
  - reference
---

`aphid` reads a `aphid.toml` file at the project root. Pass an alternate path with `--config <path>`.

# Required fields

| Field | Type | Description |
|-------|------|-------------|
| `title` | string | Site title, injected into every page |
| `base_url` | string | Canonical root URL (no trailing slash) |

# Optional fields

| Field | Default | Description |
|-------|---------|-------------|
| `description` | | Short site description — used as Atom `<subtitle>` and RSS `<channel><description>` |
| `source_dir` | `"content"` | Root of the Markdown content tree |
| `static_dir` | `"static"` | User static files copied to the output's `static/` directory |
| `theme_dir` | *(embedded)* | Path to a custom theme directory |
| `wiki_categories` | `[]` | Order for wiki category headings — see [Wiki category order](#wiki-category-order) |
| `wiki_default_category` | `"Other"` | Display name for wiki pages without an explicit `category` in frontmatter. Surfaces both as the page's own category label and as the heading for the catch-all group on the wiki index |
| `favicon` | | Path to a source image used to generate favicons at standard sizes — see [Favicon](#favicon) |
| `feed_limit` | `20` | Maximum number of blog posts in RSS/Atom feeds. Set to `0` to include all posts |
| `posts_per_page` | `10` | Posts shown per page on the blog index and tag pages — see [[pagination]] |

All path fields are resolved relative to the directory containing `aphid.toml`, so the same paths work regardless of which directory you invoke `aphid` from. For example, `aphid --config docs/aphid.toml build` looks for content in `docs/content/` even when run from the repo root.

# Source directory layout

`aphid` expects three subdirectories under `source_dir`, plus an optional `home.md` file at the root:

```
content/
  home.md    # optional — rendered into the `home` slot of home.html
  blog/      # dated posts (title, slug, author, created required)
  wiki/      # reference pages (all frontmatter optional)
  pages/     # standalone pages like About / Contact (title required)
```

Any of the three subdirectories may be absent — a site without `wiki/` simply has no wiki. Subdirectories below each kind are not walked: every `.md` file must sit directly in `blog/`, `wiki/`, or `pages/`. Files without a `.md` extension are ignored.

`content/home.md` is a special, optional, single file. Unlike other content types it does **not** use frontmatter — the entire file is markdown. It runs through the same render pipeline as every other page (wiki-links, heading anchors, syntax highlighting), but it is *not* a routable URL — its rendered HTML is exposed to the `home.html` template as the `home` variable so the template can embed it. Use `#` for section headings; the markdown pipeline shifts `#` → `<h2>`, so multiple sections in `home.md` produce a clean run of `<h2>`s. See [[themes]] for the template variable shape.

See [[frontmatter]] for the fields required by each content type.

# Wiki category order

By default, wiki categories on the wiki index are sorted alphabetically, with uncategorised pages last. Wiki pages without a `category` in frontmatter fall into a catch-all group named by `wiki_default_category` (default `"Other"`):

```toml
wiki_default_category = "Misc"
```

To pin a specific order for named categories, list them in `wiki_categories`:

```toml
wiki_categories = ["Getting Started", "Content", "Customization", "Development"]
```

Categories listed here appear in this order. Any wiki category not in the list falls through to alphabetical placement after the listed ones, and the default catch-all group stays last. Adding a new category that isn't in `wiki_categories` is safe — it just shows up at the bottom until you order it.

# Favicon

Set `favicon` to a single source image and `aphid` generates the full set of platform icons at build time:

```toml
favicon = "static/favicon.png"
```

The source can be a raster image (PNG, JPEG, …) or an SVG; SVGs are rasterised at 512 px via `resvg`. From that single source, the build emits:

- `favicon.ico` — multi-resolution ICO containing 16 px and 32 px frames
- `apple-touch-icon.png` — 180 px
- `android-chrome-192x192.png` — 192 px
- `android-chrome-512x512.png` — 512 px
- `site.webmanifest` — references the two Android icons and uses the site `title` as the app name

All of these are written to the site root (`/favicon.ico`, `/apple-touch-icon.png`, …). The matching `<link>` tags are exposed to templates as `favicon_tags`; render them in your `<head>` with `{{ favicon_tags | safe }}`. See [[themes]] for the full list of template variables.

If `favicon` is not set, no files are generated and `favicon_tags` is empty.

> [!NOTE]
> The favicon is generated once at startup and is **not** regenerated when the source image changes during `aphid serve` — the resize/encode step takes long enough that running it on every file event would make live reload sluggish. Restart the server to pick up changes to the source image.

# Authors

```toml
[[authors]]
name = "Alice"
email = "alice@example.com"   # optional
```

# Socials

```toml
[[socials]]
platform = "github"
url = "https://github.com/example"
```

# Example

```toml
title = "My Site"
base_url = "https://example.com"
description = "A blog about interesting things"
source_dir = "content"
favicon = "static/favicon.png"

[[authors]]
name = "Alice"

[[socials]]
platform = "github"
url = "https://github.com/alice"
```

See also: [[frontmatter]], [[wiki-links]].
