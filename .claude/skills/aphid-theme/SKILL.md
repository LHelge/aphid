---
name: aphid-theme
description: Reference for editing aphid themes. Use when modifying Tera templates under theme/templates/, designing layouts, working with template variables, or changing theme CSS and static assets.
---

This project uses aphid, a static site generator. Themes are directories containing Tera
templates (Jinja2-style) and optional static files. The goal is to design a complete theme.

# Theme directory layout

```
mytheme/
  theme.toml
  templates/
    base.html
    home.html
    blog_post.html
    blog_index.html
    wiki_page.html
    wiki_index.html
    page.html
    tag.html
    tags_index.html
    pagination.html
    404.html
  static/
    css/
    js/
```

`theme.toml` is required and must contain at least:

```toml
name = "mytheme"
version = "0.1.0"
```

`description` is optional.

# Template engine

Templates use Tera — a Jinja2-style engine. Key syntax:

- `{{ variable }}` — output a value
- `{{ variable | safe }}` — output HTML without escaping (required for rendered content)
- `{% block name %}...{% endblock %}` — define/override blocks
- `{% extends "base.html" %}` — inherit from a parent template
- `{% for item in list %}...{% endfor %}` — loops
- `{% if condition %}...{% elif %}...{% else %}...{% endif %}` — conditionals
- `{# comment #}` — comments

The standard pattern is a `base.html` layout that all other templates extend.

# Global variables (available in every template)

| Variable | Type | Description |
|----------|------|-------------|
| `site_title` | string | Site title from `aphid.toml` |
| `site_description` | string? | Site description from `aphid.toml` |
| `social_image_url` | string? | Default OpenGraph image URL |
| `version` | string | The aphid binary version |
| `nav_pages` | list | Standalone pages sorted by `order`; each has `title` and `url` |
| `socials` | list | Social links from `aphid.toml`; each has `platform` and `url` |
| `favicon_tags` | string | Pre-rendered `<link rel="icon">` tags |
| `feed_atom_url` | string | URL for the Atom feed |
| `feed_rss_url` | string | URL for the RSS feed |

# base.html

The root layout. All other templates extend this. Must define blocks that child templates
override. Typically contains `<html>`, `<head>`, navigation, header, footer.

Example skeleton:

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{% block page_title %}{{ site_title }}{% endblock %}</title>
  <link rel="stylesheet" href="/static/css/theme.css">
  {{ favicon_tags | safe }}
</head>
<body>
  <nav>
    <a href="/">{{ site_title }}</a>
    {% for page in nav_pages %}
      <a href="{{ page.url }}">{{ page.title }}</a>
    {% endfor %}
  </nav>
  <main>
    {% block content %}{% endblock %}
  </main>
  <footer>
    {% for social in socials %}
      <a href="{{ social.url }}">{{ social.platform }}</a>
    {% endfor %}
    <span>Built with aphid {{ version }}</span>
  </footer>
