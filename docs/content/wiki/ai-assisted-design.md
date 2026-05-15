---
title: AI-Assisted Design
category: Customization
tags:
  - guide
  - ai
  - themes
---

If you use an AI design tool like Claude to create or modify an aphid theme, you can get better results by giving it the full picture of what a theme requires. This page provides a ready-made instruction block covering the theme format, required templates, and every variable available in the Tera template context.

For background on project-level instruction files and where each tool expects them, see [[ai-assisted-writing]].

> [!TIP]
> Run `aphid agent claude` (or `copilot`, `codex`) to write these files for you in the right location with the right frontmatter. See [[cli#aphid-agent]] for the full description.

# Instructions

Copy the block below into your tool's project-level instruction file.

````markdown
This project uses aphid, a static site generator. Themes are directories containing Tera
templates (Jinja2-style) and optional static files. The goal is to design a complete theme.

## Theme directory layout

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

## Template engine

Templates use Tera — a Jinja2-style engine. Key syntax:

- `{{ variable }}` — output a value
- `{{ variable | safe }}` — output HTML without escaping (required for rendered content)
- `{% block name %}...{% endblock %}` — define/override blocks
- `{% extends "base.html" %}` — inherit from a parent template
- `{% for item in list %}...{% endfor %}` — loops
- `{% if condition %}...{% elif %}...{% else %}...{% endif %}` — conditionals
- `{# comment #}` — comments

The standard pattern is a `base.html` layout that all other templates extend.

## Global variables (available in every template)

| Variable | Type | Description |
|----------|------|-------------|
| `site_title` | string | Site title from `aphid.toml` |
| `base_url` | string | Canonical root URL from `aphid.toml` |
| `version` | string | The aphid binary version |
| `nav_pages` | list | Standalone pages sorted by `order`; each has `title` and `url` |
| `socials` | list | Social links from `aphid.toml`; each has `platform` and `url` |

## base.html

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

## home.html

Renders the site root (`/index.html`).

| Variable | Type | Description |
|----------|------|-------------|
| `posts` | list | All blog posts (see post entry shape below) |
| `home` | object? | Present when `content/home.md` exists; has `content` (rendered HTML — use `| safe`) |

## blog_post.html

Renders a single blog post.

| Variable | Type | Description |
|----------|------|-------------|
| `title` | string | Post title |
| `url` | string | Clean URL, e.g. `/blog/my-post/` |
| `content` | string | Rendered HTML body — always use `| safe` |
| `toc` | list | Heading entries; each has `level` (int), `text` (string), `id` (string) |
| `backlinks` | list | Pages linking here; each has `title` and `url` |
| `author` | string? | Author name |
| `image` | string? | Hero image path or URL |
| `created` | string? | Publication date `YYYY-MM-DD` |
| `updated` | string? | Last-edited date |
| `tags` | list | Each has `name` and `slug` |

## blog_index.html

Renders the blog listing at `/blog/`.

| Variable | Type | Description |
|----------|------|-------------|
| `posts` | list | All blog posts (see post entry shape below) |

## Post entry shape (used in home.html, blog_index.html, tag.html)

| Field | Type | Description |
|-------|------|-------------|
| `title` | string | Post title |
| `url` | string | Clean URL |
| `created` | string? | Publication date |
| `image` | string? | Hero image path or URL |
| `description` | string? | Short summary from frontmatter |
| `tags` | list | Each has `name` and `slug` |

## wiki_page.html

Renders a single wiki page. Has the same variables as `blog_post.html`, plus:

| Variable | Type | Description |
|----------|------|-------------|
| `category` | string? | Category name from frontmatter |
| `wiki_categories` | list | All wiki pages grouped by category — for a sidebar. Each entry has `name` (string or null) and `pages` (list of `{title, url}`). Named categories appear first; uncategorised pages are grouped under `name = null` at the end. |

Note: `author` and `image` are always absent on wiki pages. `created`, `updated`, and `tags`
are present only if set in frontmatter.

## wiki_index.html

Renders the wiki listing at `/wiki/`.

| Variable | Type | Description |
|----------|------|-------------|
| `categories` | list | Same shape as `wiki_categories` on wiki_page.html |

## page.html

Renders standalone pages (About, Contact, etc.). Same variables as `blog_post.html`, but
`author`, `image`, `created`, `updated`, and `tags` are always absent.

## tag.html

Renders a single tag page.

| Variable | Type | Description |
|----------|------|-------------|
| `tag` | string | Tag display name |
| `tag_slug` | string | URL-safe slug |
| `posts` | list | Tagged posts; each has `title`, `url`, and `created?` |

## tags_index.html

Renders the tag listing at `/tags/`.

| Variable | Type | Description |
|----------|------|-------------|
| `tags` | list | All tags; each has `name`, `slug`, and `count` |

## 404.html

Error page. No additional variables beyond the global ones.

## Static files and CSS

Place stylesheets, scripts, and other assets in `mytheme/static/`. They are copied to the
output's `static/` directory. Reference them with absolute paths:

```html
<link rel="stylesheet" href="/static/css/theme.css">
```

If the user's `static_dir` has a file with the same name, the user's version wins.

## Syntax highlighting CSS

Code blocks use CSS classes prefixed `hl-`. The theme stylesheet must define colors for these
classes. Key token classes:

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

Wrap code blocks in a container with `overflow-x: auto` for horizontal scrolling.
Use a monospace font and a background color that contrasts with the page.

## Design guidelines

- The page title is `<h1>`, body headings start at `<h2>` (the markdown pipeline shifts levels)
- `content` is rendered HTML — use `{{ content | safe }}`
- `toc` entries can build a table of contents sidebar or in-page nav
- `backlinks` are most useful on wiki pages — show them in a footer or sidebar section
- `wiki_categories` on wiki_page.html enables a sidebar showing all wiki pages grouped by
  category, with the current page highlighted (compare `page.url == url`)
- Test the theme against pages with: no image, no tags, no TOC, very long content, and many
  backlinks
- Ensure the layout is responsive — test at mobile, tablet, and desktop widths
````

See also: [[themes]], [[markdown]], [[ai-assisted-writing]].
