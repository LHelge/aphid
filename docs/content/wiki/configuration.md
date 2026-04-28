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
| `source_dir` | `"content"` | Root of the Markdown content tree |
| `static_dir` | `"static"` | User static files copied to the output's `static/` directory |
| `theme_dir` | *(embedded)* | Path to a custom theme directory |
| `wiki_categories` | `[]` | Order for wiki category headings — see [Wiki category order](#wiki-category-order) |

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

By default, wiki categories on the wiki index are sorted alphabetically, with uncategorised pages last. To pin a specific order, list the categories in `wiki_categories`:

```toml
wiki_categories = ["Getting Started", "Content", "Customization", "Development"]
```

Categories listed here appear in this order. Any wiki category not in the list falls through to alphabetical placement after the listed ones, and uncategorised pages stay last. Adding a new category that isn't in `wiki_categories` is safe — it just shows up at the bottom until you order it.

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
source_dir = "content"

[[authors]]
name = "Alice"

[[socials]]
platform = "github"
url = "https://github.com/alice"
```

See also: [[frontmatter]], [[wiki-links]].