</body>
</html>
```

# home.html

Renders the site root (`/index.html`).

| Variable | Type | Description |
|----------|------|-------------|
| `posts` | list | All blog posts (see post entry shape below) |
| `home` | object? | Present when `content/home.md` exists; has `content` (rendered HTML — use `\| safe`) |
| `contains_mermaid` | bool | True if any rendered block uses Mermaid |

# blog_post.html

Renders a single blog post.

| Variable | Type | Description |
|----------|------|-------------|
| `title` | string | Post title |
| `url` | string | Clean URL, e.g. `/blog/my-post/` |
| `canonical_url` | string | Absolute canonical URL |
| `content` | string | Rendered HTML body — always use `\| safe` |
| `toc` | list | Heading entries; each has `level` (int), `text` (string), `id` (string) |
| `author` | object | `{ name, link?, image? }` |
| `image` | string? | Hero image path or URL |
| `og_image` | string? | Absolute social-share image URL |
| `description` | string? | Short summary |
| `created` | string | Publication date `YYYY-MM-DD` |
| `updated` | string? | Last-edited date |
| `reading_time_minutes` | int | Estimated read time |
| `tags` | list | Each has `name` and `slug` |
| `newer_post` | object? | `{ title, url }` for navigation |
| `older_post` | object? | `{ title, url }` for navigation |
| `contains_mermaid` | bool | |

# blog_index.html

Renders the blog listing at `/blog/`.

| Variable | Type | Description |
|----------|------|-------------|
| `posts` | list | Posts on this page (see post entry shape below) |
| `pagination` | object? | Pagination state |

# Post entry shape (used in home.html, blog_index.html, tag.html)

| Field | Type | Description |
|-------|------|-------------|
| `title` | string | Post title |
| `url` | string | Clean URL |
| `created` | string? | Publication date |
| `image` | string? | Hero image path or URL |
| `description` | string? | Short summary from frontmatter |
| `tags` | list | Each has `name` and `slug` |

# wiki_page.html

Renders a single wiki page. Has many of the same variables as `blog_post.html`, plus:

| Variable | Type | Description |
|----------|------|-------------|
| `category` | string? | Category name from frontmatter |
| `backlinks` | list | Pages linking here; each has `title` and `url` |
| `wiki_categories` | list | All wiki pages grouped by category — for a sidebar. Each entry has `name` (string or null) and `pages` (list of `{title, url}`). Named categories appear first; uncategorised pages appear under `name = null`. |

`author`, `image` are always absent on wiki pages. `created`, `updated`, `tags` are present
only if set in frontmatter.

# wiki_index.html

Renders the wiki listing at `/wiki/`.

| Variable | Type | Description |
|----------|------|-------------|
| `categories` | list | Same shape as `wiki_categories` on `wiki_page.html` |

# page.html

Renders standalone pages (About, Contact, etc.). Same variables as `blog_post.html`, but
`author`, `image`, `created`, `updated`, and `tags` are always absent.

# tag.html

Renders a single tag page.

| Variable | Type | Description |
|----------|------|-------------|
| `tag` | string | Tag display name |
| `tag_slug` | string | URL-safe slug |
| `posts` | list | Tagged posts (post entry shape) |
| `pagination` | object? | Pagination state |

# tags_index.html

Renders the tag listing at `/tags/`.

| Variable | Type | Description |
|----------|------|-------------|
| `tags` | list | All tags; each has `name`, `slug`, and `count` |

# pagination.html

Included by listing templates. Receives the surrounding template's `pagination` object.

# 404.html

Error page.

| Variable | Type | Description |
|----------|------|-------------|
| `not_found` | object? | Present when `content/404.md` exists; has `content` (use `\| safe`) |
| `contains_mermaid` | bool | |

# Static files and CSS

Place stylesheets, scripts, and other assets in `mytheme/static/`. They are copied to the
output's `static/` directory. Reference them with absolute paths:

```html
<link rel="stylesheet" href="/static/css/theme.css">
```

If the user's `static_dir` has a file with the same name, the user's version wins.

# Syntax highlighting CSS

Code blocks use CSS classes prefixed `hl-`. The theme stylesheet must define colors for
these classes. Key token classes:

- `hl-keyword` — language keywords (`fn`, `if`, `return`)
- `hl-string` — string literals
- `hl-comment` — comments
- `hl-type` — type names
- `hl-function` — function/method names
- `hl-number` — numeric literals
- `hl-operator` — operators
- `hl-punctuation` — brackets, commas, semicolons
- `hl-variable` — variable names
- `hl-attribute` — attributes/decorators
- `hl-tag` — HTML/XML tags
- `hl-entity` — entities and special names

Wrap code blocks in a container with `overflow-x: auto` for horizontal scrolling. Use a
monospace font and a background color that contrasts with the page.

# Mermaid diagrams

When `contains_mermaid` is true, the template should load `/static/js/mermaid.min.js`
(bundled by aphid) and initialise it. The renderer wraps Mermaid blocks in
`<pre class="mermaid">`, which Mermaid picks up on initialise.

# Design guidelines

- The page title is `<h1>`; body headings start at `<h2>` (the markdown pipeline shifts
  heading levels up by one).
- `content` is rendered HTML — use `{{ content | safe }}`.
- `toc` entries can build a table of contents sidebar or in-page nav.
- `backlinks` are most useful on wiki pages — show them in a footer or sidebar section.
- `wiki_categories` on `wiki_page.html` enables a sidebar showing all wiki pages grouped by
  category, with the current page highlighted (compare `page.url == url`).
- Test the theme against pages with: no image, no tags, no TOC, very long content, many
  backlinks, and the 404 page.
- Ensure the layout is responsive — test at mobile, tablet, and desktop widths.
