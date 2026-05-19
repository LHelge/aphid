---
title: Configuration
category: Getting Started
tags:
  - reference
  - configuration
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
| `wiki_categories` | `[]` | Structured list of wiki categories with optional metadata — see [Wiki category order](#wiki-category-order) |
| `wiki_default_category` | `"Other"` | Display name for wiki pages without an explicit `category` in frontmatter. Surfaces both as the page's own category label and as the heading for the catch-all group on the wiki index |
| `favicon` | | Path to a source image used to generate favicons at standard sizes — see [Favicon](#favicon) |
| `social_image` | | Path or absolute URL to the default OpenGraph / Twitter card image — see [Social image](#social-image) |
| `feed_limit` | `20` | Maximum number of blog posts in RSS/Atom feeds. Set to `0` to include all posts |
| `posts_per_page` | `10` | Posts shown per page on the blog index and tag pages — see [[pagination]] |
| `reading_wpm` | `200` | Words-per-minute used for blog post `reading_time_minutes`. Raise it (e.g. `250`) for prose-heavy sites, lower it for code-heavy ones |

All path fields are resolved relative to the directory containing `aphid.toml`, so the same paths work regardless of which directory you invoke `aphid` from. For example, `aphid --config docs/aphid.toml build` looks for content in `docs/content/` even when run from the repo root.

# Source directory layout

`aphid` expects three subdirectories under `source_dir`, plus two optional special files at the root:

```
content/
  home.md    # optional — rendered into the `home` slot of home.html
  404.md     # optional — rendered into the `not_found` slot of 404.html
  wiki.md    # optional — rendered above category cards on wiki_index.html
  blog/      # dated posts (title, slug, author, created required)
  wiki/      # reference pages (all frontmatter optional)
  pages/     # standalone pages like About / Contact (title required)
```

Any of the three subdirectories may be absent — a site without `wiki/` simply has no wiki. Subdirectories below each kind are not walked: every `.md` file must sit directly in `blog/`, `wiki/`, or `pages/`. Files without a `.md` extension are ignored.

`content/home.md`, `content/404.md`, and `content/wiki.md` are special, optional, single files. Unlike other content types they do **not** use frontmatter — the entire file is markdown. Each runs through the same render pipeline as every other page (wiki-links, heading anchors, syntax highlighting, mermaid), but none is a routable URL — their rendered HTML is exposed to the corresponding template as a variable (`home` on `home.html`, `not_found` on `404.html`, `wiki_intro` on `wiki_index.html`) so the template can embed it. Use `#` for section headings; the markdown pipeline shifts `#` → `<h2>`, so multiple sections produce a clean run of `<h2>`s. See [[themes]] for the template variable shapes.

See [[frontmatter]] for the fields required by each content type.

# Wiki category order

By default, wiki categories on the wiki index are sorted alphabetically, with uncategorised pages last. Wiki pages without a `category` in frontmatter fall into a catch-all group named by `wiki_default_category` (default `"Other"`):

```toml
wiki_default_category = "Misc"
```

To pin a specific order for named categories and provide metadata for the wiki index cards, list them as `[[wiki_categories]]` entries:

```toml
[[wiki_categories]]
name = "Getting Started"
description = "Installation, configuration, and first steps."
icon = "/static/category/getting-started.svg"

[[wiki_categories]]
name = "Content"
description = "Writing blog posts, wiki pages, and standalone pages."

[[wiki_categories]]
name = "Customization"

[[wiki_categories]]
name = "Development"
description = "Contributing to aphid and local development setup."
```

Each entry has:

| Field | Required | Description |
|-------|----------|-------------|
| `name` | yes | The category name (must match the `category` field in wiki frontmatter) |
| `description` | no | One or two sentences shown on the wiki index card |
| `icon` | no | Root-relative URL path to an SVG icon (e.g. `"/static/category/getting-started.svg"`) — passed to templates as-is |

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
> In `aphid serve`, the favicon set is cached and only regenerated when the source image's mtime changes. Editing markdown triggers a fast rebuild that reuses the cached set; saving a new `favicon.png` triggers a rebuild that detects the change and re-encodes (the resize step takes ~700 ms, but only when it actually has work to do).

# Social image

```toml
social_image = "/static/social-card.png"
```

The site-wide default for OpenGraph (`og:image`) and Twitter (`twitter:image`) tags — what social platforms and chat apps use as the preview when someone shares a link. Used as the fallback on pages without their own image. Blog posts override it with their frontmatter `image`.

Write the path as a root-relative URL (`/static/social-card.png`) or an absolute `http(s)://` URL, matching the convention used for blog hero images and `favicon`. Social crawlers fetch out of site context, so the rendered meta tags always carry the full URL built from `base_url`.

Recommended dimensions: 1200×630 px (the format most platforms render best as `summary_large_image`).

# Authors

```toml
[[authors]]
name = "Alice"
link = "https://alice.example.com"     # optional — used verbatim as the author link
email = "alice@example.com"            # optional — falls back to `mailto:` link if `link` is unset
image = "/static/authors/alice.jpg"    # optional — root-relative URL or absolute URL
```

The `link` field is exposed to blog templates as `author.link`. When it's unset but `email` is configured, templates receive `mailto:{email}` instead. With neither set, `author.link` is absent and templates render the author name as plain text.

The `image` field sets the author's profile picture shown on blog posts. Write it as a root-relative URL (`/static/authors/alice.jpg`) or an absolute `http(s)://` URL — same convention as blog hero images and `favicon`. When no `image` is configured, templates could render a default gray silhouette avatar.

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
link = "https://alice.example.com"
email = "alice@example.com"
image = "/static/authors/alice.jpg"

[[socials]]
platform = "github"
url = "https://github.com/alice"
```

See also: [[frontmatter]], [[wiki-links]].
