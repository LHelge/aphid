---
title: Wiki Links
category: Content
tags:
  - reference
  - content
---

Wiki-links let any page reference any other page using only the filename stem — no full paths, no extensions.

# Basic syntax

```
[[slug]]
```

`slug` is the filename stem (the name without `.md`). For example, `[[configuration]]` links to `content/wiki/configuration.md`, `content/blog/configuration.md`, or `content/pages/configuration.md` — whichever exists.

# Alias form

```
[[slug|Display Text]]
```

The pipe separates the target slug from the link label shown to the reader. Use this when the page title is not the text you want to display:

```
See the [[configuration|config reference]] for details.
```

# Anchor links

Target a heading on the destination page by appending `#section` after the slug:

```
[[configuration#authors]]
[[configuration#authors|the authors section]]
```

The anchor text is slugified the same way as heading IDs (lowercased, non-alphanumerics replaced by `-`), so `[[configuration#Authors]]` and `[[configuration#authors]]` both resolve to the same fragment. To target an author-supplied id instead, use the literal id from the heading's `{#…}` attribute — see [[markdown#custom-ids]].

A bare cross-page anchor renders as `Page Title > section` so the reader sees which page they're heading to. Pipe-alias for a cleaner label when you don't want that:

| Source | Rendered text |
|--------|---------------|
| `[[configuration#authors]]` | `Configuration > authors` |
| `[[configuration#authors\|the authors section]]` | `the authors section` |

Anchor fragments are **not validated** against the destination page's headings — a missing anchor lands the reader at the top of the page, the same way any stale `#fragment` in an HTML link does. Only the slug portion has to resolve.

## Same-page anchors

Omit the slug to link within the current page:

```
[[#summary]]
```

Renders as `<a href="#summary">summary</a>`. Same-page anchors never count as broken links and never produce backlinks.

# Resolution rules

- The target is looked up by **filename stem** (Obsidian-style). The extension and directory are irrelevant.
- Resolution is global: blog posts, wiki pages, and standalone pages all share the same slug namespace.
- Two files with the same stem are a **hard error** in both `build` and `serve` — stems must be unique across the entire content tree.

# Slug normalization

Both filename stems and `[[…]]` targets are normalized to slugs before lookup: lowercased, with any non-alphanumeric character replaced by `-`, and consecutive hyphens collapsed.

| Filename or target | Slug |
|--------------------|------|
| `glossary.md` | `glossary` |
| `Battery Pack.md` | `battery-pack` |
| `[[Glossary]]` | resolves to `glossary` |
| `[[battery pack]]` | resolves to `battery-pack` |

Wiki and standalone-page slugs come from the filename stem this way. Blog posts are different: their slug is taken from the required `slug` field in frontmatter, so the filename can be a date prefix (`2026-04-23_aphid.md`) without affecting the URL.

# Broken links

| Mode | Behaviour |
|------|-----------|
| `aphid build` | Fails the build and reports every broken link |
| `aphid serve` | Warns to the terminal; renders the link as a `<span class="wikilink broken">` so writing can continue |

See also: [[frontmatter]], [[configuration]].
