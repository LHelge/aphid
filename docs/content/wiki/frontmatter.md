---
title: Frontmatter
category: Content
tags:
  - reference
---

Every content file begins with a YAML frontmatter block delimited by `---`. The required and optional fields differ by content type.

# Blog posts (`content/blog/`)

| Field | Required | Description |
|-------|----------|-------------|
| `title` | yes | Post title |
| `slug` | yes | URL segment — must be unique across all content |
| `author` | yes | Author name |
| `created` | yes | Publication date (`YYYY-MM-DD`) |
| `updated` | no | Last-edited date |
| `image` | no | Path or URL to a hero image, rendered above the post body and shown in blog listings |
| `description` | no | Short summary shown in blog listings (home page, blog index) |
| `tags` | no | List of tag strings |

```yaml
---
title: Hello World
slug: hello-world
author: Alice
created: 2026-01-01
tags:
  - rust
  - tutorial
---
```

Blog posts live at `/blog/<slug>/`.

# Wiki pages (`content/wiki/`)

All fields are optional. The page title falls back to the filename stem if `title` is omitted.

| Field | Required | Description |
|-------|----------|-------------|
| `title` | no | Page title |
| `category` | no | Category name — pages are grouped by category on the wiki index |
| `created` | no | Creation date |
| `updated` | no | Last-edited date |
| `tags` | no | List of tag strings |

```yaml
---
title: Glossary
category: Reference
tags:
  - reference
---
```

When `category` is set, the wiki index groups pages under category headings. Pages without a category appear in an "Uncategorized" section at the end.

Wiki pages live at `/wiki/<stem>/` regardless of category — the category is purely for display grouping.

# Standalone pages (`content/pages/`)

| Field | Required | Description |
|-------|----------|-------------|
| `title` | yes | Page title, shown in nav |
| `order` | no | Sort position in the nav (lower = earlier) |

```yaml
---
title: About
order: 1
---
```

Standalone pages live at `/<stem>/`.

See also: [[configuration]], [[wiki-links]].
